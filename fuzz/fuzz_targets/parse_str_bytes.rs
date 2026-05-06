//! Byte-level fuzz target for `forge_parser::parse_str`.
//!
//! Treats the input as opaque bytes, decodes as UTF-8 (skipping non-UTF-8
//! inputs — `parse_str` takes `&str`), and feeds the result to the parser.
//! Catches panics in the JSON-decode boundary and trivial structural
//! rejections (`ParseError::{InvalidJson,Empty,NotObject}` are valid; a
//! panic, OOM, or hang is not).
//!
//! Random bytes rarely produce input that survives `serde_json::from_str`
//! plus the root-object check (`crates/forge-parser/src/lib.rs:118-121`).
//! For coverage of the deeper schema/ref/finalize walking code, see the
//! `parse_str_structured` target.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = forge_parser::parse_str(s);
    }
});
