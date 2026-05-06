//! `transformer-noop` — returns its input unchanged. Used to smoke-test the
//! WASM ABI end-to-end: an IR that survives a roundtrip through this plugin
//! has crossed the WIT boundary in both directions.
//!
//! Demonstrates the recommended SDK pattern: the [`Guest`] impl does the
//! WIT ↔ `forge_ir` plumbing via [`forge_plugin_sdk::convert::transformer`]
//! and delegates the real work to a pure function operating on canonical
//! [`forge_plugin_sdk::ir`] types.

#![forbid(unsafe_code)]

use forge_plugin_sdk::convert::transformer as conv;
use forge_plugin_sdk::ir;
use forge_plugin_sdk::transformer::exports::forge::plugin::transformer_api::{
    Guest, TransformOutput as WitTransformOutput,
};
use forge_plugin_sdk::transformer::forge::plugin::stage::StageError;
use forge_plugin_sdk::transformer::forge::plugin::types::{Ir as WitIr, PluginInfo as WitPluginInfo};

/// Pure entry point. Operates on `forge_ir::Ir` — testable natively, no
/// `wit_bindgen` types required.
fn transform(spec: ir::Ir) -> forge_plugin_sdk::TransformOutput {
    forge_plugin_sdk::TransformOutput {
        spec,
        diagnostics: vec![],
    }
}

struct NoopTransformer;

impl Guest for NoopTransformer {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "transformer-noop".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        // The plugin takes no config.
        r#"{"type":"object","additionalProperties":false}"#.into()
    }

    fn transform(spec: WitIr, _config: String) -> Result<WitTransformOutput, StageError> {
        let canonical = conv::ir_from_wit(spec);
        let out = transform(canonical);
        Ok(conv::transform_output_to_wit(out))
    }
}

forge_plugin_sdk::transformer::export!(NoopTransformer with_types_in forge_plugin_sdk::transformer);
