# OpenAPI Forge

A WASM-plugin-based replacement for `openapi-generator`.

The host parses OpenAPI specs into a normalized intermediate representation,
then runs sandboxed WebAssembly plugins that either transform the IR or emit
code from it. Plugins are untrusted by default; the host enforces capability
limits — no filesystem, no network, no clock, no randomness, bounded fuel and
memory.

> **Status: Stage 1 / pre-alpha.** The WIT contract, IR types, and workspace
> are in place. The parser and runtime land in subsequent phases. There is
> no working CLI yet.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/MarcusDunn/openapi-forge/main/install.sh | sh
```

Downloads the right binary for your platform, verifies SHA-256 checksums,
and checks [SLSA build provenance](https://slsa.dev) if `gh` is installed.
Installs to `~/.local/bin` by default.

Pin a version or change the install directory:

```sh
FORGE_VERSION=0.1.11 FORGE_INSTALL_DIR=/usr/local/bin \
  curl -fsSL https://raw.githubusercontent.com/MarcusDunn/openapi-forge/main/install.sh | sh
```

Or install from source via crates.io:

```sh
cargo install openapi-forge-cli
```

## Why

`openapi-generator` (the Swagger project) requires generators to ship in-tree.
Adding or modifying one means a PR upstream, navigating their backwards-
compatibility constraints, and shipping on their release cadence. The result
is a long tail of unmaintained or near-unusable generators with no escape
hatch for users.

OpenAPI Forge inverts that: generators are plugins, parsing is in-tree.
Anyone can publish a generator, and anyone can run one without trusting the
author — the WASM sandbox enforces what the plugin can and can't do. The
host owns parsing, normalization, output validation, and determinism.

## Reading order

- `docs/architecture.md` — for contributors
- `docs/ir-spec.md` — the canonical IR contract
- `docs/plugin-authoring.md` — for plugin authors
- `docs/adr/` — architecture decision records

## Plugin sources

`forge.toml` accepts plugins from the local filesystem or any OCI
registry (Docker Hub, GHCR, ECR, …). Both forms can be mixed freely;
the `config = { ... }` block applies to either.

```toml
[input]
spec = "openapi.json"

# Filesystem ref — useful for in-tree plugin development.
[[transformers]]
wasm = "./plugins/my-transformer.wasm"

# OCI ref — pulled lazily on `forge generate`, cached under the user's
# XDG cache dir. Pin by digest (`@sha256:…`) for airtight reproducibility.
[generator]
oci = "ghcr.io/marcusdunn/typescript-fetch:0.1.0"
config = { packageName = "petstore-client" }

[output]
dir = "out"
```

Pulls are anonymous by default. For private `ghcr.io` packages, log in
with the [GitHub CLI](https://cli.github.com/) and `forge` reuses that
token automatically — no separate configuration. GHCR package reads need
the `read:packages` scope, which the default login does not grant:

```sh
gh auth refresh -h github.com -s read:packages
```

If a private pull is denied, `forge` prints this exact command.

**In CI**, where `gh` may not be installed, set `GH_TOKEN` (or
`GITHUB_TOKEN`) instead — `forge` reads it directly, no `gh` required.
Tokens are checked in the order `GH_TOKEN`, `GITHUB_TOKEN`, then
`gh auth token`.

```yaml
# GitHub Actions
permissions:
  packages: read            # required for the built-in GITHUB_TOKEN
steps:
  - run: forge generate ...
    env:
      # built-in token works for packages the running repo can access;
      # use a PAT with read:packages for other repos/orgs
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

See `docs/adr/0010-oci-plugin-distribution.md` for the cache layout,
accepted layer media types, and roadmap.

### Config-less invocation

For one-off runs (CI scripts, ad-hoc generation), pass the pipeline shape
directly on the command line and skip `forge.toml` entirely:

```sh
forge generate \
  -i openapi.json \
  --transformer ./plugins/my-transformer.wasm \
  --transformer ghcr.io/example/redact:1.2.0 \
  --generator  ghcr.io/marcusdunn/typescript-fetch:0.1.0 \
  -o out
```

`--transformer` is repeatable and runs in order. Each `--transformer` /
`--generator` value is auto-detected as a filesystem path or an OCI ref
(if it ends in `.wasm` or names an existing file, it's a path).
Per-plugin config defaults to `{}`; reach for `forge.toml` when you need
to pass `config = { ... }` blocks.

The full project plan lives in the issue tracker / project documentation;
this README intentionally summarises rather than duplicates.

## Platform support

macOS and Linux. **No Windows.** See plan §2.

## License

Apache-2.0 OR MIT.
