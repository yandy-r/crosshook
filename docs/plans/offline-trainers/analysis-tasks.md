# Task Analysis: Offline-First Trainer Management

**Date**: 2026-03-31
**Issue**: #44
**Source documents**: `feature-spec.md` (authoritative), `shared.md`, `research-recommendations.md`, `research-practices.md`

---

## Executive Summary

The offline-trainers feature is a well-scoped cross-cutting enhancement layered onto existing infrastructure. The codebase is ~80% offline-capable already — `sha2`, `hash_trainer_file`, `sha256_hex`, SQLite migration machinery, and `HealthIssue`/`ReadinessCheckResult` patterns are all in place. The remaining work is structural: a new `offline/` module (per `feature-spec.md`, which supersedes `research-practices.md`'s no-new-module guidance), migration 13 (3 tables), and frontend wiring.

**Key architectural decision (from feature-spec.md)**: The trainer type system is **data-driven via a TOML catalog** (identical pattern to `launch/catalog.rs`), not a simple Rust enum. `OfflineCapability` is the only compiled Rust enum. This has a significant task implication: catalog loading infrastructure must precede all classification work.

**No new crate dependencies.** `sha2`, `rusqlite`, `chrono`, and `uuid` are already present.

Maximum parallel agents in Phase 1: **3**. Phases 2–4 each support 2–3 simultaneous agents.

---

## Cross-Cutting Constraints

These apply to **every task** in every phase. They are not phase-specific and must be understood before any implementation begins.

### Data Locality (portable vs. machine-local)

| Data                                            | Storage      | Reasoning                                       |
| ----------------------------------------------- | ------------ | ----------------------------------------------- |
| `trainer_type` (catalog ID string)              | TOML profile | Portable — shared in community profiles         |
| `offline_activated`, `offline_key_activated_at` | SQLite only  | Machine-bound — Aurora HWID keys don't transfer |
| `readiness_state`, `readiness_score`            | SQLite only  | Computed from local filesystem state            |
| Trainer SHA-256 hash                            | SQLite only  | Machine-local file reference                    |

**Rule**: Never write `offline_activated`, activation timestamps, or hash values to the TOML profile. They belong in SQLite exclusively.

### Path Resolution

Always use `profile.effective_profile()` (or equivalent resolved paths) when checking filesystem state — do NOT read `trainer.path` directly. Raw `trainer.path` may be empty for profiles using `storage_profile()` overrides.

### MetadataStore Availability

Every SQLite call must handle `MetadataStore::disabled()`. The pattern:

```rust
store.with_conn("action label", |conn| { ... })
    .unwrap_or_default()  // non-critical metadata
```

When `is_available()` is false, offline readiness must return `readiness_state = "unconfigured"`, not an error.

### Async Hash Computation

Any call to `hash_trainer_file()` **must** be wrapped in `tokio::task::spawn_blocking()`. Hashing a 50MB trainer takes ~50ms — blocking the async runtime is unacceptable. All four Tauri offline commands that trigger hashing need this.

### TypeScript / Rust Type Parity

TypeScript types must exactly mirror Rust struct field names. Rust uses `snake_case` with `#[serde(rename_all = "snake_case")]`. TypeScript interfaces must use the same snake_case names — do not camelCase them.

---

## Phase-Independent Security Tasks

These are small, isolated fixes that can run at any time during implementation — they do not block or depend on any phase.

| Task                  | File                | Change                                                                                                 | Source       |
| --------------------- | ------------------- | ------------------------------------------------------------------------------------------------------ | ------------ |
| Git command hardening | `community/taps.rs` | Add `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0` to `git_command()` | Advisory A-6 |
| DB permissions check  | `metadata/db.rs`    | Verify `fs::set_permissions(0o600)` covers WAL/SHM sidecar files, not just the main `.db` file         | Advisory W-2 |

Both are 3–5 line changes. Assign to whichever agent has slack capacity during Phase 1.

---

## Key Decisions (Resolved)

Both `research-practices.md` and `feature-spec.md` conflict on two points. **`feature-spec.md` is authoritative** per `shared.md`:

1. **New `offline/` module**: `feature-spec.md` calls for `crates/crosshook-core/src/offline/` with 5 files. `research-practices.md` said "no new module for v1." **Decision: create the `offline/` module.** The catalog infrastructure and readiness scoring have sufficient scope to justify it.

2. **Hash storage**: `feature-spec.md` requires a new `trainer_hash_cache` table (migration 13). `research-practices.md` suggested reusing `version_snapshots.trainer_file_hash`. **Decision: new table.** Seed it from `version_snapshots.trainer_file_hash` at migration time (zero-cost bootstrap for existing users).

---

## IPC Contract Constraint

**TypeScript tasks (4A, 2C, 3B, 3C) must not begin until the Tauri command signatures in `commands/offline.rs` are finalized** in TASK 1D. Scaffold the hook and component files with the agreed type signatures, but do not merge frontend work that calls commands before 1D lands.

The four IPC commands to agree upfront:

- `check_offline_readiness(name: string) → OfflineReadinessReport`
- `batch_offline_readiness() → OfflineReadinessReport[]`
- `verify_trainer_hash(profile_name: string, trainer_path: string) → TrainerHashResult`
- `get_trainer_type_catalog() → TrainerTypeEntry[]`

---

## Recommended Phase Structure

### Phase 1: Foundation (3 parallel task groups → 1 convergence task)

All Phase 1 foundation tasks (1A, 1B, 1C) are independent and can run simultaneously. Task 1D is the convergence point.

```
[1A: offline/ module + catalog] ──┐
[1B: profile model + settings]  ──┼──► [1D: readiness scoring + hash.rs + commands/offline.rs]
[1C: SQLite migration 13]        ──┘
```

### Phase 2: Launch Integration (2 parallel → 1 convergence)

Can start as soon as Phase 1 is complete. Tasks 2A and 2B are independent.

```
[2A: launch validation errors]   ──┐
[2B: community tap offline]      ──┼──► [2C: frontend launch pre-flight wiring]
```

### Phase 3: Health + Community UI (2 parallel → 1 convergence)

Can start once Phase 2 is complete (2A needed for health integration). Task 4A (TypeScript types) can be done in parallel with Phase 3 since it only requires 1A to be done.

```
[3A: health system integration]  ──┐
[3B: community browser UI]       ──┼──► [3C: health dashboard offline section]
[4A: TypeScript types]           ──┘    (feeds into 4B, 4C, 4D)
```

### Phase 4: UI Components + Wiring (3 parallel → 1 final integration)

Requires Phase 3 complete plus 4A complete.

```
[4B: offline badge + panel components] ──┐
[4C: trainer type form + profile UI]   ──┼──► [4D: final wiring + CSS]
```

---

## Task Granularity Recommendations

Each task should touch **1–3 files** at the same module boundary. Keep Rust and TypeScript tasks separate — they require different context and can always run in parallel.

### Phase 1 Tasks

#### TASK 1A — `offline/` module skeleton + trainer type catalog (Rust)

**Rationale**: The TOML catalog loader is the architectural foundation — downstream tasks need `OfflineCapability` and `TrainerTypeCatalog` types. Follows the exact pattern of `launch/catalog.rs` with `OnceLock` + embedded default + user override.

| File                                                | Action                                                                                         |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/offline/mod.rs`          | CREATE — module root, re-exports                                                               |
| `crates/crosshook-core/src/offline/trainer_type.rs` | CREATE — `OfflineCapability` enum, `TrainerTypeEntry`, `TrainerTypeCatalog`, `OnceLock` loader |
| `crates/crosshook-core/src/offline/network.rs`      | CREATE — `probe_network_connectivity()` (simple `TcpStream` probe, optional)                   |
| `assets/default_trainer_type_catalog.toml`          | CREATE — 6 entries: standalone, cheat_engine, aurora, wemod, plitch, unknown                   |
| `crates/crosshook-core/src/lib.rs`                  | MODIFY — add `pub mod offline;`                                                                |

**Parallelism**: Independent of 1B and 1C.
**Complexity**: Medium (catalog loader pattern, serde, OnceLock).
**Test surface**: `TrainerTypeCatalog::load()` with in-memory TOML strings; `OfflineCapability` serde roundtrip.
**Critical gotcha**: Must match the `OptimizationCatalog` loading pattern exactly (`DEFAULT_CATALOG_TOML = include_str!(...)`, `OnceLock<OptimizationCatalog>`, user override merge). Reference `launch/catalog.rs:1–120`.

---

#### TASK 1B — Profile model extension + settings (Rust)

**Rationale**: Adding `trainer_type: String` to `TrainerSection` is a low-risk model-only change. The field defaults to `"unknown"` for backward compat. `settings/mod.rs` gets `offline_mode: bool`. Both changes are serde-only with no business logic.

| File                                          | Action                                                                                                                             |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models.rs` | MODIFY — add `trainer_type: String` to `TrainerSection` (serde default `"unknown"`); keep existing `kind: String` for display name |
| `crates/crosshook-core/src/settings/mod.rs`   | MODIFY — add `offline_mode: bool` with `#[serde(default)]`                                                                         |

**Parallelism**: Independent of 1A and 1C.
**Complexity**: Low (~10 LOC).
**Test surface**: Verify existing profile TOML files with no `trainer_type` field still deserialize (serde `#[default]` roundtrip). See legacy profile test pattern in `profile/legacy.rs`.
**Critical gotcha**: `TrainerSection.kind` is serialized as `type` in TOML (check `serde(rename)` on that field). The new `trainer_type` field must not conflict with the existing `type` rename. Consider serializing `trainer_type` as `"trainer_type"` (no rename).

---

#### TASK 1C — SQLite migration 13 + `offline_store.rs` (Rust)

**Rationale**: All three new tables belong in a single migration. `offline_store.rs` provides CRUD for all three tables, following the shape of `health_store.rs` exactly.

| File                                                  | Action                                                                                               |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/offline_store.rs` | CREATE — CRUD for `trainer_hash_cache`, `offline_readiness_snapshots`, `community_tap_offline_state` |
| `crates/crosshook-core/src/metadata/migrations.rs`    | MODIFY — add `migrate_12_to_13()` block; the runner's `if version < 13` pattern                      |
| `crates/crosshook-core/src/metadata/mod.rs`           | MODIFY — register `offline_store` and expose public methods through `MetadataStore` facade           |

**Parallelism**: Independent of 1A and 1B.
**Complexity**: Medium (3 tables, 8–10 CRUD functions, follows `health_store.rs` pattern).
**Test surface**: `open_in_memory()` + `run_migrations()` then upsert/load round-trips for each table.
**Critical gotcha**: Confirm migration 13 is not claimed by any in-flight feature branches (`git branch -r | xargs git log --oneline` for migration references). The `trainer_hash_cache` schema uses `UNIQUE(profile_id, file_path)` — bootstrap with data from `version_snapshots.trainer_file_hash` where non-null (one-time seed in migration body). The `offline_readiness_snapshots` table has a broader schema than `health_snapshots` — note `blocking_reasons TEXT` is a JSON array, not a count.

---

#### TASK 1D — Readiness scoring + hash caching + `commands/offline.rs` (Rust)

**Depends on**: 1A (OfflineCapability types), 1B (TrainerSection with trainer_type), 1C (offline_store CRUD)
**Rationale**: This is the Phase 1 convergence point. `offline/readiness.rs` computes the weighted composite score; `offline/hash.rs` wraps `hash_trainer_file()` with stat-based cache invalidation; `commands/offline.rs` exposes both to the frontend.

| File                                             | Action                                                                                                                                                   |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/offline/readiness.rs` | CREATE — `check_offline_readiness()` returning `OfflineReadinessReport`; uses `HealthIssue` pattern; pure function, no I/O inside scoring                |
| `crates/crosshook-core/src/offline/hash.rs`      | CREATE — `verify_and_cache_trainer_hash()` wrapping `hash_trainer_file()` with mtime-based fast path; calls `offline_store::upsert_trainer_hash_cache()` |
| `src-tauri/src/commands/offline.rs`              | CREATE — 4 IPC commands: `check_offline_readiness`, `batch_offline_readiness`, `verify_trainer_hash`, `check_network_status`                             |
| `src-tauri/src/commands/mod.rs`                  | MODIFY — add `pub mod offline;`                                                                                                                          |
| `src-tauri/src/lib.rs`                           | MODIFY — register offline commands in `invoke_handler!`                                                                                                  |

**Parallelism**: Sequential after 1A+1B+1C. 5 files is at the upper limit — consider splitting `offline.rs` commands from the Rust scoring functions if agent context becomes tight.
**Complexity**: Medium-High (scoring weights, stat-based invalidation, async hash on large files via `spawn_blocking`).
**Critical gotcha**: `check_offline_readiness` is informational (Warning, not Fatal) — do NOT block launch here. The score informs, never blocks. Also: `OfflineCapability::Unknown` caps at 90, so the Unknown variant must be handled explicitly in scoring logic (do not default to 0). The scoring function must be pure (no I/O) — I/O (file existence, hash lookup) happens in the Tauri command, which passes resolved values to the pure scoring function. This matches the `onboarding/readiness.rs` pattern (`check_system_readiness` does I/O; `evaluate_checks` is pure).

---

### Phase 2 Tasks

#### TASK 2A — Launch validation errors (Rust)

**Depends on**: Phase 1 complete
**Rationale**: Adds `OfflineReadinessInsufficient` to `ValidationError` and wires pre-flight check into the Tauri launch commands. Non-fatal severity — warning, not block.

| File                                          | Action                                                                                                                             |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs` | MODIFY — add `OfflineReadinessInsufficient { score: u8, reasons: Vec<String> }` variant to `ValidationError` with Warning severity |
| `src-tauri/src/commands/launch.rs`            | MODIFY — call `check_offline_readiness()` before launch; surface as warning annotation on the launch result                        |

**Complexity**: Low (~20 LOC net new).
**Critical gotcha**: `ValidationError` variants have severity levels — confirm where `Warning` vs `Fatal` is expressed in the enum (check existing `ValidationError` structure in `launch/request.rs:173-222`). The offline check must be skipped entirely when no trainer path is configured (game-only profiles must still launch cleanly).

---

#### TASK 2B — Community tap offline wiring (Rust)

**Depends on**: 1C (community_tap_offline_state table)
**Rationale**: Git command hardening (3-line env var addition) and `is_tap_available_offline()` method. Largely independent of Phase 1 model changes.

| File                                          | Action                                                                                                                                                                                                                        |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/community/taps.rs` | MODIFY — add `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0` to `git_command()`; add `is_tap_available_offline()` method; update `sync_tap()` to write `community_tap_offline_state` on success |

**Complexity**: Low (~25 LOC).
**Critical gotcha**: The git hardening env vars must be added to the existing `git_command()` helper function, not scattered at each call site. Verify `git_command()` exists as a shared builder (check `community/taps.rs`). The `is_tap_available_offline()` check is `workspace.local_path.exists()` — no git subprocess needed.

---

#### TASK 2C — Frontend launch pre-flight wiring (TypeScript/React)

**Depends on**: 1D (commands/offline.rs exists), 2A (ValidationError has offline variant)
**Rationale**: Wires the offline pre-flight check into the React launch flow. The `useLaunchState.ts` hook calls `check_offline_readiness` before `launchTrainer()` (not before `launchGame()`).

| File                                  | Action                                                                                                        |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `src/hooks/useLaunchState.ts`         | MODIFY — add offline pre-flight gate before `launchTrainer()` call; surfaces offline warning without blocking |
| `src/components/LaunchPanel.tsx`      | MODIFY — add `CollapsibleSection` for pre-flight validation results (follows existing pattern)                |
| `src/components/pages/LaunchPage.tsx` | MODIFY — thread offline readiness state from hook to panel                                                    |

**Complexity**: Medium (async state, warning display, non-blocking UX).
**Critical gotcha**: The offline check gates `launchTrainer()`, not `launchGame()`. A failing offline check should expand the pre-flight panel and show a warning, but the "Launch Trainer" button should remain enabled (user can override). This is a UX decision from `feature-spec.md` — informational, never blocking.

---

### Phase 3 Tasks

#### TASK 3A — Health system integration (Rust)

**Depends on**: Phase 1 complete
**Rationale**: Injects offline readiness into the existing health check pipeline. `profile/health.rs` calls `check_offline_readiness()` as an additional check layer. `commands/health.rs` surfaces the offline score alongside existing health data.

| File                                          | Action                                                                                                                                            |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/health.rs` | MODIFY — call `check_offline_readiness()` and merge results into `ProfileHealthReport`; persist `offline_readiness_snapshots` via `offline_store` |
| `src-tauri/src/commands/health.rs`            | MODIFY — extend `batch_validate_profiles` and `get_profile_health` to include offline readiness data                                              |

**Complexity**: Medium (health score composition, snapshot persistence).
**Critical gotcha**: Offline readiness should be a separate dimension in the health report, not merged into the main `HealthStatus` score. Adding offline score to the health check total would change the existing score semantics for all users. Keep offline readiness as an annotation alongside, not an input to, the existing score.

---

#### TASK 3B — Community browser cache status (TypeScript/React)

**Depends on**: 2B (tap offline state written to DB), 1D (commands/offline.rs)
**Rationale**: Displays "last synced" timestamps in the community browser and gracefully falls back to cached profiles when offline.

| File                                  | Action                                                                                                                               |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `src/components/CommunityBrowser.tsx` | MODIFY — display `community_tap_offline_state.cached_at` per tap; show "Cached profiles (last synced: X)" banner when tap sync fails |
| `src-tauri/src/commands/community.rs` | MODIFY — `sync_tap` handles network failure gracefully, returns cached profiles with a `from_cache: true` flag                       |

**Complexity**: Low-Medium (~30 LOC React, ~15 LOC Rust).
**Critical gotcha**: Community profile **install** (not browse) must fail with a clear "Network required" message when offline. Browse from cache is allowed. Ensure the command returns a discriminated result type, not a generic error, so the UI can distinguish "offline, showing cache" from "sync error".

---

#### TASK 3C — Health dashboard offline section (TypeScript/React)

**Depends on**: 3A (offline scores in health commands), 1E (useOfflineReadiness hook)

| File                                           | Action                                                                                                                                                          |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/pages/HealthDashboardPage.tsx` | MODIFY — add offline readiness column/section to the sortable health table                                                                                      |
| `src/hooks/useOfflineReadiness.ts`             | CREATE — `useReducer` hook following `useProfileHealth` pattern; `batchCheck` + `revalidateSingle` surface; listens for `offline-readiness-updated` Tauri event |

**Complexity**: Medium (hook pattern, reducer, batch loading state).
**Critical gotcha**: `useOfflineReadiness` must follow the `useProfileHealth` hook pattern exactly (`batch-loading / batch-complete / single-complete / error` action shapes). Do not invent a new reducer shape. Reference `src/hooks/useProfileHealth.ts`.

---

### Phase 4 Tasks

#### TASK 4A — TypeScript types (TypeScript)

**Depends on**: 1A (OfflineCapability names finalized)
**Can run concurrently with Phase 3**.

| File                   | Action                                                                                               |
| ---------------------- | ---------------------------------------------------------------------------------------------------- |
| `src/types/offline.ts` | CREATE — `OfflineCapability`, `TrainerTypeEntry`, `OfflineReadinessReport`, `OfflineReadinessChecks` |
| `src/types/profile.ts` | MODIFY — add `trainer_type?: string` to `TrainerSection` interface                                   |
| `src/types/index.ts`   | MODIFY — re-export from `./offline`                                                                  |

**Complexity**: Low (types only, no logic).
**Critical gotcha**: The `OfflineCapability` union type must use the exact snake_case strings from the Rust serde enum serialization: `'full' | 'full_with_runtime' | 'conditional_key' | 'conditional_session' | 'online_only' | 'unknown'`. Verify these match the TOML catalog `offline_capability` strings.

---

#### TASK 4B — Offline badge + readiness panel components (TypeScript/React)

**Depends on**: 4A (types defined)

| File                                       | Action                                                                                                                                                           |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/OfflineStatusBadge.tsx`    | CREATE — reuses `crosshook-status-chip` CSS pattern from `HealthBadge.tsx`; 5 states: green (≥80), amber (50–79), red (<50), unknown (grey), computing (spinner) |
| `src/components/OfflineReadinessPanel.tsx` | CREATE — expandable detail panel with per-check results and blocking reasons                                                                                     |

**Complexity**: Low-Medium (2 new components, CSS).
**Critical gotcha**: The badge must use the same `crosshook-status-chip` CSS class pattern as `HealthBadge.tsx` for visual consistency — do not introduce new CSS class naming. Score thresholds: ≥80 green, 50–79 amber, <50 red (per `feature-spec.md` UX section).

---

#### TASK 4C — Trainer type form + profile UI (TypeScript/React)

**Depends on**: 4A (types), 1B (profile model has trainer_type)

| File                                         | Action                                                                                              |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `src/components/ProfileFormSections.tsx`     | MODIFY — add trainer type `ThemedSelect` dropdown using catalog entries; default `"unknown"`        |
| `src/components/pages/ProfilesPage.tsx`      | MODIFY — integrate `OfflineStatusBadge` on profile list rows; show `trainer_type` in profile header |
| `src/components/OfflineTrainerInfoModal.tsx` | CREATE — Aurora/WeMod instructional modal; content driven by `TrainerTypeEntry.info_modal` field    |

**Complexity**: Medium (form integration, modal).
**Critical gotcha**: The trainer type dropdown must populate from the catalog (`get_trainer_type_catalog` command) not from a hardcoded list. This makes the component async on mount. The `OfflineTrainerInfoModal` is triggered by `info_modal` field on the catalog entry — only Aurora (`"aurora_offline_setup"`) and WeMod (`"wemod_offline_info"`) have this set. Modal should only show for these two types.

---

#### TASK 4D — Final wiring + CSS (Mixed)

**Depends on**: All Phase 4 tasks complete

| File                       | Action                                                                                           |
| -------------------------- | ------------------------------------------------------------------------------------------------ |
| `src/styles/variables.css` | MODIFY — add `--offline-ready`, `--offline-partial`, `--offline-not-ready` CSS custom properties |

**Complexity**: Very low (~5 LOC).
**Note**: If `variables.css` is needed earlier by badge components (likely), move this to run with TASK 4B.

---

## Dependency Analysis

```
Phase 1 ──────────────────────────────────────────
  1A (offline/ module) ──┐
  1B (profile model)   ──┼──► 1D (scoring + commands) ──► ALL Phase 2+
  1C (migration 13)    ──┘

Phase 2 ──────────────────────────────────────────
  2A (launch errors)  ──┐
  2B (community tap)  ──┼──► 2C (frontend launch)
                         │
Phase 3 ──────────────────────────────────────────
  3A (health Rust)    ──┐
  3B (community UI)   ──┼──► 3C (health dashboard)
  4A (TS types)*      ──┘

Phase 4 ──────────────────────────────────────────
  4B (badge/panel)    ──┐
  4C (form/profile)   ──┼──► 4D (CSS + final wiring)

* 4A can begin after 1A completes (OfflineCapability names finalized)
```

**Blocking chains** (longest critical path):

```
1A → 1D → 2A → 3A → 3C → 4D   (Rust backend chain, ~6 sequential tasks)
1A → 4A → 4B → 4D              (TypeScript chain, ~4 sequential tasks)
```

---

## File-to-Task Mapping

| File                                       | Task | Action |
| ------------------------------------------ | ---- | ------ |
| `offline/mod.rs`                           | 1A   | CREATE |
| `offline/trainer_type.rs`                  | 1A   | CREATE |
| `offline/network.rs`                       | 1A   | CREATE |
| `assets/default_trainer_type_catalog.toml` | 1A   | CREATE |
| `lib.rs`                                   | 1A   | MODIFY |
| `profile/models.rs`                        | 1B   | MODIFY |
| `settings/mod.rs`                          | 1B   | MODIFY |
| `metadata/offline_store.rs`                | 1C   | CREATE |
| `metadata/migrations.rs`                   | 1C   | MODIFY |
| `metadata/mod.rs`                          | 1C   | MODIFY |
| `offline/readiness.rs`                     | 1D   | CREATE |
| `offline/hash.rs`                          | 1D   | CREATE |
| `commands/offline.rs`                      | 1D   | CREATE |
| `commands/mod.rs`                          | 1D   | MODIFY |
| `src-tauri/lib.rs`                         | 1D   | MODIFY |
| `launch/request.rs`                        | 2A   | MODIFY |
| `commands/launch.rs`                       | 2A   | MODIFY |
| `community/taps.rs`                        | 2B   | MODIFY |
| `hooks/useLaunchState.ts`                  | 2C   | MODIFY |
| `components/LaunchPanel.tsx`               | 2C   | MODIFY |
| `pages/LaunchPage.tsx`                     | 2C   | MODIFY |
| `profile/health.rs`                        | 3A   | MODIFY |
| `commands/health.rs`                       | 3A   | MODIFY |
| `components/CommunityBrowser.tsx`          | 3B   | MODIFY |
| `commands/community.rs`                    | 3B   | MODIFY |
| `pages/HealthDashboardPage.tsx`            | 3C   | MODIFY |
| `hooks/useOfflineReadiness.ts`             | 3C   | CREATE |
| `types/offline.ts`                         | 4A   | CREATE |
| `types/profile.ts`                         | 4A   | MODIFY |
| `types/index.ts`                           | 4A   | MODIFY |
| `components/OfflineStatusBadge.tsx`        | 4B   | CREATE |
| `components/OfflineReadinessPanel.tsx`     | 4B   | CREATE |
| `components/ProfileFormSections.tsx`       | 4C   | MODIFY |
| `pages/ProfilesPage.tsx`                   | 4C   | MODIFY |
| `components/OfflineTrainerInfoModal.tsx`   | 4C   | CREATE |
| `styles/variables.css`                     | 4D   | MODIFY |

---

## Optimization Opportunities

### Start TypeScript Types Early

TASK 4A (`types/offline.ts`) only needs `OfflineCapability` enum names from 1A. Once 1A defines the Rust enum, 4A can begin immediately — it does not need the full 1D convergence. This unblocks frontend component work 1–2 tasks earlier.

**Revised parallel opportunity:**

```
1A → 4A (immediately after 1A) → 4B (frontend components ready before Phase 3 even starts)
```

### CSS Variables Before Badge Components

`variables.css` is a 5-LOC change that blocks badge styling. Move it into TASK 4B (run first in that task) rather than its own TASK 4D, eliminating a sequential dependency.

### Migration 13 Can Seed from `version_snapshots`

The migration body for `trainer_hash_cache` should include a one-time `INSERT INTO trainer_hash_cache SELECT ...` from `version_snapshots.trainer_file_hash` where non-null. This bootstraps hash cache for all existing profiles at migration time, so users immediately see hash validation results without needing to re-save profiles. Zero additional code — just a SQL INSERT in the migration body.

### 2B (Community Tap) is Decoupled

TASK 2B (`community/taps.rs`) can begin in Phase 1 alongside 1A/1B/1C. It only depends on 1C for the `community_tap_offline_state` write, but the git hardening and `is_tap_available_offline()` changes are fully independent. Split the git hardening into the Phase 1 window.

---

## Implementation Strategy Recommendations

### 1. Validate Migration Numbering First

Before starting 1C, confirm no other feature branches claim migration 13:

```
git log --all --oneline --grep="migration" | head -20
# Also check for version 13 references
git grep "user_version.*13" -- "*.rs"
```

### 2. The `commands/offline.rs` Pattern to Follow

The new `commands/offline.rs` should follow `commands/health.rs` as its template — `State<'_, MetadataStore>` parameter, `spawn_blocking` for hash computation, `Result<T, String>` return, `.map_err(|e| e.to_string())` conversion. Avoid the anti-pattern of doing path resolution inside `crosshook-core` — pass resolved paths from the command layer.

### 3. Score Function Must Be Pure

`check_offline_readiness()` in `offline/readiness.rs` must be a pure function: accept `GameProfile` + `Option<StoredHash>` + `Vec<PathBuf>` (tap paths) as inputs; return `OfflineReadinessReport`. All I/O (file existence, hash DB lookup) happens in the Tauri command before calling this function. This is the same pattern as `onboarding/readiness.rs::evaluate_checks()` — mandatory for unit testability.

### 4. TOML Catalog Loader Must Be Singleton

Use `OnceLock<TrainerTypeCatalog>` exactly as `launch/catalog.rs` does. The `get_trainer_type_catalog()` Tauri command returns a serialized snapshot of the loaded catalog for the frontend dropdown. Community tap contributions to the catalog are merged at load time.

### 5. Backward Compatibility for `trainer_type` Field

`TrainerSection.trainer_type` must have `#[serde(default)]` pointing to a function returning `"unknown"`. Verify that existing profile TOML files without this field still round-trip cleanly. Add a test in the 1B task using a minimal legacy profile fixture (copy the pattern from `profile/legacy.rs` tests).

### 6. Health Integration Must Be Additive, Not Replacing

When wiring offline readiness into `profile/health.rs` (TASK 3A), add offline results as an _additional_ annotation to `ProfileHealthReport`, not as a modifier to `HealthStatus::score`. Changing the existing score semantics would break the Health Dashboard sort order for all users immediately upon upgrade.

### 7. Frontend Trainer Type Dropdown Requires `get_trainer_type_catalog` Command

`ProfileFormSections.tsx` must invoke `get_trainer_type_catalog` at mount time to populate the trainer type `ThemedSelect`. Add this command to `commands/offline.rs` in TASK 1D. The frontend component should handle loading/error states (catalog load failure → show fallback hardcoded list with just "Unknown").

---

## Estimated Task Sizes

| Task | Files | Complexity  | Rust LOC | TS LOC | Notes                              |
| ---- | ----- | ----------- | -------- | ------ | ---------------------------------- |
| 1A   | 5     | Medium      | ~150     | 0      | Catalog loader + OnceLock pattern  |
| 1B   | 2     | Low         | ~15      | 0      | Serde defaults only                |
| 1C   | 3     | Medium      | ~120     | 0      | 3 tables, CRUD functions           |
| 1D   | 5     | Medium-High | ~200     | 0      | Scoring + hash cache + 4 commands  |
| 2A   | 2     | Low         | ~25      | 0      | New ValidationError variant        |
| 2B   | 1     | Low         | ~30      | 0      | Git hardening + offline check      |
| 2C   | 3     | Medium      | 0        | ~80    | Hook gate + panel wiring           |
| 3A   | 2     | Medium      | ~60      | 0      | Health annotation + snapshot write |
| 3B   | 2     | Low-Medium  | ~20      | ~50    | Cache banner + fallback            |
| 3C   | 2     | Medium      | 0        | ~100   | Hook + dashboard section           |
| 4A   | 3     | Low         | 0        | ~80    | Types only                         |
| 4B   | 2     | Low-Medium  | 0        | ~100   | 2 new components                   |
| 4C   | 3     | Medium      | 0        | ~120   | Form dropdown + modal              |
| 4D   | 1     | Very Low    | 0        | ~10    | CSS vars                           |

**Total new code estimate**: ~620 Rust LOC, ~540 TypeScript LOC. Consistent with `research-practices.md`'s "~180 LOC for v1 minimal scope" (that estimate excluded the full UI layer and the TOML catalog infrastructure).

---

## Reconciliation Notes (from teammate analysis)

Teammate analyses (`code-analyzer`, `context-synthesizer`) confirmed the overall phase structure. Points of divergence resolved here:

**Enum placement**: `code-analyzer` placed `OfflineCapability`/`TrainerType` enums in `metadata/models.rs`. Per `feature-spec.md` they belong in `offline/trainer_type.rs`. **Decision: `offline/trainer_type.rs`** — they are not storage-layer enums; they model trainer behavior.

**Catalog file location**: `code-analyzer` suggested `launch/trainer_type_catalog.rs`. Per `feature-spec.md`: `offline/trainer_type.rs` + `assets/default_trainer_type_catalog.toml`. **Decision: `offline/trainer_type.rs`** — trainer type classification is an offline concern, not a launch concern.

**`profile/models.rs` timing**: `code-analyzer` placed this in Phase 2. Since it's a simple additive `#[serde(default)]` field with no business logic, it belongs in **Phase 1 (TASK 1B)** — parallel with 1A and 1C. Deferring it would push all downstream profile-type work one phase later unnecessarily.

**Within-Phase 1 ordering**: `code-analyzer` correctly noted that migrations must complete before the store functions are written (you can't write CRUD functions for tables that don't exist yet). Within TASK 1C, write `migrations.rs` first, then `offline_store.rs`. This is a within-task ordering constraint, not a separate task dependency.
