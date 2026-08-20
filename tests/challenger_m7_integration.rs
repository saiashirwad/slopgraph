//! Empirical Challenger Test Suite for Milestone M7: Cross-Detector Integration and Output Stability.

use slopgraph::{analyze, analyze_with_options, Options};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

struct TestWorkspace {
    path: PathBuf,
}

impl TestWorkspace {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("slopgraph_chal_m7_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("src")).unwrap();
        fs::create_dir_all(path.join("tests")).unwrap();
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

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn parse_all_findings(report: &str) -> Vec<(String, String, String, u32)> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = report.lines().collect();
    let mut current_file = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if (line.ends_with(".ts") || line.ends_with(".tsx") || line.ends_with(".js") || line.ends_with(".jsx"))
            && !line.contains("←──")
            && i + 1 < lines.len()
            && lines[i + 1].is_empty()
        {
            current_file = line.to_string();
        } else if matches!(
            line,
            "UNREACHABLE"
                | "SINGLE-USE CHAIN"
                | "EMPTY WRAPPER"
                | "FALSE SHARING"
                | "NEAR-DUPLICATE"
                | "TRAMP DATA"
                | "TYPE CLONE"
                | "UNREACHING TEST"
        ) {
            let shape = line.to_string();
            let mut subject = String::new();
            let mut line_num = 0;
            if i + 1 < lines.len() && lines[i + 1].starts_with("subject: ") {
                let rest = lines[i + 1].strip_prefix("subject: ").unwrap();
                if let Some((subj_part, line_part)) = rest.split_once("  (line ") {
                    subject = subj_part.trim().to_string();
                    let line_str = line_part.trim_end_matches(')').trim();
                    line_num = line_str.parse().unwrap_or(0);
                } else {
                    subject = rest.trim().to_string();
                }
            }
            if !current_file.is_empty() && !shape.is_empty() {
                findings.push((current_file.clone(), shape, subject, line_num));
            }
        }
        i += 1;
    }
    findings
}

// -----------------------------------------------------------------------------
// Test 1: All Eight Detectors Execute Simultaneously Without Interference
// -----------------------------------------------------------------------------

