//! `transformer-noop` integration test. Smoke-tests the WIT ABI: an IR
//! that survives a roundtrip through this plugin has crossed the wit
//! boundary in both directions.

mod common;

use common::runner_for;
use forge_ir::{
    ApiInfo, Header, HttpMethod, Ir, Operation, Parameter, ParameterStyle, Response, ResponseStatus,
};

fn sample_ir() -> Ir {
    Ir {
        info: ApiInfo {
            title: "test-api".into(),
            version: "1.0.0".into(),
            summary: None,
            description: Some("a test spec".into()),
            terms_of_service: None,
            contact: None,
            license_name: None,
            license_url: None,
            license_identifier: None,
            extensions: vec![],
        },
        operations: vec![Operation {
            id: "getThing".into(),
            original_id: Some("getThing".into()),
            method: HttpMethod::Get,
            path_template: "/things/{id}".into(),
            path_params: vec![],
            // A `content` parameter and a `content` header, so both
            // `content-media-type` slots cross the boundary set.
            query_params: vec![Parameter {
                name: "ids".into(),
                r#type: "ids".into(),
                required: true,
                description: None,
                deprecated: false,
                examples: vec![],
                style: Some(ParameterStyle::Form),
                explode: true,
                allow_empty_value: false,
                allow_reserved: false,
                content_media_type: Some("application/json".into()),
                extensions: vec![],
                location: None,
            }],
            header_params: vec![],
            cookie_params: vec![],
            querystring_params: vec![],
            request_body: None,
            responses: vec![Response {
                status: ResponseStatus::Explicit { code: 200 },
                content: vec![],
                headers: vec![(
                    "X-Page".into(),
                    Header {
                        r#type: "page".into(),
                        required: false,
                        description: None,
                        deprecated: false,
                        examples: vec![],
                        style: None,
                        explode: false,
                        allow_reserved: false,
                        allow_empty_value: false,
                        content_media_type: Some("application/json".into()),
                        location: None,
                    },
                )],
                summary: None,
                description: None,
                links: vec![],
                extensions: vec![],
            }],
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
        }],
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
fn roundtrip_returns_input_unchanged() {
    let runner = runner_for("transformer-noop");
    assert_eq!(runner.info().name, "transformer-noop");

    let input = sample_ir();
    let out = runner
        .transform(input.clone(), serde_json::json!({}))
        .expect("transform");

    assert!(out.diagnostics.is_empty());
    assert_eq!(out.spec, input);
}

#[test]
fn calling_transformer_as_generator_is_a_plugin_bug() {
    let runner = runner_for("transformer-noop");
    let err = runner
        .generate(sample_ir(), serde_json::json!({}))
        .unwrap_err();
    assert!(matches!(err, forge_host::StageError::PluginBug(_)));
}
