//! Operations, parameters, request bodies, and responses.

use serde::{Deserialize, Serialize};

use crate::diagnostic::SpecLocation;
use crate::security::SecurityRequirement;
use crate::types::TypeRef;
use crate::value::ValueRef;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_id: Option<String>,
    pub method: HttpMethod,
    pub path_template: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_params: Vec<Parameter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_params: Vec<Parameter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_params: Vec<Parameter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cookie_params: Vec<Parameter>,
    /// OAS 3.2 `in: querystring` parameters — bind to the *entire*
    /// query string (opaque pass-through). Spec semantics imply at most
    /// one entry per operation; the parser warns rather than errors if
    /// a spec declares multiple, and the IR carries them as a list for
    /// uniformity with the other location buckets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub querystring_params: Vec<Parameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_body: Option<Body>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub responses: Vec<Response>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<SecurityRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// OAS §4.10 `summary` — short one-line label. PathItem-level
    /// `summary` fallback is applied at parse time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// OAS §4.10 `description` — long-form CommonMark. PathItem-level
    /// `description` fallback is applied at parse time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// OAS §4.10 `deprecated`.
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub deprecated: bool,
    /// Per-operation `externalDocs` (OAS §4.10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_docs: Option<crate::ExternalDocs>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
    /// Effective `servers` list for this operation. The parser
    /// resolves OAS §4.8.10 inheritance — operation-level entries win
    /// over path-item entries, which win over the root list — and
    /// materialises the result here. Empty if neither this operation,
    /// its path item, nor the root declared any servers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<crate::Server>,
    /// OAS Callback Objects — out-of-band requests the API makes back
    /// to the caller (event-driven / webhook APIs). Each entry pairs a
    /// callback name with one runtime expression (e.g.
    /// `{$request.body#/callbackUrl}`); a single callback name with
    /// multiple expressions becomes multiple entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callbacks: Vec<crate::Callback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<SpecLocation>,
}

/// HTTP method for an operation. The eight standard verbs plus an
/// `Other(String)` escape hatch that carries 3.2 `additionalOperations`
/// methods (e.g. RFC 9205 `QUERY`) verbatim. Generators that emit
/// against a fixed verb set should match the named variants and either
/// reject `Other` (with a `StageError::Rejected`) or pass the string
/// through to a lower-level builder API.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
    Get,
    Put,
    Post,
    Delete,
    Options,
    Head,
    Patch,
    Trace,
    /// Carries the upper-cased method name (`"QUERY"`, etc.).
    Other(String),
}

