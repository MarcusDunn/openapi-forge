//! TypeScript identifier helpers.

const TS_RESERVED: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "as",
    "implements",
    "interface",
    "let",
    "package",
    "private",
    "protected",
    "public",
    "static",
];

/// camelCase identifier safe to use as a method or variable name. Reserved
/// words get a trailing underscore.
pub fn ts_ident(raw: &str) -> String {
    let mut buf = String::with_capacity(raw.len());
    let mut upper_next = false;
    for (i, ch) in raw.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if upper_next {
                buf.extend(ch.to_uppercase());
                upper_next = false;
            } else if i == 0 {
                buf.extend(ch.to_lowercase());
            } else {
                buf.push(ch);
            }
        } else {
            // word boundary
            if !buf.is_empty() {
                upper_next = true;
            }
        }
    }
    if buf.is_empty() {
        return "_".into();
    }
    if buf.chars().next().unwrap().is_ascii_digit() {
        buf.insert(0, '_');
    }
    if TS_RESERVED.contains(&buf.as_str()) {
        buf.push('_');
    }
    buf
}

/// PascalCase identifier safe to use as a type or class name.
pub fn ts_type_name(raw: &str) -> String {
    let camel = ts_ident(raw);
    if camel == "_" {
        return camel;
    }
    let mut chars = camel.chars();
    let first = chars.next().unwrap();
    let mut out: String = first.to_uppercase().collect();
    out.extend(chars);
    out
}

// Native unit tests are not possible: the SDK imports `compile_error!` on
// non-wasm32 targets, so the plugin only builds for `wasm32-wasip2`.
// Coverage for these helpers comes from the integration test in
// `tests/integration.rs`, which exercises the full `generate` path.
