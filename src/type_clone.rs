use std::fs;
use std::path::{Path, PathBuf};

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    PropertyKey, TSFunctionType, TSInterfaceDeclaration, TSLiteral, TSSignature, TSTupleElement,
    TSType, TSTypeAliasDeclaration, TSTypeName, TSTypeQueryExprName,
};
use oxc::ast_visit::{walk, Visit};
use oxc::parser::Parser;
use oxc::span::SourceType;

use crate::call_graph::CallGraph;
use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::parse::line_at;
use crate::program::display_path;
use crate::Options;

pub const MIN_FIELDS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Field {
    pub name: String,
    pub optional: bool,
    pub readonly: bool,
    pub canonical_type: String,
}

#[derive(Debug, Clone)]
pub struct TypeCandidate {
    pub abs: PathBuf,
    pub display: PathBuf,
    pub name: String,
    pub line: u32,
    pub span_start: u32,
    pub has_extends: bool,
    pub fields: Vec<Field>,
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
        PropertyKey::Identifier(id) => Some(id.name.to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.to_string()),
        PropertyKey::NumericLiteral(n) => Some(n.raw.map(|r| r.to_string()).unwrap_or_else(|| n.value.to_string())),
        PropertyKey::PrivateIdentifier(id) => Some(id.name.to_string()),
        _ => None,
    }
}

fn type_name_to_string(name: &TSTypeName<'_>) -> String {
    match name {
        TSTypeName::IdentifierReference(id) => id.name.to_string(),
        TSTypeName::QualifiedName(q) => {
            format!("{}.{}", type_name_to_string(&q.left), q.right.name)
        }
        TSTypeName::ThisExpression(_) => "this".to_string(),
    }
}

fn type_query_expr_to_string(name: &TSTypeQueryExprName<'_>) -> String {
    match name {
        TSTypeQueryExprName::IdentifierReference(id) => id.name.to_string(),
        TSTypeQueryExprName::QualifiedName(q) => {
            format!("{}.{}", type_name_to_string(&q.left), q.right.name)
        }
        TSTypeQueryExprName::ThisExpression(_) => "this".to_string(),
        TSTypeQueryExprName::TSImportType(i) => format!("import(\"{}\")", i.source.value),
    }
}