impl HttpMethod {
    /// Wire-form of the method (uppercased verb that goes on the
    /// request line). Use this when serialising to HTTP.
    pub fn as_str(&self) -> &str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Put => "PUT",
            HttpMethod::Post => "POST",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Head => "HEAD",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Trace => "TRACE",
            HttpMethod::Other(s) => s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: TypeRef,
    pub required: bool,
    /// OAS §4.12 `description` (CommonMark).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// OAS §4.12 `deprecated`.
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub deprecated: bool,
    /// OAS §4.12 `example` / `examples`. Named entries; 3.0 bare
    /// `example` lands under the synthetic key `"_default"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<(String, crate::Example)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ParameterStyle>,
    #[serde(default)]
    pub explode: bool,
    /// OAS `allowEmptyValue`. Permits `?foo=` with no value. Only legal
    /// on `in: query`; the parser warns and clears it for other
    /// locations.
    #[serde(default)]
    pub allow_empty_value: bool,
    /// OAS `allowReserved`. Permits raw RFC 3986 reserved chars in the
    /// rendered query string. Defaults to `false`.
    #[serde(default)]
    pub allow_reserved: bool,
    /// `x-*` extensions declared on the parameter. Compound extensions
    /// drop with `parser/W-EXTENSION-DROPPED`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<SpecLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParameterStyle {
    Form,
    Simple,
    Label,
    Matrix,
    SpaceDelimited,
    PipeDelimited,
    DeepObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub content: Vec<BodyContent>,
    pub required: bool,
    /// OAS §4.13 `description` (CommonMark).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `x-*` extensions declared on the request-body object. Compound
    /// extensions drop with `parser/W-EXTENSION-DROPPED`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyContent {
    pub media_type: String,
    #[serde(rename = "type")]
    pub r#type: TypeRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub encoding: Vec<(String, Encoding)>,
    /// OAS 3.2 `itemSchema`: per-item shape for sequence-of-items
    /// responses (JSON Lines, SSE event-stream, multipart/mixed).
    /// Mutually exclusive with `schema` in the spec; when present, the
    /// parser populates `type` with the same ref so generators that
    /// don't model streaming see a usable type. Streaming-aware
    /// generators read `item_schema` to know they should decode one
    /// record at a time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_schema: Option<TypeRef>,
    /// OAS §4.14 `example` / `examples`. The Media Type Object has no
    /// `description` or `summary` field per spec — body-level prose
    /// belongs on the surrounding RequestBody / Response.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<(String, crate::Example)>,
    /// `x-*` extensions declared on the media-type object. Compound
    /// extensions drop with `parser/W-EXTENSION-DROPPED`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Encoding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ParameterStyle>,
    #[serde(default)]
    pub explode: bool,
    /// OAS `allowReserved`. When `true`, RFC 3986 reserved characters
    /// (`:/?#[]@!$&'()*+,;=`) are passed through verbatim instead of
    /// being percent-encoded; meaningful for
    /// `application/x-www-form-urlencoded` parts. Defaults to `false`.
    #[serde(default)]
    pub allow_reserved: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, Header)>,
    /// `x-*` extensions declared on the encoding object. Compound
    /// extensions drop with `parser/W-EXTENSION-DROPPED`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
}

/// OAS Header Object. Distinct from [`Parameter`] because headers
/// have a fixed serialization style (no `style`/`explode`), and
/// because `required` carries different semantics here:
/// "documented as always present in the response" rather than "the
/// request must include this".
///
/// The header's name lives on the surrounding tuple key
/// (`Vec<(String, Header)>`), not on this struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    #[serde(rename = "type")]
    pub r#type: TypeRef,
    #[serde(default)]
    pub required: bool,
    /// OAS §4.21 `description` (CommonMark).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// OAS §4.21 `deprecated`.
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub deprecated: bool,
    /// OAS §4.21 `example` / `examples`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<(String, crate::Example)>,
    /// OAS Header Object inherits the Parameter Object's serialization
    /// fields. The spec fixes `style` to `simple` for headers, but
    /// captures the value verbatim so spec-strict consumers
    /// (validators, doc generators) can see what was declared. Most
    /// generators ignore this slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ParameterStyle>,
    /// OAS `explode`. Defaults to `false` per Header Object semantics.
    #[serde(default)]
    pub explode: bool,
    /// OAS `allowReserved`. Permits raw RFC 3986 reserved characters
    /// in the rendered header value. Defaults to `false`.
    #[serde(default)]
    pub allow_reserved: bool,
    /// OAS `allowEmptyValue`. Defaults to `false`.
    #[serde(default)]
    pub allow_empty_value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<SpecLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub status: ResponseStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<BodyContent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, Header)>,
    /// OAS 3.2 §4.17 `summary` — short label, new in 3.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// OAS §4.17 `description` (CommonMark). Required in 3.0 / 3.1;
    /// optional in 3.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// OAS `links` — HATEOAS follow-ups. Named entries; order is
    /// preserved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<(String, crate::Link)>,
    /// `x-*` extensions declared on the response object. Compound
    /// extensions drop with `parser/W-EXTENSION-DROPPED`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ResponseStatus {
    Explicit {
        code: u16,
    },
    Default,
    /// A status range from `1` (1xx) through `5` (5xx).
    Range {
        class: u8,
    },
}
