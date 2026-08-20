mod call_graph;
mod detect;
mod empty_wrapper;
mod entry;
mod error;
mod false_sharing;
mod finding;
mod graph;
mod parse;
mod program;
mod report;
mod tsgo;
mod unreachable;

pub use error::Error;
pub use finding::{Evidence, Finding, Location, PathNode, Shape};
pub use graph::ModuleGraph;
pub use parse::parse_program;
pub use program::{display_path, is_js_file, is_program_file, is_test_file, load, Program};


/// Options for program analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Remove test roots for reachability analysis.
    pub production: bool,
}

/// Load one TypeScript program, detect shapes, and render a report with default options.
pub fn analyze(path: impl AsRef<std::path::Path>) -> Result<String, Error> {
    analyze_with_options(path, Options::default())
}

/// Load one TypeScript program, detect shapes with given options, and render a report.
pub fn analyze_with_options(
    path: impl AsRef<std::path::Path>,
    options: Options,
) -> Result<String, Error> {
    let program = program::load(path.as_ref())?;
    let modules = parse::parse_program(&program)?;
    let graph = graph::ModuleGraph::build(&program, modules)?;
    let calls = call_graph::build(&program)?;
    let findings = detect::run(&graph, &calls, &options);
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
