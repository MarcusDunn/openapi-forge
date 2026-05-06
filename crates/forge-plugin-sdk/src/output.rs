//! Plugin-friendly result types.
//!
//! Plugins author results against these and let the SDK's `convert::*`
//! helpers translate to the WIT-generated equivalents at the boundary.

use crate::ir;

/// Mirror of the WIT `file-mode` enum, world-independent so plugins can
/// build `OutputFile`s without touching either world's bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Text,
    Binary,
    Executable,
}

/// One file produced by a generator. Mirrors WIT `output-file`.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputFile {
    pub path: String,
    pub content: Vec<u8>,
    pub mode: FileMode,
}

impl OutputFile {
    pub fn text(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into().into_bytes(),
            mode: FileMode::Text,
        }
    }

    pub fn binary(path: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            mode: FileMode::Binary,
        }
    }

    pub fn executable(path: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            mode: FileMode::Executable,
        }
    }
}

/// What a generator returns. Mirrors WIT `generation-output`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerationOutput {
    pub files: Vec<OutputFile>,
    pub diagnostics: Vec<ir::Diagnostic>,
}

/// What a transformer returns. Mirrors WIT `transform-output`.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformOutput {
    pub spec: ir::Ir,
    pub diagnostics: Vec<ir::Diagnostic>,
}
