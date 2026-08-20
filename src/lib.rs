mod error;
mod false_sharing;
mod finding;
mod graph;
mod parse;
mod program;
mod report;

pub use error::Error;

/// Load one TypeScript program, detect false sharing, and render a report.
pub fn analyze(path: impl AsRef<std::path::Path>) -> Result<String, Error> {
    let program = program::load(path.as_ref())?;
    let modules = parse::parse_program(&program)?;
    let graph = graph::ModuleGraph::build(&program, modules)?;
    let findings = false_sharing::detect(&graph);
    Ok(report::render(&findings))
}
