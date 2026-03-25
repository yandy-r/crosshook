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
