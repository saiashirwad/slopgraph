use slopgraph::{analyze_with_options, Options};
use std::fs;
use std::path::PathBuf;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("slopgraph_stress_{}_{}", name, std::process::id()));
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

fn has_unreachable_finding(report: &str, file: &str, subject: &str) -> bool {
    let mut current_file = "";
    for section in report.split("\n\n") {
        let lines: Vec<&str> = section.lines().collect();
        if lines.is_empty() {
            continue;
        }
        if lines.len() == 1 && (lines[0].ends_with(".ts") || lines[0].ends_with(".js")) {
            current_file = lines[0].trim();
            continue;
        }
        if lines[0].trim() == "UNREACHABLE" && lines.len() >= 2 {
            let subj_line = lines[1].trim();
            if let Some(rest) = subj_line.strip_prefix("subject: ") {
                let actual_subject = rest.split("  (").next().unwrap_or(rest).trim();
                if actual_subject == subject && (file.is_empty() || current_file == file) {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
fn test_unreachable_stress_fixture_default_and_production() {
    let fixture_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unreachable_stress");

    // 1. Analyze with default options (test roots included)
    let report_default = slopgraph::analyze(&fixture_dir).unwrap();

    // Check assertions for default mode:
    // - privateDeadInEntry is reported as unreachable
    assert!(
        has_unreachable_finding(&report_default, "src/entry.ts", "privateDeadInEntry"),
        "expected privateDeadInEntry unreachable, got:\n{report_default}"
    );

    // - deadCycle1, deadCycle2, deadCycle3, selfRecursiveDead are unreachable
    assert!(has_unreachable_finding(
        &report_default,
        "src/dead_cycles.ts",
        "deadCycle1"
    ));
    assert!(has_unreachable_finding(
        &report_default,
        "src/dead_cycles.ts",
        "deadCycle2"
    ));
    assert!(has_unreachable_finding(
        &report_default,
        "src/dead_cycles.ts",
        "deadCycle3"
    ));
    assert!(has_unreachable_finding(
        &report_default,
        "src/dead_cycles.ts",
        "selfRecursiveDead"
    ));

    // - deadBranch1, deadBranch2 are unreachable
    assert!(has_unreachable_finding(
        &report_default,
        "src/diamonds.ts",
        "deadBranch1"
    ));
    assert!(has_unreachable_finding(
        &report_default,
        "src/diamonds.ts",
        "deadBranch2"
    ));

    // - completelyDeadInTestReachableMod is unreachable
    assert!(has_unreachable_finding(
        &report_default,
        "src/test_reachability.ts",
        "completelyDeadInTestReachableMod"
    ));

    // - orphan_isolated.ts is reported as unreachable file
    assert!(has_unreachable_finding(
        &report_default,
        "src/orphan_isolated.ts",
        "src/orphan_isolated.ts"
    ));

    // - orphanFn1 and orphanFn2 MUST NOT be reported as unreachable functions (suppressed)
    assert!(
        !has_unreachable_finding(&report_default, "src/orphan_isolated.ts", "orphanFn1"),
        "orphanFn1 in unreachable file must be suppressed"
    );
    assert!(
        !has_unreachable_finding(&report_default, "src/orphan_isolated.ts", "orphanFn2"),
        "orphanFn2 in unreachable file must be suppressed"
    );

    // - reachable functions must NOT be reported as unreachable
    assert!(!has_unreachable_finding(
        &report_default,
        "src/entry.ts",
        "entryFn"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/reachable_cycles.ts",
        "reachableA"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/reachable_cycles.ts",
        "cycleNode1"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/reachable_cycles.ts",
        "cycleNode2"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/diamonds.ts",
        "reachableDiamondA"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/diamonds.ts",
        "reachableDiamondB"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/diamonds.ts",
        "diamondJoin"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/diamonds.ts",
        "deepChain1"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/diamonds.ts",
        "deepChain2"
    ));

    // - functions called from test roots must NOT be reported as unreachable in default mode
    assert!(!has_unreachable_finding(
        &report_default,
        "src/test_reachability.ts",
        "testReachableHelper"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/test_reachability.ts",
        "testReachableChainA"
    ));
    assert!(!has_unreachable_finding(
        &report_default,
        "src/test_reachability.ts",
        "testReachableChainB"
    ));

    // - test functions must NEVER be reported as unreachable
    assert!(!has_unreachable_finding(&report_default, "", "testMain"));
    assert!(!has_unreachable_finding(&report_default, "", "deadTestFn"));

    // 2. Analyze with --production (test roots dropped)
    let options_prod = slopgraph::Options {
        production: true,
        ..Default::default()
    };
    let report_prod = slopgraph::analyze_with_options(&fixture_dir, options_prod).unwrap();

    // In production mode:
    // - testReachableHelper, testReachableChainA, testReachableChainB MUST now be UNREACHABLE!
    assert!(
        has_unreachable_finding(
            &report_prod,
            "src/test_reachability.ts",
            "testReachableHelper"
        ),
        "expected testReachableHelper unreachable under production mode"
    );
    assert!(
        has_unreachable_finding(
            &report_prod,
            "src/test_reachability.ts",
            "testReachableChainA"
        ),
        "expected testReachableChainA unreachable under production mode"
    );
    assert!(
        has_unreachable_finding(
            &report_prod,
            "src/test_reachability.ts",
            "testReachableChainB"
        ),
        "expected testReachableChainB unreachable under production mode"
    );

    // - test functions must still NOT be reported
    assert!(!has_unreachable_finding(&report_prod, "", "testMain"));
    assert!(!has_unreachable_finding(&report_prod, "", "deadTestFn"));
}

#[test]
fn test_cross_file_mutual_recursion_and_unexported_cycles() {
    let td = TestDir::new("cross_mutual");
    td.init_tsconfig();

    td.write_file(
        "src/index.ts",
        r#"
import "./mod_a";
import "./mod_b";

export function main(): void {
    const x = 1;
}

function deadPrivateCycle1(): void {
    deadPrivateCycle2();
}

function deadPrivateCycle2(): void {
    deadPrivateCycle1();
}
"#,
    );

    td.write_file(
        "src/mod_a.ts",
        r#"
import { crossDeadB } from "./mod_b";

export function crossDeadA(): void {
    crossDeadB();
}
"#,
    );

    td.write_file(
        "src/mod_b.ts",
        r#"
import { crossDeadA } from "./mod_a";

export function crossDeadB(): void {
    crossDeadA();
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

    assert!(has_unreachable_finding(
        &report,
        "src/index.ts",
        "deadPrivateCycle1"
    ));
    assert!(has_unreachable_finding(
        &report,
        "src/index.ts",
        "deadPrivateCycle2"
    ));
    assert!(has_unreachable_finding(
        &report,
        "src/mod_a.ts",
        "crossDeadA"
    ));
    assert!(has_unreachable_finding(
        &report,
        "src/mod_b.ts",
        "crossDeadB"
    ));
    assert!(!has_unreachable_finding(&report, "src/index.ts", "main"));
}

#[test]
fn test_deep_linear_call_chain_scaling() {
    let td = TestDir::new("deep_chain");
    td.init_tsconfig();

    let index_src = String::from(
        "import { live0 } from './chain';\nexport function main(): void {\n  live0();\n}\n",
    );
    let mut chain_src = String::new();

    // 25 live chained functions
    for i in 0..25 {
        if i == 24 {
            chain_src.push_str(&format!("export function live{i}(): void {{}}\n"));
        } else {
            chain_src.push_str(&format!(
                "export function live{i}(): void {{\n  live{}();\n}}\n",
                i + 1
            ));
        }
    }

    // 25 dead chained functions
    for i in 0..25 {
        if i == 24 {
            chain_src.push_str(&format!("export function dead{i}(): void {{}}\n"));
        } else {
            chain_src.push_str(&format!(
                "export function dead{i}(): void {{\n  dead{}();\n}}\n",
                i + 1
            ));
        }
    }

    td.write_file("src/index.ts", &index_src);
    td.write_file("src/chain.ts", &chain_src);

    let report = analyze_with_options(
        &td.path,
        Options {
            production: false,
            ..Default::default()
        },
    )
    .unwrap();

    // All live functions should NOT be reported
    for i in 0..25 {
        assert!(!has_unreachable_finding(
            &report,
            "src/chain.ts",
            &format!("live{i}")
        ));
    }

    // All dead functions MUST be reported
    for i in 0..25 {
        assert!(has_unreachable_finding(
            &report,
            "src/chain.ts",
            &format!("dead{i}")
        ));
    }
}
