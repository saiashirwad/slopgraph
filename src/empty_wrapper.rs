use std::collections::HashSet;

use crate::call_graph::CallGraph;
use crate::finding::{Evidence, Finding, Location, PathNode, Shape};

/// A function whose body is only a forward on a typed edge.
#[allow(dead_code)]
pub fn detect(graph: &CallGraph) -> Vec<Finding> {
    detect_with_suppression(graph, &HashSet::new())
}

/// A function whose body is only a forward on a typed edge, suppressing specified function indices.
pub fn detect_with_suppression(graph: &CallGraph, suppressed: &HashSet<usize>) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (from, func) in graph.functions.iter().enumerate() {
        if suppressed.contains(&from) {
            continue;
        }
        let Some(forward) = &func.forward else {
            continue;
        };
        let Some(edge) = graph
            .edges
            .iter()
            .find(|e| e.from == from && e.call_start == forward.call_start)
        else {
            continue;
        };
        let target = &graph.functions[edge.to];
        let annotation = if forward.return_only {
            Some("return only".to_string())
        } else {
            None
        };
        findings.push(Finding {
            shape: Shape::EmptyWrapper,
            location: Location {
                file: func.display.clone(),
                line: func.line,
                span_start: func.span_start,
            },
            subject: func.name.clone(),
            evidence: Evidence::Path {
                nodes: vec![
                    PathNode {
                        label: func.name.clone(),
                        annotation,
                        is_subject: true,
                    },
                    PathNode {
                        label: target.name.clone(),
                        annotation: None,
                        is_subject: false,
                    },
                ],
            },
        });
    }

    findings.sort_by(|a, b| {
        a.location
            .file
            .cmp(&b.location.file)
            .then(a.location.span_start.cmp(&b.location.span_start))
            .then(a.subject.cmp(&b.subject))
    });
    findings
}
