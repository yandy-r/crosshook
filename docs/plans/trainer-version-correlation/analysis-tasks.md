# Task Analysis: Trainer-Version Correlation

**Agent**: task-analyzer
**Date**: 2026-03-29
**Input documents read**: `shared.md`, `feature-spec.md`, `research-architecture.md`, `research-patterns.md`, `research-integration.md`, `research-docs.md`
**Source files inspected**: `metadata/health_store.rs`, `metadata/migrations.rs`, `commands/launch.rs`, `commands/health.rs`, `commands/mod.rs`, `startup.rs`

---

## Executive Summary

The trainer-version-correlation feature touches 13 existing files and creates 3 new files across a well-defined, layered architecture. All required Cargo dependencies already exist. The work divides cleanly into 4 phases with **significant parallelism available in Phase 1** (3 of 4 tasks can start simultaneously) and **moderate parallelism in Phase 2** (Rust backend tasks and TypeScript types are independent). The foundation tasks (Phase 1) are the critical path — they must complete before any wiring, UI, or integration work begins.

The recommended approach is **4 phases with 16 tasks total**, targeting Phase 1+2 as MVP (9 tasks). Phase 3 adds UX polish (5 tasks). Phase 4 (analytics) is deferred.

---

## Recommended Phase Structure

### Phase 1 — Detection Foundation (~5 days, 4 tasks)

**Goal**: Stable Rust infrastructure that compiles and passes tests. No Tauri wiring yet.
**Parallelism**: Tasks 1A, 1B, and 1D have zero interdependencies and can run simultaneously. Task 1C unlocks only after 1B completes.

```
1A (manifest) ────────────────────────────────────────────┐
1B (schema/models) ──→ 1C (version_store + mod wrapper) ──┤ → Phase 2
1D (security fixes) ──────────────────────────────────────┘
```

| Task                          | Focus                                                                              | Files                                                          | Parallelizable?   |
| ----------------------------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------- | ----------------- |
| **1A** — Manifest Extension   | Add `parse_manifest_full()` + `ManifestData` struct                                | `steam/manifest.rs` (1 file)                                   | Yes — start day 1 |
| **1B** — Schema + Models      | `migrate_8_to_9()` + `VersionSnapshotRow` struct + `VersionCorrelationStatus` enum | `metadata/models.rs`, `metadata/migrations.rs` (2 files)       | Yes — start day 1 |
| **1C** — Version Store Module | Core CRUD + pure `compute_correlation_status()` + `MetadataStore` wrapper methods  | `metadata/version_store.rs` (NEW), `metadata/mod.rs` (2 files) | After 1B          |
| **1D** — Security Fixes       | A6 bounds (W1) + pinned_commit validation (W2)                                     | `metadata/community_index.rs`, `community/taps.rs` (2 files)   | Yes — start day 1 |

### Phase 2 — Launch Integration (~4 days, 5 tasks)

**Goal**: Feature is functionally wired — version snapshots record on launch success, startup scan runs, health dashboard shows version data, and frontend can query via IPC.
**Parallelism**: 2A/2B/2C/2E can all start at Phase 2 entry. 2D requires 2E to be done first (data flows from Rust struct → TS interface).

```
Phase 1 done ─┬──→ 2A (Tauri commands + registration) ────────┐
              ├──→ 2B (launch hook) ─────────────────────────── ┤ → Phase 3
              ├──→ 2C (startup scan) ────────────────────────── ┤
              ├──→ 2D (health enrichment Rust) ──→ 2D-ts ───── ┤
              └──→ 2E (TypeScript version types) ─────────────────┘
```

