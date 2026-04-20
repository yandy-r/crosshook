# PR Review #406 — refactor: catalog.rs into smaller modules

**Reviewed**: 2026-04-20T14:16:06Z
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-catalog-rs-modules → main
**Decision**: APPROVE

## Summary

Pure file-structure refactor that splits `protonup/catalog.rs` (658 lines) into a
`catalog/` module with focused submodules (`cache.rs`, `client.rs`, `config.rs`,
`fetch.rs`, `tests.rs`). Public API is preserved byte-for-byte, all 9 tests move
verbatim, and `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, and
`cargo build` are all clean.

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/protonup/catalog/cache.rs:12` — Internal helpers are declared `pub(crate)` but no caller outside the `catalog/` submodule references them.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Tighten visibility to `pub(super)` on `scoped_cache_key`, `logical_provider_id_for_registry`, `load_catalog_rows`, `build_response_from_rows`, `persist_catalog`, and `stale_fallback_or_offline` in `cache.rs`; on `fetch_live_catalog_by_id` in `fetch.rs`; on `protonup_http_client` in `client.rs`; and on `CACHE_TTL_HOURS` in `mod.rs`. The `pub(crate) use cache::{...}` re-export block in `mod.rs` can likewise become `pub(super) use` (or be dropped — `tests.rs` could `use super::cache::*;` instead of relying on `use super::*;`). Grepping the crate confirms no crate-level consumer exists, so this only widens the potential internal API surface. Non-blocking nit.

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass   |
| Lint       | Pass   |
| Tests      | Pass   |
| Build      | Pass   |

Commands run against the worktree `~/.claude-worktrees/crosshook-pr-406/`:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` → all tests pass, including the 9 tests moved into `catalog/tests.rs`.
- `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings` → clean.
- `cargo fmt --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- --check` → clean.
- `cargo build --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` → clean.

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs` (Deleted, 658 lines)
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog/mod.rs` (Added, 108 lines)
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog/cache.rs` (Added, 236 lines)
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog/client.rs` (Added, 18 lines)
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog/config.rs` (Added, 32 lines)
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog/fetch.rs` (Added, 27 lines)
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog/tests.rs` (Added, 255 lines)

## Notes

- **Acceptance criteria from the child issue**: all met.
  - Public APIs (`catalog_config`, `CatalogProviderConfig`, `list_available_versions`, `list_available_versions_by_id`) preserved — confirmed by diffing the pre- and post-refactor signatures and by `pub use config::{catalog_config, CatalogProviderConfig};` in `mod.rs`.
  - Every resulting source file is ≤ 500 lines; max is `tests.rs` at 255.
  - Logic is identical to `main`: spot-checked `list_available_versions_by_id` (cache → live → stale), `build_response_from_rows` (TTL/expiry logic), and `persist_catalog` (registry checksum fallback) line-by-line against `git show main:…/catalog.rs`.
  - Tests unchanged; 9/9 pass.
- **Umbrella**: This PR is part of the #290 file-size split tracker; the PR body confirms the child-issue linkage.
- **Persistence**: No storage-boundary changes; no new SQLite columns, TOML keys, or runtime state. Matches the child issue's "no new persisted data" claim.
