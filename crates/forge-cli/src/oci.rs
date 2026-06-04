//! Pull WASM-component plugins from OCI registries.
//!
//! This module is the only network surface in `forge-cli`. The host
//! sandbox forbids plugins from doing any I/O, but the *act of fetching*
//! the plugin happens here, before the wasmtime engine ever sees the
//! bytes. After fetch, the bytes flow into `Plugin::load_*` exactly as
//! they would for a filesystem ref.
//!
//! The user-facing surface is one function: [`fetch_to_bytes`]. It
//! parses an OCI reference, consults a content-addressed cache under
//! the user's XDG cache dir, and pulls from the registry on miss. Pulls
//! are anonymous in v1 — see ADR-0010.
//!
//! Cache layout (`$XDG_CACHE_HOME/openapi-forge/plugins/`):
//!
//! ```text
//! by-digest/
//!   sha256/
//!     <hex>.wasm                    ← canonical, content-addressed
//! by-tag/
//!   <registry>/<repo>/<tag>.digest  ← tiny pointer file with "sha256:..."
//! ```
//!
//! Refs pinned by `@sha256:...` skip the network entirely on cache hit.
//! Tag-pinned refs read the pointer file and verify the blob still
//! exists; this is intentionally simple and accepts that a tag could
//! have been re-pushed in the registry without the cache noticing.
//! Pin by digest if you want airtight reproducibility.
//!
//! ## Auth
//!
//! Pulls are anonymous by default. For `ghcr.io` refs we look for a
//! GitHub token in precedence order — `GH_TOKEN`, `GITHUB_TOKEN`, then
//! `gh auth token` — and, if one is found, authenticate over HTTP Basic
//! so private GitHub packages resolve. The env vars let CI authenticate
//! without `gh` installed; the `gh` fallback gives local shells the
//! "just be logged in" experience. If no source yields a token (env
//! unset, `gh` missing or not logged in), the pull degrades silently to
//! anonymous — public plugins keep working with no GitHub login. See
//! ADR-0010.

use std::path::{Path, PathBuf};

use oci_client::{
    client::{ClientConfig, ClientProtocol},
    manifest::OciImageManifest,
    secrets::RegistryAuth,
    Client, Reference,
};
use sha2::{Digest, Sha256};

/// Registry whose private packages we can unlock with a GitHub token
/// (from the environment or the `gh` CLI).
const GHCR_REGISTRY: &str = "ghcr.io";

/// Username sent alongside the `gh` token over HTTP Basic. GHCR's token
/// endpoint validates only the password (the token) and ignores the
/// username, so this is a descriptive placeholder mirroring git's
/// token-as-password convention.
const GHCR_TOKEN_USERNAME: &str = "x-access-token";

/// Layer media types we accept as carrying a single WASM component.
/// First match wins; order is informational only.
const ACCEPTED_MEDIA_TYPES: &[&str] = &[
    // Bytecode Alliance convention for component-model wasm layers.
    "application/vnd.bytecodealliance.wasm.component.layer.v0+wasm",
    // Generic wasm. Used by `oras push` defaults and several existing
    // registries.
    "application/wasm",
    // Wider OCI-Wasm proposal media type some pushers use.
    "application/vnd.wasm.content.layer.v1+wasm",
];

#[derive(Debug, thiserror::Error)]
pub enum OciError {
    #[error("invalid OCI reference {reference:?}: {source}")]
    BadRef {
        reference: String,
        #[source]
        source: oci_client::ParseError,
    },
    #[error("could not determine cache directory")]
    NoCacheDir,
    #[error("cache I/O at {path}: {source}")]
    CacheIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("registry pull: {0}")]
    Registry(#[from] oci_client::errors::OciDistributionError),
    #[error(
        "access denied by ghcr.io. If this is a private package, log in with the \
         GitHub CLI and ensure your token carries the `read:packages` scope:\n    \
         gh auth refresh -h github.com -s read:packages\n  \
         (or `gh auth login` if you have not authenticated yet)"
    )]
    GhcrAccessDenied {
        reference: String,
        #[source]
        source: oci_client::errors::OciDistributionError,
    },
    #[error(
        "no acceptable wasm layer in {reference}: \
         expected one of [{}], got [{got}]",
        ACCEPTED_MEDIA_TYPES.join(", ")
    )]
    NoWasmLayer { reference: String, got: String },
    #[error("digest mismatch for {reference}: expected {expected}, got {actual}")]
    DigestMismatch {
        reference: String,
        expected: String,
        actual: String,
    },
    #[error("unsupported digest algorithm in {0}: only sha256 is supported")]
    UnsupportedDigestAlgo(String),
    #[error("async runtime: {0}")]
    Runtime(std::io::Error),
}

