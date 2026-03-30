# Config History Rollback - Architecture Research

## System overview

CrossHook already splits responsibilities cleanly: live profile state is persisted as TOML in `ProfileStore`, while durable indexed operational metadata lives in `MetadataStore`. The config-history feature should preserve that boundary by adding append-only revision history in SQLite keyed by stable `profile_id`, then routing rollback through the same save path that writes TOML.

## Existing architecture anchors

- `crates/crosshook-core/src/profile/toml_store.rs`
  - Canonical read/write path for profile files.
  - Existing validation and naming constraints.
- `crates/crosshook-core/src/metadata/mod.rs`
  - Metadata facade, availability checks, and fail-soft behavior.
- `crates/crosshook-core/src/metadata/profile_sync.rs`
  - `observe_profile_write` already computes `content_hash` and resolves stable identity.
- `crates/crosshook-core/src/metadata/migrations.rs`
  - Centralized schema lifecycle for new tables.
- `src/crosshook-native/src-tauri/src/commands/profile.rs`
  - Existing write orchestration and event emission points.

## Recommended architecture

### Persistence

- Add `config_revisions` table in metadata DB:
  - revision id, `profile_id`, write source, `content_hash`, snapshot payload, lineage pointer, known-good flag, timestamps.
- Keep full snapshot payload for MVP (`snapshot_toml`) to guarantee simple, reliable rollback.
- Apply per-profile retention and dedup at insertion time.

### Command orchestration

For profile mutation flows:

1. Persist profile TOML via `ProfileStore`.
2. Reload canonical profile from disk.
3. Sync metadata identity/hash with `observe_profile_write`.
4. Append deduped history revision.
5. Emit frontend profile-change event.

### Rollback

Rollback must stay non-destructive:

1. Resolve profile and verify revision ownership.
2. Parse snapshot into `GameProfile`.
3. Save restored profile through `ProfileStore`.
4. Re-observe metadata write.
5. Append new `rollback_apply` revision with source lineage.

## Availability model

- History/diff/rollback APIs are metadata-dependent.
- Profile editing and standard save flows remain available if metadata is disabled.
- UI should show explicit unavailable states rather than hiding failures.

## Architecture decisions for planning

1. Default retention cap: 20 (safety window) or 5 (strict MVP minimum).
2. Known-good semantics: strict successful completion vs broader success.
3. Initial tracked write sources: full saves/import/rollback only or include optimization/preset writes in MVP.
