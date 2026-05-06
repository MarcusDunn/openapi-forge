//! Plugin author test harness.
//!
//! This is the *only* supported integration-test path for plugin authors.
//! See ADR-0004. The harness drives `cargo build` (when asked) and loads
//! the resulting `.wasm` through the same `forge-host` runtime that
//! production uses.
//!
//! # Recommended usage
//!
//! ```ignore
//! use forge_test_harness::PluginRunner;
//!
//! #[test]
//! fn drops_unwanted_operations() {
//!     let runner = PluginRunner::build_and_load(env!("CARGO_MANIFEST_DIR"))
//!         .unwrap();
//!     let out = runner
//!         .transform(fixture_ir(), serde_json::json!({"keep": ["users"]}))
//!         .unwrap();
//!     assert_eq!(out.spec.operations.len(), 2);
//! }
//! ```
//!
//! The first invocation performs `cargo build --release --target
//! wasm32-wasip2` for the plugin's manifest dir. Subsequent runs reuse
//! cargo's incremental cache, so the cycle is fast in practice.
//!
//! The example above is `ignore`d because doctests can't easily build a
//! `wasm32-wasip2` target on the fly. A smaller, executable doctest on
//! [`HarnessError`] verifies that the published surface is reachable.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use forge_host::{Engine, GenerationOutput, Limits, Plugin, StageError, TransformOutput};
use forge_ir::Ir;

/// Anything that can go wrong while building or loading a plugin.
///
/// ```
/// use forge_test_harness::HarnessError;
///
/// let err = HarnessError::Build("cargo exited 101".into());
/// assert!(format!("{err}").contains("cargo exited 101"));
/// ```
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("failed to build plugin: {0}")]
    Build(String),
    #[error("plugin io: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not locate built .wasm under {0:?}")]
    NotFound(PathBuf),
    #[error("engine init: {0}")]
    Engine(String),
    #[error("plugin load: {0}")]
    Load(String),
}

/// Loaded plugin handle, ready to invoke.
#[derive(Debug)]
pub struct PluginRunner {
    engine: Engine,
    plugin: Plugin,
}

impl PluginRunner {
    /// Build the plugin from its Cargo manifest dir, then load the
    /// resulting `.wasm`. Build invalidation is delegated to cargo.
    pub fn build_and_load(manifest_dir: impl AsRef<Path>) -> Result<Self, HarnessError> {
        let manifest_dir = manifest_dir.as_ref();
        build(manifest_dir)?;
        let wasm = locate_artifact(manifest_dir)?;
        Self::load(wasm)
    }

    /// Load an already-built `.wasm`. The harness inspects the component's
    /// exports and chooses transformer vs generator automatically.
    pub fn load(wasm_path: impl AsRef<Path>) -> Result<Self, HarnessError> {
        let bytes = std::fs::read(wasm_path.as_ref())?;
        let engine = Engine::new().map_err(|e| HarnessError::Engine(e.to_string()))?;
        match Plugin::load_transformer(&engine, &bytes) {
            Ok(p) => Ok(Self { engine, plugin: p }),
            Err(_) => {
                let p = Plugin::load_generator(&engine, &bytes)
                    .map_err(|e| HarnessError::Load(e.to_string()))?;
                Ok(Self { engine, plugin: p })
            }
        }
    }

    pub fn info(&self) -> &forge_ir::PluginInfo {
        self.plugin.info()
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn plugin(&self) -> &Plugin {
        &self.plugin
    }

    pub fn transform(
        &self,
        ir: Ir,
        config: serde_json::Value,
    ) -> Result<TransformOutput, StageError> {
        let s = config.to_string();
        self.plugin.transform(ir, &s, Limits::transformer())
    }

    pub fn generate(
        &self,
        ir: Ir,
        config: serde_json::Value,
    ) -> Result<GenerationOutput, StageError> {
        let s = config.to_string();
        self.plugin.generate(ir, &s, Limits::generator())
    }
}

fn build(manifest_dir: &Path) -> Result<(), HarnessError> {
    let manifest = manifest_dir.join("Cargo.toml");
    if !manifest.exists() {
        return Err(HarnessError::Build(format!(
            "no Cargo.toml at {}",
            manifest.display()
        )));
    }
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip2",
            "--manifest-path",
        ])
        .arg(&manifest)
        .status()
        .map_err(|e| HarnessError::Build(format!("spawn cargo: {e}")))?;
    if !status.success() {
        return Err(HarnessError::Build(format!(
            "cargo build exited with {status}"
        )));
    }
    Ok(())
}

fn locate_artifact(manifest_dir: &Path) -> Result<PathBuf, HarnessError> {
    let crate_name = read_crate_name(manifest_dir)?;
    let underscore = crate_name.replace('-', "_");
    let mut search = manifest_dir.to_path_buf();
    loop {
        let candidate = search
            .join("target")
            .join("wasm32-wasip2")
            .join("release")
            .join(format!("{underscore}.wasm"));
        if candidate.exists() {
            return Ok(candidate);
        }
        let Some(parent) = search.parent() else {
            return Err(HarnessError::NotFound(
                manifest_dir
                    .join("target/wasm32-wasip2/release")
                    .join(format!("{underscore}.wasm")),
            ));
        };
        search = parent.to_path_buf();
    }
}

fn read_crate_name(manifest_dir: &Path) -> Result<String, HarnessError> {
    let manifest = std::fs::read_to_string(manifest_dir.join("Cargo.toml"))?;
    // Tiny one-pass extraction of `name = "..."` from `[package]`. We don't
    // pull in `toml` for this — the harness is meant to be a low-friction
    // dependency.
    let mut in_package = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("name") {
                let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=');
                if let Some(name) = rest
                    .trim()
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                {
                    return Ok(name.to_string());
                }
            }
        }
    }
    Err(HarnessError::Build(format!(
        "no [package] name in {}",
        manifest_dir.join("Cargo.toml").display()
    )))
}
