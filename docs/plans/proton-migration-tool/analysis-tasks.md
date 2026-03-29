# Task Analysis: Proton Migration Tool

## Executive Summary

The proton-migration-tool is a **7-task, 2-phase implementation** across ~900 lines of new code (Rust + TypeScript + tests) with zero new crate dependencies. All foundational infrastructure exists; the primary work is a ~150-line version suggestion engine, ~250 lines of Tauri IPC wiring, and ~350 lines of frontend UX. The critical path runs: prerequisites → suggestion engine → Tauri commands → frontend UX. TypeScript types and the React hook can be drafted in parallel with backend work, providing a natural parallelism point. Phase 2 is strictly sequential after Phase 1.

**One design decision affects task boundaries before planning begins**: the planning documents have an inconsistency on module placement (`steam/migration.rs` in `feature-spec.md` vs `profile/migration.rs` in `research-recommendations.md`). The `analysis-context.md` document resolves this as `profile/migration.rs` per team consensus. All task scopes below assume `profile/migration.rs`.

---

## Recommended Phase Structure

### Phase 1 — Single-Profile Migration (5 tasks)

Validates the algorithm and UX before batch complexity is added. Deliverable: a user can see stale-Proton issues in Health Dashboard and apply a one-click fix with undo toast.

```
Task 1.0 (prereqs)
    │
Task 1.1 (suggestion engine) ─── Task 1.3a (TS types + hook) ┐
    │                                                          ├─ Task 1.4 (dashboard UX)
Task 1.2 (Tauri IPC) ─────────────────────────────────────────┘
```

### Phase 2 — Batch Migration (2 tasks)

Scales the validated approach. Hard dependency on Phase 1 complete.

```
Task 2.1 (batch backend) ─── Task 2.2 (batch frontend UX)
```

---

## Task Granularity Recommendations

### Task 1.0 — Prerequisites (trivial, < 30 min)

**Scope**: Visibility changes and enum variant — must complete before 1.1 starts. Bundle all three changes into one task since they're tiny and touch only two files.

| Change                                                  | File                 | Lines |
| ------------------------------------------------------- | -------------------- | ----- |
| Promote `normalize_alias()` to `pub(crate)`             | `steam/proton.rs`    | ~1    |
| Promote `resolve_compat_tool_by_name()` to `pub(crate)` | `steam/proton.rs`    | ~1    |
| Add `AppMigration` variant to `SyncSource` enum         | `metadata/models.rs` | ~3    |

