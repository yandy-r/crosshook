# PR Review #400 — refactor: service.rs into smaller modules

**Reviewed**: 2026-04-20
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-service-rs → main
**Decision**: APPROVE

## Worktree Setup

- **Parent**: ~/.claude-worktrees/crosshook-pr-400/ (branch: codex/refactor-split-service-rs)
- **Children** (per severity; created by /ycc:review-fix --worktree):
  - LOW → ~/.claude-worktrees/crosshook-pr-400-low/ (branch: feat/pr-400-low)

## Summary

Clean behavior-preserving split of `run_executable/service.rs` (719 lines) into five cohesive submodules (`adhoc_prefix`, `command_builder`, `runner`, `validation`, plus `tests` and `mod`). All five original public functions are re-exported from `service/mod.rs`; the outer `run_executable/mod.rs` is untouched. Clippy (`-D warnings`), `cargo fmt --check`, all 25 `run_executable::service::tests::*` unit tests, and `scripts/check-host-gateway.sh` all pass. Only minor polish findings — no correctness, security, or performance concerns.

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `PR body` — PR body does not include an explicit `Part of #290` (or `Closes #<child-issue-number>`) reference, so the PR is not linked back to umbrella tracker issue #290 per `CLAUDE.md` ("Always link the related issue"). The issue description is quoted inline by the bot, but GitHub will not auto-link or cross-reference from quoted text.
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Edit the PR description and append a footer line `Part of #290` (and, if the child issue number is known, `Closes #<N>`). This is a recurring gap in the bot-opened child PRs under #290 and is worth flagging at the bot level, not just per-PR.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/run_executable/service/validation.rs:10` — `validate_run_executable_request` takes `&crate::run_executable::RunExecutableRequest` inline in the signature while `RunExecutableValidationError` is already brought in via a `use` two lines above. Mildly inconsistent import style within the same file and different from the sibling modules (`runner.rs`, `command_builder.rs`) which `use` their request type.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Add `use crate::run_executable::RunExecutableRequest;` at the top of `validation.rs` and change the parameter type to `&RunExecutableRequest` to match the rest of the split.

- **[F003]** `src/crosshook-native/crates/crosshook-core/src/run_executable/service/adhoc_prefix.rs:74,87` — `resolve_default_adhoc_prefix_path_from_data_local_dir` and `provision_prefix` are marked `pub(crate)`, but they are only used by `service/runner.rs` (and the in-tree `service/tests.rs`). Both could be tightened to `pub(super)` so the module's crate-internal surface area stays minimal after the split.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Change `pub(crate) fn resolve_default_adhoc_prefix_path_from_data_local_dir` and `pub(crate) fn provision_prefix` to `pub(super) fn …`. Re-run `cargo check -p crosshook-core` to confirm nothing outside `service/` depends on them.

## Validation Results

| Check                | Result                                 |
| -------------------- | -------------------------------------- |
| Format (`cargo fmt`) | Pass                                   |
| Lint (`clippy -D`)   | Pass                                   |
| Tests (`cargo test`) | Pass                                   |
| Host-gateway guard   | Pass                                   |
| Build                | Pass (clippy compiled the crate clean) |

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/run_executable/service.rs` (Deleted)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service/mod.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service/adhoc_prefix.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service/command_builder.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service/runner.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service/validation.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service/tests.rs` (Added)
