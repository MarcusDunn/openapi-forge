//! End-to-end CLI test: parse a `forge.toml`, run a transformer →
//! generator pipeline against an `ir.json`, write outputs.
//!
//! Uses the same plugin artifacts as `forge-host`'s `tests/plugins.rs` —
//! both expect them to have been built first via
//! `cargo build --release --manifest-path plugins/Cargo.toml`.

use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap()
}

fn plugin_artifact(name: &str) -> PathBuf {
    let path = repo_root()
        .join("plugins/target/wasm32-wasip2/release")
        .join(format!("{name}.wasm"));
    if !path.exists() {
        panic!(
            "plugin artifact missing at {}.\nBuild plugins first:\n    \
             cargo build --release --manifest-path plugins/Cargo.toml",
            path.display()
        );
    }
    path
}

const SAMPLE_IR: &str = r#"{
  "info": { "title": "test-api", "version": "1.0.0" },
  "operations": [
    {
      "id": "getThing",
      "method": "get",
      "path_template": "/things/{id}",
      "responses": []
    }
  ],
  "types": [],
  "security_schemes": [],
  "servers": []
}"#;

#[test]
fn generate_pipeline_writes_files() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    std::fs::write(project.join("ir.json"), SAMPLE_IR).unwrap();

    let xform_wasm = plugin_artifact("transformer_noop");
    let gen_wasm = plugin_artifact("generator_debug_dump");

    let toml = format!(
        r#"
[input]
ir = "ir.json"

[[transformers]]
wasm = "{xform}"

[generator]
wasm = "{gen}"

[output]
dir = "out"
"#,
        xform = xform_wasm.display(),
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .success();

    let out_path = project.join("out/ir.txt");
    let out = std::fs::read_to_string(&out_path).expect("output file");
    assert!(out.contains("title:    test-api"), "body: {out}");
    assert!(out.contains("GET /things/{id}"), "body: {out}");
}

#[test]
fn generate_from_petstore_spec() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    let petstore = repo_root().join("fixtures/e2e/petstore/spec.json");
    let spec = std::fs::read_to_string(&petstore).expect("read petstore spec");
    std::fs::write(project.join("spec.json"), spec).unwrap();

    let gen_wasm = plugin_artifact("generator_typescript_fetch");

    let toml = format!(
        r#"
[input]
spec = "spec.json"

[generator]
wasm = "{gen}"
config = {{ packageName = "petstore-client" }}

[output]
dir = "out"
"#,
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .success();

    let out_root = project.join("out");
    let client = std::fs::read_to_string(out_root.join("src/client.ts")).expect("client.ts");
    assert!(client.contains("export class ApiClient"), "{client}");
    assert!(client.contains("async listPets"), "{client}");
    assert!(client.contains("async createPet"), "{client}");
    assert!(client.contains("async showPetById"), "{client}");

    let models = std::fs::read_to_string(out_root.join("src/models.ts")).expect("models.ts");
    assert!(models.contains("export interface Pet {"), "{models}");
    assert!(
        models.contains("export type Pets = Array<Pet>;"),
        "{models}"
    );

    let pkg = std::fs::read_to_string(out_root.join("package.json")).expect("package.json");
    assert!(pkg.contains("\"name\": \"petstore-client\""), "{pkg}");
}

#[test]
fn unsupported_spec_feature_halts_with_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    // `not` composition is out-of-scope: parser must emit an error and
    // halt before running the generator.
    let bad_spec = r#"{
        "openapi": "3.0.3",
        "info": { "title": "x", "version": "1" },
        "paths": {},
        "components": {
            "schemas": {
                "Bad": {
                    "not": { "type": "string" }
                }
            }
        }
    }"#;
    std::fs::write(project.join("spec.json"), bad_spec).unwrap();
    let gen_wasm = plugin_artifact("generator_debug_dump");
    let toml = format!(
        r#"
[input]
spec = "spec.json"

[generator]
wasm = "{gen}"

[output]
dir = "out"
"#,
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .failure()
        .stderr(predicates::str::contains("parser/E-COMPOSITION-NOT"));
}