pub fn canonicalize_type(ty: &TSType<'_>) -> String {
    match ty {
        TSType::TSAnyKeyword(_) => "any".to_string(),
        TSType::TSBigIntKeyword(_) => "bigint".to_string(),
        TSType::TSBooleanKeyword(_) => "boolean".to_string(),
        TSType::TSNeverKeyword(_) => "never".to_string(),
        TSType::TSNullKeyword(_) => "null".to_string(),
        TSType::TSNumberKeyword(_) => "number".to_string(),
        TSType::TSObjectKeyword(_) => "object".to_string(),
        TSType::TSStringKeyword(_) => "string".to_string(),
        TSType::TSSymbolKeyword(_) => "symbol".to_string(),
        TSType::TSUndefinedKeyword(_) => "undefined".to_string(),
        TSType::TSUnknownKeyword(_) => "unknown".to_string(),
        TSType::TSVoidKeyword(_) => "void".to_string(),
        TSType::TSTypeReference(r) => {
            let base = type_name_to_string(&r.type_name);
            if let Some(params) = &r.type_arguments {
                let args: Vec<String> = params.params.iter().map(canonicalize_type).collect();
                format!("{}<{}>", base, args.join(", "))
            } else {
                base
            }
        }
        TSType::TSArrayType(arr) => {
            format!("{}[]", canonicalize_type(&arr.element_type))
        }
        TSType::TSUnionType(_) => {
            let mut parts = Vec::new();
            collect_union_types(ty, &mut parts);
            parts.sort();
            parts.dedup();
            parts.join(" | ")
        }
        TSType::TSIntersectionType(_) => {
            let mut parts = Vec::new();
            collect_intersection_types(ty, &mut parts);
            parts.sort();
            parts.dedup();
            parts.join(" & ")
        }
        TSType::TSParenthesizedType(p) => canonicalize_type(&p.type_annotation),
        TSType::TSLiteralType(lit) => match &lit.literal {
            TSLiteral::BooleanLiteral(b) => {
                if b.value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            TSLiteral::NumericLiteral(n) => {
                n.raw.map(|r| r.to_string()).unwrap_or_else(|| n.value.to_string())
            }
            TSLiteral::BigIntLiteral(b) => format!("{}n", b.raw.as_deref().unwrap_or("0")),
            TSLiteral::StringLiteral(s) => format!("\"{}\"", s.value),
            TSLiteral::TemplateLiteral(t) => {
                let quasis: Vec<_> = t.quasis.iter().map(|q| q.value.raw.as_str()).collect();
                format!("`{}`", quasis.join(""))
            }
            TSLiteral::UnaryExpression(u) => format!("{:?}", u.operator),
        },
        TSType::TSTypeLiteral(lit) => {
            let mut fields = Vec::new();
            for member in &lit.members {
                if let Some(f) = extract_field_from_signature(member) {
                    fields.push(f);
                }
            }
            fields.sort();
            let field_strs: Vec<String> = fields
                .into_iter()
                .map(|f| {
                    let opt = if f.optional { "?" } else { "" };
                    let ro = if f.readonly { "readonly " } else { "" };
                    format!("{}{}{}: {}", ro, f.name, opt, f.canonical_type)
                })
                .collect();
            format!("{{ {} }}", field_strs.join("; "))
        }
        TSType::TSFunctionType(func) => format_function_type(func),
        TSType::TSTupleType(tup) => {
            let elems: Vec<String> = tup.element_types.iter().map(canonicalize_tuple_element).collect();
            format!("[{}]", elems.join(", "))
        }
        TSType::TSTypeQuery(q) => {
            format!("typeof {}", type_query_expr_to_string(&q.expr_name))
        }
        _ => "unknown".to_string(),
    }
}

fn canonicalize_tuple_element(elem: &TSTupleElement<'_>) -> String {
    match elem {
        TSTupleElement::TSOptionalType(o) => format!("{}?", canonicalize_type(&o.type_annotation)),
        TSTupleElement::TSRestType(r) => format!("...{}", canonicalize_type(&r.type_annotation)),
        TSTupleElement::TSNamedTupleMember(m) => {
            let opt = if m.optional { "?" } else { "" };
            format!("{}{}: {}", m.label.name, opt, canonicalize_tuple_element(&m.element_type))
        }
        _ => {
            if let Some(ty) = elem.as_ts_type() {
                canonicalize_type(ty)
            } else {
                "unknown".to_string()
            }
        }
    }
}

fn collect_union_types(ty: &TSType<'_>, out: &mut Vec<String>) {
    if let TSType::TSUnionType(u) = ty {
        for t in &u.types {
            collect_union_types(t, out);
        }
    } else if let TSType::TSParenthesizedType(p) = ty {
        collect_union_types(&p.type_annotation, out);
    } else {
        out.push(canonicalize_type(ty));
    }
}

fn collect_intersection_types(ty: &TSType<'_>, out: &mut Vec<String>) {
    if let TSType::TSIntersectionType(i) = ty {
        for t in &i.types {
            collect_intersection_types(t, out);
        }
    } else if let TSType::TSParenthesizedType(p) = ty {
        collect_intersection_types(&p.type_annotation, out);
    } else {
        out.push(canonicalize_type(ty));
    }
}

fn format_function_type(func: &TSFunctionType<'_>) -> String {
    let params: Vec<String> = func
        .params
        .items
        .iter()
        .map(|param| {
            let name = match &param.pattern {
                oxc::ast::ast::BindingPattern::BindingIdentifier(id) => id.name.as_str(),
                _ => "_",
            };
            let opt = if param.optional { "?" } else { "" };
            let ty_str = param
                .type_annotation
                .as_ref()
                .map(|t| canonicalize_type(&t.type_annotation))
                .unwrap_or_else(|| "any".to_string());
            format!("{}{}: {}", name, opt, ty_str)
        })
        .collect();
    let ret = canonicalize_type(&func.return_type.type_annotation);
    format!("({}) => {}", params.join(", "), ret)
}

fn extract_field_from_signature(sig: &TSSignature<'_>) -> Option<Field> {
    match sig {
        TSSignature::TSPropertySignature(prop) => {
            let name = property_key_name(&prop.key)?;
            let canonical_type = prop
                .type_annotation
                .as_ref()
                .map(|t| canonicalize_type(&t.type_annotation))
                .unwrap_or_else(|| "any".to_string());
            Some(Field {
                name,
                optional: prop.optional,
                readonly: prop.readonly,
                canonical_type,
            })
        }
        TSSignature::TSMethodSignature(method) => {
            let name = property_key_name(&method.key)?;
            let params: Vec<String> = method
                .params
                .items
                .iter()
                .map(|param| {
                    let pname = match &param.pattern {
                        oxc::ast::ast::BindingPattern::BindingIdentifier(id) => id.name.as_str(),
                        _ => "_",
                    };
                    let opt = if param.optional { "?" } else { "" };
                    let ty_str = param
                        .type_annotation
                        .as_ref()
                        .map(|t| canonicalize_type(&t.type_annotation))
                        .unwrap_or_else(|| "any".to_string());
                    format!("{}{}: {}", pname, opt, ty_str)
                })
                .collect();
            let ret = method
                .return_type
                .as_ref()
                .map(|t| canonicalize_type(&t.type_annotation))
                .unwrap_or_else(|| "any".to_string());
            let canonical_type = format!("({}) => {}", params.join(", "), ret);
            Some(Field {
                name,
                optional: method.optional,
                readonly: false,
                canonical_type,
            })
        }
        _ => None,
    }
}

struct TypeVisitor<'s> {
    source: &'s str,
    abs: PathBuf,
    display: PathBuf,
    candidates: Vec<TypeCandidate>,
}

