use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use crate::call_graph::{CallGraph, FnNode};
use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::Options;

/// Detect unreachable files and unreachable functions.
///
/// A file with no import path from an entry point is reported as an unreachable file.
/// A function with no typed-edge path from an entry point is reported as an unreachable function.
/// By default, test files and test functions are roots of reachability.
/// When `--production` is set, test roots are removed.
/// Functions in already unreachable files are suppressed to avoid duplicate findings.
/// Test files and test functions are never reported as unreachable production code.
pub fn detect(modules: &ModuleGraph, calls: &CallGraph, options: &Options) -> Vec<Finding> {
    let mut findings = Vec::new();
    let reachable_files = compute_reachable_files(modules, options);

    // 1. Unreachable files
    let mut module_list: Vec<_> = modules.modules.values().collect();
    module_list.sort_by(|a, b| a.display.cmp(&b.display));

    for module in module_list {
        // Test files are not reported as unreachable production code.
        if modules.test_files.contains(&module.abs) {
            continue;
        }

        if !reachable_files.contains(&module.abs) {
            let label = module.display.to_string_lossy().into_owned();
            findings.push(Finding {
                shape: Shape::Unreachable,
                location: Location {
                    file: module.display.clone(),
                    line: 1,
                    span_start: 0,
                },
                subject: label.clone(),
                evidence: Evidence::Path {
                    nodes: vec![PathNode {
                        label,
                        annotation: None,
                        is_subject: true,
                    }],
                },
            });
        }
    }

    // 2. Unreachable functions
    findings.extend(detect_unreachable_functions(
        modules,
        calls,
        options,
        &reachable_files,
    ));

    findings
}

/// Detect unreachable files in the module graph (standalone helper).
#[allow(dead_code)]
pub fn detect_files(graph: &ModuleGraph, options: &Options) -> Vec<Finding> {
    let reachable = compute_reachable_files(graph, options);
    let mut findings = Vec::new();
    let mut modules: Vec<_> = graph.modules.values().collect();
    modules.sort_by(|a, b| a.display.cmp(&b.display));

    for module in modules {
        if graph.test_files.contains(&module.abs) {
            continue;
        }
        if !reachable.contains(&module.abs) {
            let label = module.display.to_string_lossy().into_owned();
            findings.push(Finding {
                shape: Shape::Unreachable,
                location: Location {
                    file: module.display.clone(),
                    line: 1,
                    span_start: 0,
                },
                subject: label.clone(),
                evidence: Evidence::Path {
                    nodes: vec![PathNode {
                        label,
                        annotation: None,
                        is_subject: true,
                    }],
                },
            });
        }
    }

    findings
}

pub fn compute_reachable_files(graph: &ModuleGraph, options: &Options) -> HashSet<PathBuf> {
    let mut roots: HashSet<_> = graph.entry_points.clone();
    if !options.production {
        roots.extend(graph.test_files.clone());
    }

    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();

    for root in &roots {
        if graph.modules.contains_key(root) && reachable.insert(root.clone()) {
            queue.push_back(root.clone());
        }
    }

    while let Some(current) = queue.pop_front() {
        if let Some(deps) = graph.file_dependencies.get(&current) {
            for dep in deps {
                if graph.modules.contains_key(dep) && reachable.insert(dep.clone()) {
                    queue.push_back(dep.clone());
                }
            }
        }
    }

    reachable
}

fn is_function_exported(func: &FnNode, modules: &ModuleGraph) -> bool {
    func.exported
        || modules
            .modules
            .get(&func.abs)
            .is_some_and(|m| m.exports.iter().any(|e| e.name == func.name))
}