#[test]
fn generator_config_passes_validation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    let petstore = repo_root().join("fixtures/e2e/petstore/spec.json");
    let spec = std::fs::read_to_string(&petstore).expect("read petstore spec");
    std::fs::write(project.join("spec.json"), spec).unwrap();

    let gen_wasm = plugin_artifact("generator_typescript_fetch");
    // Both `packageName` and `baseUrl` are declared in the generator's
    // schema.json — both should pass validation.
    let toml = format!(
        r#"
[input]
spec = "spec.json"

[generator]
wasm = "{gen}"
config = {{ packageName = "petstore-client", baseUrl = "https://example.com" }}

[output]
dir = "out"
"#,
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .success();
}

#[test]
fn generator_config_fails_validation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    let petstore = repo_root().join("fixtures/e2e/petstore/spec.json");
    let spec = std::fs::read_to_string(&petstore).expect("read petstore spec");
    std::fs::write(project.join("spec.json"), spec).unwrap();

    let gen_wasm = plugin_artifact("generator_typescript_fetch");
    // `bogusKey` is rejected because the schema sets
    // `additionalProperties: false`.
    let toml = format!(
        r#"
[input]
spec = "spec.json"

[generator]
wasm = "{gen}"
config = {{ bogusKey = "nope" }}

[output]
dir = "out"
"#,
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .failure()
        .stderr(predicates::str::contains("config validation failed"))
        .stderr(predicates::str::contains("bogusKey"));
}

/// Config-less mode: no `forge.toml` on disk. The pipeline shape is
/// passed entirely via CLI flags (`--input`, `--transformer`,
/// `--generator`, `-o`). Mirrors `generate_from_petstore_spec` but with
/// every knob coming from the command line and additionally chains a
/// transformer to exercise the repeatable `--transformer` flag.
#[test]
fn generate_config_less_from_spec() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    let petstore = repo_root().join("fixtures/e2e/petstore/spec.json");
    let xform_wasm = plugin_artifact("transformer_noop");
    let gen_wasm = plugin_artifact("generator_typescript_fetch");
    let out_dir = project.join("out");

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg("-i")
        .arg(&petstore)
        .arg("--transformer")
        .arg(&xform_wasm)
        .arg("--generator")
        .arg(&gen_wasm)
        .arg("-o")
        .arg(&out_dir)
        .assert()
        .success();

    let client = std::fs::read_to_string(out_dir.join("src/client.ts")).expect("client.ts");
    assert!(client.contains("export class ApiClient"), "{client}");
    assert!(client.contains("async listPets"), "{client}");

    // No forge.toml was written; lock in the contract that config-less
    // mode does not depend on one being present in the run directory.
    assert!(
        !project.join("forge.toml").exists(),
        "config-less run must not depend on forge.toml"
    );
}

/// Without `--input` and without a `forge.toml` in the project dir, the
/// run must fail with a clear "failed to read forge.toml" error rather
/// than silently doing the wrong thing.
#[test]
fn generate_no_config_no_args_fails() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("forge.toml"));
}

/// Config-less mode requires `--generator`. Asserting the dedicated
/// error surfaces keeps the contract obvious for future contributors.
#[test]
fn generate_config_less_requires_generator() {
    let dir = tempfile::tempdir().unwrap();
    let petstore = repo_root().join("fixtures/e2e/petstore/spec.json");

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg("-i")
        .arg(&petstore)
        .arg("-o")
        .arg(dir.path().join("out"))
        .assert()
        .failure()
        .stderr(predicates::str::contains("--generator is required"));
}

