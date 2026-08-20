use std::collections::HashMap;
use std::fs;
use std::path::Path;

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    ArrowFunctionExpression, BindingPattern, CallExpression, Expression,
    Function, FunctionType, IdentifierReference,
};
use oxc::ast_visit::{walk, Visit};
use oxc::parser::Parser;
use oxc::span::{SourceType, Span};
use oxc::syntax::scope::ScopeFlags;

use crate::call_graph::CallGraph;
use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::parse::line_at;
use crate::Options;

/// Recursively unwraps transparent wrappers around an expression.
pub fn unwrap_expression<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match expr {
        Expression::ParenthesizedExpression(p) => unwrap_expression(&p.expression),
        Expression::TSAsExpression(a) => unwrap_expression(&a.expression),
        Expression::TSTypeAssertion(t) => unwrap_expression(&t.expression),
        Expression::TSNonNullExpression(n) => unwrap_expression(&n.expression),
        Expression::TSInstantiationExpression(i) => unwrap_expression(&i.expression),
        Expression::TSSatisfiesExpression(s) => unwrap_expression(&s.expression),
        _ => expr,
    }
}

/// Checks whether an expression is a direct reference to the specified parameter name.
pub fn is_direct_param_reference(expr: &Expression<'_>, param_name: &str) -> bool {
    let unwrapped = unwrap_expression(expr);
    match unwrapped {
        Expression::Identifier(ident) => ident.name.as_str() == param_name,
        _ => false,
    }
}

/// Visitor that collects all identifier references to a parameter name
/// and all call expression starts where the parameter is directly forwarded as an argument.
pub struct ParamUsageVisitor<'s> {
    pub param_name: &'s str,
    pub all_refs: Vec<Span>,
    pub forward_calls: Vec<u32>,
}

impl<'a, 's> Visit<'a> for ParamUsageVisitor<'s> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        for arg in &call.arguments {
            if let Some(expr) = arg.as_expression() {
                if is_direct_param_reference(expr, self.param_name) {
                    self.forward_calls.push(call.span.start);
                }
            }
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_identifier_reference(&mut self, ident: &IdentifierReference<'a>) {
        if ident.name.as_str() == self.param_name {
            self.all_refs.push(ident.span);
        }
        walk::walk_identifier_reference(self, ident);
    }
}

struct FileCollector<'s, 'c> {
    source: &'s str,
    abs: &'s Path,
    calls: &'c CallGraph,
    by_key: &'c HashMap<(&'c Path, u32), usize>,
    findings: Vec<Finding>,
}

impl<'s, 'c> FileCollector<'s, 'c> {
    fn inspect_param(
        &mut self,
        from_idx: usize,
        param_name: &str,
        param_span: Span,
        all_refs: Vec<Span>,
        forward_calls: Vec<u32>,
    ) {
        if all_refs.is_empty() || forward_calls.is_empty() {
            return;
        }
        // If there are more identifier references than forwarding argument uses,
        // the parameter was read locally (e.g. param.x, if (param), param + 1, return param, etc.).
        if all_refs.len() != forward_calls.len() {
            return;
        }

        let mut target_indices = Vec::new();
        let mut all_typed = true;

        for &call_start in &forward_calls {
            let matched_edges: Vec<_> = self
                .calls
                .edges
                .iter()
                .filter(|e| e.from == from_idx && e.call_start == call_start)
                .collect();
            if matched_edges.is_empty() {
                all_typed = false;
                break;
            }
            for edge in matched_edges {
                if !target_indices.contains(&edge.to) {
                    target_indices.push(edge.to);
                }
            }
        }

        if !all_typed || target_indices.is_empty() {
            return;
        }

        let caller_fn = &self.calls.functions[from_idx];
        let param_line = line_at(self.source, param_span.start);

        for target_idx in target_indices {
            let target_fn = &self.calls.functions[target_idx];
            self.findings.push(Finding {
                shape: Shape::TrampData,
                location: Location {
                    file: caller_fn.display.clone(),
                    line: param_line,
                    span_start: param_span.start,
                },
                subject: param_name.to_string(),
                evidence: Evidence::Path {
                    nodes: vec![
                        PathNode {
                            label: caller_fn.name.clone(),
                            annotation: Some(format!("passes {param_name}")),
                            is_subject: true,
                        },
                        PathNode {
                            label: target_fn.name.clone(),
                            annotation: None,
                            is_subject: false,
                        },
                    ],
                },
            });
        }
    }
}

impl<'a, 's, 'c> Visit<'a> for FileCollector<'s, 'c> {
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

