# Task Structure Analysis: update-game

## Executive Summary

The update-game feature decomposes cleanly into three phases along the Rust backend / Tauri commands / React frontend boundary, with meaningful parallelization opportunities within each phase. The backend core module (models + service + tests) has zero external dependencies and can be built first in isolation. Shared utility extraction (`create_log_path`, `spawn_log_stream`, `InstallField`/`ProtonPathField`) is the main cross-cutting concern that should be front-loaded to unblock both update implementation and reduce duplication debt. The frontend layer (types, hook, component, console modifications) is the widest phase with the most independent tasks but depends on Tauri commands existing for integration testing.

## Recommended Phase Structure

### Phase 1: Backend Core + Shared Utility Extraction

**Purpose**: Build the entire `crosshook-core::update` module and extract shared utilities that both update and existing code need. This phase produces compilable, testable Rust code with no frontend or Tauri dependencies.

**Suggested Tasks**:

1. **Task 1A: Create `update/models.rs`** -- Define `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError` with serde derives, `message()` methods, `Display`, `Error`, and `From<>` impls. Follow the install/models.rs pattern exactly.
   - New file: `crates/crosshook-core/src/update/models.rs`

2. **Task 1B: Create `update/service.rs`** -- Implement `validate_update_request`, `build_update_command`, and `update_game`. Reuse `runtime_helpers::new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`. The validation helpers `is_windows_executable` and `is_executable_file` must be duplicated from install/service.rs (or extracted -- see Task 1D). The `update_game` function follows the install pattern but skips prefix provisioning and post-install discovery.
   - New file: `crates/crosshook-core/src/update/service.rs`

3. **Task 1C: Create `update/mod.rs` + register module** -- Standard three-file module layout with selective `pub use` re-exports. Add `pub mod update;` to `crates/crosshook-core/src/lib.rs`.
   - New file: `crates/crosshook-core/src/update/mod.rs`
   - Modify: `crates/crosshook-core/src/lib.rs`

4. **Task 1D: Extract shared validation helpers (optional, recommended)** -- Move `is_windows_executable` and `is_executable_file` from `install/service.rs` to a shared location (e.g., a `validation` utility module in crosshook-core, or make them `pub(crate)` in install). This avoids duplicating 20 lines of platform-specific logic. If deferred, duplicate them in `update/service.rs` with a TODO comment.
   - Modify: `crates/crosshook-core/src/install/service.rs` (change visibility or re-export)
   - Optionally new file if creating a shared util module

5. **Task 1E: Write unit tests for update module** -- Test `validate_update_request` (all 10 validation error variants), `build_update_command` (environment variables set correctly, working directory, log attachment), and `update_game` (success path, failure path). Follow the tempfile-based test pattern from install/service.rs.
   - Modify: `crates/crosshook-core/src/update/service.rs` (inline `#[cfg(test)] mod tests`)

**Parallelization**: Tasks 1A and 1D can run in parallel. Task 1B depends on 1A (needs types) and optionally 1D (needs validation helpers). Task 1C depends on 1A and 1B. Task 1E depends on 1B. In practice, 1A+1D first, then 1B, then 1C+1E.

### Phase 2: Tauri Commands + Frontend Types and Hook

**Purpose**: Wire the backend into Tauri IPC, build the frontend type layer and state machine hook, and modify console components for `update-log` support. This phase has the most parallelization: once the Tauri commands exist, frontend types, hook, and console changes can all proceed independently.

**Dependencies**: Phase 1 must complete (Tauri commands import from `crosshook_core::update`).

**Suggested Tasks**:

1. **Task 2A: Extract shared `create_log_path` utility** -- Currently duplicated identically in `commands/launch.rs:168-181` and `commands/install.rs:40-53`. Create a shared utility (e.g., `src-tauri/src/log_path.rs` or a helper in `src-tauri/src/commands/mod.rs`) and update both existing callers. The update command then imports from the same shared location.
   - New file or modify: `src-tauri/src/commands/mod.rs` or new `src-tauri/src/log_path.rs`
   - Modify: `src-tauri/src/commands/launch.rs` (remove local `create_log_path`, import shared)
   - Modify: `src-tauri/src/commands/install.rs` (remove local `create_log_path`, import shared)

2. **Task 2B: Extract or parameterize `spawn_log_stream`** -- Currently private in `commands/launch.rs:103-113`. Either: (a) extract `spawn_log_stream` and `stream_log_lines` to a shared module and parameterize with `event_name: &'static str`, or (b) duplicate in `commands/update.rs` with `"update-log"` event name. Option (a) is recommended because it also enables the completion event the spec requires.
   - Modify: `src-tauri/src/commands/launch.rs` (extract functions, add event_name parameter)
   - New file (if extracting): e.g., `src-tauri/src/log_stream.rs`

