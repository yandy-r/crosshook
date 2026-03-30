# Config History Rollback - Technical Design Research

## Overview

CrossHook now has two persistence layers for profiles: filesystem TOML as the live source of truth (`ProfileStore`) and SQLite metadata (`MetadataStore`) for indexed/history-style data. Configuration history should be implemented DB-first (SQLite snapshots keyed by `profile_id`) with optional sidecar snapshot files as a secondary storage mode, not as the primary model.

The current write flow already has reliable hook points in Tauri profile commands and metadata sync (`observe_profile_write`, `observe_profile_rename`, `observe_profile_delete`). The design below keeps rollback consistent with existing patterns: write profile TOML through `ProfileStore`, then reconcile metadata/history, with bounded retention and deduplicated snapshots.

## Existing Save Flow and Metadata Hooks

### Current write path (important)

- `src/crosshook-native/src-tauri/src/commands/profile.rs`
  - `profile_save`: `store.save()` -> `metadata_store.observe_profile_write(..., SyncSource::AppWrite, None)` -> `profiles-changed`.
  - `profile_save_launch_optimizations`: `store.save_launch_optimizations()` -> `store.load()` -> `observe_profile_write(... AppWrite ...)`.
  - `profile_apply_bundled_optimization_preset`: writes preset, reloads profile, then `observe_profile_write_launch_change(...)`.
  - `profile_save_manual_optimization_preset`: same pattern as bundled apply.
  - `profile_duplicate`: `store.duplicate()` then `observe_profile_write(..., SyncSource::AppDuplicate, source_profile_id)`.
  - `profile_rename`: `store.rename()` then `observe_profile_rename(old_name, new_name, old_path, new_path)`.
  - `profile_delete`: `store.delete()` then `observe_profile_delete(name)`.
  - `profile_import_legacy`: writes TOML then `observe_profile_write(..., SyncSource::Import, None)`.

- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`
  - `observe_profile_write` upserts `profiles` row and recomputes `content_hash` from pretty TOML serialization.
  - `observe_profile_rename` keeps same `profile_id`, updates filename/path, records `profile_name_history`.
  - `observe_profile_delete` soft-deletes via `deleted_at`.

### Implication for history feature

The profile command layer is the correct place to trigger snapshot creation because:

- all relevant mutations already pass through it;
- it has both the semantic trigger context (save vs duplicate vs optimization autosave) and resolved profile name/path;
- it already performs metadata synchronization and emits frontend events.

## Schema Options for Snapshot History

## Option A (recommended): DB full snapshots only

Add migration `10 -> 11` in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`:

