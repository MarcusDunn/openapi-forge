# generator-go-server

Reference **cross-language** plugin (issue [#58][issue]). A Go-language code
generator that emits a minimal `net/http` server scaffold from an
OpenAPI Forge IR. The point isn't a polished server generator — it's
proof that the WIT contract works for non-Rust plugins.

[issue]: https://github.com/MarcusDunn/openapi-forge/issues/58

## What it generates

Given a spec parsed into IR, emits two files:

- `go.mod` — `go 1.22` (uses `http.ServeMux` method+pattern syntax and
  `(*http.Request).PathValue` from 1.22).
- `<package>/server.go` — a `Server` interface with one method per
  operation, plus `RegisterRoutes(*http.ServeMux, Server)` that decodes
  path params (`int32` / `int64` / `float64` / `bool` / `string`) and
  dispatches.

Out of scope: typed request/response bodies, query/header param decoding,
authentication. Implementations read the body and write the response off
`*http.Request` / `http.ResponseWriter` directly.

## Config

```json
{
  "module_path": "github.com/example/petstore",
  "package_name": "petstore"   // optional; defaults to last segment
}
```

## Build

```sh
nix develop -c bash plugins/generator-go-server/build.sh
```

Produces `plugin.wasm` (~1.1 MB). The script:

1. Stages `wit/` from the shared `forge:plugin` package + TinyGo's bundled
   `wasi:cli` deps.
2. Generates Go bindings via
   `go.bytecodealliance.org/cmd/wit-bindgen-go` (installed once into a
   plugin-local `.gopath/`).
3. Cross-compiles via `tinygo build -target=wasip2`.

The first invocation is slow (~30 s) because it downloads
`go.bytecodealliance.org/cm` and installs `wit-bindgen-go`. Subsequent
runs are fast and skip rebuilding when the artifact is up to date.

`flock` is used to serialize concurrent invocations from the integration
test suite.

## Test

```sh
nix develop -c cargo nextest run -p forge-plugin-itests \
    --features go-server --test generator_go_server
```

Four tests:

- `info_round_trip` — plugin metadata round-trips through the WIT
  boundary.
- `generates_petstore_files` — runs petstore-minimal IR through the
  plugin, asserts file presence + key strings.
- `generated_petstore_compiles_with_go_build` — writes output to a
  tempdir, shells `go build ./...`, asserts exit 0. Skips cleanly if `go`
  isn't on PATH.
- `rejects_missing_module_path` — config-invalid path returns a
  `StageError::ConfigInvalid`.

## Toolchain

| Tool             | Version       | Notes                                      |
|------------------|---------------|--------------------------------------------|
| Go               | ≥ 1.22        | tested on 1.26.2 (nixpkgs `go`)            |
| TinyGo           | ≥ 0.34        | tested on 0.40.1 (nixpkgs `tinygo`)        |
| wit-bindgen-go   | ≥ 0.7.0       | installed into `.gopath/bin` by `build.sh` |
| wasm-tools       | ≥ 1.235       | invoked transitively by TinyGo             |

`go` and `tinygo` come from `flake.nix`; `wit-bindgen-go` is installed by
the build script into a plugin-local GOPATH so it doesn't pollute the
user's `~/go`.

## Why the local `wit/` shadow?

TinyGo's `wasip2` target needs the world definition to import
`wasi:cli/imports@0.2.0` (so `wasm-tools component new` resolves the Go
runtime's WASI references), but the shared `wit/` package is generic for
all plugins and doesn't pull in WASI. The build stages a plugin-local
`wit/` that combines our `forge:plugin/code-generator` with
`wasi:cli/imports`. Rust plugins don't need this step because
`cargo-component` synthesizes the WASI imports automatically; TinyGo
does not.

`wit-source/world.wit` is the only checked-in part of the staged tree.
The rest of `wit/` is regenerated each build.

## See also

- `crates/forge-plugin-itests/tests/generator_go_server.rs` — itests.
- `wit/generator.wit` — the contract this plugin honours.
- ADR-0001 (WASM-only plugins) — the principle this plugin validates.
- Issue [#58][issue] — tracking the broader cross-language push.
