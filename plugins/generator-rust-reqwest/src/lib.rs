//! `generator-rust-reqwest` — emits an async-`reqwest` Rust client crate
//! for an IR produced by the parser.
//!
//! Mirrors the SDK pattern from `generator-typescript-fetch`: the
//! [`Guest`] impl converts WIT input to canonical IR via
//! [`forge_plugin_sdk::convert::generator`] and delegates to the pure
//! `emit` module. See ADR-0004; tests live in
//! `crates/forge-plugin-itests/tests/generator_rust_reqwest.rs`.

#![forbid(unsafe_code)]

mod bodies;
mod emit;
mod naming;
mod operations;
mod params;
mod types;
mod util;

use forge_plugin_sdk::convert::generator as conv;
use forge_plugin_sdk::generator::exports::forge::plugin::generator_api::{
    GenerationOutput as WitGenerationOutput, Guest,
};
use forge_plugin_sdk::generator::forge::plugin::stage::StageError;
use forge_plugin_sdk::generator::forge::plugin::types::{Ir as WitIr, PluginInfo as WitPluginInfo};
use forge_plugin_sdk::ir;

#[derive(Debug, serde::Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ConfigInput {
    crate_name: Option<String>,
    base_url: Option<String>,
}

/// Pure entry point. Operates on `forge_ir::Ir` and returns the SDK's
/// world-independent `GenerationOutput`.
pub fn generate(spec: &ir::Ir, cfg: &emit::Config) -> forge_plugin_sdk::GenerationOutput {
    emit::all(spec, cfg)
}

struct RustReqwest;

impl Guest for RustReqwest {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "generator-rust-reqwest".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        include_str!("../schema.json").into()
    }

    fn generate(spec: WitIr, config: String) -> Result<WitGenerationOutput, StageError> {
        let raw: ConfigInput = if config.trim().is_empty() {
            ConfigInput::default()
        } else {
            forge_plugin_sdk::serde_json::from_str(&config)
                .map_err(|e| conv::config_invalid(e.to_string()))?
        };
        let cfg = emit::Config {
            crate_name: raw.crate_name.unwrap_or_else(|| "api-client".into()),
            base_url: raw.base_url,
        };
        let canonical = conv::ir_from_wit(spec);
        let out = generate(&canonical, &cfg);
        Ok(conv::generation_output_to_wit(out))
    }
}

forge_plugin_sdk::generator::export!(RustReqwest with_types_in forge_plugin_sdk::generator);