/// Fetch the wasm component bytes for an OCI reference, populating the
/// on-disk cache. Returns the bytes ready to hand to `Plugin::load_*`.
pub fn fetch_to_bytes(reference: &str) -> Result<Vec<u8>, OciError> {
    let parsed: Reference = reference.parse().map_err(|e| OciError::BadRef {
        reference: reference.to_owned(),
        source: e,
    })?;

    let cache_root = cache_root()?;

    // Fast path: ref pinned by digest, blob already on disk.
    if let Some(digest) = parsed.digest() {
        if let Some(bytes) = read_blob_by_digest(&cache_root, digest)? {
            tracing::debug!(target: "forge::oci", %reference, "cache hit (digest-pinned)");
            return Ok(bytes);
        }
    } else if let Some(tag) = parsed.tag() {
        // Tag-pinned: consult the pointer file, then the blob store.
        if let Some(digest) =
            read_tag_pointer(&cache_root, parsed.registry(), parsed.repository(), tag)?
        {
            if let Some(bytes) = read_blob_by_digest(&cache_root, &digest)? {
                tracing::debug!(target: "forge::oci", %reference, %digest, "cache hit (tag-pinned)");
                return Ok(bytes);
            }
        }
    }

    // Cache miss. Pull from the registry.
    // Resolve auth synchronously (it may shell out to `gh`) before
    // entering the async runtime, keeping the subprocess off the executor.
    let auth = resolve_auth(parsed.registry());

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(OciError::Runtime)?;
    let pulled = runtime.block_on(pull(&parsed, &auth))?;

    // If the caller pinned by digest, validate it before trusting the
    // bytes. The OCI client validates layer digests against the manifest,
    // but we want to validate against the *user-supplied* digest too.
    if let Some(expected) = parsed.digest() {
        let actual = layer_digest(&pulled.bytes);
        if !digests_equal(expected, &actual) {
            return Err(OciError::DigestMismatch {
                reference: reference.to_owned(),
                expected: expected.to_owned(),
                actual,
            });
        }
    }

    write_blob(&cache_root, &pulled.layer_digest, &pulled.bytes)?;
    if let Some(tag) = parsed.tag() {
        write_tag_pointer(
            &cache_root,
            parsed.registry(),
            parsed.repository(),
            tag,
            &pulled.layer_digest,
        )?;
    }

    tracing::info!(
        target: "forge::oci",
        %reference,
        digest = %pulled.layer_digest,
        bytes = pulled.bytes.len(),
        "pulled plugin from registry",
    );
    Ok(pulled.bytes)
}

struct Pulled {
    bytes: Vec<u8>,
    /// Digest of the wasm layer, e.g. `sha256:...`. Used as the cache key.
    layer_digest: String,
}

async fn pull(reference: &Reference, auth: &RegistryAuth) -> Result<Pulled, OciError> {
    let client = Client::new(ClientConfig {
        protocol: configured_protocol(),
        ..ClientConfig::default()
    });
    let image = match client
        .pull(reference, auth, ACCEPTED_MEDIA_TYPES.to_vec())
        .await
    {
        Ok(image) => image,
        // Turn an opaque 403 on a ghcr.io ref into an actionable hint:
        // private packages need a `gh` token with the `read:packages`
        // scope, which the default `gh auth login` token lacks.
        Err(e) if reference.registry() == GHCR_REGISTRY && is_access_denied(&e) => {
            return Err(OciError::GhcrAccessDenied {
                reference: reference.to_string(),
                source: e,
            });
        }
        Err(e) => return Err(e.into()),
    };

    let layer = pick_wasm_layer(&image.layers, &image.manifest, &reference.to_string())?;
    let bytes = layer.data.to_vec();
    let layer_digest = layer_digest(&bytes);
    Ok(Pulled {
        bytes,
        layer_digest,
    })
}

/// Honours `FORGE_OCI_INSECURE_HOSTS` (comma-separated `host[:port]`
/// list) to opt specific registries into plaintext HTTP. Default is
/// HTTPS for everything. Intended for local registries in tests/CI; do
/// not point this at production registries.
fn configured_protocol() -> ClientProtocol {
    match std::env::var("FORGE_OCI_INSECURE_HOSTS") {
        Ok(s) if !s.trim().is_empty() => {
            let list: Vec<String> = s.split(',').map(|x| x.trim().to_owned()).collect();
            ClientProtocol::HttpsExcept(list)
        }
        _ => ClientProtocol::Https,
    }
}

