# Profile Health Dashboard — Task Structure Analysis (v2)

The feature is well-scoped: one new Rust module, one new Tauri commands file, TypeScript types + hook + badge component, and integration into the existing profile list page. The Rust and TypeScript work streams are largely parallel in Phase A. Phases B and C are parallel after Phase A. Phase D depends on Phase B.

**v2 update**: This analysis reflects the full 4-phase plan (A/B/C/D) from the v2 feature spec, which adds metadata enrichment (Phase B), startup integration (Phase C), and health persistence (Phase D) on top of the Phase A MVP. The `commands/health.rs` file is **separate** from `commands/profile.rs` per spec.

---

## Executive Summary

- **Total new files (full feature)**: 8 (`health.rs`, `commands/health.rs`, `health.ts`, `useProfileHealth.ts`, `HealthBadge.tsx`, `ProfileHealthDashboard.tsx`, `[Phase D] health_store.rs`)
- **Total modified files**: 8 (`request.rs`, `profile/mod.rs`, `commands/mod.rs`, `lib.rs`, `types/index.ts`, `ProfilesPage.tsx`, `shared.rs`, `[Phase D] metadata/mod.rs` + `migrations.rs`)
- **Zero new crate dependencies** — all stdlib, already-present crates, existing CSS patterns
- **Phase A (MVP)** has two fully parallel tracks: Rust core (5 tasks) and TypeScript foundation (2 tasks), converging at Tauri commands then integration
- **Phases B and C** are both parallel after Phase A and independent of each other
- **Phase D** depends on Phase B (metadata queries must work before persistence is meaningful)
- **Critical path (Phase A)**: S2 → A5 → A7 → A9
- **Critical path (full feature)**: S2 → A5 → A7 → A9 → B-enrich → D-migration

---

## Recommended Phase Structure

### Phase S — Security Pre-Ship (run in parallel, before any Phase A work)

Both security tasks are atomic, independent, and affect all subsequent IPC work:

| Task                                                               | File(s)                                                                           | Why first                                                                                                                   |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| S1 — Enable CSP                                                    | `src-tauri/tauri.conf.json`                                                       | One-line change; security W-1; new IPC commands increase exposure surface                                                   |
| S2 — Verify/move `sanitize_display_path()` to `commands/shared.rs` | `src-tauri/src/commands/shared.rs` (existing), `src-tauri/src/commands/launch.rs` | Path sanitization must be importable via `use super::shared::sanitize_display_path;` before `commands/health.rs` can use it |

**Note on S2**: `commands/shared.rs` exists at line 20 with `sanitize_display_path()` already defined there per the `shared.md` reference (`src-tauri/src/commands/shared.rs` line 20). Verify the function is present; if it's still in `launch.rs`, move it. Either way this is a ≤5-line change.

---

### Phase A — Core Health Check (MVP)

**Scope**: Pure filesystem validation. Zero MetadataStore code. Zero new migrations.

#### Track 1: Rust Core (sequential within track, parallel to Track 2)

