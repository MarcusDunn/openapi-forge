# generator-typescript-cli

Reference **cross-language** plugin (issue [#58][issue]). Source is
TypeScript, compiled to a wasip2 component via [jco] +
[componentize-js]. Same `forge-host` runtime as Rust and Go plugins.

[issue]: https://github.com/MarcusDunn/openapi-forge/issues/58
[jco]: https://github.com/bytecodealliance/jco
[componentize-js]: https://github.com/bytecodealliance/ComponentizeJS

## What it generates

A node-runnable CLI client for any spec the parser supports. Output tree:

```
package.json              # commander 14, no other runtime deps
tsconfig.json
README.md
bin/<name>.js             # shebang shim → dist/cli.js
src/
  cli.ts                  # commander program; one subcommand per operation
  client.ts               # ApiClient class
  models.ts               # interfaces + type aliases + literal-union enums
  auth.ts                 # AuthConfig + env/flag loaders
  runtime.ts              # ApiError + encodeQuery* helpers
  format.ts               # JSON / compact / table output
  index.ts                # library exports
```

Each operation becomes a kebab-case subcommand. Path params are positional;
query, header, and cookie params are `--<name>` flags. String enums get
`commander.choices()`; integer enums get `parseInt` + `choices`. Bodies
are accepted as inline JSON, `@file.json`, or `-` for stdin.

Auth resolves in this order:
1. CLI flag (`--token` / `--api-key` / `-u user:pass`)
2. Env var (`<PREFIX>_TOKEN`, `<PREFIX>_API_KEY`, `<PREFIX>_USERNAME` +
   `<PREFIX>_PASSWORD`)

`<PREFIX>` defaults to a `SCREAMING_SNAKE` form of the bin name; override
via `envPrefix` in plugin config.

## Config

```json
{
  "name": "gh-issues",
  "baseUrl": "https://api.github.com",
  "envPrefix": "GH_ISSUES"
}
```

All fields optional. Defaults derive from `info.title` / `servers[0].url`.

## Build

```sh
nix develop -c bash plugins/generator-typescript-cli/build.sh
```

Produces `plugin.wasm` (~12 MB). The script:

1. `npm ci` — install jco / componentize-js / esbuild / typescript into
   `./node_modules/`.
2. `npm run typecheck` — `tsc --noEmit`.
3. `npm run bundle` — esbuild bundles `src/component.ts` and friends into
   a single ESM file at `dist/component.js`. componentize-js does **not**
   accept TypeScript directly and does not support multi-file ESM.
4. `npm run componentize` — jco runs componentize-js. `--disable all`
   strips runtime WASI imports the host doesn't provide; the host's
   `wasmtime_wasi` deny-all sandbox covers what's left.

`flock` serializes concurrent invocations from the integration-test
suite; an mtime check skips when `plugin.wasm` is fresher than every
input.

## Test

```sh
nix develop -c cargo nextest run -p forge-plugin-itests \
    --features typescript-cli --test generator_typescript_cli
```

Six tests:

- `info_round_trip` — plugin metadata across the WIT boundary.
- `generates_petstore_files` — petstore-minimal IR → expected file set + key strings.
- `generated_petstore_compiles_and_help_works` — `npm install` + `tsc` +
  `node bin/petstore.js --help`, asserts every operation is a subcommand.
- `generated_github_issues_compiles_with_enum_choices` — same against the
  richer github-issues fixture; asserts `state` enum values surface in
  per-subcommand `--help`.
- `generated_stripe_customers_compiles` — third real-world fixture for
  breadth (bearer auth + `allOf` flattening + paginated list envelopes).
- `rejects_unknown_config_field` — config-invalid path returns
  `StageError::ConfigInvalid`.

## Toolchain

| Tool | Version | Notes |
|---|---|---|
| Node | ≥ 22 | Tested on 22.22.2 (nixpkgs `nodejs_22`) |
| jco | ^1.16 | Installed via `npm ci` into plugin-local `node_modules` |
| componentize-js | ^0.20 | Same |
| esbuild | ^0.28 | Same |
| typescript | ^6.0 | Same |
| commander | ^14.0 | In *generated output*'s deps |

Pinned in `package.json` / `package-lock.json` (committed for
reproducibility).

## Why `--disable all`?

componentize-js auto-injects WASI imports the StarlingMonkey JS runtime
needs (clocks, random, http, fetch-event, stdio). Our host's
`wasmtime_wasi` provides a deny-all sandbox for the standard subset; the
plugin doesn't actually call any of these (no clocks, no random — see
ADR-0003 + issue #60), so we strip them at componentize time. Plugins
that needed e.g. random would un-disable it and the host would have to
provide a deterministic stub.

## See also

- `crates/forge-plugin-itests/tests/generator_typescript_cli.rs` — itests.
- `plugins/generator-go-server/` — sibling reference plugin (Go via TinyGo).
- `plugins/generator-typescript-fetch/` — Rust-source TS *library* generator (the existing in-tree generator); this plugin reuses many of its naming and runtime patterns.
- ADR-0001 (WASM-only plugins) + ADR-0004 (no native shim) — the principles this plugin validates.
- Issue [#58][issue] — tracking the broader cross-language push.
