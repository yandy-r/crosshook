# PR Review #387 — refactor(settings): split settings module

**Reviewed**: 2026-04-19T21:21:13-04:00
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-settings-file → main
**Decision**: COMMENT

## Worktree Setup

- **Parent**: ~/.claude-worktrees/crosshook-pr-387/ (branch: codex/refactor-split-settings-file)
- **Children** (per severity; created by `/ycc:review-fix --worktree`):
  - MEDIUM → ~/.claude-worktrees/crosshook-pr-387-medium/ (branch: feat/pr-387-medium)
  - LOW → ~/.claude-worktrees/crosshook-pr-387-low/ (branch: feat/pr-387-low)

## Summary

Clean, faithful refactor that splits the 877-line `settings/mod.rs` into four focused modules
(`paths.rs`, `store.rs`, `types.rs`, `tests.rs`) plus an 18-line facade `mod.rs`. Every public
symbol is preserved via explicit `pub use` re-exports, all 22 existing settings tests migrate
unchanged, full `crosshook-core` test suite passes (1110/1110), `cargo clippy --all-targets -D warnings`
is clean, and `scripts/check-host-gateway.sh` still passes (`host_std_command("getent")` path
correctly preserved in `paths.rs`). Decision is `COMMENT` because the PR is still a Draft —
code is ready to merge once the draft is lifted, modulo the issue-link gap noted in F001.

## Findings

### MEDIUM

- **[F001]** `:0` — PR body does not explicitly link the parent issue (`Closes #361`) or the tracker umbrella (`Part of #290`)
  - **Status**: Open
  - **Category**: Completeness
  - **Suggested fix**: Edit the PR description to add `Closes #361` and `Part of #290` so the merge event auto-closes the child issue and leaves a traceable reference on the umbrella. The child issue's acceptance criteria explicitly requires: "Link the implementation PR back to umbrella issue yandy-r/crosshook#290." Repo policy in `CLAUDE.md` states: "Always link the related issue (`Closes #…`, or `Part of #…` for child PRs of tracker issues)."

### LOW

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:2` — `pub mod recent;` is declared before the three private `mod` declarations, creating a minor ordering inconsistency
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Group module declarations together — either put all `mod` / `pub mod` lines consecutively, or order by visibility (private first, then public). Cosmetic only; no behavioral impact.

## Validation Results

| Check      | Result                                                                                                            |
| ---------- | ----------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (implicit via `cargo check` during test build)                                                               |
| Lint       | Pass (`cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --all-targets -D warnings`) |
| Tests      | Pass (1110 passed; 0 failed across `crosshook-core` — including 26 `settings::*` tests)                           |
| Build      | Pass (build is a prerequisite of the test run)                                                                    |

Additional checks:

- `scripts/check-host-gateway.sh` → pass (no direct host-tool bypasses introduced)
- File-size soft cap (CLAUDE.md ~500-line rule) → all split files ≤ 500 lines:
  - `mod.rs` 18
  - `paths.rs` 93
  - `store.rs` 152
  - `types.rs` 252
  - `tests.rs` 378

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (Modified, 877 → 18 lines; now a facade)
- `src/crosshook-native/crates/crosshook-core/src/settings/paths.rs` (Added, 93 lines)
- `src/crosshook-native/crates/crosshook-core/src/settings/store.rs` (Added, 152 lines)
- `src/crosshook-native/crates/crosshook-core/src/settings/types.rs` (Added, 252 lines)
- `src/crosshook-native/crates/crosshook-core/src/settings/tests.rs` (Added, 378 lines)

## Notes for reviewers

- Public API surface verified identical before/after:
  - `settings::{RECENT_FILES_LIMIT_MIN, RECENT_FILES_LIMIT_MAX, clamp_recent_files_limit}` (constants + fn)
  - `settings::{AppSettingsData, UmuPreference}` (types)
  - `settings::{SettingsStore, SettingsStoreError}` (store)
  - `settings::resolve_profiles_directory_from_config` (public helper)
  - `settings::expand_path_with_tilde` (crate-private via `pub(crate) use`)
  - `settings::recent::{RecentFilesData, RecentFilesStore, RecentFilesStoreError}` (unchanged submodule)
- `AppSettingsData` field list, `Default` impl, and manual `Debug` impl (redacting `steamgriddb_api_key`) are byte-for-byte identical to the pre-split version.
- All 22 test fns in `tests.rs` match the pre-split `mod tests {...}` block by name and body (only indentation changed — moved from nested inline module to a top-level `#[cfg(test)] mod tests;` file).
- No new dependencies, no behavior changes, no persistence changes (as the parent issue required).