| Task                                               | Files                                                                                             | Blocks     | Notes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1 — Promote path helpers to `pub(crate)`          | `crates/crosshook-core/src/launch/request.rs`                                                     | A2         | Change `fn require_directory`, `fn require_executable_file`, `fn is_executable_file` visibility. Three one-line changes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| A2 — Create `profile/health.rs` types + core logic | `crates/crosshook-core/src/profile/health.rs` (new)                                               | A3, A4, A5 | `HealthStatus`, `HealthIssueSeverity`, `HealthIssue`, `ProfileHealthReport`, `HealthCheckSummary`, `check_profile_health()`, `batch_check_health()`. Method-aware validation using `resolve_launch_method()`. **CRITICAL**: Validate `GameProfile` fields directly using the promoted path helpers — do NOT call `validate_all()` or construct a `LaunchRequest`. `validate_all()` checks `steam_client_install_path` (line 578 of `request.rs`), which is derived at runtime from `AppSettings` and is never stored in the profile; routing through it would produce a false Broken result for every `steam_applaunch` profile. Single largest new file. |
| A3 — Unit tests in `health.rs`                     | `crates/crosshook-core/src/profile/health.rs`                                                     | —          | Inline `#[cfg(test)]` module. Use `tempfile::tempdir()` + `ProfileStore::with_base_path()`. Test: healthy profile, missing path (Stale), wrong file type (Broken), EACCES (Broken), empty profile (Unconfigured/Broken), TOML parse error caught per-profile.                                                                                                                                                                                                                                                                                                                                                                                             |
| A4 — Wire profile module                           | `crates/crosshook-core/src/profile/mod.rs`                                                        | A5         | Add `pub mod health;` + `pub use health::{...}` re-export block. Three-line change.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| A5 — Tauri health commands                         | `src-tauri/src/commands/health.rs` (new), `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` | A7, A9     | New file: `batch_validate_profiles` and `get_profile_health` commands accepting `State<'_, ProfileStore>`. Apply `sanitize_display_path()` to all path fields. Register both commands in `invoke_handler!`. Depends on A2+A4 (types) and S2 (sanitize import).                                                                                                                                                                                                                                                                                                                                                                                            |

#### Track 2: TypeScript Foundation (parallel to Track 1)

| Task                         | Files                                             | Blocks | Notes                                                                                                                                                                                                                                                                                                |
| ---------------------------- | ------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A6 — TypeScript types        | `src/types/health.ts` (new), `src/types/index.ts` | A7, A8 | Pure type definitions from feature-spec data models. `HealthStatus`, `HealthIssueSeverity`, `HealthIssue`, `ProfileHealthReport`, `HealthCheckSummary`. Also include `ProfileHealthMetadata` and `EnrichedProfileHealthReport` stubs (Phase B will flesh them out). Add barrel export to `index.ts`. |
| A8 — `HealthBadge` component | `src/components/HealthBadge.tsx` (new)            | A9     | Presentational only. CSS pattern: `crosshook-status-chip crosshook-compatibility-badge--{rating}`. Map: `healthy → working`, `stale → partial`, `broken → broken`. Needs `HealthStatus` type only. No hook dependency.                                                                               |

#### Integration (converges both tracks)

| Task                          | Files                                   | Blocks           | Notes                                                                                                                                                                                                                                                                                                                                      |
| ----------------------------- | --------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| A7 — `useProfileHealth` hook  | `src/hooks/useProfileHealth.ts` (new)   | A9               | Depends on A5 (commands registered) and A6 (TS types). Mirror `useLaunchState.ts` `useReducer` pattern: `idle → loading → loaded \| error` states. Expose: `batchValidate()`, `revalidateSingle(name)`, `healthByName: Record<string, EnrichedProfileHealthReport>`, `summary`, `loading`, `error`.                                        |
| A9 — ProfilesPage integration | `src/components/pages/ProfilesPage.tsx` | Phase B, Phase C | Depends on A7 + A8. Add `HealthBadge` adjacent to profile names in sidebar list. Do NOT modify `ProfileFormSections.tsx`. Wire `save_profile` success → `revalidateSingle(name)` at `ProfilesPage.tsx` level. Add "Re-check All" button invoking `batchValidate()`. Add per-issue `CollapsibleSection` detail panel with remediation text. |

**Phase A change count**: 1 new Rust file, 1 new Tauri commands file, 1 new TypeScript types file, 1 new hook, 1 new badge component, 6 modified files.

---

### Phase B — Metadata Enrichment (after Phase A complete; internal tasks parallelizable)

**Scope**: Leverage existing `MetadataStore` queries. Zero new SQL, zero new tables, zero new migrations.

#### B-Rust: Enrich Tauri commands (sequential within sub-track)

