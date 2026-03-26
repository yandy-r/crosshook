# Task Structure Analysis: Duplicate Profile (#56)

## Executive Summary

Profile duplication is a well-scoped, low-risk feature that modifies 8 existing files across 3 architectural layers (Rust core, Tauri IPC, React frontend) with zero new files or dependencies. The feature follows established patterns at every layer, making it highly predictable. The optimal task breakdown uses 4 phases with 5 implementation tasks, where Phase 2 offers the only meaningful parallelization opportunity (Rust command vs. TypeScript/hook changes operate on non-overlapping files across the IPC boundary). Total estimated scope is ~120-150 lines of new code plus ~80-100 lines of tests.

## Recommended Phase Structure

### Phase 1: Backend Core Logic (Sequential, Foundation)

**Why first**: Every subsequent phase depends on `ProfileStore::duplicate()` existing and being testable. This phase is the foundation and the only phase with meaningful business logic.

**Files modified**: 2

- `crates/crosshook-core/src/profile/toml_store.rs` (primary)
- `crates/crosshook-core/src/profile/mod.rs` (1-line re-export)

**Scope**: ~60-70 lines of implementation + ~80-100 lines of tests

**Deliverables**:

1. `DuplicateProfileResult` struct (Debug, Clone, Serialize, Deserialize) near line 22
2. `strip_copy_suffix()` free function (after `validate_name()` at line 214)
3. `generate_unique_copy_name()` private method on `ProfileStore`
4. `duplicate()` public method on `ProfileStore`
5. Re-export `DuplicateProfileResult` in `mod.rs` line 23
6. Unit tests: basic duplicate, conflict increment to (Copy 2)/(Copy 3), suffix stripping, duplicate-of-duplicate, source not found error, explicit name parameter

**Verification gate**: `cargo test -p crosshook-core` passes with all new + existing tests

### Phase 2: IPC Wiring (Parallelizable, 2 Tracks)

**Why second**: Connects backend to frontend. Two independent tracks modify non-overlapping files.

**Dependencies**: Phase 1 complete (need `DuplicateProfileResult` and `ProfileStore::duplicate()`)

#### Track A: Rust Command Layer

**Files modified**: 2

- `src-tauri/src/commands/profile.rs` (~5 lines: new `profile_duplicate` command)
- `src-tauri/src/lib.rs` (1 line: add to `invoke_handler` at the profile block, lines 91-97)

**Scope**: ~8 lines total

#### Track B: TypeScript Types + Hook

**Files modified**: 2

- `src/types/profile.ts` (~4 lines: `DuplicateProfileResult` interface)
- `src/hooks/useProfile.ts` (~25 lines: `duplicateProfile` useCallback + extend `UseProfileResult` interface at line 20-44)

**Scope**: ~30 lines total

**Parallelization rationale**: Track A is pure Rust (commands + registration), Track B is pure TypeScript (types + hook). They share no files and no compile-time dependencies. The IPC boundary (`invoke('profile_duplicate', ...)`) is the only coupling, and both sides can be written independently against the agreed API contract in feature-spec.md.

**Verification gate**: Project compiles (`cargo build` + `npm run build`)

### Phase 3: UI Integration (Parallelizable, 2 Tracks)

**Why third**: User-facing changes that depend on the hook exposing `duplicateProfile`.

**Dependencies**: Phase 2 Track B complete (need `duplicateProfile` in `UseProfileResult`)

#### Track A: ProfileActions Component

**Files modified**: 1

- `src/components/ProfileActions.tsx` (~15 lines: add `canDuplicate`, `onDuplicate` props to interface, render Duplicate button between Save and Delete)

**Scope**: ~15 lines

#### Track B: ProfilesPage Wiring

**Files modified**: 1

- `src/components/pages/ProfilesPage.tsx` (~5 lines: destructure `duplicateProfile` from context, compute `canDuplicate`, pass props to `<ProfileActions>`)

**Scope**: ~5 lines

**Parallelization note**: These tracks are small enough that parallelizing provides minimal benefit. A single implementor can handle both in one pass. However, if a strict 1-3 files per task rule is enforced, they split cleanly.

**Verification gate**: `npm run build` succeeds; UI renders without console errors

### Phase 4: Verification (Sequential)

**Dependencies**: All previous phases complete

**Tasks**:

1. Run `cargo test -p crosshook-core` -- all tests pass
2. Manual test: duplicate a profile, verify TOML file created, copy loads in editor
3. Manual test: duplicate when `(Copy)` exists, verify `(Copy 2)` generated
4. Manual test: gamepad navigation reaches Duplicate button
5. Manual test: error states (no profile selected, disk error simulation)

