//! Request body picking + assembly. Mirrors TS-fetch's body kinds:
//! JSON, urlencoded, multipart, octet-stream, text/*, and "other"
//! (passes the body through with an explicit Content-Type).

use forge_plugin_sdk::ir;

use crate::naming::rust_ident_snake;
use crate::types::render_type_ref;
use crate::util::string_literal;

#[derive(Debug, Clone)]
pub enum BodyPick<'a> {
    None,
    Json(&'a ir::TypeRef),
    UrlEncoded(&'a ir::TypeRef),
    Multipart {
        ty: &'a ir::TypeRef,
        encoding: &'a [(String, ir::Encoding)],
    },
    OctetStream,
    Text {
        media_type: &'a str,
    },
    Other {
        media_type: &'a str,
        ty: &'a ir::TypeRef,
    },
}

/// Pick the body content the operation should send. Preference order
/// matches `generator-typescript-fetch::pick_body`: JSON → urlencoded →
/// multipart → octet-stream → text/* → first declared.
pub fn pick_body(op: &ir::Operation) -> BodyPick<'_> {
    let Some(body) = &op.request_body else {
        return BodyPick::None;
    };
    let by_media = |needle: &str| {
        body.content
            .iter()
            .find(|c| c.media_type.starts_with(needle))
    };
    if let Some(c) = by_media("application/json") {
        return BodyPick::Json(&c.r#type);
    }
    if let Some(c) = by_media("application/x-www-form-urlencoded") {
        return BodyPick::UrlEncoded(&c.r#type);
    }
    if let Some(c) = by_media("multipart/") {
        return BodyPick::Multipart {
            ty: &c.r#type,
            encoding: &c.encoding,
        };
    }
    if by_media("application/octet-stream").is_some() {
        return BodyPick::OctetStream;
    }
    if let Some(c) = by_media("text/") {
        return BodyPick::Text {
            media_type: &c.media_type,
        };
    }
    if let Some(c) = body.content.first() {
        return BodyPick::Other {
            media_type: &c.media_type,
            ty: &c.r#type,
        };
    }
    BodyPick::None
}

/// `(parameter-name, parameter-type)` for the method signature, or
/// `None` if the operation has no body. The pick decides whether the
/// body is borrowed (`&T` for serde-shaped kinds), owned (multipart
/// consumes the value), or a primitive (`Vec<u8>` / `String`).
pub fn body_signature(spec: &ir::Ir, pick: &BodyPick<'_>) -> Option<(String, String)> {
    match pick {
        BodyPick::None => None,
        BodyPick::Json(ty) => Some(("body".into(), format!("&{}", render_type_ref(spec, ty)))),
        BodyPick::UrlEncoded(ty) => {
            Some(("body".into(), format!("&{}", render_type_ref(spec, ty))))
        }
        BodyPick::Multipart { ty, .. } => {
            Some(("body".into(), render_type_ref(spec, ty)))
        }
        BodyPick::OctetStream => Some(("body".into(), "Vec<u8>".into())),
        BodyPick::Text { .. } => Some(("body".into(), "String".into())),
        BodyPick::Other { ty, .. } => {
            Some(("body".into(), format!("&{}", render_type_ref(spec, ty))))
        }
    }
}

/// Render the request-builder mutation that attaches the body to `req`.
pub fn render_body_assembly(spec: &ir::Ir, pick: &BodyPick<'_>) -> String {
    match pick {
        BodyPick::None => String::new(),
        BodyPick::Json(_) => "        req = req.json(body);\n".into(),
        BodyPick::UrlEncoded(_) => {
            // reqwest's `.form` serializes via `serde_urlencoded` and sets
            // the Content-Type to `application/x-www-form-urlencoded`.
            "        req = req.form(body);\n".into()
        }
        BodyPick::Multipart { ty, encoding } => render_multipart_body(spec, ty, encoding),
        BodyPick::OctetStream => {
            "        req = req.header(reqwest::header::CONTENT_TYPE, \"application/octet-stream\").body(body);\n".into()
        }
        BodyPick::Text { media_type, .. } => format!(
            "        req = req.header(reqwest::header::CONTENT_TYPE, {ct}).body(body);\n",
            ct = string_literal(media_type)
        ),
        BodyPick::Other { media_type, .. } => format!(
            // Best-effort: serialize via JSON. Real handling for arbitrary
            // media types is parser/IR territory.
            "        req = req.header(reqwest::header::CONTENT_TYPE, {ct}).json(body);\n",
            ct = string_literal(media_type)
        ),
    }
}

/// Build a `reqwest::multipart::Form` by destructuring the body struct's
/// properties. Binary properties (`PrimitiveKind::Bytes` → `Vec<u8>`)
/// become `Part::bytes`; everything else becomes `Part::text` via
/// `Display`. Per-property Content-Type pulled from
/// `BodyContent.encoding[name].content_type` if set.
fn render_multipart_body(
    spec: &ir::Ir,
    ty: &ir::TypeRef,
    encoding: &[(String, ir::Encoding)],
) -> String {
    let mut s = String::new();
    s.push_str("        let mut _form = reqwest::multipart::Form::new();\n");

    let nt = match spec.types.iter().find(|nt| &nt.id == ty) {
        Some(n) => n,
        None => {
            s.push_str("        // multipart body type unresolved; sending empty form\n");
            s.push_str("        req = req.multipart(_form);\n");
            return s;
        }
    };
    let obj = match &nt.definition {
        ir::TypeDef::Object(o) => o,
        _ => {
            s.push_str("        // multipart body must be an object schema; sending empty form\n");
            s.push_str("        req = req.multipart(_form);\n");
            return s;
        }
    };

    for prop in &obj.properties {
        let raw_name = &prop.name;
        let field_ident = rust_ident_snake(raw_name);
        let is_required = prop.required;
        let part_ct: Option<&str> = encoding
            .iter()
            .find(|(k, _)| k == raw_name)
            .and_then(|(_, e)| e.content_type.as_deref());
        let is_bytes = is_property_bytes(spec, &prop.r#type);

        let part_expr = if is_bytes {
            // `Part::bytes` accepts `Into<Cow<'static, [u8]>>`, which
            // `Vec<u8>` satisfies directly. An explicit `.into()` would
            // ambiguate against the many other `From<Vec<u8>>` impls in
            // scope (`Bytes`, `rustls`'s key types, ...) and fail to
            // compile. Pass the value as-is and let inference do the
            // single conversion that targets the parameter bound.
            "reqwest::multipart::Part::bytes(value)".to_string()
        } else {
            "reqwest::multipart::Part::text(value.to_string())".to_string()
        };
        let part_with_ct = match part_ct {
            Some(ct) => format!(
                "{part_expr}.mime_str({ct}).expect(\"valid content-type\")",
                ct = string_literal(ct)
            ),
            None => part_expr,
        };
        let part_name = string_literal(raw_name);

        if is_required {
            // For required required-but-bytes-or-not, `body.<field>` is
            // owned (Multipart consumes the body).
            s.push_str(&format!(
                "        {{ let value = body.{field_ident}; _form = _form.part({part_name}, {part_with_ct}); }}\n"
            ));
        } else {
            // Optional → Option<T>. Skip if None.
            s.push_str(&format!(
                "        if let Some(value) = body.{field_ident} {{ _form = _form.part({part_name}, {part_with_ct}); }}\n"
            ));
        }
    }
    s.push_str("        req = req.multipart(_form);\n");
    s
}

/// True iff the property's resolved type is a binary-encoded string
/// (`format: byte` or `format: binary`). Per #105 the IR no longer has
/// a dedicated `PrimitiveKind::Bytes`; the binary signal lives on
/// `format_extension`. Used to decide between `Part::bytes` and
/// `Part::text` for multipart fields.
fn is_property_bytes(spec: &ir::Ir, type_ref: &ir::TypeRef) -> bool {
    let Some(nt) = spec.types.iter().find(|nt| &nt.id == type_ref) else {
        return false;
    };
    let ir::TypeDef::Primitive(p) = &nt.definition else {
        return false;
    };
    if !matches!(p.kind, ir::PrimitiveKind::String) {
        return false;
    }
    matches!(p.constraints.format_extension.as_deref(), Some("byte" | "binary"))
}

/// True iff the spec uses any multipart body. Drives the conditional
/// `multipart` feature on the generated `reqwest` dep (#43).
pub fn spec_uses_multipart(spec: &ir::Ir) -> bool {
    spec.operations
        .iter()
        .chain(spec.webhooks.iter().flat_map(|w| w.operations.iter()))
        .filter_map(|op| op.request_body.as_ref())
        .flat_map(|body| body.content.iter())
        .any(|c| c.media_type.starts_with("multipart/"))
}
