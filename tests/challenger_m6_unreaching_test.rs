//! Empirical Challenger Test Suite for Milestone M6: Unreaching Test Detector (Issue #21).

use slopgraph::{analyze, analyze_with_options, Options};
use std::fs;
use std::path::PathBuf;

struct TestWorkspace {
    path: PathBuf,
}

impl TestWorkspace {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("slopgraph_chal_ut_{}_{}", name, std::process::id()));
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

fn parse_unreaching_test_findings(report: &str) -> Vec<(String, String, u32)> {
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
        } else if line == "UNREACHING TEST" {
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
            if !current_file.is_empty() && !subject.is_empty() {
                findings.push((current_file.clone(), subject, line_num));
            }
        }
        i += 1;
    }
    findings
}

// -----------------------------------------------------------------------------
// Category 1: Type Declarations and Interfaces vs Functions
// -----------------------------------------------------------------------------

#[test]
fn test_importing_type_declarations_only_is_reported_when_no_calls() {
    let ws = TestWorkspace::new("type_decl_no_calls");
    ws.init_tsconfig();

    ws.write_file(
        "src/types.ts",
        r#"
export interface UserConfig {
    id: string;
    endpoint: string;
    retries: number;
}

export type Status = "active" | "inactive";
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { UserConfig, Status } from "./types";

export function initConfig(): UserConfig {
    return { id: "1", endpoint: "http://localhost", retries: 3 };
}
"#,
    );

    ws.write_file(
        "tests/types.test.ts",
        r#"
import { UserConfig, Status } from "../src/types";

export function testTypeCheck() {
    const config: UserConfig = {
        id: "test",
        endpoint: "http://test",
        retries: 1,
    };
    const s: Status = "active";
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Because src/types.ts has 0 functions, zero typed call edges reach it.
    // Therefore, tests/types.test.ts importing src/types.ts is reported as UNREACHING TEST.
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "tests/types.test.ts");
    assert_eq!(findings[0].1, "src/types.ts");
    assert_eq!(findings[0].2, 2);
}

#[test]
fn test_importing_module_with_functions_and_types_used_only_as_type() {
    let ws = TestWorkspace::new("module_types_and_fns_used_as_type");
    ws.init_tsconfig();

    ws.write_file(
        "src/user_service.ts",
        r#"
export interface UserRecord {
    id: string;
    name: string;
}

export function fetchUser(id: string): UserRecord {
    return { id, name: "Alice" };
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { fetchUser } from "./user_service";
export function main() {
    return fetchUser("1");
}
"#,
    );

    // Test only uses interface UserRecord without calling fetchUser
    ws.write_file(
        "tests/user_service.test.ts",
        r#"
import { UserRecord } from "../src/user_service";

export function testUserMock() {
    const mockUser: UserRecord = { id: "mock-1", name: "Mock" };
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Unreaching test must be reported because fetchUser is NOT called from tests/user_service.test.ts
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "tests/user_service.test.ts");
    assert_eq!(findings[0].1, "src/user_service.ts");
}

#[test]
fn test_importing_module_with_function_called_is_not_reported() {
    let ws = TestWorkspace::new("module_fn_called");
    ws.init_tsconfig();

    ws.write_file(
        "src/user_service.ts",
        r#"
export interface UserRecord {
    id: string;
    name: string;
}

export function fetchUser(id: string): UserRecord {
    return { id, name: "Alice" };
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { fetchUser } from "./user_service";
export function run() {
    return fetchUser("admin");
}
"#,
    );

    ws.write_file(
        "tests/user_service.test.ts",
        r#"
import { fetchUser, UserRecord } from "../src/user_service";

export function testUser() {
    const user: UserRecord = fetchUser("test-id");
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // No unreaching test findings should be emitted!
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

// -----------------------------------------------------------------------------
// Category 2: Multiple Production Modules (Selective Reachability)
// -----------------------------------------------------------------------------

#[test]
fn test_multiple_production_modules_selective_calls() {
    let ws = TestWorkspace::new("multi_mod_selective");
    ws.init_tsconfig();

    ws.write_file(
        "src/auth.ts",
        r#"
export function login(user: string): boolean {
    return user === "admin";
}
"#,
    );

    ws.write_file(
        "src/db.ts",
        r#"
export function queryDb(sql: string): string[] {
    return [sql];
}
"#,
    );

    ws.write_file(
        "src/payment.ts",
        r#"
export function processPayment(amount: number): boolean {
    return amount > 0;
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { login } from "./auth";
import { queryDb } from "./db";
import { processPayment } from "./payment";

export function app() {
    login("u");
    queryDb("SELECT 1");
    processPayment(100);
}
"#,
    );

    // Test file imports all 3 modules, but only calls login (src/auth.ts) and processPayment (src/payment.ts)
    // src/db.ts is NOT called
    ws.write_file(
        "tests/app.test.ts",
        r#"
import { login } from "../src/auth";
import { queryDb } from "../src/db";
import { processPayment } from "../src/payment";

export function testApp() {
    login("test");
    processPayment(50);
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Exactly 1 finding for src/db.ts
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "tests/app.test.ts");
    assert_eq!(findings[0].1, "src/db.ts");
    assert_eq!(findings[0].2, 3);
}

#[test]
fn test_multiple_test_files_with_distinct_reachability() {
    let ws = TestWorkspace::new("multi_test_files");
    ws.init_tsconfig();

    ws.write_file(
        "src/service.ts",
        r#"
export function executeService(): string {
    return "ok";
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { executeService } from "./service";
export function main() {
    return executeService();
}
"#,
    );

    // Test 1 calls service
    ws.write_file(
        "tests/valid.test.ts",
        r#"
import { executeService } from "../src/service";

export function testValid() {
    executeService();
}
"#,
    );

    // Test 2 imports service but does NOT call it
    ws.write_file(
        "tests/invalid.test.ts",
        r#"
import { executeService } from "../src/service";

export function testInvalid() {
    const dummy = 1 + 1;
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Only tests/invalid.test.ts should produce a finding
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "tests/invalid.test.ts");
    assert_eq!(findings[0].1, "src/service.ts");
}

// -----------------------------------------------------------------------------
// Category 3: External Library Imports (vitest, lodash, node built-ins)
// -----------------------------------------------------------------------------

#[test]
fn test_external_library_imports_never_reported_or_crash() {
    let ws = TestWorkspace::new("external_imports");
    ws.init_tsconfig();

    ws.write_file(
        "src/calc.ts",
        r#"
export function add(a: number, b: number): number {
    return a + b;
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { add } from "./calc";
export function main() {
    return add(1, 2);
}
"#,
    );

    // Test file imports external libraries (node:path, lodash, vitest) along with src/calc.ts
    ws.write_file(
        "tests/calc.test.ts",
        r#"
import { describe, it, expect } from "vitest";
import _ from "lodash";
import * as path from "node:path";
import { add } from "../src/calc";

export function runTests() {
    const result = add(2, 3);
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Zero findings should exist (external libraries are ignored, calc is reached)
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

#[test]
fn test_external_library_with_unreached_prod_import() {
    let ws = TestWorkspace::new("external_with_unreached");
    ws.init_tsconfig();

    ws.write_file(
        "src/calc.ts",
        r#"
export function add(a: number, b: number): number {
    return a + b;
}
"#,
    );

    ws.write_file(
        "src/unused.ts",
        r#"
export function subtract(a: number, b: number): number {
    return a - b;
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { add } from "./calc";
import { subtract } from "./unused";
export function main() {
    add(1, 2);
    subtract(5, 3);
}
"#,
    );

    ws.write_file(
        "tests/calc.test.ts",
        r#"
import { describe, it, expect } from "vitest";
import { add } from "../src/calc";
import { subtract } from "../src/unused";

export function runTests() {
    add(1, 2);
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Exactly 1 finding for src/unused.ts; "vitest" is NOT reported
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "tests/calc.test.ts");
    assert_eq!(findings[0].1, "src/unused.ts");
}

// -----------------------------------------------------------------------------
// Category 4: Production-to-Production Imports (Negative Constraint)
// -----------------------------------------------------------------------------

#[test]
fn test_production_to_production_imports_never_emit_unreaching_test() {
    let ws = TestWorkspace::new("prod_to_prod");
    ws.init_tsconfig();

    ws.write_file(
        "src/helper.ts",
        r#"
export function helperUtil(): string {
    return "util";
}
"#,
    );

    ws.write_file(
        "src/service.ts",
        r#"
import { helperUtil } from "./helper";

// service.ts imports helper.ts but does NOT call helperUtil()
export function doService(): string {
    return "service";
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { doService } from "./service";
export function main() {
    return doService();
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Unreaching test detector must NOT emit findings for production modules
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

// -----------------------------------------------------------------------------
// Category 5: Transitive Reachability via Test Helpers and Production Modules
// -----------------------------------------------------------------------------

#[test]
fn test_transitive_reachability_via_test_helper() {
    let ws = TestWorkspace::new("transitive_helper");
    ws.init_tsconfig();

    ws.write_file(
        "src/db.ts",
        r#"
export function connectDb(): boolean {
    return true;
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { connectDb } from "./db";
export function main() {
    return connectDb();
}
"#,
    );

    // Test helper imports db.ts and calls connectDb()
    ws.write_file(
        "tests/fixture_helper.ts",
        r#"
import { connectDb } from "../src/db";

export function setupDatabase() {
    connectDb();
}
"#,
    );

    // Test file imports db.ts and fixture_helper.ts, calls setupDatabase()
    ws.write_file(
        "tests/db.test.ts",
        r#"
import { connectDb } from "../src/db";
import { setupDatabase } from "./fixture_helper";

export function runTest() {
    setupDatabase();
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // connectDb is reachable transitively from tests/db.test.ts -> setupDatabase -> connectDb!
    // And tests/fixture_helper.ts is in test_files, so it is never reported.
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

#[test]
fn test_transitive_reachability_via_production_modules() {
    let ws = TestWorkspace::new("transitive_prod");
    ws.init_tsconfig();

    ws.write_file(
        "src/repo.ts",
        r#"
export function executeSql(q: string): string {
    return q;
}
"#,
    );

    ws.write_file(
        "src/controller.ts",
        r#"
import { executeSql } from "./repo";

export function handleRequest(): string {
    return executeSql("SELECT 1");
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { handleRequest } from "./controller";
export function main() {
    return handleRequest();
}
"#,
    );

    // Test imports controller AND repo, but only directly calls controller
    ws.write_file(
        "tests/controller.test.ts",
        r#"
import { handleRequest } from "../src/controller";
import { executeSql } from "../src/repo";

export function testController() {
    handleRequest();
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Both controller.ts and repo.ts are transitively reached via handleRequest -> executeSql
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

// -----------------------------------------------------------------------------
// Category 6: Production Mode Evaluation
// -----------------------------------------------------------------------------

#[test]
fn test_production_mode_still_evaluates_unreaching_tests() {
    let ws = TestWorkspace::new("production_mode");
    ws.init_tsconfig();

    ws.write_file(
        "src/engine.ts",
        r#"
export function runEngine(): void {}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { runEngine } from "./engine";
export function main() {
    runEngine();
}
"#,
    );

    ws.write_file(
        "tests/engine.test.ts",
        r#"
import { runEngine } from "../src/engine";

export function testEngine() {
    // Zero calls to runEngine
}
"#,
    );

    let options = Options {
        production: true,
        ..Default::default()
    };
    let report = analyze_with_options(&ws.path, options).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "tests/engine.test.ts");
    assert_eq!(findings[0].1, "src/engine.ts");
}

// -----------------------------------------------------------------------------
// Category 7: Test Function Structures (describe/it callbacks, arrow functions)
// -----------------------------------------------------------------------------

#[test]
fn test_various_function_syntaxes_in_tests() {
    let ws = TestWorkspace::new("function_syntaxes");
    ws.init_tsconfig();

    ws.write_file(
        "src/api.ts",
        r#"
export function apiCall(): boolean {
    return true;
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { apiCall } from "./api";
export function main() {
    return apiCall();
}
"#,
    );

    // Framework-like callbacks inside test files
    ws.write_file(
        "tests/api.test.ts",
        r#"
import { apiCall } from "../src/api";

function describe(name: string, fn: () => void) { fn(); }
function it(name: string, fn: () => void) { fn(); }

describe("API Suite", () => {
    it("should call api", () => {
        apiCall();
    });
});
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // apiCall is called inside the nested arrow function in describe/it callback,
    // which is extracted as a function node in the test file.
    // Reachability from test file functions reaches apiCall!
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

// -----------------------------------------------------------------------------
// Category 8: Namespace, Default, and Aliased Imports
// -----------------------------------------------------------------------------

#[test]
fn test_aliased_and_namespace_imports_reachability() {
    let ws = TestWorkspace::new("alias_and_namespace");
    ws.init_tsconfig();

    ws.write_file(
        "src/math.ts",
        r#"
export function computeSum(a: number, b: number): number {
    return a + b;
}
export default function defaultMultiply(a: number, b: number): number {
    return a * b;
}
"#,
    );

    ws.write_file(
        "src/unused_helper.ts",
        r#"
export function unusedUtil(): void {}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import computeSum, { computeSum as sum } from "./math";
import { unusedUtil } from "./unused_helper";
export function main() {
    sum(1, 2);
    unusedUtil();
}
"#,
    );

    // Test imports math as namespace and default, and unused_helper as alias
    ws.write_file(
        "tests/math.test.ts",
        r#"
import * as MathLib from "../src/math";
import { unusedUtil as dummy } from "../src/unused_helper";

export function testMath() {
    MathLib.computeSum(10, 20);
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Math is reached, unused_helper is unreached
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "tests/math.test.ts");
    assert_eq!(findings[0].1, "src/unused_helper.ts");
}

// -----------------------------------------------------------------------------
// Category 9: Deterministic Finding Sorting across Multiple Files and Spans
// -----------------------------------------------------------------------------

#[test]
fn test_deterministic_multi_file_and_line_ordering() {
    let ws = TestWorkspace::new("deterministic_ordering");
    ws.init_tsconfig();

    ws.write_file("src/a.ts", "export function fnA() {}");
    ws.write_file("src/b.ts", "export function fnB() {}");
    ws.write_file("src/c.ts", "export function fnC() {}");
    ws.write_file(
        "src/index.ts",
        r#"
import { fnA } from "./a";
import { fnB } from "./b";
import { fnC } from "./c";
export function main() { fnA(); fnB(); fnC(); }
"#,
    );

    ws.write_file(
        "tests/z_test.test.ts",
        r#"
import { fnB } from "../src/b";
import { fnA } from "../src/a";

export function runZ() {}
"#,
    );

    ws.write_file(
        "tests/a_test.test.ts",
        r#"
import { fnC } from "../src/c";

export function runA() {}
"#,
    );

    let report1 = analyze(&ws.path).unwrap();
    let report2 = analyze(&ws.path).unwrap();

    // Deterministic outputs
    assert_eq!(report1, report2);

    let findings = parse_unreaching_test_findings(&report1);
    assert_eq!(findings.len(), 3);

    // Sorted by test file path: "tests/a_test.test.ts", then "tests/z_test.test.ts"
    assert_eq!(findings[0].0, "tests/a_test.test.ts");
    assert_eq!(findings[0].1, "src/c.ts");

    // In tests/z_test.test.ts: line 2 (src/b.ts) comes before line 3 (src/a.ts)
    assert_eq!(findings[1].0, "tests/z_test.test.ts");
    assert_eq!(findings[1].1, "src/b.ts");
    assert_eq!(findings[1].2, 2);

    assert_eq!(findings[2].0, "tests/z_test.test.ts");
    assert_eq!(findings[2].1, "src/a.ts");
    assert_eq!(findings[2].2, 3);
}

// -----------------------------------------------------------------------------
// Category 10: Call Cycles between Production Modules Reached from Test
// -----------------------------------------------------------------------------

#[test]
fn test_call_cycles_between_production_modules_do_not_infinite_loop() {
    let ws = TestWorkspace::new("call_cycles");
    ws.init_tsconfig();

    ws.write_file(
        "src/cycle_a.ts",
        r#"
import { cycleB } from "./cycle_b";
export function cycleA(n: number): number {
    if (n <= 0) return 0;
    return cycleB(n - 1);
}
"#,
    );

    ws.write_file(
        "src/cycle_b.ts",
        r#"
import { cycleA } from "./cycle_a";
export function cycleB(n: number): number {
    if (n <= 0) return 0;
    return cycleA(n - 1);
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { cycleA } from "./cycle_a";
export function main() { cycleA(5); }
"#,
    );

    ws.write_file(
        "tests/cycle.test.ts",
        r#"
import { cycleA } from "../src/cycle_a";
import { cycleB } from "../src/cycle_b";

export function testCycle() {
    cycleA(3);
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Both cycle_a and cycle_b are reached (cycleA calls cycleB)
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

// -----------------------------------------------------------------------------
// Category 11: Async/Await and Try-Catch Calls
// -----------------------------------------------------------------------------

#[test]
fn test_async_await_and_try_catch_calls() {
    let ws = TestWorkspace::new("async_try_catch");
    ws.init_tsconfig();

    ws.write_file(
        "src/async_service.ts",
        r#"
export async function fetchData(): Promise<string> {
    return "data";
}
export async function failData(): Promise<void> {
    throw new Error("fail");
}
"#,
    );

    ws.write_file(
        "src/index.ts",
        r#"
import { fetchData, failData } from "./async_service";
export async function main() {
    await fetchData();
    try {
        await failData();
    } catch {}
}
"#,
    );

    ws.write_file(
        "tests/async.test.ts",
        r#"
import { fetchData, failData } from "../src/async_service";

export async function testAsync() {
    try {
        const val = await fetchData();
        await failData();
    } catch (e) {
        console.error(e);
    }
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let findings = parse_unreaching_test_findings(&report);

    // Both async methods in src/async_service.ts are called
    assert!(findings.is_empty(), "Expected 0 findings, got: {:?}", findings);
}

// -----------------------------------------------------------------------------
// Category 12: High-Volume Multi-File Stress Test
// -----------------------------------------------------------------------------

#[test]
fn test_high_volume_tests_and_modules_stress() {
    let ws = TestWorkspace::new("high_volume_stress");
    ws.init_tsconfig();

    // Create 10 production modules
    for i in 0..10 {
        let content = format!(
            r#"
export function prodFn_{}() {{
    return {};
}}
"#,
            i, i
        );
        ws.write_file(&format!("src/mod_{}.ts", i), &content);
    }

    // src/index.ts calls all 10
    let mut index_content = String::new();
    for i in 0..10 {
        index_content.push_str(&format!("import {{ prodFn_{} }} from \"./mod_{}\";\n", i, i));
    }
    index_content.push_str("export function main() {\n");
    for i in 0..10 {
        index_content.push_str(&format!("    prodFn_{}();\n", i));
    }
    index_content.push_str("}\n");
    ws.write_file("src/index.ts", &index_content);

    // Create 10 test files:
    // test_i.test.ts imports mod_i and mod_{(i+1)%10}.
    // It ONLY calls prodFn_i, and does NOT call prodFn_{(i+1)%10}.
    // Therefore, each of the 10 test files produces exactly 1 UNREACHING TEST finding for mod_{(i+1)%10}.
    for i in 0..10 {
        let next = (i + 1) % 10;
        let test_content = format!(
            r#"
import {{ prodFn_{} }} from "../src/mod_{}";
import {{ prodFn_{} }} from "../src/mod_{}";

export function runTest_{}() {{
    prodFn_{}();
}}
"#,
            i, i, next, next, i, i
        );
        ws.write_file(&format!("tests/test_{}.test.ts", i), &test_content);
    }

    let report1 = analyze(&ws.path).unwrap();
    let report2 = analyze(&ws.path).unwrap();

    assert_eq!(report1, report2);

    let findings = parse_unreaching_test_findings(&report1);

    // Exactly 10 findings (1 per test file)
    assert_eq!(findings.len(), 10);
    for finding in &findings {
        assert!(finding.0.starts_with("tests/test_"));
        assert!(finding.1.starts_with("src/mod_"));
    }
}