- `profile_config_snapshots`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE`
  - `trigger TEXT NOT NULL` (`manual_save`, `launch_optimization_save`, `preset_apply`, `duplicate_create`, `rollback_apply`, `import`, `migration`, `rename_marker`, `delete_marker`)
  - `profile_name_at_write TEXT NOT NULL`
  - `content_hash TEXT` (sha256 of canonical TOML)
  - `snapshot_toml TEXT` (nullable for marker rows)
  - `source_snapshot_id INTEGER` (used by rollback lineage; optional FK to same table)
  - `created_at TEXT NOT NULL`
- Indexes:
  - `(profile_id, created_at DESC)`
  - `(profile_id, id DESC)`
  - `(profile_id, content_hash)`

Why this fits CrossHook:

- Same style as `version_snapshots` (`metadata/version_store.rs`) and existing retention/pruning patterns.
- Renames are naturally handled via stable `profile_id`.
- Querying history/diffs/rollback is single-store and transactional.

## Option B: DB index + sidecar TOML snapshots

- DB table stores metadata and sidecar file path.
- Snapshot payload lives under a dedicated history directory (for example `~/.config/crosshook/profile-history/<profile_id>/<snapshot_id>.toml`).

Pros:

- smaller SQLite file;
- easier manual inspection/export.

Cons:

- extra failure mode (DB/file divergence);
- path cleanup complexity on rename/delete/prune;
- backup semantics become multi-root.

Use only if DB growth is unacceptable after profiling.

## Option C: DB patch/delta storage

- Store diffs against previous revision instead of full TOML blobs.

Pros: smaller footprint.
Cons: complexity and rollback fragility increase significantly; poor fit for v1.

Recommendation: do not use for initial implementation.

## Snapshot Creation Hook Points

Implement a single helper in `src-tauri/src/commands/profile.rs` (or small core orchestration wrapper) that appends snapshot rows after successful writes.

### Hook matrix

- `profile_save` -> snapshot trigger `manual_save`.
- `profile_save_launch_optimizations` -> trigger `launch_optimization_save`.
- `profile_apply_bundled_optimization_preset` and `profile_save_manual_optimization_preset` -> trigger `preset_apply`.
- `profile_duplicate` -> trigger `duplicate_create` for the new profile only (do not clone source history rows).
- `profile_rename` -> optional marker row `rename_marker` with null `snapshot_toml` (history continuity remains by `profile_id` regardless).
- `profile_delete` -> optional marker row `delete_marker` before soft delete.

Also consider existing non-profile commands that mutate TOML:

- `src-tauri/src/commands/migration.rs` already calls `observe_profile_write(..., SyncSource::AppMigration)` after mutation. Add snapshot append there with trigger `migration`.
- `src-tauri/src/commands/community.rs` import path should append trigger `import`.

### Ordering recommendation

For save-like flows:

1. Write TOML via `ProfileStore`.
2. Reload from disk (`store.load(name)`) to capture canonical persisted state.
3. `observe_profile_write(...)` to ensure `profile_id` and current `content_hash` are up to date.
4. Append config snapshot row (dedup by hash).

This keeps history payload aligned with actual on-disk state.

## Rollback Semantics and Consistency Model

## Rollback semantics

- Rollback target is a snapshot row for a specific `profile_id`.
- Rollback operation:
  1. Resolve profile by current name -> `profile_id`.
  2. Fetch `snapshot_toml`.
  3. Deserialize into `GameProfile` (hard fail if invalid).
  4. `ProfileStore.save(current_name, profile_from_snapshot)`.
  5. `observe_profile_write(..., SyncSource::AppWrite)` to refresh metadata row and hash.
  6. Append new snapshot row `trigger=rollback_apply`, `source_snapshot_id=<rolled_back_id>`.
  7. Emit `profiles-changed`.

Rollback should never mutate old snapshot rows.

## Consistency model

- Source of truth remains filesystem TOML (`ProfileStore`).
- Metadata/history is strongly consistent within SQLite, but only eventually consistent with filesystem across process crashes because FS+DB cannot share one transaction.
- Existing app behavior is fail-soft for metadata. Keep that default for save flows.
- For rollback command specifically, recommended behavior is stricter:
  - if metadata unavailable: return error ("rollback history unavailable");
  - if filesystem write fails: rollback fails;
  - if metadata append fails after filesystem write: return success with warning event/log (state is rolled back but history trail incomplete).

This matches existing reliability priorities while preserving user-visible correctness.

## Diff Computation Strategy

## Data source for diffs

Use canonical TOML text from snapshot rows (`snapshot_toml`) and either:

- another snapshot row; or
- current persisted profile (serialize `store.load(name)` using `toml::to_string_pretty`).

## Algorithm

V1: line-based unified diff generated in Rust.

- Suggested dependency: `similar` crate in `crosshook-core` (small, focused).
- Normalize to `\n` line endings before diff.
- Return structured payload:
  - `summary` (`added`, `removed`, `changed` line counts)
  - `unified_diff` text for UI viewer
  - optional `left_created_at`/`right_created_at`

Why line diff first:

- predictable and cheap;
- no schema lock-in;
- avoids writing custom diff logic.

Future enhancement (not required now): semantic field diff by TOML AST/key path.

## Retention and Pruning Strategy

Follow existing `version_store` model (`metadata/version_store.rs`):

- define `MAX_PROFILE_CONFIG_SNAPSHOTS_PER_PROFILE` in `metadata/models.rs` (start with 50).
- insert snapshot + prune in one SQLite transaction:
  - keep latest N by `created_at DESC, id DESC`;
  - delete older rows.
- dedup rule: if latest snapshot for profile has same `content_hash`, skip insert.

For noisy autosaves (`launch_optimization_save`), dedup usually removes no-op writes; add time-based coalescing only if production telemetry/logging shows churn.

## Migration Approach (No-history -> New System)

### Schema migration

- Add `migrate_10_to_11` in `metadata/migrations.rs`.
- Bump `PRAGMA user_version` to `11`.
- No destructive migration required.

### Data bootstrap

Do not backfill historical revisions from prior sessions (not available).
Bootstrap options:

- Minimal: create snapshots only on new writes after upgrade.
- Better UX: one-time baseline seeding on startup reconciliation:
  - during `startup::run_metadata_reconciliation`, for each active profile without history rows, insert one `trigger=initial_baseline`.

Baseline seeding should be idempotent and bounded.

## Tauri Commands and Frontend Integration Points

## Proposed Tauri commands

Add in `src-tauri/src/commands/profile.rs` and register in `src-tauri/src/lib.rs`:

- `profile_list_config_history(name: String, limit: Option<u32>, before_id: Option<i64>) -> Vec<ConfigSnapshotSummaryDto>`
- `profile_get_config_snapshot(name: String, snapshot_id: i64) -> ConfigSnapshotDto`
- `profile_diff_config_snapshots(name: String, left_snapshot_id: i64, right_snapshot_id: Option<i64>) -> ConfigDiffDto`
  - `right_snapshot_id=None` means compare against current on-disk.
- `profile_rollback_config_snapshot(name: String, snapshot_id: i64) -> GameProfile`

All should return stringified errors matching existing IPC style.

## Frontend integration

Primary files:

- `src/crosshook-native/src/hooks/useProfile.ts`: add history actions/state (`loadHistory`, `diffHistory`, `rollbackHistory`).
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`: add "Config History" section/modal near `ProfileActions`.
- `src/crosshook-native/src/components/ProfileActions.tsx`: optional "History" button to open history modal.
- `src/crosshook-native/src/types/`: add `profile-history.ts` DTOs and re-export in `types/index.ts`.

