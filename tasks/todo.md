# Task Plan

## 2026-03-25 - proton launcher icon parity

- [x] Confirm how launcher metadata is stored and consumed for Steam and Proton profiles.
- [x] Update the profile form so Proton mode exposes launcher export metadata, including `Launcher Icon`.
- [x] Run focused verification for the affected frontend paths.
- [x] Add a short review note with outcome and residual risk.

## Review

- `ProfileFormSections.tsx` now reuses the existing launcher metadata fields for both Steam and Proton profile editing, while still keeping install review mode limited to launch-critical fields.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`
- Residual risk:
  - No graphical Tauri pass was run in this environment, so the exact Proton form spacing and wrap behavior still need a manual UI check at normal app window sizes.

## 2026-03-25 - profile-modal planning workflow

- [x] Review existing install-flow code, planning artifacts, and repo lessons.
- [x] Complete feature research and synthesize `docs/plans/profile-modal/feature-spec.md`.
- [x] Resolve pre-planning decisions for collapsed sections, post-save tab jump, and type ownership.
- [x] Generate and validate `docs/plans/profile-modal/shared.md`.
- [x] Generate analysis artifacts:
  - `docs/plans/profile-modal/analysis-context.md`
  - `docs/plans/profile-modal/analysis-code.md`
  - `docs/plans/profile-modal/analysis-tasks.md`
- [x] Synthesize `docs/plans/profile-modal/parallel-plan.md`.
- [x] Validate plan structure and run path/dependency/completeness checks.
- [x] Add final planning review summary.

## Fixed Decisions

- Keep install-critical fields visible in the modal: Profile Identity, Game, Runner Method, final executable review, Prefix Path, Proton Path, and Trainer when populated.
- Collapse only optional or override-heavy sections: empty Trainer details and Working Directory override.
- Primary save action should explicitly announce the handoff, for example `Save and Open Profile Tab`, and save success should switch to the Profile tab with the saved profile selected.
- Keep `InstallProfileReviewPayload` in `src/crosshook-native/src/types/install.ts`.
- Put modal-local `ProfileReviewSession` in `src/crosshook-native/src/types/profile-review.ts`.

## Review

- Research artifacts and `feature-spec.md` were completed and validated.
- `shared.md` was created and validated with zero warnings.
- `analysis-context.md`, `analysis-code.md`, and `analysis-tasks.md` were created.
- `parallel-plan.md` was created and passed structural validation with zero warnings.
- Plan validation findings were folded back into `parallel-plan.md`:
  - type contract fields are explicit
  - save-selection ownership is unambiguous
  - the install handoff task includes the parent callback change
  - dirty-dismiss confirmation is explicitly in-app
  - controller/focus hardening now depends on the guard task
  - final verification is a first-class task with concrete repo commands
- Final plan stats:
  - 4 phases
  - 11 tasks
  - 4 independent tasks
- Validation result:
  - structural validation passed with zero warnings
  - dependency review passed after serializing `2.2` behind `1.3`
  - task completeness review passed after tightening Tasks `1.1`, `1.2`, `2.2`, `3.2`, and adding `4.4`
- Residual caveat:
  - path validation still notes that some later tasks modify files created by earlier tasks, which is dependency-consistent but not “current branch exists now” clean. The plan is still execution-ready.

## 2026-03-25 - profile-modal final verification

- [x] Frontend build proof completed with `npm run build` in `src/crosshook-native` (`tsc && vite build` passed).
- [x] Dev shell verification path was attempted with `./scripts/dev-native.sh`; Vite started and Cargo compilation began, but the run timed out before the Tauri window could be interacted with in this environment.
- [ ] Manual UI regression checks are still pending in a local graphical session:
  - install success auto-opens the modal when a reviewable draft exists
  - dismiss and manual reopen restore the same draft
  - dirty-dismiss and retry/reset replacement confirmations behave correctly
  - save failure keeps the modal open and preserves edits
  - save success persists, reloads/selects the saved profile, and opens the Profile tab
  - controller, keyboard `Tab`, and `Escape` do not reach background controls
  - modal layout remains usable at `1280x800` with only the body scrolling
- [ ] `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` was not run because this verification task only touched frontend modal workflow behavior and no backend/shared-core contract changes were introduced in scope.
- [ ] Residual risk: focus trapping, controller suppression, and viewport behavior still need a real GUI pass in the Tauri shell before the feature can be considered fully closed out.

## 2026-03-25 - proton-optimizations planning workflow

- [x] Review the feature research, clarified scope, and repo lessons for `proton_run`-only launch optimizations.
- [x] Generate and validate `docs/plans/proton-optimizations/shared.md`.
- [x] Generate analysis artifacts:
  - `docs/plans/proton-optimizations/analysis-context.md`
  - `docs/plans/proton-optimizations/analysis-code.md`
  - `docs/plans/proton-optimizations/analysis-tasks.md`
- [x] Synthesize `docs/plans/proton-optimizations/parallel-plan.md`.
- [x] Validate plan structure and run path/dependency/completeness checks.
- [x] Add final planning review summary.

## Review

- `docs/plans/proton-optimizations/shared.md` was created and validated with zero warnings.
- `analysis-context.md`, `analysis-code.md`, and `analysis-tasks.md` were created from the planning docs and current codebase.
- `parallel-plan.md` was created and passed structural validation with zero warnings.
- Plan validation findings were folded back into `parallel-plan.md`:
  - `Task 2.1` now depends on both contract-definition tasks so option IDs stay aligned between TS and Rust
  - the Rust module-export step for `launch/mod.rs` is explicit
  - `READ THESE BEFORE TASK` sections no longer point at files that only exist after earlier tasks
  - the plan stays strictly `proton_run`-scoped, with Steam parity left as optional future work
- Final plan stats:
  - 4 phases
  - 11 tasks
  - 2 independent tasks
- Validation result:
  - structural validation passed with zero warnings
  - dependency review passed after serializing `2.1` behind both shared-contract tasks
  - path review found only expected “modify later-created file” cases for `launch/optimizations.rs` and `LaunchOptimizationsPanel.tsx`
  - task completeness review passed after tightening the backend module-registration and path-readability details
- Residual caveat:
  - a naive current-branch path checker will still flag future-created files when later tasks modify them, but those references are dependency-consistent and execution-ready within the plan.

## 2026-03-25 - proton-optimizations implementation

- [x] Task 1.1: Define the frontend optimization catalog and TS contracts.
- [x] Task 1.2: Mirror the optimization contract in Rust profile and request models.
- [x] Task 1.3: Add a section-only optimization persistence path.
- [x] Task 2.1: Build the backend optimization resolver and validation rules.
- [x] Task 2.2: Integrate optimization directives into direct Proton launch builders.
- [x] Task 2.3: Add Rust tests for persistence, validation, and command construction.
- [x] Task 3.1: Build the launch optimizations panel UI and theme support.
- [x] Task 3.2: Compose the panel into the app layout and launch request flow.
- [x] Task 3.3: Wire autosave, profile normalization, and panel status feedback.
- [x] Task 4.1: Update user-facing documentation for the new panel.
- [x] Task 4.2: Run final verification and record the planning closeout.

## Implementation Review

- Task 1.1 added a typed `launch-optimizations.ts` catalog with stable option IDs, category labels, applicability metadata, and wrapper-conflict metadata for the `proton_run` path.
- `GameProfile.launch.optimizations` and `LaunchRequest.optimizations` now accept the shared `LaunchOptimizations` payload without forcing current runtime code changes.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`
- Residual risk:
  - The catalog is intentionally broad enough for later UI/backend phases, so the follow-up tasks still need to decide which advanced entries are actually surfaced by default.
