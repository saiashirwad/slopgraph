use crate::call_graph::CallGraph;
use crate::finding::Finding;
use crate::graph::ModuleGraph;

/// Run every registered detector. A later detector is one new module plus one line here.
pub fn run(modules: &ModuleGraph, calls: &CallGraph) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(crate::false_sharing::detect(modules));
    findings.extend(crate::empty_wrapper::detect(calls));
    findings.sort_by(|a, b| {
        a.location
            .file
            .cmp(&b.location.file)
            .then(a.location.span_start.cmp(&b.location.span_start))
            .then(a.subject.cmp(&b.subject))
    });
    findings
}
