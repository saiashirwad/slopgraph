use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    ArrowFunctionBody, ArrowFunctionExpression, CallExpression, Expression, Function, FunctionBody,
    FunctionType, Statement,
};
use oxc::ast_visit::{walk, Visit};
use oxc::parser::Parser;
use oxc::span::SourceType;
use oxc::syntax::scope::ScopeFlags;

use crate::error::Error;
use crate::parse::line_at;
use crate::program::{display_path, is_program_file, Program};
use crate::tsgo::{self, TsNode, Tsgo};

/// A function-like in the program, located by file and UTF-8 span start.
#[derive(Debug, Clone)]
pub struct FnNode {
    pub abs: PathBuf,
    pub display: PathBuf,
    pub name: String,
    pub line: u32,
    pub span_start: u32,
    #[allow(dead_code)]
    pub exported: bool,
    pub forward: Option<Forward>,
}

/// Body is only a typed-edge candidate call.
#[derive(Debug, Clone)]
pub struct Forward {
    pub call_start: u32,
    pub return_only: bool,
}

/// Caller → callee on a resolved typed edge.
#[derive(Debug, Clone)]
pub struct TypedEdge {
    pub from: usize,
    pub to: usize,
    pub call_start: u32,
}

pub struct CallGraph {
    pub functions: Vec<FnNode>,
    pub edges: Vec<TypedEdge>,
    /// Call-like nodes seen in oxc, and how many resolved to a project function.
    pub calls: usize,
    pub resolved: usize,
}

struct ParsedFn {
    name: String,
    line: u32,
    span_start: u32,
    exported: bool,
    forward: Option<Forward>,
}

struct ParsedCall {
    start: u32,
    enclosing: Option<u32>,
}

pub fn build(program: &Program) -> Result<CallGraph, Error> {
    let mut tsgo = Tsgo::spawn(&program.root)?;
    let snap = tsgo.open_project(&program.tsconfig_path)?;

    let mut functions = Vec::new();
    let mut parsed_calls: Vec<(PathBuf, String, Vec<ParsedCall>, Vec<TsNode>)> = Vec::new();

    for abs in &program.files {
        let source = fs::read_to_string(abs).map_err(|e| Error::io(abs, e))?;
        let display = display_path(&program.root, abs);
        let (fns, calls) = collect(abs, &source)?;
        for func in fns {
            functions.push(FnNode {
                abs: abs.clone(),
                display: display.clone(),
                name: func.name,
                line: func.line,
                span_start: func.span_start,
                exported: func.exported,
                forward: func.forward,
            });
        }
        let nodes = tsgo.source_nodes(&snap, abs)?;
        parsed_calls.push((abs.clone(), source, calls, nodes));
    }

    let mut by_key: HashMap<(PathBuf, u32), usize> = HashMap::new();
    for (i, func) in functions.iter().enumerate() {
        by_key.insert((func.abs.clone(), func.span_start), i);
    }

    let mut edges = Vec::new();
    let mut calls = 0usize;
    let mut resolved = 0usize;

    for (abs, source, file_calls, nodes) in &parsed_calls {
        let canonical = tsgo.canonical_path(abs);
        for call in file_calls {
            calls += 1;
            let utf16 = tsgo::utf16_offset(source, call.start);
            let Some(ts_call) =
                tsgo::tightest_containing(nodes, &[tsgo::KIND_CALL_EXPRESSION], utf16)
            else {
                continue;
            };
            let handle = tsgo::node_handle(ts_call.index, ts_call.kind, &canonical);
            let Some(sig) = tsgo.resolved_signature(&snap, &handle)? else {
                continue;
            };
            let Some(decl) = sig.declaration.as_deref() else {
                continue;
            };
            if !sig.is_resolved() {
                continue;
            }
            let Some((index, _, decl_path)) = tsgo::parse_handle(decl) else {
                continue;
            };
            let decl_file = PathBuf::from(&decl_path);
            if !is_program_file(&decl_file) {
                continue;
            }
            let Some(decl_nodes) = parsed_calls
                .iter()
                .find(|(p, _, _, _)| paths_match(p, &decl_file, tsgo.case_sensitive))
            else {
                continue;
            };
            let Some(decl_node) = tsgo::node_by_index(&decl_nodes.3, index) else {
                continue;
            };
            let decl_source = &decl_nodes.1;
            let Some(to) = functions.iter().enumerate().find_map(|(i, func)| {
                if !paths_match(&func.abs, &decl_file, tsgo.case_sensitive) {
                    return None;
                }
                let start16 = tsgo::utf16_offset(decl_source, func.span_start);
                if decl_node.pos <= start16 && start16 < decl_node.end {
                    Some(i)
                } else {
                    None
                }
            }) else {
                continue;
            };
            resolved += 1;
            if let Some(from_start) = call.enclosing {
                if let Some(&from) = by_key.get(&(abs.clone(), from_start)) {
                    edges.push(TypedEdge {
                        from,
                        to,
                        call_start: call.start,
                    });
                }
            }
        }
    }

    Ok(CallGraph {
        functions,
        edges,
        calls,
        resolved,
    })
}

