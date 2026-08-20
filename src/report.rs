use std::fmt::Write as _;

use crate::finding::{Evidence, Finding};

/// Human-text report: findings grouped by file, evidence as an ASCII path.
pub fn render(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut current_file: Option<&std::path::Path> = None;

    for finding in findings {
        if current_file != Some(finding.location.file.as_path()) {
            if current_file.is_some() {
                out.push('\n');
            }
            let _ = writeln!(out, "{}", finding.location.file.display());
            out.push('\n');
            current_file = Some(finding.location.file.as_path());
        } else {
            out.push('\n');
        }

        let _ = writeln!(out, "{}", finding.shape.heading());
        let _ = writeln!(
            out,
            "subject: {}  (line {})",
            finding.subject, finding.location.line
        );
        render_evidence(&mut out, finding);
    }

    out
}

fn render_evidence(out: &mut String, finding: &Finding) {
    let Evidence::Path { nodes } = &finding.evidence;
    for (i, node) in nodes.iter().enumerate() {
        let _ = write!(out, "{}", node.label);
        if node.is_subject {
            let _ = writeln!(out, "  ←── finding");
        } else {
            out.push('\n');
        }
        if i + 1 < nodes.len() {
            match node.annotation.as_deref() {
                Some(text) => {
                    let _ = writeln!(out, "     │  {text}");
                    let _ = writeln!(out, "     ▼");
                }
                None => {
                    let _ = writeln!(out, "     │");
                    let _ = writeln!(out, "     ▼");
                }
            }
        }
    }
}
