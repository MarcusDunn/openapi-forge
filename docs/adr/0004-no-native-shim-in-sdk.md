# ADR-0004: No native execution shim in the plugin SDK

**Status:** accepted

## Context

Plugin authors using `forge-plugin-sdk` write Rust crates that compile to
`wasm32-wasip2`. They iterate in a `cargo component build && wasmtime ...`
loop, which is several seconds per cycle versus milliseconds for native
`cargo test`.

A common pattern in this kind of project is a "native shim": a feature-gated
crate-level mode that lets the plugin compile for the host so its tests run
under regular `cargo test`. Under the shim, the WIT bindings are replaced
with stub implementations, and the plugin's logic exercises real Rust types
instead of crossing a component boundary.

We considered shipping such a shim. We decided not to.

## Decision

`forge-plugin-sdk` only builds for `wasm32`. A `compile_error!` enforces this
at the top of `lib.rs`. There is no feature flag to make it build for the
host.

Plugin authors test integration through `forge-test-harness`, which loads the
plugin's actual `.wasm` and invokes it through the same `wasmtime`-based path
the production host uses.

## Rationale

A native shim creates a class of bugs that pass tests and fail in production:

1. **Integer width and overflow.** `usize` is 32-bit on `wasm32`, 64-bit on
   most hosts. Code that compiles cleanly and passes tests under the shim
   silently overflows or truncates in WASM.
2. **Allocator behavior.** WASM linear memory imposes different allocation
   patterns than glibc/jemalloc. Out-of-memory looks different. Realloc cost
   differs. Native tests don't surface this.
3. **Panic vs. trap.** Rust panics in WASM trap, ending the instance.
   Recoverable patterns under native become hard-stops in production.
4. **UTF-8 boundaries.** Component-model strings copy through the canonical
   ABI, which has its own validation. Native code that builds invalid strings
   only fails when actually crossing the boundary.

A shim also adds a maintenance surface that drifts from real WASM semantics.
Every WIT change needs a parallel shim update. Drift is corrosive: when the
shim's truth and the runtime's truth diverge, plugin authors hit bugs they
can't reproduce.

Finally, a shim trains a two-tier mental model — "fast tests" and "real
tests" — in which authors lean on the fast tier and treat the real tier as a
formality. Plugins ship with thin integration coverage as a result.

## All plugin tests cross the WIT boundary

Plugin crates ship **no native unit tests**. Every assertion goes through a
built `.wasm` loaded by `forge-test-harness::PluginRunner`. This is stricter
than the original ADR text, which permitted "pure-module" unit tests against
`forge-ir` types — and it matches reality, because `cargo test` in a plugin
crate hits the same `compile_error!` that blocks the rest of the host build.

The reasons are the same as for refusing a shim, plus one more: any pure
module that grows tests grows pressure to expand its surface, drifting toward
the de-facto shim we already rejected. Keeping the test boundary at the WIT
edge keeps the surface honest.

In-tree plugins are tested from `crates/forge-plugin-itests/tests/<plugin>.rs`
using `PluginRunner::build_and_load`. Plugin authors writing their own crates
should follow the same pattern in their own host-target test crate.

## Mitigations

The friction is real. The recommended mitigations:

- **Test harness build caching.** `forge-test-harness::PluginRunner` delegates
  build invalidation to cargo; subsequent test runs are no-ops when the
  artifact is current.
- **Smaller debug builds.** Plugin templates default to a thinner debug
  profile to keep build times low.
- **Pre-build in CI.** A separate "build plugins" CI step warms cargo's
  caches so the harness's inner build is a near-noop during the test job.

## Consequences

- New plugin authors hit a learning curve the first time they encounter the
  WASM-only constraint. `docs/plugin-authoring.md` opens with the rationale.
- Test cycles for the entry point are seconds, not milliseconds. This is the
  cost of the security guarantee.

## Alternatives considered

- **Shim behind `cfg(test)` only.** Same drift problems, slightly less
  visible.
- **Shim as a separate `forge-plugin-sdk-test` crate.** Same problems plus
  a confusing dependency story.
- **Compile WASM with debuginfo and run under wasmtime in test mode.** This
  is what we already do via `forge-test-harness`. The complaint is build
  speed, which is addressed by cargo's incremental compilation, not by a
  shim.
