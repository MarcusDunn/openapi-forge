//! Output guard: validates files returned from a generator before they hit
//! disk.
//!
//! Rules:
//!
//! * Paths are *relative*. Absolute paths and Windows drive letters are
//!   rejected.
//! * No `..` segments. The host normalises and confirms the result stays
//!   inside the output directory.
//! * No duplicate paths within a single generator output.
//! * Per-file and total byte caps.
//!
//! Generators write into a temp dir adjacent to the output dir; on success
//! the host renames into place. The rename is atomic on the same
//! filesystem.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::runtime::OutputFile;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OutputError {
    #[error("path traversal in plugin output: {0:?}")]
    Traversal(String),
    #[error("absolute path in plugin output: {0:?}")]
    Absolute(String),
    #[error("empty path in plugin output")]
    Empty,
    #[error("duplicate output path: {0:?}")]
    Duplicate(String),
    #[error("file too large: {path:?} ({bytes} > {limit})")]
    TooLargeFile {
        path: String,
        bytes: u64,
        limit: u64,
    },
    #[error("total output too large: {bytes} > {limit}")]
    TooLargeTotal { bytes: u64, limit: u64 },
    #[error("too many output files: {count} > {limit}")]
    TooMany { count: u32, limit: u32 },
    #[error("path contains a non-utf8 component: {0:?}")]
    BadComponent(String),
}

/// Validate a list of output files against the limits encoded in
/// [`Caps`].
pub fn validate_output(files: &[OutputFile], caps: Caps) -> Result<(), OutputError> {
    if files.len() as u64 > caps.max_files as u64 {
        return Err(OutputError::TooMany {
            count: files.len() as u32,
            limit: caps.max_files,
        });
    }
    let mut seen: HashSet<String> = HashSet::with_capacity(files.len());
    let mut total: u64 = 0;
    for f in files {
        let normalized = sanitize_path(&f.path)?;
        let key = normalized.to_string_lossy().into_owned();
        if !seen.insert(key.clone()) {
            return Err(OutputError::Duplicate(key));
        }
        let bytes = f.content.len() as u64;
        if bytes > caps.max_per_file_bytes {
            return Err(OutputError::TooLargeFile {
                path: f.path.clone(),
                bytes,
                limit: caps.max_per_file_bytes,
            });
        }
        total = total.saturating_add(bytes);
        if total > caps.max_total_bytes {
            return Err(OutputError::TooLargeTotal {
                bytes: total,
                limit: caps.max_total_bytes,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub max_files: u32,
    pub max_total_bytes: u64,
    pub max_per_file_bytes: u64,
}

impl Caps {
    pub const fn from_limits(limits: super::Limits) -> Self {
        Self {
            max_files: limits.output_files_max,
            max_total_bytes: limits.output_total_bytes_max,
            max_per_file_bytes: limits.output_per_file_bytes_max,
        }
    }
}

/// Reject absolute paths and `..` traversal; normalise duplicate slashes and
/// `.` segments. Returns the canonicalised relative path.
fn sanitize_path(input: &str) -> Result<PathBuf, OutputError> {
    if input.is_empty() {
        return Err(OutputError::Empty);
    }
    let p = Path::new(input);
    if p.is_absolute() {
        return Err(OutputError::Absolute(input.to_string()));
    }
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => {
                let s = s
                    .to_str()
                    .ok_or_else(|| OutputError::BadComponent(s.to_string_lossy().into_owned()))?;
                // Reject `\` on any platform; these are path separators on
                // Windows that look like literal characters on Unix and
                // create surprising behaviour on round-trip.
                if s.contains('\\') || s.contains('\0') {
                    return Err(OutputError::BadComponent(s.to_string()));
                }
                out.push(s);
            }
            Component::CurDir => {} // drop `.`
            Component::ParentDir => {
                return Err(OutputError::Traversal(input.to_string()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(OutputError::Absolute(input.to_string()));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(OutputError::Empty);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::FileMode;

    fn caps() -> Caps {
        Caps {
            max_files: 100,
            max_total_bytes: 10 * 1024,
            max_per_file_bytes: 1024,
        }
    }

    fn f(path: &str, bytes: usize) -> OutputFile {
        OutputFile {
            path: path.into(),
            content: vec![0; bytes],
            mode: FileMode::Text,
        }
    }

    #[test]
    fn ok_simple() {
        validate_output(&[f("a.txt", 4), f("b/c.txt", 4)], caps()).unwrap();
    }

    #[test]
    fn traversal_rejected() {
        assert!(matches!(
            validate_output(&[f("../etc/passwd", 1)], caps()),
            Err(OutputError::Traversal(_))
        ));
        assert!(matches!(
            validate_output(&[f("a/../../b", 1)], caps()),
            Err(OutputError::Traversal(_))
        ));
    }

    #[test]
    fn absolute_rejected() {
        assert!(matches!(
            validate_output(&[f("/etc/passwd", 1)], caps()),
            Err(OutputError::Absolute(_))
        ));
    }

    #[test]
    fn empty_rejected() {
        assert!(matches!(
            validate_output(&[f("", 1)], caps()),
            Err(OutputError::Empty)
        ));
        assert!(matches!(
            validate_output(&[f("./", 1)], caps()),
            Err(OutputError::Empty)
        ));
    }

    #[test]
    fn backslash_rejected() {
        assert!(matches!(
            validate_output(&[f("foo\\bar", 1)], caps()),
            Err(OutputError::BadComponent(_))
        ));
    }

    #[test]
    fn duplicates_rejected() {
        assert!(matches!(
            validate_output(&[f("a.txt", 1), f("./a.txt", 1)], caps()),
            Err(OutputError::Duplicate(_))
        ));
    }

    #[test]
    fn per_file_cap() {
        let mut c = caps();
        c.max_per_file_bytes = 4;
        assert!(matches!(
            validate_output(&[f("a.txt", 5)], c),
            Err(OutputError::TooLargeFile { .. })
        ));
    }

    #[test]
    fn total_cap() {
        let mut c = caps();
        c.max_total_bytes = 8;
        assert!(matches!(
            validate_output(&[f("a", 5), f("b", 5)], c),
            Err(OutputError::TooLargeTotal { .. })
        ));
    }

    #[test]
    fn count_cap() {
        let mut c = caps();
        c.max_files = 1;
        assert!(matches!(
            validate_output(&[f("a", 1), f("b", 1)], c),
            Err(OutputError::TooMany { .. })
        ));
    }
}
