//! Adversarial stress testing for Milestone M5: Type Clone Detector.

use slopgraph::analyze;
use std::fs;
use std::path::PathBuf;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("slopgraph_adv_tc_{}_{}", name, std::process::id()));
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

/// Parse TYPE CLONE pairs from rendered report.
fn find_type_clone_pairs(report: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let lines: Vec<&str> = report.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "TYPE CLONE" {
            let mut subject = String::new();
            let mut target = String::new();
            if i + 1 < lines.len() && lines[i + 1].starts_with("subject: ") {
                let rest = lines[i + 1].strip_prefix("subject: ").unwrap();
                subject = rest.split("  (").next().unwrap_or(rest).trim().to_string();
            }
            if i + 5 < lines.len() {
                target = lines[i + 5].trim().to_string();
            }
            if !subject.is_empty() && !target.is_empty() {
                pairs.push((subject, target));
            }
        }
        i += 1;
    }
    pairs
}

#[test]
fn test_multitype_three_way_clones_across_files() {
    let dir = TestDir::new("threeway");
    dir.init_tsconfig();

    dir.write_file(
        "src/a.ts",
        r#"
export interface TypeAlpha {
    id: string;
    score: number;
    enabled: boolean;
}
"#,
    );

    dir.write_file(
        "src/b.ts",
        r#"
export interface TypeBeta {
    score: number;
    id: string;
    enabled: boolean;
}
"#,
    );

    dir.write_file(
        "src/c.ts",
        r#"
export type TypeGamma = {
    enabled: boolean;
    score: number;
    id: string;
};
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);

    // Alpha vs Beta, Alpha vs Gamma, Beta vs Gamma = 3 pairs
    assert_eq!(pairs.len(), 3);
    assert!(pairs.contains(&("TypeAlpha".to_string(), "TypeBeta".to_string())));
    assert!(pairs.contains(&("TypeAlpha".to_string(), "TypeGamma".to_string())));
    assert!(pairs.contains(&("TypeBeta".to_string(), "TypeGamma".to_string())));
}

#[test]
fn test_complex_types_generics_tuples_and_nested() {
    let dir = TestDir::new("complex_types");
    dir.init_tsconfig();

    dir.write_file(
        "src/index.ts",
        r#"
export interface ComplexA<T> {
    data: T[];
    meta: { count: number; tag: string };
    tuple: [string, number, boolean];
    union: boolean | string | number;
}

export type ComplexB<T> = {
    union: number | boolean | string;
    tuple: [string, number, boolean];
    meta: { tag: string; count: number };
    data: T[];
};
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("ComplexA".to_string(), "ComplexB".to_string()));
}

#[test]
fn test_whitespace_and_comments_insensitivity() {
    let dir = TestDir::new("comments_ws");
    dir.init_tsconfig();

    dir.write_file(
        "src/types.ts",
        r#"
// Style 1: compact, single-line comments
export interface FirstRecord {
    /* unique key */
    id: string;
    name: string; // user name
    active: boolean;
}

// Style 2: verbose, multiline comments, weird indentation
export type SecondRecord = {


    name: string;


    id: string;

    /**
     * active flag
     */
    active: boolean;
};
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("FirstRecord".to_string(), "SecondRecord".to_string()));
}