/// Pick the registry credentials for `registry`. Anonymous for
/// everything except `ghcr.io`, where we try the `gh` CLI so private
/// GitHub packages resolve without the user wiring up a separate token.
fn resolve_auth(registry: &str) -> RegistryAuth {
    auth_from_token(registry, ghcr_token(registry))
}

/// Pure mapping from `(registry, token)` to a `RegistryAuth`, factored
/// out of [`resolve_auth`] so the policy is unit-testable without a
/// real `gh` on `PATH`.
fn auth_from_token(registry: &str, token: Option<String>) -> RegistryAuth {
    match (registry, token) {
        (GHCR_REGISTRY, Some(token)) => RegistryAuth::Basic(GHCR_TOKEN_USERNAME.to_owned(), token),
        _ => RegistryAuth::Anonymous,
    }
}

/// Best-effort GitHub token for `ghcr.io`, in precedence order:
/// `GH_TOKEN`, then `GITHUB_TOKEN`, then `gh auth token`. The env vars
/// let CI authenticate without `gh` installed (and mirror `gh`'s own
/// precedence, so a local shell that overrides one keeps doing so).
/// Returns `None` — never an error — for non-GHCR registries or when no
/// source yields a token; auth is optional and a miss just means an
/// anonymous pull.
fn ghcr_token(registry: &str) -> Option<String> {
    if registry != GHCR_REGISTRY {
        return None;
    }
    token_from_env().or_else(token_from_gh_cli)
}

/// First non-empty value of `GH_TOKEN`, then `GITHUB_TOKEN`.
fn token_from_env() -> Option<String> {
    first_token([
        std::env::var("GH_TOKEN").ok(),
        std::env::var("GITHUB_TOKEN").ok(),
    ])
}

/// First candidate that is present and non-blank, trimmed. Pure helper
/// so the precedence rule is testable without touching process env.
fn first_token(candidates: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    candidates
        .into_iter()
        .flatten()
        .map(|t| t.trim().to_owned())
        .find(|t| !t.is_empty())
}

/// Token from `gh auth token`. `None` if `gh` is missing, the user is
/// not logged in, or the output is empty.
fn token_from_gh_cli() -> Option<String> {
    let output = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Does this pull error mean "the registry refused you," as opposed to
/// a transport/parse/not-found failure? Used to decide whether to attach
/// the ghcr.io auth hint. Covers both the bearer-exchange failure and a
/// `DENIED`/`UNAUTHORIZED` code in the registry's error envelope.
fn is_access_denied(err: &oci_client::errors::OciDistributionError) -> bool {
    use oci_client::errors::{OciDistributionError as E, OciErrorCode};
    match err {
        E::UnauthorizedError { .. } | E::AuthenticationFailure(_) => true,
        E::RegistryError { envelope, .. } => envelope
            .errors
            .iter()
            .any(|e| matches!(e.code, OciErrorCode::Denied | OciErrorCode::Unauthorized)),
        _ => false,
    }
}

fn pick_wasm_layer<'a>(
    layers: &'a [oci_client::client::ImageLayer],
    _manifest: &Option<OciImageManifest>,
    reference: &str,
) -> Result<&'a oci_client::client::ImageLayer, OciError> {
    for accepted in ACCEPTED_MEDIA_TYPES {
        if let Some(l) = layers.iter().find(|l| l.media_type == *accepted) {
            return Ok(l);
        }
    }
    // If the ref pulled exactly one layer, accept it regardless — `oras
    // push --artifact-type ...` defaults vary across tools and we
    // already filtered by `ACCEPTED_MEDIA_TYPES` at the pull call (the
    // server may have sent the layer through anyway). This is a
    // pragmatic relaxation; the load step will fail loudly if the bytes
    // aren't a real wasm component.
    if layers.len() == 1 {
        return Ok(&layers[0]);
    }
    let got = layers
        .iter()
        .map(|l| l.media_type.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(OciError::NoWasmLayer {
        reference: reference.to_owned(),
        got,
    })
}

// ── cache I/O ────────────────────────────────────────────────────────────

fn cache_root() -> Result<PathBuf, OciError> {
    let base = if let Ok(env) = std::env::var("FORGE_CACHE_DIR") {
        PathBuf::from(env)
    } else {
        dirs::cache_dir().ok_or(OciError::NoCacheDir)?
    };
    Ok(base.join("openapi-forge").join("plugins"))
}

