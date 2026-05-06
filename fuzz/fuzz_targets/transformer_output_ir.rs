//! Host-side fuzz target for adversarial transformer output.
//!
//! Threat model: a transformer plugin returns a WIT-typed `Ir` that
//! satisfies WIT's structural contract but violates IR semantic
//! invariants — dangling type refs, duplicate type ids, status-range
//! values outside `1..=5`, oversized strings, mixed valid/invalid
//! `discriminator.mapping` entries, etc. The host must not panic on any
//! such input; it is allowed to return `BindgenError`.
//!
//! The fuzz input is a JSON byte string that gets deserialized into
//! `forge_ir::Ir` (the same shape a transformer returns once the WIT
//! conversion has run). When deserialization fails the input is dropped
//! cheaply; libFuzzer's mutator quickly converges on shapes that
//! deserialize because the corpus is seeded from real
//! `fixtures/conformance/*/expected-ir.json` files.
//!
//! Once we have a Rust `Ir`, exercise the host code that consumes it:
//!
//!   1. `validate_refs` — advertised as the host's first line of
//!      defence after every transformer output (see
//!      `crates/forge-ir-bindgen/src/lib.rs:54`).
//!   2. Round-trip through `convert::transformer::{ir_to_wit, ir_from_wit}`
//!      — what the runtime does when handing the IR to the next stage
//!      (`crates/forge-host/src/runtime.rs:545,554`).
//!   3. Same for the generator world
//!      (`crates/forge-host/src/runtime.rs:588`).
//!
//! None of those calls may panic. Returning `Err(BindgenError)` is fine.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(ir) = serde_json::from_str::<forge_ir::Ir>(s) else {
        return;
    };

    // (1) Reference validator. Currently advertised but not yet wired into
    // the runtime pipeline — pre-stress it so adding the call site doesn't
    // ship a panic.
    let _ = forge_ir_bindgen::validate_refs(&ir);

    // (2) Transformer-world round-trip.
    let wit_t = forge_ir_bindgen::convert::transformer::ir_to_wit(ir.clone());
    let _ = forge_ir_bindgen::convert::transformer::ir_from_wit(wit_t);

    // (3) Generator-world round-trip. Same Rust shape but bindgen
    // produces nominally distinct types per world.
    let wit_g = forge_ir_bindgen::convert::generator::ir_to_wit(ir);
    let _ = forge_ir_bindgen::convert::generator::ir_from_wit(wit_g);
});
