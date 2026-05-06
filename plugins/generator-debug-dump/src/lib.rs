//! `generator-debug-dump` — emits a one-file textual summary of the IR.
//!
//! Demonstrates the recommended SDK pattern: the [`Guest`] impl converts
//! WIT types to `forge_ir` via [`forge_plugin_sdk::convert::generator`],
//! and the formatting code below operates only on canonical
//! [`forge_plugin_sdk::ir`] types — fully testable without `wit_bindgen`.

#![forbid(unsafe_code)]

use forge_plugin_sdk::convert::generator as conv;
use forge_plugin_sdk::generator::exports::forge::plugin::generator_api::{
    GenerationOutput as WitGenerationOutput, Guest,
};
use forge_plugin_sdk::generator::forge::plugin::stage::StageError;
use forge_plugin_sdk::generator::forge::plugin::types::{Ir as WitIr, PluginInfo as WitPluginInfo};
use forge_plugin_sdk::ir;
use forge_plugin_sdk::{GenerationOutput, OutputFile};

/// Pure entry point. Operates on `forge_ir::Ir`.
fn generate(spec: &ir::Ir) -> GenerationOutput {
    let mut body = String::new();
    body.push_str("# OpenAPI Forge — IR debug dump\n\n");
    body.push_str(&format!("title:    {}\n", spec.info.title));
    body.push_str(&format!("version:  {}\n", spec.info.version));
    if let Some(d) = &spec.info.description {
        body.push_str(&format!("description: {d}\n"));
    }
    body.push_str(&format!("\noperations ({}):\n", spec.operations.len()));
    for op in &spec.operations {
        body.push_str(&format!(
            "  - {} {} → {}\n",
            op.method.as_str(),
            op.path_template,
            op.id,
        ));
    }
    body.push_str(&format!("\ntypes ({}):\n", spec.types.len()));
    for t in &spec.types {
        body.push_str(&format!("  - {}\n", t.id));
    }

    GenerationOutput {
        files: vec![OutputFile::text("ir.txt", body)],
        diagnostics: vec![],
    }
}

struct DebugDump;

impl Guest for DebugDump {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "generator-debug-dump".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        r#"{"type":"object","additionalProperties":false}"#.into()
    }

    fn generate(spec: WitIr, _config: String) -> Result<WitGenerationOutput, StageError> {
        let canonical = conv::ir_from_wit(spec);
        let out = generate(&canonical);
        Ok(conv::generation_output_to_wit(out))
    }
}

forge_plugin_sdk::generator::export!(DebugDump with_types_in forge_plugin_sdk::generator);
