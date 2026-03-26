# PR #81 Review: feat(update) — Add update game panel

**Branch**: `feat/update-game`
**Date**: 2026-03-26
**Reviewers**: code-reviewer, silent-failure-hunter, type-design-analyzer, pr-test-analyzer, comment-analyzer

## Critical Issues (3 found)

### Issue 1: Validation message mismatch between Rust and TypeScript

- **Severity**: Critical
- **Files**: `src/types/update.ts:28-39`, `crates/crosshook-core/src/update/models.rs:56-80`
- **Found by**: code-reviewer, silent-failure-hunter, type-design-analyzer, pr-test-analyzer, comment-analyzer
- **Description**: Four validation messages in `UPDATE_GAME_VALIDATION_MESSAGES` differ from their Rust `message()` counterparts. The `mapValidationErrorToField()` exact-match lookup fails for these variants, falling through to a fuzzy heuristic that may misroute errors. Notably `PrefixPathNotDirectory` has no fuzzy fallback for "must be a directory" so it becomes a `generalError` instead of a field-specific error.
- **Status: Fixed**

### Issue 2: Duplicate HTML element IDs when both panels render on same page

- **Severity**: Critical
- **Files**: `src/components/ui/ProtonPathField.tsx:28,44`
- **Found by**: code-reviewer
- **Description**: `ProtonPathField` hardcodes `id="install-detected-proton"` and `id="install-proton-path"`. Both `InstallGamePanel` and `UpdateGamePanel` render this component simultaneously on `InstallPage`, producing duplicate DOM IDs. This breaks `htmlFor`/label associations and accessibility.
- **Status: Fixed**

### Issue 3: Race condition — stage transitions out of order for fast-exiting updaters

- **Severity**: Critical
- **Files**: `src/hooks/useUpdateGame.ts:197-218`
- **Found by**: code-reviewer, type-design-analyzer, silent-failure-hunter
- **Description**: If the update process exits before `invoke('update_game')` resolves, the `update-complete` listener fires `setStage('complete')` before line 217 sets `setStage('running_updater')`. This produces `preparing → complete → running_updater`, leaving the UI stuck on "Running..." after the process has finished.
- **Status: Fixed**

## Important Issues (10 found)

### Issue 4: `null` exit code treated as success

- **Severity**: Important
- **Files**: `src/hooks/useUpdateGame.ts:205`
- **Found by**: type-design-analyzer
- **Description**: On Unix, `status.code()` returns `None` when a process is killed by a signal (SIGKILL, SIGTERM). The hook treats `exitCode === null` as success, but a signal-killed updater did not complete its work.
- **Status: Fixed**

### Issue 5: Event listener leak on component unmount during active update

- **Severity**: Important
- **Files**: `src/hooks/useUpdateGame.ts:197-225`
- **Found by**: code-reviewer, type-design-analyzer, silent-failure-hunter
- **Description**: `unlistenComplete` is stored in a local variable inside `startUpdate`. If the component unmounts during an update, the listener persists and calls state setters on an unmounted component. Multiple listeners can accumulate across navigations.
- **Status: Fixed**

### Issue 6: `reset()` does not clean up active event listener

- **Severity**: Important
- **Files**: `src/hooks/useUpdateGame.ts:228-235`
- **Found by**: silent-failure-hunter
- **Description**: Clicking "Reset" during an active update resets UI state but leaves the `update-complete` listener active, which can override the reset state when the process exits.
- **Status: Fixed**

### Issue 7: `canStart` blocks retry after complete/failed without explicit reset

- **Severity**: Important
- **Files**: `src/hooks/useUpdateGame.ts:282`
- **Found by**: code-reviewer, type-design-analyzer, silent-failure-hunter
- **Description**: After a successful or failed update, the "Apply Update" button is disabled because `canStart` requires `stage === 'idle'`. User must click "Reset" (which clears all fields) before retrying. No visual hint about needing to reset.
- **Status: Fixed**

### Issue 8: Empty catch blocks swallow profile load failures

- **Severity**: Important
- **Files**: `src/hooks/useUpdateGame.ts:112-113, 118`
- **Found by**: silent-failure-hunter
- **Description**: When individual profiles fail to load (corrupted TOML, permissions), they silently vanish from the dropdown. When `profile_list` itself fails, the user sees "No proton_run profiles found" with no error indication.
- **Status: Fixed**

### Issue 9: No child process termination on reset or unmount

- **Severity**: Important
- **Files**: `src-tauri/src/commands/update.rs:54-69`
- **Found by**: silent-failure-hunter
- **Description**: The child process handle is consumed by `spawn_log_stream`. There is no way for the frontend to cancel a running update. Clicking "Reset" while an update is running leaves an orphaned process modifying the prefix.
- **Status: Fixed**

### Issue 10: Hardcoded `"update-complete"` event name in parameterized function

