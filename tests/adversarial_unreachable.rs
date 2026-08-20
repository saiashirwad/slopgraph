//! Comprehensive empirical adversarial challenge tests for Milestone M1 (Unreachable detector).

use slopgraph::{analyze_with_options, Options};
use std::fs;
use std::path::PathBuf;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("slopgraph_adv_{}_{}", name, std::process::id()));
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

fn has_unreachable_finding(report: &str, subject: &str) -> bool {
    let lines: Vec<&str> = report.lines().collect();
    for i in 0..lines.len() {
        if lines[i].trim() == "UNREACHABLE" && i + 1 < lines.len() {
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
fn test_unreachable_call_cycle_disconnected() {
    let td = TestDir::new("cycle_disconnected");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
import { liveFunc } from "./live";
export function entry(): void {
    const x = 1;
    liveFunc();
}
"#,
    );

    td.write_file(
        "src/live.ts",
        r#"
export function liveFunc(): void {
    const a = 1;
}
export function deadCycleA(): void {
    deadCycleB();
}
export function deadCycleB(): void {
    deadCycleC();
}
export function deadCycleC(): void {
    deadCycleA();
}
"#,
    );

    let report_default = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(has_unreachable_finding(&report_default, "deadCycleA"));
    assert!(has_unreachable_finding(&report_default, "deadCycleB"));
    assert!(has_unreachable_finding(&report_default, "deadCycleC"));
    assert!(!has_unreachable_finding(&report_default, "liveFunc"));
    assert!(!has_unreachable_finding(&report_default, "entry"));
}

#[test]
fn test_reachable_call_cycle_connected_to_entry() {
    let td = TestDir::new("cycle_connected");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
import { cycleA } from "./cycle";
export function main(): void {
    const x = 1;
    cycleA();
}
"#,
    );

    td.write_file(
        "src/cycle.ts",
        r#"
export function cycleA(): void {
    cycleB();
}
export function cycleB(): void {
    cycleC();
}
export function cycleC(): void {
    cycleA();
}
export function unreachedInCycleFile(): void {}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!has_unreachable_finding(&report, "cycleA"));
    assert!(!has_unreachable_finding(&report, "cycleB"));
    assert!(!has_unreachable_finding(&report, "cycleC"));
    assert!(!has_unreachable_finding(&report, "main"));
    assert!(has_unreachable_finding(&report, "unreachedInCycleFile"));
}

#[test]
fn test_unreachable_file_suppresses_its_functions() {
    let td = TestDir::new("suppression_check");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function main(): void {}
"#,
    );

    td.write_file(
        "src/orphan.ts",
        r#"
export function orphanFunc1(): void {
    orphanFunc2();
}
export function orphanFunc2(): void {}
export function orphanFunc3(): void {}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(has_unreachable_finding(&report, "src/orphan.ts"));
    assert!(!has_unreachable_finding(&report, "orphanFunc1"));
    assert!(!has_unreachable_finding(&report, "orphanFunc2"));
    assert!(!has_unreachable_finding(&report, "orphanFunc3"));
}

#[test]
fn test_multiple_package_json_entry_points() {
    let td = TestDir::new("pkg_entries");
    td.init_tsconfig();

    td.write_file(
        "package.json",
        r#"{
  "name": "multi-entry-test",
  "main": "./src/main_entry.ts",
  "bin": {
    "cli1": "./src/cli_entry.ts"
  },
  "exports": {
    ".": "./src/main_entry.ts",
    "./sub": "./src/sub_entry.ts"
  }
}"#,
    );

    td.write_file(
        "src/main_entry.ts",
        r#"
import { fromMain } from "./shared";
export function runMain(): void {
    const x = 1;
    fromMain();
}
"#,
    );

    td.write_file(
        "src/cli_entry.ts",
        r#"
import { fromCli } from "./shared";
export function runCli(): void {
    const y = 2;
    fromCli();
}
"#,
    );

    td.write_file(
        "src/sub_entry.ts",
        r#"
import { fromSub } from "./shared";
export function runSub(): void {
    const z = 3;
    fromSub();
}
"#,
    );

    td.write_file(
        "src/shared.ts",
        r#"
export function fromMain(): void {}
export function fromCli(): void {}
export function fromSub(): void {}
export function deadShared(): void {}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!has_unreachable_finding(&report, "fromMain"));
    assert!(!has_unreachable_finding(&report, "fromCli"));
    assert!(!has_unreachable_finding(&report, "fromSub"));
    assert!(has_unreachable_finding(&report, "deadShared"));
}

