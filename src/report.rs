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
        let summary = finding_summary(finding);
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
            let _ = writeln!(out, "{}", summary.italic().white());
        } else {
            let _ = writeln!(out, "{heading}");
            let _ = writeln!(
                out,
                "subject: {}  (line {})",
                finding.subject, finding.location.line
            );
            let _ = writeln!(out, "{summary}");
        }

        render_evidence(&mut out, finding, colored);
    }

    out
}

fn finding_summary(finding: &Finding) -> String {
    let Evidence::Path { nodes } = &finding.evidence;
    match finding.shape {
        Shape::Unreachable => {
            if finding.subject == finding.location.file.to_string_lossy().as_ref()
                || finding.subject.ends_with(".ts")
                || finding.subject.ends_with(".tsx")
                || finding.subject.ends_with(".js")
                || finding.subject.ends_with(".jsx")
            {
                format!(
                    "File '{}' is not reachable from any entry point.",
                    finding.subject
                )
            } else {
                format!(
                    "Function '{}' is not reachable from any entry point.",
                    finding.subject
                )
            }
        }

        Shape::SingleUseChain => {
            format!(
                "Chain of {} functions with exactly one caller per function.",
                nodes.len()
            )
        }
        Shape::EmptyWrapper => {
            if let Some(target) = nodes.get(1) {
                format!(
                    "Function '{}' only forwards calls to '{}'.",
                    finding.subject, target.label
                )
            } else {
                format!(
                    "Function '{}' only forwards calls to another function.",
                    finding.subject
                )
            }
        }
        Shape::FalseSharing => {
            format!(
                "Export '{}' is imported only within a single consumer group.",
                finding.subject
            )
        }
        Shape::NearDuplicate => {
            if let Some(other) = nodes.get(1) {
                format!(
                    "Function '{}' has nearly identical implementation to '{}'.",
                    finding.subject, other.label
                )
            } else {
                format!(
                    "Function '{}' has nearly identical implementation to another function.",
                    finding.subject
                )
            }
        }
        Shape::TrampData => {
            if let (Some(caller), Some(target)) = (nodes.first(), nodes.get(1)) {
                format!(
                    "Parameter '{}' in '{}' is forwarded to '{}' without local use.",
                    finding.subject, caller.label, target.label
                )
            } else {
                format!(
                    "Parameter '{}' is forwarded without local use.",
                    finding.subject
                )
            }
        }
        Shape::TypeClone => {
            if let Some(other) = nodes.get(1) {
                format!(
                    "Type '{}' has identical fields and types to '{}' without inheritance.",
                    finding.subject, other.label
                )
            } else {
                format!(
                    "Type '{}' has identical fields and types to another type without inheritance.",
                    finding.subject
                )
            }
        }
        Shape::UnreachingTest => {
            if let (Some(test), Some(prod)) = (nodes.first(), nodes.get(1)) {
                format!(
                    "Test '{}' imports '{}' but makes zero typed calls to it.",
                    test.label, prod.label
                )
            } else {
                format!(
                    "Test imports '{}' but makes zero typed calls to it.",
                    finding.subject
                )
            }
        }
    }
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