| Task                                                                       | Files                              | Blocks | Notes                                                                                                                                                                                                                                                               |
| -------------------------------------------------------------------------- | ---------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1 — Add `ProfileHealthMetadata` type + enrichment to `commands/health.rs` | `src-tauri/src/commands/health.rs` | B2, B3 | Add `ProfileHealthMetadata` and `EnrichedProfileHealthReport` structs (already defined in feature-spec). Update `batch_validate_profiles` and `get_profile_health` to accept `State<'_, MetadataStore>`. Call `metadata_store.lookup_profile_id(name)` for UUID.    |
| B2 — Wire failure trend data                                               | `src-tauri/src/commands/health.rs` | B4     | Call `metadata_store.query_failure_trends(30)` once before per-profile loop; index by `profile_id`. Populate `failure_count_30d` and `total_launches` fields. Fail-soft: `unwrap_or_default()`. Join on `profile_id` via `lookup_profile_id()`, not `profile_name`. |
| B3 — Wire last-success timestamp                                           | `src-tauri/src/commands/health.rs` | B4     | Call `metadata_store.query_last_success_per_profile()` once; populate `last_success` field per profile.                                                                                                                                                             |
| B4 — Wire launcher drift detection                                         | `src-tauri/src/commands/health.rs` | B5     | Query `SELECT drift_state FROM launchers WHERE profile_id = ?1 AND deleted_at IS NULL LIMIT 1` per profile using `MetadataStore::with_conn()`. Populate `launcher_drift_state`. Absent launcher → `None` (not a health issue).                                      |
| B5 — Wire community import flag                                            | `src-tauri/src/commands/health.rs` | —      | Query `profiles.source` to set `is_community_import`. Uses existing `SyncSource` enum.                                                                                                                                                                              |

#### B-Frontend: Enriched display (parallel to B-Rust after A9)

| Task                                    | Files                                                                              | Notes                                                                                                                                                           |
| --------------------------------------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B6 — Failure trend badge overlay        | `src/components/HealthBadge.tsx`                                                   | Add `↑Nx` overlay when `failure_count_30d >= 2`. Show only when `metadata` is non-null.                                                                         |
| B7 — Last-success label in detail panel | `src/components/ProfileHealthDashboard.tsx` (new) or `ProfilesPage.tsx`            | Relative timestamp display: "Last worked: N days ago". Omit entirely when `metadata` is null.                                                                   |
| B8 — Launcher drift indicator           | `src/components/HealthBadge.tsx`                                                   | Add `✦` overlay when `launcher_drift_state` is `missing`, `moved`, or `stale`. Separate visual dimension from primary badge.                                    |
| B9 — Collection/favorites filter        | `src/components/pages/ProfilesPage.tsx`                                            | Filter health view by collection or favorites. Zero new backend calls — filter `healthByName` map client-side using existing collection data.                   |
| B10 — Unconfigured profile detection    | `src-tauri/src/commands/health.rs` + `crates/crosshook-core/src/profile/health.rs` | Detect all-empty `game.executable_path` as "Unconfigured" sub-state; badge-only, no banner. Can be implemented in Phase A as well — move earlier if convenient. |
| B11 — Community import context note     | `src/components/pages/ProfilesPage.tsx`                                            | Show "Imported profile — use Auto-Populate" message when `is_community_import` is true and status is broken/stale.                                              |

**B-Rust tasks B1–B5 are sequential** (same file, building up the enrichment pipeline). **B-Frontend tasks B6–B11 are all parallel with each other** after Phase A.

---

### Phase C — Startup Integration (after Phase A complete; parallel with Phase B)

**Scope**: Always-on background validation at startup with event push.

