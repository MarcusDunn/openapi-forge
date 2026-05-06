//! Shared helpers for plugin integration tests.
//!
//! Every test in `tests/<plugin>.rs` goes through
//! [`forge_test_harness::PluginRunner`]. The helper here just resolves a
//! plugin name to its manifest dir under `plugins/` and delegates to
//! `PluginRunner::build_and_load`. Cargo handles build invalidation, so
//! repeat invocations are cheap when the artifact is already up to date
//! (CI builds plugins as a separate step before running tests).
//!
//! The fixture loaders mirror the previous host-side helpers:
//! `petstore_ir()` returns the canonical Petstore IR from the parser's
//! conformance fixtures, and `ir_for(fixture)` loads any other named
//! conformance fixture's `expected-ir.json`.

use std::path::{Path, PathBuf};

use forge_ir::Ir;
use forge_test_harness::PluginRunner;

#[allow(dead_code)] // each test file uses a different subset of these helpers
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("canonicalize repo root")
}

/// Build (if needed) and load a plugin from `plugins/<name>/`.
#[allow(dead_code)]
pub fn runner_for(plugin: &str) -> PluginRunner {
    let manifest_dir = repo_root().join("plugins").join(plugin);
    PluginRunner::build_and_load(&manifest_dir)
        .unwrap_or_else(|e| panic!("load plugin `{plugin}`: {e}"))
}

#[allow(dead_code)]
pub fn petstore_ir() -> Ir {
    ir_for("petstore-minimal")
}

#[allow(dead_code)]
pub fn ir_for(fixture: &str) -> Ir {
    let path = repo_root()
        .join("fixtures/conformance")
        .join(fixture)
        .join("expected-ir.json");
    let s =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&s).expect("parse IR")
}
