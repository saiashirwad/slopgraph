use std::fs;
use std::path::{Path, PathBuf};

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    BindingPattern, Declaration, ExportAllDeclaration, ExportDeclaration, ExportDefaultDeclaration,
    ExportDefaultDeclarationKind, ExportFromDeclaration, ExportNamedDeclaration, ExportSpecifier,
    ImportDeclaration, ImportDeclarationSpecifier, ModuleExportName, Statement,
};
use oxc::parser::Parser;
use oxc::span::SourceType;

use crate::error::Error;
use crate::program::Program;

#[derive(Debug, Clone)]
pub struct ParsedModule {
    pub abs: PathBuf,
    pub exports: Vec<ParsedExport>,
    pub imports: Vec<ParsedImport>,
}

#[derive(Debug, Clone)]
pub struct ParsedExport {
    pub name: String,
    pub display: String,
    pub line: u32,
    pub span_start: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedImport {
    pub specifier: String,
    pub names: Vec<ImportedName>,
}

#[derive(Debug, Clone)]
pub enum ImportedName {
    Named(String),
    Default,
    Namespace,
    StarReexport,
}

pub fn parse_program(program: &Program) -> Result<Vec<ParsedModule>, Error> {
    let mut modules = Vec::with_capacity(program.files.len());
    for file in &program.files {
        modules.push(parse_file(file)?);
    }
    Ok(modules)
}

fn parse_file(path: &Path) -> Result<ParsedModule, Error> {
    let source = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    let allocator = Allocator::new();
    let source_type = SourceType::from_path(path)
        .unwrap_or_else(|_| SourceType::ts())
        .with_module(true);
    let parsed = Parser::new(&allocator, &source, source_type).parse();

    let mut exports = Vec::new();
    let mut imports = Vec::new();
    for stmt in &parsed.program.body {
        collect_statement(stmt, &source, &mut exports, &mut imports);
    }

    Ok(ParsedModule {
        abs: path.to_path_buf(),
        exports,
        imports,
    })
}

fn collect_statement(
    stmt: &Statement<'_>,
    source: &str,
    exports: &mut Vec<ParsedExport>,
    imports: &mut Vec<ParsedImport>,
) {
    match stmt {
        Statement::ImportDeclaration(decl) => collect_import(decl, imports),
        Statement::ExportDeclaration(decl) => collect_export_declaration(decl, source, exports),
        Statement::ExportNamedDeclaration(decl) => collect_export_named(decl, source, exports),
        Statement::ExportDefaultDeclaration(decl) => collect_export_default(decl, source, exports),
        Statement::ExportFromDeclaration(decl) => {
            collect_export_from(decl, source, exports, imports)
        }
        Statement::ExportAllDeclaration(decl) => collect_export_all(decl, source, exports, imports),
        _ => {}
    }
}

fn collect_import(decl: &ImportDeclaration<'_>, imports: &mut Vec<ParsedImport>) {
    let specifier = decl.source.value.to_string();
    let Some(specifiers) = &decl.specifiers else {
        return;
    };
    let mut names = Vec::new();
    for spec in specifiers {
        match spec {
            ImportDeclarationSpecifier::ImportSpecifier(s) => {
                names.push(ImportedName::Named(export_name(&s.imported)));
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
                names.push(ImportedName::Default);
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
                names.push(ImportedName::Namespace);
            }
        }
    }
    if !names.is_empty() {
        imports.push(ParsedImport { specifier, names });
    }
}

fn collect_export_declaration(
    decl: &ExportDeclaration<'_>,
    source: &str,
    exports: &mut Vec<ParsedExport>,
) {
    exports.extend(names_from_declaration(&decl.declaration, source));
}

fn collect_export_named(
    decl: &ExportNamedDeclaration<'_>,
    source: &str,
    exports: &mut Vec<ParsedExport>,
) {
    for spec in &decl.specifiers {
        exports.push(from_export_specifier(spec, source));
    }
}

fn collect_export_default(
    decl: &ExportDefaultDeclaration<'_>,
    source: &str,
    exports: &mut Vec<ParsedExport>,
) {
    let (display, span_start) = match &decl.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(f) => match &f.id {
            Some(id) => (id.name.to_string(), id.span.start),
            None => ("default".to_string(), decl.span.start),
        },
        ExportDefaultDeclarationKind::ClassDeclaration(c) => match &c.id {
            Some(id) => (id.name.to_string(), id.span.start),
            None => ("default".to_string(), decl.span.start),
        },
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(i) => {
            (i.id.name.to_string(), i.id.span.start)
        }
        _ => ("default".to_string(), decl.span.start),
    };
    exports.push(ParsedExport {
        name: "default".to_string(),
        display,
        line: line_at(source, span_start),
        span_start,
    });
}