| Task                                  | Files                                   | Blocks | Notes                                                                                                                                                                                                                                                                                                   |
| ------------------------------------- | --------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1 — Background startup health scan   | `src-tauri/src/lib.rs`                  | C2     | Spawn async task in `setup` closure: `sleep(500ms)` → `batch_validate_profiles_internal()` → `app_handle.emit("profile-health-batch-complete", summary)`. Follow existing `auto-load-profile` emit pattern (lines 59–72 of `lib.rs`). Do NOT add to `startup.rs` — keep synchronous startup path clean. |
| C2 — Listen for startup event in hook | `src/hooks/useProfileHealth.ts`         | C3     | Add `listen("profile-health-batch-complete")` inside `useProfileHealth`. On receipt, merge payload into `healthByName`. Follow `useLaunchState.ts` `active` flag + `unlisten()` cleanup pattern (lines 157–186).                                                                                        |
| C3 — Startup summary banner           | `src/components/pages/ProfilesPage.tsx` | —      | Show dismissible non-modal banner if `broken_count > 0` after startup event. Reuse `crosshook-rename-toast` pattern with `role="status"` + `aria-live="polite"`. Per-session dismiss only — reappears on next launch if issues persist. Stale/Degraded profiles: badge only, no banner.                 |

C1 and C2 can be developed in parallel (C2 listens for the event C1 emits; neither blocks the other during development).

---

### Phase D — Persistence + Trends (after Phase B complete)

**Scope**: `health_snapshots` table (migration v6), instant startup badge rendering from cache, trend arrows.

| Task                                             | Files                                                                           | Blocks | Notes                                                                                                                                                                                                                                                      |
| ------------------------------------------------ | ------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1 — Migration v6 for `health_snapshots`         | `crates/crosshook-core/src/metadata/migrations.rs`                              | D2     | Add `migrate_5_to_6()` with `CREATE TABLE IF NOT EXISTS health_snapshots (profile_id TEXT PK, status TEXT NOT NULL, issue_count INTEGER NOT NULL DEFAULT 0, checked_at TEXT NOT NULL)` + index. Follow sequential migration pattern (v0→v5 already exist). |
| D2 — `metadata/health_store.rs` module           | `crates/crosshook-core/src/metadata/health_store.rs` (new)                      | D3     | Implement `upsert_health_snapshot()`, `load_health_snapshot(profile_id)`, `load_all_health_snapshots()`. Use `INSERT OR REPLACE` for UPSERT. Apply `sanitize_display_path()` to any stored path strings (security N-3).                                    |
| D3 — Wire `MetadataStore` public API             | `crates/crosshook-core/src/metadata/mod.rs`                                     | D4     | Add `mod health_store;` + public delegation methods: `upsert_health_snapshot()`, `load_all_health_snapshots()`. Wrap via `with_conn()` for fail-soft.                                                                                                      |
| D4 — Persist results after each batch validation | `src-tauri/src/commands/health.rs`                                              | D5     | After `batch_check_health()` completes, call `metadata_store.upsert_health_snapshot()` for each profile result. Filter `deleted_at IS NULL` — use `ProfileStore::list()` as the authoritative source (security N-4).                                       |
| D5 — Load cached snapshots at startup            | `src-tauri/src/commands/health.rs` or new `get_cached_health_snapshots` command | D6     | New Tauri command `get_cached_health_snapshots` → `load_all_health_snapshots()`. Frontend calls this on mount to show last-known badges instantly before live scan completes. Eliminates loading spinner for returning users.                              |
| D6 — Trend arrows in UI                          | `src/components/HealthBadge.tsx` or `ProfileHealthDashboard.tsx`                | —      | Compare current status to cached snapshot: `got_worse` (healthy→stale, healthy→broken, stale→broken), `got_better` (broken→stale, broken→healthy, stale→healthy), `unchanged`. Show trend arrow overlay on badge.                                          |
| D7 — Stale-snapshot detection                    | `src/hooks/useProfileHealth.ts`                                                 | —      | If cached snapshot `checked_at` is >7 days old, prompt re-check. Client-side: compare `checked_at` ISO string to `Date.now()`.                                                                                                                             |

**D1–D5 are sequential.** D6 and D7 are parallel after D5.

---

## Task Granularity Recommendations

### Keep Tasks Small and File-Focused

