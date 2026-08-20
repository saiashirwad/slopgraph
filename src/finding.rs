use std::path::PathBuf;

/// Canonical shape name from CONTEXT.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    EmptyWrapper,
    FalseSharing,
    NearDuplicate,
    SingleUseChain,
    Unreachable,
}

impl Shape {
    pub fn heading(self) -> &'static str {
        match self {
            Shape::EmptyWrapper => "EMPTY WRAPPER",
            Shape::FalseSharing => "FALSE SHARING",
            Shape::NearDuplicate => "NEAR-DUPLICATE",
            Shape::SingleUseChain => "SINGLE-USE CHAIN",
            Shape::Unreachable => "UNREACHABLE",
        }
    }
}

/// File and span of the subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: PathBuf,
    pub line: u32,
    pub span_start: u32,
}

/// One node on an ASCII evidence path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathNode {
    pub label: String,
    pub annotation: Option<String>,
    pub is_subject: bool,
}

/// Shape-specific proof a human can check. No remedy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    Path { nodes: Vec<PathNode> },
}

/// One instance of a shape in one program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub shape: Shape,
    pub location: Location,
    pub subject: String,
    pub evidence: Evidence,
}
