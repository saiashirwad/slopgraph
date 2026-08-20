//! Empirical adversarial stress testing for Milestone M4: Tramp Data Detector.

use slopgraph::analyze;
use std::fs;
use std::path::PathBuf;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "slopgraph_adv_tramp_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
  "include": ["src/**/*"]
}"#,
        );
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrampFinding {
    file: String,
    subject: String,
    caller: String,
    target: String,
}

fn parse_tramp_findings(report: &str) -> Vec<TrampFinding> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = report.lines().collect();
    let mut current_file = String::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // A file header in slopgraph report is a non-empty line followed by an empty line and a shape name
        if (line.ends_with(".ts") || line.ends_with(".tsx"))
            && i + 1 < lines.len()
            && lines[i + 1].trim().is_empty()
            && i + 2 < lines.len()
            && is_shape_header(lines[i + 2].trim())
        {
            current_file = line.to_string();
            i += 1;
            continue;
        }

        if line == "TRAMP DATA" {
            let mut subject = String::new();
            let mut caller = String::new();
            let mut target = String::new();

            if i + 1 < lines.len() && lines[i + 1].starts_with("subject: ") {
                let rest = lines[i + 1].strip_prefix("subject: ").unwrap();
                subject = rest.split("  (").next().unwrap_or(rest).trim().to_string();
            }

            let mut j = i + 2;
            while j < lines.len() && !lines[j].trim().is_empty() {
                let l = lines[j].trim();
                if l.ends_with("←── finding") {
                    caller = l
                        .strip_suffix("←── finding")
                        .unwrap()
                        .trim()
                        .to_string();
                } else if !l.starts_with("│")
                    && !l.starts_with("▼")
                    && !l.is_empty()
                    && !caller.is_empty()
                    && target.is_empty()
                {
                    target = l.to_string();
                }
                j += 1;
            }

            if !subject.is_empty() && !caller.is_empty() && !target.is_empty() {
                findings.push(TrampFinding {
                    file: current_file.clone(),
                    subject,
                    caller,
                    target,
                });
            }
            i = j;
            continue;
        }
        i += 1;
    }

    findings
}

fn is_shape_header(s: &str) -> bool {
    matches!(
        s,
        "UNREACHABLE"
            | "FALSE SHARING"
            | "EMPTY WRAPPER"
            | "SINGLE-USE CHAIN"
            | "NEAR-DUPLICATE"
            | "TRAMP DATA"
            | "TYPE CLONE"
            | "UNREACHING TEST"
    )
}

// =========================================================================
// 1. Multi-Hop Parameter Forwarding Chains (A -> B -> C)
// =========================================================================

