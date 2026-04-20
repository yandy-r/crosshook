---
pr: 388
url: https://github.com/yandy-r/crosshook/pull/388
head: eea30a0e05e8844a5d32aa8290e5737e78726d4f
base: main
title: '[WIP] Refactor capability.rs into smaller modules'
author: app/anthropic-code-agent
reviewed_at: 2026-04-19
decision: REQUEST CHANGES
---

# PR #388 review — split `capability.rs` into a module directory

## Worktree Setup

- **Worktree**: `~/.claude-worktrees/crosshook-pr-388/`
- **Base**: `main`
- **Head branch**: `claude/refactor-split-capability-rs` (fetched as local ref `pr-388-head`)
- **Head SHA**: `eea30a0e05e8844a5d32aa8290e5737e78726d4f`
- **Cleanup**: `git worktree remove ~/.claude-worktrees/crosshook-pr-388 && git branch -D pr-388-head`

Reviewers/fix batches spawned by `/ycc:review-fix --worktree` should create child
worktrees off `pr-388-head` (one per severity group).

## Scope

Child of umbrella refactor tracker issue #290. Splits the single 860-line file
`src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` into a
module directory with five focused files:

| File                                           | Lines | Responsibility                                                           |
| ---------------------------------------------- | ----: | ------------------------------------------------------------------------ |
| `onboarding/capability/mod.rs`                 |    26 | Module entry point, re-exports, `derive_capabilities` public wrapper     |
| `onboarding/capability/types.rs`               |    70 | `Capability`, `CapabilityState`, `CapabilityDefinition`, `CapabilityMap` |
| `onboarding/capability/derive.rs`              |    97 | `derive_capabilities_with_map`, `capability_rationale`                   |
| `onboarding/capability/tool_check.rs`          |   163 | `resolve_tool_check`, `synthesize_umu_run_check`, install-hint helpers   |
| `onboarding/capability/formatting.rs`          |    25 | `format_tool_list`, `join_list`                                          |
| `onboarding/capability/tests.rs` _(cfg(test))_ |   507 | Existing tests, moved verbatim                                           |

Diff vs `main`: **+888 / −860**, 7 files changed (the old file is deleted, six
new files added). Three commits on the branch:
`eeab267 Initial plan` → `437424e refactor(core): split capability.rs into smaller modules`
→ `eea30a0 chore: apply format (lint-autofix workflow)`.

Code is moved verbatim — every function body is byte-for-byte identical to
`main`. The only substantive edits are:

- New module boundary doc-comments (one-line `//!` per file).
- Visibility changes: `derive_capabilities_with_map`, `resolve_tool_check`,
  `synthesize_umu_run_check`, `install_hint_for_tool`, `collect_install_hints`,
  `capability_rationale`, `format_tool_list`, and `join_list` went from
  file-private `fn` to `pub(super) fn` so they can cross the sibling-module
  boundary inside `capability/`.
- Imports rearranged across files; `capability/mod.rs` re-exports all four
  public types that `main` exposed from the flat `capability.rs`.

## Validation

All commands run from `~/.claude-worktrees/crosshook-pr-388/` at
`eea30a0e05e8844a5d32aa8290e5737e78726d4f`.

| Check                  | Command                                                                                                       | Result                         |
| ---------------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| Rust fmt               | `cargo fmt --manifest-path src/crosshook-native/Cargo.toml -- --check`                                        | pass                           |
| Clippy (`-D warnings`) | `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --all-targets -- -D warnings` | pass (no warnings)             |
| Rust tests             | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --lib onboarding::capability`   | **pass (12 passed, 0 failed)** |
| Host-gateway           | `./scripts/check-host-gateway.sh`                                                                             | pass                           |
| PR title (CI)          | `.github/workflows/pr-title.yml` — `action-semantic-pull-request`                                             | **FAIL** — see F001            |

The 12 capability tests that pass on `main` still pass here (7 `capability::tests::*`

- 5 `capability_loader::tests::*` filtered by the `onboarding::capability` path
  prefix), confirming behavior parity.

## Findings

### F001 — PR title is a rejected `[WIP]` placeholder and blocks merge (Medium, process) — Open

**File:** PR metadata (title on `https://github.com/yandy-r/crosshook/pull/388`)

The PR title is `[WIP] Refactor capability.rs into smaller modules`. The repo's
`.github/workflows/pr-title.yml:38` pins
`subjectPattern: '^(?!\[WIP\]|\[Draft\]|Initial plan|WIP:|Draft:).+'` on the
`amannn/action-semantic-pull-request` action, so this prefix is explicitly
rejected. The `PR title` check on this branch has been failing on every
`pull_request_target` event (confirmed via
`gh run list --workflow=pr-title.yml` — five `failure` entries on
`claude/refactor-split-capability-rs`). Per `CLAUDE.md § MUST/MUST NOT` and
`.github/copilot-instructions.md`, squash-merges land the PR title verbatim as
the commit subject in `CHANGELOG.md`, so this must be fixed before merge.

Additionally, even after stripping `[WIP]` the remaining `Refactor capability.rs
into smaller modules` is **not** Conventional Commits compliant — no type
prefix. The companion commit on the branch already uses the correct form:
`refactor(core): split capability.rs into smaller modules` (437424e). The recent
`46a2e2e ci: auto-strip placeholder prefixes from PR titles` on `main` only
strips the `[WIP]` / `Draft:` / `Initial plan` prefix; it does not add a type.

**Fix:** update the PR title to match the refactor commit subject:

