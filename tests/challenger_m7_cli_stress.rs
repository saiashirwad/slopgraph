//! Empirical stress-testing suite for CLI execution, options parsing, and error handling.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_slopgraph"))
}

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run_cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(bin_path())
        .args(args)
        .output()
        .expect("Failed to execute slopgraph binary");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

#[test]
fn test_cli_help_flag() {
    let (code, stdout, stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0, "CLI --help must exit with 0");
    assert!(
        stdout.contains("Usage: slopgraph"),
        "stdout should contain usage string"
    );
    assert!(
        stdout.contains("--production"),
        "stdout should document --production"
    );
    assert!(
        stdout.contains("--include-exported"),
        "stdout should document --include-exported"
    );
    assert!(stderr.is_empty(), "stderr should be empty on --help");

    let (code_short, stdout_short, _) = run_cli(&["-h"]);
    assert_eq!(code_short, 0);
    assert_eq!(stdout, stdout_short);
}

#[test]
fn test_cli_directory_input() {
    let dir = fixture_dir("full-report");
    let dir_str = dir.to_str().unwrap();

    let (code, stdout, stderr) = run_cli(&[dir_str]);
    assert_eq!(code, 0, "CLI on valid directory must exit with 0");
    assert!(stderr.is_empty(), "stderr should be empty on success");

    // All 8 shapes must be in the output
    assert!(stdout.contains("\nUNREACHABLE\n"));
    assert!(stdout.contains("\nSINGLE-USE CHAIN\n"));
    assert!(stdout.contains("\nEMPTY WRAPPER\n"));
    assert!(stdout.contains("\nFALSE SHARING\n"));
    assert!(stdout.contains("\nNEAR-DUPLICATE\n"));
    assert!(stdout.contains("\nTRAMP DATA\n"));
    assert!(stdout.contains("\nTYPE CLONE\n"));
    assert!(stdout.contains("\nUNREACHING TEST\n"));
}

#[test]
fn test_cli_tsconfig_file_input() {
    let dir = fixture_dir("full-report");
    let file = dir.join("tsconfig.json");

    let (dir_code, dir_out, _) = run_cli(&[dir.to_str().unwrap()]);
    let (file_code, file_out, file_err) = run_cli(&[file.to_str().unwrap()]);

    assert_eq!(file_code, 0);
    assert!(file_err.is_empty());
    assert_eq!(
        dir_code, file_code,
        "Directory input and tsconfig.json file input should behave identically"
    );
    assert_eq!(
        dir_out, file_out,
        "Report for directory should equal report for tsconfig.json file"
    );
}

