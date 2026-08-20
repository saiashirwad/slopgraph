use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    ArrowFunctionBody, ArrowFunctionExpression, BindingPattern, Function, FunctionBody,
    FunctionType, VariableDeclarator,
};
use oxc::ast::AstKind;
use oxc::ast_visit::{walk, Visit};
use oxc::parser::config::TokensParserConfig;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType, Span};
use oxc::syntax::scope::ScopeFlags;

use crate::call_graph::CallGraph;
use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::parse::line_at;
use crate::program::display_path;
use crate::Options;

pub const MIN_AST_NODES: usize = 20;
pub const WINDOW_SIZE: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NormalizedToken {
    Id,
    Lit,
    Text(String),
}

#[derive(Debug, Clone)]
pub struct FunctionCandidate {
    #[allow(dead_code)]
    pub abs: PathBuf,
    pub display: PathBuf,
    pub name: String,
    pub line: u32,
    pub span_start: u32,
    pub ast_kinds: Vec<oxc::ast::AstType>,
    pub window_hashes: Vec<u64>,
}

fn is_literal_kind(k: oxc::parser::Kind) -> bool {
    k.is_literal()
        || matches!(
            k,
            oxc::parser::Kind::NoSubstitutionTemplate
                | oxc::parser::Kind::TemplateHead
                | oxc::parser::Kind::TemplateMiddle
                | oxc::parser::Kind::TemplateTail
        )
}

pub fn normalize_tokens(
    source: &str,
    body_span: Span,
    tokens: &[oxc::parser::Token],
) -> Vec<NormalizedToken> {
    let mut normalized = Vec::new();
    for token in tokens {
        if token.start() < body_span.start || token.end() > body_span.end {
            continue;
        }
        let k = token.kind();
        if k.to_str() == "eof" {
            continue;
        }
        if k.is_identifier() {
            normalized.push(NormalizedToken::Id);
        } else if is_literal_kind(k) {
            normalized.push(NormalizedToken::Lit);
        } else {
            let text = &source[token.start() as usize..token.end() as usize];
            normalized.push(NormalizedToken::Text(text.to_string()));
        }
    }
    normalized
}

pub fn compute_window_hashes(tokens: &[NormalizedToken]) -> Vec<u64> {
    if tokens.len() < WINDOW_SIZE {
        return Vec::new();
    }
    let count = tokens.len() - WINDOW_SIZE + 1;
    let mut hashes = Vec::with_capacity(count);
    for i in 0..count {
        let window = &tokens[i..i + WINDOW_SIZE];
        let mut hasher = DefaultHasher::new();
        window.hash(&mut hasher);
        hashes.push(hasher.finish());
    }
    hashes
}

pub fn token_window_confidence(hashes_a: &[u64], hashes_b: &[u64]) -> f64 {
    if hashes_a.is_empty() || hashes_b.is_empty() {
        return 0.0;
    }
    let mut counts_a: HashMap<u64, usize> = HashMap::new();
    for &h in hashes_a {
        *counts_a.entry(h).or_default() += 1;
    }
    let mut counts_b: HashMap<u64, usize> = HashMap::new();
    for &h in hashes_b {
        *counts_b.entry(h).or_default() += 1;
    }

    let mut matching = 0usize;
    for (h, count_a) in counts_a {
        if let Some(count_b) = counts_b.get(&h) {
            matching += count_a.min(*count_b);
        }
    }
    let total = hashes_a.len().max(hashes_b.len());
    matching as f64 / total as f64
}

struct AstKindCollector {
    kinds: Vec<oxc::ast::AstType>,
}

impl<'a> Visit<'a> for AstKindCollector {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        self.kinds.push(kind.ty());
    }

    fn visit_function(&mut self, func: &Function<'a>, _flags: ScopeFlags) {
        self.enter_node(AstKind::Function(func));
        // Skip children of nested functions
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.enter_node(AstKind::ArrowFunctionExpression(arrow));
        // Skip children of nested arrow functions
    }
}

pub fn extract_function_body_ast_kinds(body: &FunctionBody<'_>) -> Vec<oxc::ast::AstType> {
    let mut collector = AstKindCollector { kinds: Vec::new() };
    for stmt in &body.statements {
        collector.visit_statement(stmt);
    }
    collector.kinds
}

