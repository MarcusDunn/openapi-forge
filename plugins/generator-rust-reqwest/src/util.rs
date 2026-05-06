//! Shared string-escape helpers used by every render module.

/// Render `s` as a Rust `&str` literal, including surrounding quotes
/// and standard escape sequences. Used everywhere we splice user-supplied
/// strings into generated source.
pub fn string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Escape `s` so it's safe to splice into a `format!(...)` template
/// string: doubles `{` / `}` so they don't get interpreted as format
/// args, and escapes `"` / `\` so the resulting Rust source still
/// parses.
pub fn escape_for_format(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out
}
