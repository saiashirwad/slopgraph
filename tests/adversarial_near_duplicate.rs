//! Adversarial stress testing and empirical validation for Milestone M3: Near-Duplicate Detector.

use slopgraph::analyze;
use std::fs;
use std::path::PathBuf;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("slopgraph_adv_nd_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("src")).unwrap();
        Self { path }
    }

    fn write_file(&self, rel_path: &str, content: &str) {
        let target = self.path.join(rel_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(target, content).unwrap();
    }

    fn init_tsconfig(&self) {
        self.write_file(
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "target": "es2022",
    "module": "commonjs",
    "strict": true
  },
  "include": ["src/**/*", "tests/**/*"]
}"#,
        );
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Parse NEAR-DUPLICATE pairs from rendered report.
fn find_near_duplicate_pairs(report: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let lines: Vec<&str> = report.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "NEAR-DUPLICATE" {
            let mut subject = String::new();
            let mut target = String::new();
            if i + 1 < lines.len() && lines[i + 1].starts_with("subject: ") {
                let rest = lines[i + 1].strip_prefix("subject: ").unwrap();
                subject = rest.split("  (").next().unwrap_or(rest).trim().to_string();
            }
            if i + 5 < lines.len() && lines[i + 4].trim() == "▼" {
                target = lines[i + 5].trim().to_string();
            }
            if !subject.is_empty() && !target.is_empty() {
                pairs.push((subject, target));
            }
        }
        i += 1;
    }
    pairs
}

