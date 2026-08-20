//! Golden-report fixture tests.
//!
//! Later detectors copy this pattern:
//! 1. Add a program under `tests/fixtures/<name>/` with a `tsconfig.json`.
//! 2. Put the expected report in `tests/fixtures/<name>/report.golden.txt`.
//! 3. Call `assert_golden("<name>")`.
//!
//! Refresh a golden with `UPDATE_GOLDEN=1 cargo test`.

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn assert_golden(name: &str) {
    let dir = fixture(name);
    let report = slopgraph::analyze(&dir).unwrap_or_else(|e| panic!("analyze {name}: {e}"));
    let golden_path = dir.join("report.golden.txt");
    maybe_update(&golden_path, &report);
    let expected = fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", golden_path.display()));
    assert_eq!(
        report,
        expected,
        "report for fixture `{name}` did not match {}",
        golden_path.display()
    );
}

fn maybe_update(path: &Path, report: &str) {
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::write(path, report).unwrap();
    }
}

#[test]
fn false_sharing_same_directory_and_one_group() {
    assert_golden("false-sharing");
}

#[test]
fn js_files_never_produce_findings() {
    assert_golden("js-ignored");
}

#[test]
fn paths_extends_and_base_url_resolve() {
    assert_golden("paths-extends");
}

#[test]
fn project_references_are_ignored() {
    assert_golden("project-references");
}

#[test]
fn empty_wrapper_three_forward_forms() {
    assert_golden("empty-wrapper");
}

#[test]
fn unreachable_files_and_entry_points() {
    assert_golden("unreachable");
}

#[test]
fn unreachable_files_production_flag_drops_test_roots() {
    let dir = fixture("unreachable");
    let options = slopgraph::Options {
        production: true,
        ..Default::default()
    };
    let report = slopgraph::analyze_with_options(&dir, options).unwrap();
    let expected = "\
src/index.ts

UNREACHABLE
subject: unusedIndexFn  (line 3)
unusedIndexFn  ←── finding

src/orphan.ts

UNREACHABLE
subject: src/orphan.ts  (line 1)
src/orphan.ts  ←── finding

src/test_only.ts

UNREACHABLE
subject: src/test_only.ts  (line 1)
src/test_only.ts  ←── finding

src/used.ts

UNREACHABLE
subject: deadHelper  (line 5)
deadHelper  ←── finding

UNREACHABLE
subject: deadInternal  (line 9)
deadInternal  ←── finding

EMPTY WRAPPER
subject: deadChainA  (line 11)
deadChainA  ←── finding
     │
     ▼
deadChainB

UNREACHABLE
subject: deadChainA  (line 11)
deadChainA  ←── finding

UNREACHABLE
subject: deadChainB  (line 15)
deadChainB  ←── finding

FALSE SHARING
subject: testOnlyHelper  (line 17)
tests/a.test.ts
     │  one consumer group
     ▼
testOnlyHelper  ←── finding

UNREACHABLE
subject: testOnlyHelper  (line 17)
testOnlyHelper  ←── finding
";
    assert_eq!(report, expected);
}

#[test]
fn test_files_remain_in_graph_under_production() {
    let dir = fixture("unreachable");
    let program = slopgraph::load(&dir).unwrap();
    assert!(program.files.iter().any(|f| f.ends_with("tests/a.test.ts")));
    let modules = slopgraph::parse_program(&program).unwrap();
    let graph = slopgraph::ModuleGraph::build(&program, modules).unwrap();
    assert!(graph
        .test_files
        .iter()
        .any(|f| f.ends_with("tests/a.test.ts")));
    assert!(graph.modules.keys().any(|f| f.ends_with("tests/a.test.ts")));
}

#[test]
fn accepts_tsconfig_file_or_directory() {
    let dir = fixture("false-sharing");
    let from_dir = slopgraph::analyze(&dir).unwrap();
    let from_file = slopgraph::analyze(dir.join("tsconfig.json")).unwrap();
    assert_eq!(from_dir, from_file);
}

#[test]
fn single_use_chain_default_options() {
    assert_golden("single-use-chain");
}

#[test]
fn single_use_chain_include_exported_flag() {
    let dir = fixture("single-use-chain");
    let options = slopgraph::Options {
        include_exported: true,
        ..Default::default()
    };
    let report = slopgraph::analyze_with_options(&dir, options).unwrap();
    assert!(report.contains("subject: exportedMiddle"));
}

#[test]
fn single_use_chain_test_callers_prevent_chain() {
    let dir = fixture("single-use-chain");
    let report = slopgraph::analyze(&dir).unwrap();
    // helperWithTest is called by productionCaller and testHelper (in-degree 2), so not in single-use chain
    assert!(!report.contains("SINGLE-USE CHAIN\nsubject: helperWithTest"));
}

#[test]
fn near_duplicate_functions() {
    assert_golden("near-duplicate");
}
