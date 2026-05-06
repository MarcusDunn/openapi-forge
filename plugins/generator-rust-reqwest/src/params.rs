//! Path / query / header / cookie parameter assembly. Mirrors
//! TS-fetch's `render_query_assembly` / `render_url_construction` /
//! `render_cookie_assembly` shape, including style dispatch.

use forge_plugin_sdk::ir;

use crate::naming::rust_ident_snake;
use crate::util::{escape_for_format, string_literal};

/// Inline percent-encoding helpers + URL builders that every generated
/// `client.rs` imports. Emitted once near the top of the generated
/// `client.rs` so the per-method code can call them without re-emitting
/// boilerplate. Uses the `percent_encoding` crate (added as a generated
/// dep) — small, widely trusted, no transitive deps.
pub const URL_HELPERS: &str = r#"// --- URL / param helpers (generated) ---------------------------------

#[allow(dead_code)]
const _PE_SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

#[allow(dead_code)]
fn _pe<T: std::fmt::Display + ?Sized>(v: &T) -> String {
    percent_encoding::utf8_percent_encode(&v.to_string(), _PE_SET).to_string()
}

#[allow(dead_code)]
fn _append_query(url: &mut String, key: &str, value: &str) {
    let sep = if url.contains('?') { '&' } else { '?' };
    url.push(sep);
    url.push_str(&_pe(key));
    url.push('=');
    url.push_str(&_pe(value));
}

/// `style: form, explode: true` over an array → repeats the key per
/// element. Empty arrays append nothing.
#[allow(dead_code)]
fn _append_query_form_explode<T: std::fmt::Display>(url: &mut String, key: &str, values: &[T]) {
    for v in values {
        _append_query(url, key, &v.to_string());
    }
}

/// Single key, values joined by `delim`, each percent-encoded.
/// Covers `style: pipeDelimited` (`|`), `spaceDelimited` (` `), and
/// `style: form, explode: false` (`,`).
#[allow(dead_code)]
fn _append_query_delimited<T: std::fmt::Display>(
    url: &mut String,
    key: &str,
    values: &[T],
    delim: char,
) {
    if values.is_empty() {
        return;
    }
    let sep = if url.contains('?') { '&' } else { '?' };
    url.push(sep);
    url.push_str(&_pe(key));
    url.push('=');
    let mut first = true;
    for v in values {
        if !first {
            url.push(delim);
        }
        url.push_str(&_pe(&v.to_string()));
        first = false;
    }
}

"#;

// -- URL construction -----------------------------------------------------

/// Build the request URL with path-template substitution, including
/// percent-encoding of scalar params and comma-joining of `style: simple`
/// array path params (#42).
pub fn render_url_construction(spec: &ir::Ir, op: &ir::Operation) -> String {
    let mut s = String::new();
    let template = &op.path_template;

    if op.path_params.is_empty() {
        // No substitutions; still need a `mut` binding so the query
        // assembly can append.
        s.push_str(&format!(
            "        let mut url = format!(\"{{base}}{}\", base = self.base_url);\n",
            escape_for_format(template)
        ));
        return s;
    }

    // Pre-compute the rendered (already encoded / joined) value for each
    // path param. Done as a `let` binding per param so the template can
    // reference identifiers.
    for p in &op.path_params {
        let snake = rust_ident_snake(&p.name);
        if is_array_param(spec, &p.r#type) {
            // style: simple over an array → comma-joined,
            // per-element percent-encoded.
            s.push_str(&format!(
                "        let {snake} = {snake}.iter().map(|v| _pe(v)).collect::<Vec<_>>().join(\",\");\n"
            ));
        } else {
            s.push_str(&format!("        let {snake} = _pe(&{snake});\n"));
        }
    }

    // Convert `{petId}` placeholders into format-arg placeholders
    // referencing the (snake-cased) Rust ident. Real-world specs
    // sometimes leave a `{x}` in the template without declaring `x`
    // under `parameters`; substituting that would emit `format!` with
    // an undefined identifier and fail to compile. Emit such
    // placeholders literally instead — the URL is wrong at runtime,
    // but the client compiles, matching the spec author's apparent
    // intent that the path text passes through unchanged.
    let declared: std::collections::HashSet<String> = op
        .path_params
        .iter()
        .map(|p| rust_ident_snake(&p.name))
        .collect();
    let mut tpl = String::new();
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(end) = chars[i + 1..].iter().position(|c| *c == '}') {
                let raw_name: String = chars[i + 1..i + 1 + end].iter().collect();
                let snake = rust_ident_snake(&raw_name);
                if declared.contains(&snake) {
                    tpl.push('{');
                    tpl.push_str(&snake);
                    tpl.push('}');
                } else {
                    tpl.push_str("{{");
                    tpl.push_str(&raw_name);
                    tpl.push_str("}}");
                }
                i += 1 + end + 1;
                continue;
            }
        }
        match chars[i] {
            '{' => tpl.push_str("{{"),
            '}' => tpl.push_str("}}"),
            c => tpl.push(c),
        }
        i += 1;
    }
    let mut args = String::from("base = self.base_url");
    for p in &op.path_params {
        let snake = rust_ident_snake(&p.name);
        args.push_str(&format!(", {snake} = {snake}"));
    }
    s.push_str(&format!(
        "        let mut url = format!(\"{{base}}{}\", {});\n",
        tpl, args
    ));
    s
}

