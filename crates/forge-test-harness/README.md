# forge-test-harness

Test harness for [OpenAPI Forge](https://github.com/marcusdunn/openapi-forge)
plugin authors.

This is the *only* supported integration-test path for plugins. It builds
the plugin's `.wasm` (delegating invalidation to cargo) and loads it
through the same `wasmtime`-based [`forge-host`] runtime the production
CLI uses. There is no native shim — see
[ADR-0004](https://github.com/marcusdunn/openapi-forge/blob/main/docs/adr/0004-no-native-shim-in-sdk.md).

## Usage

```rust,ignore
use forge_test_harness::PluginRunner;

#[test]
fn drops_unwanted_operations() {
    let runner = PluginRunner::build_and_load(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let out = runner
        .transform(fixture_ir(), serde_json::json!({"keep": ["users"]}))
        .unwrap();
    assert_eq!(out.spec.operations.len(), 2);
}
```

The first invocation runs `cargo build --release --target wasm32-wasip2`
on the plugin's manifest dir. Subsequent runs reuse cargo's incremental
cache; the inner-loop cycle is fast in practice.

`PluginRunner::load(wasm_path)` skips the build step if you already have
a `.wasm` artifact.

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) or
[MIT](../../LICENSE-MIT) at your option.
