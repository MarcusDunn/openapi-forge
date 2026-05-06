# ADR-0007: Structured extension values, not JSON strings

**Status:** accepted

## Context

OpenAPI specs allow `x-*` extension fields with arbitrary JSON values
attached to nearly every construct. Generators routinely consume these:
`x-go-package`, `x-rust-derives`, `x-stripe-resource`, etc.

The IR needs to carry extension data across the WIT boundary. The natural
options:

1. Pass extensions as JSON strings; let plugins parse them.
2. Pass extensions as structured `value` data, parsed by the host.

## Decision

Extensions are structured. Every place that holds extensions stores
`list<tuple<string, value>>`, where `value` is a closed variant:

```wit
variant value {
    null,
    bool(bool),
    int(s64),
    float(f64),
    string(string),
    list(list<value>),
    object(list<tuple<string, value>>),
}
```

Constraint values that JSON Schema permits to be any-numeric (`minimum`,
`maximum`, `multipleOf`, `exclusiveMinimum`, `exclusiveMaximum`) use the
same `value` type so int64 minima and decimal multiples roundtrip cleanly.

## Rationale

- **No JSON parser per plugin.** A plugin reading `x-foo` no longer links
  `serde_json` (or its language equivalent) into its `.wasm`. For a small
  extension flag, that saves tens of kilobytes and a startup cost. For
  generators in non-Rust languages, it removes a hard requirement.
- **Type safety at the boundary.** Plugins receive `value`s with statically
  known shape. Errors like "this `x-go-package` was a `[]string` instead of
  a `string`" surface as match arms, not as runtime JSON parse failures.
- **Determinism.** Parsing JSON in a plugin would let the plugin observe
  `serde_json`'s ordering (currently insertion-preserving in objects) and
  potentially diverge from the host's interpretation. Structured values pass
  through unambiguously.

The cost is host-side: the parser populates `value`s when it walks the spec.
This is a bookkeeping cost we accept once, instead of imposing a JSON parse
on every plugin.

## Consequences

- Plugins must handle the variant. The SDK provides convenience helpers.
- The parser must convert YAML/JSON values to `Value` faithfully, including
  preserving int-vs-float distinctions where they matter.

## Alternatives considered

- **JSON string per extension.** Rejected for the reasons above.
- **YAML node string.** Same problem plus YAML implementation drift.
- **Two-tier (string for unknown, structured for known).** Rejected as
  unnecessary complexity.

## Amendment — Value pool (implemented)

WIT does not support recursive variants. The `value` definition originally
specified in plan §5.2 includes `list(list<value>)` and `object(...<value>)`
arms which `wasm-tools` rejects with a "type depends on itself" error.

Recursion in the IR's *type* layer is unaffected — types reference each
other through `type-ref = string` ids into `Ir.types`, which is exactly the
indirection ADR-0006 was designed for. Only `value` is affected.

**Implemented form** (post-#107 follow-up): a value pool parallel to the
type pool. `value-ref = u32` indexes into a flat `Ir.values: list<value>`;
the compound arms (`list`, `object`) hold `list<value-ref>` /
`list<tuple<string, value-ref>>` so values never recurse by structure.
Every IR field that used to hold a `Value` now holds a `ValueRef`.

The pool is structurally deduplicated by the parser: pushing a `Value`
that is already present at index `i` returns `i` (saves space when many
properties default to the same constant).

Plugin-facing API: `forge_plugin_sdk::values_ext` provides `resolve`,
`resolve_to_serde`, `to_json_compact`, `to_json_pretty` so plugins
materialise tree-shaped representations on demand without threading the
pool through their own logic. Plugins still do not link a JSON parser
to *read* values — they consume the structured `Value` enum.

The previous Stage 1 `W-DEFAULT-DROPPED` and `W-EXTENSION-DROPPED`
warnings are gone — compound defaults and compound `x-*` extensions
now survive the WIT boundary intact.
