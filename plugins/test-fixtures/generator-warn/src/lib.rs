//! Test fixture generator. Accepts any IR; emits a `Severity::Warning`
//! diagnostic for each operation that uses a `multipart/*` request body
//! (so generation succeeds but the user is told the plugin is dropping
//! information). Used by `crates/forge-plugin-itests/tests/rejection.rs`
//! to exercise the soft-warn pattern.

#![forbid(unsafe_code)]

use forge_plugin_sdk::convert::generator as conv;
use forge_plugin_sdk::generator::exports::forge::plugin::generator_api::{
    GenerationOutput as WitGenerationOutput, Guest,
};
use forge_plugin_sdk::generator::forge::plugin::stage::StageError;
use forge_plugin_sdk::generator::forge::plugin::types::{Ir as WitIr, PluginInfo as WitPluginInfo};
use forge_plugin_sdk::ir;
use forge_plugin_sdk::{GenerationOutput, OutputFile};

fn warn_for(op: &ir::Operation) -> Option<ir::Diagnostic> {
    let body = op.request_body.as_ref()?;
    if !body.content.iter().any(|c| c.media_type.starts_with("multipart/")) {
        return None;
    }
    Some(ir::Diagnostic {
        severity: ir::Severity::Warning,
        code: "generator-warn-fixture/W-MULTIPART".into(),
        message: format!(
            "operation `{}` uses a multipart body; emitted output ignores form structure",
            op.id
        ),
        location: Some(ir::SpecLocation::new(format!(
            "/paths/{}/{}/requestBody",
            op.path_template.replace('/', "~1"),
            op.method.as_str().to_lowercase(),
        ))),
        related: vec![],
        suggested_fix: None,
    })
}

struct WarnFixture;

impl Guest for WarnFixture {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "generator-warn-fixture".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        r#"{"type":"object","additionalProperties":false}"#.into()
    }

    fn generate(spec: WitIr, _config: String) -> Result<WitGenerationOutput, StageError> {
        let canonical = conv::ir_from_wit(spec);
        let diagnostics: Vec<_> = canonical.operations.iter().filter_map(warn_for).collect();
        let body = format!(
            "# generator-warn-fixture\ntitle: {}\noperations: {}\n",
            canonical.info.title,
            canonical.operations.len()
        );
        Ok(conv::generation_output_to_wit(GenerationOutput {
            files: vec![OutputFile::text("warn.txt", body)],
            diagnostics,
        }))
    }
}

forge_plugin_sdk::generator::export!(WarnFixture with_types_in forge_plugin_sdk::generator);
