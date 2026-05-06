# forge-plugin-sdk

Author SDK for [OpenAPI Forge](https://github.com/marcusdunn/openapi-forge)
plugins.

WASM-only. Plugins are WebAssembly Component Model components targeting
`wasm32-wasip2`; this crate provides the `wit-bindgen` glue, helper types,
and conversions between the canonical [`forge-ir`] types and the WIT
boundary.

## Worlds

A plugin implements **one** world. Pick the matching feature in your
plugin's `Cargo.toml`:

```toml
[dependencies]
forge-plugin-sdk = { version = "0.1", features = ["transformer"] }
# or
forge-plugin-sdk = { version = "0.1", features = ["generator"] }
```

Both features at once is rejected at compile time.

## No native shim

`forge-plugin-sdk` only builds for `wasm32`. There is no native shim and no
opt-in to one. Plugin integration tests run through
[`forge-test-harness`](https://crates.io/crates/forge-test-harness), which
loads the plugin's `.wasm` through the same `wasmtime` runtime the host
uses in production. The rationale is in
[ADR-0004](https://github.com/marcusdunn/openapi-forge/blob/main/docs/adr/0004-no-native-shim-in-sdk.md).

## Recommended structure

Factor pure logic (naming, templating, type rendering) into modules that
build natively under `cargo test`, and keep the WASM-boundary entry point
thin. See
[`docs/plugin-authoring.md`](https://github.com/marcusdunn/openapi-forge/blob/main/docs/plugin-authoring.md).

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) or
[MIT](../../LICENSE-MIT) at your option.
