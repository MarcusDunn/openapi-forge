# ADR-0005: Separate Cargo workspace for plugins

**Status:** accepted

## Context

The repo contains both the host (`forge-cli`, `forge-host`,
`forge-pipeline`, …) and the in-tree plugins shipped with it
(`transformer-noop`, `generator-typescript-fetch`, …). The host builds
for the developer's native target; plugins build for `wasm32-wasip2`.

The obvious layout — one Cargo workspace at the root with both host
crates and plugin crates as members — does not work, because Cargo
performs **feature unification** per workspace per resolver run.

## Decision

The repo has two Cargo workspaces:

- **Host workspace** at the repo root. Members are the `forge-*` crates
  the CLI needs.
- **Plugin workspace** at `plugins/`. Members are the in-tree plugins.

The root workspace `exclude`s `plugins/` (and `crates/forge-plugin-sdk`,
which is plugin-only and refuses to build for the host — see ADR-0004).

## Rationale

Cargo's resolver computes a single feature set for each crate in a
workspace, taking the union of features requested across all members.
That union is then the feature set the crate is built with for *every*
target the workspace is asked to build for.

When host and plugins live in the same workspace:

- Some shared dep — e.g. anything that pulls in `tokio`, `mio`,
  `socket2`, `getrandom`, or `ring` — has different feature requirements
  on host vs. wasm. The host enables the "real" features; the plugin
  needs the cut-down `wasm32` features.
- Cargo unifies them, then tries to build the dep for `wasm32-wasip2`
  with features that require `mio` (or another non-wasm crate). The
  build fails.

There is no per-target feature unification in stable Cargo. The
practical workaround is to give each target its own workspace so each
gets its own resolver run.

## Code references

- Root `Cargo.toml`: `[workspace] members = [...]` and
  `exclude = ["crates/forge-plugin-sdk", "plugins"]`.
- `plugins/Cargo.toml`: independent `[workspace]` declaration with its
  own `[workspace.dependencies]` table.
- `crates/forge-test-harness/src/lib.rs::build` shells out to
  `cargo build --release --target wasm32-wasip2 --manifest-path
  <plugin>/Cargo.toml`, then `locate_artifact` walks up looking for
  `target/wasm32-wasip2/release/<name>.wasm`. This is how host-side
  integration tests reach across the workspace boundary.

## Consequences

- The dev loop has two `cargo build` invocations: one for the host
  workspace, one for the plugin workspace. CI handles this by building
  plugins first (`.github/workflows/ci.yml` "Plugin .wasm artifacts
  must exist before host integration tests run") so `forge-host` and
  `forge-cli` integration tests find their fixtures.
- Plugin crates cannot use `path = "../../crates/forge-ir"` to depend on
  host crates directly — they go through `forge-plugin-sdk`, which
  re-exports the IR types and is itself outside both workspaces.
- `cargo workspace` commands at the root never touch `plugins/`. Anyone
  running `cargo fmt --all` or `cargo clippy --workspace` needs to
  remember the second workspace. CI runs both.

## Alternatives considered

- **Single workspace with `default-members`.** Rejected — `default-members`
  changes which crates `cargo build` defaults to, but feature unification
  still applies across every workspace member. The root cause is not
  scope, it's resolver semantics.
- **Single workspace, split deps via `cfg(target_arch)`.** Rejected —
  `cfg`-gated deps in `Cargo.toml` are honored at build time, but
  feature unification still happens across the whole workspace and
  surfaces the same conflicts.
- **One Cargo workspace per plugin.** Rejected — scales badly as the
  plugin count grows. A single shared `[workspace.dependencies]` table
  in `plugins/Cargo.toml` is exactly what's needed.
- **Move plugins to a separate repo.** Rejected for now — keeps in-tree
  plugins easy to refactor in lockstep with the IR. Will revisit if the
  plugin set grows large enough that monorepo overhead exceeds the
  workspace-split overhead.