impl<'a, 's> Visit<'a> for TypeVisitor<'s> {
    fn visit_ts_interface_declaration(&mut self, decl: &TSInterfaceDeclaration<'a>) {
        let name = decl.id.name.to_string();
        let span_start = decl.id.span.start;
        let line = line_at(self.source, span_start);
        let has_extends = !decl.extends.is_empty();

        let mut fields = Vec::new();
        for member in &decl.body.body {
            if let Some(field) = extract_field_from_signature(member) {
                fields.push(field);
            }
        }
        fields.sort();

        self.candidates.push(TypeCandidate {
            abs: self.abs.clone(),
            display: self.display.clone(),
            name,
            line,
            span_start,
            has_extends,
            fields,
        });

        walk::walk_ts_interface_declaration(self, decl);
    }

    fn visit_ts_type_alias_declaration(&mut self, decl: &TSTypeAliasDeclaration<'a>) {
        let name = decl.id.name.to_string();
        let span_start = decl.id.span.start;
        let line = line_at(self.source, span_start);

        match &decl.type_annotation {
            TSType::TSTypeLiteral(lit) => {
                let mut fields = Vec::new();
                for member in &lit.members {
                    if let Some(field) = extract_field_from_signature(member) {
                        fields.push(field);
                    }
                }
                fields.sort();

                self.candidates.push(TypeCandidate {
                    abs: self.abs.clone(),
                    display: self.display.clone(),
                    name,
                    line,
                    span_start,
                    has_extends: false,
                    fields,
                });
            }
            TSType::TSIntersectionType(_) => {
                self.candidates.push(TypeCandidate {
                    abs: self.abs.clone(),
                    display: self.display.clone(),
                    name,
                    line,
                    span_start,
                    has_extends: true,
                    fields: Vec::new(),
                });
            }
            _ => {}
        }

        walk::walk_ts_type_alias_declaration(self, decl);
    }
}

