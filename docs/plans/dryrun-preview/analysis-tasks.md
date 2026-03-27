# Dry Run / Preview — Task Structure Analysis

## Executive Summary

The dryrun-preview feature decomposes into **4 phases with 9 implementation tasks**. The critical path runs through 3 sequential backend tasks (foundation types, `validate_all()`, `build_launch_preview()`) before the Tauri command and frontend can proceed. Two independent tracks — backend types and frontend types — can run in parallel from the start. The feature touches 2 new files and 7 existing files; no task exceeds 3 files, and each is independently verifiable via `cargo check`, `cargo test`, or visual inspection.

---

## Recommended Phase Structure

### Phase 1: Foundation (2 tasks, parallelizable)

Establish the type system on both sides of the IPC boundary. These tasks have zero dependencies and can run simultaneously.

| Task                                  | Files                                                                 | Description                                                                                                                                                                                                                                                                            | Estimated Size                   | Verifiable By                             |
| ------------------------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ----------------------------------------- |
| **T1: Backend types + module wiring** | `preview.rs` (create), `mod.rs` (modify), `optimizations.rs` (modify) | Define all Rust structs (`LaunchPreview`, `PreviewEnvVar`, `EnvVarSource`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation`) in new `preview.rs`. Add `pub mod preview;` and re-exports in `mod.rs`. Add `Serialize, Deserialize` to `LaunchDirectives` in `optimizations.rs`. | ~90 lines new, ~5 lines modified | `cargo check -p crosshook-core`           |
| **T2: Frontend types**                | `launch.ts` (modify)                                                  | Add TypeScript interfaces: `LaunchPreview`, `PreviewEnvVar`, `EnvVarSource`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation`.                                                                                                                                                 | ~45 lines added                  | TypeScript compilation (no errors in IDE) |

### Phase 2: Core Logic (2 tasks, sequential)

Build the two core computation functions. `validate_all()` must be complete before `build_launch_preview()` can call it.

| Task                                           | Files                                                                            | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Estimated Size | Verifiable By                                          |
| ---------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------------------------------------------------------ |
| **T3: `validate_all()` + collectors**          | `request.rs` (modify)                                                            | Implement `validate_all()` with method dispatch and 3 collector helpers: `collect_steam_issues()`, `collect_proton_issues()`, `collect_native_issues()`. Each mirrors its `validate_*` counterpart but pushes to `Vec<LaunchValidationIssue>` instead of returning `Err`. Key gotcha: `validate_proton_run` calls `resolve_launch_directives()` at line 505 — the collector must catch that error and push it as an issue.                                                                                                 | ~100-120 lines | `cargo test -p crosshook-core` (new unit tests inline) |
| **T4: `build_launch_preview()` + env helpers** | `preview.rs` (modify — extend file from T1), `runtime_helpers.rs` (minor modify) | Implement `build_launch_preview()`, pure env collection functions (`collect_host_environment`, `collect_runtime_proton_environment`, `collect_steam_proton_environment`, `collect_optimization_environment`), `build_effective_command_string()`, `build_proton_setup()`, `build_trainer_info()`, `resolve_working_directory()`, and `to_display_text()`. **Note:** `env_value()` in `runtime_helpers.rs:184` is private — change to `pub(crate)` (1-char fix) so preview env helpers can reuse it instead of duplicating. | ~180-220 lines | `cargo test -p crosshook-core` (new unit tests)        |

### Phase 3: Integration (2 tasks, parallelizable after Phase 2)

Wire backend to frontend through Tauri IPC and create the React state hook.

| Task                                 | Files                                            | Description                                                                                                                                                                                                                                   | Estimated Size                   | Verifiable By                                          |
| ------------------------------------ | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------------------------ |
| **T5: Tauri command + registration** | `commands/launch.rs` (modify), `lib.rs` (modify) | Add sync `preview_launch` command following the `validate_launch` thin-wrapper pattern. Register in `invoke_handler` after line 90 alongside other launch commands. Import `build_launch_preview` and `LaunchPreview` from `crosshook_core`.  | ~10 lines new, ~2 lines modified | `cargo check -p crosshook-native` (Tauri app compiles) |
| **T6: `usePreviewState` hook**       | `usePreviewState.ts` (create)                    | React hook with `useState` pattern (loading, preview, error states). Wraps `invoke<LaunchPreview>('preview_launch', { request })`. Follows `useProfile.ts` pattern — individual `useState` hooks, async invoke function, error normalization. | ~40-50 lines                     | Import in LaunchPanel compiles; manual invoke test     |

### Phase 4: Frontend UI (2 tasks, sequential)

Build the preview button and modal display. The modal depends on the button + hook integration.

| Task                                  | Files                                       | Description                                                                                                                                                                                                                                                                                                                                          | Estimated Size | Verifiable By                                                      |
| ------------------------------------- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------------------------------------------------------------------ |
| **T7: Preview button in LaunchPanel** | `LaunchPanel.tsx` (modify)                  | Add "Preview Launch" ghost button in `__actions` div (lines 101-118). Wire to `usePreviewState` hook. Disable when `request === null` or `phase !== Idle`. Same guard as launch button.                                                                                                                                                              | ~20-30 lines   | Visual: button appears, disabled states correct                    |
| **T8: Preview modal display**         | `LaunchPanel.tsx` (modify — extend from T7) | Render preview modal using `ProfileReviewModal` portal/focus-trap infrastructure and `CollapsibleSection` accordions. Sections: Summary banner (always visible), Validation Results (expanded), Command Chain (expanded), Environment Variables (collapsed), Proton Setup (collapsed, hidden for native). Footer: Copy Preview / Launch Now / Close. | ~120-160 lines | Visual: modal opens with structured data, sections collapse/expand |

### Phase 5: Testing (1 task, after Phase 4)

| Task                           | Files                                    | Description                                                                                                                                                                                                                                                                                                                                                     | Estimated Size | Verifiable By                             |
| ------------------------------ | ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ----------------------------------------- |
| **T9: Unit tests for preview** | `preview.rs` (add `#[cfg(test)]` module) | Tests for `build_launch_preview()` covering: steam_applaunch method, proton_run method, native method, validation failures (partial results), directive resolution failure (partial results), trainer info with copy_to_prefix staged path, empty/minimal request. Use existing test fixtures (`tempfile::TempDir`, factory functions from `request.rs` tests). | ~150-200 lines | `cargo test -p crosshook-core -- preview` |

---

## Dependency Analysis

```
T1 (Backend types)  ──────┐
                           ├──▶ T3 (validate_all) ──▶ T4 (build_launch_preview) ──┐
                           │                                                        ├──▶ T5 (Tauri command) ──┐
T2 (Frontend types) ──────┤                                                        │                         ├──▶ T7 (Preview button) ──▶ T8 (Preview modal)
                           └──▶ T6 (usePreviewState hook) ────────────────────────┘                         │
                                                                                                             │
                                                                                    T9 (Tests) ◀─────────────┘
```

### Critical Path

**T1 → T3 → T4 → T5 → T7 → T8** (6 tasks in sequence)

This is the longest dependency chain. T3 (`validate_all`) is the gating task — it's the largest single implementation (~100 lines) and blocks the core preview function.

### What Blocks What

| Task | Blocks     | Reason                                              |
| ---- | ---------- | --------------------------------------------------- |
| T1   | T3, T4, T5 | Types must exist before logic can reference them    |
| T2   | T6, T7     | TypeScript interfaces needed for hook and component |
| T3   | T4         | `build_launch_preview()` calls `validate_all()`     |
| T4   | T5         | Tauri command wraps `build_launch_preview()`        |
| T5   | T7         | Preview button invokes the Tauri command            |
| T6   | T7         | Preview button uses the hook                        |
| T7   | T8         | Modal renders preview data from button's invoke     |
| T4   | T9         | Tests exercise `build_launch_preview()`             |

---

## File-to-Task Mapping

### Files to Create

| File                                          | Task(s)                                  | Total Lines |
| --------------------------------------------- | ---------------------------------------- | ----------- |
| `crates/crosshook-core/src/launch/preview.rs` | T1 (structs), T4 (functions), T9 (tests) | ~400-500    |
| `src/hooks/usePreviewState.ts`                | T6                                       | ~40-50      |

### Files to Modify

| File                                                  | Task   | Change Scope                                           |
| ----------------------------------------------------- | ------ | ------------------------------------------------------ |
| `crates/crosshook-core/src/launch/mod.rs`             | T1     | +2 lines (`pub mod`, `pub use`)                        |
| `crates/crosshook-core/src/launch/optimizations.rs`   | T1     | +1 line (derive macro)                                 |
| `crates/crosshook-core/src/launch/request.rs`         | T3     | +100-120 lines (`validate_all` + collectors)           |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | T4     | 1-char change (`fn` → `pub(crate) fn` for `env_value`) |
| `src-tauri/src/commands/launch.rs`                    | T5     | +8-10 lines (import + command)                         |
| `src-tauri/src/lib.rs`                                | T5     | +1 line (handler registration)                         |
| `src/types/launch.ts`                                 | T2     | +45 lines (interfaces)                                 |
| `src/components/LaunchPanel.tsx`                      | T7, T8 | +150-190 lines (button + modal)                        |

---

## Parallelization Opportunities

### Maximum Parallelism Plan

**Batch 1** (parallel — 2 agents):

- Agent A: T1 (Backend types + module wiring)
- Agent B: T2 (Frontend types)

**Batch 2** (sequential — 1 agent, after T1):

- Agent A: T3 (`validate_all`) → T4 (`build_launch_preview`)

**Batch 3** (parallel — 2 agents, after T4 + T2):

- Agent A: T5 (Tauri command + registration)
- Agent B: T6 (`usePreviewState` hook)

**Batch 4** (sequential — 1 agent, after T5 + T6):

- Agent A: T7 (Preview button) → T8 (Preview modal)

**Batch 5** (parallel with Batch 4 — 1 agent, after T4):

- Agent B: T9 (Unit tests)

### Realistic Parallelism (2-Agent Model)

Given the tight dependency chain on the backend, the practical parallelism is:

| Step | Agent A (Backend-focused)    | Agent B (Frontend-focused) |
| ---- | ---------------------------- | -------------------------- |
| 1    | T1: Backend types            | T2: Frontend types         |
| 2    | T3: `validate_all()`         | (blocked — waiting for T4) |
| 3    | T4: `build_launch_preview()` | T6: `usePreviewState` hook |
| 4    | T5: Tauri command            | T7: Preview button         |
| 5    | T9: Unit tests               | T8: Preview modal          |

This achieves 5 steps with 2 agents vs. 6 steps with 1 agent — a modest but real speedup. The frontend agent has a gap at step 2 where it's blocked waiting for backend types to propagate through the Tauri command.

---

## Implementation Strategy Recommendations

### 1. Start with T1 (Backend types) — it unblocks everything

The `LaunchPreview` struct definition and module wiring is the single most unblocking task. It's also simple (~90 lines of struct definitions) and low-risk. Getting this right establishes the data contract for all subsequent work.

### 2. T3 (`validate_all`) deserves the most review attention

This is the largest task by line count, mirrors existing code (duplication risk), and has the subtle gotcha where `validate_proton_run` internally calls `resolve_launch_directives()`. The collector version must handle this as a pushed issue, not a propagated error. Consider extracting shared helper logic from existing `validate_*` functions to reduce duplication.

### 3. T4 (`build_launch_preview`) is the integration point

This function calls into `validate_all()`, `resolve_launch_directives()`, and the new env collection helpers. It's the function that proves the whole backend works. Implement it alongside its helper functions and verify with manual test data before wiring Tauri.

### 4. Verify `chrono` availability before T4

The spec calls for `chrono::Utc::now().to_rfc3339()` for `generated_at`. The `shared.md` gotchas section flags that `chrono` may not be in `Cargo.toml`. Check this during T1 or T4 — if missing, either add `chrono = "0.4"` to the workspace or use `std::time::SystemTime` with manual ISO 8601 formatting.

### 5. T8 (Preview modal) is the largest frontend task

The modal rendering with collapsible sections, severity indicators, and footer actions is ~120-160 lines. It reuses existing infrastructure (`ProfileReviewModal`, `CollapsibleSection`) but requires careful attention to:

- Section visibility rules (hide Proton section for native method)
- `data-severity` attributes for color-coding
- Gamepad navigation via `data-crosshook-focus-root="modal"`
- 48px touch targets for Steam Deck

### 6. Tests (T9) can overlap with frontend work

Since tests only depend on the backend `build_launch_preview()` function (T4), they can run in parallel with frontend tasks T7-T8. This is the main parallelization win.

### 7. One-line quick win: `LaunchDirectives` Serde derive

Adding `Serialize, Deserialize` to `LaunchDirectives` in `optimizations.rs` is a single-line change that unblocks the preview struct's ability to include directive data. Bundle this with T1 to keep the foundation clean.