        if let Some(&from_idx) = self.by_key.get(&(self.abs, span_start)) {
            if let Some(body) = &func.body {
                for param in &func.params.items {
                    if let BindingPattern::BindingIdentifier(binding_id) = &param.pattern {
                        let param_name = binding_id.name.as_str();
                        let mut visitor = ParamUsageVisitor {
                            param_name,
                            all_refs: Vec::new(),
                            forward_calls: Vec::new(),
                        };
                        visitor.visit_function_body(body);
                        self.inspect_param(
                            from_idx,
                            param_name,
                            binding_id.span,
                            visitor.all_refs,
                            visitor.forward_calls,
                        );
                    }
                }
            }
        }

        walk::walk_function(self, func, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let span_start = arrow.span.start;

        if let Some(&from_idx) = self.by_key.get(&(self.abs, span_start)) {
            for param in &arrow.params.items {
                if let BindingPattern::BindingIdentifier(binding_id) = &param.pattern {
                    let param_name = binding_id.name.as_str();
                    let mut visitor = ParamUsageVisitor {
                        param_name,
                        all_refs: Vec::new(),
                        forward_calls: Vec::new(),
                    };
                    visitor.visit_arrow_function_body(&arrow.body);
                    self.inspect_param(
                        from_idx,
                        param_name,
                        binding_id.span,
                        visitor.all_refs,
                        visitor.forward_calls,
                    );
                }
            }
        }

        walk::walk_arrow_function_expression(self, arrow);
    }
}

