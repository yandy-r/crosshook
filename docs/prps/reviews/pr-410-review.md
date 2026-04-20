# PR Review #410 — refactor: collection_exchange.rs into smaller modules

**Reviewed**: 2026-04-20T11:09:40-04:00
**Mode**: PR
**Author**: Claude (self-review — Claude also authored the commits after the prior `openai-code-agent` draft #407 timed out)
**Branch**: refactor/collection-exchange-modules -> main
**Decision**: COMMENT

## Summary

Mechanical split of `profile/collection_exchange.rs` (648 lines) into a `profile/collection_exchange/` module with single-responsibility files (error/types/matching/export/import + `mod.rs`). Public API, re-exports from `profile::mod.rs`, serde-derived DTO shapes, error variants, and behavior are all preserved. No persisted data or storage-boundary changes. The split follows the exact convention used by the prior sibling refactor #402 (`exchange/` module). PR-head validation passed.

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

None.

### LOW

None.

### INFO

#### F001 — `LocalMatchIndex` fields exposed as `pub(super)` instead of being hidden behind accessor methods

- **File**: `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/matching.rs:11-16`
- **Status**: Open
- **Rationale**: `import.rs` reads `index.profile_display` directly (in `candidates_for_names(&names, &index.profile_display)`), and `classify_descriptor` in `matching.rs` reads the other two maps. To keep the refactor strictly behavior-preserving, the fields were marked `pub(super)` rather than adding accessor methods and changing call sites. This matches the one-file-at-a-time scope of umbrella #290 (no API restructuring, just code movement). A follow-up could collapse the cross-file coupling by exposing a single `classify` entry point on `LocalMatchIndex`, but that is out of scope for a "split the file" refactor.
- **Action**: None required.

#### F002 — `write_preset_toml` kept as `pub(super)` and imported directly into the `tests` submodule

- **File**: `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/mod.rs:16-19`, `export.rs:85`
- **Status**: Open
- **Rationale**: The test module needs to fabricate preset files from hand-built manifests (three of the seven tests use this). Promoting the writer to `pub` on the crate API would leak a test helper; keeping it `pub(super)` and pulling it in via `use super::export::write_preset_toml;` inside the `#[cfg(test)] mod tests` block is the tightest visibility that works. The sibling `exchange/` module uses the same pattern (`#[cfg(test)] pub use validation::validate_manifest_value;`) for a `pub` item; because `write_preset_toml` is not public, a `pub use` re-export is invalid (confirmed by a `rustc E0364` failure during initial authoring), so the direct `use` inside `tests` is the correct form.
- **Action**: None required.

## Behavior-preservation Audit

All symbols from the pre-refactor file are present in the new module with identical signatures:

| Symbol                                                                                                                                                  | Before                           | After                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------------ |
| `CollectionExchangeError` (enum + `Display`/`Error`/`From<ProfileStoreError>`/`From<MetadataStoreError>`)                                               | `collection_exchange.rs:20-89`   | `collection_exchange/error.rs:13-83`                                                 |
| `CollectionExportResult`, `CollectionPresetMatchCandidate`, `CollectionPresetMatchedEntry`, `CollectionPresetAmbiguousEntry`, `CollectionImportPreview` | `collection_exchange.rs:92-124`  | `collection_exchange/types.rs:12-44`                                                 |
| `LocalMatchIndex`, `MatchClass`, `build_local_match_index`, `candidates_for_names`, `classify_descriptor`                                               | `collection_exchange.rs:126-254` | `collection_exchange/matching.rs:11-137` (private → `pub(super)`)                    |
| `export_collection_preset_to_toml`, `descriptor_from_profile`, `write_preset_toml`                                                                      | `collection_exchange.rs:257-356` | `collection_exchange/export.rs:16-113` (`write_preset_toml`: private → `pub(super)`) |
| `preview_collection_preset_import`, `parse_collection_preset_toml`                                                                                      | `collection_exchange.rs:359-427` | `collection_exchange/import.rs:20-86`                                                |
| Tests (7 tests)                                                                                                                                         | `collection_exchange.rs:429-647` | `collection_exchange/mod.rs:17-244`                                                  |

No function bodies were altered. No `use` reordering changes name resolution. `profile/mod.rs` declares `mod collection_exchange;` and re-exports via `pub use collection_exchange::{...}` — both continue to resolve to the new directory-based module with no edits, and the re-export set is preserved.

## Validation Results

| Check                                         | Result                                                 |
| --------------------------------------------- | ------------------------------------------------------ |
| `cargo test -p crosshook-core`                | Pass — 1123 tests; 7 `collection_exchange` tests green |
| `./scripts/lint.sh --rust` (rustfmt+clippy)   | Pass                                                   |
| `./scripts/lint.sh --host-gateway` (ADR-0001) | Pass                                                   |
| File-size cap (≤500 lines)                    | Pass — max new file is `mod.rs` at 244                 |

### New file line counts

| File          | Lines   |
| ------------- | ------- |
| `error.rs`    | 82      |
| `export.rs`   | 115     |
| `import.rs`   | 88      |
| `matching.rs` | 139     |
| `mod.rs`      | 244     |
| `types.rs`    | 44      |
| **Total**     | **712** |

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange.rs` (Deleted)
- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/error.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/export.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/import.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/matching.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/mod.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/types.rs` (Added)
