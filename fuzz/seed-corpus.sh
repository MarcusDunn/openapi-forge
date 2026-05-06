#!/usr/bin/env bash
# Seed the fuzz corpus from in-tree fixtures.
#
# Idempotent: re-running just re-copies. Each conformance/e2e/real-world
# spec.json is hashed by name so collisions across fixture trees produce
# distinct corpus entries.
#
# `parse_str_structured` is intentionally not seeded: that target consumes
# its input via `arbitrary::Unstructured`, so feeding it raw spec.json
# bytes produces near-random `Value`s that bounce off entry validation.
# It builds its own corpus from libFuzzer mutations on first run.
#
# Usage:
#   ./seed-corpus.sh            # seed all seedable targets
#   ./seed-corpus.sh parse_str_bytes
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${here}/.." && pwd)"
fixtures="${repo_root}/fixtures"

targets=(parse_str_bytes transformer_output_ir)
if [[ $# -gt 0 ]]; then
  targets=("$@")
fi

# Each target expects a different fixture filename. The byte parser target
# is fed spec JSON; the transformer-output target is fed canonical IR JSON,
# since that's the shape a transformer returns post-WIT-conversion.
fixture_filename_for() {
  case "$1" in
    parse_str_bytes) echo "spec.json" ;;
    transformer_output_ir) echo "expected-ir.json" ;;
    parse_str_structured)
      echo "parse_str_structured is not seedable from JSON fixtures (see header)" >&2
      exit 2
      ;;
    *)
      echo "unknown target: $1" >&2
      exit 1
      ;;
  esac
}

seed_one_target() {
  local target="$1"
  local out="${here}/corpus/${target}"
  local fname
  fname="$(fixture_filename_for "${target}")"
  mkdir -p "${out}"

  local count=0
  while IFS= read -r -d '' spec; do
    # Use the parent + tree directory names so seeds from disjoint fixture
    # subtrees (`conformance/foo/`, `real-world/foo/`) don't collide.
    local parent
    parent="$(basename "$(dirname "${spec}")")"
    local tree
    tree="$(basename "$(dirname "$(dirname "${spec}")")")"
    cp -f "${spec}" "${out}/${tree}-${parent}.json"
    count=$((count + 1))
  done < <(find "${fixtures}" -name "${fname}" -print0)

  printf 'seeded %s with %d files into %s\n' "${target}" "${count}" "${out}"
}

for t in "${targets[@]}"; do
  seed_one_target "${t}"
done
