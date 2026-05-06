# ADR-0006: String ids and type map for IR references

**Status:** accepted

## Context

The IR has to encode references between types: an array's element type, an
object's property types, a union's variants, a discriminator's mapping. There
are several plausible encodings:

1. **Inline recursion**: `TypeDef::Array { items: Box<TypeDef> }`.
2. **Indexed handle**: `TypeRef(u32)` indexing into `Ir.types`.
3. **String id**: `TypeRef = String`, with names looked up in `Ir.types`.

WIT does not allow cyclic record definitions, so option 1 doesn't survive the
boundary anyway. The choice is between indexed handles and string ids.

## Decision

`TypeRef = String`. Every named type lives in `Ir.types` keyed by `id`. Inline
(anonymous) types are lifted into `Ir.types` during parsing with synthesized
ids. Recursion is encoded as two named types referring to each other.

## Rationale

- **Stable across normalization.** Sorting `Ir.types` topologically, deduping
  structurally-identical types, or filtering operations with a tag-filter
  transformer can all reorder or remove entries. Indexed handles would require
  every transformer to recompute indices on output. String ids survive any
  reordering trivially.
- **Debuggable.** A debug-dump of the IR with `getPetsResponse200Body` is
  legible; with `#42` it is not. Generators that emit comments referencing the
  source schema get human-readable names for free.
- **Cross-stage stability.** Diagnostics emitted by stage 1 referencing type
  `Foo` remain valid after stage 2 reorders or renames the type map, as long
  as the plugin uses ids consistently.
- **WIT-friendly.** `string` is a primitive; `list<u32>` of indices would
  also work but loses the above properties.

The cost is one `HashMap` lookup per dereference. Negligible compared to
codegen output formatting.

## Consequences

- Inline schemas need synthesized ids. The parser owns id generation.
- Every IR-returning stage runs `validate_refs` (`forge-ir-bindgen`) to
  guarantee no dangling references. A dangling ref is a `plugin-bug` error.
- Two passes of normalization: light (before transformers — deref `$ref`,
  resolve compositions) and full (after transformers — sanitize identifiers,
  dedup, topo-sort).

## Alternatives considered

- **Indexed handles**: rejected for the reordering reason above.
- **Hybrid (id + index hint)**: rejected as redundant.
- **Recursive WIT types**: not supported by WIT.