fn blob_path(cache_root: &Path, digest: &str) -> Result<PathBuf, OciError> {
    let (algo, hex) = digest
        .split_once(':')
        .ok_or_else(|| OciError::UnsupportedDigestAlgo(digest.to_owned()))?;
    if algo != "sha256" {
        return Err(OciError::UnsupportedDigestAlgo(digest.to_owned()));
    }
    Ok(cache_root
        .join("by-digest")
        .join(algo)
        .join(format!("{hex}.wasm")))
}

fn tag_pointer_path(cache_root: &Path, registry: &str, repository: &str, tag: &str) -> PathBuf {
    cache_root
        .join("by-tag")
        .join(sanitize(registry))
        .join(sanitize(repository))
        .join(format!("{}.digest", sanitize(tag)))
}

/// Strip path separators so registry/repo segments become single
/// directory names. Collisions are technically possible (e.g.
/// `foo/bar` vs `foo_bar`) but harmless: the canonical truth is the
/// blob store keyed by digest.
fn sanitize(s: &str) -> String {
    s.replace(['/', '\\'], "_")
}

fn read_blob_by_digest(cache_root: &Path, digest: &str) -> Result<Option<Vec<u8>>, OciError> {
    let path = blob_path(cache_root, digest)?;
    match std::fs::read(&path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(OciError::CacheIo { path, source }),
    }
}

fn write_blob(cache_root: &Path, digest: &str, bytes: &[u8]) -> Result<(), OciError> {
    let path = blob_path(cache_root, digest)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| OciError::CacheIo {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let tmp = path.with_extension("wasm.tmp");
    std::fs::write(&tmp, bytes).map_err(|source| OciError::CacheIo {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, &path).map_err(|source| OciError::CacheIo {
        path: path.clone(),
        source,
    })
}

fn read_tag_pointer(
    cache_root: &Path,
    registry: &str,
    repository: &str,
    tag: &str,
) -> Result<Option<String>, OciError> {
    let path = tag_pointer_path(cache_root, registry, repository, tag);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s.trim().to_owned())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(OciError::CacheIo { path, source }),
    }
}

fn write_tag_pointer(
    cache_root: &Path,
    registry: &str,
    repository: &str,
    tag: &str,
    digest: &str,
) -> Result<(), OciError> {
    let path = tag_pointer_path(cache_root, registry, repository, tag);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| OciError::CacheIo {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&path, digest).map_err(|source| OciError::CacheIo {
        path: path.clone(),
        source,
    })
}

fn layer_digest(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{}", hex::encode(h.finalize()))
}

