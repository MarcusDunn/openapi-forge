# ADR-0001: WASM-only plugins

**Status:** accepted

## Context

`openapi-generator`'s template-based, in-tree generators are the project's
defining problem: every change requires landing a PR upstream, every release
ships at upstream cadence, and the result is a long tail of unmaintained or
near-unusable generators. The thesis of OpenAPI Forge is that generators
should be plugins, not tree-resident templates.

For "plugin" to mean anything, plugin authors must be able to ship code that
arbitrary users can run *without trusting the author*.

## Decision

Plugins are WebAssembly Component Model components, sandboxed via `wasmtime`.

Native (dynamic-library) plugins are **never** supported. Not as an opt-in
feature, not as a flag, not as a "trusted plugin" tier.

The host enforces, at the runtime boundary:

- No filesystem access (output is a return value)
- No network access
- No clock (no `wasi:clocks`)
- No randomness (no `wasi:random`)
- No environment variables, no process args, no host context
- Bounded fuel, memory, wall clock, and output size

## Rationale

A plugin is, by design, code from someone the user does not trust. The
sandbox is the entire reason this architecture is workable. A native plugin
escape hatch would mean every generator gets the security posture of its
author — i.e., the same posture that drove us away from in-tree generators.

The performance cost of WASM is real but acceptable for a code generator,
which is not on a hot path. The expressiveness cost is also real and is paid
back by the sandbox guarantee.

## Consequences

- Plugin authors target `wasm32-wasip2`. Some Rust crates don't compile there.
- The dev loop has a `cargo component build` step. See ADR-0004.
- The host import surface is deliberately tiny: `log`, `case-convert`. New
  imports require explicit ADRs because each one widens the trust boundary.

## Alternatives considered

- **Trusted-author tier with native plugins.** Rejected — once present, the
  pressure to use it for performance, "just this one feature," etc., is
  endless. Either it's a sandbox or it isn't.
- **JavaScript / Lua / TCL plugins.** Rejected — sandboxing language runtimes
  in production is harder than sandboxing WASM. WASI Preview 2 is good enough.
- **Rhai / Starlark.** Rejected for the same reason plus toolchain
  fragmentation: people want to write generators in their language of choice.