/// Detect tramp data parameters according to spec:
/// A parameter that a function does not read locally, except as an argument to a call, on a typed call path.
pub fn detect(modules: &ModuleGraph, calls: &CallGraph, _options: &Options) -> Vec<Finding> {
    let mut by_key: HashMap<(&Path, u32), usize> = HashMap::new();
    for (i, func) in calls.functions.iter().enumerate() {
        by_key.insert((func.abs.as_path(), func.span_start), i);
    }

    let mut file_paths: Vec<_> = modules.modules.keys().cloned().collect();
    file_paths.sort();

    let mut findings = Vec::new();

    for abs in file_paths {
        let Ok(source) = fs::read_to_string(&abs) else {
            continue;
        };
        let allocator = Allocator::new();
        let source_type = SourceType::from_path(&abs)
            .unwrap_or_else(|_| SourceType::ts())
            .with_module(true);
        let parsed = Parser::new(&allocator, &source, source_type).parse();

        let mut collector = FileCollector {
            source: &source,
            abs: &abs,
            calls,
            by_key: &by_key,
            findings: Vec::new(),
        };
        collector.visit_program(&parsed.program);
        findings.extend(collector.findings);
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
    use crate::call_graph::{FnNode, TypedEdge};
    use std::path::PathBuf;

    fn analyze_code(source: &str, edges: Vec<TypedEdge>, functions: Vec<FnNode>) -> Vec<Finding> {
        let allocator = Allocator::new();
        let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();

        let file_path = PathBuf::from("test.ts");
        let calls = CallGraph {
            functions,
            edges,
            calls: 1,
            resolved: 1,
        };

        let mut by_key: HashMap<(&Path, u32), usize> = HashMap::new();
        for (i, func) in calls.functions.iter().enumerate() {
            by_key.insert((func.abs.as_path(), func.span_start), i);
        }

        let mut collector = FileCollector {
            source,
            abs: &file_path,
            calls: &calls,
            by_key: &by_key,
            findings: Vec::new(),
        };
        collector.visit_program(&parsed.program);
        collector.findings
    }

    #[test]
    fn test_clean_forward_detected() {
        let source = r#"
function intermediate(param: string) {
    target(param);
}
"#;
        let file_path = PathBuf::from("test.ts");
        let intermediate_span = source.find("intermediate").unwrap() as u32;
        let target_call_start = source.find("target(param)").unwrap() as u32;

        let functions = vec![
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "intermediate".to_string(),
                line: 2,
                span_start: intermediate_span,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "target".to_string(),
                line: 10,
                span_start: 100,
                exported: false,
                forward: None,
            },
        ];

        let edges = vec![TypedEdge {
            from: 0,
            to: 1,
            call_start: target_call_start,
        }];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].shape, Shape::TrampData);
        assert_eq!(findings[0].subject, "param");
        assert_eq!(findings[0].location.line, 2);

        let Evidence::Path { nodes } = &findings[0].evidence;
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].label, "intermediate");
        assert_eq!(nodes[0].annotation.as_deref(), Some("passes param"));
        assert!(nodes[0].is_subject);
        assert_eq!(nodes[1].label, "target");
        assert_eq!(nodes[1].annotation, None);
        assert!(!nodes[1].is_subject);
    }

    #[test]
    fn test_forward_with_type_assertions_detected() {
        let source = r#"
function intermediate(param: any) {
    target(((param as string)!));
}
"#;
        let file_path = PathBuf::from("test.ts");
        let intermediate_span = source.find("intermediate").unwrap() as u32;
        let target_call_start = source.find("target").unwrap() as u32;

        let functions = vec![
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "intermediate".to_string(),
                line: 2,
                span_start: intermediate_span,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "target".to_string(),
                line: 10,
                span_start: 100,
                exported: false,
                forward: None,
            },
        ];

        let edges = vec![TypedEdge {
            from: 0,
            to: 1,
            call_start: target_call_start,
        }];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "param");
    }

    #[test]
    fn test_property_read_rejected() {
        let source = r#"
function intermediate(param: { id: string }) {
    console.log(param.id);
    target(param);
}
"#;
        let file_path = PathBuf::from("test.ts");
        let intermediate_span = source.find("intermediate").unwrap() as u32;
        let target_call_start = source.find("target(param)").unwrap() as u32;

        let functions = vec![
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "intermediate".to_string(),
                line: 2,
                span_start: intermediate_span,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "target".to_string(),
                line: 10,
                span_start: 100,
                exported: false,
                forward: None,
            },
        ];

        let edges = vec![TypedEdge {
            from: 0,
            to: 1,
            call_start: target_call_start,
        }];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_condition_read_rejected() {
        let source = r#"
function intermediate(param: string | null) {
    if (param) {
        target(param);
    }
}
"#;
        let file_path = PathBuf::from("test.ts");
        let intermediate_span = source.find("intermediate").unwrap() as u32;
        let target_call_start = source.find("target(param)").unwrap() as u32;

        let functions = vec![
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "intermediate".to_string(),
                line: 2,
                span_start: intermediate_span,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "target".to_string(),
                line: 10,
                span_start: 100,
                exported: false,
                forward: None,
            },
        ];

        let edges = vec![TypedEdge {
            from: 0,
            to: 1,
            call_start: target_call_start,
        }];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_untyped_call_rejected() {
        let source = r#"
function intermediate(param: string) {
    console.log(param);
}
"#;
        let file_path = PathBuf::from("test.ts");
        let intermediate_span = source.find("intermediate").unwrap() as u32;

        let functions = vec![FnNode {
            abs: file_path.clone(),
            display: file_path.clone(),
            name: "intermediate".to_string(),
            line: 2,
            span_start: intermediate_span,
            exported: false,
            forward: None,
        }];

        // No typed edges in CallGraph for console.log
        let edges = vec![];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_unused_parameter_rejected() {
        let source = r#"
function intermediate(param: string) {
    target();
}
"#;
        let file_path = PathBuf::from("test.ts");
        let intermediate_span = source.find("intermediate").unwrap() as u32;
        let target_call_start = source.find("target()").unwrap() as u32;

        let functions = vec![
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "intermediate".to_string(),
                line: 2,
                span_start: intermediate_span,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "target".to_string(),
                line: 10,
                span_start: 100,
                exported: false,
                forward: None,
            },
        ];

        let edges = vec![TypedEdge {
            from: 0,
            to: 1,
            call_start: target_call_start,
        }];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_callee_use_rejected() {
        let source = r#"
function intermediate(param: () => void) {
    param();
}
"#;
        let file_path = PathBuf::from("test.ts");
        let intermediate_span = source.find("intermediate").unwrap() as u32;

        let functions = vec![FnNode {
            abs: file_path.clone(),
            display: file_path.clone(),
            name: "intermediate".to_string(),
            line: 2,
            span_start: intermediate_span,
            exported: false,
            forward: None,
        }];

        let edges = vec![];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_multiple_params_selective() {
        let source = r#"
function process(readParam: string, passParam: number, unusedParam: boolean) {
    if (readParam.length > 0) {
        target(passParam);
    }
}
"#;
        let file_path = PathBuf::from("test.ts");
        let process_span = source.find("process").unwrap() as u32;
        let target_call_start = source.find("target(passParam)").unwrap() as u32;

        let functions = vec![
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "process".to_string(),
                line: 2,
                span_start: process_span,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "target".to_string(),
                line: 10,
                span_start: 100,
                exported: false,
                forward: None,
            },
        ];

        let edges = vec![TypedEdge {
            from: 0,
            to: 1,
            call_start: target_call_start,
        }];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "passParam");
    }

    #[test]
    fn test_arrow_function_forward_detected() {
        let source = r#"
const intermediate = (param: string) => {
    target(param);
};
"#;
        let file_path = PathBuf::from("test.ts");
        let arrow_span = source.find("(param: string)").unwrap() as u32;
        let target_call_start = source.find("target(param)").unwrap() as u32;

        let functions = vec![
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "default".to_string(),
                line: 2,
                span_start: arrow_span,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: file_path.clone(),
                display: file_path.clone(),
                name: "target".to_string(),
                line: 10,
                span_start: 100,
                exported: false,
                forward: None,
            },
        ];

        let edges = vec![TypedEdge {
            from: 0,
            to: 1,
            call_start: target_call_start,
        }];

        let findings = analyze_code(source, edges, functions);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "param");
    }
}
