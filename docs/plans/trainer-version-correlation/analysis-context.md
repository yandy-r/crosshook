# Context Analysis: trainer-version-correlation

## Executive Summary

This feature adds a `version_snapshots` SQLite table (migration 8→9) to track Steam game build IDs vs. trainer binary hashes, detecting mismatches after game updates and surfacing non-blocking warnings through the existing health pipeline. It hooks into three existing paths (post-launch success, startup reconciliation, health dashboard enrichment) with zero new Cargo dependencies and no changes to the synchronous launch flow.

---

## Architecture Context

- **System Structure**: Tauri v2 app — Rust backend in `crosshook-core` library crate, React/TS frontend. All version state lives in SQLite (`metadata.db`), never in TOML profiles (which remain portable). New module `metadata/version_store.rs` follows `health_store.rs` facade pattern; new Tauri command module `commands/version.rs` follows `commands/health.rs` pattern.

- **Data Flow**:
  1. **Record path**: `commands/launch.rs::stream_log_lines()` → `record_launch_finished()` → (on `LaunchOutcome::Succeeded`) → `upsert_version_snapshot()` reading manifest `buildid` + trainer SHA-256
  2. **Scan path**: `startup.rs::run_metadata_reconciliation()` (2–3s delayed async task) → iterate profiles with `steam.app_id` → compare manifest `buildid` vs. latest snapshot → emit `version-scan-complete` event
  3. **Health path**: `commands/health.rs::BatchMetadataPrefetch` bulk-loads version snapshots → extended `ProfileHealthMetadata` → `EnrichedProfileHealthReport` → frontend

- **Integration Points**:
  - `steam/manifest.rs`: Add `parse_manifest_full()` → `ManifestData { build_id, state_flags, last_updated }` (keep existing `parse_manifest()` untouched)
  - `metadata/migrations.rs`: Add `migrate_8_to_9()` with `version_snapshots` table + 2 indexes
  - `metadata/mod.rs`: Add `mod version_store;` + 4 public wrapper methods via `with_conn`/`with_conn_mut`
  - `commands/launch.rs`: Post-success hook after `record_launch_finished()`
  - `commands/health.rs`: Extend `BatchMetadataPrefetch` + `ProfileHealthMetadata` with version fields
  - `commands/community.rs`: Seed initial `version_snapshot` row (status=`untracked`) on community import
  - `startup.rs`: Add background version scan to reconciliation flow
  - `lib.rs`: Register 4 new Tauri commands in `invoke_handler!`
  - `metadata/community_index.rs`: Add `MAX_VERSION_BYTES = 256` bounds check for version fields (Security W1)

---

## Critical Files Reference

### New Files

- `crates/crosshook-core/src/metadata/version_store.rs`: Core CRUD + pure `compute_correlation_status()` — use `health_store.rs` as template
- `src-tauri/src/commands/version.rs`: 4 IPC handlers (`check_version_status`, `get_version_snapshot`, `set_trainer_version`, `acknowledge_version_change`)
- `src/types/version.ts`: TS types (`VersionCheckResult`, `VersionSnapshotInfo`, `VersionCorrelationStatus`)

### Modified Files (Priority Order)

- `crates/crosshook-core/src/metadata/migrations.rs`: Migration 8→9; currently at version 8
- `crates/crosshook-core/src/steam/manifest.rs`: Add `parse_manifest_full()` — `app_state_node.get_child("buildid")` pattern already exists
- `crates/crosshook-core/src/metadata/mod.rs`: Module registration + 4 wrapper methods
- `crates/crosshook-core/src/metadata/models.rs`: Add `VersionSnapshotRow` struct
- `src-tauri/src/commands/launch.rs`: Post-success snapshot hook — `LaunchRequest` already carries full profile including `steam.app_id`
- `src-tauri/src/commands/health.rs`: Extend `BatchMetadataPrefetch` (lines 81–156) + `ProfileHealthMetadata`
- `src-tauri/src/startup.rs`: Background version scan with delayed async spawn
- `crates/crosshook-core/src/metadata/community_index.rs`: W1 security fix — add bounds check for `game_version`/`trainer_version`

### Reference/Pattern Files

- `crates/crosshook-core/src/metadata/health_store.rs`: Template for `version_store.rs` — upsert/load/lookup triad
- `crates/crosshook-core/src/metadata/profile_sync.rs`: SHA-256 hashing pattern (`sha2::{Digest, Sha256}`)
- `crates/crosshook-core/src/steam/vdf.rs`: `VdfNode::get_child()` already works for `buildid`
- `crates/crosshook-core/src/metadata/launch_history.rs:56-119`: `record_launch_finished()` hook + `LaunchOutcome` enum
- `crates/crosshook-core/src/profile/community_schema.rs`: `CommunityProfileMetadata` with existing `game_version`/`trainer_version` fields
- `src/hooks/useProfileHealth.ts`: Frontend hook pattern — `useReducer` + `useCallback` + `AbortController`

