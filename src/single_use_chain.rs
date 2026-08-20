use std::collections::HashSet;

use crate::call_graph::{CallGraph, FnNode};
use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::Options;

/// Detect call chains of two or more functions where each function has in-degree 1 on typed edges.
/// Returns the detected findings and a set of function indices to suppress from empty wrapper detection.
pub fn detect(
    modules: &ModuleGraph,
    calls: &CallGraph,
    options: &Options,
) -> (Vec<Finding>, HashSet<usize>) {
    let n = calls.functions.len();
    if n == 0 {
        return (Vec::new(), HashSet::new());
    }

    let mut in_degree = vec![0usize; n];
    for edge in &calls.edges {
        if edge.to < n {
            in_degree[edge.to] += 1;
        }
    }

    let mut out_edges = vec![Vec::new(); n];
    for edge in &calls.edges {
        if edge.from < n && edge.to < n {
            out_edges[edge.from].push(edge.to);
        }
    }
    for edges in &mut out_edges {
        edges.sort_by(|&a, &b| {
            let fa = &calls.functions[a];
            let fb = &calls.functions[b];
            fa.display
                .cmp(&fb.display)
                .then(fa.span_start.cmp(&fb.span_start))
                .then(fa.name.cmp(&fb.name))
        });
        edges.dedup();
    }

    let is_eligible = |idx: usize| -> bool {
        if idx >= n {
            return false;
        }
        if in_degree[idx] != 1 {
            return false;
        }
        if !options.include_exported && is_function_exported(&calls.functions[idx], modules) {
            return false;
        }
        true
    };

    let mut entry_edges = Vec::new();
    for edge in &calls.edges {
        let v0 = edge.from;
        let v1 = edge.to;
        if v0 < n && v1 < n && !is_eligible(v0) && is_eligible(v1) {
            entry_edges.push((v0, v1));
        }
    }

    entry_edges.sort_by(|&(a0, a1), &(b0, b1)| {
        let fa0 = &calls.functions[a0];
        let fb0 = &calls.functions[b0];
        fa0.display
            .cmp(&fb0.display)
            .then(fa0.span_start.cmp(&fb0.span_start))
            .then(fa0.name.cmp(&fb0.name))
            .then_with(|| {
                let fa1 = &calls.functions[a1];
                let fb1 = &calls.functions[b1];
                fa1.display
                    .cmp(&fb1.display)
                    .then(fa1.span_start.cmp(&fb1.span_start))
                    .then(fa1.name.cmp(&fb1.name))
            })
    });
    entry_edges.dedup();

    let mut chains: Vec<Vec<usize>> = Vec::new();
    for (v0, v1) in entry_edges {
        let mut path = vec![v0, v1];
        let mut visited = HashSet::new();
        visited.insert(v0);
        visited.insert(v1);
        dfs_extend(
            v1,
            &mut path,
            &mut visited,
            &out_edges,
            &is_eligible,
            &mut chains,
        );
    }

    let mut findings = Vec::new();
    let mut suppressed = HashSet::new();

    for chain in chains {
        let v0 = chain[0];
        let v1 = chain[1];

        for &node_idx in &chain[1..] {
            suppressed.insert(node_idx);
        }

        if let Some(forward) = &calls.functions[v0].forward {
            let forward_to_v1 = calls
                .edges
                .iter()
                .any(|e| e.from == v0 && e.to == v1 && e.call_start == forward.call_start);
            if forward_to_v1 {
                suppressed.insert(v0);
            }
        }

        let mut nodes = Vec::with_capacity(chain.len());
        let v0_exported = is_function_exported(&calls.functions[v0], modules);
        let v0_label = if v0_exported && !options.include_exported {
            format!("{}   (exported, not in chain)", calls.functions[v0].name)
        } else {
            calls.functions[v0].name.clone()
        };

        nodes.push(PathNode {
            label: v0_label,
            annotation: None,
            is_subject: false,
        });

        nodes.push(PathNode {
            label: calls.functions[v1].name.clone(),
            annotation: None,
            is_subject: true,
        });

        for &vi in &chain[2..] {
            nodes.push(PathNode {
                label: calls.functions[vi].name.clone(),
                annotation: None,
                is_subject: false,
            });
        }

        findings.push(Finding {
            shape: Shape::SingleUseChain,
            location: Location {
                file: calls.functions[v1].display.clone(),
                line: calls.functions[v1].line,
                span_start: calls.functions[v1].span_start,
            },
            subject: calls.functions[v1].name.clone(),
            evidence: Evidence::Path { nodes },
        });
    }

    findings.sort_by(|a, b| {
        a.location
            .file
            .cmp(&b.location.file)
            .then(a.location.span_start.cmp(&b.location.span_start))
            .then(a.subject.cmp(&b.subject))
    });

    (findings, suppressed)
}

fn dfs_extend(
    curr: usize,
    path: &mut Vec<usize>,
    visited: &mut HashSet<usize>,
    out_edges: &[Vec<usize>],
    is_eligible: &impl Fn(usize) -> bool,
    chains: &mut Vec<Vec<usize>>,
) {
    let mut extended = false;
    for &next in &out_edges[curr] {
        if is_eligible(next) && !visited.contains(&next) {
            extended = true;
            visited.insert(next);
            path.push(next);
            dfs_extend(next, path, visited, out_edges, is_eligible, chains);
            path.pop();
            visited.remove(&next);
        }
    }
    if !extended && path.len() >= 3 {
        chains.push(path.clone());
    }
}

