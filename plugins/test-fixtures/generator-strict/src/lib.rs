//! Test fixture generator. Rejects any IR containing a `multipart/*`
//! request body via `StageError::Rejected` with a structured diagnostic
//! pointing at the offending operation. Otherwise emits a one-line dummy
//! file. Used by `crates/forge-plugin-itests/tests/rejection.rs` to
//! exercise the plugin rejection pattern end-to-end across the WIT
//! boundary.

#![forbid(unsafe_code)]

use forge_plugin_sdk::convert::generator as conv;
use forge_plugin_sdk::generator::exports::forge::plugin::generator_api::{
    GenerationOutput as WitGenerationOutput, Guest,
};
use forge_plugin_sdk::generator::forge::plugin::stage::StageError;
use forge_plugin_sdk::generator::forge::plugin::types::{Ir as WitIr, PluginInfo as WitPluginInfo};
use forge_plugin_sdk::ir;
use forge_plugin_sdk::{GenerationOutput, OutputFile};

fn find_multipart(spec: &ir::Ir) -> Option<&ir::Operation> {
    spec.operations.iter().find(|op| {
        op.request_body
            .as_ref()
            .map(|b| b.content.iter().any(|c| c.media_type.starts_with("multipart/")))
            .unwrap_or(false)
    })
}

struct StrictFixture;

impl Guest for StrictFixture {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "generator-strict-fixture".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        r#"{"type":"object","additionalProperties":false}"#.into()
    }

    fn generate(spec: WitIr, _config: String) -> Result<WitGenerationOutput, StageError> {
        let canonical = conv::ir_from_wit(spec);

        if let Some(op) = find_multipart(&canonical) {
            let diag = ir::Diagnostic {
                severity: ir::Severity::Error,
                code: "generator-strict-fixture/E-MULTIPART".into(),
                message: format!(
                    "operation `{}` uses a multipart request body, which this generator does not support",
                    op.id
                ),
                location: Some(ir::SpecLocation::new(format!(
                    "/paths/{}/{}/requestBody",
                    op.path_template.replace('/', "~1"),
                    op.method.as_str(),
                ))),
                related: vec![],
                suggested_fix: Some(ir::FixSuggestion {
                    message: "remove the multipart body or pick a generator that handles multipart".into(),
                    edits: vec![],
                }),
            };
            return Err(conv::rejected(
                "spec uses multipart request bodies",
                vec![diag],
            ));
        }

        let body = format!(
            "# generator-strict-fixture\ntitle: {}\noperations: {}\n",
            canonical.info.title,
            canonical.operations.len()
        );
        Ok(conv::generation_output_to_wit(GenerationOutput {
            files: vec![OutputFile::text("strict.txt", body)],
            diagnostics: vec![],
        }))
    }
}


forge_plugin_sdk::generator::export!(StrictFixture with_types_in forge_plugin_sdk::generator);