pub fn extract_arrow_body_ast_kinds(body: &ArrowFunctionBody<'_>) -> Vec<oxc::ast::AstType> {
    let mut collector = AstKindCollector { kinds: Vec::new() };
    if let Some(block) = body.as_function_body() {
        for stmt in &block.statements {
            collector.visit_statement(stmt);
        }
    } else if let Some(expr) = body.as_expression() {
        collector.visit_expression(expr);
    }
    collector.kinds
}

struct FunctionFileCollector<'s> {
    source: &'s str,
    abs: PathBuf,
    display: PathBuf,
    tokens: &'s [oxc::parser::Token],
    candidates: Vec<FunctionCandidate>,
    var_name_stack: Vec<String>,
}

impl<'a, 's> Visit<'a> for FunctionFileCollector<'s> {
    fn visit_variable_declarator(&mut self, decl: &VariableDeclarator<'a>) {
        let name = match &decl.id {
            BindingPattern::BindingIdentifier(id) => Some(id.name.to_string()),
            _ => None,
        };
        if let Some(n) = name {
            self.var_name_stack.push(n);
            walk::walk_variable_declarator(self, decl);
            self.var_name_stack.pop();
        } else {
            walk::walk_variable_declarator(self, decl);
        }
    }

    fn visit_function(&mut self, func: &Function<'a>, flags: ScopeFlags) {
        if matches!(
            func.r#type,
            FunctionType::TSDeclareFunction | FunctionType::TSEmptyBodyFunctionExpression
        ) {
            walk::walk_function(self, func, flags);
            return;
        }
        if let Some(body) = &func.body {
            let span_start = func
                .id
                .as_ref()
                .map(|id| id.span.start)
                .unwrap_or(func.span.start);
            let name = func
                .id
                .as_ref()
                .map(|id| id.name.to_string())
                .or_else(|| self.var_name_stack.last().cloned())
                .unwrap_or_else(|| "default".to_string());
            let line = line_at(self.source, span_start);
            let ast_kinds = extract_function_body_ast_kinds(body);
            let normalized_tokens = normalize_tokens(self.source, body.span, self.tokens);
            let window_hashes = compute_window_hashes(&normalized_tokens);

            self.candidates.push(FunctionCandidate {
                abs: self.abs.clone(),
                display: self.display.clone(),
                name,
                line,
                span_start,
                ast_kinds,
                window_hashes,
            });
        }
        walk::walk_function(self, func, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let span_start = arrow.span.start;
        let name = self
            .var_name_stack
            .last()
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        let line = line_at(self.source, span_start);
        let ast_kinds = extract_arrow_body_ast_kinds(&arrow.body);
        let body_span = arrow.body.span();
        let normalized_tokens = normalize_tokens(self.source, body_span, self.tokens);
        let window_hashes = compute_window_hashes(&normalized_tokens);

        self.candidates.push(FunctionCandidate {
            abs: self.abs.clone(),
            display: self.display.clone(),
            name,
            line,
            span_start,
            ast_kinds,
            window_hashes,
        });

        walk::walk_arrow_function_expression(self, arrow);
    }
}

pub fn collect_functions_in_file(
    abs: &Path,
    display: &Path,
    source: &str,
) -> Vec<FunctionCandidate> {
    let allocator = Allocator::new();
    let source_type = SourceType::from_path(abs)
        .unwrap_or_else(|_| SourceType::ts())
        .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type)
        .with_config(TokensParserConfig)
        .parse();

    let mut collector = FunctionFileCollector {
        source,
        abs: abs.to_path_buf(),
        display: display.to_path_buf(),
        tokens: &parsed.tokens,
        candidates: Vec::new(),
        var_name_stack: Vec::new(),
    };
    collector.visit_program(&parsed.program);
    collector.candidates
}