fn is_function_exported(func: &FnNode, modules: &ModuleGraph) -> bool {
    func.exported
        || modules
            .modules
            .get(&func.abs)
            .is_some_and(|m| m.exports.iter().any(|e| e.name == func.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_graph::{Forward, TypedEdge};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_test_graph(
        fns: Vec<(&str, bool, Option<Forward>)>,
        edges: Vec<(usize, usize)>,
    ) -> (ModuleGraph, CallGraph) {
        let root = PathBuf::from("/project");
        let abs = root.join("src/test.ts");
        let display = PathBuf::from("src/test.ts");

        let functions: Vec<FnNode> = fns
            .into_iter()
            .enumerate()
            .map(|(i, (name, exported, forward))| FnNode {
                abs: abs.clone(),
                display: display.clone(),
                name: name.to_string(),
                line: (i as u32) + 1,
                span_start: (i as u32) * 20,
                exported,
                forward,
            })
            .collect();

        let edges_len = edges.len();
        let typed_edges: Vec<TypedEdge> = edges
            .into_iter()
            .map(|(from, to)| TypedEdge {
                from,
                to,
                call_start: (from as u32) * 20 + 5,
            })
            .collect();

        let calls = CallGraph {
            functions,
            edges: typed_edges,
            calls: edges_len,
            resolved: edges_len,
        };

        let modules = ModuleGraph {
            root,
            modules: HashMap::new(),
            entry_points: HashSet::new(),
            test_files: HashSet::new(),
            file_dependencies: HashMap::new(),
            consumers: HashMap::new(),
        };

        (modules, calls)
    }

    #[test]
    fn test_two_node_chain_detection() {
        let (modules, calls) = make_test_graph(
            vec![
                ("handleOrder", true, None),
                ("prepareOrder", false, None),
                ("validateAndSave", false, None),
            ],
            vec![(0, 1), (1, 2)],
        );

        let options = Options::default();
        let (findings, suppressed) = detect(&modules, &calls, &options);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].shape, Shape::SingleUseChain);
        assert_eq!(findings[0].subject, "prepareOrder");

        let Evidence::Path { nodes } = &findings[0].evidence;
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].label, "handleOrder   (exported, not in chain)");
        assert!(!nodes[0].is_subject);
        assert_eq!(nodes[1].label, "prepareOrder");
        assert!(nodes[1].is_subject);
        assert_eq!(nodes[2].label, "validateAndSave");
        assert!(!nodes[2].is_subject);

        assert!(suppressed.contains(&1));
        assert!(suppressed.contains(&2));
    }

    #[test]
    fn test_single_eligible_node_is_not_chain() {
        let (modules, calls) = make_test_graph(
            vec![
                ("handleOrder", true, None),
                ("prepareOrder", false, None),
                ("multiCalled", false, None),
                ("anotherCaller", true, None),
            ],
            vec![(0, 1), (1, 2), (3, 2)],
        );

        let options = Options::default();
        let (findings, suppressed) = detect(&modules, &calls, &options);

        assert_eq!(findings.len(), 0);
        assert!(suppressed.is_empty());
    }

    #[test]
    fn test_include_exported_flag() {
        let (modules, calls) = make_test_graph(
            vec![
                ("entryRoot", true, None),
                ("exportedMiddle", true, None),
                ("finalLeaf", false, None),
            ],
            vec![(0, 1), (1, 2)],
        );

        // Without flag: exportedMiddle is excluded, chain length is 1 (finalLeaf only), no finding
        let (findings_default, _) = detect(&modules, &calls, &Options::default());
        assert_eq!(findings_default.len(), 0);

        // With flag: exportedMiddle is included, chain is entryRoot -> exportedMiddle -> finalLeaf
        let options = Options {
            include_exported: true,
            production: false,
        };
        let (findings_included, _) = detect(&modules, &calls, &options);
        assert_eq!(findings_included.len(), 1);
        assert_eq!(findings_included[0].subject, "exportedMiddle");

        let Evidence::Path { nodes } = &findings_included[0].evidence;
        assert_eq!(nodes[0].label, "entryRoot");
        assert_eq!(nodes[1].label, "exportedMiddle");
        assert_eq!(nodes[2].label, "finalLeaf");
    }

    #[test]
    fn test_forwarder_suppression() {
        let (modules, calls) = make_test_graph(
            vec![
                (
                    "startPipeline",
                    true,
                    Some(Forward {
                        call_start: 5,
                        return_only: true,
                    }),
                ),
                (
                    "forwardStep",
                    false,
                    Some(Forward {
                        call_start: 25,
                        return_only: true,
                    }),
                ),
                ("computeStep", false, None),
            ],
            vec![(0, 1), (1, 2)],
        );

        let options = Options::default();
        let (findings, suppressed) = detect(&modules, &calls, &options);

        assert_eq!(findings.len(), 1);
        assert!(suppressed.contains(&0)); // startPipeline forwards to chain head
        assert!(suppressed.contains(&1)); // forwardStep is inside chain
        assert!(suppressed.contains(&2)); // computeStep is inside chain
    }
}
