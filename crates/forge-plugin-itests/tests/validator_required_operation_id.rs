//! `validator-required-operation-id` integration test.
//!
//! Routes through `forge_test_harness::PluginRunner` rather than raw
//! `Plugin::load_*` so plugin authors can copy this exact pattern. See
//! ADR-0004 and `docs/plugin-authoring.md`.

mod common;

use common::runner_for;
use forge_ir::{ApiInfo, HttpMethod, Ir, Operation, Severity};

fn op(id: &str, original_id: Option<&str>) -> Operation {
    Operation {
        id: id.into(),
        original_id: original_id.map(str::to_string),
        method: HttpMethod::Get,
        path_template: format!("/{id}"),
        path_params: vec![],
        query_params: vec![],
        header_params: vec![],
        cookie_params: vec![],
        querystring_params: vec![],
        request_body: None,
        responses: vec![],
        security: vec![],
        tags: vec![],
        summary: None,
        description: None,
        deprecated: false,
        external_docs: None,
        extensions: vec![],
        servers: vec![],
        callbacks: vec![],
        location: None,
    }
}

fn ir(ops: Vec<Operation>) -> Ir {
    Ir {
        info: ApiInfo {
            title: "test-api".into(),
            version: "1.0.0".into(),
            summary: None,
            description: None,
            terms_of_service: None,
            contact: None,
            license_name: None,
            license_url: None,
            license_identifier: None,
            extensions: vec![],
        },
        operations: ops,
        types: vec![],
        security_schemes: vec![],
        servers: vec![],
        webhooks: vec![],
        external_docs: None,
        tags: vec![],
        json_schema_dialect: None,
        self_url: None,
        values: vec![],
    }
}

#[test]
fn passes_when_every_op_has_original_id() {
    let runner = runner_for("validator-required-operation-id");
    let input = ir(vec![
        op("getThing", Some("getThing")),
        op("listThings", Some("listThings")),
    ]);
    let out = runner
        .transform(input.clone(), serde_json::json!({}))
        .expect("transform");

    assert!(
        out.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        out.diagnostics
    );
    assert_eq!(out.spec, input);
}

#[test]
fn flags_op_missing_original_id() {
    let runner = runner_for("validator-required-operation-id");
    let input = ir(vec![op("derived", None), op("explicit", Some("explicit"))]);
    let out = runner
        .transform(input.clone(), serde_json::json!({}))
        .expect("transform");

    assert_eq!(out.diagnostics.len(), 1, "got: {:?}", out.diagnostics);
    let d = &out.diagnostics[0];
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.code, "validator-required-operation-id/E-MISSING-ID");
    assert!(
        d.message.contains("derived"),
        "message should name the offending op id, got: {}",
        d.message
    );
    assert_eq!(out.spec, input);
}
