//! `validator-required-operation-id` — first reference validator plugin.
//!
//! Validators are degenerate transformers: they return their input
//! unchanged and emit diagnostics. This one walks every [`ir::Operation`]
//! and emits `validator-required-operation-id/E-MISSING-ID` for any
//! operation whose `original_id` is `None` (i.e. the spec omitted
//! `operationId`).
//!
//! In production today the parser already requires `operationId`
//! (`parser/E-MISSING-FIELD`), so this plugin will not catch anything
//! against a real spec. It exists to demonstrate the validator pattern
//! end-to-end for future style / security validators.

#![forbid(unsafe_code)]

use forge_plugin_sdk::convert::transformer as conv;
use forge_plugin_sdk::ir;
use forge_plugin_sdk::transformer::exports::forge::plugin::transformer_api::{
    Guest, TransformOutput as WitTransformOutput,
};
use forge_plugin_sdk::transformer::forge::plugin::stage::StageError;
use forge_plugin_sdk::transformer::forge::plugin::types::{
    Ir as WitIr, PluginInfo as WitPluginInfo,
};

const E_MISSING_ID: &str = "validator-required-operation-id/E-MISSING-ID";

/// Pure entry point. Operates on `forge_ir::Ir` so it's testable natively
/// without crossing the WIT boundary.
fn validate(spec: ir::Ir) -> forge_plugin_sdk::TransformOutput {
    let diagnostics = spec
        .operations
        .iter()
        .filter(|op| op.original_id.is_none())
        .map(|op| {
            let d = forge_plugin_sdk::diag::error(
                E_MISSING_ID,
                format!(
                    "operation `{}` has no `operationId` declared in the spec",
                    op.id
                ),
            );
            match op.location.clone() {
                Some(loc) => forge_plugin_sdk::diag::at(d, loc),
                None => d,
            }
        })
        .collect();

    forge_plugin_sdk::TransformOutput { spec, diagnostics }
}

struct ValidatorRequiredOperationId;

impl Guest for ValidatorRequiredOperationId {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "validator-required-operation-id".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        // The plugin takes no config.
        r#"{"type":"object","additionalProperties":false}"#.into()
    }

    fn transform(spec: WitIr, _config: String) -> Result<WitTransformOutput, StageError> {
        let canonical = conv::ir_from_wit(spec);
        let out = validate(canonical);
        Ok(conv::transform_output_to_wit(out))
    }
}

forge_plugin_sdk::transformer::export!(ValidatorRequiredOperationId with_types_in forge_plugin_sdk::transformer);