fn detect_unreachable_functions(
    modules: &ModuleGraph,
    calls: &CallGraph,
    options: &Options,
    reachable_files: &HashSet<PathBuf>,
) -> Vec<Finding> {
    let mut reachable_fns = HashSet::new();
    let mut queue = VecDeque::new();

    // Map which entry files have at least one exported function
    let mut entry_files_with_exports = HashSet::new();
    for func in &calls.functions {
        if modules.entry_points.contains(&func.abs) && is_function_exported(func, modules) {
            entry_files_with_exports.insert(func.abs.clone());
        }
    }

    // Identify root functions
    for (i, func) in calls.functions.iter().enumerate() {
        let is_entry_root = if modules.entry_points.contains(&func.abs) {
            if entry_files_with_exports.contains(&func.abs) {
                is_function_exported(func, modules)
            } else {
                true
            }
        } else {
            false
        };

        let is_test_root = !options.production && modules.test_files.contains(&func.abs);

        if (is_entry_root || is_test_root) && reachable_fns.insert(i) {
            queue.push_back(i);
        }
    }

    // Build adjacency list for typed edges
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for edge in &calls.edges {
        adj.entry(edge.from).or_default().push(edge.to);
    }

    // BFS along typed call edges
    while let Some(curr) = queue.pop_front() {
        if let Some(targets) = adj.get(&curr) {
            for &target in targets {
                if target < calls.functions.len() && reachable_fns.insert(target) {
                    queue.push_back(target);
                }
            }
        }
    }

    let mut findings = Vec::new();

    for (i, func) in calls.functions.iter().enumerate() {
        // Suppression Rule 1: Do not report test functions
        if modules.test_files.contains(&func.abs) {
            continue;
        }

        // Suppression Rule 2: Do not report functions in already unreachable files
        if !reachable_files.contains(&func.abs) {
            continue;
        }

        // Suppression Rule 3: Check reachability
        if !reachable_fns.contains(&i) {
            findings.push(Finding {
                shape: Shape::Unreachable,
                location: Location {
                    file: func.display.clone(),
                    line: func.line,
                    span_start: func.span_start,
                },
                subject: func.name.clone(),
                evidence: Evidence::Path {
                    nodes: vec![PathNode {
                        label: func.name.clone(),
                        annotation: None,
                        is_subject: true,
                    }],
                },
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_graph::{FnNode, TypedEdge};
    use crate::graph::Module;
    use crate::parse::ParsedExport;
    use std::collections::HashMap;

    fn make_export(name: &str) -> ParsedExport {
        ParsedExport {
            name: name.to_string(),
            display: name.to_string(),
            line: 1,
            span_start: 0,
        }
    }

    #[test]
    fn test_unreachable_function_suppression_and_production_mode() {
        let mut modules_map = HashMap::new();
        let root_entry = PathBuf::from("/project/src/index.ts");
        let helper_mod = PathBuf::from("/project/src/helper.ts");
        let orphan_mod = PathBuf::from("/project/src/orphan.ts");
        let test_mod = PathBuf::from("/project/tests/unit.test.ts");

        modules_map.insert(
            root_entry.clone(),
            Module {
                abs: root_entry.clone(),
                display: PathBuf::from("src/index.ts"),
                exports: vec![make_export("main")],
            },
        );
        modules_map.insert(
            helper_mod.clone(),
            Module {
                abs: helper_mod.clone(),
                display: PathBuf::from("src/helper.ts"),
                exports: vec![
                    make_export("used"),
                    make_export("dead"),
                    make_export("test_used"),
                ],
            },
        );
        modules_map.insert(
            orphan_mod.clone(),
            Module {
                abs: orphan_mod.clone(),
                display: PathBuf::from("src/orphan.ts"),
                exports: vec![make_export("orphan_fn")],
            },
        );
        modules_map.insert(
            test_mod.clone(),
            Module {
                abs: test_mod.clone(),
                display: PathBuf::from("tests/unit.test.ts"),
                exports: vec![make_export("test_case")],
            },
        );

        let mut entry_points = HashSet::new();
        entry_points.insert(root_entry.clone());

        let mut test_files = HashSet::new();
        test_files.insert(test_mod.clone());

        let mut file_dependencies = HashMap::new();
        file_dependencies.insert(root_entry.clone(), vec![helper_mod.clone()]);
        file_dependencies.insert(test_mod.clone(), vec![helper_mod.clone()]);

        let graph = ModuleGraph {
            root: PathBuf::from("/project"),
            modules: modules_map,
            consumers: HashMap::new(),
            file_dependencies,
            entry_points,
            test_files,
        };

        // Functions:
        // 0: main in src/index.ts (exported)
        // 1: unused_local in src/index.ts (not exported)
        // 2: used in src/helper.ts
        // 3: dead in src/helper.ts
        // 4: test_used in src/helper.ts
        // 5: orphan_fn in src/orphan.ts
        // 6: test_case in tests/unit.test.ts
        let functions = vec![
            FnNode {
                abs: root_entry.clone(),
                display: PathBuf::from("src/index.ts"),
                name: "main".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: root_entry.clone(),
                display: PathBuf::from("src/index.ts"),
                name: "unused_local".to_string(),
                line: 5,
                span_start: 50,
                exported: false,
                forward: None,
            },
            FnNode {
                abs: helper_mod.clone(),
                display: PathBuf::from("src/helper.ts"),
                name: "used".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: helper_mod.clone(),
                display: PathBuf::from("src/helper.ts"),
                name: "dead".to_string(),
                line: 5,
                span_start: 50,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: helper_mod.clone(),
                display: PathBuf::from("src/helper.ts"),
                name: "test_used".to_string(),
                line: 9,
                span_start: 90,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: orphan_mod.clone(),
                display: PathBuf::from("src/orphan.ts"),
                name: "orphan_fn".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: test_mod.clone(),
                display: PathBuf::from("tests/unit.test.ts"),
                name: "test_case".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
        ];

        // Edges:
        // main (0) -> used (2)
        // test_case (6) -> test_used (4)
        let edges = vec![
            TypedEdge {
                from: 0,
                to: 2,
                call_start: 15,
            },
            TypedEdge {
                from: 6,
                to: 4,
                call_start: 15,
            },
        ];

        let call_graph = CallGraph {
            functions,
            edges,
            calls: 2,
            resolved: 2,
        };

        // 1. Default mode (test roots included)
        let findings_default = detect(
            &graph,
            &call_graph,
            &Options {
                production: false,
                ..Default::default()
            },
        );
        let subjects: Vec<_> = findings_default
            .iter()
            .map(|f| (f.location.file.to_str().unwrap(), f.subject.as_str()))
            .collect();
        assert_eq!(
            subjects,
            vec![
                ("src/orphan.ts", "src/orphan.ts"), // Unreachable file
                ("src/index.ts", "unused_local"),   // Unreachable private function
                ("src/helper.ts", "dead"),          // Unreachable function
            ]
        );

        // 2. Production mode (test roots excluded)
        let findings_prod = detect(
            &graph,
            &call_graph,
            &Options {
                production: true,
                ..Default::default()
            },
        );
        let subjects_prod: Vec<_> = findings_prod
            .iter()
            .map(|f| (f.location.file.to_str().unwrap(), f.subject.as_str()))
            .collect();
        assert_eq!(
            subjects_prod,
            vec![
                ("src/orphan.ts", "src/orphan.ts"), // Unreachable file
                ("src/index.ts", "unused_local"),   // Unreachable private function
                ("src/helper.ts", "dead"),          // Unreachable function
                ("src/helper.ts", "test_used"),     // Unreachable in production
            ]
        );
    }
}