| Recommendation                                         | Rationale                                                                    |
| ------------------------------------------------------ | ---------------------------------------------------------------------------- |
| A1 (visibility promotion) is its own task              | Reviewed independently; unblocks A2; clean atomic commit                     |
| A3 (tests) tracked separately from A2 (implementation) | Tests can be written in parallel or reviewed separately; inline in same file |
| A4 (module wiring) separate from A2 (logic)            | One is a 3-line change; the other is 100+ lines — different risk profiles    |
| S2 pre-ship task not bundled with A5                   | Refactoring before use is cleaner and independently verifiable               |
| B-Rust tasks B1–B5 are sequential (same file)          | Enrichment pipeline builds incrementally; each task has a testable end-state |
| Phase D tasks D1–D5 are sequential                     | Each depends on prior migration/module state; cannot parallelize safely      |

### Tasks That Can Be Combined

| Combination                  | Reason                                                                             |
| ---------------------------- | ---------------------------------------------------------------------------------- |
| A2 + A3 combined into one PR | Tests live inline in `health.rs` as `#[cfg(test)]` — same file, same review        |
| A5 both commands in one task | Two commands in the same new file, registered together; splitting adds overhead    |
| A6 types + `index.ts` export | Trivially small; barrel update ships with the types file                           |
| B6 + B8 in `HealthBadge.tsx` | Both add overlays to the same component; same visual review context                |
| B10 can merge into Phase A   | "Unconfigured" detection is pure-filesystem — belongs in `health.rs` Phase A logic |

---

## Dependency Analysis (DAG)

```
[S1] Enable CSP ─────────────────────────────────────────────── (no dependents; unblocks ship)
[S2] Move sanitize_display_path ─────────────────────────────►  A5

[A1] Promote pub(crate) helpers ─────────────────────────────►  A2
[A2] Create profile/health.rs ──────────────────────────────┬── A3
                                                             ├── A4
                                                             └── A5 (via A4)
[A3] Unit tests ─────────────────────────────────────────────── (terminal — no dependents)
[A4] Wire profile/mod.rs ────────────────────────────────────►  A5
[A5] Tauri commands (health.rs) ─────────────────────────────►  A7

[A6] TypeScript types ───────────────────────────────────────►  A7, A8
[A8] HealthBadge component ──────────────────────────────────►  A9
[A7] useProfileHealth hook ──────────────────────────────────►  A9
[A9] ProfilesPage integration ───────────────────────────────►  Phase B, Phase C

[B1] ProfileHealthMetadata type + dual-store command ────────►  B2, B3, B4, B5
[B2] Failure trend data ─────────────────────────────────────►  B4 (test with data)
[B3] Last-success timestamp ─────────────────────────────────►  B4 (test with data)
[B4] Launcher drift detection ────────────────────────────────► B5
[B5] Community import flag ─────────────────────────────────── (terminal B-Rust)

[B6] Trend badge overlay ───────────────────────────────────── (parallel with B7–B11)
[B7] Last-success label ────────────────────────────────────── (parallel)
[B8] Drift indicator ───────────────────────────────────────── (parallel)
[B9] Collection/favorites filter ───────────────────────────── (parallel)
[B10] Unconfigured detection ──────────────────────────────────(can move to Phase A)
[B11] Community import note ────────────────────────────────── (parallel)

[C1] Startup scan spawn ─────────────────────────────────────►  C2
[C2] Listen in hook ─────────────────────────────────────────►  C3
[C3] Startup banner ────────────────────────────────────────── (terminal C)

[D1] Migration v6 ───────────────────────────────────────────►  D2
[D2] health_store.rs ────────────────────────────────────────►  D3
[D3] MetadataStore API ──────────────────────────────────────►  D4
[D4] Persist results ────────────────────────────────────────►  D5
[D5] Cached startup snapshots ───────────────────────────────►  D6, D7
[D6] Trend arrows ──────────────────────────────────────────── (terminal D)
[D7] Stale-snapshot prompt ─────────────────────────────────── (terminal D)
```

