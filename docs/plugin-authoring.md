# Authoring an OpenAPI Forge plugin

Plugins are `wasm32-wasip2` components. There are two worlds: `ir-transformer`
(IR → IR + diagnostics) and `code-generator` (IR → files + diagnostics).
Validators are degenerate transformers — they return their input unchanged and
use diagnostics to communicate findings.

## Why WASM-only, no native shim

`forge-plugin-sdk` targets `wasm32-wasip2` exclusively. It refuses to build
for the host. There is no native test path.

This is a deliberate choice (ADR-0004). A native shim sounds convenient but
silently masks integer-overflow, allocator, panic-vs-trap, and `usize`-width
bugs. Plugins that pass native tests then fail in production. Worse, a shim is
a maintenance surface that drifts from the real WASM semantics.

The friction is real. We mitigate it by writing thin entry points (so build
cycles only cover small surface), by relying on cargo's incremental cache,
and by treating the WIT boundary as the only test boundary — see
*Integration tests* below.

## Project structure

```
plugins/my-generator/
├── Cargo.toml
├── schema.json              ← JSON Schema for plugin config
├── src/
│   ├── lib.rs               ← thin Guest entry point
│   ├── emit.rs              ← orchestrates IR → GenerationOutput
│   ├── naming.rs            ← pure logic
│   └── templates.rs         ← pure logic
```

Integration tests for a plugin live in a *host*-target test crate, not in
the plugin's own `tests/` dir — the plugin is `cdylib`-only and depends on
`forge-plugin-sdk`, which refuses to build for the host. For in-tree
plugins, the test crate is `crates/forge-plugin-itests/`. Plugin tests
**never** include `#[test]` or `#[cfg(test)]` modules inside the plugin
crate itself; see *Integration tests* below for the canonical pattern.

## Working against canonical IR types

The SDK exposes [`forge_plugin_sdk::ir`] (re-export of the `forge-ir` crate)
plus full bidirectional conversions between WIT-generated types and `ir::*`:

```rust
use forge_plugin_sdk::convert::generator as conv;

// Inside Guest::generate:
let canonical: forge_plugin_sdk::ir::Ir = conv::ir_from_wit(spec);
let out: forge_plugin_sdk::GenerationOutput = pure_logic(&canonical);
Ok(conv::generation_output_to_wit(out))
```

All pure logic — `emit::all`, `naming::*`, `types::*` — should accept and
return canonical types ([`forge_plugin_sdk::ir`], [`forge_plugin_sdk::OutputFile`],
[`forge_plugin_sdk::GenerationOutput`], [`forge_plugin_sdk::TransformOutput`]).
The WIT shapes only appear at the `Guest` boundary.

The world-specific `convert::*` modules (`convert::generator` /
`convert::transformer`) expose:

| Function | Direction | Notes |
|----------|-----------|-------|
| `ir_from_wit(WitIr) -> ir::Ir` | input | Receive the spec inside `Guest` |
| `ir_to_wit(ir::Ir) -> WitIr` | output | If a transformer wants to mutate then return |
| `diagnostic_from_wit` / `diagnostic_to_wit` | both | |
| `plugin_info_to_wit(ir::PluginInfo) -> WitPluginInfo` | output | Build the `info()` return |
| `plugin_info_from_wit` | input | Useful when composing plugin chains |
| `generation_output_to_wit(GenerationOutput)` | output | Generators only |
| `transform_output_to_wit(TransformOutput)` | output | Transformers only |
| `config_invalid(reason)`, `plugin_bug(reason)`, `rejected(reason, diags)` | output | Build a `stage-error` |

## Entry point — generator

```rust
use forge_plugin_sdk::convert::generator as conv;
use forge_plugin_sdk::generator::exports::forge::plugin::generator_api::{
    GenerationOutput as WitGenerationOutput, Guest,
};
use forge_plugin_sdk::generator::forge::plugin::stage::StageError;
use forge_plugin_sdk::generator::forge::plugin::types::{Ir as WitIr, PluginInfo as WitPluginInfo};
use forge_plugin_sdk::ir;

#[derive(Debug, serde::Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MyConfig {
    output_dir: Option<String>,
}

/// Pure entry point: takes the canonical `Ir`, returns a canonical
/// `GenerationOutput`. Tested from a host-target integration test that
/// loads the built `.wasm` via `forge-test-harness::PluginRunner` — never
/// from inside this crate.
pub fn generate(spec: &ir::Ir, cfg: &MyConfig) -> forge_plugin_sdk::GenerationOutput {
    emit::all(spec, cfg)
}

struct MyGenerator;

impl Guest for MyGenerator {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "my-generator".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        include_str!("../schema.json").into()
    }

    fn generate(spec: WitIr, config: String) -> Result<WitGenerationOutput, StageError> {
        let cfg: MyConfig = if config.trim().is_empty() {
            MyConfig::default()
        } else {
            forge_plugin_sdk::serde_json::from_str(&config)
                .map_err(|e| conv::config_invalid(e.to_string()))?
        };
        let canonical = conv::ir_from_wit(spec);
        let out = generate(&canonical, &cfg);
        Ok(conv::generation_output_to_wit(out))
    }
}

forge_plugin_sdk::generator::export!(MyGenerator with_types_in forge_plugin_sdk::generator);
```

