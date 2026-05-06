//! Directory-driven parser conformance tests.
//!
//! Each subdirectory of `fixtures/conformance/` contains a `spec.json`,
//! an `expected-ir.json`, and optionally an `expected-diagnostics.json`.
//! The test parses the spec and asserts the parsed IR + diagnostics match
//! the committed expectations.
//!
//! To regenerate expected files (after an intentional change):
//!
//!     FORGE_REGEN=1 cargo test -p forge-parser --test conformance
//!
//! The first run creates the files; subsequent runs only write when the
//! env var is set, so accidental drift fails the test.

use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("conformance")
        .canonicalize()
        .expect("fixtures/conformance must exist")
}

fn list_fixture_dirs() -> Vec<PathBuf> {
    let root = fixtures_root();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|_| panic!("read_dir {root:?}"))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

fn regen() -> bool {
    std::env::var("FORGE_REGEN").is_ok()
}

#[test]
fn conformance_fixtures() {
    let mut failures: Vec<String> = Vec::new();
    let dirs = list_fixture_dirs();
    assert!(!dirs.is_empty(), "no fixtures found under conformance/");

    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let spec_path = dir.join("spec.json");
        if !spec_path.exists() {
            failures.push(format!("{name}: missing spec.json"));
            continue;
        }
        // Use `parse_path` so external `$ref`s in multi-file fixtures
        // resolve relative to the fixture directory.
        let out = match forge_parser::parse_path(&spec_path) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{name}: parser fatal error: {e}"));
                continue;
            }
        };

        // The Io-error path scrubs the file label which is platform-dependent;
        // for stability we never read `out.diagnostics[].location.file` in
        // expected-diagnostics. The parser itself records `Some(<path>)`.
        let _ = spec_path;

        let actual_ir = serde_json::to_string_pretty(&out.spec).unwrap() + "\n";
        let actual_diags = serde_json::to_string_pretty(&out.diagnostics).unwrap() + "\n";

        let ir_path = dir.join("expected-ir.json");
        let diag_path = dir.join("expected-diagnostics.json");

        if regen() || !ir_path.exists() {
            std::fs::write(&ir_path, &actual_ir).unwrap();
        }
        if regen() || !diag_path.exists() {
            std::fs::write(&diag_path, &actual_diags).unwrap();
        }

        let expected_ir = std::fs::read_to_string(&ir_path).unwrap();
        if actual_ir != expected_ir {
            failures.push(format!(
                "{name}: IR diff. Run with FORGE_REGEN=1 to update.\n--- expected\n{expected_ir}\n--- actual\n{actual_ir}"
            ));
        }
        let expected_diags = std::fs::read_to_string(&diag_path).unwrap();
        if actual_diags != expected_diags {
            failures.push(format!(
                "{name}: diagnostics diff. Run with FORGE_REGEN=1 to update.\n--- expected\n{expected_diags}\n--- actual\n{actual_diags}"
            ));
        }
    }

    if !failures.is_empty() {
        panic!("\n{}", failures.join("\n\n"));
    }
}
