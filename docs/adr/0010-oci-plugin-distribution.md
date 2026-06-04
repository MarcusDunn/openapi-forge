# ADR-0010: OCI registries as the plugin distribution channel

**Status:** accepted

## Context

`forge.toml` originally only accepted filesystem paths for plugin refs:

```toml
[generator]
wasm = "../../../plugins/target/wasm32-wasip2/release/generator_typescript_fetch.wasm"
```

That's fine for in-tree development, but it forces every consumer of a
generator to vendor the `.wasm` themselves. The README and ADR-0001 both
state that "anyone can publish a generator, and anyone can run one
without trusting the author" — that promise needs a distribution
channel, not just a runtime contract.

The WASM component ecosystem has converged on OCI registries (Docker
Hub, GHCR, ECR, …) as the place to ship components. The Bytecode
Alliance defines `application/vnd.bytecodealliance.wasm.component.layer.v0+wasm`
for component-model layers; `oras push` is the de-facto publishing tool.
Adopting that convention costs us no new infrastructure and lets plugin
authors publish with tooling they already know.

## Decision

`forge.toml` accepts an `oci = "..."` field anywhere a `wasm = "..."`
field used to be valid:

```toml
[generator]
oci = "ghcr.io/marcusdunn/typescript-fetch:0.1.0"
config = { packageName = "petstore-client" }
```

The CLI pulls lazily on `forge generate`. There is no separate
`forge plugin pull` subcommand in v1; the cache makes one unnecessary.

**Library:** `oci-client` (the maintained fork of `oci-distribution`).
We use the raw OCI Distribution Spec client directly, not
`wasm-pkg-client`, because:
- The user-facing concept is "OCI ref," not "wasm-pkg name." A bare
  `ghcr.io/org/plugin:tag` is what users have in their muscle memory.
- `wasm-pkg-client` adds a layer of name resolution we'd have to
  document and configure (`wkg config` / well-known mappings).
- Smaller dependency surface — `oci-client` has roughly half the
  transitive dependency count of `wasm-pkg-client`.

**Cache:** `$XDG_CACHE_HOME/openapi-forge/plugins/`, content-addressed
by sha256 digest, with a small pointer file per tag:

```text
by-digest/sha256/<hex>.wasm        ← canonical, content-addressed
by-tag/<reg>/<repo>/<tag>.digest   ← pointer "sha256:..."
```

Refs pinned by `@sha256:...` skip the network entirely on cache hit.
Tag-pinned refs read the pointer file. Stale tags are accepted as the
cost of the simpler design — pin by digest if you need airtight
reproducibility.

**Auth:** anonymous by default. For `ghcr.io` refs the CLI looks for a
GitHub token in precedence order — `GH_TOKEN`, `GITHUB_TOKEN`, then
`gh auth token` — and, if one is found, authenticates over HTTP Basic
(GHCR exchanges those credentials for a bearer token at its token
endpoint), so private GitHub packages resolve with no extra
configuration. The env vars let CI authenticate without `gh` installed
(GitHub Actions exposes `GITHUB_TOKEN`); the `gh` fallback gives local
shells the "just be logged in" experience and mirrors `gh`'s own env
precedence. If no source yields a token — env unset, `gh` missing or not
logged in — the pull degrades silently to anonymous, so public plugins
keep working without a GitHub login. A `DENIED`/
`UNAUTHORIZED` response on a `ghcr.io` ref is rewritten into an
actionable error pointing at `gh auth refresh -h github.com -s
read:packages` — the `read:packages` scope GHCR requires is not part of
the default `gh auth login` token. Other registries and
`~/.docker/config.json` reading remain deferred to a follow-up.

**Accepted layer media types:**
1. `application/vnd.bytecodealliance.wasm.component.layer.v0+wasm`
2. `application/wasm`
3. `application/vnd.wasm.content.layer.v1+wasm`

Single-layer artifacts whose media type is none of the above are
accepted as a pragmatic relaxation; the wasmtime instantiator will
fail loudly on non-wasm bytes anyway.

## Rationale

- **Distribution is the bottleneck on the "anyone can publish" thesis.**
  Without it, plugin authors are stuck telling users to `git clone` and
  `cargo build`. With it, `oci = "..."` is one line.
- **Determinism is preserved.** The fetch happens before the wasmtime
  engine sees a byte. The sandbox guarantees in ADR-0001 are intact —
  plugins still have no network, no clock, no env, no filesystem.
- **Caching keeps CI fast.** Content-addressed storage means a second
  `forge generate` (or a determinism-job re-run) hits the disk, not the
  network.

## Consequences

- One new network surface in `forge-cli`. It is the only network surface
  in the host workspace (the parser does file I/O, plugins do nothing).
  CVE/audit scope grows by `oci-client` + `reqwest` + `rustls`.
- Plugin authors need a publish workflow. `oras push <ref> <plugin>.wasm:application/vnd.bytecodealliance.wasm.component.layer.v0+wasm`
  is the canonical incantation; `wkg publish` works too.
- The cache can grow without bound. We do not garbage-collect in v1;
  users can `rm -rf ~/.cache/openapi-forge` if needed. Add a
  `forge plugin gc` subcommand if this becomes a real complaint.
- `FORGE_OCI_INSECURE_HOSTS` (comma-separated host[:port] list) opts
  specific registries into plaintext HTTP. Intended for local
  registries in tests and air-gapped CI; documented in the module
  comment and the README, not advertised on the happy path.

## Alternatives considered

- **`wasm-pkg-client`.** Strong argument: it's the BA-blessed tool and
  already speaks OCI under the hood. Rejected because the abstract
  package-name layer adds UX/config surface that the v1 use case
  doesn't need. Easy to migrate later if we want broader registry
  support (warg, etc.).
- **HTTP/HTTPS direct download.** `wasm = "https://..."`. Simpler, but
  no content-addressing, no de-facto signature/digest pinning, and
  ignores the ecosystem's convergence on OCI.
- **`forge plugin install` ahead-of-time.** A separate subcommand that
  manages a lockfile. Worth doing later if reproducibility on tag
  drift becomes a pain point. Pinning by digest covers it for now.
- **Bundling generators in the host.** The thing ADR-0001 explicitly
  rejected. Repeating the rejection for completeness.
