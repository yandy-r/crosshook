# PR Review #214 — feat(flatpak): harden phase 3 process execution

**Reviewed**: 2026-04-12T13:13:56-04:00
**Mode**: PR
**Author**: yandy-r
**Branch**: feat/flatpak-phase-3-process-execution-hardening → main
**Decision**: REQUEST CHANGES

## Summary

The PR materially improves Flatpak host-command handling and validation, and the local Rust test suite plus frontend build both pass in an isolated checkout. I found three blocking regressions, though: the new Flatpak `unshare` path breaks Steam trainer launches, document-portal selections are rewritten too early for sandbox-local file reads, and custom launch env vars are now exposed in host process argv.

## Findings

### CRITICAL

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:67` — Flatpak trainer launches with `network_isolation=true` now execute `steam-launch-trainer.sh` via `unshare ... bash -c <script>`, but that helper resolves its sibling runner from `${BASH_SOURCE[0]}` in [`steam-launch-trainer.sh`](/tmp/crosshook-pr-214-worktree/src/crosshook-native/runtime-helpers/steam-launch-trainer.sh:71). Under `bash -c`, `BASH_SOURCE[0]` is empty and `$0` becomes `--`, so the helper looks in the current working directory and fails before it can start `steam-host-trainer-runner.sh`.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Stop inlining this helper through `bash -c`; execute a real script file path on the host side so `${BASH_SOURCE[0]}` stays valid, or pass the runner path/content explicitly from Rust.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/platform.rs:242` — In Flatpak mode, the new host wrapper serializes every env var as `--env=KEY=VALUE` on the `flatpak-spawn` argv. The launch builders merge `request.custom_env_vars` into that env map before calling this wrapper from [`script_runner.rs`](/tmp/crosshook-pr-214-worktree/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:153), [`script_runner.rs`](/tmp/crosshook-pr-214-worktree/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:265), and [`script_runner.rs`](/tmp/crosshook-pr-214-worktree/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:356), so any sensitive user-provided launch variable is now exposed in host process argv during launch.
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Do not pass user-controlled env values through `--env=` argv. Restrict argv-passed env to fixed runtime keys and move custom env through a non-argv channel such as a 0600 temp env file or another host-side handoff.

- **[F003]** `src/crosshook-native/src/utils/dialog.ts:25` — `resolveDialogPath()` now rewrites all file-picker results to raw host paths via `normalize_host_path`, including document-portal selections. That drops the portal grant before the path reaches sandbox-local I/O call sites such as [`import.rs`](/tmp/crosshook-pr-214-worktree/src/crosshook-native/crates/crosshook-core/src/game_images/import.rs:33) and trainer copy-to-prefix staging in [`script_runner.rs`](/tmp/crosshook-pr-214-worktree/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:333). In Flatpak, selections from external drives or other portal-only locations will regress from readable to “missing” because the sandbox can read the portal path, not the rewritten host path.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Preserve the portal path for sandbox-local reads/copies, or carry both portal and host-normalized forms and only switch to the host path at host-executed command boundaries.

### MEDIUM

- **[F004]** `src/crosshook-native/src/components/pages/LaunchPage.tsx:89` — The new Flatpak “No network isolation” badge on LaunchPage is populated from `profile_list_summaries`, but [`profile.rs`](/tmp/crosshook-pr-214-worktree/src/crosshook-native/src-tauri/src/commands/profile.rs:316) only loads `effective_profile()` and never merges collection defaults. LaunchPage itself is collection-aware, so a collection override of `launch.network_isolation` can make the badge disagree with the actual request the page launches.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Make the badge source collection-aware by threading `collection_id` into the summary path or by deriving the badge from the already-selected collection-merged profile state.

### LOW

## Validation Results

| Check      | Result  |
| ---------- | ------- |
| Type check | Pass    |
| Lint       | Skipped |
| Tests      | Pass    |
| Build      | Pass    |

## Files Reviewed

- `.gitignore` (Modified)
- `docs/prps/plans/completed/flatpak-phase-3-process-execution-hardening.plan.md` (Modified)
- `docs/prps/reports/flatpak-phase-3-process-execution-hardening-report.md` (Added)
- `src/crosshook-native/Cargo.lock` (Modified)
- `src/crosshook-native/crates/crosshook-cli/src/main.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/export/diagnostics.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/install/models.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/install/service.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/patterns.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/offline/readiness.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/platform.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/update/service.rs` (Modified)
- `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh` (Modified)
- `src/crosshook-native/runtime-helpers/steam-launch-helper.sh` (Modified)
- `src/crosshook-native/runtime-helpers/steam-launch-trainer.sh` (Modified)
- `src/crosshook-native/src-tauri/src/commands/launch.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/profile.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/run_executable.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/shared.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/update.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/components/pages/LaunchPage.tsx` (Modified)
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx` (Modified)
- `src/crosshook-native/src/components/ui/ThemedSelect.tsx` (Modified)
- `src/crosshook-native/src/context/LaunchStateContext.tsx` (Modified)
- `src/crosshook-native/src/hooks/useLaunchPlatformStatus.ts` (Added)
- `src/crosshook-native/src/hooks/useLibrarySummaries.ts` (Modified)
- `src/crosshook-native/src/hooks/useProfileSummaries.ts` (Added)
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/profile.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/wrapHandler.ts` (Modified)
- `src/crosshook-native/src/types/library.ts` (Modified)
- `src/crosshook-native/src/utils/dialog.ts` (Modified)
- `tasks/lessons.md` (Modified)
- `tasks/todo.md` (Modified)
