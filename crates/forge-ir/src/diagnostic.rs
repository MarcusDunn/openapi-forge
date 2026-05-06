//! Diagnostics and source locations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable diagnostic code, e.g. `"rust-axum/E-UNTAGGED-UNION"`.
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<SpecLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<FixSuggestion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedInfo {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<SpecLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub message: String,
    pub edits: Vec<FixEdit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixEdit {
    pub location: SpecLocation,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecLocation {
    /// RFC 6901 JSON pointer, e.g. `/paths/~1pets/get`.
    pub pointer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

impl SpecLocation {
    pub fn new(pointer: impl Into<String>) -> Self {
        Self {
            pointer: pointer.into(),
            file: None,
        }
    }

    pub fn with_file(pointer: impl Into<String>, file: impl Into<String>) -> Self {
        Self {
            pointer: pointer.into(),
            file: Some(file.into()),
        }
    }
}
