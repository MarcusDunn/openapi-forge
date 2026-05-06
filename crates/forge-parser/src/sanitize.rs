//! Identifier sanitization for IR ids.
//!
//! Produces stable, deterministic ids that are safe to use as map keys and
//! that round-trip through serialization unchanged. The sanitizer does not
//! attempt to produce idiomatic identifiers for any target language —
//! generators handle their own per-language casing.

/// Sanitize an arbitrary string into a stable IR id.
///
/// Replaces every character outside `[A-Za-z0-9_]` with `_`. Empty input
/// produces `_`. Leading digits are preserved (IR ids are not Rust
/// identifiers; generators do their own escaping).
pub fn ident(raw: &str) -> String {
    if raw.is_empty() {
        return "_".to_string();
    }
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

/// Combine pieces into an inline-type id like `getPetById_response_200`.
pub fn join(parts: &[&str]) -> String {
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push('_');
        }
        out.push_str(&ident(p));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough() {
        assert_eq!(ident("Pet"), "Pet");
        assert_eq!(ident("pet_id"), "pet_id");
        assert_eq!(ident("X123"), "X123");
    }

    #[test]
    fn replaces_specials() {
        assert_eq!(ident("foo-bar"), "foo_bar");
        assert_eq!(ident("a.b/c"), "a_b_c");
        assert_eq!(ident("$ref"), "_ref");
    }

    #[test]
    fn empty_is_underscore() {
        assert_eq!(ident(""), "_");
    }

    #[test]
    fn join_concat() {
        assert_eq!(
            join(&["getPetById", "response", "200"]),
            "getPetById_response_200"
        );
    }
}
