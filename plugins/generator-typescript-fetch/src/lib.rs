//! `generator-typescript-fetch` — emits a minimal-runtime, fetch-based
//! TypeScript client for an IR produced by Stage 3's parser.
//!
//! Recommended SDK pattern: the [`Guest`] impl converts WIT input to
//! `forge_ir` via [`forge_plugin_sdk::convert::generator`] and delegates to
//! pure modules that operate on canonical IR types. Pure logic
//! (`naming`, `types`, `operations`, `emit`) builds natively under
//! `cargo test`; only this entry point depends on `wit_bindgen`.

#![forbid(unsafe_code)]

mod emit;
mod naming;
mod operations;
mod runtime;
mod types;

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
    package_name: Option<String>,
    base_url: Option<String>,
}

/// Pure entry point. Operates on `forge_ir::Ir` and returns the SDK's
/// world-independent `GenerationOutput`.
pub fn generate(spec: &ir::Ir, cfg: &emit::Config) -> forge_plugin_sdk::GenerationOutput {
    emit::all(spec, cfg)
}

struct TsFetch;

impl Guest for TsFetch {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "generator-typescript-fetch".into(),
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
            package_name: raw.package_name.unwrap_or_else(|| "api-client".into()),
            base_url: raw.base_url,
        };
        let canonical = conv::ir_from_wit(spec);
        let out = generate(&canonical, &cfg);
        Ok(conv::generation_output_to_wit(out))
    }
}

forge_plugin_sdk::generator::export!(TsFetch with_types_in forge_plugin_sdk::generator);