#[test]
fn test_multihop_3_step_linear_chain() {
    let t = TestDir::new("multihop_3step");
    t.init_tsconfig();
    t.write_file(
        "src/chain.ts",
        r#"
export function nodeA(payload: string): void {
    nodeB(payload);
}

function nodeB(payload: string): void {
    nodeC(payload);
}

function nodeC(payload: string): void {
    sink(payload);
}

function sink(payload: string): void {
    console.log(payload.length);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert_eq!(
        findings.len(),
        3,
        "Expected 3 tramp findings along 3-step chain, got {}: {report}",
        findings.len()
    );

    assert!(findings.iter().any(|f| f.subject == "payload"
        && f.caller == "nodeA"
        && f.target == "nodeB"));
    assert!(findings.iter().any(|f| f.subject == "payload"
        && f.caller == "nodeB"
        && f.target == "nodeC"));
    assert!(findings.iter().any(|f| f.subject == "payload"
        && f.caller == "nodeC"
        && f.target == "sink"));

    // sink reads payload.length, so sink MUST NOT be in tramp findings
    assert!(!findings.iter().any(|f| f.caller == "sink"));
}

#[test]
fn test_multihop_5_step_chain_renamed_params() {
    let t = TestDir::new("multihop_5step_renamed");
    t.init_tsconfig();
    t.write_file(
        "src/pipeline.ts",
        r#"
export function hop1(paramOne: string): void {
    hop2(paramOne);
}

function hop2(paramTwo: string): void {
    hop3(paramTwo);
}

function hop3(paramThree: string): void {
    hop4(paramThree);
}

function hop4(paramFour: string): void {
    hop5(paramFour);
}

function hop5(paramFive: string): void {
    terminal(paramFive);
}

function terminal(paramEnd: string): void {
    if (paramEnd.includes("ok")) {
        console.log("valid");
    }
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert_eq!(
        findings.len(),
        5,
        "Expected 5 tramp data findings in 5-hop chain, got: {report}"
    );

    assert!(findings.iter().any(|f| f.subject == "paramOne"
        && f.caller == "hop1"
        && f.target == "hop2"));
    assert!(findings.iter().any(|f| f.subject == "paramTwo"
        && f.caller == "hop2"
        && f.target == "hop3"));
    assert!(findings.iter().any(|f| f.subject == "paramThree"
        && f.caller == "hop3"
        && f.target == "hop4"));
    assert!(findings.iter().any(|f| f.subject == "paramFour"
        && f.caller == "hop4"
        && f.target == "hop5"));
    assert!(findings.iter().any(|f| f.subject == "paramFive"
        && f.caller == "hop5"
        && f.target == "terminal"));

    // terminal performs local reads, must not be reported
    assert!(!findings.iter().any(|f| f.caller == "terminal"));
}

#[test]
fn test_multihop_branching_and_tree_forwarding() {
    let t = TestDir::new("multihop_branching");
    t.init_tsconfig();
    t.write_file(
        "src/tree.ts",
        r#"
export function root(data: number): void {
    branchLeft(data);
    branchRight(data);
}

function branchLeft(leftData: number): void {
    leafLeft(leftData);
}

function branchRight(rightData: number): void {
    leafRight(rightData);
}

function leafLeft(l: number): void {
    console.log(l + 10);
}

function leafRight(r: number): void {
    console.log(r * 2);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert_eq!(
        findings.len(),
        4,
        "Expected 4 tramp findings (root->branchLeft, root->branchRight, branchLeft->leafLeft, branchRight->leafRight): {report}"
    );

    assert!(findings.iter().any(|f| f.subject == "data"
        && f.caller == "root"
        && f.target == "branchLeft"));
    assert!(findings.iter().any(|f| f.subject == "data"
        && f.caller == "root"
        && f.target == "branchRight"));
    assert!(findings.iter().any(|f| f.subject == "leftData"
        && f.caller == "branchLeft"
        && f.target == "leafLeft"));
    assert!(findings.iter().any(|f| f.subject == "rightData"
        && f.caller == "branchRight"
        && f.target == "leafRight"));

    assert!(!findings.iter().any(|f| f.caller == "leafLeft"));
    assert!(!findings.iter().any(|f| f.caller == "leafRight"));
}

#[test]
fn test_multihop_partial_tramp_mixed_with_local_read() {
    let t = TestDir::new("multihop_partial");
    t.init_tsconfig();
    t.write_file(
        "src/mixed.ts",
        r#"
export function step1(a: string, b: number): void {
    step2(a, b);
}

function step2(x: string, y: number): void {
    // x is read locally, y is purely forwarded
    console.log(x.toLowerCase());
    step3(y);
}

function step3(z: number): void {
    sink(z);
}

function sink(num: number): void {
    console.log(num + 1);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    // In step1: both a and b are tramp data -> 2 findings
    assert!(findings.iter().any(|f| f.caller == "step1"
        && f.subject == "a"
        && f.target == "step2"));
    assert!(findings.iter().any(|f| f.caller == "step1"
        && f.subject == "b"
        && f.target == "step2"));

    // In step2: x is read locally (NOT tramp), y is tramp data -> 1 finding
    assert!(!findings.iter().any(|f| f.caller == "step2" && f.subject == "x"));
    assert!(findings.iter().any(|f| f.caller == "step2"
        && f.subject == "y"
        && f.target == "step3"));

    // In step3: z is tramp data -> 1 finding
    assert!(findings.iter().any(|f| f.caller == "step3"
        && f.subject == "z"
        && f.target == "sink"));

    // In sink: num is read locally -> 0 findings
    assert!(!findings.iter().any(|f| f.caller == "sink"));

    assert_eq!(findings.len(), 4, "Total tramp findings should be 4: {report}");
}

// =========================================================================
// 2. Cross-File Parameter Forwarding Across Modules
// =========================================================================

#[test]
fn test_cross_file_3_module_pipeline() {
    let t = TestDir::new("cross_file_pipeline");
    t.init_tsconfig();

    t.write_file(
        "src/entry.ts",
        r#"
import { handleService } from "./service";

export function handleRequest(token: string): void {
    handleService(token);
}
"#,
    );

    t.write_file(
        "src/service.ts",
        r#"
import { executeDb } from "./db";

export function handleService(authToken: string): void {
    executeDb(authToken);
}
"#,
    );

    t.write_file(
        "src/db.ts",
        r#"
export function executeDb(dbToken: string): void {
    console.log(dbToken.toUpperCase());
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert_eq!(
        findings.len(),
        2,
        "Expected 2 cross-file tramp data findings: {report}"
    );

    let entry_finding = findings.iter().find(|f| f.caller == "handleRequest");
    assert!(entry_finding.is_some());
    let ef = entry_finding.unwrap();
    assert_eq!(ef.subject, "token");
    assert_eq!(ef.target, "handleService");
    assert!(ef.file.contains("entry.ts"));

    let service_finding = findings.iter().find(|f| f.caller == "handleService");
    assert!(service_finding.is_some());
    let sf = service_finding.unwrap();
    assert_eq!(sf.subject, "authToken");
    assert_eq!(sf.target, "executeDb");
    assert!(sf.file.contains("service.ts"));

    assert!(!findings.iter().any(|f| f.caller == "executeDb"));
}

#[test]
fn test_cross_file_multiple_params_selective_consumption() {
    let t = TestDir::new("cross_file_selective");
    t.init_tsconfig();

    t.write_file(
        "src/controller.ts",
        r#"
import { processOrder } from "./order_service";

export function createOrder(userId: string, orderData: { id: string; amount: number }): void {
    processOrder(userId, orderData);
}
"#,
    );

    t.write_file(
        "src/order_service.ts",
        r#"
import { auditLog } from "./audit";

export function processOrder(uid: string, order: { id: string; amount: number }): void {
    // order is read locally, uid is passed through untouched
    console.log(order.amount);
    auditLog(uid);
}
"#,
    );

    t.write_file(
        "src/audit.ts",
        r#"
export function auditLog(actor: string): void {
    console.log(actor.trim());
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    // In controller.ts: both userId and orderData are forwarded to processOrder
    assert!(findings.iter().any(|f| f.caller == "createOrder"
        && f.subject == "userId"
        && f.target == "processOrder"));
    assert!(findings.iter().any(|f| f.caller == "createOrder"
        && f.subject == "orderData"
        && f.target == "processOrder"));

    // In order_service.ts: uid is forwarded to auditLog; order is read locally
    assert!(findings.iter().any(|f| f.caller == "processOrder"
        && f.subject == "uid"
        && f.target == "auditLog"));
    assert!(!findings.iter().any(|f| f.caller == "processOrder" && f.subject == "order"));

    // In audit.ts: actor is read locally
    assert!(!findings.iter().any(|f| f.caller == "auditLog"));

    assert_eq!(findings.len(), 3, "Expected 3 total tramp findings: {report}");
}

#[test]
fn test_cross_file_barrel_and_helper_forwarding() {
    let t = TestDir::new("cross_file_barrel");
    t.init_tsconfig();

    t.write_file(
        "src/helpers/utils.ts",
        r#"
export function sinkHelper(config: { env: string }): void {
    console.log(config.env);
}
"#,
    );

    t.write_file(
        "src/helpers/index.ts",
        r#"
import { sinkHelper } from "./utils";

export function forwardHelper(cfg: { env: string }): void {
    sinkHelper(cfg);
}
"#,
    );

    t.write_file(
        "src/index.ts",
        r#"
import { forwardHelper } from "./helpers/index";

export function mainApp(appConfig: { env: string }): void {
    forwardHelper(appConfig);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert_eq!(findings.len(), 2, "Expected 2 tramp findings: {report}");
    assert!(findings.iter().any(|f| f.caller == "mainApp"
        && f.subject == "appConfig"
        && f.target == "forwardHelper"));
    assert!(findings.iter().any(|f| f.caller == "forwardHelper"
        && f.subject == "cfg"
        && f.target == "sinkHelper"));
}

#[test]
fn test_cross_file_mutual_recursive_forwarding() {
    let t = TestDir::new("cross_file_cycle");
    t.init_tsconfig();

    t.write_file(
        "src/ping.ts",
        r#"
import { pong } from "./pong";

export function ping(msg: string): void {
    pong(msg);
}
"#,
    );

    t.write_file(
        "src/pong.ts",
        r#"
import { ping } from "./ping";

export function pong(msg: string): void {
    ping(msg);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    // Both ping and pong forward msg without reading locally
    assert_eq!(findings.len(), 2, "Expected 2 cyclic tramp findings: {report}");
    assert!(findings.iter().any(|f| f.caller == "ping"
        && f.subject == "msg"
        && f.target == "pong"));
    assert!(findings.iter().any(|f| f.caller == "pong"
        && f.subject == "msg"
        && f.target == "ping"));
}

// =========================================================================
// 3. False Positive Resistance (Local Read Operations)
// =========================================================================

#[test]
fn test_false_positive_property_access_and_optional_chaining() {
    let t = TestDir::new("fp_prop_access");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any): void {}

// Dot access
export function checkDot(item: { a: number }): void {
    const val = item.a;
    target(item);
}

// Bracket access
export function checkBracket(item: { [k: string]: any }): void {
    const val = item["key"];
    target(item);
}

// Optional chaining
export function checkOptChain(item: { nested?: { id: string } }): void {
    const val = item?.nested?.id;
    target(item);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Property access should reject tramp data, got: {report}"
    );
}

#[test]
fn test_false_positive_unary_binary_and_bitwise_operators() {
    let t = TestDir::new("fp_operators");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any): void {}

export function checkUnaryNot(item: boolean): void {
    if (!item) return;
    target(item);
}

export function checkTypeof(item: unknown): void {
    if (typeof item === "string") {
        target(item);
    }
}

export function checkBinaryComparison(num: number): void {
    if (num > 100) {
        target(num);
    }
}

export function checkBitwise(mask: number): void {
    const active = mask & 0x01;
    target(mask);
}

export function checkEquality(item: string | null): void {
    if (item === null) return;
    target(item);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Operators and comparisons must reject tramp data, got: {report}"
    );
}

#[test]
fn test_false_positive_control_flow_and_loops() {
    let t = TestDir::new("fp_control_flow");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any): void {}

export function checkSwitch(action: string): void {
    switch (action) {
        case "A":
            target(action);
            break;
    }
}

export function checkForOf(items: string[]): void {
    for (const x of items) {
        console.log(x);
    }
    target(items);
}

export function checkForLoop(items: string[]): void {
    for (let i = 0; i < items.length; i++) {
        console.log(i);
    }
    target(items);
}

export function checkWhile(cond: boolean): void {
    while (cond) {
        target(cond);
        break;
    }
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Control flow reads must reject tramp data, got: {report}"
    );
}

#[test]
fn test_false_positive_template_literals() {
    let t = TestDir::new("fp_template");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any): void {}

export function checkTemplate(id: string): void {
    target(`ID: ${id}`);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Template literal interpolated parameter is an expression, not direct param forward, got: {report}"
    );
}

#[test]
fn test_false_positive_closure_capture_and_nested_usage() {
    let t = TestDir::new("fp_closure");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any): void {}

export function checkClosure(param: string): void {
    const callback = () => {
        console.log(param);
    };
    callback();
    target(param);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Closure reading param must reject tramp data, got: {report}"
    );
}

#[test]
fn test_false_positive_param_passed_as_callee_or_receiver() {
    let t = TestDir::new("fp_callee");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function checkDirectInvoke(fn: () => void): void {
    fn();
}

export function checkMethodInvoke(obj: { execute: () => void }): void {
    obj.execute();
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Parameter used as callee/receiver must not be tramp data, got: {report}"
    );
}

#[test]
fn test_false_positive_param_passed_and_read_in_same_call() {
    let t = TestDir::new("fp_same_call_read");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function multiTarget(val: string, len: number): void {}

export function checkMultiArgRead(str: string): void {
    // str is passed directly in arg 0, but read in arg 1
    multiTarget(str, str.length);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Parameter read in another argument of same call must reject tramp data: {report}"
    );
}

#[test]
fn test_false_positive_untyped_call_mixed_with_typed() {
    let t = TestDir::new("fp_untyped_mix");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function typedTarget(val: string): void {}

export function checkMixedCalls(str: string): void {
    console.log(str);
    typedTarget(str);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Untyped console.log(str) is a local use / untyped forward, must reject tramp data: {report}"
    );
}

#[test]
fn test_false_positive_shadowed_parameter_in_nested_function() {
    let t = TestDir::new("fp_shadow");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: number): void {}

export function outer(x: string): void {
    function inner(x: number): void {
        target(x);
    }
    inner(42);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    // outer's x is string and NOT forwarded to target
    assert!(
        !findings.iter().any(|f| f.caller == "outer"),
        "outer() parameter x must not be reported: {report}"
    );
    // inner's x is number and IS forwarded to target
    assert!(
        findings.iter().any(|f| f.caller == "inner" && f.subject == "x" && f.target == "target"),
        "inner() forwards x to target, should be reported: {report}"
    );
}

#[test]
fn test_parameter_swapping_and_multi_parameter_pure_forwards() {
    let t = TestDir::new("param_swap");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(second: number, first: string): void {}

export function swapForward(first: string, second: number): void {
    target(second, first);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert_eq!(
        findings.len(),
        2,
        "Both swapped parameters must be reported as tramp data: {report}"
    );
    assert!(findings.iter().any(|f| f.subject == "first"
        && f.caller == "swapForward"
        && f.target == "target"));
    assert!(findings.iter().any(|f| f.subject == "second"
        && f.caller == "swapForward"
        && f.target == "target"));
}

#[test]
fn test_arrow_functions_and_expression_bodies() {
    let t = TestDir::new("arrow_expr");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: string): void {}

export const arrowBlock = (item: string) => {
    target(item);
};

export const arrowExpr = (item: string) => target(item);
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.iter().any(|f| f.subject == "item" && f.target == "target"),
        "Arrow functions with block and expression bodies should detect tramp data: {report}"
    );
}

#[test]
fn test_nested_call_argument_forwarding() {
    let t = TestDir::new("nested_call_forward");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function outer(val: any): void {}
export function inner(val: any): any { return val; }

export function nestedForward(p: string): void {
    outer(inner(p));
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert_eq!(
        findings.len(),
        1,
        "nestedForward should report p forwarded to inner: {report}"
    );
    assert_eq!(findings[0].caller, "nestedForward");
    assert_eq!(findings[0].subject, "p");
    assert_eq!(findings[0].target, "inner");
}

#[test]
fn test_ternary_condition_reads_and_branches() {
    let t = TestDir::new("ternary_eval");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function sink(val: any): void {}
export function fallback(): void {}

// p in ternary condition: NOT tramp data
export function conditionOnly(p: boolean): void {
    p ? sink("yes") : sink("no");
}

// p in branch only: IS tramp data
export function branchForward(cond: boolean, p: string): void {
    cond ? sink(p) : fallback();
}

// p in ternary value expression passed to call: NOT direct param reference
export function ternaryValue(cond: boolean, p: string): void {
    sink(cond ? p : "default");
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        !findings.iter().any(|f| f.caller == "conditionOnly"),
        "conditionOnly should not report tramp data: {report}"
    );
    assert!(
        !findings.iter().any(|f| f.caller == "ternaryValue"),
        "ternaryValue should not report tramp data: {report}"
    );
    assert!(
        findings.iter().any(|f| f.caller == "branchForward" && f.subject == "p" && f.target == "sink"),
        "branchForward should report tramp data: {report}"
    );
}

#[test]
fn test_unused_parameters_never_reported() {
    let t = TestDir::new("unused_only");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function sink(val: any): void {}

export function noop(a: string, b: number, c: boolean): void {}

export function unusedWithCall(unusedA: string, unusedB: number): void {
    sink(42);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Unused parameters must never be reported as tramp data: {report}"
    );
}

#[test]
fn test_untyped_call_alone_never_reported() {
    let t = TestDir::new("untyped_alone");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function untypedLog(p: string): void {
    console.log(p);
}

export function untypedWarn(p: string): void {
    console.warn(p);
}
"#,
    );

    let report = analyze(&t.path).expect("analyze");
    let findings = parse_tramp_findings(&report);

    assert!(
        findings.is_empty(),
        "Untyped calls alone must not report tramp data: {report}"
    );
}

