//! Empirical adversarial stress testing for Milestone M6: Unreaching Test Detector.

use slopgraph::analyze;
use std::fs;
use std::path::PathBuf;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "slopgraph_adv_unreach_{}_{}_{}",
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
  "include": ["src/**/*", "tests/**/*", "__tests__/**/*"]
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
struct UnreachingFinding {
    test_file: String,
    subject: String,
    line: u32,
}

fn parse_unreaching_findings(report: &str) -> Vec<UnreachingFinding> {
    let mut findings = Vec::new();
    let sections: Vec<&str> = report.split("\n\n").collect();
    let mut current_file: Option<String> = None;

    let mut i = 0;
    while i < sections.len() {
        let sec = sections[i].trim();
        if sec.is_empty() {
            i += 1;
            continue;
        }

        if !sec.contains("UNREACHING TEST")
            && !sec.contains("FALSE SHARING")
            && !sec.contains("UNREACHABLE")
            && !sec.contains("SINGLE USE CHAIN")
            && !sec.contains("NEAR DUPLICATE")
            && !sec.contains("TRAMP DATA")
            && !sec.contains("TYPE CLONE")
        {
            current_file = Some(sec.lines().next().unwrap_or("").trim().to_string());
            i += 1;
            continue;
        }

        if sec.starts_with("UNREACHING TEST") {
            let mut subject = String::new();
            let mut line = 0u32;
            for l in sec.lines() {
                if let Some(rest) = l.strip_prefix("subject: ") {
                    if let Some((subj, line_part)) = rest.split_once("(line ") {
                        subject = subj.trim().to_string();
                        if let Some(line_num_str) = line_part.strip_suffix(')') {
                            line = line_num_str.trim().parse().unwrap_or(0);
                        }
                    } else {
                        subject = rest.trim().to_string();
                    }
                }
            }
            if let Some(ref f) = current_file {
                findings.push(UnreachingFinding {
                    test_file: f.clone(),
                    subject,
                    line,
                });
            }
        }
        i += 1;
    }

    findings
}

