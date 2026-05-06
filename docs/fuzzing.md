# Fuzzing

We exercise the host with [cargo-fuzz](https://rust-fuzz.github.io/book/)
(libFuzzer) from a separate `fuzz/` crate that pins its own nightly
toolchain and is excluded from the main workspace. The two host-side
boundaries that consume untrusted input are the OpenAPI parser
(`crates/forge-parser`) and the IR-bindgen layer that runs over output
returned by every transformer plugin (`crates/forge-ir-bindgen`).

Three targets:

| Target                  | Input shape                                         | Best at finding                                              |
| ----------------------- | --------------------------------------------------- | ------------------------------------------------------------ |
| `parse_str_bytes`       | Raw `&[u8]` decoded as UTF-8                        | JSON-decode boundary panics, root-object reject path bugs    |
| `parse_str_structured`  | `arbitrary`-derived `serde_json::Value` (biased)    | Schema/ref/finalize walking bugs that need a valid root      |
| `transformer_output_ir` | JSON bytes deserialized into `forge_ir::Ir`         | Convert/validate panics on adversarial transformer output    |

The structured parser target builds depth-bounded JSON values whose keys
are heavily biased toward terms the parser actually inspects (`paths`,
`components`, `$ref`, `allOf`, `discriminator`, …). Random bytes almost
never reach the parser's deeper code paths because they fail
`serde_json::from_str` or the root-object check first; biased structured
input gets to the interesting code in milliseconds.

`transformer_output_ir` models the threat that a hostile transformer
returns a WIT-typed `Ir` whose semantic invariants are violated —
dangling type refs, duplicate type ids, status ranges outside `1..=5`,
discriminator mappings pointing at types that don't exist. Each input
is parsed as `forge_ir::Ir`, then driven through the host code that
runs after every transformer call: `validate_refs` plus
`convert::{transformer,generator}::{ir_to_wit,ir_from_wit}` — see
`crates/forge-host/src/runtime.rs:545,554,588`. Returning
`BindgenError` is fine; panicking is not. Corpus is seeded from
`fixtures/conformance/*/expected-ir.json`, so libFuzzer mutates from
real, valid IRs.

## Prerequisites

- Rust nightly. The `fuzz/rust-toolchain.toml` pins it; rustup users get
  the right channel automatically when `cd`-ing into `fuzz/`.
- `cargo-fuzz` installed: `cargo install cargo-fuzz` (or `nix develop`
  picks it up from `flake.nix`).
- libFuzzer comes with nightly's compiler-rt; no separate install.
- For coverage: `llvm-tools-preview` (provides `llvm-cov` / `llvm-profdata`).
  Rustup users: `rustup component add llvm-tools-preview --toolchain nightly`.
  Nix users get it via the `fuzz` shell — see `flake.nix`.

NixOS users: a separate `fuzz` shell in `flake.nix` provides nightly
Rust and `cargo-fuzz`. Run `nix develop .#fuzz` from the repo root before
working in `fuzz/`. The default shell stays on stable.

## Run locally

```bash
# One-time corpus seeding from in-tree fixtures (~65 specs). Only seeds
# parse_str_bytes and transformer_output_ir; parse_str_structured builds
# its corpus from libFuzzer mutations (see "Corpus seeding" below).
bash fuzz/seed-corpus.sh

# Short run, all targets:
cd fuzz
cargo fuzz run parse_str_bytes        -- -runs=10000 -max_total_time=60
cargo fuzz run parse_str_structured   -- -runs=10000 -max_total_time=60
cargo fuzz run transformer_output_ir  -- -runs=10000 -max_total_time=60
```

Longer runs simply raise the time/run budget; the fuzzer remembers
coverage via the persisted corpus so a second run starts from where the
first left off.

## Corpus seeding

`seed-corpus.sh` only seeds the targets whose corpus is meaningfully
served by raw fixture JSON:

- `parse_str_bytes` — seeded from `fixtures/**/spec.json`. The target
  reads bytes directly, so valid spec JSON is exactly what we want in
  the corpus.
- `transformer_output_ir` — seeded from `fixtures/**/expected-ir.json`.
  Same shape the target deserializes into `forge_ir::Ir`.
- `parse_str_structured` — **intentionally not seeded.** This target
  consumes its input via `arbitrary::Unstructured`, so seed bytes are
  interpreted as `Arbitrary` decisions, not parsed as JSON. Feeding it
  spec.json bytes produces near-random `Value`s. It builds its own
  corpus from libFuzzer mutations starting cold.

## Measuring coverage

cargo-fuzz integrates with LLVM source-based coverage. From the
`fuzz/` directory inside `nix develop .#fuzz`:

```bash
cargo fuzz coverage <target>
```

This compiles the target with `-C instrument-coverage`, replays every
file in `fuzz/corpus/<target>/` against it, and writes
`fuzz/coverage/<target>/coverage.profdata`.

Render a human-readable report with `llvm-cov` (lives in the nightly
sysroot under rust-overlay; not available as `cargo cov`):

```bash
LLVM_COV="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-cov"
COV_DIR="target/x86_64-unknown-linux-gnu/coverage/x86_64-unknown-linux-gnu/release"

# Text summary per file:
"$LLVM_COV" report \
  "$COV_DIR/<target>" \
  --instr-profile="coverage/<target>/coverage.profdata" \
  --ignore-filename-regex="/.cargo/registry|/rustc/|/library/|fuzz_targets/|/build/|/out/"

# Annotated HTML (open coverage/<target>/html/index.html):
"$LLVM_COV" show \
  "$COV_DIR/<target>" \
  --instr-profile="coverage/<target>/coverage.profdata" \
  --format=html \
  --output-dir="coverage/<target>/html" \
  --ignore-filename-regex="/.cargo/registry|/rustc/|/library/|fuzz_targets/|/build/|/out/"
```

**Important — measure after a real run.** Coverage reflects the
current corpus; reading it on a freshly-seeded (or empty) corpus
produces a misleading number that captures only what the seeds happen
to cover, not what the fuzzer can reach. Standard protocol:

```bash
cargo fuzz run <target> -- -max_total_time=300   # at least a few minutes
cargo fuzz coverage <target>
"$LLVM_COV" report ...
```

For `parse_str_structured` specifically, this is mandatory — it has
no seed corpus, so a cold `cargo fuzz coverage` call reports near-zero
coverage regardless of what the target can actually reach.

## When the fuzzer finds a crash

1. cargo-fuzz writes the offending input to
   `fuzz/artifacts/<target>/crash-<hash>`.
2. Minimize it:

   ```bash
   cargo fuzz tmin <target> fuzz/artifacts/<target>/crash-<hash>
   ```

   The minimized input lands at `fuzz/artifacts/<target>/minimized-from-...`.
3. Fix the bug. Then commit the **minimized** sample to the regression
   directory that matches the target:

   - `parse_str_bytes`, `parse_str_structured` →
     `crates/forge-parser/tests/fuzz_regressions/`
   - `transformer_output_ir` →
     `crates/forge-ir-bindgen/tests/fuzz_regressions/`

   Use a descriptive filename (e.g. `cyclic-allof.json`,
   `dangling-discriminator.json`). The matching `fuzz_regressions.rs`
   integration test in each crate replays committed samples on stable
   forever after.
4. Open a PR. Both `fuzz-smoke` (per-PR) and the `fuzz_regressions`
   tests will guard the bug.

## Add a new target

1. Create `fuzz/fuzz_targets/<name>.rs` with a `fuzz_target!` macro.
2. Add a matching `[[bin]]` entry to `fuzz/Cargo.toml`.
3. Wire the target into `.github/workflows/ci.yml` (`fuzz-smoke` job)
   and `.github/workflows/fuzz.yml` (matrix).
4. Extend `fuzz/seed-corpus.sh` if the new target wants different seeds.

Keep targets small and focused. One harness per public API surface is
the right default.

## CI

- `.github/workflows/ci.yml` → `fuzz-smoke`: ~60s per target on every PR
  and push to `main`. Fails the build on a crash.
- `.github/workflows/fuzz.yml`: nightly cron, 10 minutes per target.
  Uploads `fuzz/artifacts/` and the resulting corpus on failure for
  triage.

The smoke job is intentionally short. It exists to catch obvious
regressions on the hot path, not to substitute for the deeper run.

## Out of scope

- OSS-Fuzz integration. Worth doing once a corpus has stabilized.
- Fuzzing transformers/generators. They don't take untrusted input on
  the host side; the WASM sandbox is the boundary, not their `Guest` impl.
- Fuzzing `parse_path`. Adds filesystem complexity without exercising
  meaningfully different parser code; defer until a concrete need arises.
