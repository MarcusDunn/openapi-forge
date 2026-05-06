//! Structured fuzz target for `forge_parser::parse_str`.
//!
//! Builds a depth-bounded `serde_json::Value` from the fuzzer's input
//! stream, biased toward keys the parser actually inspects (`paths`,
//! `components`, `$ref`, `allOf`, `discriminator`, …). Serializes it and
//! feeds the JSON string to `parse_str`.
//!
//! Random bytes almost never satisfy `serde_json::from_str` followed by
//! the root-object check (`crates/forge-parser/src/lib.rs:118-121`), so the
//! `parse_str_bytes` target rarely reaches the parser's deeper walking
//! code. This target gets there immediately.

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use serde_json::{Map, Number, Value};

/// Maximum nesting depth. Six is enough to express `paths.{path}.get.responses.200.content.{mime}.schema`.
const MAX_DEPTH: u32 = 6;

/// Maximum elements per object/array. Keeps single-input cost bounded.
const MAX_BREADTH: usize = 6;

/// Keys the parser cares about. Heavy bias toward these makes time-to-
/// coverage of `crates/forge-parser/src/{schema,refs,operations,security,finalize}.rs`
/// dramatically faster than uniform random key strings.
const OPENAPI_KEYS: &[&str] = &[
    // top level
    "openapi",
    "info",
    "paths",
    "components",
    "servers",
    "security",
    "tags",
    "webhooks",
    // info
    "title",
    "version",
    "description",
    // paths / operations
    "get",
    "put",
    "post",
    "delete",
    "patch",
    "options",
    "head",
    "trace",
    "operationId",
    "summary",
    "deprecated",
    "parameters",
    "requestBody",
    "responses",
    "content",
    "default",
    // params
    "name",
    "in",
    "required",
    "style",
    "explode",
    "allowReserved",
    "schema",
    // schema
    "$ref",
    "type",
    "format",
    "properties",
    "items",
    "enum",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "discriminator",
    "mapping",
    "additionalProperties",
    "nullable",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "pattern",
    "example",
    "examples",
    "readOnly",
    "writeOnly",
    "xml",
    // components
    "schemas",
    "securitySchemes",
    // security schemes
    "scheme",
    "bearerFormat",
    "flows",
    "scopes",
    "openIdConnectUrl",
    "tokenUrl",
    "authorizationUrl",
    "refreshUrl",
    // servers
    "url",
    "variables",
    // misc
    "200",
    "201",
    "204",
    "400",
    "401",
    "404",
    "500",
    "application/json",
    "application/xml",
    "multipart/form-data",
];

/// Strings that frequently appear as schema-leaf values.
const COMMON_STRINGS: &[&str] = &[
    "string",
    "integer",
    "number",
    "boolean",
    "object",
    "array",
    "null",
    "int32",
    "int64",
    "float",
    "double",
    "date",
    "date-time",
    "uuid",
    "uri",
    "email",
    "byte",
    "binary",
    "password",
    "header",
    "query",
    "path",
    "cookie",
    "form",
    "simple",
    "deepObject",
    "spaceDelimited",
    "pipeDelimited",
    "true",
    "false",
];

/// $ref targets — many will dangle (good: exercises `E_DANGLING_REF`); some
/// will be cyclic when combined with definitions emitted elsewhere.
const REF_TARGETS: &[&str] = &[
    "#/components/schemas/A",
    "#/components/schemas/B",
    "#/components/schemas/Cycle",
    "#/components/parameters/Page",
    "#/components/responses/Default",
    "#/components/securitySchemes/Bearer",
    "#/paths/~1users/get",
    "external.json#/components/schemas/X",
    "#/does/not/exist",
];

/// Top-level wrapper so `fuzz_target!` can deserialize a single value via
/// `Arbitrary` and we can hand-write the recursive impl.
#[derive(Debug)]
struct ArbDoc(Value);

/// Versions the parser accepts at `check_version` (lib.rs:238). Picking
/// one at the top level is the difference between hitting `parse_info`
/// onwards (the bulk of the parser) and bailing out after the version
/// gate. The byte-level target still covers the rejected-version path.
const VALID_VERSIONS: &[&str] = &["3.0.0", "3.0.3", "3.1.0", "3.2.0"];

