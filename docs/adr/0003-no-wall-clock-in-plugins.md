# ADR-0003: No wall-clock, RNG, or host I/O for plugins

**Status:** accepted

## Context

Plugins are WASM components (ADR-0001) running under `wasmtime`. WASI
Preview 2 defines a rich capability surface — `wasi:clocks`,
`wasi:random`, `wasi:filesystem`, `wasi:sockets`,
`wasi:cli/environment`, stdio — and the host gets to choose, per
component instantiation, which subset is wired in.

The default for most WASI hosts is "grant everything." That is the wrong
default for OpenAPI Forge.

## Decision

Plugins receive **no** ambient host capabilities. Specifically:

- No clock (no `wasi:clocks/wall-clock`, no `monotonic-clock`).
- No randomness (no `wasi:random`).
- No filesystem (no preopens; reads and writes both unavailable).
- No network (no `wasi:sockets`).
- No environment variables, process args, or inherited stdio
  (`wasi:cli/environment`, `wasi:cli/stdin`, etc. all empty).
- No `exit` semantics beyond a trap.

The only host imports plugins may use are the ones declared in `wit/` —
today `log` and `case-convert`. New imports require a new ADR because
each one widens what a malicious or buggy plugin can do, and what
non-determinism it can introduce.

## Rationale

**Determinism is load-bearing.** Same input must produce the same output
bytes, run-to-run, machine-to-machine. CI proves this with a
`determinism` job that runs the petstore pipeline twice and `diff -r`s
the output (plan §14, and see the CI workflow). A wall clock or RNG
breaks determinism in the most trivial way possible: a generator that
embeds the build timestamp passes its tests on the author's machine and
makes every consumer's diff churn forever.

**Sandboxing.** Plugins are untrusted code (ADR-0001). Filesystem
access turns "this generator scrapes the spec for typos" into "this
generator exfiltrates `~/.aws/credentials`." Network access turns it
into "this generator beacons home." The cost of saying *no* by default
is one config field per plugin author who genuinely needs an input;
the cost of saying *yes* is unbounded.

**No legitimate use case yet.** Code generators are pure functions of
their input. A plugin that wants the current date can take it as
config. A plugin that wants random ids should derive them deterministically
from input (e.g. content hash). Neither is a hardship.

## Code references

- `crates/forge-host/src/runtime.rs::HostState::new` builds the WASI
  context with `WasiCtxBuilder::new().build()` — no preopens, no env,
  no stdio. The accompanying comment notes that `wasmtime-wasi` is
  still wired into the linker because `wasm32-wasip2` libstd
  unconditionally imports WASI interfaces; the deny-all context turns
  every call into a runtime error.
- `docs/ir-spec.md` §Determinism rules enumerates the IR-level
  invariants that depend on plugins being pure.
- Plan §14 documents determinism as a project-wide invariant.

## Consequences

- Plugin authors who want timestamps, random ids, or external lookup
  data must accept them as config. This is a feature, not a workaround:
  it makes the dependency explicit and forces the generator's caller to
  pin the value, which is what determinism requires.
- The host import surface stays tiny — `log`, `case-convert`. Adding
  more requires a new ADR.
- Some Rust crates that "just work" on host targets fail to build for
  `wasm32-wasip2` because they call into `std::time` or `std::env`
  unconditionally. ADR-0004 documents the plugin-author dev loop that
  surfaces this early.

## Alternatives considered

- **Opt-in clock per plugin manifest.** Rejected — opt-in is still
  non-determinism in practice, because the host cannot tell whether the
  plugin's use of the clock affects its output. Once any plugin in the
  pipeline has a clock, the determinism invariant is gone.
- **Virtual clock seeded from input hash.** Rejected for now — premature
  abstraction with no concrete need. If a real use case appears, an
  explicit host import (e.g. `wasi:clocks` shimmed to return a
  config-supplied epoch) is a small addition behind its own ADR.
- **Filesystem read access scoped to the spec directory.** Rejected —
  plugin authors who need extra inputs should declare them in config.
  Walking the filesystem from inside the plugin couples the plugin to
  the user's directory layout and makes its output non-portable.
