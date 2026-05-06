# ADR-0009: No static feature-compatibility gate

**Status:** accepted

## Context

The original `plugin-info` carried three lists — `requires`, `forbids`,
`provides` — over a closed `ir-feature` enum. Before running a pipeline
the host walked the chain, maintained an `available` set, and rejected
combinations where a plugin's `requires` was missing or `forbids` was
present.

Two problems surfaced as the system was used:

1. **Confusing semantics.** `provides` did not mean "produces this
   feature in the IR." It meant "handles this feature, so downstream code
   that requires it can run." The polarity tripped readers and showed up
   in our own docstrings.

2. **Cross-language burden.** When `generator-go-server` landed (the
   first non-Rust plugin, see `da21cda`) it became clear that every
   non-Rust SDK would have to either maintain a parallel `IrFeature`
   constant table in lockstep with `forge-ir`, or call a host import in
   every `info()` invocation. That cost only paid off if the static gate
   was carrying weight.

It wasn't. Of five Rust plugins plus one Go plugin, only
`generator-typescript-fetch` populated `provides`, and even that wasn't
load-bearing — every other plugin compiled and ran with all three lists
empty. The "fail fast" benefit of catching incompatible pipelines before
running them was marginal next to the parser cost we'd already paid.

Meanwhile, `wit/stage.wit` already defined `StageError::Rejected
{ reason, diagnostics }` for exactly the case the static gate was trying
to cover. A plugin that rejects with a structured diagnostic can name
the *specific* operation, parameter, or property that broke — strictly
more actionable than "your spec uses `multipart-forms`."

## Decision

Remove the static feature-compatibility system entirely:

- Delete the `ir-feature` enum from `wit/ir.wit`.
- Shrink `plugin-info` to `{ name, version }`.
- Delete `crates/forge-pipeline/src/compatibility.rs` and the compat
  call from `driver::run`.
- Delete the corresponding conversion code in `forge-ir-bindgen` and
  `forge-plugin-sdk`.

Replace it with two existing primitives, no new contract:

- **Hard reject** — `StageError::Rejected { reason, diagnostics }`. Use
  when the plugin cannot produce working code for the input. The
  pipeline aborts before any files are written.
- **Soft warn** — `Severity::Warning` diagnostics in normal output. Use
  when the plugin can produce output but is dropping information.

`docs/plugin-authoring.md` documents the pattern with copyable examples.
`plugins/test-fixtures/generator-strict` (hard reject) and
`plugins/test-fixtures/generator-warn` (soft warn) are minimal worked
examples covered by `crates/forge-plugin-itests/tests/rejection.rs`.

## Rationale

- **Fewer moving parts in the WIT contract.** A two-field `plugin-info`
  is the minimum the host needs to identify a plugin. Anything more
  serves a use case that wasn't being served.
- **No cross-language enum maintenance.** Other-language SDKs match the
  contract one-for-one with no parallel constants.
- **Better error messages.** Plugins inspecting their own input can
  point at exactly which operation broke; the static gate could only
  point at a feature label.
- **Adoption signal.** The system was widely declared but rarely used.
  Removing it loses no behavior anyone relied on.

## Consequences

- Plugin authors are responsible for inspecting the IR they receive and
  rejecting (or warning about) shapes they can't handle. The plugin
  authoring guide gives the recipe.
- Pipeline misconfigurations now fail at the offending stage rather
  than before any stage runs. The cost is one extra wasm invocation;
  the benefit is a diagnostic that names the operation.
- Diagnostic codes (e.g. `my-gen/E-MULTIPART`) are the user-facing
  vocabulary. They are namespaced by plugin, so collisions are impossible.

## Alternatives considered

- **Single `supports: list<ir-feature>` field, parser computes
  `present-features`, host re-derives after each transformer.** Cleaner
  semantics than the three-list system, but inherits the cross-language
  burden and adds parser-side detection logic without removing the need
  for plugins to inspect their input anyway. The redesign was sketched
  out and abandoned in favor of removal.
- **Keep `IrFeature` as an internal tool for spec-introspection
  (`forge inspect spec.yaml`).** Worth doing if introspection becomes a
  feature; not worth keeping the type around speculatively.
- **Provenance** ("the spec originally used `allOf` / external refs").
  A separate concern from in-IR shape. If load-bearing later, model
  explicitly as `spec-traits`, not as a feature enum.