## Entry point — transformer

```rust
use forge_plugin_sdk::convert::transformer as conv;
use forge_plugin_sdk::ir;
use forge_plugin_sdk::transformer::exports::forge::plugin::transformer_api::{
    Guest, TransformOutput as WitTransformOutput,
};
use forge_plugin_sdk::transformer::forge::plugin::stage::StageError;
use forge_plugin_sdk::transformer::forge::plugin::types::{Ir as WitIr, PluginInfo as WitPluginInfo};

fn transform(spec: ir::Ir) -> forge_plugin_sdk::TransformOutput {
    forge_plugin_sdk::TransformOutput {
        spec, // ...mutate as needed...
        diagnostics: vec![],
    }
}

struct MyTransformer;

impl Guest for MyTransformer {
    fn info() -> WitPluginInfo {
        conv::plugin_info_to_wit(ir::PluginInfo {
            name: "my-transformer".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        })
    }

    fn config_schema() -> String {
        r#"{"type":"object","additionalProperties":false}"#.into()
    }

    fn transform(spec: WitIr, _config: String) -> Result<WitTransformOutput, StageError> {
        let canonical = conv::ir_from_wit(spec);
        let out = transform(canonical);
        Ok(conv::transform_output_to_wit(out))
    }
}

forge_plugin_sdk::transformer::export!(MyTransformer with_types_in forge_plugin_sdk::transformer);
```

The first-party plugins under `plugins/` are written this way; cross-reference
`plugins/transformer-noop`, `plugins/generator-debug-dump`, and
`plugins/generator-typescript-fetch` for end-to-end examples.

## Integration tests

Plugin tests live in a host-target crate (the plugin itself is `cdylib`-only)
and route through `forge-test-harness::PluginRunner`. In-tree plugins are
tested from `crates/forge-plugin-itests/tests/<plugin>.rs`; out-of-tree
plugin authors should follow the same pattern in their own host-target
test crate.

```rust
// crates/forge-plugin-itests/tests/my_generator.rs
use forge_test_harness::PluginRunner;
use forge_ir::Ir;

#[test]
fn generates_expected_files() {
    let runner = PluginRunner::build_and_load("plugins/my-generator").unwrap();
    let ir: Ir = sample_ir();
    let out = runner.generate(ir, serde_json::json!({})).unwrap();
    assert_eq!(out.files.len(), 7);
}
```

`build_and_load(manifest_dir)` shells out to `cargo build --release
--target wasm32-wasip2 --manifest-path <dir>/Cargo.toml`, then loads the
resulting `.wasm` through the same `wasmtime` runtime the production CLI
uses. Cargo handles incremental rebuilds; CI's separate "build plugins"
step warms the cache so the inner build is a near-noop during the test
job.

`PluginRunner::load(wasm_path)` skips the build step if you already have
an artifact path. The harness inspects the component's exports and chooses
transformer vs. generator automatically.

**Do not** add `#[test]` or `#[cfg(test)]` modules to the plugin crate
itself. The wasm-only `compile_error!` in `forge-plugin-sdk` will trip
the moment cargo tries to build the test binary for the host. The
`xtask plugin-test-discipline` scan run as part of `cargo xtask ci`
catches this in CI.

## Diagnostics

Diagnostic codes are namespaced by plugin name. Build them with
`forge_plugin_sdk::diag` against `forge_plugin_sdk::ir::Diagnostic`:

```rust
use forge_plugin_sdk::diag;

let d = diag::error("my-gen/E-UNTAGGED-UNION", "untagged unions are not supported");
output.diagnostics.push(d);
```

Use `error` for things that prevent the plugin from continuing, `warning` for
problems the user should fix but that don't block, `info` and `hint` for
guidance. The convert helpers translate them to the WIT shape automatically
when you wrap them in `GenerationOutput` / `TransformOutput`.

## Handling specs you can't generate code for

There is no static feature gate. The host does not pre-check pipelines
against a feature manifest — plugins are responsible for inspecting the IR
they receive and either rejecting it or warning about information loss.
Two existing primitives cover the cases:

**Hard reject** (`StageError::Rejected`). Use when the plugin cannot
produce working code for the input. The pipeline aborts cleanly before
any files are written and the host surfaces your reason and diagnostics
to the user. Build it with `conv::rejected`:

```rust
let diag = ir::Diagnostic {
    severity: ir::Severity::Error,
    code: "my-gen/E-MULTIPART".into(),
    message: format!(
        "operation `{}` uses a multipart body, which this generator does not support",
        op.id,
    ),
    location: Some(ir::SpecLocation::new(format!(
        "/paths/{}/{}/requestBody",
        op.path_template.replace('/', "~1"),
        method,
    ))),
    related: vec![],
    suggested_fix: Some(ir::FixSuggestion {
        message: "remove the multipart body or pick a generator that handles it".into(),
        edits: vec![],
    }),
};
return Err(conv::rejected("spec uses multipart request bodies", vec![diag]));
```

The diagnostic should name the *specific* operation/parameter/property
that broke and ideally include a `FixSuggestion` so the user knows what
to do.

**Soft warn** (`Severity::Warning` in the normal output). Use when the
plugin can produce output but is dropping information (e.g. a generator
that compiles a discriminated union as untagged). Generation proceeds:

```rust
output.diagnostics.push(ir::Diagnostic {
    severity: ir::Severity::Warning,
    code: "my-gen/W-DISCRIMINATOR".into(),
    message: format!("discriminator on `{}` is not modeled by this generator", t.id),
    location: Some(ir::SpecLocation::new(format!("/components/schemas/{}", t.id))),
    related: vec![],
    suggested_fix: None,
});
```

Reference: `plugins/test-fixtures/generator-strict` (hard reject) and
`plugins/test-fixtures/generator-warn` (soft warn) are minimal examples
covered by `crates/forge-plugin-itests/tests/rejection.rs`.

## Distributing your plugin

Once your plugin builds to a wasip2 component, push it to any OCI
registry (Docker Hub, GHCR, ECR, …) and users reference it directly
from `forge.toml`:

```toml
[generator]
oci = "ghcr.io/<you>/my-generator:0.1.0"
```

The canonical publish incantation, using
[oras](https://oras.land):

```bash
oras push ghcr.io/<you>/my-generator:0.1.0 \
  ./target/wasm32-wasip2/release/my_generator.wasm:application/vnd.bytecodealliance.wasm.component.layer.v0+wasm
```

`forge` accepts three layer media types: the Bytecode-Alliance
component-layer media type above, plain `application/wasm`, and
`application/vnd.wasm.content.layer.v1+wasm`. Single-layer artifacts
with any media type are also accepted.

Pin by digest for reproducible builds:

```toml
[generator]
oci = "ghcr.io/<you>/my-generator@sha256:abc123…"
```

Pulled artifacts are cached under
`$XDG_CACHE_HOME/openapi-forge/plugins/`. See ADR-0010 for the cache
layout and v1 limitations (anonymous registries only).

## Determinism

Plugins must be deterministic. The host enforces:

- No clock, no random, no env, no filesystem access from inside WASM
- `IndexMap` over `HashMap` whenever iteration order is observable
- All `list<>` outputs sorted by stable keys before returning

Non-determinism is a bug. CI runs every fixture twice and diffs.

## Plugins in other languages

The WIT contract in `wit/` is the cross-language source of truth. Anything
that compiles to a wasip2 component matching the `code-generator` or
`ir-transformer` world will load through the same `forge-host` runtime
the CLI uses — no Rust required. `forge-plugin-sdk` is a Rust-side
ergonomic layer over the WIT, not the contract itself.

Two non-Rust references live in tree:

- **`plugins/generator-go-server/`** — Go via TinyGo +
  `go.bytecodealliance.org/cmd/wit-bindgen-go`. Emits a minimal `net/http`
  server scaffold; itest shells `go build` on the output. Smallest
  cross-language footprint (~1 MB component).
- **`plugins/generator-typescript-cli/`** — TypeScript via
  [jco](https://github.com/bytecodealliance/jco) +
  [componentize-js](https://github.com/bytecodealliance/ComponentizeJS).
  Emits a `commander`-based CLI client per spec — kebab-case
  subcommands, typed param parsing with enum `choices`, env-var auth.
  Larger component (~12 MB; the StarlingMonkey JS runtime is the floor)
  but the developer experience inside the plugin is closest to writing
  ordinary node code.

Each plugin's `README.md` documents its toolchain pin and build flow. The
integration tests
(`crates/forge-plugin-itests/tests/generator_{go_server,typescript_cli}.rs`,
gated behind the `go-server` and `typescript-cli` features respectively)
load the resulting `.wasm` through `PluginRunner::load` — the same path
the CLI takes — and exercise the output with the target language's
toolchain (`go build`, `npm install` + `tsc` + `node ... --help`).

Tracking issue for the broader cross-language push (publishing
`forge-test-harness`, lifting invariants out of the Rust SDK into a
language-neutral `plugin-contract.md`, locking diagnostic-code
namespacing, adding a `forge conformance` CLI subcommand): [#58].

[#58]: https://github.com/MarcusDunn/openapi-forge/issues/58
