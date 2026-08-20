//! Empirical Challenger Test Suite for Milestone M5: Type Clone Detector (Issue #20).

use slopgraph::analyze;
use std::fs;
use std::path::PathBuf;

struct TestWorkspace {
    path: PathBuf,
}

impl TestWorkspace {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("slopgraph_chal_tc_{}_{}", name, std::process::id()));
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

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn parse_type_clone_pairs(report: &str) -> Vec<(String, String)> {
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
            let mut j = i + 2;
            while j < lines.len() && !lines[j].trim().is_empty() {
                if lines[j].trim() == "▼" && j + 1 < lines.len() {
                    target = lines[j + 1].trim().to_string();
                    break;
                }
                j += 1;
            }
            if !subject.is_empty() && !target.is_empty() {
                pairs.push((subject, target));
            }
        }
        i += 1;
    }
    pairs
}


// -----------------------------------------------------------------------------
// Category 1: Boundary Field Counts (0, 1, 2, 3, 4, and Mismatched Counts)
// -----------------------------------------------------------------------------

#[test]
fn test_field_count_boundaries_0_1_2_3_4() {
    let ws = TestWorkspace::new("field_count_boundaries");
    ws.init_tsconfig();

    ws.write_file(
        "src/types.ts",
        r#"
// 0 fields: Empty interface
export interface EmptyA {}
export interface EmptyB {}

// 1 field: Single property
export interface SingleA {
    id: string;
}
export interface SingleB {
    id: string;
}

// 2 fields: Two properties (must NOT be reported)
export interface TwoFieldsA {
    id: string;
    name: string;
}
export interface TwoFieldsB {
    id: string;
    name: string;
}

// 3 fields: Exact boundary threshold (MUST be reported)
export interface ThreeFieldsA {
    id: string;
    name: string;
    age: number;
}
export interface ThreeFieldsB {
    id: string;
    name: string;
    age: number;
}

// 4 fields: Above threshold (MUST be reported)
export interface FourFieldsA {
    id: string;
    name: string;
    age: number;
    active: boolean;
}
export interface FourFieldsB {
    id: string;
    name: string;
    age: number;
    active: boolean;
}

// 3 vs 4 fields mismatch: Must NOT clone together
export interface ThreeFieldsC {
    id: string;
    name: string;
    age: number;
}
export interface FourFieldsC {
    id: string;
    name: string;
    age: number;
    extra: string;
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // Only ThreeFieldsA <-> ThreeFieldsB, and FourFieldsA <-> FourFieldsB,
    // plus ThreeFieldsA <-> ThreeFieldsC and ThreeFieldsB <-> ThreeFieldsC (a 3-way clone of 3-field types)
    assert!(pairs.contains(&("ThreeFieldsA".to_string(), "ThreeFieldsB".to_string())));
    assert!(pairs.contains(&("ThreeFieldsA".to_string(), "ThreeFieldsC".to_string())));
    assert!(pairs.contains(&("ThreeFieldsB".to_string(), "ThreeFieldsC".to_string())));
    assert!(pairs.contains(&("FourFieldsA".to_string(), "FourFieldsB".to_string())));

    // Verify 0, 1, 2 field pairs and 3-vs-4 are NOT reported
    for (a, b) in &pairs {
        assert!(!a.contains("Empty") && !b.contains("Empty"));
        assert!(!a.contains("Single") && !b.contains("Single"));
        assert!(!a.contains("TwoFields") && !b.contains("TwoFields"));
        assert!(!(a.contains("ThreeFields") && b.contains("FourFields")));
    }
}

// -----------------------------------------------------------------------------
// Category 2: Union and Intersection Element Orderings and Deduplication
// -----------------------------------------------------------------------------

#[test]
fn test_union_element_orderings_and_deduplication() {
    let ws = TestWorkspace::new("union_orderings");
    ws.init_tsconfig();

    ws.write_file(
        "src/unions.ts",
        r#"
// Permutation of 3 types in union
export interface UnionAlpha {
    id: string;
    tag: string | number | boolean;
    flag: boolean;
}

export type UnionBeta = {
    flag: boolean;
    tag: boolean | string | number;
    id: string;
};

export interface UnionGamma {
    tag: number | boolean | string;
    id: string;
    flag: boolean;
}

// Nested parenthesized union with duplicate members
export interface NestedUnionA {
    id: string;
    value: string | (number | boolean);
    status: "active" | "inactive" | "pending";
}

export type NestedUnionB = {
    id: string;
    value: (boolean | string) | (number | string); // includes duplicate string
    status: "pending" | "inactive" | "active";
};
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // Alpha <-> Beta, Alpha <-> Gamma, Beta <-> Gamma
    assert!(pairs.contains(&("UnionAlpha".to_string(), "UnionBeta".to_string())));
    assert!(pairs.contains(&("UnionAlpha".to_string(), "UnionGamma".to_string())));
    assert!(pairs.contains(&("UnionBeta".to_string(), "UnionGamma".to_string())));

    // NestedUnionA <-> NestedUnionB
    assert!(pairs.contains(&("NestedUnionA".to_string(), "NestedUnionB".to_string())));
}

#[test]
fn test_intersection_element_orderings() {
    let ws = TestWorkspace::new("intersection_orderings");
    ws.init_tsconfig();

    ws.write_file(
        "src/intersections.ts",
        r#"
export interface TraitA { a: string; }
export interface TraitB { b: number; }
export interface TraitC { c: boolean; }

export interface CombinedA {
    id: string;
    composite: TraitA & TraitB & TraitC;
    count: number;
}

export type CombinedB = {
    count: number;
    composite: TraitC & TraitA & TraitB;
    id: string;
};
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("CombinedA".to_string(), "CombinedB".to_string()));
}

// -----------------------------------------------------------------------------
// Category 3: Deeply Nested Object Shapes in Field Annotations
// -----------------------------------------------------------------------------

#[test]
fn test_deeply_nested_object_shapes() {
    let ws = TestWorkspace::new("nested_shapes");
    ws.init_tsconfig();

    ws.write_file(
        "src/nested.ts",
        r#"
export interface DeepA {
    id: string;
    payload: {
        header: { version: number; encoding: string };
        body: { data: string; timestamp: number };
    };
    created: number;
}

export type DeepB = {
    created: number;
    payload: {
        body: { timestamp: number; data: string };
        header: { encoding: string; version: number };
    };
    id: string;
};

// Nested array of objects
export interface ArrayOfObjectsA {
    id: string;
    items: { sku: string; qty: number; price: number }[];
    total: number;
}

export type ArrayOfObjectsB = {
    total: number;
    items: { price: number; sku: string; qty: number }[];
    id: string;
};
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    assert_eq!(pairs.len(), 2);
    assert!(pairs.contains(&("DeepA".to_string(), "DeepB".to_string())));
    assert!(pairs.contains(&("ArrayOfObjectsA".to_string(), "ArrayOfObjectsB".to_string())));
}

// -----------------------------------------------------------------------------
// Category 4: Method Signatures vs Property Signatures with Function Types
// -----------------------------------------------------------------------------

#[test]
fn test_method_signatures_vs_property_function_types() {
    let ws = TestWorkspace::new("method_vs_property_fn");
    ws.init_tsconfig();

    ws.write_file(
        "src/service.ts",
        r#"
// Interface using method signature syntax
export interface ServiceInterfaceMethods {
    connect(host: string, port: number): boolean;
    disconnect(): void;
    query(sql: string): string[];
}

// Interface using property signature with function type syntax
export interface ServiceInterfaceProperties {
    connect: (host: string, port: number) => boolean;
    disconnect: () => void;
    query: (sql: string) => string[];
}

// Type alias using object literal property function syntax
export type ServiceTypeAliasProperties = {
    connect: (host: string, port: number) => boolean;
    disconnect: () => void;
    query: (sql: string) => string[];
};

// Mixed method and property signatures
export interface ServiceMixed {
    connect(host: string, port: number): boolean;
    disconnect: () => void;
    query(sql: string): string[];
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // All 4 variations represent the exact same 3-member function contract!
    // Total pairs = 4 * 3 / 2 = 6 pairs
    assert_eq!(pairs.len(), 6);
    assert!(pairs.contains(&("ServiceInterfaceMethods".to_string(), "ServiceInterfaceProperties".to_string())));
    assert!(pairs.contains(&("ServiceInterfaceMethods".to_string(), "ServiceTypeAliasProperties".to_string())));
    assert!(pairs.contains(&("ServiceInterfaceMethods".to_string(), "ServiceMixed".to_string())));
    assert!(pairs.contains(&("ServiceInterfaceProperties".to_string(), "ServiceTypeAliasProperties".to_string())));
    assert!(pairs.contains(&("ServiceInterfaceProperties".to_string(), "ServiceMixed".to_string())));
    assert!(pairs.contains(&("ServiceTypeAliasProperties".to_string(), "ServiceMixed".to_string())));
}

// -----------------------------------------------------------------------------
// Category 5: Optional and Readonly Property Mismatches and Matches
// -----------------------------------------------------------------------------

#[test]
fn test_optional_and_readonly_mismatches_rejected() {
    let ws = TestWorkspace::new("optional_readonly");
    ws.init_tsconfig();

    ws.write_file(
        "src/mod_check.ts",
        r#"
export interface BaseShape {
    id: string;
    name: string;
    count: number;
}

export interface OptionalShape {
    id: string;
    name?: string; // mismatch on optional
    count: number;
}

export interface ReadonlyShape {
    readonly id: string; // mismatch on readonly
    name: string;
    count: number;
}

export interface ReadonlyOptionalShape {
    readonly id?: string;
    name: string;
    count: number;
}

// Exact clones of each:
export type BaseShapeClone = {
    id: string;
    name: string;
    count: number;
};

export type OptionalShapeClone = {
    id: string;
    name?: string;
    count: number;
};

export type ReadonlyShapeClone = {
    readonly id: string;
    name: string;
    count: number;
};
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // Exact matches should clone only with their identical counterparts
    assert_eq!(pairs.len(), 3);
    assert!(pairs.contains(&("BaseShape".to_string(), "BaseShapeClone".to_string())));
    assert!(pairs.contains(&("OptionalShape".to_string(), "OptionalShapeClone".to_string())));
    assert!(pairs.contains(&("ReadonlyShape".to_string(), "ReadonlyShapeClone".to_string())));
}

// -----------------------------------------------------------------------------
// Category 6: Heritage Clauses (Extends & Intersection Aliases) Rejection
// -----------------------------------------------------------------------------

#[test]
fn test_heritage_and_intersections_strict_rejection() {
    let ws = TestWorkspace::new("heritage_strict");
    ws.init_tsconfig();

    ws.write_file(
        "src/heritage.ts",
        r#"
export interface RootContract {
    a: string;
    b: number;
    c: boolean;
}

// Extends single interface
export interface ChildSingle extends RootContract {
    a: string;
    b: number;
    c: boolean;
}

// Extends multiple interfaces
export interface EmptyParent {}
export interface ChildMulti extends RootContract, EmptyParent {
    a: string;
    b: number;
    c: boolean;
}

// Type alias intersection
export type IntersectionChild = RootContract & {
    a: string;
    b: number;
    c: boolean;
};

// Standalone type with identical fields without extends (MUST clone with RootContract)
export interface StandaloneClone {
    a: string;
    b: number;
    c: boolean;
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // Only RootContract <-> StandaloneClone should be detected
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("RootContract".to_string(), "StandaloneClone".to_string()));
}

// -----------------------------------------------------------------------------
// Category 7: Advanced Type Annotations (Tuples, Literals, Generics, Typeof)
// -----------------------------------------------------------------------------

#[test]
fn test_advanced_type_annotations() {
    let ws = TestWorkspace::new("advanced_types");
    ws.init_tsconfig();

    ws.write_file(
        "src/advanced.ts",
        r#"
const sampleConfig = { host: "localhost", port: 8080 };

export interface AdvancedA<T, U> {
    genericMap: Map<T, U[]>;
    tupleVal: [string, number, boolean?];
    literalVal: "GET" | "POST" | 404 | true;
    queryVal: typeof sampleConfig;
}

export type AdvancedB<T, U> = {
    queryVal: typeof sampleConfig;
    literalVal: true | 404 | "POST" | "GET";
    tupleVal: [string, number, boolean?];
    genericMap: Map<T, U[]>;
};
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("AdvancedA".to_string(), "AdvancedB".to_string()));
}

// -----------------------------------------------------------------------------
// Category 8: Multiple Cross-File Type Clone Clusters and Deterministic Ordering
// -----------------------------------------------------------------------------

#[test]
fn test_cross_file_multiple_clusters_and_determinism() {
    let ws = TestWorkspace::new("clusters");
    ws.init_tsconfig();

    ws.write_file(
        "src/cluster_a.ts",
        r#"
export interface UserDTO {
    userId: string;
    username: string;
    userEmail: string;
}

export interface OrderDTO {
    orderId: string;
    amount: number;
    currency: string;
}
"#,
    );

    ws.write_file(
        "src/cluster_b.ts",
        r#"
export interface AccountRecord {
    userId: string;
    username: string;
    userEmail: string;
}

export type InvoiceRecord = {
    orderId: string;
    amount: number;
    currency: string;
};
"#,
    );

    let report1 = analyze(&ws.path).unwrap();
    let report2 = analyze(&ws.path).unwrap();

    // Verify determinism across repeated runs
    assert_eq!(report1, report2);

    let pairs = parse_type_clone_pairs(&report1);
    assert_eq!(pairs.len(), 2);
    assert!(pairs.contains(&("UserDTO".to_string(), "AccountRecord".to_string())));
    assert!(pairs.contains(&("OrderDTO".to_string(), "InvoiceRecord".to_string())));
}

// -----------------------------------------------------------------------------
// Category 9: Numeric Keys, String Literal Keys, and Identifier Keys
// -----------------------------------------------------------------------------

#[test]
fn test_numeric_and_string_literal_keys() {
    let ws = TestWorkspace::new("key_types");
    ws.init_tsconfig();

    ws.write_file(
        "src/keys.ts",
        r#"
export interface NumKeys {
    0: string;
    1: number;
    2: boolean;
}

export interface StrKeys {
    "0": string;
    "1": number;
    "2": boolean;
}

export interface IdentKeys {
    a0: string;
    a1: number;
    a2: boolean;
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // NumKeys and StrKeys both resolve property keys to "0", "1", "2"
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("NumKeys".to_string(), "StrKeys".to_string()));
}

// -----------------------------------------------------------------------------
// Category 10: Tuple Element Ordering Sensitivity (Tuple vs Set)
// -----------------------------------------------------------------------------

#[test]
fn test_tuple_element_ordering_sensitivity() {
    let ws = TestWorkspace::new("tuple_order");
    ws.init_tsconfig();

    ws.write_file(
        "src/tuples.ts",
        r#"
// Tuples have strict positional semantics: [string, number, boolean] != [number, string, boolean]
export interface TupleOriginal {
    pos: [string, number, boolean];
    name: string;
    valid: boolean;
}

export interface TupleSwapped {
    pos: [number, string, boolean];
    name: string;
    valid: boolean;
}

export interface TupleClone {
    pos: [string, number, boolean];
    name: string;
    valid: boolean;
}
"#,
    );

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // Only TupleOriginal <-> TupleClone match. TupleSwapped must NOT match.
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("TupleOriginal".to_string(), "TupleClone".to_string()));
}

// -----------------------------------------------------------------------------
// Category 11: High-Volume Multi-File Scaling Stress Test
// -----------------------------------------------------------------------------

#[test]
fn test_high_volume_scale_stress() {
    let ws = TestWorkspace::new("high_volume");
    ws.init_tsconfig();

    for file_idx in 0..5 {
        let mut content = String::new();
        for type_idx in 0..4 {
            let name = format!("Type_{}_{}", file_idx, type_idx);
            // Every type with type_idx == 0 is in Clone Group 0
            // Every type with type_idx == 1 is in Clone Group 1
            // Every type with type_idx == 2 is in Clone Group 2
            // Every type with type_idx == 3 is a unique type with 2 fields (not reported)
            if type_idx == 0 {
                content.push_str(&format!(
                    r#"
export interface {} {{
    f1: string;
    f2: number;
    f3: boolean;
}}
"#,
                    name
                ));
            } else if type_idx == 1 {
                content.push_str(&format!(
                    r#"
export type {} = {{
    f3: boolean;
    f2: number;
    f1: string;
    extra: string[];
}};
"#,
                    name
                ));
            } else if type_idx == 2 {
                content.push_str(&format!(
                    r#"
export interface {} {{
    alpha: {{ x: number; y: number }};
    beta: string | number;
    gamma: [boolean, string];
}}
"#,
                    name
                ));
            } else {
                content.push_str(&format!(
                    r#"
export interface {} {{
    shortA: string;
    shortB: number;
}}
"#,
                    name
                ));
            }
        }
        ws.write_file(&format!("src/file_{}.ts", file_idx), &content);
    }

    let report = analyze(&ws.path).unwrap();
    let pairs = parse_type_clone_pairs(&report);

    // 5 files:
    // Group 0 has 5 types -> (5 * 4) / 2 = 10 pairs
    // Group 1 has 5 types -> (5 * 4) / 2 = 10 pairs
    // Group 2 has 5 types -> (5 * 4) / 2 = 10 pairs
    // Group 3 has 2 fields -> 0 pairs
    // Total pairs = 30
    assert_eq!(pairs.len(), 30);
}
