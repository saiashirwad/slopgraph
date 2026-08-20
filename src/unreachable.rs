use std::collections::{HashSet, VecDeque};

use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::Options;

/// Detect unreachable files in the module graph.
///
/// A file with no import path from an entry point is reported.
/// By default, test files are roots of reachability.
/// When `--production` is set, test roots are removed, so files reached only
/// through tests become unreachable. Test files themselves stay in the graph
/// and are not reported as unreachable production files.
pub fn detect_files(graph: &ModuleGraph, options: &Options) -> Vec<Finding> {
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

    let mut findings = Vec::new();

    let mut modules: Vec<_> = graph.modules.values().collect();
    modules.sort_by(|a, b| a.display.cmp(&b.display));

    for module in modules {
        // Test files are not reported as unreachable production code.
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
