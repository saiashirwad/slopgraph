mod call_graph;
mod detect;
mod empty_wrapper;
mod error;
mod false_sharing;
mod finding;
mod graph;
mod parse;
mod program;
mod report;
mod tsgo;

pub use error::Error;

/// Load one TypeScript program, detect shapes, and render a report.
pub fn analyze(path: impl AsRef<std::path::Path>) -> Result<String, Error> {
    let program = program::load(path.as_ref())?;
    let modules = parse::parse_program(&program)?;
    let graph = graph::ModuleGraph::build(&program, modules)?;
    let calls = call_graph::build(&program)?;
    let findings = detect::run(&graph, &calls);
    Ok(report::render(&findings))
}

/// How many oxc call nodes mapped onto a tsgo `getResolvedSignature` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedCallCounts {
    pub calls: usize,
    pub resolved: usize,
}

/// Resolve signatures by file+offset on a program. Proves the oxc ↔ tsgo map.
pub fn resolved_call_counts(
    path: impl AsRef<std::path::Path>,
) -> Result<ResolvedCallCounts, Error> {
    let program = program::load(path.as_ref())?;
    let calls = call_graph::build(&program)?;
    Ok(ResolvedCallCounts {
        calls: calls.calls,
        resolved: calls.resolved,
    })
}