#[test]
fn test_deep_transitive_call_chain_through_helpers() {
    let fixture = TestDir::new("deep_chain");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/production.ts",
        r#"export function deepTarget(): number {
  return 100;
}
"#,
    );

    fixture.write_file(
        "src/unused.ts",
        r#"export function unusedProdFn(): number {
  return 999;
}
"#,
    );

    fixture.write_file(
        "tests/shared_helper.ts",
        r#"import { deepTarget } from "../src/production";

export function sharedHelper(): number {
  return deepTarget() + 1;
}
"#,
    );

    fixture.write_file(
        "tests/chain.test.ts",
        r#"import { deepTarget } from "../src/production";
import { unusedProdFn } from "../src/unused";
import { sharedHelper } from "./shared_helper";

function localHelper(): number {
  return sharedHelper() * 2;
}

export function runTest(): void {
  const res = localHelper();
  console.log(res);
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // `src/production.ts` must NOT be flagged because runTest -> localHelper -> sharedHelper -> deepTarget reaches it.
    assert!(
        !findings.iter().any(|f| f.subject == "src/production.ts"),
        "Reached production.ts should not be flagged"
    );

    // `src/unused.ts` MUST be flagged because it is imported on line 2 but never called.
    assert!(
        findings
            .iter()
            .any(|f| f.test_file == "tests/chain.test.ts" && f.subject == "src/unused.ts" && f.line == 2),
        "Unused production module should be flagged at line 2: {:?}",
        findings
    );
}

#[test]
fn test_5_hop_linear_helper_chain() {
    let fixture = TestDir::new("5_hop_chain");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/destination.ts",
        r#"export function destFn(): number {
  return 42;
}
"#,
    );

    fixture.write_file(
        "tests/h4.ts",
        r#"import { destFn } from "../src/destination";
export function f4(): number { return destFn(); }
"#,
    );

    fixture.write_file(
        "tests/h3.ts",
        r#"import { f4 } from "./h4";
export function f3(): number { return f4(); }
"#,
    );

    fixture.write_file(
        "tests/h2.ts",
        r#"import { f3 } from "./h3";
export function f2(): number { return f3(); }
"#,
    );

    fixture.write_file(
        "tests/h1.ts",
        r#"import { f2 } from "./h2";
export function f1(): number { return f2(); }
"#,
    );

    fixture.write_file(
        "tests/hop.test.ts",
        r#"import { destFn } from "../src/destination";
import { f1 } from "./h1";

export function test5Hop(): void {
  f1();
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    assert_eq!(
        findings.len(),
        0,
        "5-hop call chain should successfully reach destination.ts"
    );
}

#[test]
fn test_anonymous_arrow_functions_and_framework_callbacks() {
    let fixture = TestDir::new("callbacks");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/auth.ts",
        r#"export function loginUser(): boolean {
  return true;
}
"#,
    );

    fixture.write_file(
        "src/db.ts",
        r#"export function setupDb(): void {}
"#,
    );

    fixture.write_file(
        "src/calc.ts",
        r#"export function computeSum(a: number, b: number): number {
  return a + b;
}
"#,
    );

    fixture.write_file(
        "src/uncalled.ts",
        r#"export function deadProd(): void {}
"#,
    );

    fixture.write_file(
        "tests/framework.test.ts",
        r#"import { loginUser } from "../src/auth";
import { setupDb } from "../src/db";
import { computeSum } from "../src/calc";
import { deadProd } from "../src/uncalled";

declare function describe(name: string, fn: () => void): void;
declare function beforeEach(fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function it(name: string, fn: () => void): void;

describe("Framework suite", () => {
  beforeEach(() => {
    setupDb();
  });

  test("test with arrow function", () => {
    loginUser();
  });

  it("it with nested arrow function callback", () => {
    [1, 2, 3].forEach((num) => {
      computeSum(num, 10);
    });
  });
});
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // Reached modules via describe/beforeEach/test/it callbacks: auth, db, calc
    assert!(!findings.iter().any(|f| f.subject == "src/auth.ts"));
    assert!(!findings.iter().any(|f| f.subject == "src/db.ts"));
    assert!(!findings.iter().any(|f| f.subject == "src/calc.ts"));

    // Uncalled module: uncalled.ts imported on line 4
    assert!(
        findings.iter().any(
            |f| f.test_file == "tests/framework.test.ts"
                && f.subject == "src/uncalled.ts"
                && f.line == 4
        ),
        "Expected finding on src/uncalled.ts, got: {:?}",
        findings
    );
}

#[test]
fn test_chained_promises_and_async_callbacks() {
    let fixture = TestDir::new("async_promises");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/async_service.ts",
        r#"export async function fetchRemote(): Promise<string> {
  return "remote";
}
"#,
    );

    fixture.write_file(
        "src/error_handler.ts",
        r#"export function handleErr(): void {}
"#,
    );

    fixture.write_file(
        "tests/async.test.ts",
        r#"import { fetchRemote } from "../src/async_service";
import { handleErr } from "../src/error_handler";

export async function testAsync(): Promise<void> {
  return fetchRemote()
    .then((val) => {
      console.log(val);
    })
    .catch(() => {
      handleErr();
    });
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // Both async_service and error_handler are reached inside promise handlers
    assert_eq!(findings.len(), 0);
}

#[test]
fn test_multiple_imports_and_duplicate_import_statements() {
    let fixture = TestDir::new("multi_imports");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/mod_a.ts",
        r#"export function fnA(): string {
  return "A";
}
"#,
    );

    fixture.write_file(
        "src/mod_b.ts",
        r#"export function fnB(): string {
  return "B";
}
"#,
    );

    fixture.write_file(
        "src/mod_c.ts",
        r#"export function fnC(): string {
  return "C";
}
"#,
    );

    fixture.write_file(
        "tests/multiple.test.ts",
        r#"import { fnA } from "../src/mod_a";
import { fnB } from "../src/mod_b";
import { fnC } from "../src/mod_c";

export function testRun(): void {
  // Only fnB is called
  console.log(fnB());
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // fnB was called, so mod_b is NOT unreached
    assert!(!findings.iter().any(|f| f.subject == "src/mod_b.ts"));

    // mod_a and mod_c are unreached
    let finding_a = findings
        .iter()
        .find(|f| f.subject == "src/mod_a.ts")
        .expect("mod_a should be flagged");
    assert_eq!(finding_a.line, 1);

    let finding_c = findings
        .iter()
        .find(|f| f.subject == "src/mod_c.ts")
        .expect("mod_c should be flagged");
    assert_eq!(finding_c.line, 3);
}

#[test]
fn test_diamond_reachability_graph() {
    let fixture = TestDir::new("diamond");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/core.ts",
        r#"export function coreOperation(): number {
  return 42;
}
"#,
    );

    fixture.write_file(
        "tests/helper_left.ts",
        r#"import { coreOperation } from "../src/core";

export function leftHelper(): number {
  return coreOperation() + 1;
}
"#,
    );

    fixture.write_file(
        "tests/helper_right.ts",
        r#"import { coreOperation } from "../src/core";

export function rightHelper(): number {
  return coreOperation() * 2;
}
"#,
    );

    fixture.write_file(
        "tests/diamond.test.ts",
        r#"import { coreOperation } from "../src/core";
import { leftHelper } from "./helper_left";
import { rightHelper } from "./helper_right";

export function runDiamondTest(): void {
  leftHelper();
  rightHelper();
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // core.ts is reachable through both left and right helper paths
    assert!(
        !findings.iter().any(|f| f.subject == "src/core.ts"),
        "core.ts should be reached via diamond call paths"
    );
    assert_eq!(findings.len(), 0);
}

#[test]
fn test_various_test_file_naming_conventions() {
    let fixture = TestDir::new("test_naming");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/service.ts",
        r#"export function serviceFn(): string {
  return "service";
}
"#,
    );

    fixture.write_file(
        "src/unused.ts",
        r#"export function unusedFn(): string {
  return "unused";
}
"#,
    );

    // 1. __tests__ directory
    fixture.write_file(
        "__tests__/service.ts",
        r#"import { serviceFn } from "../src/service";
import { unusedFn } from "../src/unused";

export function testInUnderUnder(): void {
  serviceFn();
}
"#,
    );

    // 2. spec.ts in src/
    fixture.write_file(
        "src/component.spec.ts",
        r#"import { serviceFn } from "./service";
import { unusedFn } from "./unused";

export function testInSpec(): void {
  serviceFn();
}
"#,
    );

    // 3. test.tsx in src/
    fixture.write_file(
        "src/widget.test.tsx",
        r#"import { serviceFn } from "./service";
import { unusedFn } from "./unused";

export function testInWidget(): void {
  serviceFn();
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // All three test files reach service.ts, but none call unused.ts
    assert!(!findings.iter().any(|f| f.subject == "src/service.ts"));

    assert!(findings.iter().any(|f| f.test_file == "__tests__/service.ts" && f.subject == "src/unused.ts"));
    assert!(findings.iter().any(|f| f.test_file == "src/component.spec.ts" && f.subject == "src/unused.ts"));
    assert!(findings.iter().any(|f| f.test_file == "src/widget.test.tsx" && f.subject == "src/unused.ts"));
    assert_eq!(findings.len(), 3);
}

#[test]
fn test_reexported_forwarding_function_in_production() {
    let fixture = TestDir::new("reexport_forward");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/internal.ts",
        r#"export function internalAction(): string {
  return "action";
}
"#,
    );

    // Barrel module that calls or forwards internal action
    fixture.write_file(
        "src/barrel.ts",
        r#"import { internalAction } from "./internal";

export function barrelAction(): string {
  return internalAction();
}
"#,
    );

    fixture.write_file(
        "tests/barrel.test.ts",
        r#"import { barrelAction } from "../src/barrel";

export function testBarrel(): void {
  barrelAction();
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // Both barrel.ts and internal.ts are reached
    assert_eq!(findings.len(), 0);
}

#[test]
fn test_complex_closure_and_method_calls() {
    let fixture = TestDir::new("closures_methods");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/client.ts",
        r#"export class ApiClient {
  fetchData(): string {
    return "data";
  }
}

export function createClient(): ApiClient {
  return new ApiClient();
}
"#,
    );

    fixture.write_file(
        "src/dead_client.ts",
        r#"export function unusedClient(): void {}
"#,
    );

    fixture.write_file(
        "tests/client.test.ts",
        r#"import { createClient } from "../src/client";
import { unusedClient } from "../src/dead_client";

export function testWithClosure(): void {
  const runner = () => {
    const client = createClient();
    return client.fetchData();
  };
  runner();
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    assert!(!findings.iter().any(|f| f.subject == "src/client.ts"));
    assert!(
        findings
            .iter()
            .any(|f| f.test_file == "tests/client.test.ts" && f.subject == "src/dead_client.ts" && f.line == 2)
    );
}

#[test]
fn test_type_only_module_import_is_reported() {
    let fixture = TestDir::new("type_only_module");
    fixture.init_tsconfig();

    fixture.write_file(
        "src/types.ts",
        r#"export interface Config {
  host: string;
  port: number;
}
"#,
    );

    fixture.write_file(
        "tests/types.test.ts",
        r#"import { Config } from "../src/types";

export function testTypeUsage(): void {
  const cfg: Config = { host: "localhost", port: 8080 };
  console.log(cfg);
}
"#,
    );

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].subject, "src/types.ts");
    assert_eq!(findings[0].line, 1);
}

