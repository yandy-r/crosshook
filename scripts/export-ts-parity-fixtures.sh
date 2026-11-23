#!/usr/bin/env bash
#
# export-ts-parity-fixtures.sh — regenerate the TypeScript parity fixtures that
# the Rust view-model tests replay.
#
# Runs src/utils/__tests__/exportParityFixtures.test.ts in write mode. It
# executes the Classic TypeScript launch pipeline, pipeline detail, validation
# mapping and Hero Detail model on fixed inputs and writes their outputs to
#   src/crosshook-native/crates/crosshook-view-model/tests/fixtures/ts_parity/*.json
# Without write mode (plain `npm test`) the same test fails when the committed
# files differ, so the export must be reproducible: run this, then
# `git diff --exit-code` on that directory.
#
# Usage:  ./scripts/export-ts-parity-fixtures.sh [--help|-h]
#
# Requires: npm dependencies installed in src/crosshook-native (npm ci).
#
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  sed -n '2,16p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  exit 0
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT/src/crosshook-native"
CROSSHOOK_WRITE_PARITY_FIXTURES=1 npx vitest run src/utils/__tests__/exportParityFixtures.test.ts
