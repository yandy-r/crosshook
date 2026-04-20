# PR Review #386 — [WIP] Refactor community_index.rs into smaller modules

**Reviewed**: 2026-04-19T21:20:45-04:00
**Mode**: PR
**Author**: app/anthropic-code-agent
**Branch**: claude/refactor-community-index-into-modules → main
**Decision**: COMMENT

## Worktree Setup

- **Parent**: ~/.claude-worktrees/crosshook-pr-386/ (branch: claude/refactor-community-index-into-modules)
- **Children** (per severity; created by /ycc:review-fix --worktree):
  - LOW → ~/.claude-worktrees/crosshook-pr-386-low/ (branch: feat/pr-386-low)

## Summary

Clean mechanical split of `community_index.rs` (881 lines) into 7 focused submodules, with full behavioral parity — all 12 original tests preserved and passing, clippy/fmt/host-gateway clean, and consumer-facing API (`index_community_tap_result_with_trainers`, `list_community_tap_profiles`) re-exported identically. Only low-severity polish items remain (glob import, `pub` visibility consistency, branch is behind `main` so the diff spuriously shows `pr-title-autofix.yml` as deleted).

## Findings

### CRITICAL

_None._

### HIGH

_None._

### MEDIUM

_None._

### LOW

- **[F001]** `.github/workflows/pr-title-autofix.yml:1` — Spurious "deleted workflow" in PR diff from stale branch
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Rebase the branch onto `main` (`git fetch origin && git rebase origin/main`) and force-push. The branch was cut at `807973d`, before `46a2e2e ci: auto-strip placeholder prefixes from PR titles` landed. The two-dot `git diff main..HEAD` that GitHub renders shows this file as "-68 lines" even though a 3-way merge would not actually delete it (merge base predates the file). Rebasing removes the ambiguity and lets a reviewer see only the intended changes.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/indexing.rs:14` — `pub fn` on items of a private submodule obscures intent
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: The parent `community_index` module is declared `mod community_index;` (private in `metadata/mod.rs:58`), so `pub fn` on `index_community_tap_result_with_trainers` / `index_community_tap_result` (indexing.rs:14, 60), `index_trainer_sources` (trainer_sources.rs:15), and `list_community_tap_profiles` (queries.rs:12) compiles identically to `pub(super)`. Downgrade each `pub fn` that is not re-exported from `mod.rs` to `pub(super) fn` (or `pub(crate)` if you want `MetadataStore` impl blocks outside `metadata/` to call it). Leave `pub fn` only on the two items that `mod.rs` re-exports. This prevents future accidental exposure and matches the intent the re-exports already encode.

- **[F003]** `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/indexing.rs:3` — Glob import hides which helpers are in use
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Replace `use super::helpers::*;` with an explicit list — `use super::helpers::{check_a6_bounds, compatibility_rating_str, get_tap_head_commit, nullable_text};`. Only four items are used, and explicit imports make the module's surface area scannable at a glance (also mirrors what `tests.rs` does with `use super::helpers::check_a6_bounds;`).

- **[F004]** _PR title_ — `[WIP]` placeholder prefix violates repo convention
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Remove the `[WIP]` prefix from the PR title. Per `CLAUDE.md`: "placeholder prefixes like `[WIP]`, `Draft:`, or `Initial plan` are rejected and will block merge". Use GitHub's native Draft PR state (already set) to signal work-in-progress — the title should read the way it will appear in `CHANGELOG.md` after squash. A repository workflow (`.github/workflows/pr-title-autofix.yml` on `main`) now auto-strips these, but because this branch predates that workflow, the fix should be applied manually before (or coincident with) a rebase.

## Validation Results

| Check      | Result                                                                                                                  |
| ---------- | ----------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (`cargo check` implicit in test build)                                                                             |
| Lint       | Pass (`cargo clippy -p crosshook-core --all-targets -- -D warnings`) + host-gateway                                     |
| Tests      | Pass (1097 crosshook-core unit tests + 17 community_index tests + 4 external client + integration binaries, 0 failures) |
| Build      | Pass (compiled as part of tests)                                                                                        |
| Format     | Pass (`cargo fmt -p crosshook-core --check`)                                                                            |

## Files Reviewed

- `.github/workflows/pr-title-autofix.yml` (Diff-only "Deleted" — see F001; merge base predates this file on `main`)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` (Deleted — split into submodules)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/mod.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/constants.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/helpers.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/indexing.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/queries.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/trainer_sources.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index/tests.rs` (Added)

## Notes on behavioral parity

- SQL strings unchanged across the split (verified byte-for-byte against `/tmp/pr386-original.rs`).
- A6 bounds constants, HTTPS-only URL check, and `javascript:` rejection test all carry over.
- Consumer call sites (`community_ops.rs:15`, `community_ops.rs:77`, `src-tauri/src/commands/community.rs:273,306`) still compile and resolve via `mod.rs` re-exports — no external signature changes.
- Every file is well under the 500-line soft cap (largest is `tests.rs` at 334).
