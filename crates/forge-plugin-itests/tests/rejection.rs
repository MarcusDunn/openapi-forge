//! Plugin rejection-pattern coverage. The host has no static feature
//! gate — plugins that can't handle their input return
//! `StageError::Rejected` with diagnostics; plugins that can but lose
//! information emit `Severity::Warning` diagnostics. Both patterns are
//! exercised here against real `.wasm` plugins via the test harness.

mod common;

use common::{ir_for, petstore_ir, runner_for};
use forge_host::StageError;
use forge_ir::Severity;

#[test]
fn strict_accepts_spec_without_multipart() {
    let runner = runner_for("test-fixtures/generator-strict");
    let out = runner
        .generate(petstore_ir(), serde_json::json!({}))
        .expect("petstore has no multipart; generator should accept");
    assert!(out.diagnostics.is_empty());
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].path, "strict.txt");
}

#[test]
fn strict_rejects_multipart_with_diagnostic() {
    let runner = runner_for("test-fixtures/generator-strict");
    let err = runner
        .generate(ir_for("body-multipart"), serde_json::json!({}))
        .expect_err("multipart spec should be rejected");

    let StageError::Rejected {
        reason,
        diagnostics,
    } = err
    else {
        panic!("expected Rejected, got {err:?}");
    };

    assert!(reason.contains("multipart"), "reason: {reason}");
    assert_eq!(diagnostics.len(), 1, "expected one diagnostic");
    let d = &diagnostics[0];
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.code, "generator-strict-fixture/E-MULTIPART");
    let loc = d
        .location
        .as_ref()
        .expect("diagnostic must have a location");
    assert!(
        loc.pointer.contains("requestBody"),
        "pointer should name the offending location: {}",
        loc.pointer
    );
    assert!(
        d.suggested_fix.is_some(),
        "rejection should include a fix suggestion"
    );
}

#[test]
fn warn_accepts_multipart_but_emits_warning() {
    let runner = runner_for("test-fixtures/generator-warn");
    let out = runner
        .generate(ir_for("body-multipart"), serde_json::json!({}))
        .expect("warn fixture should accept multipart");

    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].path, "warn.txt");

    let warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect();
    assert!(
        !warnings.is_empty(),
        "expected at least one Warning diagnostic for the multipart operation"
    );
    assert!(warnings
        .iter()
        .any(|d| d.code == "generator-warn-fixture/W-MULTIPART"));
}
