#!/usr/bin/env bash
#
# export-ts-parity-fixtures.sh — regenerate the TypeScript parity fixtures that
# the Rust view-model and core golden tests replay.
#
# Runs:
#   1. src/utils/__tests__/exportParityFixtures.test.ts in write mode →
#      crates/crosshook-view-model/tests/fixtures/ts_parity/*.json
#   2. src/utils/__tests__/exportProfileNormalizeFixtures.test.ts in write mode →
#      crates/crosshook-core/tests/fixtures/profile_normalize/*.classic.normalized.json
#   3. cargo test -p crosshook-core --test profile_normalize_golden in write mode →
#      crates/crosshook-core/tests/fixtures/profile_normalize/*.classic.toml
#
# Without write mode (plain `npm test` and `cargo test`) the same tests fail
# when committed files differ, so the export must be reproducible: run this,
# then `git diff --exit-code` on both fixture directories.
#
# Usage:  ./scripts/export-ts-parity-fixtures.sh [--help|-h]
#
# Requires: npm dependencies installed in src/crosshook-native (npm ci) and cargo.
#
set -euo pipefail

case "${1:-}" in
  "") ;;
  --help | -h)
    sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  *)
    echo "export-ts-parity-fixtures.sh: unknown argument: $1 (see --help)" >&2
    exit 2
    ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT/src/crosshook-native"
CROSSHOOK_WRITE_PARITY_FIXTURES=1 npx --no-install vitest run \
  src/utils/__tests__/exportParityFixtures.test.ts \
  src/utils/__tests__/exportProfileNormalizeFixtures.test.ts
CROSSHOOK_WRITE_CLASSIC_TOML_FIXTURES=1 cargo test \
  --manifest-path "$REPO_ROOT/src/crosshook-native/Cargo.toml" \
  -p crosshook-core \
  --test profile_normalize_golden