| Task                            | Focus                                                                                                | Files                                                                            | Parallelizable?                       |
| ------------------------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------- |
| **2A** — Tauri Commands         | 4 IPC handlers + module declaration + handler registration                                           | `commands/version.rs` (NEW), `commands/mod.rs`, `src-tauri/src/lib.rs` (3 files) | Yes, from Phase 2 start               |
| **2B** — Launch Hook            | Call `upsert_version_snapshot()` after `LaunchOutcome::Succeeded`                                    | `commands/launch.rs` (1 file)                                                    | Yes, from Phase 2 start               |
| **2C** — Startup Version Scan   | Extend `run_metadata_reconciliation()` with background scan + `version-scan-complete` event          | `startup.rs` (1 file)                                                            | Yes, from Phase 2 start               |
| **2D** — Health Enrichment      | Extend `BatchMetadataPrefetch` + `ProfileHealthMetadata` with version fields; extend TS health types | `commands/health.rs`, `src/types/health.ts` (2 files)                            | Yes, from Phase 2 start               |
| **2E** — Frontend Version Types | TypeScript types mirroring Rust IPC payloads                                                         | `src/types/version.ts` (NEW, 1 file)                                             | Yes — can draft during Phase 1 review |

### Phase 3 — User Experience (~4 days, 5 tasks)

**Goal**: User-visible warning system, "Mark as Verified", trainer version field, health dashboard column, community import seeding.
**Parallelism**: 3A (community seeding) depends only on Phase 1; it can be done at Phase 2 start. 3B/3C/3D/3E are frontend tasks that can mostly run in parallel once 2D and 2E are complete.

| Task                                  | Focus                                                                          | Files                                                                                       | Depends On               |
| ------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- | ------------------------ |
| **3A** — Community Import Seeding     | Seed `version_snapshot` row (status='untracked') on `community_import_profile` | `commands/community.rs` (1 file)                                                            | Phase 1 (version_store)  |
| **3B** — Launch Page Warning Banner   | Persistent warning strip + info note for update-in-progress state              | `src/components/LaunchPanel.tsx`, `src/components/HealthBadge.tsx` (~2 files)               | 2D + 2E                  |
| **3C** — Profile Page Version Display | Card badge with version status; trainer version hint field in profile form     | `src/components/pages/ProfilesPage.tsx`, `src/components/ProfileFormSections.tsx` (2 files) | 2E                       |
| **3D** — Health Dashboard Enhancement | Version mismatch column + bulk check action in sortable dashboard              | `src/components/pages/HealthDashboardPage.tsx` (1 file)                                     | 2D                       |
| **3E** — Mark as Verified             | "Mark as Verified" user action in ProfileActions                               | `src/components/ProfileActions.tsx` (1 file)                                                | 2A (acknowledge command) |

### Phase 4 — Community & Analytics (~5 days, deferred)

Not blocked, but explicitly deferred to avoid scope creep. Revisit after Phase 3 ships.

| Task                            | Focus                                                 | Notes                                        |
| ------------------------------- | ----------------------------------------------------- | -------------------------------------------- |
| Community version advisory feed | Phase 4 community tap schema v2                       | Needs schema versioning decision first       |
| Compatibility scoring           | Launch success rates per version pair                 | Requires `launch_operations` query extension |
| SteamDB/PCGamingWiki cache      | External API integration via `external_cache_entries` | Table already exists; needs network layer    |

---

## Task Granularity Recommendations

### Keep These Tasks Together (Tightly Coupled)

- **1B (models + migrations)** — `VersionSnapshotRow` must exist before `migrate_8_to_9()` can reference it; same conceptual change
- **1C (version_store + mod.rs wrapper)** — The store module is private; wrapper methods in `mod.rs` are its only public surface. Splitting creates an unusable intermediate state
- **2A (version.rs + mod.rs + lib.rs)** — Command registration is 3 files but a single atomic unit; partial registration causes compile errors
- **2D (health.rs Rust + health.ts TypeScript)** — The Rust struct shape drives the TS interface; keeping them together prevents drift

### Split These Apart (Independent Concerns)

- **1D security fixes**: W1 (bounds in `community_index.rs`) and W2 (pinned_commit in `taps.rs`) are independent of the version tracking data model — merge separately if needed
- **3B warning banner vs. 3C profile display**: Both consume version state but render in different page contexts with different data requirements
- **2E (version.ts)**: The TypeScript type file is so small and isolated that it can be drafted speculatively during Phase 1 review without risk

