# ADR-0008: Plugins receive only IR and config — nothing else

**Status:** accepted

## Context

Plugins are pure functions: `(ir, config) → ir + diagnostics` for
transformers, `(ir, config) → files + diagnostics` for generators.

There are recurring requests for additional plugin inputs:

- "What TypeScript version is the user targeting?"
- "Is this a CI build or a local build?"
- "What's the current git revision?"
- "What is `$RUST_VERSION`?"
- "What output directory was given on the command line?"

Each on its own seems harmless. The cumulative effect is a plugin that
behaves differently across runs, environments, machines, and developers.

## Decision

The host provides plugins with exactly two inputs:

1. The IR (a value that round-trips deterministically).
2. The config string (a JSON document the host validated against the
   plugin's declared schema).

Nothing else reaches a plugin. No environment variables, no clock, no
network, no filesystem, no host context, no "what version of TypeScript do
you want" knob from the CLI, no command-line passthrough.

When a plugin must behave differently across configurations — TS 4 vs TS 5,
strict vs lenient — those are **separate plugin artifacts**, not runtime
knobs. `generator-typescript-fetch-strict` and
`generator-typescript-fetch-lenient` are two distinct `.wasm` files.

## Rationale

- **Determinism.** A plugin invoked twice with the same IR and config
  produces byte-identical output. No exceptions. CI can verify this with a
  diff. Hidden inputs would break the property silently.
- **Reproducibility.** A user opening a bug report against a plugin says
  "spec X plus config Y produced output Z". That's the entire reproduction.
  The plugin author doesn't need to ask for env dumps or system info.
- **Slippery-slope avoidance.** The first knob ("just one env var, please")
  becomes the tenth and then the fiftieth. Plugins start papering over spec
  problems with "well, in CI we want it different." The right place for that
  branching is in the spec or the config, not in plugin runtime state.
- **Trust boundary clarity.** The host's threat model is clean: plugins see
  IR and config. Anything else would require a sandbox audit.

## Consequences

- The CLI does not pass through env vars or command-line "context" flags to
  plugins.
- Plugin variants for different targets are separate builds. Distribution
  metadata (tags, descriptions) communicates which is which.
- Configuration that legitimately varies across runs goes in the config
  file. The plugin's JSON Schema declares it; the host validates.

## Alternatives considered

- **A small whitelist of "context" inputs (e.g. CI / non-CI).** Rejected —
  the line is impossible to draw consistently. Once the host hands plugins
  *anything* beyond IR and config, every plugin author has a reason their
  case is special.
- **Plugin-author-declared host imports.** Considered briefly; rejected
  because every additional import widens the trust boundary, and we would
  end up with a permission system inside a permission system.
