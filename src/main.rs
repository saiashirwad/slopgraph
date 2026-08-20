use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use slopgraph::Options;

#[derive(Parser)]
#[command(
    name = "slopgraph",
    about = "Detect graph-shaped slop in a TypeScript program"
)]
struct Cli {
    /// Path to a tsconfig.json, or to the directory that contains it
    path: PathBuf,

    /// Remove test roots for unreachable detection
    #[arg(long)]
    production: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let options = Options {
        production: cli.production,
    };
    match slopgraph::analyze_with_options(&cli.path, options) {
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