On rollback success:

- refresh selected profile (`profile_load`);
- refresh health (`revalidateSingle`) because paths/settings may change;
- optionally show toast with "Rollback applied to snapshot <timestamp>".

## Testing Strategy

## Rust unit tests (core)

Add tests in new `metadata/config_history_store.rs` and/or `metadata/mod.rs` test module using `MetadataStore::open_in_memory()`:

- insert snapshot row;
- dedup on same hash;
- prune after N+1 inserts;
- lookup/list order correctness;
- diff output sanity;
- rollback lineage (`source_snapshot_id`) correctness.

## Integration-style Rust tests

Use `ProfileStore::with_base_path(tempdir)` + in-memory metadata:

- save profile v1 -> snapshot;
- mutate -> snapshot v2;
- rollback to v1 -> file content hash matches v1;
- ensure post-rollback snapshot row exists with `trigger=rollback_apply`.

Cover lifecycle hooks:

- duplicate creates independent history for copy;
- rename preserves history continuity by `profile_id`;
- delete marker behavior and list filtering for deleted profiles.

## Tauri command contract tests

As done in existing command modules (`commands/profile.rs`, `commands/version.rs`):

- compile-time function signature assertions for new commands.

## Frontend tests

No enforced framework currently; keep frontend logic testable with pure helpers:

- timestamp formatting, diff summary rendering, optimistic state transitions.
- mock `invoke` in hook-level tests if test harness is added later.

## Actionable Implementation Notes

- Add new metadata submodule (for example `metadata/config_history_store.rs`) and thin wrappers in `MetadataStore`.
- Keep command-layer hook logic centralized in one helper to avoid drift across save paths.
- Reuse existing hash convention (`toml::to_string_pretty` + SHA-256) from `profile_sync`.
- Maintain fail-soft behavior for ordinary saves, but explicit fail-fast when user calls rollback and metadata is unavailable.
- Start with DB full snapshots; only add sidecar mode if DB size becomes a demonstrated problem.
