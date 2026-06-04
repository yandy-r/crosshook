# Implementation Report: Profile schema — pre/post launch hooks (`LaunchHook`, `HookStage`)

- **Plan**: [`github-issue-468-launch-hooks-schema.plan.md`](../plans/completed/github-issue-468-launch-hooks-schema.plan.md)
- **GitHub Issue**: [#468](https://github.com/yandy-r/crosshook/issues/468) (tracker: #478; downstream consumer: #471)
- **Follow-up issue created**: [#482](https://github.com/yandy-r/crosshook/issues/482) — Runtime execution of pre/post launch hooks
- **Branch**: `feat/468-launch-hooks-schema`
- **Date**: 2026-06-03
- **Execution mode**: parallel sub-agents (`--parallel --no-worktree`), 4 batches / 7 tasks, max width 3

## Summary

Added `LaunchHook { id, name, path, stage, enabled }` and `HookStage::{PreLaunch, PostExit}` (kebab-case wire format) to `crosshook-core/src/profile/models/hooks.rs`, plus two top-level `GameProfile` fields — `pre_launch_hooks` / `post_exit_hooks` — with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. Round-trip TOML tests lock the contract; ts-rs exports the types to `src/types/generated/launch_hooks.ts`; the hand-written `GameProfile` TS interface and normalizer gained matching optional arrays. Community-exchange export strips hooks and import force-disables them. No launcher consumption — `TODO(hooks-runtime)` breadcrumbs reference #482.

## Tasks Completed

| Batch | Task                                                       | Outcome                                                                                                                                                                                                                                                                                      |
| ----- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1    | 1.1 `models/hooks.rs` + module wiring                      | New module; re-exported via `models/mod.rs` and `profile/mod.rs`                                                                                                                                                                                                                             |
| B1    | 1.2 Follow-up GitHub issue                                 | **#482** created with labels `type:feature`, `area:launch`, `priority:medium`, `feat:hero-detail-consolidation`; Storage boundary + Persistence & usability subsections present                                                                                                              |
| B2    | 2.1 `GameProfile` fields + ripple sites + TODO breadcrumbs | Fields appended last; merge-audit note added at `effective_profile_with`; 7 exhaustive-literal sites fixed (`legacy.rs`, 3 fixture files, `exchange/mod.rs`, `install/models.rs`, `metadata/test_support.rs`); struct-update sites needed no change; launcher breadcrumb in `proton_game.rs` |
| B3    | 3.1 Round-trip + compat tests                              | 6 new tests in `models/tests/hooks.rs` — round-trip equality, empty-omission, legacy-default, unknown-variant rejection, stage default, malformed-hook tolerance                                                                                                                             |
| B3    | 3.2 Exchange hardening                                     | Export clears both vecs (denylist-invariant comment added); import force-disables hooks; 2 new tests (`community_export_strips_launch_hooks`, `community_import_force_disables_launch_hooks`)                                                                                                |
| B3    | 3.3 ts-rs registration + regeneration                      | `export_launch_hooks()` registered in `ts_rs_exports.rs`; `generated/launch_hooks.ts` emitted + Biome-formatted (`HookStage = 'pre-launch' \| 'post-exit'`; `LaunchHook` snake_case fields)                                                                                                  |
| B4    | 4.1 Frontend type mirror                                   | `profile.ts`: funnel re-export of `HookStage`/`LaunchHook`, optional `GameProfile` fields, deep-copy defaults in `normalizeSerializedGameProfile`                                                                                                                                            |

## Validation Results

All five levels pass.

| Level | Command                                                                        | Result                                                       |
| ----- | ------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| 1     | `cargo fmt --all --check`                                                      | PASS                                                         |
| 1     | `cargo clippy --all-targets -- -D warnings`                                    | PASS                                                         |
| 1     | `cargo clippy -p crosshook-core --features ts-rs --all-targets -- -D warnings` | PASS (pre-existing ts-rs parse notes only)                   |
| 1     | `./scripts/lint.sh --rust`                                                     | PASS                                                         |
| 2     | `cargo test -p crosshook-core`                                                 | PASS — 10 suites ok, 0 failures (1178 lib tests incl. 8 new) |
| 2     | `cargo test -p crosshook-core --features ts-rs`                                | PASS                                                         |
| 3     | `ts_rs_export` regen + Biome format + `git diff --exit-code generated/`        | PASS — no drift                                              |
| 3     | `npm run typecheck`                                                            | PASS                                                         |
| 3     | `npm test`                                                                     | PASS — 37 files / 205 tests                                  |
| 4     | `cargo build --workspace`                                                      | PASS                                                         |
| 4     | Launcher scope-guard grep (non-TODO hook refs in `launch/`)                    | 0 — PASS                                                     |
| 5     | `cargo test -p crosshook-core hooks`                                           | 8 passed                                                     |
| 5     | `cargo test -p crosshook-core exchange`                                        | 15 passed (incl. 2 new)                                      |
| —     | `./scripts/check-host-gateway.sh`                                              | PASS                                                         |

## Acceptance Criteria

- [x] AC1 — `cargo test -p crosshook-core` green
- [x] AC2 — `LaunchHook` + `HookStage` in `generated/launch_hooks.ts`, re-exported from the `types` barrel
- [x] AC3 — old profiles (no hook keys) deserialize with empty vecs (`legacy_profile_without_hook_keys_defaults_to_empty`)
- [x] AC4 — empty vecs emit no keys (`empty_hook_vecs_omitted_from_toml`)
- [x] AC5 — two-hook round-trip equality with kebab-case stage values (`launch_hooks_two_each_toml_roundtrip`)
- [x] AC6 — zero hook references in `launch/` beyond the TODO breadcrumb
- [x] Open question resolved: single struct + stage enum; top-level field home (plan, Resolved Decisions)
- [x] Community export strips hooks; community import force-disables them
- [x] Hooks-runtime follow-up issue (#482) exists with persistence subsections; both TODO comments reference it

## Deviations from Plan

- The plan listed ~12 ripple sites; 5 of them (`metadata/profile_sync.rs`, `collections_jtbd_integration.rs`, three `src-tauri` test files) already use `..GameProfile::default()` struct-update syntax and required no edit.
- `cargo test --features ts-rs` writes a transient `crates/crosshook-core/bindings/` directory (ts-rs default output); it is not a deliverable and was removed. Consider gitignoring it in a future hygiene pass.

## Storage Boundary (as shipped)

- `pre_launch_hooks` / `post_exit_hooks` and all `LaunchHook` fields: **TOML settings** (per-profile `profile.toml`, `[[pre_launch_hooks]]` / `[[post_exit_hooks]]` array-of-tables).
- **No SQLite change** — metadata DB stays at schema v23.
- **No new runtime-only state.**
- Migration: additive only; old profiles load with empty vecs; new-but-empty profiles serialize byte-identically to old ones; old builds skip unknown keys.
