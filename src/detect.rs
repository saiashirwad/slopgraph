use crate::call_graph::CallGraph;
use crate::finding::Finding;
use crate::graph::ModuleGraph;
use crate::Options;


/// Run every registered detector. A later detector is one new module plus one line here.
pub fn run(modules: &ModuleGraph, calls: &CallGraph, options: &Options) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(crate::false_sharing::detect(modules));
    let (chain_findings, suppressed) = crate::single_use_chain::detect(modules, calls, options);
    findings.extend(chain_findings);
    findings.extend(crate::empty_wrapper::detect_with_suppression(
        calls,
        &suppressed,
    ));
    findings.extend(crate::unreachable::detect(modules, calls, options));
    findings.extend(crate::near_duplicate::detect(modules, calls, options));
    findings.extend(crate::tramp_data::detect(modules, calls, options));
    findings.extend(crate::type_clone::detect(modules, calls, options));
    findings.extend(crate::unreaching_test::detect(modules, calls, options));
    findings.sort_by(|a, b| {
        a.location
            .file
            .cmp(&b.location.file)
            .then(a.location.span_start.cmp(&b.location.span_start))
            .then(a.subject.cmp(&b.subject))
    });
    findings
}
