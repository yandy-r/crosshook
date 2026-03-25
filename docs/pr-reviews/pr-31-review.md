# PR #31 Review: feat(launch): add proton-run launch optimizations

**Branch**: `feat/proton-optimizations` -> `main`
**Reviewed**: 2026-03-25
**Critical / important follow-up**: 2026-03-25 (validated against tree, fixes applied)
**Scope**: 4806 additions, 121 deletions across 45 files
**Agents**: code-reviewer, silent-failure-hunter, type-design-analyzer, pr-test-analyzer, comment-analyzer

---

## Critical Issues (1 found)

### 1. Missing bidirectional `conflictsWith` for `enable_fsr4_upgrade` in TypeScript catalog

**File**: [`launch-optimizations.ts`](../../src/crosshook-native/src/types/launch-optimizations.ts) (`enable_fsr4_upgrade` entry, now includes symmetric `conflictsWith`)
**Confidence**: 95%
**Status**: Resolved

The Rust backend defines FSR4 conflicts bidirectionally: `enable_fsr4_upgrade` conflicts with `enable_fsr4_rdna3_upgrade` (`optimizations.rs` ~124) and vice versa (~132). Originally, the TypeScript catalog had `conflictsWith` only on `enable_fsr4_rdna3_upgrade`, not on `enable_fsr4_upgrade`.

**User impact (before fix)**: If `enable_fsr4_rdna3_upgrade` was already enabled and the user enabled `enable_fsr4_upgrade`, the frontend conflict check could succeed (empty conflict array from the toggled option’s matrix row), both options appeared enabled, autosave could persist the pair, and the Rust backend rejected the combination at launch with `IncompatibleLaunchOptimizations`.

**Resolution**: Added `conflictsWith: ['enable_fsr4_rdna3_upgrade']` to the `enable_fsr4_upgrade` catalog entry so [`getConflictingLaunchOptimizationIds`](../../src/crosshook-native/src/types/launch-optimizations.ts) blocks the pair in both directions, matching Rust.

---

## Important Issues (5 found)

### 2. `derive_target_home_path` silently returned empty string when HOME is unset

**File**: [`profile.rs`](../../src/crosshook-native/src-tauri/src/commands/profile.rs) (`derive_target_home_path`, ~20–38)
**Severity**: High (silent failure)
**Status**: Resolved

When the Steam client install path did not match any known suffix, the function fell back to `std::env::var("HOME").unwrap_or_default()`. If `HOME` was unset or empty, this returned `""` without diagnostics.

**Resolution**: On missing or empty `HOME`, log `tracing::warn!(...)` explaining that the derived home for launcher cleanup is empty, then return `String::new()` as before.

### 3. `setDirty` after conflict-blocked toggle (new-profile case)

**File**: [`useProfile.ts`](../../src/crosshook-native/src/hooks/useProfile.ts) — `toggleLaunchOptimization` (~510–526), helper `applyLaunchOptimizationToggle` (~139–176), `profileRef` (~349–350)
**Severity**: Medium (state consistency)
**Status**: Resolved

Previously, `toggleLaunchOptimization` used a `setProfile` updater and then `setDirty((currentDirty) => currentDirty || !hasExistingSavedProfile)`. The updater was not “unconditional”: for **saved** profiles, a blocked conflict left `dirty` unchanged (`false || false`). For **new** drafts (`!hasExistingSavedProfile`), the expression became `currentDirty || true`, so `dirty` flipped to `true` even when `setProfile` returned the previous profile unchanged.

**Resolution**: Compute the next profile with `applyLaunchOptimizationToggle(profileRef.current, ...)`. On conflict, show status and **return** without `setProfile` or `setDirty`. On success, `setProfile(result.profile)` and then `setDirty` as before.

### 4. Copy-paste artifact: duplicate “Launch Optimizations” heading element

**File**: [`LaunchOptimizationsPanel.tsx`](../../src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx) (~323–327)
**Severity**: Medium (UI/accessibility)
**Status**: Resolved

The header had both a `crosshook-install-section-title` div and an `<h2>` with the same visible text.

**Resolution**: Removed the redundant div; kept the `<h2>` with `aria-labelledby`.

### 5. `save_launch_optimizations()` performed no ID validation before persisting

**File**: [`toml_store.rs`](../../src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs) (`save_launch_optimizations`, ~100–119); catalog helper [`is_known_launch_optimization_id`](../../src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs)
**Also**: [`profile.rs`](../../src/crosshook-native/src-tauri/src/commands/profile.rs) (`save_launch_optimizations_for_profile`) — unchanged; errors surface via `ProfileStoreError` string
**Severity**: Medium (data integrity)
**Status**: Resolved

**Resolution**: Before load/save, each id is checked with `is_known_launch_optimization_id`. Unknown ids return `ProfileStoreError::InvalidLaunchOptimizationId`. Unit test: `save_launch_optimizations_rejects_unknown_option_ids`.

