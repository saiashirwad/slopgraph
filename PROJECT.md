# Project: slopgraph

## Architecture
`slopgraph` detects graph-shaped anti-patterns ("slop") in TypeScript codebases using oxc AST parsing and tsgo typed call resolution.
- Module Graph (`src/graph.rs`): Resolves module imports, file dependencies, entry points, and test files.
- Call Graph (`src/call_graph.rs`): Extracts function definitions, spans, forwarding signatures, and typed call edges.
- Detectors (`src/`): Standalone analysis passes that examine the module and call graphs and emit `Finding` objects.
- Report Engine (`src/report.rs`): Formats findings grouped by file with ASCII path evidence and `←── finding` markers.
- CLI (`src/main.rs` & `src/lib.rs`): CLI entry point accepting path and flags (`--production`, `--include-exported`).

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Unreachable Functions Detector | Detect functions with no typed-edge path from entry points; exclude test roots under `--production`; suppress if file unreachable | M1 | Issue #16 (DONE) |
| 2 | Single-Use Chain Detector | Detect call chains >= 2 functions where in-degree is 1 on typed edges; exclude exported by default; allow via `--include-exported`; deduplicate empty wrappers | M2 | Issue #17 |
| 3 | Near-Duplicate Detector | Detect function pairs with distinct names, >= 20 AST nodes, >= 0.7 confidence (50-token window hash + AST-kind hash) | M3 | Issue #18 (DONE) |
| 4 | Tramp Data Detector | Detect forwarded parameters with 0 local read operations passed to typed call targets | M4 | Issue #19 |
| 5 | Type Clone Detector | Detect distinct types with >= 3 identical fields and type annotations, with no `extends` relationship | M5 | Issue #20 |
| 6 | Unreaching Test Detector | Detect test files importing production modules with 0 typed calls reaching that module | M6 | Issue #21 |
| 7 | Full-Report Conformance & Golden Tests | Full integration across all 8 detectors, CLI flags, README documentation, and golden integration tests | M7 | Issue #22 & #1 |
| 8 | Issue Lifecycle Management | Close each issue (#16-#22, #1) via `gh issue close <num> --reason completed` after verification | M1-M7 | R8 (Issues #16, #17, #18 closed) |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1: Unreachable Functions | `src/unreachable.rs`, `src/detect.rs`, tests, close #16 | none | DONE |
| 2 | M2: Single-Use Chain | `src/single_use_chain.rs`, `src/lib.rs`, `src/main.rs`, `src/empty_wrapper.rs`, `src/detect.rs`, tests, close #17 | none | DONE |
| 3 | M3: Near-Duplicate | `src/near_duplicate.rs`, `src/detect.rs`, `src/finding.rs`, tests, close #18 | none | DONE |
| 4 | M4: Tramp Data | `src/tramp_data.rs`, `src/detect.rs`, `src/finding.rs`, tests, close #19 | none | DONE |
| 5 | M5: Type Clone | `src/type_clone.rs`, `src/detect.rs`, `src/finding.rs`, tests, close #20 | none | PLANNED |
| 6 | M6: Unreaching Test | `src/unreaching_test.rs`, `src/detect.rs`, `src/finding.rs`, tests, close #21 | none | PLANNED |
| 7 | M7: Full-Report Conformance | CLI flags, `README.md`, full golden tests, close #22 & #1 | M1-M6 | PLANNED |

## Interface Contracts
### Detectors ↔ Report Engine
- Every detector returns `Vec<Finding>`.
- `Finding` has `Shape`, `Location` (`file`, `line`, `span_start`), `subject: String`, and `Evidence::Path { nodes: Vec<PathNode> }`.
- In `PathNode`, `is_subject: true` places the `←── finding` marker on that node.
- `Shape::heading()` provides the exact string heading for the card.

## Code Layout
- `src/main.rs`: CLI entry point using Clap
- `src/lib.rs`: Library API and `Options`
- `src/finding.rs`: `Shape`, `Location`, `PathNode`, `Evidence`, `Finding`
- `src/graph.rs`: Module graph extraction and dependencies
- `src/call_graph.rs`: Call graph and function AST extraction
- `src/detect.rs`: Runner invoking all detectors and sorting findings
- `src/report.rs`: ASCII report formatter
- `src/unreachable.rs`: Unreachable files and functions detector
- `src/single_use_chain.rs`: Single-Use Chain detector
- `src/near_duplicate.rs`: Near-duplicate function detector
- `src/tramp_data.rs`: Tramp data parameter detector
- `src/type_clone.rs`: Type clone detector
- `src/unreaching_test.rs`: Unreaching test detector
- `tests/`: Integration and golden tests
