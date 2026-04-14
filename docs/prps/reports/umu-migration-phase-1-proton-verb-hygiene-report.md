# Implementation Report: umu-launcher Migration — Phase 1: PROTON_VERB Hygiene

**Plan**: `docs/prps/plans/completed/umu-migration-phase-1-proton-verb-hygiene.plan.md`
**Date**: 2026-04-14
**Issues**: [#254](https://github.com/yandy-r/crosshook/issues/254) (phase tracker), [#234](https://github.com/yandy-r/crosshook/issues/234) (implementation)

---

## Outcome

All 5 tasks completed (B1: 1.1, 1.2, 1.3, 1.4 in parallel; B2: 2.1 verification). All acceptance criteria satisfied.

---

## Changes Made

### Task 1.1 — `env.rs`

- Appended `"PROTON_VERB"` to `WINE_ENV_VARS_TO_CLEAR` after `PROTON_ENABLE_NVAPI` with trailing comment.
- Bumped `assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 31)` → `32`.
- Added `assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_VERB"))` to `wine_env_vars_match_expected_list`.

### Task 1.2 — `steam-host-trainer-runner.sh`

- Appended `PROTON_VERB` to `unset PROTON_NO_ESYNC PROTON_NO_FSYNC PROTON_ENABLE_NVAPI` line (line 458).
- Satisfies the "Keep in sync with `WINE_ENV_VARS_TO_CLEAR`" contract at line 446.

### Task 1.3 — `preview.rs`

- Added `PROTON_VERB` push in `collect_runtime_proton_environment` using `request.launch_trainer_only` to select `"runinprefix"` (trainer) vs `"waitforexitandrun"` (game). Uses `EnvVarSource::ProtonRuntime`.
- Added test `preview_proton_verb_is_waitforexitandrun_for_game_and_runinprefix_for_trainer` asserting both verbs.

### Task 1.4 — `script_runner.rs`

- `build_proton_game_command`: `env.insert("PROTON_VERB", "waitforexitandrun")` after `GAMEID` insert.
- `build_proton_trainer_command`: `env.insert("PROTON_VERB", "runinprefix")` after `GAMEID` insert.
- `build_flatpak_steam_trainer_command`: One-line comment documenting `PROTON_VERB=runinprefix` via delegation.
- 3 new tests: `proton_game_command_sets_proton_verb_to_waitforexitandrun`, `proton_trainer_command_sets_proton_verb_to_runinprefix`, `flatpak_steam_trainer_command_inherits_proton_verb_runinprefix`.

### Task 2.1 — Verification

- Format drift auto-fixed (`cargo fmt`) in one test call chain in `script_runner.rs`.
- All commands exit 0:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test -p crosshook-core` — **859 tests, 0 failures**
  - `./scripts/lint.sh` — Rust + TS + Shell all clean

---

## Validation Results

| Check                                               | Result         |
| --------------------------------------------------- | -------------- |
| `cargo fmt -- --check`                              | PASS           |
| `cargo clippy -D warnings`                          | PASS           |
| `cargo test -p crosshook-core`                      | PASS (859/859) |
| `./scripts/lint.sh`                                 | PASS           |
| `WINE_ENV_VARS_TO_CLEAR` len == 32                  | PASS           |
| `steam-host-trainer-runner.sh` unsets `PROTON_VERB` | PASS           |
| `PROTON_VERB` only in 4 expected source files       | PASS           |

---

## Acceptance Criteria

- [x] All 5 tasks completed (1.1, 1.2, 1.3, 1.4, 2.1).
- [x] `cargo test -p crosshook-core` fully green (859 tests).
- [x] `./scripts/lint.sh` green.
- [x] `cargo clippy ... -D warnings` green.
- [x] Preview renders `PROTON_VERB` with correct verb for both game and trainer paths.
- [x] `WINE_ENV_VARS_TO_CLEAR` contains `"PROTON_VERB"`; `len() == 32`.
- [x] `steam-host-trainer-runner.sh` unsets `PROTON_VERB` in the sync block.
- [x] Zero observable behavior change under direct Proton (verb is idempotent/well-formed).
- [ ] PRD tracking issue [#234](https://github.com/yandy-r/crosshook/issues/234) updated (`Closes #234`) — pending commit/PR.
- [ ] Phase tracker [#254](https://github.com/yandy-r/crosshook/issues/254) receives a linking comment — pending PR.

---

## Architectural Note

Landing `PROTON_VERB` hygiene before any umu code path activates makes the PR #148 regression architecturally impossible: when Phase 3 branches on `use_umu`, the env map already carries the correct verb per builder. No additional Phase 3 verb work is required.