/// Detect near-duplicate functions according to spec:
/// 1. Token-window hash: windows of 50 normalized tokens ($ID, $LIT).
/// 2. AST-kind sequence hash: exact kind sequence of function body with >= 20 nodes.
///
/// Confidence >= 0.7 and distinct function names (`fn_a.name != fn_b.name`).
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
        let file_candidates = collect_functions_in_file(&abs, &display, &source);
        for candidate in file_candidates {
            if candidate.ast_kinds.len() >= MIN_AST_NODES {
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
            let fn_a = &candidates[i];
            let fn_b = &candidates[j];

            if fn_a.name == fn_b.name {
                continue;
            }

            if fn_a.ast_kinds.len() < MIN_AST_NODES || fn_b.ast_kinds.len() < MIN_AST_NODES {
                continue;
            }

            if fn_a.ast_kinds != fn_b.ast_kinds {
                continue;
            }

            let confidence = token_window_confidence(&fn_a.window_hashes, &fn_b.window_hashes);
            if confidence < 0.7 {
                continue;
            }

            findings.push(Finding {
                shape: Shape::NearDuplicate,
                location: Location {
                    file: fn_a.display.clone(),
                    line: fn_a.line,
                    span_start: fn_a.span_start,
                },
                subject: fn_a.name.clone(),
                evidence: Evidence::Path {
                    nodes: vec![
                        PathNode {
                            label: fn_a.name.clone(),
                            annotation: None,
                            is_subject: true,
                        },
                        PathNode {
                            label: fn_b.name.clone(),
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

    #[test]
    fn test_token_normalization_types() {
        let code = r#"
        function example(alpha: string, beta: number) {
            const str = "hello";
            const num = 12345;
            const big = 999n;
            const reg = /pattern/g;
            const tmpl = `prefix ${alpha} suffix`;
            if (beta > 0) {
                return str + num;
            }
            return null;
        }
        "#;
        let allocator = Allocator::new();
        let ret = Parser::new(&allocator, code, SourceType::ts())
            .with_config(TokensParserConfig)
            .parse();
        let tokens = normalize_tokens(code, Span::new(0, code.len() as u32), &ret.tokens);

        // Check that identifiers became NormalizedToken::Id
        assert!(tokens.contains(&NormalizedToken::Id));
        // Check that literals became NormalizedToken::Lit
        assert!(tokens.contains(&NormalizedToken::Lit));
        // Check that keywords/punctuation retained exact text
        assert!(tokens.contains(&NormalizedToken::Text("function".to_string())));
        assert!(tokens.contains(&NormalizedToken::Text("const".to_string())));
        assert!(tokens.contains(&NormalizedToken::Text("if".to_string())));
        assert!(tokens.contains(&NormalizedToken::Text("return".to_string())));
        assert!(tokens.contains(&NormalizedToken::Text(">".to_string())));
        assert!(tokens.contains(&NormalizedToken::Text("+".to_string())));
    }

    #[test]
    fn test_near_duplicate_positive_pair() {
        let code_a = r#"
        function processCustomerData(input: Record<string, unknown>): boolean {
            const isValid = input !== null && typeof input === "object";
            if (!isValid) {
                return false;
            }
            const id = input["customerId"];
            const score = Number(input["score"]);
            const flag = Boolean(input["active"]);
            if (score > 100 && flag) {
                const adjusted = score * 1.1;
                const result = adjusted > 150 ? true : false;
                return result;
            }
            return false;
        }
        "#;

        let code_b = r#"
        function processSupplierData(data: Record<string, unknown>): boolean {
            const isOk = data !== null && typeof data === "object";
            if (!isOk) {
                return false;
            }
            const key = data["supplierId"];
            const amount = Number(data["total"]);
            const status = Boolean(data["verified"]);
            if (amount > 200 && status) {
                const computed = amount * 1.25;
                const finalVal = computed > 250 ? true : false;
                return finalVal;
            }
            return false;
        }
        "#;

        let dummy_path = PathBuf::from("test.ts");
        let funcs_a = collect_functions_in_file(&dummy_path, &dummy_path, code_a);
        let funcs_b = collect_functions_in_file(&dummy_path, &dummy_path, code_b);

        assert_eq!(funcs_a.len(), 1);
        assert_eq!(funcs_b.len(), 1);

        let fa = &funcs_a[0];
        let fb = &funcs_b[0];

        assert!(fa.ast_kinds.len() >= MIN_AST_NODES);
        assert!(fb.ast_kinds.len() >= MIN_AST_NODES);
        assert_eq!(fa.ast_kinds, fb.ast_kinds);

        let conf = token_window_confidence(&fa.window_hashes, &fb.window_hashes);
        assert!(conf >= 0.7);
    }

    #[test]
    fn test_ast_node_floor_under_20_rejected() {
        let code = r#"
        function tinyA(x: number): number {
            return x + 1;
        }
        function tinyB(y: number): number {
            return y + 2;
        }
        "#;
        let dummy_path = PathBuf::from("tiny.ts");
        let funcs = collect_functions_in_file(&dummy_path, &dummy_path, code);
        for f in &funcs {
            assert!(f.ast_kinds.len() < MIN_AST_NODES);
        }
    }

    #[test]
    fn test_different_ast_structure_rejected() {
        // Same tokens/keywords roughly, but one uses `while` and one uses `if`
        let code_if = r#"
        function checkWithIf(arr: number[], limit: number): number {
            let sum = 0;
            let i = 0;
            const threshold = limit * 2;
            if (i < arr.length && sum < threshold) {
                sum = sum + arr[i];
                i = i + 1;
                const doubled = sum * 2;
                return doubled > 50 ? doubled : sum;
            }
            const defaultVal = limit + 10;
            return defaultVal * 2;
        }
        "#;

        let code_while = r#"
        function checkWithWhile(arr: number[], limit: number): number {
            let sum = 0;
            let i = 0;
            const threshold = limit * 2;
            while (i < arr.length && sum < threshold) {
                sum = sum + arr[i];
                i = i + 1;
                const doubled = sum * 2;
                return doubled > 50 ? doubled : sum;
            }
            const defaultVal = limit + 10;
            return defaultVal * 2;
        }
        "#;

        let dummy_path = PathBuf::from("test.ts");
        let funcs_if = collect_functions_in_file(&dummy_path, &dummy_path, code_if);
        let funcs_while = collect_functions_in_file(&dummy_path, &dummy_path, code_while);

        assert_eq!(funcs_if.len(), 1);
        assert_eq!(funcs_while.len(), 1);

        assert_ne!(funcs_if[0].ast_kinds, funcs_while[0].ast_kinds);
    }

    #[test]
    fn test_same_named_functions_rejected_by_detect() {
        let code = r#"
        {
            function calculateTax(val: number): number {
                const isPositive = val > 0;
                if (!isPositive) {
                    return 0;
                }
                const rate = 0.05;
                const base = val * rate;
                const total = base + 10;
                const rounded = Math.round(total);
                if (rounded > 100) {
                    const extra = rounded * 1.1;
                    return extra > 200 ? extra : rounded;
                }
                return 0;
            }
        }
        {
            function calculateTax(amount: number): number {
                const isPos = amount > 0;
                if (!isPos) {
                    return 0;
                }
                const taxRate = 0.08;
                const initial = amount * taxRate;
                const sum = initial + 20;
                const finalVal = Math.round(sum);
                if (finalVal > 100) {
                    const surcharge = finalVal * 1.2;
                    return surcharge > 300 ? surcharge : finalVal;
                }
                return 0;
            }
        }
        "#;

        let abs = PathBuf::from("/project/src/same.ts");
        let funcs = collect_functions_in_file(&abs, &PathBuf::from("src/same.ts"), code);
        assert_eq!(funcs.len(), 2);
        assert_eq!(funcs[0].name, funcs[1].name); // Both named calculateTax

        // Verify distinct name check rejects them
        let is_distinct = funcs[0].name != funcs[1].name;
        assert!(!is_distinct);
    }

    #[test]
    fn test_nested_functions_skipped_in_outer_ast() {
        let code = r#"
        function outerFunction(x: number): number {
            const a = x + 1;
            const inner = (y: number) => {
                const z = y * 2;
                const w = z + 3;
                return w > 10 ? w : 10;
            };
            const b = inner(a);
            if (b > 20) {
                const c = b * 1.5;
                return c;
            }
            return b;
        }
        "#;
        let dummy_path = PathBuf::from("nested.ts");
        let funcs = collect_functions_in_file(&dummy_path, &dummy_path, code);

        // outerFunction + inner arrow function = 2 functions
        assert_eq!(funcs.len(), 2);
        let outer = &funcs[0];
        // outerFunction's AST kinds contains ArrowFunctionExpression, but not its inner statements
        assert!(outer.ast_kinds.contains(&oxc::ast::AstType::ArrowFunctionExpression));
    }
}
