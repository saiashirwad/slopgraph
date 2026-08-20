use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "slopgraph",
    about = "Detect graph-shaped slop in a TypeScript program"
)]
struct Cli {
    /// Path to a tsconfig.json, or to the directory that contains it
    path: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match slopgraph::analyze(&cli.path) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