```
refactor(core): split capability.rs into smaller modules
```

Use GitHub's native Draft PR status (not a title prefix) if this is still WIP.

**Decision impact:** **blocker** — the `PR title` required check is red and
cannot be merged until the title is edited. All code-level findings below are
non-blocking; this is the only reason the decision is REQUEST CHANGES.

### F002 — PR body does not link back to umbrella issue #290 (Medium, process) — Open

**File:** PR description on `https://github.com/yandy-r/crosshook/pull/388`

The originating issue body explicitly requires: _"Link the implementation PR
back to umbrella issue yandy-r/crosshook#290."_ The current PR description is
the default agent template with only the quoted original issue — no `Closes #N`
and no `Part of #290`. Per repo memory rule (`Umbrella issue refs`) and
`CLAUDE.md § MUST/MUST NOT`, child PRs of tracker issues like #290 must use
`Part of #290` (not `Closes`) so the umbrella stays open.

**Fix:** edit the PR body to include at least:

```
Closes #<child-issue-number>
Part of #290
```

Substitute the actual child issue number from the task that spawned this PR
(the issue body quoted in the PR description does not show the issue number, so
pull it from the tracking system).

### F003 — `tests.rs` sits 7 lines over the 500-line soft cap (Low, maintainability) — Open

**File:** `src/crosshook-native/crates/crosshook-core/src/onboarding/capability/tests.rs:1-507`

Per `CLAUDE.md § File size (~500 lines)` the cap is soft, and test fixtures are
explicitly called out as one of the contiguous-content exceptions. However, the
module has a natural split seam along the functions it exercises:

- `derive_capabilities_*` tests (5 tests, lines 143-371)
- `synthesize_umu_run_check_*` tests (2 tests, lines 378-507)

Splitting into `tests/derive.rs` + `tests/umu_synth.rs` (and a `tests/mod.rs`
holding the shared `sample_*` / `tool_check` / `issue` / `readiness_result`
builders) would bring every file well under 500 lines and match the partition
the production code now uses. Not a blocker — flag for follow-up or fold into
this PR if the author wants the test layout to mirror the module layout.

### F004 — `capability/mod.rs` doc comment wording drifts after the split (Low, docs nit) — Open

**File:** `src/crosshook-native/crates/crosshook-core/src/onboarding/capability/mod.rs:1-5`

```rust
//! Host capability map and derived capability state.
//!
//! Struct definitions, [`CapabilityState`], and [`derive_capabilities`] live here.
//! TOML parsing, map loading, and the process-global singleton are in
//! [`super::capability_loader`].
```

The second line is the original wording from the flat `capability.rs` — the
structs and `derive_capabilities` are now re-exported from `mod.rs`, not
declared in it. The statement is technically true from the consumer's
point-of-view (they resolve through `mod.rs`), but it under-describes the new
layout. Suggest:

```rust
//! Host capability map and derived capability state.
//!
//! Re-exports the public surface from submodules:
//! - [`types`] — data types (`Capability`, `CapabilityState`, `CapabilityDefinition`,
//!   `CapabilityMap`).
//! - [`derive`] — [`derive_capabilities`] and rationale synthesis.
//! - [`tool_check`] — host tool resolution and install-hint collection.
//! - [`formatting`] — tool-list formatting helpers.
//!
//! TOML parsing, map loading, and the process-global singleton are in
//! [`super::capability_loader`].
```

Pure documentation polish; no behavioral impact.

## What's solid

- **Verbatim code moves.** Every function body in the four new source files
  matches the corresponding block in `main`'s `capability.rs` byte-for-byte —
  only whitespace and `use` blocks shift. Low regression risk.
- **Public surface preserved.** `mod.rs` re-exports
  `{Capability, CapabilityDefinition, CapabilityMap, CapabilityState}` and
  re-declares `pub fn derive_capabilities(...)` with the same signature and
  body. The two external call sites —
  `onboarding/mod.rs:16` (`pub use capability::{derive_capabilities, Capability, CapabilityMap, CapabilityState}`)
  and `onboarding/capability_loader.rs:12` (`use super::capability::{CapabilityDefinition, CapabilityMap}`)
  — resolve unchanged. No downstream crate edits needed.
- **Visibility scoped correctly.** Cross-sibling helpers are `pub(super)`, not
  `pub(crate)`, so nothing leaks beyond `capability/`. Tests access them via
  `super::derive::...` / `super::tool_check::...`.
- **All acceptance criteria from the child issue are met.**
  - Public APIs preserved ✓
  - Every resulting source file ≤500 lines ✓ (tests.rs at 507 is `#[cfg(test)]`,
    covered by the soft-cap contiguous-fixture carve-out)
  - `cargo test -p crosshook-core` passes ✓ (12/12 capability-scoped tests)
  - `./scripts/lint.sh` equivalents (fmt + clippy + host-gateway) pass ✓
- **Test isolation preserved.** Tests that rely on
  `ScopedCommandSearchPath::new(umu_dir.path())` to pin the live-probe path
  (e.g. `derive_capabilities_all_available_fixture`,
  `derive_capabilities_missing_optional_fixture`) continue to hold the scope
  for their full body — no guard was dropped early in the move.

## Decision

**REQUEST CHANGES** — gated solely on **F001** (PR title blocks the required
`PR title` CI check). The refactor itself is mechanically clean and the
code-level findings (F002–F004) are non-blocking process / docs notes.

The author can unblock merge with a single PR-title edit; no code push is
required for approval.