- Task 1.2 completed with a Rust-only schema mirror for `launch.optimizations.enabled_option_ids`.
- Added `LaunchOptimizationsSection` to the profile model and `LaunchOptimizationsRequest` to the launch request model with `serde(default)` and compact empty serialization.
- Updated existing struct literals to include the new defaulted field so the crate stays buildable without behavior changes.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passed
- Task 1.3 added `profile_save_launch_optimizations`, a narrow Tauri command that accepts `enabled_option_ids` and delegates to `ProfileStore::save_launch_optimizations`.
- `ProfileStore::save_launch_optimizations` now loads an existing profile, merges only `launch.optimizations`, and writes the full TOML document back without creating a new profile file on missing names.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passed
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run` passed
- Residual risk:
  - The Tauri command is wired and compile-checked, but no runtime IPC call was exercised in this environment.
- Task 2.1 added the backend-owned `launch/optimizations.rs` resolver, expanded request validation for unknown/duplicate/conflicting/method-gated IDs, and centralized the allowed optimization env vars in `launch/env.rs`.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passed
- Task 2.2 now routes both direct Proton builders through `resolve_launch_directives()` and `new_direct_proton_command_with_wrappers()`, so wrapper order and launch-specific env vars are centralized in Rust.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core proton_game_command_applies_optimization_wrappers_and_env` passed
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core proton_trainer_command_applies_optimization_wrappers_and_env` passed
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch_optimization_vars_match_expected_list` passed
- Follow-up:
  - Shared launch-test support now avoids mutating process-wide `PATH`, which stabilized the full `crosshook-core` suite under parallel test execution.
