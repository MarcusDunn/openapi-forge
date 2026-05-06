//! Integration tests for the in-tree OpenAPI Forge plugins.
//!
//! The library is intentionally empty: every assertion lives in
//! `tests/<plugin>.rs` and routes through `forge_test_harness::PluginRunner`,
//! which builds the plugin as `wasm32-wasip2` and loads it through the same
//! `wasmtime` runtime the production host uses.
//!
//! See ADR-0004 (no native shim) and `docs/plugin-authoring.md` for why
//! plugin tests live here rather than inside the plugin crates themselves.