#[test]
fn test_package_json_dist_js_mapped_to_src_ts() {
    let td = TestDir::new("dist_js_mapping");
    td.init_tsconfig();

    td.write_file(
        "package.json",
        r#"{
  "name": "dist-mapping-test",
  "main": "./dist/index.js",
  "bin": "./dist/cli.js"
}"#,
    );

    td.write_file(
        "src/index.ts",
        r#"
import { indexHelper } from "./helper";
export function entry(): void {
    const a = 1;
    indexHelper();
}
"#,
    );

    td.write_file(
        "src/cli.ts",
        r#"
import { cliHelper } from "./helper";
export function run(): void {
    const b = 2;
    cliHelper();
}
"#,
    );

    td.write_file(
        "src/helper.ts",
        r#"
export function indexHelper(): void {}
export function cliHelper(): void {}
export function unreachedHelper(): void {}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!has_unreachable_finding(&report, "entry"));
    assert!(!has_unreachable_finding(&report, "run"));
    assert!(!has_unreachable_finding(&report, "indexHelper"));
    assert!(!has_unreachable_finding(&report, "cliHelper"));
    assert!(has_unreachable_finding(&report, "unreachedHelper"));
}

#[test]
fn test_production_mode_drops_test_roots_and_preserves_test_files() {
    let td = TestDir::new("prod_mode_test");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
import { prodReachable } from "./service";
export function main(): void {
    const x = 1;
    prodReachable();
}
"#,
    );

    td.write_file(
        "src/service.ts",
        r#"
export function prodReachable(): void {}
export function testOnlyReachable(): void {}
"#,
    );

    td.write_file(
        "src/test_helper.ts",
        r#"
export function helperForTests(): void {}
"#,
    );

    td.write_file(
        "tests/suite.test.ts",
        r#"
import { testOnlyReachable } from "../src/service";
import { helperForTests } from "../src/test_helper";

export function runTests(): void {
    testOnlyReachable();
    helperForTests();
}
"#,
    );

    let report_dev = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!has_unreachable_finding(&report_dev, "testOnlyReachable"));
    assert!(!has_unreachable_finding(&report_dev, "helperForTests"));
    assert!(!has_unreachable_finding(&report_dev, "src/test_helper.ts"));
    assert!(!has_unreachable_finding(&report_dev, "tests/suite.test.ts"));

    let report_prod = analyze_with_options(
        &td.path,
        Options {
            production: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(has_unreachable_finding(&report_prod, "testOnlyReachable"));
    assert!(has_unreachable_finding(&report_prod, "src/test_helper.ts"));
    assert!(!has_unreachable_finding(&report_prod, "helperForTests"));
    assert!(!has_unreachable_finding(
        &report_prod,
        "tests/suite.test.ts"
    ));
    assert!(!has_unreachable_finding(&report_prod, "prodReachable"));
}

#[test]
fn test_script_entry_point_without_exports() {
    let td = TestDir::new("script_entry");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
import { helper } from "./helper";

function run(): void {
    helper();
}

function unusedScriptFn(): void {}
"#,
    );

    td.write_file(
        "src/helper.ts",
        r#"
export function helper(): void {}
export function unusedHelper(): void {}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!has_unreachable_finding(&report, "helper"));
    assert!(has_unreachable_finding(&report, "unusedHelper"));
}

#[test]
fn test_diamond_call_graph() {
    let td = TestDir::new("diamond_call");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
import { branchA, branchB } from "./branches";
export function main(): void {
    const x = 1;
    branchA();
    branchB();
}
"#,
    );

    td.write_file(
        "src/branches.ts",
        r#"
import { leaf } from "./leaf";
export function branchA(): void {
    const a = 1;
    leaf();
}
export function branchB(): void {
    const b = 2;
    leaf();
}
"#,
    );

    td.write_file(
        "src/leaf.ts",
        r#"
export function leaf(): void {}
export function deadLeaf(): void {}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!has_unreachable_finding(&report, "leaf"));
    assert!(!has_unreachable_finding(&report, "branchA"));
    assert!(!has_unreachable_finding(&report, "branchB"));
    assert!(has_unreachable_finding(&report, "deadLeaf"));
}

