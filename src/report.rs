use std::fmt::Write as _;
use colored::Colorize;

use crate::finding::{Evidence, Finding, Shape};

/// Human-text report: findings grouped by file, evidence as an ASCII path.
pub fn render(findings: &[Finding]) -> String {
    render_styled(findings, false)
}

/// Human-text report with optional ANSI color styling.
pub fn render_styled(findings: &[Finding], colored: bool) -> String {
    if findings.is_empty() {
        return String::new();
    }

    if colored {
        colored::control::set_override(true);
    } else {
        colored::control::set_override(false);
    }

    let mut out = String::new();
    let mut current_file: Option<&std::path::Path> = None;


    for finding in findings {
        if current_file != Some(finding.location.file.as_path()) {
            if current_file.is_some() {
                out.push('\n');
            }
            if colored {
                let _ = writeln!(
                    out,
                    "{}",
                    finding.location.file.display().to_string().bold().cyan().underline()
                );
            } else {
                let _ = writeln!(out, "{}", finding.location.file.display());
            }
            out.push('\n');
            current_file = Some(finding.location.file.as_path());
        } else {
            out.push('\n');
        }

        let heading = finding.shape.heading();
        if colored {
            let colored_heading = match finding.shape {
                Shape::Unreachable => heading.bold().bright_red(),
                Shape::SingleUseChain => heading.bold().bright_yellow(),
                Shape::EmptyWrapper => heading.bold().bright_yellow(),
                Shape::FalseSharing => heading.bold().bright_magenta(),
                Shape::NearDuplicate => heading.bold().bright_cyan(),
                Shape::TrampData => heading.bold().bright_yellow(),
                Shape::TypeClone => heading.bold().bright_magenta(),
                Shape::UnreachingTest => heading.bold().bright_red(),
            };
            let _ = writeln!(out, "{colored_heading}");
            let _ = writeln!(
                out,
                "{} {}  {}",
                "subject:".dimmed(),
                finding.subject.bold(),
                format!("(line {})", finding.location.line).dimmed()
            );
        } else {
            let _ = writeln!(out, "{heading}");
            let _ = writeln!(
                out,
                "subject: {}  (line {})",
                finding.subject, finding.location.line
            );
        }

        render_evidence(&mut out, finding, colored);
    }

    out
}

fn render_evidence(out: &mut String, finding: &Finding, colored: bool) {
    let Evidence::Path { nodes } = &finding.evidence;
    for (i, node) in nodes.iter().enumerate() {
        if node.is_subject {
            if colored {
                let _ = write!(out, "{}", node.label.bold().bright_white());
                let _ = writeln!(out, "  {}", "←── finding".bold().bright_red());
            } else {
                let _ = write!(out, "{}", node.label);
                let _ = writeln!(out, "  ←── finding");
            }
        } else {
            if colored {
                if let Some(idx) = node.label.find('(') {
                    let name = &node.label[..idx];
                    let paren = &node.label[idx..];
                    let _ = write!(out, "{}{}", name, paren.dimmed());
                } else {
                    let _ = write!(out, "{}", node.label);
                }
                out.push('\n');
            } else {
                let _ = writeln!(out, "{}", node.label);
            }
        }

        if i + 1 < nodes.len() {
            match node.annotation.as_deref() {
                Some(text) => {
                    if colored {
                        let _ = writeln!(out, "     {}  {}", "│".dimmed(), text.italic().bright_cyan());
                        let _ = writeln!(out, "     {}", "▼".dimmed());
                    } else {
                        let _ = writeln!(out, "     │  {text}");
                        let _ = writeln!(out, "     ▼");
                    }
                }
                None => {
                    if colored {
                        let _ = writeln!(out, "     {}", "│".dimmed());
                        let _ = writeln!(out, "     {}", "▼".dimmed());
                    } else {
                        let _ = writeln!(out, "     │");
                        let _ = writeln!(out, "     ▼");
                    }
                }
            }
        }
    }
}