pub fn collect_types_in_file(abs: &Path, display: &Path, source: &str) -> Vec<TypeCandidate> {
    let allocator = Allocator::new();
    let source_type = SourceType::from_path(abs)
        .unwrap_or_else(|_| SourceType::ts())
        .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();

    let mut visitor = TypeVisitor {
        source,
        abs: abs.to_path_buf(),
        display: display.to_path_buf(),
        candidates: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    visitor.candidates
}

/// Detect type clone pairs according to spec:
/// Two distinct types with the same field names and the same field types,
/// no `extends` link, and at least 3 fields.
pub fn detect(
    modules: &ModuleGraph,
    _calls: &CallGraph,
    _options: &Options,
) -> Vec<Finding> {
    let mut candidates = Vec::new();

    let mut file_paths: Vec<_> = modules.modules.keys().cloned().collect();
    file_paths.sort();

    for abs in file_paths {
        let Ok(source) = fs::read_to_string(&abs) else {
            continue;
        };
        let display = display_path(&modules.root, &abs);
        let file_candidates = collect_types_in_file(&abs, &display, &source);
        for candidate in file_candidates {
            if !candidate.has_extends && candidate.fields.len() >= MIN_FIELDS {
                candidates.push(candidate);
            }
        }
    }

    candidates.sort_by(|a, b| {
        a.display
            .cmp(&b.display)
            .then(a.span_start.cmp(&b.span_start))
            .then(a.name.cmp(&b.name))
    });

    let mut findings = Vec::new();
    let n = candidates.len();

    for i in 0..n {
        for j in (i + 1)..n {
            let type_a = &candidates[i];
            let type_b = &candidates[j];

            // Must be distinct types
            if type_a.name == type_b.name && type_a.abs == type_b.abs {
                continue;
            }

            // Both must not have extends
            if type_a.has_extends || type_b.has_extends {
                continue;
            }

            // Both must have >= 3 fields
            if type_a.fields.len() < MIN_FIELDS || type_b.fields.len() < MIN_FIELDS {
                continue;
            }

            // Must have identical fields
            if type_a.fields != type_b.fields {
                continue;
            }

            findings.push(Finding {
                shape: Shape::TypeClone,
                location: Location {
                    file: type_a.display.clone(),
                    line: type_a.line,
                    span_start: type_a.span_start,
                },
                subject: type_a.name.clone(),
                evidence: Evidence::Path {
                    nodes: vec![
                        PathNode {
                            label: type_a.name.clone(),
                            annotation: None,
                            is_subject: true,
                        },
                        PathNode {
                            label: type_b.name.clone(),
                            annotation: None,
                            is_subject: false,
                        },
                    ],
                },
            });
        }
    }

    findings.sort_by(|a, b| {
        a.location
            .file
            .cmp(&b.location.file)
            .then(a.location.span_start.cmp(&b.location.span_start))
            .then(a.subject.cmp(&b.subject))
    });

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze_types(files: &[(&str, &str)]) -> Vec<Finding> {
        let mut candidates = Vec::new();
        for (rel_path, source) in files {
            let path = PathBuf::from(rel_path);
            let file_candidates = collect_types_in_file(&path, &path, source);
            for c in file_candidates {
                if !c.has_extends && c.fields.len() >= MIN_FIELDS {
                    candidates.push(c);
                }
            }
        }

        candidates.sort_by(|a, b| {
            a.display
                .cmp(&b.display)
                .then(a.span_start.cmp(&b.span_start))
                .then(a.name.cmp(&b.name))
        });

        let mut findings = Vec::new();
        let n = candidates.len();

        for i in 0..n {
            for j in (i + 1)..n {
                let type_a = &candidates[i];
                let type_b = &candidates[j];

                if type_a.name == type_b.name && type_a.abs == type_b.abs {
                    continue;
                }
                if type_a.has_extends || type_b.has_extends {
                    continue;
                }
                if type_a.fields.len() < MIN_FIELDS || type_b.fields.len() < MIN_FIELDS {
                    continue;
                }
                if type_a.fields != type_b.fields {
                    continue;
                }

                findings.push(Finding {
                    shape: Shape::TypeClone,
                    location: Location {
                        file: type_a.display.clone(),
                        line: type_a.line,
                        span_start: type_a.span_start,
                    },
                    subject: type_a.name.clone(),
                    evidence: Evidence::Path {
                        nodes: vec![
                            PathNode {
                                label: type_a.name.clone(),
                                annotation: None,
                                is_subject: true,
                            },
                            PathNode {
                                label: type_b.name.clone(),
                                annotation: None,
                                is_subject: false,
                            },
                        ],
                    },
                });
            }
        }

        findings.sort_by(|a, b| {
            a.location
                .file
                .cmp(&b.location.file)
                .then(a.location.span_start.cmp(&b.location.span_start))
                .then(a.subject.cmp(&b.subject))
        });

        findings
    }

    #[test]
    fn test_interface_pair_positive() {
        let code = r#"
        interface UserA {
            id: string;
            name: string;
            age: number;
        }
        interface UserB {
            id: string;
            name: string;
            age: number;
        }
        "#;
        let findings = analyze_types(&[("types.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].shape, Shape::TypeClone);
        assert_eq!(findings[0].subject, "UserA");

        let Evidence::Path { nodes } = &findings[0].evidence;
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].label, "UserA");
        assert!(nodes[0].is_subject);
        assert_eq!(nodes[1].label, "UserB");
        assert!(!nodes[1].is_subject);
    }

    #[test]
    fn test_type_alias_and_interface_positive() {
        let code = r#"
        interface ProductInfo {
            sku: string;
            price: number;
            inStock: boolean;
        }
        type ProductData = {
            sku: string;
            price: number;
            inStock: boolean;
        };
        "#;
        let findings = analyze_types(&[("products.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "ProductInfo");

        let Evidence::Path { nodes } = &findings[0].evidence;
        assert_eq!(nodes[0].label, "ProductInfo");
        assert_eq!(nodes[1].label, "ProductData");
    }

    #[test]
    fn test_type_alias_pair_positive() {
        let code = r#"
        type ConfigA = {
            host: string;
            port: number;
            ssl: boolean;
        };
        type ConfigB = {
            host: string;
            port: number;
            ssl: boolean;
        };
        "#;
        let findings = analyze_types(&[("config.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "ConfigA");
    }

    #[test]
    fn test_field_permutation_order_independent() {
        let code = r#"
        interface CoordinateXYZ {
            x: number;
            y: number;
            z: number;
        }
        interface CoordinateZYX {
            z: number;
            y: number;
            x: number;
        }
        "#;
        let findings = analyze_types(&[("coords.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "CoordinateXYZ");
    }

    #[test]
    fn test_union_order_independent() {
        let code = r#"
        interface UnionTypeA {
            id: string;
            value: string | number;
            active: boolean;
        }
        interface UnionTypeB {
            id: string;
            value: number | string;
            active: boolean;
        }
        "#;
        let findings = analyze_types(&[("union.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "UnionTypeA");
    }

    #[test]
    fn test_intersection_order_independent() {
        let code = r#"
        interface Foo {}
        interface Bar {}
        interface InterA {
            id: string;
            mix: Foo & Bar;
            flag: boolean;
        }
        interface InterB {
            id: string;
            mix: Bar & Foo;
            flag: boolean;
        }
        "#;
        let findings = analyze_types(&[("inter.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "InterA");
    }

    #[test]
    fn test_fewer_than_3_fields_rejected() {
        let code = r#"
        interface PairA {
            a: string;
            b: number;
        }
        interface PairB {
            a: string;
            b: number;
        }
        interface SingleA {
            value: string;
        }
        interface SingleB {
            value: string;
        }
        "#;
        let findings = analyze_types(&[("pairs.ts", code)]);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_extends_interface_rejected() {
        let code = r#"
        interface Base {
            id: string;
            name: string;
            email: string;
        }
        interface Sub extends Base {
            id: string;
            name: string;
            email: string;
        }
        "#;
        let findings = analyze_types(&[("extends.ts", code)]);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_intersection_type_alias_rejected() {
        let code = r#"
        type Base = {
            id: string;
        };
        type Sub = Base & {
            name: string;
            email: string;
            role: string;
        };
        "#;
        let path = PathBuf::from("inter_alias.ts");
        let candidates = collect_types_in_file(&path, &path, code);
        let sub_candidate = candidates.iter().find(|c| c.name == "Sub").unwrap();
        assert!(sub_candidate.has_extends);
    }

    #[test]
    fn test_optionality_mismatch_rejected() {
        let code = r#"
        interface Req {
            a: string;
            b: number;
            c: boolean;
        }
        interface Opt {
            a?: string;
            b: number;
            c: boolean;
        }
        "#;
        let findings = analyze_types(&[("opt.ts", code)]);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_readonly_mismatch_rejected() {
        let code = r#"
        interface Mutable {
            a: string;
            b: number;
            c: boolean;
        }
        interface Immutable {
            readonly a: string;
            b: number;
            c: boolean;
        }
        "#;
        let findings = analyze_types(&[("ro.ts", code)]);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_field_type_mismatch_rejected() {
        let code = r#"
        interface NumType {
            a: number;
            b: string;
            c: boolean;
        }
        interface StrType {
            a: string;
            b: string;
            c: boolean;
        }
        "#;
        let findings = analyze_types(&[("types.ts", code)]);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_field_name_mismatch_rejected() {
        let code = r#"
        interface XYZ {
            x: number;
            y: number;
            z: number;
        }
        interface UVW {
            u: number;
            v: number;
            w: number;
        }
        "#;
        let findings = analyze_types(&[("coords.ts", code)]);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_nested_type_literal_order_independence() {
        let code = r#"
        interface NestedA {
            id: string;
            meta: { a: number; b: string };
            valid: boolean;
        }
        interface NestedB {
            id: string;
            meta: { b: string; a: number };
            valid: boolean;
        }
        "#;
        let findings = analyze_types(&[("nested.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "NestedA");
    }

    #[test]
    fn test_method_signatures_matching() {
        let code = r#"
        interface ServiceA {
            init(config: string): void;
            run(timeout: number): boolean;
            stop(): void;
        }
        interface ServiceB {
            init(config: string): void;
            run(timeout: number): boolean;
            stop(): void;
        }
        "#;
        let findings = analyze_types(&[("service.ts", code)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "ServiceA");
    }

    #[test]
    fn test_cross_file_type_clones() {
        let file1 = r#"
        export interface ConfigDTO {
            host: string;
            port: number;
            tls: boolean;
        }
        "#;
        let file2 = r#"
        export interface ServerConfig {
            host: string;
            port: number;
            tls: boolean;
        }
        "#;
        let findings = analyze_types(&[("src/a.ts", file1), ("src/b.ts", file2)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "ConfigDTO");

        let Evidence::Path { nodes } = &findings[0].evidence;
        assert_eq!(nodes[0].label, "ConfigDTO");
        assert_eq!(nodes[1].label, "ServerConfig");
    }
}