**Files**: 2 modified
**Estimated size**: ~5–10 lines total
**Blocks**: 1.1 (directly — migration module can't compile without these)

> **Note**: Do not add `pub mod migration;` to `profile/mod.rs` in this task — wait until 1.1 so the module file exists.

---

### Task 1.1 — Backend: Version Suggestion Engine (core algorithm)

**Scope**: Create `profile/migration.rs` with the suggestion engine in full isolation. No Tauri dependency — this is pure logic with unit tests.

**Files created/modified**:

- `crates/crosshook-core/src/profile/migration.rs` (NEW — primary deliverable)
- `crates/crosshook-core/src/profile/mod.rs` (MODIFIED — add `pub mod migration;`)

**What goes in `migration.rs`**:

- Data structs: `ProtonPathField`, `MigrationSuggestion`, `UnmatchedProfile`, `MigrationScanResult`, `MigrationApplyResult`, `BatchMigrationResult` (Rust side of types)
- `extract_proton_family(name: &str) -> Option<String>` — strips trailing digits from normalized alias
- `extract_version_segments(normalized: &str) -> Vec<u32>` — parses integer tuple from name
- `find_best_replacement(stale_path: &str, installed: &[ProtonInstall]) -> Option<MigrationSuggestion>` — same-family match with confidence scoring
- `scan_proton_migrations(profiles: &[GameProfile], installed: &[ProtonInstall]) -> MigrationScanResult` — iterate all profiles, collect suggestions and unmatched
- Unit tests covering:
  - Family extraction (GE-Proton, official Proton, Experimental)
  - Integer-tuple ordering (`"9-10"` > `"9-9"` — the critical edge case)
  - Same-family newer / same-family older / cross-family exclusion
  - TKG / hash-versioned builds excluded from numeric ranking
  - "Proton Experimental" — versionless path
  - `crosses_major_version` flag set correctly
  - No-match returns `None` gracefully
- **Migration-specific round-trip test**: `load()` → modify `steam.proton_path` → `save()` → re-`load()` confirms new path in effective profile (validates `storage_profile()` invariant is preserved)

**Estimated size**: ~150 lines Rust code + ~120 lines tests
**Blocks**: 1.2

---

### Task 1.2 — Backend: Tauri IPC Commands (migration write path)

**Scope**: Create `commands/migration.rs` and wire into Tauri. This is the only task that performs actual profile writes.

**Files created/modified**:

- `src-tauri/src/commands/migration.rs` (NEW)
- `src-tauri/src/commands/mod.rs` (MODIFIED — add `pub mod migration;`)
- `src-tauri/src/lib.rs` (MODIFIED — register commands in `invoke_handler`)

**What goes in `commands/migration.rs`**:

- `check_proton_migrations(steam_client_install_path: Option<String>, store: State<'_, ProfileStore>) -> Result<MigrationScanResult, String>` — calls `discover_compat_tools()` then `scan_proton_migrations()`; read-only; applies `sanitize_display_path()` to all path strings before return
- `apply_proton_migration(request: ApplyMigrationRequest, store: State<'_, ProfileStore>, metadata_store: State<'_, MetadataStore>) -> Result<MigrationApplyResult, String>` — write path:
  - Re-validate replacement path with `try_exists()` immediately before write (TOCTOU mitigation)
  - `store.load()` → modify effective profile via `resolve_launch_method()` to target correct field
  - Atomic write: serialize to `.toml.tmp` then `fs::rename()` (security W-1 — NOT `store.save()`)
  - Call `observe_profile_write()` with `SyncSource::AppMigration`
  - Invalidate `health_snapshots` row for migrated `profile_id`
  - Apply `sanitize_display_path()` on returned paths
- Validate `steam_client_install_path` IPC argument with `candidate.join("steamapps").is_dir()` (A-5)
- Unit tests: field targeting correctness, TOCTOU rejection, path sanitization, `local_override` round-trip

**In `lib.rs`**, add to `invoke_handler`:

```rust
commands::migration::check_proton_migrations,
commands::migration::apply_proton_migration,
```

**Estimated size**: ~200 lines Rust + ~50 lines wiring + ~80 lines tests
**Blocks**: 1.4 (frontend needs invoke targets)

---

### Task 1.3 — Frontend: Types and Hook (can run in parallel with 1.2)

**Scope**: TypeScript types and the React hook. Can start immediately after 1.1 completes since the Rust structs define the contract; does NOT need 1.2 to be done.

**Files created/modified**:

- `src/types/migration.ts` (NEW)
- `src/hooks/useProtonMigration.ts` (NEW)
- `src/types/index.ts` (MODIFIED — re-export migration types)

**What goes in `migration.ts`**: Direct TypeScript translation of Rust structs from `feature-spec.md` §Data Models — `ProtonPathField`, `MigrationOutcome`, `MigrationSuggestion`, `UnmatchedProfile`, `MigrationScanResult`, `MigrationApplyResult`, `BatchMigrationResult`, `ApplyMigrationRequest`.

**What goes in `useProtonMigration.ts`**:

- `scanMigrations(steamClientInstallPath?: string)` — invokes `check_proton_migrations`; manages loading/error state
- `applySingleMigration(request: ApplyMigrationRequest)` — invokes `apply_proton_migration`; calls `revalidateSingle(profileName)` on success
- No optimistic updates — wait for filesystem confirmation before updating state
- Handle TOCTOU error response with re-scan CTA

**Estimated size**: ~80 lines types + ~100 lines hook
**Blocks**: 1.4

---

### Task 1.4 — Frontend: Health Dashboard UX (Phase 1 deliverable)

**Scope**: Integrate migration actions into the Health Dashboard. Depends on both 1.2 (invoke targets) and 1.3 (types/hook).

**Files modified**:

- `src/components/pages/HealthDashboardPage.tsx` (MODIFIED — only file)

**Changes**:

- In `categorizeIssue()` or issue-row rendering: detect `missing_proton` category; add inline "Update Proton" action button
- On click: call `scanMigrations()` from hook → display inline before/after with `old_proton_name → new_proton_name`
- Confidence display: simplified "Recommended" (same-family same-major) vs amber warning (cross-major); no numeric scores in Phase 1
- Cross-major suggestion: display explicit prefix incompatibility warning ("Major version change — WINE prefix may need recreation")
- "Update to [version name]" button (descriptive, not generic "Apply") → calls `applySingleMigration()`
- On success: dismissible undo toast (NOT modal — avoid dialog fatigue for single-profile fix), `revalidateSingle()` called by hook automatically
- "No match" state: "No Proton installations detected. [Browse...] / [Install Proton →]"
- All interactive elements: `crosshook-focus-ring`, `crosshook-nav-target` CSS classes; minimum `var(--crosshook-touch-target-min)` touch target
- All displayed paths: pass through `sanitize_display_path()` equivalent (home → `~`)
- No optimistic updates

**Estimated size**: ~150–180 lines TypeScript modifications
**Completes Phase 1**

---

### Task 2.1 — Backend: Batch Migration Commands

**Dependencies**: Phase 1 fully complete and validated.

**Files modified**:

- `src-tauri/src/commands/migration.rs` (MODIFIED — add batch command)
- `src-tauri/src/lib.rs` (MODIFIED — register batch command)

**What to add to `commands/migration.rs`**:

- `apply_batch_migration(request: BatchMigrationRequest, store: State<'_, ProfileStore>, metadata_store: State<'_, MetadataStore>) -> Result<BatchMigrationResult, String>`
- **Pre-flight validation pass** (security W-4 — non-negotiable): serialize all targets to TOML + verify each replacement path via `try_exists()` before any write; abort with `BatchMigrationResult { applied: 0, failed: all }` if any pre-flight fails
- Cross-family suggestions excluded from batch operations by default (never in `BatchMigrationRequest.targets` unless user explicitly opted in per row)
- Per-profile error isolation: one profile write failure does not abort remaining profiles
- Per-profile atomic writes (same `.toml.tmp` + `fs::rename()` pattern as single migration)
- `observe_profile_write(SyncSource::AppMigration)` for each successful write
- Unit tests: batch partial failure, pre-flight rejection, all-success path, cross-family exclusion enforcement

**Estimated size**: ~150 lines
**Blocks**: 2.2

---

### Task 2.2 — Frontend: Batch Migration UX

**Dependencies**: Task 2.1 complete.

**Files created/modified**:

- New modal component file (e.g., `src/components/MigrationReviewModal.tsx` — NEW)
- `src/hooks/useProtonMigration.ts` (MODIFIED — add batch state)
- `src/components/pages/HealthDashboardPage.tsx` (MODIFIED — toolbar button)

**Changes**:

- `HealthDashboardPage.tsx`: Add "Fix Proton Paths (N)" button to `TableToolbar` (file-local component — modify in place); visible only when `missing_proton` count ≥ 2; triggers full scan then opens review modal
- `MigrationReviewModal.tsx`: Use `LauncherPreviewModal` as shell (NOT `ProfileReviewModal` — wrong layout); body = before/after table with per-row checkboxes; sections: "Safe to update" (same-family same-major, pre-checked), "Review recommended" (cross-major, unchecked), "No suggestion" (informational only); "Update N Profiles" confirm button (descriptive); progress bar for ≥ 3 profiles; post-migration: "X updated, Y need manual attention" summary; surface launcher re-export notice for affected profiles via `launcher_drift_map`
- `useProtonMigration.ts`: Add `applyBatchMigration(requests)` → invokes `apply_batch_migration`; calls `batchValidate()` on completion
- All modal focus management: `useGamepadNav` hook; Tab order: Select All → row checkboxes → Cancel → Update N Profiles; Escape closes

**Estimated size**: ~220–250 lines TypeScript
**Completes Phase 2**

---

## Dependency Analysis

```
1.0 (prereqs: proton.rs + models.rs)
  └─ 1.1 (profile/migration.rs)
       ├─ 1.2 (commands/migration.rs)   ← critical path
       │    └─ 1.4 (HealthDashboard UX)  ← Phase 1 done
       │         └─ (Phase 1 validation)
       │              └─ 2.1 (batch commands)
       │                   └─ 2.2 (batch UX) ← Phase 2 done
       │
       └─ 1.3 (TS types + hook) ──────────────┘ (feeds 1.4, parallel with 1.2)
```

**Hard dependencies**:

- 1.0 must precede 1.1 (visibility changes enable compilation)
- 1.1 must precede 1.2 (Tauri commands call core functions)
- 1.2 AND 1.3 must precede 1.4 (UI uses invoke calls + types)
- Phase 1 complete and validated must precede 2.1 (algorithm correctness gates batch trust)

**Soft recommendation** (not a hard dependency):

- Add a short manual smoke-test gate between Phase 1 completion and 2.1 kick-off — batch migration writes N profiles in one operation; catching a family-matching bug at single-profile scale is much cheaper to debug

---

## File-to-Task Mapping

### Files Created

| File                                             | Task      | Purpose                                          |
| ------------------------------------------------ | --------- | ------------------------------------------------ |
| `crates/crosshook-core/src/profile/migration.rs` | 1.1       | Core suggestion engine, data structs, scan logic |
| `src-tauri/src/commands/migration.rs`            | 1.2       | IPC command handlers (single + batch)            |
| `src/types/migration.ts`                         | 1.3       | TypeScript type contracts                        |
| `src/hooks/useProtonMigration.ts`                | 1.3 / 2.2 | React state management hook                      |
| `src/components/MigrationReviewModal.tsx`        | 2.2       | Batch review modal                               |

### Files Modified

| File                                           | Task      | Change                                  |
| ---------------------------------------------- | --------- | --------------------------------------- |
| `crates/crosshook-core/src/steam/proton.rs`    | 1.0       | Promote two private fns to `pub(crate)` |
| `crates/crosshook-core/src/metadata/models.rs` | 1.0       | Add `AppMigration` to `SyncSource`      |
| `crates/crosshook-core/src/profile/mod.rs`     | 1.1       | `pub mod migration;`                    |
| `src-tauri/src/commands/mod.rs`                | 1.2       | `pub mod migration;`                    |
| `src-tauri/src/lib.rs`                         | 1.2 / 2.1 | Register migration commands             |
| `src/types/index.ts`                           | 1.3       | Re-export migration types               |
| `src/components/pages/HealthDashboardPage.tsx` | 1.4 / 2.2 | Inline fix (1.4), toolbar button (2.2)  |

> `HealthDashboardPage.tsx` is touched in two separate tasks. The Phase 2 change (toolbar button) is additive and non-conflicting with Phase 1 inline changes — no merge risk.

---

## Optimization Opportunities

### 1. Prerequisites as First-Mover Task

Task 1.0 is ~10 lines across 2 files and unblocks everything. Assign to whichever implementor is available first — it's a perfect warm-up task with clear scope and zero risk.

### 2. Frontend-Backend Parallelism in Phase 1

**Tasks 1.2 and 1.3 can run simultaneously** once 1.1 is merged. The TypeScript type contracts are fully specified in `feature-spec.md` §Data Models — a frontend implementor does not need to wait for Rust compilation. The hook (`useProtonMigration.ts`) can be written against the invoke contract and tested with mock data.

Recommended split for 2-person team during Phase 1:

- Person A: 1.0 → 1.1 → 1.2
- Person B: 1.3 (starts after 1.1 merges, parallel with 1.2) → 1.4

### 3. Discover-Once Optimization in 1.2

`discover_compat_tools()` scans the filesystem on every call. In `check_proton_migrations`, call it once and pass the result to `scan_proton_migrations()` — do not re-invoke per profile. This is especially important since the scan may cover 20+ Proton versions across multiple library folders.

### 4. `metadata/models.rs` vs `metadata/profile_sync.rs` Placement

The research documents reference `SyncSource` in `profile_sync.rs`, but `grep` shows it is actually defined in `metadata/models.rs:76`. Task 1.0 should target `models.rs`, not `profile_sync.rs`.

---

## Implementation Strategy Recommendations

### 1. Resolve Module Placement Before Starting

The planning documents have a documented inconsistency:

- `feature-spec.md` → `steam/migration.rs`
- `research-recommendations.md` → `profile/migration.rs`
- `analysis-context.md` → `profile/migration.rs` (team consensus)

`profile/migration.rs` is the correct choice — migration logic consumes `steam/` functions but its write target is profiles. Placing it in `steam/` would create a circular dependency direction (steam module writing profiles). **Confirm this as the canonical decision before 1.1 starts.**

### 2. Algorithm-First, TDD Approach for Task 1.1

The version-ranking algorithm is the highest-risk logic in the feature. Write unit tests for `extract_proton_family()` and `extract_version_segments()` before the implementation. Key cases that must have explicit tests before merging:

- `"9-10"` > `"9-9"` (integer-tuple, not lexicographic — the documented high-likelihood mis-ordering risk)
- `"Proton Experimental"` → versionless, only matches another Experimental
- TKG builds → detected, excluded from numeric ranking
- `"GE-Proton9-7"` and `"GE-Proton10-2"` → same family, different major (→ `crosses_major_version: true`)

### 3. Security W-1 is Non-Negotiable in 1.2

Every migration write path must use:

```rust
let tmp = profile_path.with_extension("toml.tmp");
fs::write(&tmp, toml_content)?;
fs::rename(&tmp, &profile_path)?;
```

**Do NOT route through `ProfileStore::save()`** for migration writes — it uses `fs::write()` (truncate-then-write) which is not crash-safe. The temp+rename pattern is mandatory in `commands/migration.rs`.

### 4. Phase Boundary — Validate Before Batch

Do not start Task 2.1 from a plan alone. The batch migration writes N profiles in sequence; a family-matching bug that produces wrong suggestions at single-profile scale will produce N wrong suggestions at batch scale. The Phase 1 → Phase 2 transition should include real-world validation:

- At least one actual stale-Proton profile migrated via the inline fix
- Integer-tuple ordering confirmed correct on real GE-Proton naming from the user's filesystem

### 5. Command Naming Alignment

The feature-spec uses `check_proton_migrations` / `apply_proton_migration`; research-recommendations uses `preview_proton_migration` / `apply_proton_migration`. The "preview/apply" split more explicitly communicates the read-only vs write semantics. Recommend renaming `check_proton_migrations` → `preview_proton_migration` for clarity, but this is a minor decision — pick one and be consistent across Rust function names, frontend `invoke()` calls, and TypeScript hook method names.

### 6. Health Dashboard — No New Component File for Phase 1

The Phase 1 inline fix (Task 1.4) touches only `HealthDashboardPage.tsx`. Resist the urge to extract a `MigrationActionRow.tsx` helper for Phase 1 — the inline change is ~50–80 lines and the `TableToolbar` is already file-local. Extract to a component only in Phase 2 when the modal and batch toolbar are added, at which point the abstraction boundary is clearer.

---

## Summary Table

| Task                  | Phase | Files                 | Estimated Lines     | Parallelizable With        |
| --------------------- | ----- | --------------------- | ------------------- | -------------------------- |
| 1.0 Prerequisites     | 1     | 2 modified            | ~10                 | Nothing (blocks all)       |
| 1.1 Suggestion engine | 1     | 1 new + 1 modified    | ~270 (code + tests) | Nothing (blocks 1.2 + 1.3) |
| 1.2 Tauri IPC         | 1     | 1 new + 2 modified    | ~330 (code + tests) | Task 1.3                   |
| 1.3 TS types + hook   | 1     | 2 new + 1 modified    | ~180                | Task 1.2                   |
| 1.4 Dashboard UX (P1) | 1     | 1 modified            | ~160                | Nothing (caps Phase 1)     |
| 2.1 Batch backend     | 2     | 2 modified            | ~150 + tests        | Nothing                    |
| 2.2 Batch frontend    | 2     | 1 new + 2 modified    | ~250                | Nothing                    |
| **Total**             |       | **4 new, 8 modified** | **~1,350**          |                            |

> Line counts include tests. Production-only estimates: ~500 Rust, ~350 TypeScript.