#[test]
fn test_boundary_conditions_2_vs_3_fields() {
    let dir = TestDir::new("boundary");
    dir.init_tsconfig();

    dir.write_file(
        "src/boundary.ts",
        r#"
export interface TwoFieldA {
    first: string;
    second: number;
}
export interface TwoFieldB {
    first: string;
    second: number;
}

export interface ThreeFieldA {
    first: string;
    second: number;
    third: boolean;
}
export interface ThreeFieldB {
    first: string;
    second: number;
    third: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("ThreeFieldA".to_string(), "ThreeFieldB".to_string()));
}

#[test]
fn test_heritage_and_intersections_rejected() {
    let dir = TestDir::new("heritage");
    dir.init_tsconfig();

    dir.write_file(
        "src/heritage.ts",
        r#"
export interface BaseEntity {
    id: string;
    created: number;
    updated: number;
}

export interface DerivedA extends BaseEntity {
    id: string;
    created: number;
    updated: number;
}

export interface DerivedB extends BaseEntity, ExtraBase {
    id: string;
    created: number;
    updated: number;
}

export interface ExtraBase {}

export type InterType = BaseEntity & {
    id: string;
    created: number;
    updated: number;
};
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);
    // None should be reported as clones because DerivedA, DerivedB have extends and InterType is intersection
    assert_eq!(pairs.len(), 0);
}

#[test]
fn test_optional_and_readonly_flags() {
    let dir = TestDir::new("flags");
    dir.init_tsconfig();

    dir.write_file(
        "src/flags.ts",
        r#"
export interface Plain {
    a: string;
    b: number;
    c: boolean;
}

export interface WithOpt {
    a?: string;
    b: number;
    c: boolean;
}

export interface WithRo {
    readonly a: string;
    b: number;
    c: boolean;
}

export interface PlainClone {
    a: string;
    b: number;
    c: boolean;
}

export interface WithOptClone {
    a?: string;
    b: number;
    c: boolean;
}

export interface WithRoClone {
    readonly a: string;
    b: number;
    c: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);
    assert_eq!(pairs.len(), 3);
    assert!(pairs.contains(&("Plain".to_string(), "PlainClone".to_string())));
    assert!(pairs.contains(&("WithOpt".to_string(), "WithOptClone".to_string())));
    assert!(pairs.contains(&("WithRo".to_string(), "WithRoClone".to_string())));
}

#[test]
fn test_interfaces_extending_empty_or_single_field_interfaces() {
    let dir = TestDir::new("heritage_empty_single");
    dir.init_tsconfig();

    dir.write_file(
        "src/heritage_empty.ts",
        r#"
export interface EmptyBase {}

export interface ChildA extends EmptyBase {
    x: string;
    y: number;
    z: boolean;
}

export interface ChildB extends EmptyBase {
    x: string;
    y: number;
    z: boolean;
}

export interface SingleFieldBase {
    tag: string;
}

export interface ChildC extends SingleFieldBase {
    x: string;
    y: number;
    z: boolean;
}

export interface StandaloneClone {
    x: string;
    y: number;
    z: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);
    // ChildA, ChildB, ChildC all have extends clauses, so they must be excluded.
    // StandaloneClone has no other un-extended 3-field clone.
    assert_eq!(pairs.len(), 0);
}

#[test]
fn test_multiple_interfaces_in_inheritance_chains() {
    let dir = TestDir::new("inheritance_chains");
    dir.init_tsconfig();

    dir.write_file(
        "src/chain.ts",
        r#"
export interface RootBase {
    alpha: string;
    beta: number;
    gamma: boolean;
}

export interface DerivedAlpha extends RootBase {
    delta: string;
    epsilon: number;
    zeta: boolean;
}

export interface DerivedBeta extends RootBase {
    delta: string;
    epsilon: number;
    zeta: boolean;
}

export interface RootBaseClone {
    alpha: string;
    beta: number;
    gamma: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);

    // RootBase matches RootBaseClone.
    // DerivedAlpha and DerivedBeta are excluded by their extends clause.
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("RootBase".to_string(), "RootBaseClone".to_string()));
}

#[test]
fn test_cross_file_same_name_and_different_names() {
    let dir = TestDir::new("cross_file_names");
    dir.init_tsconfig();

    dir.write_file(
        "src/service1.ts",
        r#"
export interface Config {
    host: string;
    port: number;
    secure: boolean;
}
"#,
    );

    dir.write_file(
        "src/service2.ts",
        r#"
export interface Config {
    host: string;
    port: number;
    secure: boolean;
}
"#,
    );

    dir.write_file(
        "src/service3.ts",
        r#"
export interface ServerSettings {
    host: string;
    port: number;
    secure: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);

    // All 3 pairs should be detected across distinct files:
    // (Config@service1, Config@service2)
    // (Config@service1, ServerSettings@service3)
    // (Config@service2, ServerSettings@service3)
    assert_eq!(pairs.len(), 3);
    assert!(pairs.contains(&("Config".to_string(), "Config".to_string())));
    assert!(pairs.contains(&("Config".to_string(), "ServerSettings".to_string())));
}

#[test]
fn test_same_file_duplicate_name_declaration_merging() {
    let dir = TestDir::new("decl_merging");
    dir.init_tsconfig();

    dir.write_file(
        "src/merged.ts",
        r#"
export interface MergedType {
    propA: string;
    propB: number;
    propC: boolean;
}

export interface MergedType {
    propA: string;
    propB: number;
    propC: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);
    // Identical name within the same file is not reported as a clone against itself.
    assert_eq!(pairs.len(), 0);
}

#[test]
fn test_readonly_and_optional_modifier_matrix() {
    let dir = TestDir::new("modifier_matrix");
    dir.init_tsconfig();

    dir.write_file(
        "src/matrix.ts",
        r#"
export interface TargetType {
    readonly a: string;
    b?: number;
    c: boolean;
}

export interface ExactMatch {
    c: boolean;
    readonly a: string;
    b?: number;
}

export interface ReadonlyMismatchA {
    a: string;
    b?: number;
    c: boolean;
}

export interface OptionalMismatchB {
    readonly a: string;
    b: number;
    c: boolean;
}

export interface ExtraOptionalA {
    readonly a?: string;
    b?: number;
    c: boolean;
}

export interface ExtraReadonlyC {
    readonly a: string;
    b?: number;
    readonly c: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);

    // Only TargetType and ExactMatch should match.
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("TargetType".to_string(), "ExactMatch".to_string()));
}

#[test]
fn test_method_signatures_and_string_literals_and_generics() {
    let dir = TestDir::new("methods_literals_generics");
    dir.init_tsconfig();

    dir.write_file(
        "src/methods.ts",
        r#"
export interface ServiceMethodForm {
    init(s: string): void;
    ping(): boolean;
    close(code: number): void;
}

export type ServicePropertyForm = {
    init: (s: string) => void;
    ping: () => boolean;
    close: (code: number) => void;
};

export interface ServiceQuotedForm {
    "init"(s: string): void;
    "ping"(): boolean;
    "close"(code: number): void;
}

export interface GenericA<T> {
    item: T;
    id: string;
    valid: boolean;
}

export interface GenericB<T> {
    valid: boolean;
    id: string;
    item: T;
}

export interface GenericDiffParam<U> {
    item: U;
    id: string;
    valid: boolean;
}
"#,
    );

    let report = analyze(&dir.path).unwrap();
    let pairs = find_type_clone_pairs(&report);

    // ServiceMethodForm, ServicePropertyForm, ServiceQuotedForm all pairwise match -> 3 pairs
    // GenericA and GenericB match -> 1 pair
    // GenericDiffParam uses "U" rather than "T", so no match.
    assert_eq!(pairs.len(), 4);
    assert!(pairs.contains(&("ServiceMethodForm".to_string(), "ServicePropertyForm".to_string())));
    assert!(pairs.contains(&("ServiceMethodForm".to_string(), "ServiceQuotedForm".to_string())));
    assert!(pairs.contains(&("ServicePropertyForm".to_string(), "ServiceQuotedForm".to_string())));
    assert!(pairs.contains(&("GenericA".to_string(), "GenericB".to_string())));
}

