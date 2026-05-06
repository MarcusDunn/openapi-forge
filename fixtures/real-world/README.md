# Real-world fixtures

Each subdirectory holds an OpenAPI 3.0 spec **modeled after** a real
production API — Stripe, GitHub, etc. — but hand-curated rather than
copied verbatim. The goal: exercise the parser + both generators against
the kinds of shapes real specs use, while keeping each fixture small
enough to commit, fast enough to iterate on, and within feature coverage
that openapi-forge supports today.

## Why hand-curated, not upstream-snapshot?

Upstream specs (Stripe's `spec3.json`, GitHub's `api.github.com.json`)
are 7–12 MB monoliths. Even trimming to a single endpoint pulls hundreds
of transitively-referenced schemas. Hand-curating gets the same
**structural** properties — pagination, $ref chains, allOf composition,
discriminated unions, security schemes, formatted strings — at 1–2% of
the file size, with no licensing question and no fragile auto-trimming
script.

## Policy when something doesn't compile

The MVP-gate test (`crates/forge-plugin-itests/tests/real_world.rs`,
gated on `--features real-world`) runs each fixture through both
generators and asserts the output compiles (`tsc --noEmit` for TS,
`cargo check` for Rust).

If a feature in a fixture can't be parsed cleanly or doesn't compile:

- **Trim** that feature out of the fixture, AND
- **File a follow-up issue** for the parser / generator gap.

Never expand IR / parser / generator scope inside the real-world PR.

## Fixtures today

- `stripe-customers/` — `/v1/customers` (list, create, retrieve, update,
  delete) and `/v1/charges` (list, retrieve). Bearer auth, paginated
  list envelopes (`{ object, data, has_more, url }`), `allOf` for the
  update-params type, nullable formatted strings (`email`, `uri`).
- `github-issues/` — `/repos/{owner}/{repo}/issues` (list, create) plus
  get / update / list-comments. Bearer auth, lots of `nullable: true`
  in nested user/milestone refs, `date-time` formats, string enums for
  state and reactions.