## Task Granularity Recommendations

### Option A: Minimal Tasks (3 Implementation Tasks)

Best for: fast execution, minimal coordination overhead.

| Task                        | Files                                                                | Lines | Parallelizable  |
| --------------------------- | -------------------------------------------------------------------- | ----- | --------------- |
| T1: Backend core + tests    | `toml_store.rs`, `mod.rs`                                            | ~150  | No (foundation) |
| T2: IPC wiring (both sides) | `commands/profile.rs`, `lib.rs`, `types/profile.ts`, `useProfile.ts` | ~38   | After T1        |
| T3: UI integration          | `ProfileActions.tsx`, `ProfilesPage.tsx`                             | ~20   | After T2        |

**Pro**: Lowest overhead, each task is self-contained within a layer.
**Con**: No parallelism; T2 touches 4 files (borderline on 1-3 file guideline).

### Option B: Optimal Tasks (5 Implementation Tasks, Recommended)

Best for: maximizing parallelism while keeping tasks focused.

| Task                   | Files                                    | Lines      | Parallelizable With |
| ---------------------- | ---------------------------------------- | ---------- | ------------------- |
| T1: Backend core logic | `toml_store.rs`, `mod.rs`                | ~70 impl   | None (foundation)   |
| T2: Backend tests      | `toml_store.rs`                          | ~100 tests | None (needs T1)     |
| T3: Rust IPC command   | `commands/profile.rs`, `lib.rs`          | ~8         | T4 (after T1)       |
| T4: TS types + hook    | `types/profile.ts`, `useProfile.ts`      | ~30        | T3 (after T1)       |
| T5: UI integration     | `ProfileActions.tsx`, `ProfilesPage.tsx` | ~20        | None (after T3+T4)  |

**Pro**: T3 and T4 run in parallel; every task is 1-2 files; tests are isolated.
**Con**: T2 modifies the same file as T1 (sequential dependency within toml_store.rs).

### Option C: Maximum Granularity (7 Implementation Tasks)

Best for: strict 1-file-per-task policy.

| Task                                 | Files                                    | Lines | Parallelizable With |
| ------------------------------------ | ---------------------------------------- | ----- | ------------------- |
| T1: DuplicateProfileResult + helpers | `toml_store.rs`                          | ~40   | None                |
| T2: duplicate() method               | `toml_store.rs`                          | ~20   | None (after T1)     |
| T3: Backend tests                    | `toml_store.rs`                          | ~100  | None (after T2)     |
| T4: mod.rs re-export                 | `mod.rs`                                 | ~1    | T5, T6 (after T2)   |
| T5: Tauri command + registration     | `commands/profile.rs`, `lib.rs`          | ~8    | T4, T6 (after T2)   |
| T6: TS type + hook                   | `types/profile.ts`, `useProfile.ts`      | ~30   | T4, T5 (after T2)   |
| T7: UI button + page wiring          | `ProfileActions.tsx`, `ProfilesPage.tsx` | ~20   | None (after T5+T6)  |

**Pro**: Maximum isolation; easy to review.
**Con**: Too many tasks for the scope; T1-T3 all modify toml_store.rs so they serialize anyway; coordination cost exceeds parallelism benefit.

### Recommendation: Option B (5 Tasks)

Option B strikes the best balance. It respects the 1-3 file guideline, enables meaningful parallelism in Phase 2, and keeps each task small enough for a single focused implementor. The total feature is ~200 lines including tests -- splitting beyond 5 tasks creates more coordination overhead than implementation effort.

## Dependency Analysis

```
T1: Backend core logic
 |
 +---> T2: Backend tests (sequential, same file)
 |
 +---> T3: Rust IPC command ----+
 |                               |
 +---> T4: TS types + hook -----+--> T5: UI integration
```

### Critical Path

T1 -> T2 -> T5 (if tests block UI work)
T1 -> T3 + T4 (parallel) -> T5

**Shortest path**: T1 -> (T3 || T4) -> T5, with T2 running alongside T3/T4.

### Blocking Dependencies

| Dependency           | Why                                                                       |
| -------------------- | ------------------------------------------------------------------------- |
| T1 blocks everything | All tasks consume `DuplicateProfileResult` or `ProfileStore::duplicate()` |
| T3 blocks T5         | UI invoke calls need the registered Tauri command to compile              |
| T4 blocks T5         | ProfileActions needs `duplicateProfile` from UseProfileResult             |
| T2 blocks nothing    | Tests validate but don't produce artifacts consumed downstream            |

### Non-blocking Dependencies

| Relationship | Why Non-blocking                                        |
| ------------ | ------------------------------------------------------- |
| T3 and T4    | Non-overlapping file sets across IPC boundary           |
| T2 and T3/T4 | Tests don't produce types or APIs needed by other tasks |

