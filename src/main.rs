use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use slopgraph::Options;

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

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

    /// Include exported functions in single-use chain detection
    #[arg(long)]
    include_exported: bool,

    /// Control colored terminal output [default: auto]
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,
}

fn should_color(choice: ColorChoice) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let options = Options {
        production: cli.production,
        include_exported: cli.include_exported,
    };
    let use_color = should_color(cli.color);
    match slopgraph::analyze_styled_with_options(&cli.path, options, use_color) {
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