---

## Patterns to Follow

- **MetadataStore Facade**: All DB operations via `with_conn()`/`with_conn_mut()` on `MetadataStore`. New module → `mod` declaration in `metadata/mod.rs` → public delegating methods. Example: `metadata/mod.rs:79-115`

- **Fail-Soft DB Access**: `with_conn*` wrappers return `Ok(T::default())` when `available = false`. Never propagate metadata errors to callers or block launch. Example: `commands/health.rs:92-99`

- **Batch Prefetch**: Bulk-load all version snapshots into `HashMap<profile_id, VersionSnapshotRow>` before iterating profiles — O(1) lookup per profile. Anti-pattern: N+1 per-profile queries. Example: `commands/health.rs:81-156`

- **Enrichment in Command Layer**: Core provides focused query functions; Tauri command layer assembles composite response structs. Version fields slot into `ProfileHealthMetadata` inside `commands/health.rs`, not in core. Example: `commands/health.rs:26-41`

- **Pure Function + I/O Separation**: `compute_correlation_status(stored_build_id, current_build_id, stored_hash, current_hash) → VersionCorrelationStatus` — pure, no I/O, fully unit-testable. Pattern: `resolve_launch_method()` in `profile/models.rs`

- **Tauri Command Error Handling**: Return `Result<T, String>`; define `fn map_error(e: impl ToString) -> String { e.to_string() }` locally per module. Non-fatal: `tracing::warn!` + continue. Example: `commands/community.rs:12-14`

- **Serde Conventions**: IPC types get `#[derive(Debug, Clone, Serialize, Deserialize)]`; enums use `#[serde(rename_all = "snake_case")]`; `Option<String>` DB columns use `nullable_text()` helper. Example: `metadata/community_index.rs:305-311`

- **Testing**: Real in-memory SQLite via `MetadataStore::open_in_memory()` — no mocks. Tests inline in `#[cfg(test)] mod tests {}`. IPC contract tests via type-cast assertions. Naming: `{scenario}_{expected_behavior}`.

---

## Implementation Gotchas (From Code Analysis)

- **`request` consumed by spawn closure in `launch.rs:282-297`**: `LaunchRequest` is moved into the async `spawn_log_stream()` closure. Version snapshot fields (`steam.app_id`, `trainer.path`, `profile_name`) must be extracted into owned values **before** the `spawn_log_stream()` call — accessing them after will fail to compile. This is the #1 implementation trap in the launch hook.
- **`failure_mode == CleanExit` is the success gate**: Version snapshot is inserted when `failure_mode == CleanExit` (not on `LaunchOutcome` alone) — verify this enum variant matches `LaunchOutcome::Succeeded` semantics in `launch_history.rs:56-119`
- **Startup scan async pattern**: `lib.rs:73-101` shows the existing background health scan spawn — exact template for version scan: `sleep(N ms) → compute → emit("version-scan-complete")`. Use this, don't invent a new pattern.
- **`BatchMetadataPrefetch` keyed by `profile_id` (not name)**: Add `version_status_map: HashMap<String, String>` following `launcher_drift_map` pattern. Key is `profile_id` UUID, not filename.
- **Community seed at `community_import_profile` line 108**: After `observe_profile_write()`, seed `status='untracked'` snapshot using same `if let Err` fail-soft pattern already at that call site.

---

## Cross-Cutting Concerns

- **Security W1**: `check_a6_bounds()` in `community_index.rs` does not currently bound `game_version`/`trainer_version` — must fix before any version data is read from community columns. (~4 lines)
- **Security W2**: `pinned_commit` in `community/taps.rs` passed to git subprocess without validation — must validate hex-only 7–64 chars before ship (pre-existing gap, addressed here)
- **Security W3**: Community `game_version`/`trainer_version` are display-only **always** — never use as mismatch baseline. Architectural hard constraint (BR-8).
- **Security A8**: DB failure must never block launch. Guaranteed by `with_conn*` fail-soft wrappers — but implementers must NOT bypass with direct `rusqlite` calls.
- **Testing**: No frontend test framework exists; Rust unit tests required for `compute_correlation_status()` and store CRUD. Use in-memory SQLite.
- **Profile portability**: Version data is machine-local SQLite state — must NOT appear in profile exports (`storage_profile()` or community export). Check `profile/models.rs::storage_profile()`.
- **Health pipeline coupling**: Version mismatch surfaces as `HealthIssueSeverity::Warning` (not Error — BR-6). `version_untracked` produces no badge — must NOT be mapped to warning state (see `tasks/lessons.md`: "untracked ≠ error").