3. **Task 2C: Create `commands/update.rs` + register** -- Implement `update_game` and `validate_update_request` Tauri commands. The `update_game` command follows the launch streaming pattern: validate, build command via core, spawn process, call `spawn_log_stream` with `"update-log"` event, return immediately with result. Add `pub mod update;` to `commands/mod.rs` and register both commands in `lib.rs` `invoke_handler`.
   - New file: `src-tauri/src/commands/update.rs`
   - Modify: `src-tauri/src/commands/mod.rs`
   - Modify: `src-tauri/src/lib.rs` (add to `generate_handler![]`)

4. **Task 2D: Create `types/update.ts`** -- Define `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameStage`, `UpdateGameValidationError` type union, `UPDATE_GAME_VALIDATION_MESSAGES` map, `UPDATE_GAME_VALIDATION_FIELD` map. Add `export * from './update';` to `types/index.ts`.
   - New file: `src/types/update.ts`
   - Modify: `src/types/index.ts`

5. **Task 2E: Create `hooks/useUpdateGame.ts`** -- State machine hook with stages: `'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed'`. Simpler than `useInstallGame` (no prefix resolution, no review stage, no executable discovery). Manages: request state, validation state, stage, result, error. Exposes: `startUpdate()`, `reset()`, `setRequest`, `updateRequest`, derived `statusText`/`hintText`/`actionLabel`.
   - New file: `src/hooks/useUpdateGame.ts`

6. **Task 2F: Add `update-log` subscription to ConsoleView and ConsoleDrawer** -- Both components currently hardcode `listen('launch-log', ...)`. Add a second `listen('update-log', ...)` call in each component's `useEffect`, using the same handler logic. Both listeners share the same `active` guard and cleanup.
   - Modify: `src/components/ConsoleView.tsx` (add second `listen` call for `'update-log'`)
   - Modify: `src/components/layout/ConsoleDrawer.tsx` (add second `listen` call for `'update-log'`)

**Parallelization**: Tasks 2A and 2B can run in parallel (shared utility extractions). Task 2C depends on 2A and 2B. Tasks 2D, 2E, and 2F are independent of each other and can all run in parallel. Task 2E depends on 2D (needs types). Task 2F has no dependency on other Phase 2 tasks. Best sequence: 2A+2B+2D+2F in parallel, then 2C+2E, with 2E starting as soon as 2D completes.

### Phase 3: UI Component + Integration

**Purpose**: Build the `UpdateGamePanel` component, extract shared sub-components, integrate into the Install page, and perform end-to-end validation.

**Dependencies**: Phase 2 tasks 2C (Tauri commands), 2E (hook), and 2F (console subscription) must complete.

**Suggested Tasks**:

1. **Task 3A: Extract `InstallField` and `ProtonPathField` to shared components** -- Currently defined locally in `InstallGamePanel.tsx` (lines 64-111 and 113-179). Move to `src/components/ui/InstallField.tsx` and `src/components/ui/ProtonPathField.tsx` (or a single `src/components/ui/FormFields.tsx`). Update `InstallGamePanel.tsx` imports. This unblocks clean reuse in `UpdateGamePanel`.
   - New file(s): `src/components/ui/InstallField.tsx`, `src/components/ui/ProtonPathField.tsx`
   - Modify: `src/components/InstallGamePanel.tsx` (remove local definitions, import from shared)

2. **Task 3B: Create `UpdateGamePanel.tsx`** -- Main UI component with: profile selector (ThemedSelect, filtered to `proton_run` profiles), update executable field (extracted InstallField), Proton path field (extracted ProtonPathField), status card, action buttons ("Apply Update" / "Retry"), confirmation dialog. Uses `useUpdateGame` hook. Loads profiles via `invoke('profile_list')` + `invoke('profile_load', ...)` to populate and filter the selector.
   - New file: `src/components/UpdateGamePanel.tsx`

3. **Task 3C: Integrate into InstallPage.tsx** -- Import `UpdateGamePanel` and render it below `InstallGamePanel` with appropriate section separator. Pass shared `protonInstalls` and `protonInstallsError` state as props (already loaded in InstallPage).
   - Modify: `src/components/pages/InstallPage.tsx`

4. **Task 3D: End-to-end validation and CSS polish** -- Manual testing: select profile, verify auto-fill, browse for update exe, apply update, verify console streaming, verify completion status. Verify gamepad navigation. Ensure CSS classes follow `crosshook-update-*` naming and visual consistency with install section.
   - May modify: `src/styles/theme.css` (new update-specific CSS classes)

**Parallelization**: Task 3A can start immediately at Phase 3 start. Task 3B depends on 3A (needs extracted components). Task 3C depends on 3B. Task 3D depends on 3C.