impl<'a> Arbitrary<'a> for ArbDoc {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Always emit a top-level object with a valid `openapi` version —
        // the parser rejects non-objects at lib.rs:118-121 and bails out
        // past `check_version` if the version is missing or unsupported,
        // so for structural coverage we force both gates open. The
        // byte-level target covers the reject paths.
        let mut map = arb_object_inner(u, 0)?;
        let version = u.choose(VALID_VERSIONS)?;
        map.insert("openapi".to_string(), Value::String((*version).to_string()));
        Ok(ArbDoc(Value::Object(map)))
    }
}

fn arb_value(u: &mut Unstructured<'_>, depth: u32) -> arbitrary::Result<Value> {
    if depth >= MAX_DEPTH {
        return arb_leaf(u);
    }
    // Weighted choice. Objects/arrays slightly less likely as depth grows
    // so single-input cost stays bounded even when the fuzzer happens upon
    // deep schemas.
    let pick = u.int_in_range(0u8..=10)?;
    match pick {
        0 => Ok(Value::Null),
        1 => Ok(Value::Bool(<bool as Arbitrary>::arbitrary(u)?)),
        2 => arb_number(u),
        3..=4 => arb_string_value(u),
        5..=8 => Ok(Value::Object(arb_object_inner(u, depth + 1)?)),
        _ => arb_array_value(u, depth + 1),
    }
}

fn arb_leaf(u: &mut Unstructured<'_>) -> arbitrary::Result<Value> {
    match u.int_in_range(0u8..=4)? {
        0 => Ok(Value::Null),
        1 => Ok(Value::Bool(<bool as Arbitrary>::arbitrary(u)?)),
        2 => arb_number(u),
        _ => arb_string_value(u),
    }
}

fn arb_number(u: &mut Unstructured<'_>) -> arbitrary::Result<Value> {
    // `serde_json::Number` rejects NaN/Infinity, so we route via i64 and
    // fall back to a small finite f64.
    if <bool as Arbitrary>::arbitrary(u)? {
        let n: i64 = <i64 as Arbitrary>::arbitrary(u)?;
        Ok(Value::Number(Number::from(n)))
    } else {
        let raw: i32 = <i32 as Arbitrary>::arbitrary(u)?;
        let f = (raw as f64) / 1024.0;
        Number::from_f64(f).map(Value::Number).ok_or(arbitrary::Error::IncorrectFormat)
    }
}

fn arb_string_value(u: &mut Unstructured<'_>) -> arbitrary::Result<Value> {
    Ok(Value::String(arb_string(u)?))
}

fn arb_string(u: &mut Unstructured<'_>) -> arbitrary::Result<String> {
    // 60% common, 30% ref target, 10% arbitrary. Adjust if coverage stalls.
    let pick = u.int_in_range(0u8..=9)?;
    if pick < 6 {
        let s = u.choose(COMMON_STRINGS)?;
        Ok((*s).to_string())
    } else if pick < 9 {
        let s = u.choose(REF_TARGETS)?;
        Ok((*s).to_string())
    } else {
        let s: &str = <&str as Arbitrary>::arbitrary(u)?;
        Ok(s.to_string())
    }
}

fn arb_key(u: &mut Unstructured<'_>) -> arbitrary::Result<String> {
    // 90% biased keys; 10% random. The 10% lets the fuzzer find issues in
    // unknown-key handling (extensions, typos, etc.).
    if u.int_in_range(0u8..=9)? < 9 {
        let s = u.choose(OPENAPI_KEYS)?;
        Ok((*s).to_string())
    } else {
        let s: &str = <&str as Arbitrary>::arbitrary(u)?;
        Ok(s.to_string())
    }
}

fn arb_object_inner(u: &mut Unstructured<'_>, depth: u32) -> arbitrary::Result<Map<String, Value>> {
    let n = u.int_in_range(0..=MAX_BREADTH)?;
    let mut map = Map::with_capacity(n);
    for _ in 0..n {
        let k = arb_key(u)?;
        let v = arb_value(u, depth)?;
        map.insert(k, v);
    }
    Ok(map)
}

fn arb_array_value(u: &mut Unstructured<'_>, depth: u32) -> arbitrary::Result<Value> {
    let n = u.int_in_range(0..=MAX_BREADTH)?;
    let mut arr = Vec::with_capacity(n);
    for _ in 0..n {
        arr.push(arb_value(u, depth)?);
    }
    Ok(Value::Array(arr))
}

fuzz_target!(|doc: ArbDoc| {
    // `serde_json::to_string` on a finite, well-formed `Value` cannot fail,
    // but stay defensive — a panic here would mask parser panics.
    let Ok(s) = serde_json::to_string(&doc.0) else {
        return;
    };
    let _ = forge_parser::parse_str(&s);
});
