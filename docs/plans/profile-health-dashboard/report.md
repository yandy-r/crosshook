# Profile Health Dashboard — Phase D Implementation Report

## Overview

Phase D adds **health snapshot persistence and trend analysis** to the profile health dashboard. Health check results are now cached in SQLite so the frontend can display instant badge status on startup (before the live scan completes), show trend arrows comparing current vs. cached status, and detect stale snapshots that haven't been refreshed in over 7 days.

**Phase**: D (Persistence + Trends)
**GitHub Issue**: #94
**Prior Phases**: A (core MVP), B (metadata enrichment), C (startup integration) — all shipped

## Files Changed

### Created (1 file)

| File                                                 | Purpose                                                                                                                                    |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/metadata/health_store.rs` | SQLite persistence layer — `upsert_health_snapshot()`, `load_health_snapshots()`, `lookup_health_snapshot()` with `HealthSnapshotRow` type |

### Modified (8 files)

| File                                               | Change                                                                                                                                                                                       |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/migrations.rs` | Added `migrate_5_to_6()` — `health_snapshots` table (profile_id PK, status, issue_count, checked_at) + checked_at index                                                                      |
| `crates/crosshook-core/src/metadata/mod.rs`        | Added `mod health_store`, `pub use HealthSnapshotRow`, and 3 public delegation methods under "Phase D: Health snapshot persistence" section                                                  |
| `src-tauri/src/commands/health.rs`                 | Added fail-soft snapshot persistence in `build_enriched_health_summary()`, `CachedHealthSnapshot` IPC struct with `From<HealthSnapshotRow>`, and `get_cached_health_snapshots` Tauri command |
| `src-tauri/src/lib.rs`                             | Registered `get_cached_health_snapshots` in `invoke_handler!`                                                                                                                                |
| `src/types/health.ts`                              | Added `CachedHealthSnapshot` TypeScript interface                                                                                                                                            |
| `src/hooks/useProfileHealth.ts`                    | Added cached snapshot loading on mount, `computeTrend()` function, `trendByName` and `staleInfoByName` derived state, renamed local `HealthStatus` to `HookStatus` to avoid collision        |
| `src/components/HealthBadge.tsx`                   | Added `trend` prop with ↓/↑ arrow rendering (got_worse/got_better)                                                                                                                           |
| `src/components/pages/ProfilesPage.tsx`            | Added stale-snapshot note in health detail CollapsibleSection ("Last checked N days ago — consider re-checking")                                                                             |

## Features Implemented

### D.1 — Migration v6: health_snapshots table

- Schema: `profile_id TEXT PRIMARY KEY REFERENCES profiles(profile_id)`, `status TEXT NOT NULL`, `issue_count INTEGER NOT NULL DEFAULT 0`, `checked_at TEXT NOT NULL`
- Index on `checked_at` for temporal queries
- Follows existing v0–v5 migration pattern exactly

### D.2 — Health store persistence layer

- `upsert_health_snapshot()` — INSERT OR REPLACE for idempotent writes
- `load_health_snapshots()` — JOIN profiles WHERE deleted_at IS NULL (security N-4 compliance)
- `lookup_health_snapshot()` — single-profile lookup with `.optional()` pattern
- Row type `HealthSnapshotRow` with profile_id, profile_name, status, issue_count, checked_at

### D.3 — MetadataStore public API wiring

- Three delegation methods via `self.with_conn()` — all satisfy `T: Default` constraint
- Section comment follows existing convention

### D.4 — Fail-soft persistence after batch validation

- Iterates enriched profiles after validation, persists status + issue_count per profile
- Only persists when metadata is available (profile_id exists)
- Failures logged with `tracing::warn!` — never affects the returned health summary

### D.5 — Cached snapshots IPC command

- `get_cached_health_snapshots` Tauri command returns `Vec<CachedHealthSnapshot>`
- `CachedHealthSnapshot` IPC struct with `From<HealthSnapshotRow>` conversion
- Returns empty Vec when MetadataStore is unavailable (fail-soft)

### D.6 — Trend arrows

- `computeTrend()` compares current health status to cached status via ordinal ranking (healthy=0, stale=1, broken=2)
- `trendByName` useMemo derives per-profile trend from live results + cached snapshots
- HealthBadge renders ↓ (warning color) for got_worse, ↑ (success color) for got_better
- No visual noise for unchanged profiles

### D.7 — Stale snapshot detection

- 7-day threshold (`STALE_THRESHOLD_DAYS`)
- `staleInfoByName` useMemo derives `{ isStale, daysAgo }` from cached snapshots
- ProfilesPage renders "Last checked N days ago — consider re-checking" note in health detail panel
- Only shown when cached data is stale AND live scan hasn't completed yet

## Architecture Decisions

- **Fail-soft everywhere**: All persistence and retrieval operations use `unwrap_or_default()` or `tracing::warn!` on failure — MetadataStore unavailability never blocks the health dashboard
- **IPC boundary serialization**: `HealthSnapshotRow` (crosshook-core) is `Debug + Clone` only; `CachedHealthSnapshot` (src-tauri) adds `Serialize + Deserialize` for IPC with a clean `From` conversion
- **Cached-then-live pattern**: Frontend loads cached snapshots first for instant badges, then runs the live scan which replaces them — stale notes auto-dismiss once live results arrive
- **Security N-4 compliance**: All `health_snapshots` queries JOIN `profiles WHERE deleted_at IS NULL` to exclude soft-deleted profiles

## Validation

| Check                             | Result                          |
| --------------------------------- | ------------------------------- |
| `cargo test -p crosshook-core`    | 240/240 passed                  |
| `cargo check -p crosshook-native` | Clean                           |
| `npx tsc --noEmit`                | Clean                           |
| Diff stats                        | +255 / -11 lines across 9 files |

## Test Guidance

### Manual Testing

1. Start the app — health badges should appear instantly from cached snapshots (after first run)
2. Break a profile path (rename game executable) — re-check should show ↓ trend arrow
3. Fix a profile path — re-check should show ↑ trend arrow
4. Wait 7+ days (or manually set checked_at in SQLite) — stale note should appear before live scan completes
5. Verify badges update after live scan replaces cached data

### Automated Testing

- `crosshook-core` unit tests cover migration and existing health check logic
- `health_store.rs` functions are testable via `MetadataStore::open_in_memory()` if additional integration tests are desired