#[test]
fn test_large_scale_unreaching_detector_stress() {
    let fixture = TestDir::new("scale_stress");
    fixture.init_tsconfig();

    // Create 30 production modules
    for i in 0..30 {
        fixture.write_file(
            &format!("src/prod_{}.ts", i),
            &format!(
                "export function prodFn_{}(): number {{\n  return {};\n}}\n",
                i, i
            ),
        );
    }

    // Create 30 test files:
    // Evens (0, 2, 4...) import prod_i and CALL prodFn_i
    // Odds (1, 3, 5...) import prod_i and DO NOT CALL prodFn_i
    for i in 0..30 {
        if i % 2 == 0 {
            fixture.write_file(
                &format!("tests/test_{}.test.ts", i),
                &format!(
                    r#"import {{ prodFn_{} }} from "../src/prod_{}";

export function runTest_{}(): void {{
  prodFn_{}();
}}
"#,
                    i, i, i, i
                ),
            );
        } else {
            fixture.write_file(
                &format!("tests/test_{}.test.ts", i),
                &format!(
                    r#"import {{ prodFn_{} }} from "../src/prod_{}";

export function runTest_{}(): void {{
  console.log("No call made here");
}}
"#,
                    i, i, i
                ),
            );
        }
    }

    let report = analyze(&fixture.path).expect("analyze should succeed");
    let findings = parse_unreaching_findings(&report);

    // Exactly 15 odd test files must be reported as UNREACHING TEST
    assert_eq!(
        findings.len(),
        15,
        "Expected exactly 15 unreaching test findings, got {}: {:?}",
        findings.len(),
        findings
    );

    for i in 0..30 {
        let expected_subject = format!("src/prod_{}.ts", i);
        let expected_test_file = format!("tests/test_{}.test.ts", i);
        if i % 2 == 0 {
            assert!(
                !findings.iter().any(|f| f.subject == expected_subject),
                "Even prod_{} should have been reached",
                i
            );
        } else {
            assert!(
                findings
                    .iter()
                    .any(|f| f.test_file == expected_test_file && f.subject == expected_subject && f.line == 1),
                "Odd prod_{} should be flagged in test_{}.test.ts at line 1",
                i,
                i
            );
        }
    }
}