---

## Parallelization Opportunities

### Critical Path (Longest Chain)

**1B (schema/models) → 1C (version_store) → Phase 2 Tauri commands** — this sequence must be prioritized; everything else can run around it.

### Phase 1 — All Start Simultaneously

- **1A** (manifest): `steam/manifest.rs::parse_manifest_full()` — standalone, no DB dependency
- **1B** (schema/models): Migration 8→9 + `VersionSnapshotRow` in `models.rs` — prerequisite for 1C
- **1C** (version_store): `version_store.rs` CRUD + `metadata/mod.rs` wiring — depends on 1B models
- **1D** (security): W1 bounds fix in `community_index.rs` + W2 `pinned_commit` validation — fully independent; **recommend shipping as standalone PR before new code reads from those paths**

### Phase 2 — After Phase 1; Backend and Frontend in Parallel

- **2A/2B/2C** (Tauri commands, launch hook, startup scan): Requires 1A + 1C complete
- **2E** (TS types `src/types/version.ts`): Independent — can begin immediately after interfaces are defined in 1C

### Community Seeding (Early-Eligible)

`commands/community.rs` version snapshot seeding on import depends only on Phase 1C (version_store) — **can be scheduled alongside Phase 2**, not gated on Phase 2 completion.

### Phase 3 — Sequential After Phase 2 (UX Polish, not MVP-blocking)

"Mark as Verified", trainer version hint field, Health Dashboard version column — all depend on Phase 2 backend being wired.

---

## Implementation Constraints

- **No new Cargo dependencies**: `rusqlite`, `sha2`, `chrono`, `uuid`, `serde` all already in workspace — adding any new crate is a violation
- **`parse_manifest()` signature frozen**: Multiple callers — add `parse_manifest_full()` alongside, never modify existing signature
- **`steam.app_id` not in SQLite `profiles` table**: Must be passed from `LaunchRequest` (launch path), loaded from TOML `ProfileStore` (startup scan + on-demand check), or from community metadata (import path). Cannot JOIN from `profiles` table.
- **Multi-row history table (not single-row)**: `version_snapshots` departs from `health_snapshots` pattern. Mismatch detection always queries `ORDER BY checked_at DESC LIMIT 1`. **Row pruning required on every INSERT** (Security A7) — prune to N most recent per `profile_id`.
- **StateFlags guard**: `StateFlags != 4` (Steam update in progress) → return `update_in_progress: true`, skip mismatch logic. StateFlags 4 = fully installed; 1026 = update in progress.
- **Version check not in synchronous launch path**: SD card latency on Steam Deck — all version I/O runs in startup scan or on-demand, never in the launch blocking path
- **Migration sequence**: Schema at version 8; next is `migrate_8_to_9()` — do not skip or reorder
- **`VersionCorrelationStatus` must be a typed enum**: 6 string values (`untracked`, `matched`, `game_updated`, `trainer_changed`, `both_changed`, `unknown`) — define as Rust enum with `as_str()`/`FromStr`, mirrored as TS union type (not bare strings)

---

## Key Recommendations

1. **Start with 1A + 1B + 1D simultaneously** — manifest extension (~10 lines) and schema/models are independent; ship 1D (W1+W2 security fixes) as a standalone PR before any new code reads community version fields
2. **Use `health_store.rs` as direct template** for `version_store.rs` — same upsert/load/lookup triad, same `with_conn*` wrappers; key difference is multi-row INSERT + prune instead of single-row upsert
3. **`compute_correlation_status()` first** — pure function with no dependencies; write + test it before any DB or IPC work to establish correct comparison semantics
4. **Batch prefetch version snapshots** in the same query pass as health snapshots in `BatchMetadataPrefetch` — avoids adding a separate N+1 DB call per profile in the health command
5. **Version data in `EnrichedProfileHealthReport`** (not a new hook) — `useProfileHealth` already drives LaunchPage and Health Dashboard; version data flows through existing `metadata` field. Only create `useVersionCorrelation` if a standalone version page is added (not in scope for v1)
6. **Guard community data access everywhere**: Any code path that reads `community_profiles.game_version` or `trainer_version` is display-only output — document this explicitly in code comments at the call site to prevent future regressions
7. **Emit `version-scan-complete` event** even when zero mismatches found — frontend needs the signal to remove any loading indicators from the startup scan state
8. **Task breakdown should track**: (a) Does the migration run cleanly on existing DBs? (b) Does the launch hook fire only on `LaunchOutcome::Succeeded`? (c) Does startup scan skip `StateFlags != 4` manifests? These are the three correctness properties to verify before Phase 2 is done.