---

## Critical Paths

### Phase A Critical Path

```
S2 → A5 → A7 → A9
```

4 sequential tasks. All other Phase A tasks run in parallel around this chain.

### Full Feature Critical Path

```
S2 → A5 → A7 → A9 → B1 → B2/B3 → B4 → B5 → D1 → D2 → D3 → D4 → D5 → D6
```

Phase C (C1 → C2 → C3) is parallel to Phase B/D and not on the critical path for the full feature.

---

## Maximum Parallelism Points

| Point                   | Parallel work                                              |
| ----------------------- | ---------------------------------------------------------- |
| S1 ∥ S2                 | Both pre-ship tasks are independent                        |
| A1 ∥ A6                 | First Rust task and first TypeScript task are independent  |
| A2 ∥ A6 ∥ A8            | After A1 completes, A2 runs while TypeScript A6+A8 proceed |
| A3 ∥ A4                 | Tests and module wiring can proceed in parallel after A2   |
| Phase B ∥ Phase C       | Both are parallel after A9                                 |
| B6 ∥ B7 ∥ B8 ∥ B9 ∥ B11 | All B-Frontend tasks are independent                       |
| D6 ∥ D7                 | Both terminal D tasks are independent                      |

---

## File-to-Task Mapping

### New Files

| File                                                 | Task           | Phase |
| ---------------------------------------------------- | -------------- | ----- |
| `crates/crosshook-core/src/profile/health.rs`        | A2 + A3        | A     |
| `src-tauri/src/commands/health.rs`                   | A5, B1–B5, D4  | A/B/D |
| `src/types/health.ts`                                | A6             | A     |
| `src/hooks/useProfileHealth.ts`                      | A7, C2         | A/C   |
| `src/components/HealthBadge.tsx`                     | A8, B6, B8, D6 | A/B/D |
| `src/components/ProfileHealthDashboard.tsx`          | B7, B9         | B     |
| `crates/crosshook-core/src/metadata/health_store.rs` | D2             | D     |

### Modified Files

