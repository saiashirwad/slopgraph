use std::fs;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn test_unreaching_test_detector_end_to_end_on_fixture() {
    let dir = fixture("unreaching-test");
    let report = slopgraph::analyze(&dir).expect("slopgraph::analyze should succeed");

    // Must contain UNREACHING TEST cards
    assert!(report.contains("UNREACHING TEST"));

    // Verify tests/unreached.test.ts reports src/unreached.ts
    assert!(
        report.contains("tests/unreached.test.ts\n\nUNREACHING TEST\nsubject: src/unreached.ts  (line 1)\nTest 'tests/unreached.test.ts' imports 'src/unreached.ts' but makes zero typed calls to it.\ntests/unreached.test.ts\n     │\n     ▼\nsrc/unreached.ts  ←── finding")
    );

    // Verify tests/multi.test.ts reports src/multi_unreached.ts
    assert!(
        report.contains("tests/multi.test.ts\n\nUNREACHING TEST\nsubject: src/multi_unreached.ts  (line 2)\nTest 'tests/multi.test.ts' imports 'src/multi_unreached.ts' but makes zero typed calls to it.\ntests/multi.test.ts\n     │\n     ▼\nsrc/multi_unreached.ts  ←── finding")
    );
}


#[test]
fn test_unreaching_test_detector_does_not_flag_reached_modules() {
    let dir = fixture("unreaching-test");
    let report = slopgraph::analyze(&dir).expect("slopgraph::analyze should succeed");

    // tests/reached.test.ts calls calculateTotal in src/reached.ts
    assert!(!report.contains("tests/reached.test.ts\n\nUNREACHING TEST"));
    assert!(!report.contains("subject: src/reached.ts"));

    // tests/multi.test.ts calls formatGreeting in src/multi_reached.ts
    assert!(!report.contains("subject: src/multi_reached.ts"));
}

#[test]
fn test_unreaching_test_detector_ignores_test_helpers() {
    let dir = fixture("unreaching-test");
    let report = slopgraph::analyze(&dir).expect("slopgraph::analyze should succeed");

    // tests/helper.ts is in test_files, so importing it never triggers unreaching test
    assert!(!report.contains("subject: tests/helper.ts"));
}

#[test]
fn test_unreaching_test_detector_ignores_production_to_production_imports() {
    let dir = fixture("unreaching-test");
    let report = slopgraph::analyze(&dir).expect("slopgraph::analyze should succeed");

    // src/service.ts imports src/repo.ts
    assert!(!report.contains("src/service.ts\n\nUNREACHING TEST"));
    assert!(!report.contains("subject: src/repo.ts"));
}

#[test]
fn test_unreaching_test_with_production_flag_still_evaluates_tests() {
    let dir = fixture("unreaching-test");
    let options = slopgraph::Options {
        production: true,
        ..Default::default()
    };
    let report = slopgraph::analyze_with_options(&dir, options).expect("analyze_with_options should succeed");

    // Unreaching test detector analyzes test files even if --production is passed
    assert!(report.contains("UNREACHING TEST\nsubject: src/unreached.ts"));
    assert!(report.contains("UNREACHING TEST\nsubject: src/multi_unreached.ts"));
}

#[test]
fn test_fixture_files_all_exist() {
    let dir = fixture("unreaching-test");
    assert!(dir.join("tsconfig.json").exists());
    assert!(dir.join("src/index.ts").exists());
    assert!(dir.join("src/service.ts").exists());
    assert!(dir.join("src/repo.ts").exists());
    assert!(dir.join("src/reached.ts").exists());
    assert!(dir.join("src/unreached.ts").exists());
    assert!(dir.join("src/multi_reached.ts").exists());
    assert!(dir.join("src/multi_unreached.ts").exists());
    assert!(dir.join("tests/helper.ts").exists());
    assert!(dir.join("tests/reached.test.ts").exists());
    assert!(dir.join("tests/unreached.test.ts").exists());
    assert!(dir.join("tests/multi.test.ts").exists());
    assert!(dir.join("report.golden.txt").exists());

    let golden_content = fs::read_to_string(dir.join("report.golden.txt")).unwrap();
    assert!(golden_content.contains("UNREACHING TEST"));
}
