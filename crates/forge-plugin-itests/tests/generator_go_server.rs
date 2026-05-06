//! Reference cross-language plugin smoke test (issue #58).
//!
//! Builds `plugins/generator-go-server/` via `build.sh` (TinyGo +
//! wit-bindgen-go), loads the resulting `.wasm` through the same
//! `forge-host` runtime production uses, and asserts the generated Go
//! server scaffold compiles via `go build`.
//!
//! Gated behind the `go-server` feature so the default `cargo test` skips
//! the Go toolchain. CI runs this through the dedicated `plugin-go-server`
//! job; see `.github/workflows/ci.yml`.

#![cfg(feature = "go-server")]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use common::{petstore_ir, repo_root};
use forge_test_harness::PluginRunner;

/// Run `build.sh` exactly once per test process. Tests run in parallel so
/// without serialization they would race on the staged `wit/` directory.
fn ensure_built() -> PathBuf {
    static ONCE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    let cell = ONCE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap();
    if let Some(p) = guard.as_ref() {
        return p.clone();
    }
    let plugin_dir = repo_root().join("plugins/generator-go-server");
    let status = Command::new("bash")
        .arg(plugin_dir.join("build.sh"))
        .status()
        .unwrap_or_else(|e| panic!("spawn build.sh: {e}"));
    assert!(status.success(), "build.sh failed (status {status:?})");
    let wasm = plugin_dir.join("plugin.wasm");
    *guard = Some(wasm.clone());
    wasm
}

/// Load the built plugin through the same host runtime production uses.
/// Each test gets its own `PluginRunner` (cheap — no rebuild); the
/// component bytes are loaded into a fresh `wasmtime::Engine` per test so
/// state doesn't leak between cases.
fn build_and_load_go_plugin() -> PluginRunner {
    let wasm = ensure_built();
    PluginRunner::load(&wasm).unwrap_or_else(|e| panic!("load {}: {e}", wasm.display()))
}

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(bin))
            .find(|p| p.is_file())
    })
}

fn write_files(out: &forge_host::GenerationOutput, dir: &Path) {
    for f in &out.files {
        let target = dir.join(&f.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&target, &f.content).unwrap();
    }
}

#[test]
fn info_round_trip() {
    let runner = build_and_load_go_plugin();
    let info = runner.info();
    assert_eq!(info.name, "generator-go-server");
    assert_eq!(info.version, "0.1.0");
}

#[test]
fn generates_petstore_files() {
    let runner = build_and_load_go_plugin();
    let out = runner
        .generate(
            petstore_ir(),
            serde_json::json!({"module_path": "example.com/petstore"}),
        )
        .expect("generate");

    let paths: Vec<_> = out.files.iter().map(|f| f.path.clone()).collect();
    assert!(paths.contains(&"go.mod".to_string()), "got: {paths:?}");
    assert!(
        paths.contains(&"petstore/server.go".to_string()),
        "got: {paths:?}"
    );

    let server = out
        .files
        .iter()
        .find(|f| f.path == "petstore/server.go")
        .expect("server.go present");
    let body = std::str::from_utf8(&server.content).expect("utf-8");

    // PascalCase'd operation ids.
    assert!(body.contains("CreatePet"), "missing CreatePet");
    assert!(body.contains("ListPets"), "missing ListPets");
    assert!(body.contains("ShowPetById"), "missing ShowPetById");

    // Path templates routed through Go 1.22 method+pattern syntax.
    assert!(body.contains("\"POST /pets\""), "missing POST /pets");
    assert!(body.contains("\"GET /pets\""), "missing GET /pets");
    assert!(
        body.contains("\"GET /pets/{petId}\""),
        "missing GET /pets/{{petId}}"
    );

    // Path-param decode passes through the *original* OpenAPI name as the
    // PathValue key, not the Go-sanitized identifier.
    assert!(
        body.contains("r.PathValue(\"petId\")"),
        "missing r.PathValue(\"petId\") — handler isn't using the original param name"
    );

    // No diagnostics from the happy path.
    assert!(
        out.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        out.diagnostics
    );
}

#[test]
fn generated_petstore_compiles_with_go_build() {
    let runner = build_and_load_go_plugin();
    let out = runner
        .generate(
            petstore_ir(),
            serde_json::json!({"module_path": "example.com/petstore"}),
        )
        .expect("generate");

    let dir = tempfile::tempdir().expect("tempdir");
    write_files(&out, dir.path());

    if which("go").is_none() {
        eprintln!("skipping go build: `go` not on PATH");
        return;
    }

    // `go build ./...` requires GOCACHE writable. Tempdir is fine.
    let cache = dir.path().join(".gocache");
    std::fs::create_dir_all(&cache).unwrap();
    let status = Command::new("go")
        .args(["build", "./..."])
        .current_dir(dir.path())
        .env("GOCACHE", &cache)
        .env("GOFLAGS", "-mod=mod")
        .status()
        .expect("spawn go");
    assert!(status.success(), "go build failed (status {status:?})");
}

#[test]
fn rejects_missing_module_path() {
    let runner = build_and_load_go_plugin();
    let err = runner
        .generate(petstore_ir(), serde_json::json!({}))
        .expect_err("expected config-invalid for missing module_path");
    let msg = format!("{err:?}");
    assert!(
        msg.to_lowercase().contains("module_path"),
        "expected config error mentioning module_path, got: {msg}"
    );
}
