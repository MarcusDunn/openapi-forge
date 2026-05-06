# Architecture

For contributors. End users want `README.md`; plugin authors want
`plugin-authoring.md`.

## Workspace layout

Two Cargo workspaces.

- **Host workspace** at the repo root: everything the `forge` CLI needs.
  Builds for Linux and macOS.
- **Plugin workspace** at `plugins/`: each plugin is a member, all targeting
  `wasm32-wasip2`.

`crates/forge-plugin-sdk` lives in the host repo but is excluded from the host
workspace (`exclude = [...]` in the root `Cargo.toml`). It targets WASM only
and refuses to build for the host (`compile_error!` guard). See ADR-0004.

## Crate dependency graph

```
                  ┌────────────────┐
                  │   forge-ir     │ ← canonical Rust IR types
                  └───────┬────────┘
                          │
        ┌─────────────────┼──────────────────┐
        │                 │                  │
┌───────▼────────┐ ┌──────▼──────┐ ┌────────▼─────────┐
│ forge-parser   │ │forge-ir-    │ │ forge-plugin-sdk │
│                │ │   bindgen   │ │   (wasm only)    │
└───────┬────────┘ └──────┬──────┘ └──────────────────┘
        │                 │
        │           ┌─────▼──────┐
        │           │ forge-host │ (wasmtime; landed Step 1.4)
        │           └─────┬──────┘
        │                 │
        │     ┌───────────┼─────────────┐
        │     │           │             │
        │  ┌──▼─────────┐ │   ┌─────────▼────────┐
        │  │ forge-test-│ │   │  forge-pipeline  │
        │  │   harness  │ │   └─────────┬────────┘
        │  └────────────┘ │             │
        │                 │       ┌─────▼──────┐
        │                 │       │ forge-cli  │
        │                 │       └────────────┘
```

## Pipeline

See plan §4 for the diagram. The CLI:

1. Loads `forge.toml` (deferred to Step 1.8)
2. Parses the spec → IR
3. Runs light normalization ($ref deref, allOf flattening)
4. Runs each transformer in sequence; halts between stages if any returned
   `error`-severity diagnostics (default; `--allow-errors` overrides)
5. Runs full normalization (sanitization, dedup, topo-sort)
6. Runs the generator
7. Validates output paths against the output guard
8. Atomically writes to the output directory

There is no static feature-compatibility gate before the pipeline runs.
Plugins inspect the IR they receive and either reject (`StageError::Rejected`)
or warn (`Severity::Warning`); see `docs/plugin-authoring.md`.

## Determinism

Enforced. Plugins receive no clock, RNG, env, filesystem, or network — see
ADR-0003. The IR-level invariants (sorted operations, topo-sorted types)
live in `docs/ir-spec.md` §Determinism rules and are validated in
`crates/forge-parser/src/finalize.rs`.

The `determinism` job in `.github/workflows/ci.yml` runs
`forge generate fixtures/e2e/petstore --out /tmp/forge-{a,b}` twice and
`diff -r`s the outputs; any byte difference fails CI. See plan §14.

## Fuzzing

The parser is the only host-side untrusted-input boundary. `fuzz/` holds
two cargo-fuzz targets — `parse_str_bytes` (raw bytes) and
`parse_str_structured` (`arbitrary`-derived JSON values biased toward
OpenAPI keywords). A short smoke run executes on every PR; a deeper
matrix runs nightly. Crashes get minimized and committed under
`crates/forge-parser/tests/fuzz_regressions/` so they replay on stable
without nightly. See `docs/fuzzing.md`.

## ADRs

`docs/adr/` holds architecture decision records. New ADRs get a number, a
title in kebab-case, and a markdown file. The number is monotonic; do not
renumber.