---

## Dependency Analysis

### Hard Dependencies (cannot proceed without)

```
1B → 1C           VersionSnapshotRow struct must exist before version_store.rs can compile
1C → 2A           version_store wrapper methods must exist for Tauri commands to call
1C → 2B           upsert_version_snapshot() must exist before launch hook calls it
1C → 2C           version_store lookup must exist before startup scan can compare
1C → 3A           upsert_version_snapshot() needed for community import seeding
1A → 2A/2C        parse_manifest_full() needed for on-demand check and startup scan
2D → 3B/3D        Version fields in ProfileHealthMetadata drive the warning banner + dashboard
2E → 3B/3C        TypeScript version types needed for frontend components to type-check
2A → 3E           acknowledge_version_change command must exist for "Mark as Verified" button
```

### Soft Dependencies (conventions to follow, not blocking)

```
1D before Phase 2  Security fixes (W1, W2) should land before version store reads community data
2E early           TypeScript types are pure definition — draft at Phase 1 completion
```

### No Dependencies (fully independent)

- 1A, 1B, and 1D can all start on day 1 without coordination
- 2B, 2C, 2E start simultaneously at Phase 2 entry
- 3C (profile display) needs only 2E, not health enrichment

---

## File-to-Task Mapping

### New Files to Create (3 files)

| File                                                  | Task | Complexity                                                        |
| ----------------------------------------------------- | ---- | ----------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/version_store.rs` | 1C   | High — 4 functions + pure comparison function + row pruning logic |
| `src-tauri/src/commands/version.rs`                   | 2A   | Medium — 4 IPC handlers following existing pattern                |
| `src/types/version.ts`                                | 2E   | Low — 3 TypeScript interfaces                                     |

### Files to Modify (13 files)

| File                                                    | Task | Change Scope                                                                 |
| ------------------------------------------------------- | ---- | ---------------------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/models.rs`          | 1B   | Add `VersionSnapshotRow` + `VersionCorrelationStatus` enum                   |
| `crates/crosshook-core/src/metadata/migrations.rs`      | 1B   | Add `migrate_8_to_9()` + if-guard in `run_migrations()`                      |
| `crates/crosshook-core/src/metadata/mod.rs`             | 1C   | Add `mod version_store;` + 4 wrapper methods                                 |
| `crates/crosshook-core/src/steam/manifest.rs`           | 1A   | Add `parse_manifest_full()` + `ManifestData` struct                          |
| `crates/crosshook-core/src/metadata/community_index.rs` | 1D   | Add `MAX_VERSION_BYTES` + 2 bounds checks                                    |
| `crates/crosshook-core/src/community/taps.rs`           | 1D   | Add hex validation for `pinned_commit`                                       |
| `src-tauri/src/commands/mod.rs`                         | 2A   | Add `pub mod version;`                                                       |
| `src-tauri/src/lib.rs`                                  | 2A   | Register 4 new commands in `invoke_handler!`                                 |
| `src-tauri/src/commands/launch.rs`                      | 2B   | Hook `upsert_version_snapshot()` after `record_launch_finished()`            |
| `src-tauri/src/startup.rs`                              | 2C   | Extend `run_metadata_reconciliation()` with version scan + event emit        |
| `src-tauri/src/commands/health.rs`                      | 2D   | Extend `BatchMetadataPrefetch` + `ProfileHealthMetadata` with version fields |
| `src/types/health.ts`                                   | 2D   | Extend `ProfileHealthMetadata` TypeScript interface                          |
| `src-tauri/src/commands/community.rs`                   | 3A   | Seed version snapshot on `community_import_profile`                          |

### Reference Files (patterns to follow, no modification)