- Task 3.1 added a dedicated `LaunchOptimizationsPanel` with grouped option cards, advanced disclosure, keyboard-focusable info popovers, and method-aware disabled states.
- Added theme classes for the new panel, summary chips, status tones, option metadata pills, and responsive stacking.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`
- Task 3.3 tightened the profile hook autosave baseline so loading or clearing a profile resets the saved optimization snapshot before the effect compares against it.
- The autosave path stays narrow and debounced, and unsaved profiles continue to show the panel-local `Save profile first to enable autosave` warning instead of bubbling a global error.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`
- Task 3.2 already matched the plan in `App.tsx`: the `LaunchOptimizationsPanel` renders beneath `LaunchPanel`, the launch request only forwards optimization IDs for `proton_run`, and native/install contexts do not render the Proton-only panel.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`
- Task 2.3 required no new Rust tests. The existing coverage already includes TOML round-tripping for `launch.optimizations.enabled_option_ids`, validation for unknown/duplicate/conflicting/method-gated IDs, and resolver/command-construction checks.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passed
- Task 4.1 updated the top-level README, quickstart guide, and Steam/Proton workflow doc to describe the `Launch Optimizations` panel, its `proton_run` scope, autosave behavior for existing profiles, and per-option info tooltips.
- Verification:
  - docs-only change; no code-level test run was required beyond the already completed TypeScript and Rust verification for the feature
- Task 4.2 final verification results:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passed
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run` passed
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`
- Remaining manual checks:
  - Tauri UI validation for panel placement, tooltip accessibility, autosave status messaging, unsaved-profile deferral, and live `proton_run` behavior was not run in this headless environment.
- Verification caveat:
  - `git diff --check` still flags indentation because the local Git `core.whitespace` setting requires tab indentation globally, while the existing Rust, TypeScript, CSS, and Markdown files in this repo use spaces. I did not rewrite the feature diff into tabs because that would create large style-only churn unrelated to the feature.

## 2026-03-25 - main layout cleanup

- [x] Move the runtime console into its own top-level tab.
- [x] Move `Launch Optimizations` into the full-width area below the main editor/launcher layout.
- [x] Run focused frontend verification for the updated tab and layout wiring.

## Review

- `App.tsx` now exposes a dedicated `Logs` tab for `ConsoleView`, which removes the console from the `Main` workspace and keeps the runtime log stream available without competing with launch controls.
- The `Launch Optimizations` panel now renders in the full-width row below the main two-column layout, which preserves the right-column space for `LaunchPanel` and `LauncherExport` and gives the option grid much more horizontal room.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`
