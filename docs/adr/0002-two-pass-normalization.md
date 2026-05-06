# ADR-0002: Two-pass normalization

**Status:** accepted

## Context

The pipeline runs `parser → transformers → generator` (see plan §4 and
`docs/architecture.md` §Pipeline). Both the parser and the generator need
the IR to be in a canonical shape — `$ref`s resolved, `allOf` flattened,
type ids unique, types topologically sorted by dependency — but the IR
that *transformers* see is open: a transformer may rename types, add new
ones, drop unreachable ones, or splice in extensions.

Two reasonable places to normalize:

- **Before transformers**, so transformers always see clean, canonical IR.
- **After transformers**, so the generator always sees clean, canonical IR.

Picking one or the other forces a tradeoff. Normalize only before:
transformers can desync the IR (introduce duplicate ids, dangling refs,
out-of-order types) and the generator inherits the mess. Normalize only
after: every transformer has to handle raw `$ref` graphs and
allOf-composed objects defensively, reimplementing parser logic to do
even simple work.

## Decision

Normalization runs in **two passes**, not one.

- **Light pass — parser-side, before transformers.** Resolves `$ref`,
  flattens `allOf` (eager normalization, plan §3), lifts inline schemas
  into the type pool with synthesized ids. The IR a transformer sees has
  no remaining composition operators and no unresolved references.
- **Full pass — host-side, after the last transformer, before the
  generator.** Sanitizes synthesized identifiers, deduplicates types
  emitted by transformers, validates that every `TypeRef` resolves,
  topologically sorts `types`, and sorts `operations` by id. These are
  the determinism invariants documented in `docs/ir-spec.md`
  §Determinism rules.

## Rationale

Transformers can operate on a single `ObjectType` per declared schema
instead of reimplementing `$ref` walks and `allOf` merging. That's the
load-bearing reason: most transformers want to inspect or rewrite shapes,
not navigate composition graphs.

The full pass runs *after* the last transformer because a transformer can
introduce duplicates, dangling refs, or re-order. Running dedup or
topo-sort earlier would either force every transformer to redo it (plan
§3) or make it impossible for transformers to legitimately rename or add
types.

## Code references

`crates/forge-parser/src/finalize.rs::canonicalize` performs the second
pass today — sort by id, validate `TypeRef` resolution, topo-sort with
alphabetical tiebreak (Kahn's algorithm). It runs at parser exit. As
transformers grow, this same routine will be invoked again after the
transformer chain in `crates/forge-pipeline/src/driver.rs::run` so the
generator sees a canonical IR regardless of what the transformers did.

## Consequences

- Transformers do not need to handle raw `$ref` or `allOf`. They may
  assume objects are flattened.
- Transformers *may* introduce duplicate ids or dangling refs; the host
  is responsible for detecting and either fixing or surfacing them in
  the second pass.
- The cost of the second pass is paid even when no transformers ran.
  This is acceptable — `canonicalize` is O(n log n) over a small n.

## Alternatives considered

- **Single pass, pre-transformers only.** Rejected — generators would
  receive whatever shape the transformer chain produced, including
  duplicates and cycles, and would crash or emit non-deterministic
  output.
- **Single pass, post-transformers only.** Rejected — transformers
  would have to reimplement `$ref` resolution and `allOf` flattening to
  do anything useful with object schemas. The compatibility surface
  would explode: every transformer's behaviour would depend on which
  composition shapes the parser left in.
- **Three passes (light, mid, full).** Rejected — no concrete need.
  The two-pass split is sufficient because everything the *light* pass
  produces is composition-free, and everything the *full* pass cleans up
  is observable only post-transformer.