/// `[limits]` in `forge.toml` overrides the built-in sandbox limits.
/// Raising every knob must still let a normal run succeed.
#[test]
fn generate_with_raised_limits_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    std::fs::write(project.join("ir.json"), SAMPLE_IR).unwrap();

    let xform_wasm = plugin_artifact("transformer_noop");
    let gen_wasm = plugin_artifact("generator_debug_dump");

    let toml = format!(
        r#"
[input]
ir = "ir.json"

[[transformers]]
wasm = "{xform}"

[generator]
wasm = "{gen}"

[output]
dir = "out"

[limits.transformer]
fuel = 10000000000
wall_clock_ms = 10000

[limits.generator]
fuel = 100000000000
memory_bytes = 1073741824
output_files_max = 50000
output_total_bytes_max = 1073741824
output_per_file_bytes_max = 134217728
"#,
        xform = xform_wasm.display(),
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .success();

    let out = std::fs::read_to_string(project.join("out/ir.txt")).expect("output file");
    assert!(out.contains("title:    test-api"), "body: {out}");
}

/// Lowered limits are enforced, not just parsed: a one-unit fuel budget
/// must trap the generator with a fuel-exhaustion error.
#[test]
fn generate_with_tiny_fuel_limit_fails() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    std::fs::write(project.join("ir.json"), SAMPLE_IR).unwrap();

    let gen_wasm = plugin_artifact("generator_debug_dump");

    let toml = format!(
        r#"
[input]
ir = "ir.json"

[generator]
wasm = "{gen}"

[output]
dir = "out"

[limits.generator]
fuel = 1
"#,
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .failure()
        .stderr(predicates::str::contains("exceeded Fuel"));
}

/// A typo'd key in `[limits]` fails the run loudly instead of silently
/// keeping the default.
#[test]
fn generate_with_unknown_limit_key_fails() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    std::fs::write(project.join("ir.json"), SAMPLE_IR).unwrap();

    let gen_wasm = plugin_artifact("generator_debug_dump");

    let toml = format!(
        r#"
[input]
ir = "ir.json"

[generator]
wasm = "{gen}"

[output]
dir = "out"

[limits.generator]
feul = 100
"#,
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .failure()
        .stderr(predicates::str::contains("failed to parse forge.toml"));
}

#[test]
fn ir_version_subcommand() {
    Command::cargo_bin("forge")
        .unwrap()
        .arg("ir-version")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("0."));
}

/// Regression: the CLI previously read the spec into memory and called
/// `forge_parser::parse_str_with_file`, which has no file-based $ref
/// resolver. Specs that split paths / components across sibling files
/// (Stripe, GitHub, …) every external `$ref` would surface as
/// `parser/E-EXTERNAL-REF` and halt the run with hundreds of diagnostics.
/// The parser already supports split-document specs via `parse_path` (#56);
/// this test points the CLI at `fixtures/real-world/multi-tenant-shape/`,
/// which exercises that exact layout, and asserts the run succeeds end
/// to end.
#[test]
fn generate_from_split_document_spec() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    let fixture = repo_root().join("fixtures/real-world/multi-tenant-shape");
    let gen_wasm = plugin_artifact("generator_debug_dump");

    let toml = format!(
        r#"
[input]
spec = "{spec}"

[generator]
wasm = "{gen}"

[output]
dir = "out"
"#,
        spec = fixture.join("spec.json").display(),
        gen = gen_wasm.display(),
    );
    std::fs::write(project.join("forge.toml"), toml).unwrap();

    Command::cargo_bin("forge")
        .unwrap()
        .arg("generate")
        .arg(project)
        .assert()
        .success();

    // Operations resolved from sibling files actually appear in the IR.
    let out = std::fs::read_to_string(project.join("out/ir.txt")).expect("output");
    assert!(out.contains("listUsers"), "missing listUsers: {out}");
    assert!(out.contains("createUser"), "missing createUser: {out}");
    assert!(out.contains("getDocument"), "missing getDocument: {out}");
    assert!(
        out.contains("uploadDocumentAttachment"),
        "missing uploadDocumentAttachment: {out}"
    );
    assert!(
        out.contains("updateNoteSavedView"),
        "missing updateNoteSavedView: {out}"
    );
}