fn paths_match(a: &Path, b: &Path, case_sensitive: bool) -> bool {
    if a == b {
        return true;
    }
    let sa = a.to_string_lossy().replace('\\', "/");
    let sb = b.to_string_lossy().replace('\\', "/");
    if case_sensitive {
        sa == sb
    } else {
        sa.eq_ignore_ascii_case(&sb)
    }
}

fn collect(path: &Path, source: &str) -> Result<(Vec<ParsedFn>, Vec<ParsedCall>), Error> {
    let allocator = Allocator::new();
    let source_type = SourceType::from_path(path)
        .unwrap_or_else(|_| SourceType::ts())
        .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let mut visitor = Collector {
        source,
        functions: Vec::new(),
        calls: Vec::new(),
        enclosing: Vec::new(),
        export_depth: 0,
    };
    visitor.visit_program(&parsed.program);
    Ok((visitor.functions, visitor.calls))
}

struct Collector<'s> {
    source: &'s str,
    functions: Vec<ParsedFn>,
    calls: Vec<ParsedCall>,
    enclosing: Vec<u32>,
    export_depth: u32,
}

impl Collector<'_> {
    fn push_fn(&mut self, name: String, span_start: u32, exported: bool, forward: Option<Forward>) {
        self.functions.push(ParsedFn {
            name,
            line: line_at(self.source, span_start),
            span_start,
            exported: exported || self.export_depth > 0,
            forward,
        });
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_export_named_declaration(&mut self, it: &oxc::ast::ast::ExportNamedDeclaration<'a>) {
        self.export_depth += 1;
        walk::walk_export_named_declaration(self, it);
        self.export_depth -= 1;
    }

    fn visit_export_default_declaration(
        &mut self,
        it: &oxc::ast::ast::ExportDefaultDeclaration<'a>,
    ) {
        self.export_depth += 1;
        walk::walk_export_default_declaration(self, it);
        self.export_depth -= 1;
    }

    fn visit_function(&mut self, func: &Function<'a>, flags: ScopeFlags) {
        if matches!(
            func.r#type,
            FunctionType::TSDeclareFunction | FunctionType::TSEmptyBodyFunctionExpression
        ) {
            walk::walk_function(self, func, flags);
            return;
        }
        let span_start = func
            .id
            .as_ref()
            .map(|id| id.span.start)
            .unwrap_or(func.span.start);
        let name = func
            .id
            .as_ref()
            .map(|id| id.name.to_string())
            .unwrap_or_else(|| "default".to_string());
        let forward = func.body.as_deref().and_then(forward_from_block);
        self.push_fn(name, span_start, false, forward);
        self.enclosing.push(span_start);
        walk::walk_function(self, func, flags);
        self.enclosing.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let span_start = arrow.span.start;
        let forward = forward_from_arrow(&arrow.body);
        self.push_fn("default".to_string(), span_start, false, forward);
        self.enclosing.push(span_start);
        walk::walk_arrow_function_expression(self, arrow);
        self.enclosing.pop();
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        self.calls.push(ParsedCall {
            start: call.span.start,
            enclosing: self.enclosing.last().copied(),
        });
        walk::walk_call_expression(self, call);
    }
}

fn forward_from_block(body: &FunctionBody<'_>) -> Option<Forward> {
    match body.statements.as_slice() {
        [Statement::ReturnStatement(ret)] => {
            as_forward_call(ret.argument.as_ref()?).map(|call| Forward {
                call_start: call.span.start,
                return_only: true,
            })
        }
        [Statement::ExpressionStatement(stmt)] => {
            as_forward_call(&stmt.expression).map(|call| Forward {
                call_start: call.span.start,
                return_only: false,
            })
        }
        _ => None,
    }
}

fn forward_from_arrow(body: &ArrowFunctionBody<'_>) -> Option<Forward> {
    match body {
        ArrowFunctionBody::FunctionBody(block) => forward_from_block(block),
        _ => None,
    }
}

fn as_forward_call<'a>(expr: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expr {
        Expression::CallExpression(call) => Some(call),
        Expression::TSAsExpression(as_expr) => as_forward_call(&as_expr.expression),
        Expression::TSTypeAssertion(assert) => as_forward_call(&assert.expression),
        Expression::ParenthesizedExpression(paren) => as_forward_call(&paren.expression),
        _ => None,
    }
}
