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
fn accepts_tsconfig_file_or_directory() {
    let dir = fixture("false-sharing");
    let from_dir = slopgraph::analyze(&dir).unwrap();
    let from_file = slopgraph::analyze(dir.join("tsconfig.json")).unwrap();
    assert_eq!(from_dir, from_file);
}
