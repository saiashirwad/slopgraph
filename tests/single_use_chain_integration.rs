//! Integration tests for Milestone M2 (Single-Use Chain detector).

use slopgraph::{analyze_with_options, Options};
use std::fs;
use std::path::PathBuf;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("slopgraph_suc_{}_{}", name, std::process::id()));
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

fn has_finding_shape(report: &str, shape: &str, subject: &str) -> bool {
    let lines: Vec<&str> = report.lines().collect();
    for i in 0..lines.len() {
        if lines[i].trim() == shape && i + 1 < lines.len() {
            let subj_line = lines[i + 1];
            if let Some(rest) = subj_line.strip_prefix("subject: ") {
                let actual_subject = rest.split("  (").next().unwrap_or(rest).trim();
                if actual_subject == subject {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
fn test_linear_deep_single_use_chain() {
    let td = TestDir::new("linear_deep");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function entryPoint(): void {
    step1();
}

function step1(): void {
    step2();
}

function step2(): void {
    step3();
}

function step3(): void {
    step4();
}

function step4(): void {
    console.log("end of chain");
}
"#,
    );

    let report = analyze_with_options(&td.path, Options::default()).unwrap();

    assert!(has_finding_shape(&report, "SINGLE-USE CHAIN", "step1"));
    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "step2"));
    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "step3"));
    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "step4"));

    assert!(report.contains("step1  ←── finding"));
    assert!(report.contains("entryPoint   (exported, not in chain)"));
    assert!(report.contains("step2"));
    assert!(report.contains("step3"));
    assert!(report.contains("step4"));
}

#[test]
fn test_branching_single_use_chain() {
    let td = TestDir::new("branching_chain");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function controller(): void {
    branchRoot();
}

function branchRoot(): void {
    leafA();
    leafB();
}

function leafA(): void {
    console.log("leaf A");
}

function leafB(): void {
    console.log("leaf B");
}
"#,
    );

    let report = analyze_with_options(&td.path, Options::default()).unwrap();

    // Both branchRoot -> leafA and branchRoot -> leafB are >=2 eligible node chains
    assert!(has_finding_shape(&report, "SINGLE-USE CHAIN", "branchRoot"));
    assert!(report.contains("leafA"));
    assert!(report.contains("leafB"));
}

#[test]
fn test_convergence_blocks_single_use_chain() {
    let td = TestDir::new("convergence_blocks");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function entryOne(): void {
    middleOne();
}

export function entryTwo(): void {
    middleTwo();
}

function middleOne(): void {
    sharedLeaf();
}

function middleTwo(): void {
    sharedLeaf();
}

function sharedLeaf(): void {
    console.log("shared by two callers");
}
"#,
    );

    let report = analyze_with_options(&td.path, Options::default()).unwrap();

    // middleOne and middleTwo only call sharedLeaf (in-degree 2).
    // So middleOne has no eligible callee, chain length is 1 (not >= 2).
    // Therefore no SINGLE-USE CHAIN finding.
    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "middleOne"));
    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "middleTwo"));
    assert!(!has_finding_shape(
        &report,
        "SINGLE-USE CHAIN",
        "sharedLeaf"
    ));
}

#[test]
fn test_cross_file_single_use_chain_with_include_exported() {
    let td = TestDir::new("cross_file_exported");
    td.init_tsconfig();

    td.write_file(
        "src/a.ts",
        r#"
import { serviceB } from "./b";

export function apiHandler(): void {
    serviceB();
}
"#,
    );

    td.write_file(
        "src/b.ts",
        r#"
import { repoC } from "./c";

export function serviceB(): void {
    repoC();
}
"#,
    );

    td.write_file(
        "src/c.ts",
        r#"
export function repoC(): void {
    console.log("repo save");
}
"#,
    );

    // Default mode: exported functions excluded, no chain
    let report_default = analyze_with_options(&td.path, Options::default()).unwrap();
    assert!(!has_finding_shape(
        &report_default,
        "SINGLE-USE CHAIN",
        "serviceB"
    ));

    // With --include-exported: serviceB (in-degree 1) and repoC (in-degree 1) form a chain
    let report_included = analyze_with_options(
        &td.path,
        Options {
            include_exported: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(has_finding_shape(
        &report_included,
        "SINGLE-USE CHAIN",
        "serviceB"
    ));
}

#[test]
fn test_deduplication_empty_wrapper_and_chain() {
    let td = TestDir::new("dedup_empty_wrapper");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function router(): void {
    forwarderInChain();
}

function forwarderInChain(): void {
    finalProcessor();
}

function finalProcessor(): void {
    console.log("process");
}

function loneForwarder(): void {
    isolatedTarget();
}

function isolatedTarget(): void {
    console.log("isolated");
}
"#,
    );

    let report = analyze_with_options(&td.path, Options::default()).unwrap();

    // forwarderInChain is inside single-use chain -> SINGLE-USE CHAIN only, no EMPTY WRAPPER
    assert!(has_finding_shape(
        &report,
        "SINGLE-USE CHAIN",
        "forwarderInChain"
    ));
    assert!(!has_finding_shape(
        &report,
        "EMPTY WRAPPER",
        "forwarderInChain"
    ));

    // loneForwarder is in-degree 0 -> EMPTY WRAPPER only
    assert!(has_finding_shape(&report, "EMPTY WRAPPER", "loneForwarder"));
}

#[test]
fn test_test_file_call_increases_in_degree() {
    let td = TestDir::new("test_in_degree");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function mainEntry(): void {
    candidate();
}

export function candidate(): void {
    leaf();
}

function leaf(): void {
    console.log("leaf");
}
"#,
    );

    td.write_file(
        "tests/main.test.ts",
        r#"
import { candidate } from "../src/index";

function runTest(): void {
    candidate();
}

runTest();
"#,
    );

    // candidate is called by mainEntry and runTest (in-degree 2)
    // Therefore candidate cannot be a chain node under include_exported or default
    let report = analyze_with_options(
        &td.path,
        Options {
            include_exported: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "candidate"));
}

#[test]
fn test_isolated_cycle_does_not_loop_or_produce_false_chain() {
    let td = TestDir::new("cycle_chain");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function main(): void {
    console.log("live");
}

function cycleA(): void {
    cycleB();
}

function cycleB(): void {
    cycleA();
}
"#,
    );

    let report = analyze_with_options(&td.path, Options::default()).unwrap();

    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "cycleA"));
    assert!(!has_finding_shape(&report, "SINGLE-USE CHAIN", "cycleB"));
    assert!(has_finding_shape(&report, "UNREACHABLE", "cycleA"));
    assert!(has_finding_shape(&report, "UNREACHABLE", "cycleB"));
}