#[test]
fn test_all_eight_detectors_simultaneous_execution() {
    let ws = TestWorkspace::new("all_eight_simultaneous");
    ws.init_tsconfig();

    // 1. Unreachable module & function
    ws.write_file(
        "src/dead_module.ts",
        r#"
export function deadFunctionA() {
    return "dead";
}
"#,
    );

    // 2. Near-duplicate pair (>= 20 AST nodes and >= 50 tokens)
    ws.write_file(
        "src/duplicates.ts",
        r#"
export function processCustomerA(input: Record<string, unknown>): boolean {
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

export function processCustomerB(data: Record<string, unknown>): boolean {
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

    // 3. Single-use chain
    ws.write_file(
        "src/pipeline.ts",
        r#"
function stepOne(x: number): number {
    return stepTwo(x + 1);
}

function stepTwo(y: number): number {
    return stepThree(y * 2);
}

function stepThree(z: number): number {
    return z + 10;
}

export function executePipeline(input: number): number {
    const adjusted = input + 1;
    return stepOne(adjusted);
}
"#,
    );

    // 4. Standalone Empty Wrapper
    ws.write_file(
        "src/wrapper.ts",
        r#"
export function targetAction(n: number): number {
    return n * 100;
}

export function standaloneWrapper(n: number): number {
    return targetAction(n);
}
"#,
    );

    // 5. Tramp Data
    ws.write_file(
        "src/tramp.ts",
        r#"
export function finalConsumer(payload: string): string {
    return payload.toUpperCase();
}

export function trampForwarder(payload: string, count: number): string {
    const unused = count + 1;
    return finalConsumer(payload);
}
"#,
    );

    // 6. Type Clone
    ws.write_file(
        "src/types.ts",
        r#"
export interface UserRecord {
    id: string;
    username: string;
    email: string;
}

export interface CustomerRecord {
    id: string;
    username: string;
    email: string;
}
"#,
    );

    // 7. False Sharing & Entry Points
    ws.write_file(
        "src/service.ts",
        r#"
export function serviceHelper(): string {
    return "service";
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { processCustomerA, processCustomerB } from "./duplicates";
import { executePipeline } from "./pipeline";
import { standaloneWrapper } from "./wrapper";
import { trampForwarder } from "./tramp";
import { UserRecord, CustomerRecord } from "./types";
import { serviceHelper } from "./service";

export function main() {
    const a = processCustomerA({});
    const b = processCustomerB({});
    const c = executePipeline(5);
    const d = standaloneWrapper(10);
    const e = trampForwarder("data", 1);
    const f = serviceHelper();
    return { a, b, c, d, e, f };
}
"#,
    );

    // 8. Unreaching test
    ws.write_file(
        "src/unreached_prod.ts",
        r#"
export function unreachedProdFn(): string {
    return "unreached";
}
"#,
    );

    ws.write_file(
        "tests/suite.test.ts",
        r#"
import { serviceHelper } from "../src/service";
import { unreachedProdFn } from "../src/unreached_prod";

export function testRun() {
    serviceHelper();
    // unreachedProdFn is imported but NEVER called!
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_all_findings(&report);

    let mut found_unreachable = false;
    let mut found_single_use = false;
    let mut found_empty_wrapper = false;
    let mut found_false_sharing = false;
    let mut found_near_dup = false;
    let mut found_tramp = false;
    let mut found_type_clone = false;
    let mut found_unreaching_test = false;

    for (_, shape, _, _) in &findings {
        match shape.as_str() {
            "UNREACHABLE" => found_unreachable = true,
            "SINGLE-USE CHAIN" => found_single_use = true,
            "EMPTY WRAPPER" => found_empty_wrapper = true,
            "FALSE SHARING" => found_false_sharing = true,
            "NEAR-DUPLICATE" => found_near_dup = true,
            "TRAMP DATA" => found_tramp = true,
            "TYPE CLONE" => found_type_clone = true,
            "UNREACHING TEST" => found_unreaching_test = true,
            _ => {}
        }
    }

    assert!(found_unreachable, "Missing UNREACHABLE finding");
    assert!(found_single_use, "Missing SINGLE-USE CHAIN finding");
    assert!(found_empty_wrapper, "Missing EMPTY WRAPPER finding");
    assert!(found_false_sharing, "Missing FALSE SHARING finding");
    assert!(found_near_dup, "Missing NEAR-DUPLICATE finding");
    assert!(found_tramp, "Missing TRAMP DATA finding");
    assert!(found_type_clone, "Missing TYPE CLONE finding");
    assert!(found_unreaching_test, "Missing UNREACHING TEST finding");
}

// -----------------------------------------------------------------------------
// Test 2: Finding Sorting Order Stability Across Multiple Runs
// -----------------------------------------------------------------------------

#[test]
fn test_finding_sorting_order_stability_across_runs() {
    let ws = TestWorkspace::new("sorting_stability");
    ws.init_tsconfig();

    // Create 4 files with various findings
    ws.write_file(
        "src/z_last.ts",
        r#"
export function actionZ(): string {
    return "z";
}
export function wrapperZ(): string {
    return actionZ();
}
"#,
    );

    ws.write_file(
        "src/a_first.ts",
        r#"
export interface TypeA {
    field1: string;
    field2: number;
    field3: boolean;
}
export interface TypeB {
    field1: string;
    field2: number;
    field3: boolean;
}
"#,
    );

    ws.write_file(
        "src/m_middle.ts",
        r#"
function chain1(x: number): number { return chain2(x); }
function chain2(x: number): number { return chain3(x); }
function chain3(x: number): number { return x + 1; }
export function startChain(x: number): number { return chain1(x); }
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { wrapperZ } from "./z_last";
import { startChain } from "./m_middle";

export function entry() {
    const res1 = wrapperZ();
    const res2 = startChain(10);
    return { res1, res2 };
}
"#,
    );

    let first_report = analyze(&ws.path).unwrap();

    // Run 50 iterations and assert report equality
    for iter in 0..50 {
        let current_report = analyze(&ws.path).unwrap();
        assert_eq!(
            first_report, current_report,
            "Report output diverged at iteration {}",
            iter
        );
    }

    // Verify ordering invariant:
    // Files must appear in alphabetical order: src/a_first.ts -> src/m_middle.ts -> src/z_last.ts
    let findings = parse_all_findings(&first_report);
    for window in findings.windows(2) {
        let (f1, _, _s1, l1) = &window[0];
        let (f2, _, _s2, l2) = &window[1];
        if f1 == f2 {
            assert!(
                l1 <= l2,
                "Findings within same file {} must be sorted by line/span: line {} > line {}",
                f1, l1, l2
            );
        } else {
            assert!(
                f1 < f2,
                "Files must be sorted alphabetically: {} came before {}",
                f1, f2
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Test 3: Empty Wrappers Inside Single-Use Chains vs Standalone Empty Wrappers
// -----------------------------------------------------------------------------

#[test]
fn test_empty_wrappers_suppressed_in_chains_and_standalone_detected() {
    let ws = TestWorkspace::new("wrappers_and_chains");
    ws.init_tsconfig();

    ws.write_file(
        "src/chains_and_wrappers.ts",
        r#"
// Target endpoint
export function finalTarget(v: number): number {
    return v + 42;
}

// Single-use chain with forwarding nodes (empty wrappers inside chain)
// Chain: startPipeline -> forwardNodeA -> forwardNodeB -> finalTarget
function forwardNodeB(v: number): number {
    return finalTarget(v);
}

function forwardNodeA(v: number): number {
    return forwardNodeB(v);
}

export function startPipeline(v: number): number {
    return forwardNodeA(v);
}

// Standalone Empty Wrapper 1 (called by entry)
export function standaloneAction(v: number): number {
    return v * 2;
}
export function standaloneWrapper1(v: number): number {
    return standaloneAction(v);
}

// Standalone Empty Wrapper 2 with multiple callers (in-degree 2 -> cannot be single-use chain)
export function multiTarget(v: number): number {
    return v * 3;
}
export function multiWrapper(v: number): number {
    return multiTarget(v);
}

export function callerOne(): number {
    return multiWrapper(1);
}
export function callerTwo(): number {
    return multiWrapper(2);
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import {
    startPipeline,
    standaloneWrapper1,
    callerOne,
    callerTwo
} from "./chains_and_wrappers";

export function main() {
    const a = startPipeline(10);
    const b = standaloneWrapper1(20);
    const c = callerOne();
    const d = callerTwo();
    return { a, b, c, d };
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_all_findings(&report);

    // Single-use chain must be detected for startPipeline -> forwardNodeA -> forwardNodeB -> finalTarget
    let chain_findings: Vec<_> = findings
        .iter()
        .filter(|(_, shape, _, _)| shape == "SINGLE-USE CHAIN")
        .collect();
    assert_eq!(chain_findings.len(), 1, "Expected exactly 1 SINGLE-USE CHAIN finding");
    assert_eq!(chain_findings[0].2, "forwardNodeA");

    // Empty wrappers inside the chain (forwardNodeA, forwardNodeB) MUST be suppressed!
    let empty_wrapper_findings: Vec<_> = findings
        .iter()
        .filter(|(_, shape, _, _)| shape == "EMPTY WRAPPER")
        .collect();

    // Verify standalone wrappers ARE detected
    let empty_wrapper_subjects: Vec<&str> = empty_wrapper_findings
        .iter()
        .map(|(_, _, subj, _)| subj.as_str())
        .collect();

    assert!(
        empty_wrapper_subjects.contains(&"standaloneWrapper1"),
        "standaloneWrapper1 must be detected as EMPTY WRAPPER"
    );
    assert!(
        empty_wrapper_subjects.contains(&"multiWrapper"),
        "multiWrapper (called from 2 callers) must be detected as EMPTY WRAPPER"
    );

    // Verify chain forwarders are NOT in empty wrapper findings
    assert!(
        !empty_wrapper_subjects.contains(&"forwardNodeA"),
        "forwardNodeA is inside single-use chain and must be suppressed from EMPTY WRAPPER"
    );
    assert!(
        !empty_wrapper_subjects.contains(&"forwardNodeB"),
        "forwardNodeB is inside single-use chain and must be suppressed from EMPTY WRAPPER"
    );
}

// -----------------------------------------------------------------------------
// Test 4: Empty Wrapper at Chain Head (Entry Forwarder)
// -----------------------------------------------------------------------------

#[test]
fn test_empty_wrapper_at_chain_entry_suppression() {
    let ws = TestWorkspace::new("chain_entry_wrapper");
    ws.init_tsconfig();

    ws.write_file(
        "src/pipeline.ts",
        r#"
function internalStep1(x: number): number {
    return internalStep2(x + 1);
}

function internalStep2(x: number): number {
    return x * 2;
}

// Entry point is itself an empty wrapper forwarding directly to internalStep1
export function runWorkflow(x: number): number {
    return internalStep1(x);
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { runWorkflow } from "./pipeline";
export function main() {
    const value = 5;
    const computed = runWorkflow(value);
    return computed;
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_all_findings(&report);

    // The chain is runWorkflow -> internalStep1 -> internalStep2
    let chain_findings: Vec<_> = findings
        .iter()
        .filter(|(_, shape, _, _)| shape == "SINGLE-USE CHAIN")
        .collect();
    assert_eq!(chain_findings.len(), 1);
    assert_eq!(chain_findings[0].2, "internalStep1");

    // runWorkflow forwards to internalStep1 (the chain head).
    // It should be suppressed from EMPTY WRAPPER because it forwards to the chain.
    let empty_wrapper_findings: Vec<_> = findings
        .iter()
        .filter(|(_, shape, _, _)| shape == "EMPTY WRAPPER")
        .collect();
    assert!(
        empty_wrapper_findings.is_empty(),
        "Expected 0 EMPTY WRAPPER findings, got: {:?}",
        empty_wrapper_findings
    );
}

// -----------------------------------------------------------------------------
// Test 5: Complex Interlocking Detectors Under CLI Flags (--production and --include-exported)
// -----------------------------------------------------------------------------

#[test]
fn test_cli_flags_cross_detector_interactions() {
    let ws = TestWorkspace::new("cli_flags_interaction");
    ws.init_tsconfig();

    ws.write_file(
        "src/service.ts",
        r#"
export function exportedHop1(n: number): number {
    return exportedHop2(n + 1);
}

export function exportedHop2(n: number): number {
    return leafCalc(n * 2);
}

function leafCalc(n: number): number {
    return n;
}

export function testOnlyOperation(): string {
    return "test_only";
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { exportedHop1 } from "./service";
export function app() {
    const val = 10;
    const result = exportedHop1(val);
    return result;
}
"#,
    );

    ws.write_file(
        "src/unused_prod.ts",
        r#"
export function neverCalled(): void {}
"#,
    );

    ws.write_file(
        "tests/app.test.ts",
        r#"
import { testOnlyOperation } from "../src/service";
import { neverCalled } from "../src/unused_prod";

export function runTest() {
    testOnlyOperation();
}
"#,
    );

    // 1. Default mode:
    // - testOnlyOperation is reached via tests/app.test.ts
    // - src/unused_prod.ts is imported by test but neverCalled is not called -> UNREACHING TEST finding
    // - exportedHop1 -> exportedHop2 -> leafCalc has exportedHop2 (exported), so by default exportedHop2 is excluded
    //   from single-use chain (chain length = 1 leafCalc, so no single-use chain)
    let default_report = analyze(&ws.path).unwrap();
    let default_findings = parse_all_findings(&default_report);

    let has_unreaching = default_findings
        .iter()
        .any(|(_, shape, subj, _)| shape == "UNREACHING TEST" && subj == "src/unused_prod.ts");
    assert!(has_unreaching, "Default mode must report UNREACHING TEST for src/unused_prod.ts");

    let has_single_use = default_findings
        .iter()
        .any(|(_, shape, _, _)| shape == "SINGLE-USE CHAIN");
    assert!(!has_single_use, "Default mode must exclude exported functions from SINGLE-USE CHAIN");

    // 2. With --include-exported:
    // - exportedHop1 is the subject of the chain!
    let include_exp_options = Options {
        include_exported: true,
        ..Default::default()
    };
    let inc_exp_report = analyze_with_options(&ws.path, include_exp_options).unwrap();
    let inc_exp_findings = parse_all_findings(&inc_exp_report);

    let inc_has_single_use = inc_exp_findings
        .iter()
        .any(|(_, shape, subj, _)| shape == "SINGLE-USE CHAIN" && subj == "exportedHop1");
    assert!(inc_has_single_use, "With --include-exported, SINGLE-USE CHAIN must include exportedHop1 as head");

    // 3. With --production:
    // - test roots are dropped -> testOnlyOperation becomes UNREACHABLE
    // - tests/app.test.ts still triggers UNREACHING TEST for src/unused_prod.ts
    let prod_options = Options {
        production: true,
        ..Default::default()
    };
    let prod_report = analyze_with_options(&ws.path, prod_options).unwrap();
    let prod_findings = parse_all_findings(&prod_report);

    let prod_has_unreachable_fn = prod_findings
        .iter()
        .any(|(_, shape, subj, _)| shape == "UNREACHABLE" && subj == "testOnlyOperation");
    assert!(
        prod_has_unreachable_fn,
        "With --production, testOnlyOperation must be reported as UNREACHABLE"
    );

    let prod_has_unreaching = prod_findings
        .iter()
        .any(|(_, shape, subj, _)| shape == "UNREACHING TEST" && subj == "src/unused_prod.ts");
    assert!(
        prod_has_unreaching,
        "With --production, tests are still analyzed and emit UNREACHING TEST"
    );
}

// -----------------------------------------------------------------------------
// Test 6: High Volume and Memory Scaling Stress Test
// -----------------------------------------------------------------------------

#[test]
fn test_high_volume_cross_detector_stress() {
    let ws = TestWorkspace::new("high_volume_stress");
    ws.init_tsconfig();

    // Create 15 modules with dense interconnected graphs
    for mod_i in 0..15 {
        let mut content = String::new();
        content.push_str(&format!("// Module {}\n", mod_i));

        // Add near duplicates (>= 50 tokens and >= 20 AST nodes)
        content.push_str(&format!(
            r#"
export function nearDupAlpha_{mod_i}(input: Record<string, unknown>): boolean {{
    const isValid = input !== null && typeof input === "object";
    if (!isValid) {{
        return false;
    }}
    const id = input["customerId_{mod_i}"];
    const score = Number(input["score"]);
    const flag = Boolean(input["active"]);
    if (score > 100 && flag) {{
        const adjusted = score * 1.1;
        const result = adjusted > 150 ? true : false;
        return result;
    }}
    return false;
}}

export function nearDupBeta_{mod_i}(data: Record<string, unknown>): boolean {{
    const isOk = data !== null && typeof data === "object";
    if (!isOk) {{
        return false;
    }}
    const key = data["supplierId_{mod_i}"];
    const amount = Number(data["total"]);
    const status = Boolean(data["verified"]);
    if (amount > 200 && status) {{
        const computed = amount * 1.25;
        const finalVal = computed > 250 ? true : false;
        return finalVal;
    }}
    return false;
}}
"#
        ));

        // Add tramp data
        content.push_str(&format!(
            r#"
export function trampSink_{mod_i}(data: string): string {{
    return data.trim();
}}

export function trampForward_{mod_i}(data: string, unusedFlag: boolean): string {{
    const local = 100;
    return trampSink_{mod_i}(data);
}}
"#
        ));

        // Add type clone
        content.push_str(&format!(
            r#"
export interface TypeAlpha_{mod_i} {{
    id: string;
    count: number;
    enabled: boolean;
}}

export interface TypeBeta_{mod_i} {{
    id: string;
    count: number;
    enabled: boolean;
}}
"#
        ));

        ws.write_file(&format!("src/mod_{}.ts", mod_i), &content);
    }

    // Connect them in src/index.ts
    let mut index_content = String::new();
    for mod_i in 0..15 {
        index_content.push_str(&format!(
            "import {{ nearDupAlpha_{mod_i}, nearDupBeta_{mod_i}, trampForward_{mod_i} }} from './mod_{mod_i}';\n"
        ));
    }
    index_content.push_str("\nexport function main() {\n");
    for mod_i in 0..15 {
        index_content.push_str(&format!(
            "    nearDupAlpha_{mod_i}({{}});\n    nearDupBeta_{mod_i}({{}});\n    trampForward_{mod_i}('val', true);\n"
        ));
    }
    index_content.push_str("}\n");
    ws.write_file("src/index.ts", &index_content);

    // Run analysis and verify memory/runtime efficiency
    let start = std::time::Instant::now();
    let report = analyze(&ws.path).unwrap();
    let duration = start.elapsed();

    assert!(
        duration.as_secs() < 5,
        "High volume cross-detector analysis took too long: {:?}",
        duration
    );

    let findings = parse_all_findings(&report);
    // 15 near-duplicate pairs + 15 tramp data + 15 type clones = 45 findings
    assert!(
        findings.len() >= 45,
        "Expected at least 45 findings from 15 modules, got {}",
        findings.len()
    );
}

// -----------------------------------------------------------------------------
// Test 7: Multithreaded Concurrent Execution Safety
// -----------------------------------------------------------------------------

#[test]
fn test_multithreaded_concurrent_analysis_safety() {
    let ws = Arc::new(TestWorkspace::new("concurrent_safety"));
    ws.init_tsconfig();

    ws.write_file(
        "src/a.ts",
        r#"
export function compute(x: number): number { return x * 2; }
export function wrapCompute(x: number): number { return compute(x); }
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { wrapCompute } from "./a";
export function main() {
    const res = wrapCompute(5);
    return res;
}
"#,
    );

    let mut handles = Vec::new();
    for _ in 0..8 {
        let ws_clone = Arc::clone(&ws);
        handles.push(std::thread::spawn(move || {
            let res = analyze(&ws_clone.path);
            assert!(res.is_ok());
            res.unwrap()
        }));
    }

    let mut outputs = Vec::new();
    for h in handles {
        outputs.push(h.join().unwrap());
    }

    // All threads must produce the exact same report
    for i in 1..outputs.len() {
        assert_eq!(outputs[0], outputs[i], "Concurrent thread report mismatch");
    }
}

// -----------------------------------------------------------------------------
// Test 8: Output Conformance - No Automated Remedies or Patches Emitted
// -----------------------------------------------------------------------------

#[test]
fn test_no_automated_code_remedies_or_patches_in_reports() {
    let ws = TestWorkspace::new("no_patches");
    ws.init_tsconfig();

    ws.write_file(
        "src/dead.ts",
        r#"
export function deadFn() {
    return "dead";
}
"#,
    );

    ws.write_file(
        "src/types.ts",
        r#"
export interface TypeOne {
    a: string;
    b: number;
    c: boolean;
}
export interface TypeTwo {
    a: string;
    b: number;
    c: boolean;
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { TypeOne, TypeTwo } from "./types";
export function main() {
    const t1: TypeOne = { a: "1", b: 2, c: true };
    const t2: TypeTwo = { a: "2", b: 3, c: false };
    return { t1, t2 };
}
"#,
    );

    let report = analyze(&ws.path).unwrap();

    // Verify no automated patch / remediation keywords appear in report
    let forbidden_phrases = [
        "diff --git",
        "+++",
        "---",
        "Suggested fix:",
        "Patch:",
        "Auto-fix",
        "Refactoring suggestion:",
        "Delete this line",
        "Remove this function",
    ];

    for phrase in forbidden_phrases {
        assert!(
            !report.contains(phrase),
            "Report should not emit automated remedies or patches, but contained: '{}'",
            phrase
        );
    }
}
