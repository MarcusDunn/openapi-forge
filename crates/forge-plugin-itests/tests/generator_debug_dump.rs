//! `generator-debug-dump` integration test.

mod common;

use common::runner_for;
use forge_ir::{ApiInfo, HttpMethod, Ir, Operation};

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
fn emits_one_file_with_expected_contents() {
    let runner = runner_for("generator-debug-dump");
    assert_eq!(runner.info().name, "generator-debug-dump");

    let out = runner
        .generate(sample_ir(), serde_json::json!({}))
        .expect("generate");

    assert_eq!(out.files.len(), 1);
    let f = &out.files[0];
    assert_eq!(f.path, "ir.txt");
    let body = std::str::from_utf8(&f.content).unwrap();
    assert!(body.contains("title:    test-api"), "body was: {body}");
    assert!(body.contains("GET /things/{id}"), "body was: {body}");
}