/// Boundary Test: AST node count exactly 19 vs 20.
/// Spec: function body must have at least 20 nodes.
#[test]
fn test_boundary_ast_nodes_19_vs_20() {
    let dir = TestDir::new("ast_nodes_boundary");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./sub_20";
export * from "./exact_20";
"#,
    );

    // sub_20.ts: Functions with exactly 19 AST nodes in body:
    // Statement 1: const a = (1 as any); (7 AST nodes)
    // Statement 2: const b = 2;          (4 AST nodes)
    // Statement 3: const c = 3;          (4 AST nodes)
    // Statement 4: return a + 1;         (4 AST nodes)
    // Total AST nodes = 7 + 4 + 4 + 4 = 19 (< 20 AST nodes).
    dir.write_file(
        "src/sub_20.ts",
        r#"
export function exact19AstA(): number {
    const a = (1 as any);
    const b = 2;
    const c = 3;
    return a + 1;
}

export function exact19AstB(): number {
    const x = (10 as any);
    const y = 20;
    const z = 30;
    return x + 1;
}
"#,
    );

    // exact_20.ts: Functions with >= 20 AST nodes and >= 50 tokens
    dir.write_file(
        "src/exact_20.ts",
        r#"
export function validAstA(input: Record<string, unknown>): boolean {
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

export function validAstB(data: Record<string, unknown>): boolean {
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
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.iter().any(|(a, b)| a == "validAstA" && b == "validAstB"),
        "Expected validAstA -> validAstB near-duplicate finding, got: {:?}",
        pairs
    );

    assert!(
        !pairs.iter().any(|(a, b)| a == "exact19AstA" || b == "exact19AstA"),
        "Did not expect exact19AstA (19 AST nodes) in near-duplicate findings, got: {:?}",
        pairs
    );
}

/// Boundary Test: Token count exactly 49 vs 50 tokens.
/// Spec: 50-token window hash.
#[test]
fn test_boundary_tokens_49_vs_50() {
    let dir = TestDir::new("tokens_boundary");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./tokens_49";
export * from "./tokens_50";
"#,
    );

    // tokens_49.ts: 38 AST nodes and EXACTLY 49 tokens in body:
    // { + 8*5 + 7 + } = 49 tokens.
    dir.write_file(
        "src/tokens_49.ts",
        r#"
export function smallTokensA(): number {
    const a = 1;
    const b = 2;
    const c = 3;
    const d = 4;
    const e = 5;
    const f = 6;
    const g = 7;
    const h = 8;
    return a + b + c;
}

export function smallTokensB(): number {
    const z = 10;
    const y = 20;
    const x = 30;
    const w = 40;
    const v = 50;
    const u = 60;
    const t = 70;
    const s = 80;
    return z + y + x;
}
"#,
    );

    // tokens_50.ts: 38 AST nodes and EXACTLY 50 tokens in body:
    // { + 9*5 + 3 + } = 50 tokens.
    dir.write_file(
        "src/tokens_50.ts",
        r#"
export function exact50TokensA(): number {
    const a = 1;
    const b = 2;
    const c = 3;
    const d = 4;
    const e = 5;
    const f = 6;
    const g = 7;
    const h = 8;
    const i = 9;
    return a;
}

export function exact50TokensB(): number {
    const z = 10;
    const y = 20;
    const x = 30;
    const w = 40;
    const v = 50;
    const u = 60;
    const t = 70;
    const s = 80;
    const r = 90;
    return z;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.iter().any(|(a, b)| a == "exact50TokensA" && b == "exact50TokensB"),
        "Expected exact50TokensA -> exact50TokensB near-duplicate finding, got: {:?}",
        pairs
    );

    assert!(
        !pairs.iter().any(|(a, b)| a == "smallTokensA" || b == "smallTokensA"),
        "Did not expect smallTokensA (49 tokens) in near-duplicate findings, got: {:?}",
        pairs
    );
}

/// Distinct Function Names Test.
/// Spec: Two or more functions with distinct names and the same shape.
#[test]
fn test_identical_name_functions_vs_different_names() {
    let dir = TestDir::new("same_names");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./block_scopes";
"#,
    );

    dir.write_file(
        "src/block_scopes.ts",
        r#"
// Two identical-named functions in different blocks
{
    function computeMetrics(val: number): number {
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
    function computeMetrics(amount: number): number {
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

// Two differently-named functions with the same shape
export function evaluateAlpha(val: number): number {
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

export function evaluateBeta(amount: number): number {
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
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.iter().any(|(a, b)| a == "evaluateAlpha" && b == "evaluateBeta"),
        "Expected evaluateAlpha -> evaluateBeta, got: {:?}",
        pairs
    );

    assert!(
        !pairs.iter().any(|(a, b)| a == "computeMetrics" && b == "computeMetrics"),
        "Did not expect same-name computeMetrics -> computeMetrics pair, got: {:?}",
        pairs
    );
}

/// Nested Functions Inside Bodies Test.
/// Verifies:
/// 1. Outer functions containing nested functions match each other.
/// 2. Outer function AST collector skips inner function bodies.
/// 3. Inner functions with >= 20 AST nodes and >= 50 tokens are also detected as a pair.
#[test]
fn test_nested_functions_inside_bodies() {
    let dir = TestDir::new("nested_functions");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./nested";
"#,
    );

    dir.write_file(
        "src/nested.ts",
        r#"
export function outerHandlerA(input: Record<string, unknown>): boolean {
    const isValid = input !== null && typeof input === "object";
    if (!isValid) {
        return false;
    }
    // Inner function 1 (>= 20 AST nodes and >= 50 tokens)
    const innerFormatterA = (val: number, factor: number): string => {
        const prefix = "VAL_";
        const numStr = String(val * 2);
        const suffix = "_END";
        const tagA = "TAG_A";
        const tagB = "TAG_B";
        const multiplier = factor > 0 ? factor * 1.5 : 1.0;
        if (val > 50 && multiplier > 1.0) {
            const res = prefix + numStr + suffix + tagA + tagB;
            return res;
        }
        return prefix + "DEFAULT" + suffix;
    };
    const id = input["customerId"];
    const score = Number(input["score"]);
    const formatted = innerFormatterA(score, 2.0);
    if (score > 100 && formatted.length > 5) {
        const adjusted = score * 1.1;
        return adjusted > 150;
    }
    return false;
}

export function outerHandlerB(data: Record<string, unknown>): boolean {
    const isOk = data !== null && typeof data === "object";
    if (!isOk) {
        return false;
    }
    // Inner function 2 (>= 20 AST nodes and >= 50 tokens)
    const innerFormatterB = (num: number, scale: number): string => {
        const tag = "NUM_";
        const valueStr = String(num * 3);
        const tail = "_TAIL";
        const markA = "MARK_A";
        const markB = "MARK_B";
        const ratio = scale > 0 ? scale * 2.5 : 1.0;
        if (num > 60 && ratio > 1.0) {
            const output = tag + valueStr + tail + markA + markB;
            return output;
        }
        return tag + "FALLBACK" + tail;
    };
    const key = data["supplierId"];
    const amount = Number(data["total"]);
    const text = innerFormatterB(amount, 3.0);
    if (amount > 200 && text.length > 5) {
        const computed = amount * 1.25;
        return computed > 250;
    }
    return false;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    // Both outer functions and inner functions should be detected
    assert!(
        pairs.iter().any(|(a, b)| a == "outerHandlerA" && b == "outerHandlerB"),
        "Expected outerHandlerA -> outerHandlerB near duplicate pair, got: {:?}",
        pairs
    );

    assert!(
        pairs.iter().any(|(a, b)| a == "innerFormatterA" && b == "innerFormatterB"),
        "Expected innerFormatterA -> innerFormatterB near duplicate pair, got: {:?}",
        pairs
    );
}

/// Heavy Variable Renaming & Literal Diversity Test.
/// Spec: Identifiers normalize to $ID; literals normalize to $LIT.
#[test]
fn test_heavy_variable_renaming_and_literal_diversity() {
    let dir = TestDir::new("heavy_renaming");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./renamed";
"#,
    );

    dir.write_file(
        "src/renamed.ts",
        r#"
export function pipelineFirst(alpha: string, beta: number, gamma: boolean): string {
    const strLiteral = "alpha_prefix_string_literal_value";
    const numLiteral = 123456789;
    const bigIntVal = 999999999999n;
    const regexPattern = /^[a-z]+@[0-9]+$/i;
    const templateVal = `item: ${alpha} with count: ${beta}`;
    
    if (gamma && beta > numLiteral) {
        const intermediate = strLiteral + templateVal;
        const matchResult = regexPattern.test(intermediate);
        if (matchResult) {
            const transformed = intermediate.toUpperCase() + String(bigIntVal);
            return transformed;
        }
    }
    return strLiteral;
}

export function pipelineSecond(zeta: string, eta: number, theta: boolean): string {
    const customTag = "different_literal_tag_for_second_function";
    const thresholdLimit = 987654321;
    const largeNumber = 111111111111n;
    const validationRegex = /^[0-9]+-[A-Z]+$/g;
    const formattedMessage = `entity: ${zeta} total amount: ${eta}`;
    
    if (theta && eta > thresholdLimit) {
        const combined = customTag + formattedMessage;
        const isMatched = validationRegex.test(combined);
        if (isMatched) {
            const outputString = combined.toUpperCase() + String(largeNumber);
            return outputString;
        }
    }
    return customTag;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.iter().any(|(a, b)| a == "pipelineFirst" && b == "pipelineSecond"),
        "Expected pipelineFirst -> pipelineSecond near duplicate with heavy renaming and literal changes, got: {:?}",
        pairs
    );
}

/// Arrow Functions: Expression vs Block Bodies.
/// Verifies:
/// 1. Arrow functions with block bodies match other block bodies.
/// 2. Arrow functions with block bodies match standard function declarations with same body structure.
/// 3. Arrow functions with expression bodies match other expression bodies.
/// 4. Arrow functions with expression bodies do NOT match block bodies (different AST structure).
#[test]
fn test_arrow_functions_expression_vs_block_bodies() {
    let dir = TestDir::new("arrow_functions");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./arrows";
"#,
    );

    dir.write_file(
        "src/arrows.ts",
        r#"
// Arrow function with block body 1
export const arrowBlockA = (input: Record<string, unknown>): boolean => {
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
};

// Arrow function with block body 2
export const arrowBlockB = (data: Record<string, unknown>): boolean => {
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
};

// Function declaration with matching block body
export function declBlockC(record: Record<string, unknown>): boolean {
    const isCheck = record !== null && typeof record === "object";
    if (!isCheck) {
        return false;
    }
    const entryKey = record["accountId"];
    const rating = Number(record["rating"]);
    const enabled = Boolean(record["enabled"]);
    if (rating > 300 && enabled) {
        const factored = rating * 1.5;
        const isHigh = factored > 350 ? true : false;
        return isHigh;
    }
    return false;
}

// Arrow functions with expression bodies (>= 20 AST nodes and >= 50 tokens)
export const arrowExprA = (x: number, y: number, z: number): number =>
    x > 100 && y > 200 && z > 300
        ? x * 2 + y * 3 + z * 4 + (x > 500 ? x / 2 + y / 3 : z / 4 + 10) + (x % 10) + (y % 20) + Math.max(x, y, z) + 100
        : 0;

export const arrowExprB = (a: number, b: number, c: number): number =>
    a > 100 && b > 200 && c > 300
        ? a * 2 + b * 3 + c * 4 + (a > 500 ? a / 2 + b / 3 : c / 4 + 10) + (a % 10) + (b % 20) + Math.max(a, b, c) + 100
        : 0;
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    // arrowBlockA and arrowBlockB MUST be detected
    assert!(
        pairs.iter().any(|(a, b)| a == "arrowBlockA" && b == "arrowBlockB"),
        "Expected arrowBlockA -> arrowBlockB, got: {:?}",
        pairs
    );

    // declBlockC and arrowBlockA have matching block body structure
    assert!(
        pairs.iter().any(|(a, b)| (a == "arrowBlockA" && b == "declBlockC") || (a == "declBlockC" && b == "arrowBlockA")),
        "Expected arrowBlockA -> declBlockC, got: {:?}",
        pairs
    );

    // arrowExprA and arrowExprB MUST be detected as matching expression bodies
    assert!(
        pairs.iter().any(|(a, b)| a == "arrowExprA" && b == "arrowExprB"),
        "Expected arrowExprA -> arrowExprB, got: {:?}",
        pairs
    );

    // Expression body arrows should NOT match block body arrows
    assert!(
        !pairs.iter().any(|(a, b)| (a == "arrowExprA" && b == "arrowBlockA") || (a == "arrowBlockA" && b == "arrowExprA")),
        "Did not expect arrowExprA to match arrowBlockA, got: {:?}",
        pairs
    );
}

/// Cross-File Detection and Report Formatting.
#[test]
fn test_cross_file_near_duplicate_and_report_evidence() {
    let dir = TestDir::new("cross_file");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./serviceA";
export * from "./serviceB";
"#,
    );

    dir.write_file(
        "src/serviceA.ts",
        r#"
export function handleCustomerService(payload: Record<string, unknown>): boolean {
    const isOk = payload !== null && typeof payload === "object";
    if (!isOk) {
        return false;
    }
    const id = payload["id"];
    const count = Number(payload["count"]);
    const ready = Boolean(payload["ready"]);
    if (count > 50 && ready) {
        const val = count * 2;
        return val > 120;
    }
    return false;
}
"#,
    );

    dir.write_file(
        "src/serviceB.ts",
        r#"
export function handleSupplierService(payload: Record<string, unknown>): boolean {
    const isOk = payload !== null && typeof payload === "object";
    if (!isOk) {
        return false;
    }
    const key = payload["key"];
    const quantity = Number(payload["quantity"]);
    const available = Boolean(payload["available"]);
    if (quantity > 50 && available) {
        const val = quantity * 2;
        return val > 120;
    }
    return false;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();

    assert!(report.contains("NEAR-DUPLICATE"), "Report missing NEAR-DUPLICATE section:\n{}", report);
    assert!(report.contains("subject: handleCustomerService"), "Report missing subject:\n{}", report);
    assert!(report.contains("handleCustomerService  ←── finding"), "Report missing finding marker:\n{}", report);
    assert!(report.contains("handleSupplierService"), "Report missing target node:\n{}", report);
}

/// Confidence Threshold Test (< 0.70 rejected).
#[test]
fn test_confidence_threshold_rejection_below_0_7() {
    let dir = TestDir::new("confidence_threshold");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./divergent";
"#,
    );

    // Two functions with the same AST kind sequence (series of if-checks, declarations, returns),
    // but the token sequences in windows differ substantially (e.g. operators +, -, *, /, %, typeof, instanceof, in, delete, void, new).
    dir.write_file(
        "src/divergent.ts",
        r#"
export function algorithmArithmetic(data: any): number {
    const a = data + 10;
    const b = data - 20;
    const c = data * 30;
    const d = data / 40;
    const e = data % 50;
    const f = a + b + c + d + e;
    if (f > 1000) {
        const g = f * 2;
        const h = g + 100;
        return h > 2000 ? h : g;
    }
    return f;
}

export function algorithmBitwise(data: any): number {
    const a = data << 10;
    const b = data >> 20;
    const c = data >>> 30;
    const d = data & 40;
    const e = data | 50;
    const f = a ^ b ^ c ^ d ^ e;
    if (f < 1000) {
        const g = f >> 2;
        const h = g & 100;
        return h < 2000 ? h : g;
    }
    return f;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    // Due to differing token windows (bitwise vs arithmetic operators), confidence falls below 0.7
    assert!(
        !pairs.iter().any(|(a, b)| (a == "algorithmArithmetic" && b == "algorithmBitwise") || (a == "algorithmBitwise" && b == "algorithmArithmetic")),
        "Did not expect algorithmArithmetic and algorithmBitwise to match with low token confidence, got: {:?}",
        pairs
    );
}

/// Comments and narrative trivia do not alter AST nodes or normalized tokens.
#[test]
fn test_comments_and_whitespace_insensitivity() {
    let dir = TestDir::new("comments_trivia");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./comments";
"#,
    );

    dir.write_file(
        "src/comments.ts",
        r#"
// Heavy commentary function
export function processWithDocstrings(input: Record<string, unknown>): boolean {
    /**
     * Step 1: Validate input object
     */
    const isValid = /* check non-null */ input !== null && typeof input === "object";
    // Check if valid
    if (!isValid) {
        // Return early
        return false;
    }
    // Extract properties
    const id = input["customerId"]; // ID field
    const score = Number(input["score"]); // Score field
    const flag = Boolean(input["active"]); // Active flag
    // Check threshold condition
    if (score > 100 && flag) {
        // Scale the score by 10%
        const adjusted = score * 1.1;
        // Check final limit
        const result = adjusted > 150 ? true : false;
        return result;
    }
    return false;
}

// Clean function without comments
export function processWithoutDocstrings(data: Record<string, unknown>): boolean {
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
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.iter().any(|(a, b)| a == "processWithDocstrings" && b == "processWithoutDocstrings"),
        "Expected processWithDocstrings -> processWithoutDocstrings near duplicate, got: {:?}",
        pairs
    );
}

/// Generic type parameters on functions match each other.
#[test]
fn test_generic_functions_and_type_parameters() {
    let dir = TestDir::new("generics");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./generics";
"#,
    );

    dir.write_file(
        "src/generics.ts",
        r#"
export function genericHandlerA<T extends Record<string, unknown>, K extends keyof T>(
    item: T,
    keyName: K
): boolean {
    const isValid = item !== null && typeof item === "object";
    if (!isValid) {
        return false;
    }
    const id = item[keyName];
    const score = Number(id);
    const flag = Boolean(item);
    if (score > 100 && flag) {
        const adjusted = score * 1.1;
        const result = adjusted > 150 ? true : false;
        return result;
    }
    return false;
}

export function genericHandlerB<U extends Record<string, unknown>, V extends keyof U>(
    entry: U,
    propName: V
): boolean {
    const isOk = entry !== null && typeof entry === "object";
    if (!isOk) {
        return false;
    }
    const key = entry[propName];
    const amount = Number(key);
    const status = Boolean(entry);
    if (amount > 200 && status) {
        const computed = amount * 1.25;
        const finalVal = computed > 250 ? true : false;
        return finalVal;
    }
    return false;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.iter().any(|(a, b)| a == "genericHandlerA" && b == "genericHandlerB"),
        "Expected genericHandlerA -> genericHandlerB near duplicate, got: {:?}",
        pairs
    );
}

/// Multiple pairs and determinism in report generation.
#[test]
fn test_multiple_pairs_and_ordering() {
    let dir = TestDir::new("multiple_pairs");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export * from "./pairA";
export * from "./pairB";
"#,
    );

    dir.write_file(
        "src/pairA.ts",
        r#"
export function alphaA(record: Record<string, unknown>): boolean {
    const isCheck = record !== null && typeof record === "object";
    if (!isCheck) {
        return false;
    }
    const key = record["keyA"];
    const num = Number(key);
    const ok = Boolean(record["flagA"]);
    if (num > 50 && ok) {
        const res = num * 2;
        return res > 100 ? true : false;
    }
    return false;
}

export function alphaB(data: Record<string, unknown>): boolean {
    const isCheck = data !== null && typeof data === "object";
    if (!isCheck) {
        return false;
    }
    const key = data["keyB"];
    const num = Number(key);
    const ok = Boolean(data["flagB"]);
    if (num > 50 && ok) {
        const res = num * 2;
        return res > 100 ? true : false;
    }
    return false;
}
"#,
    );

    dir.write_file(
        "src/pairB.ts",
        r#"
export function betaA(record: Record<string, unknown>): boolean {
    const isCheck = record !== null && typeof record === "object";
    if (!isCheck) {
        return false;
    }
    const key = record["key1"];
    const num = Number(key);
    const ok = Boolean(record["flag1"]);
    if (num > 100 && ok) {
        const res = num * 3;
        return res > 300 ? true : false;
    }
    return false;
}

export function betaB(data: Record<string, unknown>): boolean {
    const isCheck = data !== null && typeof data === "object";
    if (!isCheck) {
        return false;
    }
    const key = data["key2"];
    const num = Number(key);
    const ok = Boolean(data["flag2"]);
    if (num > 100 && ok) {
        const res = num * 3;
        return res > 300 ? true : false;
    }
    return false;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.iter().any(|(a, b)| a == "alphaA" && b == "alphaB"),
        "Expected alphaA -> alphaB, got: {:?}",
        pairs
    );
    assert!(
        pairs.iter().any(|(a, b)| a == "betaA" && b == "betaB"),
        "Expected betaA -> betaB, got: {:?}",
        pairs
    );
}

/// Empty functions, single line helpers, and TS declarations do not panic or produce false findings.
#[test]
fn test_empty_and_minimal_functions_no_panic() {
    let dir = TestDir::new("minimal_functions");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export declare function declaredA(x: number): number;
export declare function declaredB(y: number): number;

export function emptyA() {}
export function emptyB() {}

export function oneLinerA(x: number) { return x + 1; }
export function oneLinerB(y: number) { return y + 2; }
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_near_duplicate_pairs(&report);

    assert!(
        pairs.is_empty(),
        "Expected no near-duplicate findings for empty/minimal functions, got: {:?}",
        pairs
    );
}