| File                                               | Task(s)         | Risk                                          |
| -------------------------------------------------- | --------------- | --------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs`      | A1              | Low — 3 visibility changes                    |
| `crates/crosshook-core/src/profile/mod.rs`         | A4              | Low — add `pub mod health;` + re-export       |
| `src-tauri/src/commands/mod.rs`                    | A5              | Low — add `pub mod health;`                   |
| `src-tauri/src/commands/shared.rs`                 | S2              | Low — verify/move function                    |
| `src-tauri/src/commands/launch.rs`                 | S2              | Low — update import if S2 moves the function  |
| `src-tauri/src/lib.rs`                             | A5, C1          | Low — append to macro list + spawn async task |
| `src/types/index.ts`                               | A6              | Low — one-line barrel add                     |
| `src/components/pages/ProfilesPage.tsx`            | A9, B9, B11, C3 | Medium — large existing component             |
| `crates/crosshook-core/src/metadata/mod.rs`        | D3              | Low — add mod + delegation methods            |
| `crates/crosshook-core/src/metadata/migrations.rs` | D1              | Low — append migration function               |

**Note**: `commands/health.rs` is a **new file**, not an addition to `commands/profile.rs`. This matches the feature-spec ("Files to Create" table) and keeps command surface area bounded.

---

## Phase A Parallelization Detail

Per the feature-spec note: "Tasks 1–5 (Rust) can run in parallel with tasks 7–9 (TypeScript types + hook + component). Tasks 6 and 10–12 depend on both."

Mapping spec numbering to this analysis:

| Spec #                       | This analysis  | Can parallelize with                           |
| ---------------------------- | -------------- | ---------------------------------------------- |
| 1 (pub(crate) helpers)       | A1             | A6, A8                                         |
| 2 (health.rs types)          | A2             | A6, A8                                         |
| 3 (check_profile_health)     | A2 (continued) | A6, A8                                         |
| 4 (batch_check_health)       | A2 (continued) | A6, A8                                         |
| 5 (unit tests)               | A3             | A6, A8                                         |
| 6 (Tauri commands)           | A5             | Requires A2+A4+S2 complete; TypeScript A6 done |
| 7 (TypeScript types)         | A6             | A1, A2, A3, A4                                 |
| 8 (useProfileHealth hook)    | A7             | Requires A5+A6 complete                        |
| 9 (HealthBadge component)    | A8             | A1, A2, A3 (only needs A6)                     |
| 10 (ProfilesPage badges)     | A9             | Requires A7+A8                                 |
| 11 (per-issue detail)        | A9 (continued) | Same convergence point                         |
| 12 (auto-revalidate on save) | A9 (continued) | Same convergence point                         |

---

## Implementation Strategy Recommendations

### Start Sequence (Day 1)

1. **S1 + S2 in parallel** — minimal change, security-correct baseline
2. **A1** — 5-minute visibility change; unlocks all Rust work
3. **A6** — TypeScript types from feature-spec data models; unlocks A8 immediately

### Parallel Development Window (Days 1–3)

After S1/S2/A1/A6 complete:

- **Developer A**: A2 (core logic) + A3 (tests) + A4 (module wiring) — sequential
- **Developer B**: A8 (`HealthBadge`) — can start after A6; fully independent of Rust

### Convergence Point (Day 3–4)

After A2+A4+S2 complete:

- **A5**: New `commands/health.rs` file — write both commands, register in `commands/mod.rs` and `lib.rs`
- **A7**: Hook wraps A5's commands
- **A9**: Integration in `ProfilesPage.tsx`

### Phase B + C (Days 4–6, parallel)

- **Thread 1 (B-Rust)**: B1 → B2 → B3 → B4 → B5 sequentially in `commands/health.rs`
- **Thread 2 (B-Frontend)**: B6, B7, B8, B9, B11 all parallel in frontend files
- **Thread 3 (C)**: C1 (`lib.rs` spawn) → C2 (hook listener) → C3 (banner), parallel to Threads 1+2
- B4+B5 frontend counterparts (B8, B11) can batch with C3 — all touch `ProfilesPage.tsx`

### Phase D (Days 6–8, after Phase B)

- D1 → D2 → D3 → D4 → D5 sequentially (migration dependency chain)
- D6 ∥ D7 after D5

### Validation Gates Before Each Phase

| Gate           | Check                                                                              |
| -------------- | ---------------------------------------------------------------------------------- |
| Before A5      | `cargo test -p crosshook-core` passes with new `health.rs` tests                   |
| Before A9      | Dev build: invoke `batch_validate_profiles` via frontend devtools manually         |
| After A9       | Smoke test: open app, verify health badges render on profile list                  |
| Before Phase B | Verify `MetadataStore::available` false → enrichment returns `None` gracefully     |
| After Phase B  | Smoke test: failure trend badge appears for a profile with recorded failures       |
| After Phase C  | Smoke test: startup banner appears for profile with missing executable path        |
| Before Phase D | `cargo test -p crosshook-core` passes including migration v6 roundtrip test        |
| After Phase D  | Smoke test: health badges show instantly on app reopen (cached from prior session) |

---

## Risk Flags for Implementors

| Risk                                                                     | Mitigation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **[CRITICAL] Do NOT call `validate_all()` in `health.rs`**               | `validate_all()` (and `collect_steam_issues()`) checks `steam_client_install_path` which is derived at runtime from `AppSettings`, never stored in the profile TOML. Calling it from health checks would flag every `steam_applaunch` profile as Broken. **Validate `GameProfile` fields directly** using the `pub(crate)` path helpers promoted in A1. The distinction is: `check_profile_health()` in `health.rs` reads `profile.steam.proton_path`, `profile.steam.compatdata_path`, etc. via `std::fs::metadata()` directly — it does not construct a `LaunchRequest`. |
| `commands/health.rs` vs `commands/profile.rs` placement                  | Create **new** `src-tauri/src/commands/health.rs` per spec. Do NOT add health commands to `profile.rs`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `ProfilesPage.tsx` already large (600+ lines)                            | Inject health as narrow `healthStatus: Record<string, EnrichedProfileHealthReport>` prop or via `useProfileHealth` hook at page level; never thread through `ProfileFormSections.tsx`                                                                                                                                                                                                                                                                                                                                                                                      |
| `ProfileFormSections.tsx` renders the profile selector list              | Do NOT modify this component for health badges — render `HealthBadge` in `ProfilesPage.tsx` sidebar adjacent to profile names                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Startup health check timing                                              | Use existing pattern: `tauri::async_runtime::spawn` + `sleep(500ms)` + `app_handle.emit(...)` in `lib.rs`; never add to `startup.rs`                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `injection.dll_paths` is a `Vec<String>`                                 | Must iterate all entries; do not check only the first element                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| IPC startup race (Phase C)                                               | Frontend should call `invoke('batch_validate_profiles')` on mount (Phase A) rather than waiting for the push event; Phase C event is additive for startup UX, not the sole data source                                                                                                                                                                                                                                                                                                                                                                                     |
| MetadataStore enrichment queries join on `profile_id` not `profile_name` | Use `lookup_profile_id(name)` to get UUID before all metadata queries; renames preserve UUID but break name-based joins                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `deleted_at IS NULL` filter for collection-scoped health                 | Use `ProfileStore::list()` (TOML-authoritative) as the profile source; never build health list from SQLite `profiles` table directly (security N-4)                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Phase D migration coupling                                               | Phase A/B have zero migration dependency. Migration v6 (`health_snapshots`) is Phase D only — do not pull it forward into Phase A or B                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `sanitize_display_path()` must apply at struct-assembly time (Phase D)   | Apply sanitization before both IPC serialization AND SQLite persistence — not only at IPC boundary (security N-3)                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

---

## Optimization Opportunities

### 1. `ProfileHealthDashboard.tsx` vs inline in `ProfilesPage.tsx`

The feature-spec lists `ProfileHealthDashboard.tsx` as a new component. Given that `ProfilesPage.tsx` is already the profile list page, the health dashboard content can be integrated inline rather than as a separate routed page. Recommend placing `ProfileHealthDashboard` as a sub-section within `ProfilesPage.tsx` using `CollapsibleSection`, avoiding route changes and ContentArea modifications.

### 2. Batch MetadataStore Queries Before Per-Profile Loop (Phase B)

`query_failure_trends(30)` and `query_last_success_per_profile()` return data for **all** profiles in a single SQL query. Call them once before the per-profile enrichment loop and index results by `profile_id`. This is O(1) queries for metadata regardless of profile count, versus O(n) if called per-profile.

### 3. Phase D Cached Startup Display Eliminates Spinner

If Phase D ships before or alongside Phase C, calling `get_cached_health_snapshots` on mount (before `batch_validate_profiles` completes) shows last-known badges instantly for returning users. This turns the 400ms–2s filesystem scan from a blocking UX cost into an eventual-consistency background refresh. Consider making `get_cached_health_snapshots` a free call in `useProfileHealth` initialization.

### 4. `EnrichedProfileHealthReport` Stubs in Phase A TypeScript Types

Define `ProfileHealthMetadata` and `EnrichedProfileHealthReport` interfaces in `health.ts` during Phase A (as stubs with all fields optional or nullable) so Phase B frontend tasks don't require a types file update — only filling in the implementation. This keeps Phase B frontend tasks self-contained.

### 5. B10 "Unconfigured" Detection Belongs in Phase A

The `check_profile_health()` function in `health.rs` can detect an all-empty profile (no `game.executable_path`) as "Unconfigured" during Phase A with near-zero extra effort. Moving B10 into A2 simplifies Phase B scope and ensures the badge-only (no banner) behavior is correct from MVP.
