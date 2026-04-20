# PR Review #395 — refactor: config_history_store.rs into smaller modules

**Reviewed**: 2026-04-19
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-split-config-history-store → main
**Decision**: APPROVE

## Worktree Setup

- **Parent**: ~/.claude-worktrees/crosshook-pr-395/ (branch: codex/refactor-split-config-history-store)
- **Children** (per severity; created by /ycc:review-fix --worktree):
  - LOW → ~/.claude-worktrees/crosshook-pr-395-low/ (branch: feat/pr-395-low)

## Summary

Clean mechanical refactor: `config_history_store.rs` (781 lines) becomes a directory module where the impl stays byte-identical in `mod.rs` (294 lines) and the inline `tests {}` block is split into five topic files under `tests/` with shared helpers in `tests/common.rs`. All 18 original tests preserved with identical names, bodies, and assertions; crate-wide `cargo test` (1097 core + 13 integration), `cargo clippy -D warnings`, `cargo fmt --check`, and `check-host-gateway.sh` all pass. Acceptance criteria from child issue #368 are met (public API preserved, every file ≤500 lines).

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `<PR body>` — PR description lacks the explicit `Part of #290` / `Closes #368` footer required by `CLAUDE.md` (§ MUST / MUST NOT → Pull requests). The bot-generated body embeds the child-issue description inline but has no machine-parseable link line, so the PR will not auto-close #368 nor appear in #290's cross-reference panel on merge.
  - **Status**: Open
  - **Category**: Completeness
  - **Suggested fix**: Edit the PR body to append a short footer such as `Closes #368` and `Part of #290` before marking ready; the rest of the bot-generated body can stay. Consistent with the manually-authored sibling PR #296 which includes `Closes #295` / `Refs #290`.

## Validation Results

| Check      | Result                                                                                                                     |
| ---------- | -------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (`cargo check` succeeds as part of clippy)                                                                            |
| Lint       | Pass (`cargo clippy -p crosshook-core --all-targets -- -D warnings`, `cargo fmt --check`, `scripts/check-host-gateway.sh`) |
| Tests      | Pass (`cargo test -p crosshook-core` — 1097 unit + 3 config_history_integration + 10 other integration tests, 0 failed)    |
| Build      | Pass (test-profile build succeeded in clippy + test compile)                                                               |

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store.rs` (Deleted — flat file replaced by directory module)
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store/mod.rs` (Added — 294 lines; impl byte-identical to the former file's lines 1–292, only change is replacing the inline `mod tests { … }` block with `mod tests;`)
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store/tests/mod.rs` (Added — 5 lines; `mod` declarations only)
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store/tests/common.rs` (Added — 43 lines; shared `open_test_db`, `ensure_profile`, `insert_revision` helpers with `pub(super)` visibility)
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store/tests/insert_and_list.rs` (Added — 141 lines; 7 tests covering insert/list/dedup/lineage)
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store/tests/known_good.rs` (Added — 76 lines; 3 tests covering set/supersede/clear known-good markers)
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store/tests/pruning.rs` (Added — 112 lines; 3 tests covering retention + FK-safe pruning)
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store/tests/validation.rs` (Added — 110 lines; 5 tests covering ownership, oversize rejection, disabled-store defaults, cross-profile lineage rejection)