fn collect_export_from(
    decl: &ExportFromDeclaration<'_>,
    source: &str,
    exports: &mut Vec<ParsedExport>,
    imports: &mut Vec<ParsedImport>,
) {
    let specifier = decl.source.value.to_string();
    let mut names = Vec::new();
    for spec in &decl.specifiers {
        exports.push(from_export_specifier(spec, source));
        names.push(ImportedName::Named(export_name(&spec.local)));
    }
    if !names.is_empty() {
        imports.push(ParsedImport { specifier, names });
    }
}

fn collect_export_all(
    decl: &ExportAllDeclaration<'_>,
    source: &str,
    exports: &mut Vec<ParsedExport>,
    imports: &mut Vec<ParsedImport>,
) {
    let specifier = decl.source.value.to_string();
    if let Some(exported) = &decl.exported {
        let name = export_name(exported);
        let span_start = name_span_start(exported);
        exports.push(ParsedExport {
            name: name.clone(),
            display: name,
            line: line_at(source, span_start),
            span_start,
        });
        imports.push(ParsedImport {
            specifier,
            names: vec![ImportedName::Namespace],
        });
    } else {
        imports.push(ParsedImport {
            specifier,
            names: vec![ImportedName::StarReexport],
        });
    }
}

fn from_export_specifier(spec: &ExportSpecifier<'_>, source: &str) -> ParsedExport {
    let name = export_name(&spec.exported);
    let span_start = name_span_start(&spec.exported);
    ParsedExport {
        name: name.clone(),
        display: name,
        line: line_at(source, span_start),
        span_start,
    }
}

fn names_from_declaration(decl: &Declaration<'_>, source: &str) -> Vec<ParsedExport> {
    match decl {
        Declaration::FunctionDeclaration(f) => {
            f.id.as_ref()
                .map(|id| vec![named_export(&id.name, id.span.start, source)])
                .unwrap_or_default()
        }
        Declaration::ClassDeclaration(c) => {
            c.id.as_ref()
                .map(|id| vec![named_export(&id.name, id.span.start, source)])
                .unwrap_or_default()
        }
        Declaration::VariableDeclaration(v) => v
            .declarations
            .iter()
            .flat_map(|d| binding_exports(&d.id, source))
            .collect(),
        Declaration::TSTypeAliasDeclaration(t) => {
            vec![named_export(&t.id.name, t.id.span.start, source)]
        }
        Declaration::TSInterfaceDeclaration(i) => {
            vec![named_export(&i.id.name, i.id.span.start, source)]
        }
        Declaration::TSEnumDeclaration(e) => {
            vec![named_export(&e.id.name, e.id.span.start, source)]
        }
        _ => Vec::new(),
    }
}

fn binding_exports(pat: &BindingPattern<'_>, source: &str) -> Vec<ParsedExport> {
    match pat {
        BindingPattern::BindingIdentifier(id) => {
            vec![named_export(&id.name, id.span.start, source)]
        }
        _ => Vec::new(),
    }
}

fn named_export(name: &str, span_start: u32, source: &str) -> ParsedExport {
    ParsedExport {
        name: name.to_string(),
        display: name.to_string(),
        line: line_at(source, span_start),
        span_start,
    }
}

fn export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(id) => id.name.to_string(),
        ModuleExportName::IdentifierReference(id) => id.name.to_string(),
        ModuleExportName::StringLiteral(s) => s.value.to_string(),
    }
}

fn name_span_start(name: &ModuleExportName<'_>) -> u32 {
    match name {
        ModuleExportName::IdentifierName(id) => id.span.start,
        ModuleExportName::IdentifierReference(id) => id.span.start,
        ModuleExportName::StringLiteral(s) => s.span.start,
    }
}

fn line_at(source: &str, offset: u32) -> u32 {
    let mut off = (offset as usize).min(source.len());
    while off > 0 && !source.is_char_boundary(off) {
        off -= 1;
    }
    source[..off].bytes().filter(|&b| b == b'\n').count() as u32 + 1
}
