use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::program::display_path;

/// An export with exactly one consumer group.
pub fn detect(graph: &ModuleGraph) -> Vec<Finding> {
    let mut findings = Vec::new();

    let mut modules: Vec<_> = graph.modules.values().collect();
    modules.sort_by(|a, b| a.display.cmp(&b.display));

    for module in modules {
        let mut exports = module.exports.clone();
        exports.sort_by(|a, b| a.span_start.cmp(&b.span_start).then(a.name.cmp(&b.name)));
        for export in exports {
            let key = (module.abs.clone(), export.name.clone());
            let Some(importers) = graph.consumers.get(&key) else {
                continue;
            };
            if importers.is_empty() {
                continue;
            }

            let mut groups: BTreeSet<PathBuf> = BTreeSet::new();
            for importer in importers {
                groups.insert(graph.consumer_group_dir(importer));
            }
            if groups.len() != 1 {
                continue;
            }

            let mut importer_labels: Vec<String> = importers
                .iter()
                .map(|p| display_path(&graph.root, p).to_string_lossy().into_owned())
                .collect();
            importer_labels.sort();
            importer_labels.dedup();

            let mut nodes: Vec<PathNode> = importer_labels
                .into_iter()
                .map(|label| PathNode {
                    label,
                    annotation: None,
                    is_subject: false,
                })
                .collect();
            if let Some(first) = nodes.first_mut() {
                first.annotation = Some("one consumer group".to_string());
            }
            nodes.push(PathNode {
                label: export.display.clone(),
                annotation: None,
                is_subject: true,
            });

            findings.push(Finding {
                shape: Shape::FalseSharing,
                location: Location {
                    file: module.display.clone(),
                    line: export.line,
                    span_start: export.span_start,
                },
                subject: export.display,
                evidence: Evidence::Path { nodes },
            });
        }
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
