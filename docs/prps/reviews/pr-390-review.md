# PR Review #390 — refactor: split readiness.rs into smaller modules

**Reviewed**: 2026-04-19
**Mode**: PR
**Author**: app/openai-code-agent
**Branch**: codex/refactor-readiness-rs-into-modules → main
**Decision**: APPROVE

## Worktree Setup

- **Parent**: ~/.claude-worktrees/crosshook-pr-390/ (branch: codex/refactor-readiness-rs-into-modules)
- **Children** (per severity; created by /ycc:review-fix --worktree):
  - LOW → ~/.claude-worktrees/crosshook-pr-390-low/ (branch: feat/pr-390-low)

## Summary

Behavior-preserving split of `onboarding/readiness.rs` (787 lines) into a four-module directory (`mod.rs`, `dismissals.rs`, `host_tools.rs`, `system.rs`) plus `tests.rs`. Public API is preserved via re-exports through `onboarding/mod.rs`; all 1097 crate tests (including 14 readiness-specific tests) pass; clippy and fmt are clean. Only two LOW-severity style nits.

## Findings

### LOW

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness/host_tools.rs:9` — `evaluate_host_tool_checks` is declared `pub(super) fn` but is only called within the same module file (by `check_generalized_readiness` at line 77). No sibling module references it, so the visibility escalation is unnecessary.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Drop `pub(super)` and declare `fn evaluate_host_tool_checks(...)` — keeps the symbol truly private to this file and matches the other internal helper (`home_to_tilde` in `system.rs`).

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness/host_tools.rs:5-6` — Imports use `super::super::distro::detect_host_distro_family` and `super::system::check_system_readiness`. The `super::super::` chain reaches into a sibling module of the parent (`onboarding::distro`) via relative paths, which is fragile if the module tree ever changes. Same pattern in `system.rs:9-10` (`super::super::distro`, `super::super::install_advice`).
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Prefer crate-absolute paths for cross-sibling imports: `use crate::onboarding::distro::detect_host_distro_family;` and `use crate::onboarding::install_advice::build_umu_install_advice;`. Intra-module `super::system::check_system_readiness` in `host_tools.rs` is fine as-is.

## Validation Results

| Check      | Result                                                             |
| ---------- | ------------------------------------------------------------------ |
| Type check | Pass (cargo build -p crosshook-core)                               |
| Lint       | Pass (cargo clippy -p crosshook-core --all-targets -- -D warnings) |
| Tests      | Pass (1097 tests in crosshook-core; 14 readiness tests)            |
| Build      | Pass                                                               |

Additional repo-specific checks:

| Check                             | Result |
| --------------------------------- | ------ |
| `cargo fmt --all -- --check`      | Pass   |
| `./scripts/check-host-gateway.sh` | Pass   |

## Files Reviewed

- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` (Deleted)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness/mod.rs` (Added, 12 lines)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness/dismissals.rs` (Added, 44 lines)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness/host_tools.rs` (Added, 85 lines)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness/system.rs` (Added, 218 lines)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness/tests.rs` (Added, 447 lines)

## Notes

- **File-size rule**: All resulting files are under the 500-line soft cap (largest: `tests.rs` at 447). The acceptance criterion `<=500` is satisfied.
- **Public API preservation**: The five re-exported functions (`check_system_readiness`, `check_generalized_readiness`, `apply_install_nag_dismissal`, `apply_readiness_nag_dismissals`, `apply_steam_deck_caveats_dismissal`) are exposed in `readiness/mod.rs` and the parent `onboarding/mod.rs` is unchanged. Existing callers in `src-tauri/src/commands/onboarding.rs` continue to compile without edits.
- **Test-visibility change**: `evaluate_checks_inner` went from private `fn` to `pub(super) fn` to allow `tests.rs` to call it directly. Same visibility that existed effectively before (tests lived in the same file) — acceptable.
- **Linked umbrella**: Part of #290.