// -- Query assembly -------------------------------------------------------

/// Per-style query assembly. Emits direct `_append_query*` helper calls
/// against the `mut url` bound by `render_url_construction`.
pub fn render_query_assembly(spec: &ir::Ir, op: &ir::Operation) -> String {
    if op.query_params.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    for p in &op.query_params {
        s.push_str(&render_one_query_param(spec, p));
    }
    s
}

fn render_one_query_param(spec: &ir::Ir, p: &ir::Parameter) -> String {
    let snake = rust_ident_snake(&p.name);
    let key = string_literal(&p.name);
    let style = p.style.unwrap_or(ir::ParameterStyle::Form);
    let is_array = is_array_param(spec, &p.r#type);
    let is_object = is_object_param(spec, &p.r#type);

    // For arrays in pipe / space / form-collapsed styles we must reach into
    // the array via `&v[..]`. The "value binding" inside the optional-
    // unwrap (`if let Some(v) = ...`) needs to be `&Vec<T>` so `v.iter()`
    // works.
    let body_expr = match (style, is_array, is_object) {
        // form-explode arrays → repeated key
        (ir::ParameterStyle::Form, true, _) if p.explode => format!(
            "_append_query_form_explode(&mut url, {key}, &$VAL$);"
        ),
        // form-collapsed arrays → comma-joined
        (ir::ParameterStyle::Form, true, _) => format!(
            "_append_query_delimited(&mut url, {key}, &$VAL$, ',');"
        ),
        (ir::ParameterStyle::PipeDelimited, true, _) => format!(
            "_append_query_delimited(&mut url, {key}, &$VAL$, '|');"
        ),
        (ir::ParameterStyle::SpaceDelimited, true, _) => format!(
            "_append_query_delimited(&mut url, {key}, &$VAL$, ' ');"
        ),
        (ir::ParameterStyle::DeepObject, _, true) => render_deep_object_appends(spec, p, "$VAL$"),
        (ir::ParameterStyle::Form, false, true) if p.explode => {
            // form+explode on an object → field-per-key (without brackets).
            // Rare; treat like deepObject without the bracket prefix.
            render_object_form_explode_appends(spec, p, "$VAL$")
        }
        // Scalars (or fallthroughs)
        _ => format!("_append_query(&mut url, {key}, &$VAL$.to_string());"),
    };

    if p.required {
        format!("        {{ {} }}\n", body_expr.replace("$VAL$", &snake))
    } else {
        // Optional → Option<T>. For arrays the unwrapped binding is `&Vec<T>`;
        // for scalars it's `&T`. Both work with the helpers (which take `&[T]`
        // / `&T: Display`).
        format!(
            "        if let Some(v) = &{snake} {{ {} }}\n",
            body_expr.replace("$VAL$", "v")
        )
    }
}

/// Emit `_append_query` calls per object property, using `key[propname]`
/// as the URL-side key. Skips properties that are themselves `Option<T>`
/// when `None`.
fn render_deep_object_appends(spec: &ir::Ir, p: &ir::Parameter, val: &str) -> String {
    render_object_field_appends(spec, p, val, /*bracketed=*/ true)
}

fn render_object_form_explode_appends(spec: &ir::Ir, p: &ir::Parameter, val: &str) -> String {
    render_object_field_appends(spec, p, val, /*bracketed=*/ false)
}

fn render_object_field_appends(
    spec: &ir::Ir,
    p: &ir::Parameter,
    val: &str,
    bracketed: bool,
) -> String {
    let Some(obj) = resolve_object(spec, &p.r#type) else {
        // Unresolvable → fall back to scalar Display.
        return format!(
            "_append_query(&mut url, {}, &{val}.to_string());",
            string_literal(&p.name)
        );
    };
    let mut out = String::new();
    for prop in &obj.properties {
        let prop_field = rust_ident_snake(&prop.name);
        let url_key = if bracketed {
            string_literal(&format!("{}[{}]", p.name, prop.name))
        } else {
            string_literal(&prop.name)
        };
        let is_required = prop.required;
        if is_required {
            out.push_str(&format!(
                "_append_query(&mut url, {url_key}, &{val}.{prop_field}.to_string()); "
            ));
        } else {
            out.push_str(&format!(
                "if let Some(_v) = &{val}.{prop_field} {{ _append_query(&mut url, {url_key}, &_v.to_string()); }} "
            ));
        }
    }
    out
}

// -- Header assembly ------------------------------------------------------

pub fn render_header_assembly(spec: &ir::Ir, op: &ir::Operation) -> String {
    if op.header_params.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    for p in &op.header_params {
        let snake = rust_ident_snake(&p.name);
        let key = string_literal(&p.name);
        let is_array = is_array_param(spec, &p.r#type);

        let value_expr_for_owned = if is_array {
            format!("{snake}.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(\",\")")
        } else {
            format!("{snake}.to_string()")
        };
        let value_expr_for_borrowed = if is_array {
            "v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(\",\")".to_string()
        } else {
            "v.to_string()".to_string()
        };

        if p.required {
            s.push_str(&format!(
                "        req = req.header({key}, {value_expr_for_owned});\n"
            ));
        } else {
            s.push_str(&format!(
                "        if let Some(v) = &{snake} {{ req = req.header({key}, {value_expr_for_borrowed}); }}\n"
            ));
        }
    }
    s
}

// -- Cookie assembly ------------------------------------------------------

pub fn render_cookie_assembly(spec: &ir::Ir, op: &ir::Operation) -> String {
    if op.cookie_params.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    s.push_str("        let mut _cookies: Vec<String> = Vec::new();\n");
    for p in &op.cookie_params {
        let snake = rust_ident_snake(&p.name);
        let key = string_literal(&p.name);
        let is_array = is_array_param(spec, &p.r#type);
        let value_for_owned = if is_array {
            format!("{snake}.iter().map(|v| _pe(v)).collect::<Vec<_>>().join(\",\")")
        } else {
            format!("_pe(&{snake})")
        };
        let value_for_borrowed = if is_array {
            "v.iter().map(|x| _pe(x)).collect::<Vec<_>>().join(\",\")".to_string()
        } else {
            "_pe(v)".to_string()
        };
        if p.required {
            s.push_str(&format!(
                "        _cookies.push(format!(\"{{}}={{}}\", {key}, {value_for_owned}));\n"
            ));
        } else {
            s.push_str(&format!(
                "        if let Some(v) = &{snake} {{ _cookies.push(format!(\"{{}}={{}}\", {key}, {value_for_borrowed})); }}\n"
            ));
        }
    }
    s.push_str("        if !_cookies.is_empty() {\n");
    s.push_str("            req = req.header(reqwest::header::COOKIE, _cookies.join(\"; \"));\n");
    s.push_str("        }\n");
    s
}

// -- Type-shape probes ----------------------------------------------------

fn is_array_param(spec: &ir::Ir, type_ref: &ir::TypeRef) -> bool {
    spec.types
        .iter()
        .find(|nt| &nt.id == type_ref)
        .is_some_and(|nt| matches!(&nt.definition, ir::TypeDef::Array(_)))
}

fn is_object_param(spec: &ir::Ir, type_ref: &ir::TypeRef) -> bool {
    spec.types
        .iter()
        .find(|nt| &nt.id == type_ref)
        .is_some_and(|nt| matches!(&nt.definition, ir::TypeDef::Object(_)))
}

fn resolve_object<'a>(spec: &'a ir::Ir, type_ref: &ir::TypeRef) -> Option<&'a ir::ObjectType> {
    spec.types
        .iter()
        .find(|nt| &nt.id == type_ref)
        .and_then(|nt| match &nt.definition {
            ir::TypeDef::Object(o) => Some(o),
            _ => None,
        })
}