| File                                | Provides pattern for                                 |
| ----------------------------------- | ---------------------------------------------------- |
| `metadata/health_store.rs`          | Template for `version_store.rs` structure            |
| `metadata/profile_sync.rs`          | SHA-256 hashing pattern (`sha2::Sha256`)             |
| `metadata/launch_history.rs:56-119` | `record_launch_finished()` hook point                |
| `commands/community.rs:12-14`       | `map_error()` Tauri error helper                     |
| `commands/health.rs:81-156`         | `BatchMetadataPrefetch` bulk-load pattern            |
| `src/hooks/useProfileHealth.ts`     | Frontend hook pattern (`useReducer` + `useCallback`) |

---

## Optimization Opportunities

### Quick Wins (low effort, high signal)

1. **Task 2E first**: `src/types/version.ts` takes ~30 minutes and unblocks all Phase 3 frontend work. Can be drafted during Phase 1 code review
2. **Task 1D standalone merge**: Security fixes (W1 + W2) can be merged as a standalone PR before Phase 1 is complete — they have zero dependencies on new code
3. **Task 1A is ~50 lines of Rust**: `parse_manifest_full()` is a thin wrapper around the existing VDF parser; 2-3 hours of work including tests

### Parallelism Budget (Phase 1)

Three implementors can work simultaneously:

- Implementor A → Task 1A (`steam/manifest.rs`)
- Implementor B → Task 1B (`metadata/models.rs`, `migrations.rs`)
- Implementor C → Task 1D (`community_index.rs`, `taps.rs`)

As soon as Implementor B finishes 1B, Implementor B (or a 4th) starts 1C (version_store).

### Parallelism Budget (Phase 2)

Four implementors can work simultaneously:

- Implementor A → Task 2A (Tauri commands)
- Implementor B → Task 2B (launch hook)
- Implementor C → Task 2C (startup scan)
- Implementor D → Task 2D (health enrichment) + Task 2E (TypeScript types)

### Test Coverage Per Task

| Task | Recommended Tests                                                                                            |
| ---- | ------------------------------------------------------------------------------------------------------------ |
| 1A   | Unit tests for `parse_manifest_full()` in `manifest.rs` — parse fixture ACF strings                          |
| 1B   | Schema smoke test: `open_in_memory()` + run migrations, verify table/indexes exist                           |
| 1C   | In-memory DB tests: upsert→lookup→acknowledge lifecycle; pure function coverage for all 6 status transitions |
| 1D   | Bounds rejection test for 257-byte version strings; hex validation rejection for invalid commit strings      |
| 2A   | IPC contract tests (type-cast each command to expected signature)                                            |
| 2B   | Integration: confirm snapshot row exists after simulated successful launch                                   |
| 2C   | Unit: scan emits correct `scanned`/`mismatches` counts; `StateFlags != 4` skips correctly                    |
| 2D   | Confirm `version_status` field appears in `EnrichedProfileHealthReport`                                      |

---

## Implementation Strategy Recommendations

### 1. Respect the Frozen `parse_manifest()` Signature

`parse_manifest()` has existing callers. Add `parse_manifest_full()` alongside as a new public function returning `ManifestData { build_id, state_flags, last_updated }`. Do **not** modify the existing function's signature.

### 2. Multi-Row vs. Single-Row (Critical Distinction)

`health_snapshots` uses `INSERT OR REPLACE` (single-row upsert). `version_snapshots` is multi-row history. When writing `version_store.rs`, **do not copy the upsert pattern from `health_store.rs`** — use `INSERT` + a pruning `DELETE` in the same transaction to keep N most recent rows. This is mandatory (Security A7) and is the most likely copy-paste error.

### 3. `steam.app_id` Is Not In SQLite

The SQLite `profiles` table does NOT store `steam.app_id` — it lives in the TOML profile file only. Every Tauri command that needs `steam.app_id` must load the full `GameProfile` from `ProfileStore` first. Callers that already have `LaunchRequest` in scope (like task 2B) get `steam.app_id` for free; others (2A commands, 2C scan loop) must load from TOML.

### 4. `version_untracked` Is Not An Error

