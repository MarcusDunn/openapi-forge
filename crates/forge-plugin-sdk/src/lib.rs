//! OpenAPI Forge plugin author SDK.
//!
//! WASM-only. There is **no** native shim and **no** native test path. See
//! ADR-0004. Plugin authors test through `forge-test-harness`, which runs
//! plugins through the same `wasmtime`-based runtime the host uses in
//! production.
//!
//! # Choosing a world
//!
//! A plugin implements **one** world. Pick the matching feature in your
//! plugin's `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! forge-plugin-sdk = { workspace = true, features = ["transformer"] }
//! # or:
//! forge-plugin-sdk = { workspace = true, features = ["generator"] }
//! ```
//!
//! Both features at once is rejected at compile time: a component implements
//! one world.
//!
//! # Recommended structure
//!
//! Factor pure logic into modules (`naming`, `templates`, etc.) that build
//! natively under `cargo test`, and keep the WASM-boundary entry point
//! thin. See `docs/plugin-authoring.md`.
//!
//! # Doctests
//!
//! All doctests in this crate are marked `ignore`. The `compile_error!`
//! guard fires for any non-`wasm32` target, which includes the host where
//! `cargo test --doc` runs. The examples are still rendered by `cargo doc`
//! for plugin-author reference.

#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
compile_error!(
    "forge-plugin-sdk only supports wasm32-wasip2. There is no native shim by design \
     (see ADR-0004). Plugin integration tests must use forge-test-harness."
);

// Cargo's workspace-level feature unification pulls in both features when
// the workspace contains plugins for both worlds. We tolerate that: the two
// `wit_bindgen::generate!` invocations live in separate modules and produce
// independent type trees, so having both enabled is harmless. A given plugin
// implements one world's `Guest` and calls one world's `export!`; the unused
// module is dead code that the linker drops.
#[cfg(not(any(feature = "transformer", feature = "generator")))]
compile_error!(
    "forge-plugin-sdk: enable either the `transformer` or `generator` feature \
     to select a world."
);

pub use forge_ir as ir;

/// Re-export `serde_json` so plugin authors don't need a direct dep just to
/// deserialize their config string.
pub use serde_json;

mod convert_impl;
pub mod output;
pub mod types_ext;
pub mod values_ext;
pub use output::{FileMode, GenerationOutput, OutputFile, TransformOutput};
pub use types_ext::{is_canonical_null_id, is_null_typeref, peel_nullable, union_has_null};
pub use values_ext::{resolve, resolve_to_serde, to_json_compact, to_json_pretty};

pub mod config {
    //! Helpers for parsing the JSON config string the host hands to a plugin.

    #[derive(Debug, thiserror::Error)]
    pub enum ConfigError {
        #[error("invalid JSON: {0}")]
        InvalidJson(String),
        #[error("config did not match plugin's declared schema: {0}")]
        SchemaMismatch(String),
    }

    pub fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, ConfigError> {
        serde_json::from_str(s).map_err(|e| ConfigError::InvalidJson(e.to_string()))
    }
}

pub mod diag {
    //! Diagnostic builders. Operate on [`forge_ir::Diagnostic`]; see
    //! [`crate::convert`] to translate to the WIT shape the macros expect.
    use crate::ir::{Diagnostic, Severity, SpecLocation};

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            location: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            location: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity: Severity::Info,
            code: code.into(),
            message: message.into(),
            location: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    pub fn hint(code: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity: Severity::Hint,
            code: code.into(),
            message: message.into(),
            location: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    pub fn at(mut d: Diagnostic, location: SpecLocation) -> Diagnostic {
        d.location = Some(location);
        d
    }
}

// -----------------------------------------------------------------------------
// World bindings — generated at compile time by wit-bindgen.
// -----------------------------------------------------------------------------

#[cfg(feature = "transformer")]
pub mod transformer {
    //! Transformer-world bindings. Plugin authors implement [`Guest`] and
    //! call [`export!`] with their type.
    wit_bindgen::generate!({
        world: "ir-transformer",
        path: "wit",
        pub_export_macro: true,
        export_macro_name: "export",
    });
}

#[cfg(feature = "generator")]
pub mod generator {
    //! Generator-world bindings. Plugin authors implement [`Guest`] and
    //! call [`export!`] with their type.
    wit_bindgen::generate!({
        world: "code-generator",
        path: "wit",
        pub_export_macro: true,
        export_macro_name: "export",
    });
}

// -----------------------------------------------------------------------------
// Convert — between forge_ir types and the wit-bindgen-generated types.
// -----------------------------------------------------------------------------

/// Conversions between [`forge_ir`] / [`crate::output`] and the
/// `wit_bindgen`-generated WIT types.
///
/// A plugin can author its logic against the canonical [`forge_ir`] types
/// plus the SDK's [`crate::output::GenerationOutput`] /
/// [`crate::output::TransformOutput`] and rely on `convert::*` to bridge to
/// the WIT boundary. Both directions are supported:
///
/// - `ir_from_wit(wit_ir) -> forge_ir::Ir` to receive the spec.
/// - `generation_output_to_wit(out)` / `transform_output_to_wit(out)` to
///   return the result.
///
/// This is a near-identical mirror of the host's `forge_ir_bindgen::convert`.
/// Pre-1.0 the two are kept in sync by hand; the proptest roundtrip in the
/// host is the authority.
pub mod convert {
    #[cfg(feature = "transformer")]
    pub mod transformer {
        //! Conversions for the transformer world.
        crate::__impl_transformer_world!(
            crate::transformer::forge::plugin::types,
            crate::transformer::exports::forge::plugin::transformer_api,
            crate::transformer::forge::plugin::stage
        );
    }

    #[cfg(feature = "generator")]
    pub mod generator {
        //! Conversions for the generator world.
        crate::__impl_generator_world!(
            crate::generator::forge::plugin::types,
            crate::generator::exports::forge::plugin::generator_api,
            crate::generator::forge::plugin::stage
        );
    }
}
