use std::fs;
use std::path::PathBuf;

use slopgraph::analyze;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "slopgraph_test_tramp_{}_{}_{}",
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

#[test]
fn test_spread_argument_rejected() {
    let t = TestDir::new("spread");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(...args: any[]) {}
export function forwarder(items: any[]) {
    target(...items);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: items"),
        "Spread arguments should count as local read/unpack and reject tramp data: {report}"
    );
}

#[test]
fn test_object_property_shorthand_rejected() {
    let t = TestDir::new("obj_short");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(obj: any) {}
export function forwarder(item: any) {
    const data = { item };
    target(data);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Object shorthand should count as local read and reject tramp data: {report}"
    );
}

#[test]
fn test_object_property_passed_rejected() {
    let t = TestDir::new("obj_prop");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(obj: any) {}
export function forwarder(item: any) {
    target({ value: item });
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Object literal argument should reject tramp data: {report}"
    );
}

#[test]
fn test_array_element_passed_rejected() {
    let t = TestDir::new("array");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(arr: any[]) {}
export function forwarder(item: any) {
    target([item]);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Array literal argument should reject tramp data: {report}"
    );
}

#[test]
fn test_binary_arithmetic_rejected() {
    let t = TestDir::new("arithmetic");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(num: number) {}
export function forwarder(item: number) {
    target(item + 10);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Arithmetic operation should reject tramp data: {report}"
    );
}

#[test]
fn test_logical_operator_rejected() {
    let t = TestDir::new("logical");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any) {}
export function forwarder(item: any) {
    item && target(item);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Logical expression condition should reject tramp data: {report}"
    );
}

#[test]
fn test_ternary_operator_rejected() {
    let t = TestDir::new("ternary");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any) {}
export function fallback() {}
export function forwarder(item: any) {
    item ? target(item) : fallback();
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Ternary conditional read should reject tramp data: {report}"
    );
}

#[test]
fn test_return_statement_without_call_rejected() {
    let t = TestDir::new("return_param");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any) {}
export function forwarder(item: any) {
    target(item);
    return item;
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Direct return of parameter should reject tramp data: {report}"
    );
}

#[test]
fn test_variable_assignment_rejected() {
    let t = TestDir::new("var_assign");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: any) {}
export function forwarder(item: any) {
    const alias = item;
    target(alias);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        !report.contains("TRAMP DATA\nsubject: item"),
        "Variable assignment should reject tramp data: {report}"
    );
}

#[test]
fn test_multiple_forwards_to_different_typed_targets() {
    let t = TestDir::new("multi_target");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function targetA(val: string) {}
export function targetB(val: string) {}
export function broadcast(item: string) {
    targetA(item);
    targetB(item);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        report.contains("TRAMP DATA\nsubject: item"),
        "Clean multi-forward should emit tramp data: {report}"
    );
    assert!(report.contains("passes item\n     ▼\ntargetA"));
    assert!(report.contains("passes item\n     ▼\ntargetB"));
}

#[test]
fn test_multiple_arguments_in_single_call() {
    let t = TestDir::new("multi_args");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function compare(a: string, b: string) {}
export function forwarder(item: string) {
    compare(item, item);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        report.contains("TRAMP DATA\nsubject: item"),
        "Parameter passed twice as arguments should emit tramp data: {report}"
    );
}

#[test]
fn test_nested_complex_type_assertions() {
    let t = TestDir::new("type_assert");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: string) {}
export function forwarder(item: any) {
    target(((((item as unknown) as string)!)));
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        report.contains("TRAMP DATA\nsubject: item"),
        "Deep type assertion forward should emit tramp data: {report}"
    );
}

#[test]
fn test_satisfies_expression_forward() {
    let t = TestDir::new("satisfies");
    t.init_tsconfig();
    t.write_file(
        "src/test.ts",
        r#"
export function target(val: string) {}
export function forwarder(item: string) {
    target(item satisfies string);
}
"#,
    );
    let report = analyze(&t.path).expect("analyze");
    assert!(
        report.contains("TRAMP DATA\nsubject: item"),
        "Satisfies expression forward should emit tramp data: {report}"
    );
}