`status = 'untracked'` means "no baseline yet" — it must not show a warning badge, trigger health issues, or surface any UI indication beyond "no data". Only `game_updated`, `trainer_changed`, and `both_changed` trigger warnings. This is BR-4 and also reflected in `tasks/lessons.md` ("do not map 'no scan result yet' to NotFound/error").

### 5. Security Fixes Are Prerequisite, Not Parallel Risk

W1 (A6 bounds) and W2 (pinned_commit validation) should be merged **before** version store code starts reading from community version fields. They're low-risk standalone fixes — ship them first as a small PR to eliminate the security gap before new code depends on those paths.

### 6. Community Data Trust Boundary (BR-8/W3)

`community_profiles.trainer_version` and `game_version` are **display-only forever**. They are seeded into the initial `version_snapshot` row as `human_game_ver` (display label) but must **never** populate `steam_build_id` or drive mismatch comparison logic. Any task touching `commands/community.rs` (3A) must enforce this boundary explicitly in code comments.

### 7. DB Failure Must Not Block Launch (A8)

All version store calls go through `MetadataStore.with_conn*()`, which already returns `T::default()` when unavailable. No extra guard code is needed in the version store itself — but the Tauri command layer (2A, 2B) should log `tracing::warn!` on errors rather than propagating them. Never surface version tracking failure to the user as a launch blocker.

### 8. `request` Is Consumed Before the Version Hook Point in `commands/launch.rs`

**Critical implementation constraint (from code-analyzer):** In `launch_game()` and related command functions, the `LaunchRequest` (`request`) is moved into an async closure for `spawn_log_stream()` around lines 80/135. Any fields needed for `upsert_version_snapshot()` — specifically `steam.app_id`, `trainer.path`, and `profile_name` — **must be extracted (cloned/copied) before `spawn_log_stream()` is called**, while `request` is still owned. Attempting to access `request` after the spawn call will fail to compile. This extraction must happen in Task 2B.

### 9. `load_version_snapshots_for_profiles` Needed for Batch Prefetch

The batch health enrichment pattern in `commands/health.rs` (`BatchMetadataPrefetch`) requires bulk-loading all version snapshot data in a single query, not per-profile lookups. Task 1C (`version_store.rs`) must include a `load_version_snapshots_for_profiles()` function that accepts a slice of profile IDs and returns a `HashMap<String, VersionSnapshotRow>`. This is analogous to `load_health_snapshots()` being joined against `profiles` table.

### 10. Health Enrichment Is The Right UX Delivery Channel

For MVP (Phase 1+2), version mismatch surfaces **only** through the existing `EnrichedProfileHealthReport` pipeline. A dedicated hook is not needed until Phase 3 UI work. The `LaunchPanel.tsx` warning banner (3B) can read `version_status` from the health data already loaded by `useProfileHealth`. If a standalone `useVersionStatus` hook is added (Phase 3), it should listen for the `version-scan-complete` Tauri event — see `lib.rs:73-101` for the existing health scan event pattern to follow.

---

## Summary Table

| Phase             | Tasks | Files Created | Files Modified | Parallelism           | MVP?    |
| ----------------- | ----- | ------------- | -------------- | --------------------- | ------- |
| 1 — Foundation    | 4     | 1             | 5              | High (3 parallel)     | Yes     |
| 2 — Integration   | 5     | 2             | 6              | High (4 parallel)     | Yes     |
| 3 — UX Polish     | 5     | 0             | 5              | Moderate (3 parallel) | No      |
| 4 — Analytics     | ~4    | 0             | ~4             | TBD                   | No      |
| **Total (P1+P2)** | **9** | **3**         | **11**         | —                     | **MVP** |

**Critical path**: 1B → 1C → 2A/2B/2C/2D → 3B/3D/3E

**Earliest shippable state**: Phase 2 complete — version snapshots record on launch success, startup scan detects mismatches, health dashboard enriched, TypeScript types ready for Phase 3.