### 6. Non-atomic load-modify-save in `save_launch_optimizations`

**File**: [`toml_store.rs`](../../src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs) (`save_launch_optimizations`)
**Severity**: Medium (theoretical TOCTOU)
**Status**: Resolved (documented)

**Resolution**: Added a rustdoc paragraph on `save_launch_optimizations` stating that concurrent `save` / `save_launch_optimizations` for the same profile are not synchronized and the last completed write wins. No file locking added (low practical risk).

---

## Suggestions (11 found)

### Test Coverage Gaps

| Priority | Gap                                                                               | File                               | Effort |
| -------- | --------------------------------------------------------------------------------- | ---------------------------------- | ------ |
| **9**    | No Rust/TS optimization ID catalog sync test                                      | `optimizations.rs`                 | Low    |
| **8**    | No `steam_applaunch` rejects optimizations test                                   | `request.rs`                       | Low    |
| **8**    | No FSR4 conflict pair test                                                        | `optimizations.rs` or `request.rs` | Low    |
| **8**    | No catalog-level invariant test (ID uniqueness, conflict symmetry, env allowlist) | `optimizations.rs`                 | Low    |
| **6**    | No test for clearing optimizations (empty IDs round-trip)                         | `toml_store.rs`                    | Low    |
| **6**    | No explicit empty-optimizations directive resolution test                         | `optimizations.rs`                 | Low    |

### Type Design Improvements

- **Eliminate `LaunchOptimizationsPayload`** in the Tauri command layer by importing `LaunchOptimizationsSection` from `crosshook-core`. Three structurally identical types (`Request`, `Section`, `Payload`) with the same field and serde config is excessive.
- **Unify TypeScript status types**: `LaunchOptimizationsPanelStatus` (component) and `LaunchOptimizationsStatus` (hook) are structurally identical. Define once in `launch-optimizations.ts`.

### Error Handling Improvements

- **Add trace logging in `is_executable_file`** (`optimizations.rs:286`) when `fs::metadata` fails, to aid debugging binary-not-found issues.
- **Add `console.warn` in `normalizeLaunchOptimizationIds`** (`useProfile.ts:123`) when unknown IDs are silently dropped, for forward-compatibility visibility.

### Documentation Improvements

- **Add doc comment on `LAUNCH_OPTIMIZATION_ENV_VARS`** (`env.rs:48`) explaining its purpose and why it doesn't need shell script sync (unlike `WINE_ENV_VARS_TO_CLEAR`).
- **Fix MangoHud description** (`launch-optimizations.ts:103`): "during launch" -> "while the game runs" (MangoHud persists through the session).
- **Fix shader cache description** (`launch-optimizations.ts:171`): "per profile" -> "per prefix" (isolation is at the Proton prefix level, not the CrossHook profile level).
- **Broaden `DEFAULT_HOST_PATH` doc comment** (`runtime_helpers.rs:8`): now used by both `apply_host_environment` and `is_command_available`, but comment only references the former.
- **Remove "v1" version language** (`steam-proton-trainer-launch.doc.md:71`): will age poorly; use "initial" or omit.

---

## Strengths

- **Architecture**: Clean separation between catalog (static data), resolver (validation + directive emission), and consumers (script runner, React panel). The three-layer design (TS types -> Rust validation -> command construction) is well-layered.
- **Rust optimization resolver**: Proper validation of duplicates, unknown IDs, conflict pairs, and binary availability. Directives emitted in deterministic catalog order. Well-tested with meaningful assertions.
- **Test infrastructure**: `ScopedCommandSearchPath` RAII guard with mutex serialization and poisoned-lock recovery is elegant. Tests are behavior-focused, not implementation-coupled.
- **IPC boundary**: Serde types align correctly between Rust and TypeScript. Field names match. The `skip_serializing_if = "Vec::is_empty"` annotation keeps TOML files clean.
- **Frontend autosave**: The debounced effect with `cancelled` flag closure pattern correctly prevents stale async updates. The race condition concern was investigated and found properly handled.
- **Accessibility**: `aria-labelledby`, `aria-expanded`, `aria-describedby`, `aria-live="polite"`, keyboard escape handling on tooltips. Good Steam Deck/controller awareness.
- **ValidationError variants**: All 6 new variants have clear, actionable user-facing messages. Each maps to a distinct failure mode with specific context.
- **Defense in depth**: Conflict detection runs in both TypeScript (UI prevention) and Rust (launch-time validation), providing two layers of safety.

---

## Recommended Action

1. **Done**: Critical issue #1 (symmetric FSR4 `conflictsWith` in TS) and important issues #2–#6 addressed in tree; see **Status** above.
2. **Follow-up (optional)**: Suggestions section — tests, type consolidation, logging, and doc tweaks can be scheduled separately.