#[test]
fn test_dead_call_chain_across_multiple_files() {
    let td = TestDir::new("dead_chain_files");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
import "./a";
import "./b";
import "./c";
export function main(): void {
    const m = 1;
}
"#,
    );

    td.write_file(
        "src/a.ts",
        r#"
import { deadB } from "./b";
export function deadA(): void {
    const x = 1;
    deadB();
}
"#,
    );

    td.write_file(
        "src/b.ts",
        r#"
import { deadC } from "./c";
export function deadB(): void {
    const y = 2;
    deadC();
}
"#,
    );

    td.write_file(
        "src/c.ts",
        r#"
export function deadC(): void {
    const z = 3;
}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(has_unreachable_finding(&report, "deadA"));
    assert!(has_unreachable_finding(&report, "deadB"));
    assert!(has_unreachable_finding(&report, "deadC"));
    assert!(!has_unreachable_finding(&report, "main"));
}

#[test]
fn test_self_recursive_unreachable_function() {
    let td = TestDir::new("self_recursive");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
export function main(): void {}
function deadRecursive(): void {
    deadRecursive();
}
"#,
    );

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(has_unreachable_finding(&report, "deadRecursive"));
    assert!(!has_unreachable_finding(&report, "main"));
}

#[test]
fn test_finding_order_determinism() {
    let td = TestDir::new("ordering_test");
    td.init_tsconfig();

    td.write_file(
        "src/z_file.ts",
        r#"
export function zDead1(): void {}
export function zDead2(): void {}
"#,
    );

    td.write_file(
        "src/a_file.ts",
        r#"
export function aDead1(): void {}
export function aDead2(): void {}
"#,
    );

    td.write_file(
        "src/index.ts",
        r#"
import "./a_file";
import "./z_file";
export function main(): void {}
"#,
    );

    let report1 = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();
    let report2 = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(report1, report2);

    let pos_a = report1.find("src/a_file.ts").unwrap();
    let pos_z = report1.find("src/z_file.ts").unwrap();
    assert!(
        pos_a < pos_z,
        "Findings must be sorted alphabetically by file path"
    );

    let pos_a1 = report1.find("subject: aDead1").unwrap();
    let pos_a2 = report1.find("subject: aDead2").unwrap();
    assert!(
        pos_a1 < pos_a2,
        "Findings in same file must be sorted by line / span"
    );
}

#[test]
fn test_large_scale_stress_graph() {
    let td = TestDir::new("large_scale");
    td.init_tsconfig();

    // Generate 100 reachable functions in chain and 100 unreachable functions
    let mut live_content = String::from("export function live0(): void { const a = 1; }\n");
    for i in 1..100 {
        live_content.push_str(&format!(
            "export function live{}(): void {{ const x = {}; live{}(); }}\n",
            i,
            i,
            i - 1
        ));
    }

    let mut dead_content = String::from("export function dead0(): void { const b = 1; }\n");
    for i in 1..100 {
        dead_content.push_str(&format!(
            "export function dead{}(): void {{ const y = {}; dead{}(); }}\n",
            i,
            i,
            i - 1
        ));
    }

    td.write_file("src/live.ts", &live_content);
    td.write_file("src/dead.ts", &dead_content);

    td.write_file(
        "src/index.ts",
        r#"
import { live99 } from "./live";
import "./dead";
export function main(): void {
    const m = 1;
    live99();
}
"#,
    );

    let start = std::time::Instant::now();
    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();
    let elapsed = start.elapsed();
    eprintln!("Large scale analysis elapsed: {:?}", elapsed);

    // All live functions should be reachable
    for i in 0..100 {
        assert!(!has_unreachable_finding(&report, &format!("live{}", i)));
    }

    // All dead functions should be reported unreachable
    for i in 0..100 {
        assert!(has_unreachable_finding(&report, &format!("dead{}", i)));
    }
}
