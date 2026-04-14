# PR Review #231 — feat: auto-derive trainer gamescope config

**Reviewed**: 2026-04-14
**Mode**: PR (parallel: 3 reviewers — correctness, security, quality)
**Author**: yandy-r
**Branch**: feat/229-auto-trainer-gamescope → main
**Head commit**: b7f792d706a505e728f23a60297b44cbefa2c81a
**Decision**: APPROVE with comments

## Summary

Correctly replaces `effective_trainer_gamescope()` with `resolved_trainer_gamescope()` across launch, preview, export, and UI paths, and aligns the trainer Gamescope tab with the new derivation. Core logic, tests, and validations are solid; findings are non-blocking maintainability and test-coverage improvements, plus one stale lesson entry referencing the removed API.

## Findings

### CRITICAL

_(none)_

### HIGH

- **[F001]** `tasks/lessons.md:8` — Stale lesson references the deleted method name: "use `effective_trainer_gamescope()`…". That API is removed by this PR; the lesson now points to a non-existent symbol and contradicts the two new lessons at lines 5–6. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Replace the occurrence of `effective_trainer_gamescope()` in line 8 with `resolved_trainer_gamescope()` (and keep the rest of the lesson intact).

### MEDIUM

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:322` — `build_trainer_command` (non-Flatpak Steam trainer-only helper path) no longer emits `CROSSHOOK_TRAINER_BUILTIN_ENV_KEYS` / `CROSSHOOK_TRAINER_CUSTOM_ENV_KEYS`, but the downstream runner `runtime-helpers/steam-host-trainer-runner.sh:441-442` still captures them via `capture_preserved_trainer_env`. The runner handles missing/empty vars gracefully, but the behavioral change is silent and undocumented, which will trip future debugging of env inheritance under `steam_applaunch` trainer-only runs. [correctness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add a short comment above `build_trainer_command` explaining that the trainer-only path intentionally drops optimization env (per CLAUDE.md trainer execution parity rule) and that downstream helper scripts must handle missing `CROSSHOOK_TRAINER_*_ENV_KEYS` as empty; link to issue #229. Consider also emitting empty `CROSSHOOK_TRAINER_BUILTIN_ENV_KEYS=""` explicitly to keep the env contract stable for the runner.
- **[F003]** `src/crosshook-native/crates/crosshook-core/src/launch/request.rs:119` — `LaunchRequest::resolved_trainer_gamescope` and `LaunchSection::resolved_trainer_gamescope` (profile/models.rs:404) duplicate the same three-branch resolution algorithm (enabled override → windowed clone of game config → default). Any future rule change (e.g. also zero `output_*` dims, or respect `allow_nested`) must be applied in lockstep across both, with no compile-time coupling to enforce it. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Extract a `pub(crate) fn resolve_trainer_gamescope(game: &GamescopeConfig, trainer_override: Option<&GamescopeConfig>) -> GamescopeConfig` into a shared module (e.g. `crates/crosshook-core/src/launch/gamescope.rs` or `mod.rs`) and delegate both `impl` methods to it.
- **[F004]** `src/crosshook-native/src/components/ProfileSubTabs.tsx:50` — `resolveTrainerGamescopeForDisplay` re-implements the Rust `resolved_trainer_gamescope` algorithm in TypeScript without a parity-required comment. Per CLAUDE.md, business logic lives in `crosshook-core`; the UI mirror is pragmatic but undocumented, so the next contributor has no signal that a backend change must be mirrored here. [quality]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Add a leading comment: `// Must mirror LaunchRequest::resolved_trainer_gamescope / LaunchSection::resolved_trainer_gamescope in crosshook-core — update both sites together.`
- **[F005]** `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` test `falls_back_to_main_gamescope_when_trainer_disabled` — updated test sets `request.trainer_gamescope = Some(GamescopeConfig::default())` (explicit disabled override), which only exercises the "disabled explicit override" branch of `resolved_trainer_gamescope`. The `trainer_gamescope = None` auto-derive branch has no preview-level coverage. [correctness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add a second preview variant with `trainer_gamescope = None` and `gamescope.enabled = true`, asserting the trainer-only preview contains the windowed auto-derived flags (`-W`/`-H` present, `-f` absent).
- **[F006]** `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` tests — no parity test confirms `LaunchSection::resolved_trainer_gamescope` and `LaunchRequest::resolved_trainer_gamescope` return equal results from logically equivalent inputs. Given the duplication in F003, this is the cheapest guardrail against future drift. [correctness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add a parity test that constructs equivalent `LaunchSection` and `LaunchRequest` inputs (same `gamescope` + same `trainer_gamescope` content) and asserts `launch_section.resolved_trainer_gamescope() == request.resolved_trainer_gamescope()` across all three branches (explicit enabled, disabled-override auto-derive, disabled-game default).
- **[F007]** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:242` — `build_helper_command` calls `request.resolved_trainer_gamescope()` at line 242 and again inside `helper_arguments` at line 667, cloning the full `GamescopeConfig` (including `extra_args: Vec<String>`) twice per launch. Similarly, `effective_gamescope_config` now returns owned instead of `&GamescopeConfig`, so any validation+build pair clones more than before. Small impact in absolute terms, but avoidable. [security]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Resolve once at the top of `build_helper_command`/`build_trainer_command` and pass the resolved value into `helper_arguments`/`trainer_arguments` as a parameter. Or consider returning `Cow<'_, GamescopeConfig>` from `resolved_trainer_gamescope` so the explicit-override and disabled-default branches borrow and only the auto-derive branch owns.
- **[F008]** `docs/prps/reports/auto-trainer-gamescope-report.md:12` — Report lists "Files Changed: 8" but the PR actually touches 11 files (source: 8, plus `docs/prps/plans/completed/auto-trainer-gamescope.plan.md`, `docs/prps/reports/auto-trainer-gamescope-report.md`, and `tasks/lessons.md`). Makes the post-implementation artifact inaccurate. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Update the row to "Files Changed: 11" with a note (e.g., "8 production code + 3 docs/lessons") or split the Files Changed table to include the non-code files.

### LOW

- **[F009]** `src/crosshook-native/src/components/GamescopeConfigPanel.tsx:117-121` — `derivedConfigNotice` renders with `role="note"` (correctly informational) but reuses the `crosshook-warning-banner` CSS class, which in the adjacent session-warning branch (line 111) is used for genuine warnings. Informational copy will appear visually alarming. [quality]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Introduce a `crosshook-info-banner` (or `crosshook-notice-banner`) style in `styles/` and apply it here; keep `crosshook-warning-banner` reserved for `role="alert"` warnings. If no info-banner pattern exists today, at minimum add inline styling to visually differentiate this notice.
- **[F010]** `src/crosshook-native/src/components/ProfileSubTabs.tsx:274-277` — UI notice text is 189 chars and slightly redundant ("currently generated"); "saving will create a trainer-specific override" is ambiguous because the tab itself is part of the profile form. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Trim to: `Trainer gamescope is auto-generated from the game config. Edit any value here and save the profile to create a trainer-specific override.`
- **[F011]** `src/crosshook-native/src/components/ProfileSubTabs.tsx:54-76` — `resolveTrainerGamescopeForDisplay` uses `?.enabled` on `trainer_gamescope` / `gamescope` even though TS declares them non-optional. Either the type is actually optional (and the TS decl is wrong) or the optional chaining is dead code. [correctness]
  - **Status**: Fixed
  - **Category**: Type Safety
  - **Suggested fix**: Align the runtime usage with the declared types. If legacy TOML profiles can omit these sections (per prior lessons.md guidance), adjust the TS types to reflect the optional nature; otherwise drop the `?.`.
- **[F012]** `src/crosshook-native/src/components/ProfileSubTabs.tsx:112` — `resolveTrainerGamescopeForDisplay` runs on every render of `ProfileSubTabs` (all tab switches, all unrelated state changes). Cheap, but not memoized. [security]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Wrap with `const trainerGamescopeDisplay = useMemo(() => resolveTrainerGamescopeForDisplay(profile), [profile]);`.

## Validation Results

| Check      | Result                                                                                                                                             |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (included in `./scripts/lint.sh`: `tsc` ran clean on touched files)                                                                           |
| Lint       | Pass (`./scripts/lint.sh` exit 0; 105 pre-existing Biome warnings, none in this PR's touched files; Clippy `-D warnings` pass on `crosshook-core`) |
| Tests      | Pass (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — all tests green)                                            |
| Build      | Skipped (PR report states `vite build` passed locally; AppImage build not re-run)                                                                  |

## Files Reviewed

- `docs/prps/plans/completed/auto-trainer-gamescope.plan.md` (Added)
- `docs/prps/reports/auto-trainer-gamescope-report.md` (Added)
- `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/export.rs` (Modified)
- `src/crosshook-native/src/components/GamescopeConfigPanel.tsx` (Modified)
- `src/crosshook-native/src/components/ProfileSubTabs.tsx` (Modified)
- `tasks/lessons.md` (Modified)

## Reviewer notes

- The correctness- and security-reviewers each flagged a HIGH around env-key regressions (`CROSSHOOK_TRAINER_BUILTIN_ENV_KEYS`) and a "partial removal" of optimization env in `build_helper_command`. After verifying call sites (`src-tauri/src/commands/launch.rs:314-329,447-472`), `build_helper_command` is only used for the combined game+trainer helper path; the `launch_trainer_only` flows route through `build_trainer_command`, `build_flatpak_steam_trainer_command` → `build_proton_trainer_command`, or `build_proton_trainer_command`. The optimization-env retention in `build_helper_command` is therefore correct for the game-launch contract. The remaining concern — silent behavioral drift versus the shell helper — was downgraded to F002 (MEDIUM).
- Parallel reviewer agent IDs for traceability: correctness `ac91ee10498d9b880`, security `ae5863914a73d4397`, quality `a6a79c6a11926eef6`.