## Task Granularity Recommendations

- **Smallest meaningful unit**: A single file creation or modification that can be compiled/type-checked independently. Example: `update/models.rs` is one task because it has no internal dependencies beyond the crate's existing types.
- **Grouping rationale**: Tasks that touch the same file should be grouped (e.g., creating `mod.rs` and adding the `pub mod` line to `lib.rs` together). Tasks that touch different layers (Rust core vs. Tauri commands vs. frontend) should be separate even if logically related.
- **Test tasks**: Unit tests should be part of the same task as the code they test (not separate tasks), because the test file is co-located in Rust and the test patterns are straightforward adaptations of existing install tests.
- **Extraction tasks**: Shared utility extraction (1D, 2A, 2B, 3A) are separate tasks because they modify existing working code and have their own risk surface (regressions in install/launch).

## Dependency Analysis

### Independent Tasks (can start immediately)

- **Task 1A** (update/models.rs) -- no dependencies beyond existing crate types
- **Task 1D** (extract validation helpers) -- modifies install/service.rs independently
- **Task 2D** (types/update.ts) -- can be written from the feature spec without backend existing
- **Task 2F** (ConsoleView/ConsoleDrawer update-log subscription) -- independent modification, event name is a known string

### Sequential Dependencies

```
Phase 1:
  1A (models) ──> 1B (service) ──> 1C (mod.rs + lib.rs registration) ──> 1E (tests)
  1D (extract helpers) ──> 1B (service uses them)

Phase 2:
  Phase 1 complete ──> 2C (Tauri commands)
  2A (extract create_log_path) ──> 2C (commands/update.rs uses it)
  2B (extract spawn_log_stream) ──> 2C (commands/update.rs uses it)
  2D (types/update.ts) ──> 2E (useUpdateGame hook imports types)

Phase 3:
  3A (extract shared components) ──> 3B (UpdateGamePanel uses them)
  2E (hook) ──> 3B (UpdateGamePanel uses hook)
  2C (commands registered) ──> 3B (component calls invoke)
  3B (component) ──> 3C (InstallPage integration)
  3C (integration) ──> 3D (e2e validation)
```

### Potential Bottlenecks

1. **Task 2B (spawn_log_stream extraction)**: This is the riskiest extraction because it modifies the live launch commands and introduces a parameterized event name. If it regresses launch log streaming, it blocks both update and launch features. Mitigation: test launch log streaming immediately after extraction.

2. **Task 2C (Tauri command registration)**: This is the gateway between backend and frontend. Until commands are registered in `generate_handler![]`, no frontend integration testing is possible. It depends on three prior tasks (Phase 1, 2A, 2B).

3. **Profile filtering on frontend (within Task 3B)**: The `profile_list` command returns only names, not launch methods. The frontend must call `profile_load` for each profile to determine if it is `proton_run`. For users with many profiles, this could cause a visible loading delay. Consider whether a new backend command (`profile_list_with_method`) would be worthwhile, or accept the N+1 load pattern for the initial implementation.

4. **Exit code reporting with streaming**: The current `stream_log_lines` discards the child exit status. The feature spec requires reporting success/failure. Task 2B must decide the mechanism: emit a completion event (`"update-complete"`) with the exit status, or have the hook poll for completion. This architectural decision must be made before 2C.

## File-to-Task Mapping

### Files to Create

| File                                          | Task | Phase | Description                                                        |
| --------------------------------------------- | ---- | ----- | ------------------------------------------------------------------ |
| `crates/crosshook-core/src/update/models.rs`  | 1A   | 1     | Request, Result, Error, ValidationError types with serde + Display |
| `crates/crosshook-core/src/update/service.rs` | 1B   | 1     | validate, build_command, update_game functions + unit tests (1E)   |
| `crates/crosshook-core/src/update/mod.rs`     | 1C   | 1     | Module root with pub use re-exports                                |
| `src-tauri/src/commands/update.rs`            | 2C   | 2     | Tauri IPC commands: update_game, validate_update_request           |
| `src/types/update.ts`                         | 2D   | 2     | TypeScript types, validation maps, stage union                     |
| `src/hooks/useUpdateGame.ts`                  | 2E   | 2     | React hook for update flow state machine                           |
| `src/components/UpdateGamePanel.tsx`          | 3B   | 3     | Main UI component with profile selector, form, status              |
| `src/components/ui/InstallField.tsx`          | 3A   | 3     | Extracted shared field component (from InstallGamePanel)           |
| `src/components/ui/ProtonPathField.tsx`       | 3A   | 3     | Extracted shared Proton path component (from InstallGamePanel)     |

### Files to Modify