fn digests_equal(a: &str, b: &str) -> bool {
    // Normalise: both should be `algo:hex`. Compare case-insensitively
    // on the hex portion to be lenient about uppercase digests.
    let (a_algo, a_hex) = a.split_once(':').unwrap_or(("", a));
    let (b_algo, b_hex) = b.split_once(':').unwrap_or(("", b));
    a_algo == b_algo && a_hex.eq_ignore_ascii_case(b_hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci_client::manifest::{OciDescriptor, OciImageManifest};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parses_typical_ref() {
        let r: Reference = "ghcr.io/marcusdunn/typescript-fetch:0.1.0".parse().unwrap();
        assert_eq!(r.registry(), "ghcr.io");
        assert_eq!(r.repository(), "marcusdunn/typescript-fetch");
        assert_eq!(r.tag(), Some("0.1.0"));
        assert_eq!(r.digest(), None);
    }

    #[test]
    fn parses_digest_pinned_ref() {
        let s =
            "ghcr.io/x/y@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let r: Reference = s.parse().unwrap();
        assert!(r.digest().unwrap().starts_with("sha256:"));
    }

    #[test]
    fn blob_path_layout() {
        let root = PathBuf::from("/tmp/x");
        let p = blob_path(&root, "sha256:deadbeef").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/x/by-digest/sha256/deadbeef.wasm"));
    }

    #[test]
    fn rejects_non_sha256_digest() {
        let root = PathBuf::from("/tmp/x");
        assert!(matches!(
            blob_path(&root, "sha512:abc"),
            Err(OciError::UnsupportedDigestAlgo(_))
        ));
        assert!(matches!(
            blob_path(&root, "no-colon"),
            Err(OciError::UnsupportedDigestAlgo(_))
        ));
    }

    #[test]
    fn tag_pointer_layout_sanitises_slashes() {
        let root = PathBuf::from("/tmp/x");
        let p = tag_pointer_path(&root, "ghcr.io", "owner/repo", "1.0.0");
        assert_eq!(
            p,
            PathBuf::from("/tmp/x/by-tag/ghcr.io/owner_repo/1.0.0.digest")
        );
    }

    #[test]
    fn ghcr_token_maps_to_basic_auth() {
        let auth = auth_from_token("ghcr.io", Some("ghp_secret".to_owned()));
        assert_eq!(
            auth,
            RegistryAuth::Basic(GHCR_TOKEN_USERNAME.to_owned(), "ghp_secret".to_owned())
        );
    }

    #[test]
    fn ghcr_without_token_is_anonymous() {
        assert_eq!(auth_from_token("ghcr.io", None), RegistryAuth::Anonymous);
    }

    #[test]
    fn non_ghcr_registry_is_anonymous_even_with_token() {
        // We only ever surface a token for ghcr.io, but guard the policy
        // anyway: a token must never leak to a different registry.
        assert_eq!(
            auth_from_token("docker.io", Some("ghp_secret".to_owned())),
            RegistryAuth::Anonymous
        );
    }

    #[test]
    fn ghcr_token_skips_non_ghcr_registries() {
        // Must short-circuit before reading env or invoking `gh`.
        assert_eq!(ghcr_token("docker.io"), None);
    }

    #[test]
    fn first_token_picks_first_non_blank_trimmed() {
        // Empty / whitespace-only candidates are skipped; the winner is
        // trimmed. This is the GH_TOKEN-then-GITHUB_TOKEN precedence.
        assert_eq!(first_token([None, None]), None);
        assert_eq!(
            first_token([Some("".to_owned()), Some("  ".to_owned())]),
            None
        );
        assert_eq!(
            first_token([Some("  gh_a  ".to_owned()), Some("gh_b".to_owned())]),
            Some("gh_a".to_owned())
        );
        assert_eq!(
            first_token([Some("   ".to_owned()), Some("gh_b".to_owned())]),
            Some("gh_b".to_owned())
        );
    }

    #[test]
    fn denied_envelope_is_access_denied() {
        use oci_client::errors::{OciDistributionError, OciEnvelope, OciError, OciErrorCode};
        let err = OciDistributionError::RegistryError {
            url: "https://ghcr.io/v2/org/pkg/manifests/latest".to_owned(),
            envelope: OciEnvelope {
                errors: vec![OciError {
                    code: OciErrorCode::Denied,
                    message: "requested access to the resource is denied".to_owned(),
                    detail: serde_json::Value::Null,
                }],
            },
        };
        assert!(is_access_denied(&err));
    }

    #[test]
    fn unauthorized_error_is_access_denied() {
        use oci_client::errors::OciDistributionError;
        let err = OciDistributionError::UnauthorizedError {
            url: "https://ghcr.io/v2/org/pkg/manifests/latest".to_owned(),
        };
        assert!(is_access_denied(&err));
    }

    #[test]
    fn not_found_is_not_access_denied() {
        use oci_client::errors::OciDistributionError;
        // A genuine 404 / missing-manifest must not masquerade as an
        // auth problem — that would send users chasing a scope they
        // already have.
        let err = OciDistributionError::ImageManifestNotFoundError("nope".to_owned());
        assert!(!is_access_denied(&err));
    }

    #[test]
    fn digests_equal_normalises_case() {
        assert!(digests_equal("sha256:ABCDEF", "sha256:abcdef"));
        assert!(!digests_equal("sha256:abc", "sha512:abc"));
    }

    #[test]
    fn round_trip_blob_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let digest = layer_digest(b"hello");

        assert!(read_blob_by_digest(&root, &digest).unwrap().is_none());
        write_blob(&root, &digest, b"hello").unwrap();
        let got = read_blob_by_digest(&root, &digest).unwrap().unwrap();
        assert_eq!(got, b"hello");
    }

    #[test]
    fn round_trip_tag_pointer() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        assert!(read_tag_pointer(&root, "ghcr.io", "o/r", "1.0")
            .unwrap()
            .is_none());
        write_tag_pointer(&root, "ghcr.io", "o/r", "1.0", "sha256:deadbeef").unwrap();
        let got = read_tag_pointer(&root, "ghcr.io", "o/r", "1.0").unwrap();
        assert_eq!(got.as_deref(), Some("sha256:deadbeef"));
    }

    /// Stand up a wiremock server that speaks just enough of the OCI
    /// Distribution v2 protocol to satisfy `oci-client::Client::pull`,
    /// then drive `fetch_to_bytes` against it. Asserts:
    /// - the returned bytes match the layer payload byte-for-byte;
    /// - the cache is populated under the digest path on first run;
    /// - the second call hits the cache (verified by tearing down the
    ///   server before the second call — if it hit the network it would
    ///   fail with a connection error).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fetch_to_bytes_pulls_from_registry_and_caches() {
        let payload = b"\0asm\x01\x00\x00\x00 fake-wasm-payload-for-test".to_vec();
        let layer_dig = layer_digest(&payload);
        let config_blob = json!({}).to_string().into_bytes();
        let config_digest = layer_digest(&config_blob);

        let manifest = OciImageManifest {
            schema_version: 2,
            media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
            config: OciDescriptor {
                media_type: "application/vnd.oci.image.config.v1+json".to_string(),
                digest: config_digest.clone(),
                size: config_blob.len() as i64,
                urls: None,
                annotations: None,
                artifact_type: None,
            },
            layers: vec![OciDescriptor {
                media_type: "application/wasm".to_string(),
                digest: layer_dig.clone(),
                size: payload.len() as i64,
                urls: None,
                annotations: None,
                artifact_type: None,
            }],
            subject: None,
            artifact_type: None,
            annotations: None,
        };
        let manifest_json = serde_json::to_string(&manifest).unwrap();
        let manifest_digest = layer_digest(manifest_json.as_bytes());

        let server = MockServer::start().await;
        // Anonymous v2 ping — return 200 so the client skips the bearer flow.
        Mock::given(method("GET"))
            .and(path("/v2/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        // Manifest by tag.
        Mock::given(method("GET"))
            .and(path("/v2/test/repo/manifests/v1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
                    .insert_header("Docker-Content-Digest", manifest_digest.as_str())
                    .set_body_raw(
                        manifest_json.clone(),
                        "application/vnd.oci.image.manifest.v1+json",
                    ),
            )
            .mount(&server)
            .await;
        // Manifest by digest (the client may re-fetch by digest).
        Mock::given(method("GET"))
            .and(path(format!("/v2/test/repo/manifests/{manifest_digest}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
                    .set_body_raw(manifest_json, "application/vnd.oci.image.manifest.v1+json"),
            )
            .mount(&server)
            .await;
        // Config blob.
        Mock::given(method("GET"))
            .and(path(format!("/v2/test/repo/blobs/{config_digest}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(config_blob, "application/vnd.oci.image.config.v1+json"),
            )
            .mount(&server)
            .await;
        // Layer blob.
        Mock::given(method("GET"))
            .and(path(format!("/v2/test/repo/blobs/{layer_dig}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(payload.clone(), "application/wasm"),
            )
            .mount(&server)
            .await;

        let cache = tempfile::tempdir().unwrap();
        let host = server.address().to_string();
        let reference = format!("{host}/test/repo:v1");

        // The async test owns a multi-thread runtime; `fetch_to_bytes`
        // spins up its own current_thread runtime and `block_on`s on
        // it. That panics if called from within a runtime, so wrap in
        // spawn_blocking — this is also how the CLI binary will look
        // from the perspective of any future async caller.
        let cache_path = cache.path().to_path_buf();
        let reference2 = reference.clone();
        let bytes = tokio::task::spawn_blocking(move || {
            std::env::set_var("FORGE_CACHE_DIR", &cache_path);
            std::env::set_var("FORGE_OCI_INSECURE_HOSTS", &host);
            fetch_to_bytes(&reference2)
        })
        .await
        .unwrap()
        .expect("first fetch should succeed");
        assert_eq!(bytes, payload, "fetched bytes must equal layer payload");

        // Cache should now contain the blob keyed by digest and the
        // tag pointer file.
        let blob = blob_path(
            &cache.path().join("openapi-forge").join("plugins"),
            &layer_dig,
        )
        .unwrap();
        assert!(
            blob.exists(),
            "blob cache should exist at {}",
            blob.display()
        );

        // Second fetch — tear down the server so any network call
        // would fail with a connection-refused.
        drop(server);
        let cache_path = cache.path().to_path_buf();
        let bytes2 = tokio::task::spawn_blocking(move || {
            std::env::set_var("FORGE_CACHE_DIR", &cache_path);
            fetch_to_bytes(&reference)
        })
        .await
        .unwrap()
        .expect("second fetch should hit the cache, not the network");
        assert_eq!(bytes2, payload);
    }
}