## File-to-Task Mapping

| File                                              | Task   | Change Type                        | Lines Changed |
| ------------------------------------------------- | ------ | ---------------------------------- | ------------- |
| `crates/crosshook-core/src/profile/toml_store.rs` | T1, T2 | Add struct, 3 functions, 6+ tests  | ~170          |
| `crates/crosshook-core/src/profile/mod.rs`        | T1     | Add 1 re-export identifier         | ~1            |
| `src-tauri/src/commands/profile.rs`               | T3     | Add 1 command function             | ~7            |
| `src-tauri/src/lib.rs`                            | T3     | Add 1 line to invoke_handler       | ~1            |
| `src/types/profile.ts`                            | T4     | Add 1 interface                    | ~4            |
| `src/hooks/useProfile.ts`                         | T4     | Add useCallback + extend interface | ~25           |
| `src/components/ProfileActions.tsx`               | T5     | Add props + button                 | ~15           |
| `src/components/pages/ProfilesPage.tsx`           | T5     | Wire context to props              | ~5            |

**Files NOT modified** (confirmed no changes needed):

- `src/context/ProfileContext.tsx` -- auto-propagates via `...profileState` spread
- `src/types/index.ts` -- `profile.ts` is already re-exported via `export * from './profile'`
- `crates/crosshook-core/src/profile/models.rs` -- GameProfile already derives Clone
- `crates/crosshook-core/src/profile/exchange.rs` -- name generation is independent

## Optimization Opportunities

### 1. Merge T1 + T2 into a Single Backend Task

Since both modify `toml_store.rs` and tests are written alongside implementation in Rust (inline `#[cfg(test)]` convention), combining them avoids the artificial split. This reduces to 4 tasks total with the same parallelism.

### 2. Merge T3 + T4 if No Parallel Execution Available

If the plan executor doesn't support parallel task dispatch, combining the Rust command and TypeScript hook into one "IPC wiring" task (4 files) is pragmatic. The files don't conflict, but a single implementor can handle both in sequence faster than the coordination overhead of parallelizing.

### 3. Consider T5 as a Subtask of T4

The UI integration is only ~20 lines and directly consumes T4's output. If the implementor working on T4 can continue into T5, it eliminates the handoff delay. The combined task (types + hook + UI) is still only 4 files.

### 4. Tests as Parallel Validation

Backend tests (T2) can run in parallel with Phase 2 tasks since they only validate T1's output and don't produce artifacts for downstream consumption. Schedule T2 alongside T3/T4 for maximum throughput.

## Implementation Strategy Recommendations

### Shared Context Requirements

Every implementor task needs access to:

1. `docs/plans/duplicate-profile/shared.md` -- condensed architecture + patterns + file list
2. `docs/plans/duplicate-profile/feature-spec.md` -- authoritative design with code snippets

Backend tasks additionally need: 3. `docs/plans/duplicate-profile/research-technical.md` -- test cases and technical decisions

Frontend tasks additionally need: 4. `docs/plans/duplicate-profile/research-ux.md` -- button placement, terminology, loading states

### Critical Safety Constraint

All implementors modifying `toml_store.rs` must be aware: **`ProfileStore::save()` silently overwrites via `fs::write()`**. The `duplicate()` method MUST check `list()` before saving auto-generated names. This is the single most important correctness requirement and is documented in shared.md, feature-spec.md, and research-patterns.md.

### Commit Strategy

Each task should produce one conventional commit:

- T1+T2: `feat(profile): add ProfileStore::duplicate() with unique name generation`
- T3: `feat(profile): add profile_duplicate Tauri IPC command`
- T4: `feat(profile): add duplicateProfile hook and DuplicateProfileResult type`
- T5: `feat(ui): add Duplicate button to profile actions`

Or if merged (Option A, 3 tasks):

- `feat(profile): implement profile duplication in crosshook-core`
- `feat(profile): wire profile_duplicate across IPC boundary`
- `feat(ui): add Duplicate button to profile editor`

### Verification Checklist

- [ ] `cargo test -p crosshook-core` -- all tests pass (after T1/T2)
- [ ] `cargo build` -- Rust compiles (after T3)
- [ ] `npm run build` -- TypeScript compiles (after T4/T5)
- [ ] Manual: duplicate creates `"Name (Copy)"` file on disk
- [ ] Manual: duplicate of `"Name (Copy)"` creates `"Name (Copy 2)"`
- [ ] Manual: button disabled when no profile is selected or unsaved changes exist
- [ ] Manual: gamepad D-pad navigation reaches Duplicate button
