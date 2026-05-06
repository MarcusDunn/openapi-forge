//! JSON Pointer builder used to thread diagnostic locations through every
//! parse step.
//!
//! Wraps [`jsonptr::PointerBuf`]. The walker code uses one of two patterns:
//!
//! - manual `push_token` / `pop` for tight loops where the borrow checker
//!   makes a guard awkward;
//! - the `with_token` / `with_index` closure helpers when the scope is
//!   small and obvious.

use jsonptr::PointerBuf;

#[derive(Debug, Default, Clone)]
pub(crate) struct Ptr {
    buf: PointerBuf,
}

impl Ptr {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a string token. Encoding (`~0`/`~1`) is handled by `jsonptr`.
    pub fn push_token(&mut self, token: &str) {
        self.buf.push_back(token);
    }

    pub fn push_index(&mut self, index: usize) {
        self.buf.push_back(index);
    }

    pub fn pop(&mut self) {
        self.buf.pop_back();
    }

    #[cfg(test)]
    pub fn as_str(&self) -> &str {
        self.buf.as_str()
    }

    /// Snapshot the current pointer as a [`forge_ir::SpecLocation`].
    pub fn loc(&self, file: Option<&str>) -> forge_ir::SpecLocation {
        forge_ir::SpecLocation {
            pointer: self.buf.as_str().to_string(),
            file: file.map(|f| f.to_string()),
        }
    }

    /// Run `body` with `token` pushed; pop on return (panic-safe via the
    /// internal guard).
    pub fn with_token<F, R>(&mut self, token: &str, body: F) -> R
    where
        F: FnOnce(&mut Ptr) -> R,
    {
        self.push_token(token);
        let _g = PopGuard { ptr: self };
        body(_g.ptr)
    }

    pub fn with_index<F, R>(&mut self, index: usize, body: F) -> R
    where
        F: FnOnce(&mut Ptr) -> R,
    {
        self.push_index(index);
        let _g = PopGuard { ptr: self };
        body(_g.ptr)
    }
}

struct PopGuard<'a> {
    ptr: &'a mut Ptr,
}

impl Drop for PopGuard<'_> {
    fn drop(&mut self) {
        self.ptr.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pointer() {
        let p = Ptr::new();
        assert_eq!(p.as_str(), "");
    }

    #[test]
    fn nested_with_token() {
        let mut p = Ptr::new();
        p.with_token("paths", |p| {
            assert_eq!(p.as_str(), "/paths");
            p.with_token("/pets", |p| {
                // `/` is escaped to `~1`
                assert_eq!(p.as_str(), "/paths/~1pets");
            });
            assert_eq!(p.as_str(), "/paths");
        });
        assert_eq!(p.as_str(), "");
    }

    #[test]
    fn index_segments() {
        let mut p = Ptr::new();
        p.with_token("operations", |p| {
            p.with_index(3, |p| {
                assert_eq!(p.as_str(), "/operations/3");
            });
        });
    }

    #[test]
    fn tilde_in_token_is_escaped() {
        let mut p = Ptr::new();
        p.with_token("a~b", |p| {
            assert_eq!(p.as_str(), "/a~0b");
        });
    }
}