- **Severity**: Important
- **Files**: `src-tauri/src/commands/update.rs:106`
- **Found by**: code-reviewer
- **Description**: `stream_log_lines` accepts an `event_name` parameter but hardcodes `"update-complete"` for the completion event, breaking the generic contract.
- **Status: Fixed**

### Issue 11: `succeeded: true` returned before process actually succeeds

- **Severity**: Important
- **Files**: `crates/crosshook-core/src/update/service.rs:58-59`
- **Found by**: silent-failure-hunter, type-design-analyzer
- **Description**: `update_game` returns `succeeded: true` immediately after spawning, before the process runs. Misleading for future consumers.
- **Status: Fixed**

### Issue 12: ConsoleView empty state text only mentions launch-log

- **Severity**: Important
- **Files**: `src/components/ConsoleView.tsx:113-116`
- **Found by**: comment-analyzer
- **Description**: Empty state placeholder says output appears for `launch-log` events but doesn't mention `update-log`. Users applying updates may not expect output to appear here.
- **Status: Fixed**

### Issue 13: `LogPayload` JSDoc references only launch-log

- **Severity**: Important
- **Files**: `src/utils/log.ts:1-4, 11-15`
- **Found by**: comment-analyzer
- **Description**: JSDoc says "Payload shape emitted by backend `launch-log` events" but the type is now shared with `update-log`.
- **Status: Fixed**

## Suggestions (8 found)

### Issue 14: No integration test for `update_game` orchestration function

- **Severity**: Suggestion
- **Files**: `crates/crosshook-core/src/update/service.rs:45-65`
- **Found by**: pr-test-analyzer
- **Description**: The `update_game` function wiring (validate → build command → spawn) is untested at the integration level.
- **Status: Fixed**

### Issue 15: `build_update_command` test only asserts `is_ok()` without inspecting command

- **Severity**: Suggestion
- **Files**: `crates/crosshook-core/src/update/service.rs:330-338`
- **Found by**: pr-test-analyzer
- **Description**: The test verifies the command was constructed but doesn't inspect environment variables or arguments.
- **Status: Fixed**

### Issue 16: `is_executable_file` silently returns false on metadata read failure

- **Severity**: Suggestion
- **Files**: `crates/crosshook-core/src/update/service.rs:125-128`
- **Found by**: silent-failure-hunter
- **Description**: Pre-existing pattern from install module. Metadata errors produce misleading "not executable" messages instead of surfacing the real I/O issue.
- **Status: Open**

### Issue 17: CSS section comment scope narrower than actual usage

- **Severity**: Suggestion
- **Files**: `src/styles/theme.css:2489`
- **Found by**: comment-analyzer
- **Description**: Modal classes are generic but labeled "Update Game: Confirmation Modal". Should be labeled generically to encourage reuse.
- **Status: Fixed**

### Issue 18: PascalCase TS variants vs snake_case Rust serde — latent mismatch

- **Severity**: Suggestion
- **Files**: `src/types/update.ts:15-25`, `crates/crosshook-core/src/update/models.rs:42-55`
- **Found by**: type-design-analyzer
- **Description**: Pre-existing systemic pattern from install module. Not triggered currently because errors are transported as Display strings, not serialized enum variants.
- **Status: Open**

### Issue 19: Duplicated `create_log_path` and slug functions across command modules

- **Severity**: Suggestion
- **Files**: `src-tauri/src/commands/update.rs:39-52`, `src-tauri/src/commands/install.rs:40-53`
- **Found by**: type-design-analyzer, code-reviewer
- **Description**: Pre-existing code duplication pattern. Three copies now exist (install, launch, update).
- **Status: Fixed**

### Issue 20: Missing case-insensitive `.EXE` test

- **Severity**: Suggestion
- **Files**: `crates/crosshook-core/src/update/service.rs:118-122`
- **Found by**: pr-test-analyzer
- **Description**: `is_windows_executable` uses `eq_ignore_ascii_case` but no test verifies `.EXE` or `.Exe` passes validation.
- **Status: Fixed**

### Issue 21: Log file read error spams warnings without notifying frontend

- **Severity**: Suggestion
- **Files**: `src-tauri/src/commands/update.rs:99-101`
- **Found by**: silent-failure-hunter
- **Description**: Pre-existing pattern from launch module. Log file permanently inaccessible produces infinite warnings with no user feedback.
- **Status: Fixed**

## Strengths

- **Architecture**: Clean three-file module layout mirrors install module. Workspace crate separation respected.
- **Validation**: All 10 validation variants tested with dedicated tests using real filesystem fixtures.
- **Subscribe-before-invoke**: Race condition fix correctly registers `update-complete` listener before invoking the command.
- **Confirmation dialog**: Appropriate for an irreversible prefix-modifying operation with default focus on Cancel.
- **Component extraction**: `InstallField` and `ProtonPathField` cleanly extracted to `ui/` for reuse.
- **Self-documenting code**: Rust types and functions use descriptive names; minimal comments are appropriate.
