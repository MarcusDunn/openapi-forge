//! Rust identifier helpers.

/// Strict Rust keywords (2021 edition + reserved). Anything in this set
/// gets a `r#` raw-identifier prefix when it appears as a function /
/// field / variant name. Type-name PascalCase essentially never collides
/// with these (they're all-lowercase) but we still guard for safety.
const RUST_KEYWORDS: &[&str] = &[
    // 2015
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while",
    // 2018
    "async", "await", "dyn",
    // reserved
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof",
    "unsized", "virtual", "yield", "try",
];

/// Keywords that cannot be made into raw identifiers. These get a
/// trailing underscore instead.
const RAW_FORBIDDEN: &[&str] = &["self", "Self", "super", "extern", "crate"];

/// snake_case identifier safe to use as a function, variable, field, or
/// module name. Splits on word boundaries (non-alphanumeric, or
/// lower→upper transitions in camelCase input). Reserved Rust words get
/// a `r#` prefix; the small set that can't be raw idents (`self`, etc.)
/// gets a trailing underscore.
pub fn rust_ident_snake(raw: &str) -> String {
    let parts = split_word_parts(raw);
    let mut out = String::with_capacity(raw.len());
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push('_');
        }
        out.push_str(&part.to_lowercase());
    }
    if out.is_empty() {
        return "_".into();
    }
    if out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    apply_keyword_guard(out)
}

/// PascalCase identifier safe to use as a struct, enum, or variant name.
pub fn rust_ident_pascal(raw: &str) -> String {
    let parts = split_word_parts(raw);
    let mut out = String::with_capacity(raw.len());
    for part in &parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            for c in chars {
                out.extend(c.to_lowercase());
            }
        }
    }
    if out.is_empty() {
        return "_".into();
    }
    if out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    // PascalCase rarely collides, but guard anyway.
    apply_keyword_guard(out)
}

/// Split `raw` into camel/snake-aware word parts. `"createPet"` →
/// `["create", "Pet"]`; `"X-API-Key"` → `["X", "API", "Key"]`;
/// `"foo_bar_baz"` → `["foo", "bar", "baz"]`.
fn split_word_parts(raw: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev_was_lower = false;
    let mut prev_was_upper = false;
    let chars: Vec<char> = raw.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if !ch.is_ascii_alphanumeric() {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            prev_was_lower = false;
            prev_was_upper = false;
            continue;
        }
        let is_upper = ch.is_ascii_uppercase();
        let is_lower = ch.is_ascii_lowercase();
        // lower→upper boundary: `createPet` → `create | Pet`
        if is_upper && prev_was_lower && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        // upper→upper-then-lower (acronym tail): `XMLParser` → `XML | Parser`
        if is_lower && prev_was_upper && current.len() > 1 {
            let last = current.pop().unwrap();
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            current.push(last);
        }
        current.push(ch);
        prev_was_lower = is_lower;
        prev_was_upper = is_upper;
        let _ = i; // index unused
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn apply_keyword_guard(ident: String) -> String {
    if RAW_FORBIDDEN.contains(&ident.as_str()) {
        return format!("{ident}_");
    }
    if RUST_KEYWORDS.contains(&ident.as_str()) {
        return format!("r#{ident}");
    }
    ident
}
