# PR Review #227 — fix(launch): align Steam trainer with proton_run and Flatpak parity

**Reviewed**: 2026-04-13T15:05:00-04:00
**Mode**: PR
**Author**: yandy-r
**Branch**: fix/steam-trainer-launch → main
**Decision**: REQUEST CHANGES

## Summary

The trainer-parity goal is directionally correct, but this PR is not merge-ready. The branch fails required validation on the PR head, introduces two high-severity Flatpak host-boundary regressions in trainer env handling, and still leaves the combined Steam helper path out of full trainer-gamescope parity.

## Findings

### CRITICAL

None.

### HIGH

- **[F001]** `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh:110` — Flatpak trainer launches now replay preserved optimization and custom env keys through `flatpak-spawn --env=...`, which exposes user-supplied trainer env values in host process argv (`ps`) instead of keeping them in a `0600` env-file path. The same leak exists in `src/crosshook-native/runtime-helpers/steam-launch-helper.sh:111`. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Keep user `custom_env_vars` on the existing temp env-file path and only replay a vetted built-in optimization allowlist through `flatpak-spawn --env`, or reload preserved custom vars from a protected host-side file instead of argv.

- **[F002]** `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh:96` — The new Flatpak host-env replay logic forwards wildcard `LD_*`, `PROTON_*`, `DXVK_*`, and `VKD3D_*` variables from the sandbox session into `flatpak-spawn --host`, broadening the host contract well beyond the curated `proton_run` path and allowing sandbox-side `LD_PRELOAD` / `LD_LIBRARY_PATH` contamination of host `gamescope` / Proton execution. The same allowlist exists in `src/crosshook-native/runtime-helpers/steam-launch-helper.sh:97`, and `steam-launch-trainer.sh` now preserves the full session env instead of `env -i`. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Replace the wildcard pass-through with a narrow allowlist derived from resolved launch directives plus explicit session/display keys, and never forward generic `LD_*` across the Flatpak host boundary.

- **[F003]** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:603` — `helper_arguments()` now forwards `runtime.working_directory`, but it still does not forward trainer-gamescope flags for the normal Steam “launch game + trainer” helper flow. As a result, `effective_trainer_gamescope()` is honored for trainer-only Steam launches and exported launchers, but silently ignored when the game and trainer are launched together. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Mirror the `trainer_arguments()` gamescope block in `helper_arguments()`, extend `steam-launch-helper.sh` to parse those flags and wrap its trainer `proton run` with gamescope like `steam-host-trainer-runner.sh`, and add a regression test for the full Steam helper path.

- **[F004]** `src/crosshook-native/src/components/InstallGamePanel.tsx:33` — The new `Object.hasOwn(...)` call is not supported by the current TypeScript target, so the PR head fails both `./scripts/lint.sh` and `./scripts/build-native.sh --binary-only` with `TS2550`. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Revert this call to `Object.prototype.hasOwnProperty.call(...)` or raise the project lib/target in a separate, validated tooling change.

### MEDIUM

- **[F005]** `src/crosshook-native/src-tauri/src/commands/launch.rs:292` — The repo rule says trainer subprocesses should be analyzed by their actual runtime path, but the combined Steam helper flow still records and diagnoses Flatpak game+trainer launches as `steam_applaunch` even when the trainer leg is now executed via direct Proton semantics elsewhere in the PR. That leaves mixed game+trainer failures outside the new parity rule. [correctness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Split or annotate trainer-side tracking/diagnostics in the combined helper path so the Flatpak trainer leg is recorded/analyzed as `proton_run` too, and add coverage for the mixed game+trainer case.

- **[F006]** `tasks/todo.md:1` — This PR deletes the repo’s active task tracker while also bundling unrelated hook/autofix/docs/cache-cleanup churn, which conflicts with the repo’s `tasks/todo.md` planning convention and makes the launch-parity change materially harder to review in isolation. [quality]
  - **Status**: Failed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Restore `tasks/todo.md` in this branch and split the non-launch hook/docs/autofix work into separate `chore(...)` or `docs(internal): ...` changes outside PR #227.

- **[F007]** `.github/workflows/lint-autofix.yml:43` — The new autofix workflow formats the entire repository and commits `git add -A`, so a formatting bot run can rewrite unrelated history artifacts like `CHANGELOG.md`, archived PRP reports, and review files. This branch already shows that churn, which expands merge-conflict surface and pollutes feature PRs with unrelated formatting noise. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Restrict the autofix commit to files changed in the PR, or explicitly exclude generated/history artifacts such as `CHANGELOG.md`, archived reports, and review outputs.

- **[F008]** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:75` — The Flatpak trainer env contract is now duplicated across Rust and two shell helpers (`script_runner.rs`, `steam-host-trainer-runner.sh`, and `steam-launch-helper.sh`), which is a drift risk on a parity-sensitive path. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Centralize the allowlist/preserved-env contract in one shared helper or generated source instead of hand-maintaining parallel copies across Rust and shell.

### LOW

- **[F009]** `.claude/PRPs/reports/ui-standardization-phase-4-report.md:93` — The formatting churn in this PR damaged archived Markdown (`snake*case`, `\\_message strings*`), which is minor but concrete evidence that the current autofix scope mutates historical docs rather than just active source files. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Revert the malformed Markdown in archived/internal report files and keep those paths out of automated autofix scope.

## Validation Results

| Check      | Result                                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Fail (`./scripts/lint.sh` and `./scripts/build-native.sh --binary-only` both fail with `TS2550` at `src/components/InstallGamePanel.tsx:33`) |
| Lint       | Fail (`./scripts/lint.sh`)                                                                                                                   |
| Tests      | Pass (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`)                                                        |
| Build      | Fail (`./scripts/build-native.sh --binary-only`)                                                                                             |

## Files Reviewed

- `.claude/PRPs/reports/ui-standardization-phase-4-report.md` (Modified)
- `.cursorrules` (Modified)
- `.github/pull_request_template.md` (Modified)
- `.github/workflows/lint-autofix.yml` (Added)
- `AGENTS.md` (Modified)
- `CHANGELOG.md` (Modified)
- `CLAUDE.md` (Modified)
- `CONTRIBUTING.md` (Modified)
- `docs/internal-docs/local-build-publish.md` (Modified)
- `docs/prps/prds/flatpak-distribution.prd.md` (Modified)
- `docs/prps/reports/flatpak-phase-3-process-execution-hardening-report.md` (Modified)
- `docs/prps/reviews/fixes/pr-214-fixes.md` (Modified)
- `lefthook.yml` (Modified)
- `scripts/dev-native.sh` (Modified)
- `scripts/setup-dev-hooks.sh` (Added)
- `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` (Modified)
- `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh` (Modified)
- `src/crosshook-native/runtime-helpers/steam-launch-helper.sh` (Modified)
- `src/crosshook-native/runtime-helpers/steam-launch-trainer.sh` (Modified)
- `src/crosshook-native/src-tauri/src/commands/export.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/launch.rs` (Modified)
- `src/crosshook-native/src/components/InstallGamePanel.tsx` (Modified)
- `src/crosshook-native/src/components/pages/LaunchPage.tsx` (Modified)
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx` (Modified)
- `tasks/lessons.md` (Modified)
- `tasks/todo.md` (Deleted)