| File                                           | Task   | Phase | Change                                                                           |
| ---------------------------------------------- | ------ | ----- | -------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs`             | 1C     | 1     | Add `pub mod update;`                                                            |
| `crates/crosshook-core/src/install/service.rs` | 1D     | 1     | Change visibility of `is_windows_executable` and `is_executable_file` or extract |
| `src-tauri/src/commands/mod.rs`                | 2C     | 2     | Add `pub mod update;`                                                            |
| `src-tauri/src/lib.rs`                         | 2C     | 2     | Add update commands to `generate_handler![]`                                     |
| `src-tauri/src/commands/launch.rs`             | 2A, 2B | 2     | Extract `create_log_path`, extract/parameterize `spawn_log_stream`               |
| `src-tauri/src/commands/install.rs`            | 2A     | 2     | Remove local `create_log_path`, import shared                                    |
| `src/types/index.ts`                           | 2D     | 2     | Add `export * from './update';`                                                  |
| `src/components/ConsoleView.tsx`               | 2F     | 2     | Add `listen('update-log', ...)` subscription                                     |
| `src/components/layout/ConsoleDrawer.tsx`      | 2F     | 2     | Add `listen('update-log', ...)` subscription                                     |
| `src/components/InstallGamePanel.tsx`          | 3A     | 3     | Remove local InstallField/ProtonPathField, import from shared                    |
| `src/components/pages/InstallPage.tsx`         | 3C     | 3     | Import and render UpdateGamePanel below InstallGamePanel                         |
| `src/styles/theme.css`                         | 3D     | 3     | Add `crosshook-update-*` CSS classes (may reuse install patterns)                |

## Optimization Opportunities

1. **Front-load all extractions**: Tasks 1D, 2A, 2B, and 3A are all extractions of existing code into shared locations. Running them early (1D in Phase 1, 2A+2B at start of Phase 2, 3A at start of Phase 3) unblocks dependent tasks and reduces merge conflict risk.

2. **Types-first frontend development**: Task 2D (types/update.ts) can be written from the feature spec alone, without the backend existing. Starting it in parallel with Phase 1 gives the hook author a stable type surface to build against.

3. **Console modification is fully independent**: Task 2F (adding `update-log` to ConsoleView/ConsoleDrawer) requires only knowing the event name string. It can run in any phase and has no dependency on backend or hook implementation.

4. **Consider a unified log stream utility in src-tauri**: Rather than extracting `spawn_log_stream` as a one-off, create a `src-tauri/src/log_stream.rs` module that provides both `create_log_path` and `spawn_log_stream` with event name parameterization. This consolidates two extraction tasks (2A + 2B) into one and provides a clean home for future streaming features.

5. **Skip `waitforexitandrun` for Phase 1**: The feature spec mentions `proton waitforexitandrun` as preferred, but `proton run` works identically for the streaming pattern (the process blocks at the Proton level regardless). Using `run` avoids needing a new runtime_helpers function and keeps the implementation simpler. `waitforexitandrun` can be added as a Phase 2 enhancement if needed.

6. **Profile filtering strategy**: Rather than N+1 `profile_load` calls to filter by launch method, consider adding a lightweight backend command that returns `Vec<(String, String)>` of (name, method) pairs. This is a small addition to `commands/profile.rs` and eliminates the frontend filtering bottleneck. However, for an MVP with typically fewer than 20 profiles, the N+1 pattern is acceptable.

## Implementation Strategy Recommendations

1. **Start with Task 1A + 1D + 2D + 2F in parallel** -- These four tasks have zero interdependencies and cover all three layers (core Rust, Tauri, frontend). This front-loads the most independent work.

2. **Critical path is**: 1A -> 1B -> 1C -> 2A+2B -> 2C -> 3B -> 3C -> 3D. Optimize for unblocking Task 2C (Tauri commands) as early as possible, since it is the gateway between backend and frontend.

3. **Test at each phase boundary**: After Phase 1, run `cargo test -p crosshook-core` to verify the update module. After Phase 2 Task 2C, verify the commands compile and register (dev build starts without error). After Phase 3 Task 3C, perform manual end-to-end testing.

4. **Handle exit code reporting in Task 2B**: When extracting `spawn_log_stream`, add a mechanism for the streaming task to report process completion. The simplest approach: emit a final event (`"update-complete"` or a parameterized `"{event_name}-complete"`) with a JSON payload containing the exit code. The hook subscribes to this event to transition from `'running_updater'` to `'complete'` or `'failed'`.

5. **CSS reuse over duplication**: The update panel's visual structure is nearly identical to the install panel. Prefer reusing `crosshook-install-*` class names where semantics match, and only introduce `crosshook-update-*` classes for update-specific elements (e.g., the profile selector section). This minimizes theme.css growth.
