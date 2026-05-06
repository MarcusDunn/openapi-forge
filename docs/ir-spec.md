# IR Specification

The intermediate representation is the contract that everything in the project
depends on: parser produces it, transformers consume and produce it, generators
consume it. Pre-1.0 it changes whenever the design demands; post-1.0 it
freezes.

This document is the canonical description. The WIT in `wit/ir.wit` is the
mechanical encoding. The Rust types in `forge-ir` are the host-side mirror.
When in doubt, this file wins; report drift as a bug.

## Top level

`Ir` carries:

- `info` — title, version, optional description.
- `operations` — every callable endpoint, sorted by `id` for determinism.
- `types` — every named type, **topologically sorted** so a generator emitting
  files in iteration order never references a type before it has been declared.
- `security_schemes` — auth methods declared by the spec.
- `servers` — base URL templates with variables.

There is no `api-version` field. Plugins are bound to a specific
`forge-ir` version through the WIT type system; mismatches surface as load-time
errors.

## Type model

Every named type lives in `Ir.types` keyed by `id`. A `TypeRef` is a `string`
id into that map.

This shape is deliberate (ADR-0006). It has three consequences:

1. **Recursion is free.** Recursive types are just two `NamedType`s that
   reference each other by id. The WIT layer never sees a recursive record.
2. **Inline (anonymous) types are lifted.** A schema declared inline at
   `paths.~1pets.get.responses.200.content.application~1json.schema` becomes a
   `NamedType` with a synthesized id (e.g. `getPetsResponse200Body`). Generators
   never need to handle "is this inline or named?".
3. **Dedup is well-defined.** Two structurally-identical schemas hash to the
   same canonical id during full normalization.

`TypeDef` variants:

| Variant      | Meaning                                                      |
| ------------ | ------------------------------------------------------------ |
| `Primitive`  | A scalar with a `kind` and optional constraints              |
| `Object`     | Properties, `required`, `additional_properties` policy       |
| `Array`      | `items: TypeRef` plus `ArrayConstraints`                     |
| `Map`        | Like `additional_properties: typed`, but as a top-level type |
| `EnumString` | Closed set of string values                                  |
| `EnumInt`    | Closed set of integer values, with `int-32` / `int-64` width |
| `Union`      | `oneOf` / `anyOf`, with optional discriminator               |
| `Null`       | The JSON `null` unit type — see "Nullability" below          |

### Nullability vs optionality

These are **orthogonal**.

- *Optionality* lives on the container: a `Property` is optional iff its `name`
  is absent from `ObjectType.required`. A `Parameter` is optional iff
  `required: false`.
- *Nullability* lives on the type pool: there is no per-`TypeDef` `nullable`
  flag. `T | null` is represented as a `Union` whose variants list contains
  the canonical Null TypeRef, canonicalised to **last position** (issue #107).

The four request-shape states are all distinct and representable:

| `required` | nullable type? | Meaning                                  |
| ---------- | -------------- | ---------------------------------------- |
| true       | false          | must appear, must not be `null`          |
| true       | true           | must appear, may be `null`               |
| false      | false          | may be absent, but if present non-null   |
| false      | true           | may be absent or null                    |

Per-element nullability for arrays and per-value nullability for maps are
*not* a separate axis on the container — the items / values `TypeRef` simply
points at a `Union(T, Null)` when the underlying schema is nullable.

#### The Null type

The canonical Null singleton lives in `Ir.types` under id `"null"` and has
`definition: TypeDef::Null`. It is registered lazily — only specs that
contain at least one nullable schema produce the entry. A user-declared
schema named `null` is renamed (e.g. to `null_2`) with a
`parser/W-RESERVED-NAME` warning.

User-named null aliases (e.g. `components.schemas.Foo: { type: "null" }`)
emit a separate `TypeDef::Null` `NamedType` under the user's id so generators
that emit `type Foo = ...` continue to produce a top-level `Foo`.

Generator helpers `peel_nullable`, `is_null_typeref`, and `union_has_null`
in `forge-plugin-sdk::types_ext` translate the canonical wrap shape back
into "is this nullable, and what's the inner type" at use sites.

#### Canonicalisation invariant

For any `Union` whose variants list contains a reference to the Null
singleton, the Null reference is the **last** entry. The parser enforces
this at construction; the proptest strategies sort post-construction.

### Constraints

`PrimitiveConstraints`, `ArrayConstraints`, `ObjectConstraints` are populated
by the parser whenever the spec carries the corresponding JSON Schema field.
Generators that emit runtime validators have everything they need; those that
don't can ignore them.

Numeric constraints (`minimum`, `maximum`, ...) use the structured `Value`
type, not `f64`. `int64` minima and decimal multiples roundtrip cleanly.

`pattern` is an ECMA-262 regex per JSON Schema. Generators that target
languages with non-ECMA regex engines (Python's `re`, Rust's `regex`) are
responsible for translation or for emitting a diagnostic.

## Extensions

OpenAPI `x-*` extensions are stored as `Vec<(String, Value)>` on every IR
construct that supports them. `Value` is a structured variant which means
plugins read extension data without linking a JSON parser. See ADR-0007.

**Stage 1 limitation.** WIT does not support recursive variants, so `Value`
is currently scalar-only — `null | bool | int | float | string`. Compound
extensions are dropped at the boundary with a diagnostic. Nested-extension
fidelity lands in Phase 3 via a value pool parallel to the type pool. See
the amendment in ADR-0007.

## Operations

`Operation` carries everything one endpoint contributes:

- `id` — unique, sanitized identifier (the spec's `operationId` after cleanup)
- `original_id` — the spec's raw `operationId` if it was provided
- `method`, `path_template` — `GET /pets/{petId}`
- Parameters split by location: `path_params`, `query_params`, `header_params`,
  `cookie_params`
- `request_body` — at most one, with one or more `content` entries (one per
  media type)
- `responses` — by status: `Explicit { code }`, `Default`, or
  `Range { class }` for `2XX`/`3XX`/etc.
- `security` — list of requirements (any-of); each lists scheme ids and scopes
- `tags`, `documentation`, `deprecated`, `extensions`, `location`

## Diagnostics

`Diagnostic` is structured: severity, stable code, message, optional location,
related notes, optional fix suggestion. Codes are namespaced — by convention
`<plugin-name>/E-<KIND>` for errors and `W-<KIND>` for warnings. The host's
parser uses `parser/...`. The format is not yet finalized — see open question
in plan §17.

`SpecLocation` is an RFC 6901 JSON Pointer plus optional file path. Multi-file
specs (external `$ref`) carry `file`.

## Determinism rules

These are invariants the IR must always satisfy. The host validates them after
every stage that returns IR.

1. `operations` is sorted by `id`.
2. `types` is topologically sorted by reference dependency.
3. Every `TypeRef` resolves to a `NamedType.id`.
4. `id`s are unique within `types`.
5. `Vec<(String, V)>` lists (`extensions`, `mapping`, `headers`, `variables`,
   `scopes`) preserve declared order — they are *not* required to be sorted by
   key.