#[test]
fn test_cli_production_flag() {
    let dir = fixture_dir("full-report");
    let dir_str = dir.to_str().unwrap();

    let (code, stdout, stderr) = run_cli(&[dir_str, "--production"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());

    // In production mode, test-only reachable functions become UNREACHABLE
    assert!(stdout.contains("UNREACHABLE\nsubject: testOnlyService  (line 5)"));
    assert!(stdout.contains("UNREACHABLE\nsubject: src/unreached_prod.ts  (line 1)"));
}

#[test]
fn test_cli_include_exported_flag() {
    let dir = fixture_dir("full-report");
    let dir_str = dir.to_str().unwrap();

    let (code, stdout, stderr) = run_cli(&[dir_str, "--include-exported"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());

    // With --include-exported, single-use chains incorporate exported steps
    assert!(stdout.contains("SINGLE-USE CHAIN\nsubject: runPipeline"));
    assert!(stdout.contains("exportedStep\n     │\n     ▼\nleafStep"));
}

#[test]
fn test_cli_combined_flags_both_orderings() {
    let dir = fixture_dir("full-report");
    let dir_str = dir.to_str().unwrap();

    let (code1, stdout1, stderr1) = run_cli(&[dir_str, "--production", "--include-exported"]);
    let (code2, stdout2, stderr2) = run_cli(&[dir_str, "--include-exported", "--production"]);

    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert!(stderr1.is_empty());
    assert!(stderr2.is_empty());

    // Both flags active simultaneously
    assert!(stdout1.contains("UNREACHABLE\nsubject: testOnlyService  (line 5)"));
    assert!(stdout1.contains("exportedStep\n     │\n     ▼\nleafStep"));

    assert_eq!(
        stdout1, stdout2,
        "Flag combination order should not affect output"
    );
}

#[test]
fn test_cli_flags_before_and_after_path() {
    let dir = fixture_dir("full-report");
    let dir_str = dir.to_str().unwrap();

    let (code_after, out_after, _) = run_cli(&[dir_str, "--production", "--include-exported"]);
    let (code_before, out_before, _) = run_cli(&["--production", "--include-exported", dir_str]);

    assert_eq!(code_after, 0);
    assert_eq!(code_before, 0);
    assert_eq!(
        out_after, out_before,
        "Flags placed before or after path argument should yield identical results"
    );
}

#[test]
fn test_cli_non_existent_directory_error() {
    let non_existent = fixture_dir("non_existent_directory_12345");
    let (code, stdout, stderr) = run_cli(&[non_existent.to_str().unwrap()]);

    assert_eq!(
        code, 1,
        "Non-existent directory should return failure exit code 1"
    );
    assert!(stdout.is_empty(), "stdout should be empty on error");
    assert!(
        stderr.contains("no tsconfig.json at"),
        "stderr should contain descriptive error"
    );
    assert!(
        !stderr.contains("panicked at"),
        "Error handling should not panic"
    );
}

#[test]
fn test_cli_non_existent_file_error() {
    let non_existent = Path::new("/tmp/non_existent_tsconfig_98765.json");
    let (code, stdout, stderr) = run_cli(&[non_existent.to_str().unwrap()]);

    assert_eq!(
        code, 1,
        "Non-existent file should return failure exit code 1"
    );
    assert!(stdout.is_empty());
    assert!(stderr.contains("no tsconfig.json at"));
    assert!(
        !stderr.contains("panicked at"),
        "Error handling should not panic"
    );
}

#[test]
fn test_cli_directory_without_tsconfig_error() {
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let (code, stdout, stderr) = run_cli(&[src_dir.to_str().unwrap()]);

    assert_eq!(
        code, 1,
        "Directory without tsconfig.json should return exit code 1"
    );
    assert!(stdout.is_empty());
    assert!(stderr.contains("no tsconfig.json at"));
    assert!(
        !stderr.contains("panicked at"),
        "Error handling should not panic"
    );
}

#[test]
fn test_cli_invalid_tsconfig_json_error() {
    let cargo_toml = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let (code, stdout, stderr) = run_cli(&[cargo_toml.to_str().unwrap()]);

    assert_eq!(code, 1, "Invalid tsconfig file should return exit code 1");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("Failed to load tsconfig") || stderr.contains("JSONError"),
        "stderr should describe the JSON parsing failure"
    );
    assert!(
        !stderr.contains("panicked at"),
        "Error handling should not panic"
    );
}

#[test]
fn test_cli_missing_path_argument_error() {
    let (code, stdout, stderr) = run_cli(&[]);

    assert_eq!(code, 2, "Missing required argument should return exit code 2 (clap error)");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("the following required arguments were not provided")
            && stderr.contains("<PATH>"),
        "stderr should inform about missing PATH argument"
    );
    assert!(
        !stderr.contains("panicked at"),
        "Clap parsing error should not panic"
    );
}

#[test]
fn test_cli_unknown_flag_error() {
    let dir = fixture_dir("full-report");
    let (code, stdout, stderr) = run_cli(&[dir.to_str().unwrap(), "--unrecognized-custom-flag"]);

    assert_eq!(code, 2, "Unknown CLI flag should return exit code 2");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("unexpected argument '--unrecognized-custom-flag' found"),
        "stderr should inform about unexpected flag"
    );
    assert!(
        !stderr.contains("panicked at"),
        "Clap parsing error should not panic"
    );
}

#[test]
fn test_cli_relative_and_absolute_paths() {
    let rel_path = "tests/fixtures/full-report";
    let abs_path = fixture_dir("full-report");

    let (rel_code, rel_out, rel_err) = run_cli(&[rel_path]);
    let (abs_code, abs_out, abs_err) = run_cli(&[abs_path.to_str().unwrap()]);

    assert_eq!(rel_code, 0);
    assert_eq!(abs_code, 0);
    assert!(rel_err.is_empty());
    assert!(abs_err.is_empty());
    assert_eq!(
        rel_out, abs_out,
        "Relative path and absolute path should produce identical report"
    );
}
