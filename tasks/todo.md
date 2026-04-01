# Task Plan

## 2026-03-31 - protondb lookup implementation

- [x] Task 1.1: Define the backend ProtonDB contract and exact-tier model.
- [x] Task 1.2: Implement cache-backed ProtonDB fetch and safe recommendation normalization.
- [x] Task 1.3: Add backend tests for tier mapping, cache fallback, and safe suggestion parsing.
- [x] Task 2.1: Expose ProtonDB lookup as a thin Tauri command.
- [x] Task 2.2: Add frontend ProtonDB types and lookup hook.
- [x] Task 2.3: Extend theme styling for exact ProtonDB tiers and degraded states.
- [x] Task 3.1: Build a dedicated ProtonDB lookup card component.
- [x] Task 3.2: Compose the ProtonDB card into the profile editor flow.
- [x] Task 3.3: Implement explicit copy/apply merge behavior for supported suggestions.
- [x] Task 4.1: Join ProtonDB panel state with version-correlation signals.
- [x] Task 4.2: Add regression coverage for version-aware ProtonDB messaging.
- [x] Task 5.1: Define cross-feature Steam App ID and cache namespace contract.
- [x] Task 5.2: Document explicit integration boundary for issue `#52`.
- [x] Task 6.1: Define promotion eligibility for advisory recommendations.
- [x] Task 6.2: Define preset-candidate persistence and rollback guidance.
- [x] Task 7.1: Update user-facing docs for ProtonDB lookup behavior.
- [x] Task 7.2: Run verification, record manual regression coverage, and add the implementation closeout.

### ProtonDB Version-Aware Verification Scenarios

- Up-to-date version: when the selected profile `version_status` is `matched` or `unknown`, the ProtonDB card should show exact tier + guidance without any stale-build warning and must remain fully advisory.
- Unknown version: when no version snapshot exists, the ProtonDB card should still load and render normally, with no blocking validation or forced retry path.
- Game updated: when `version_status` is `game_updated` or `both_changed`, the ProtonDB card should surface a stale-guidance warning that recommendations may lag the new build, but profile editing, saving, and launch validation must remain non-blocking.
- Update in progress: when `version_status` is `update_in_progress`, the ProtonDB card should warn that Steam is still changing the build and the guidance may not match yet, again without blocking edits or save.

## Implementation Review

- Added a new `crosshook_core::protondb` module with exact-tier DTOs, cache-backed async lookup, summary-first live fetches, stale-cache fallback, and best-effort recommendation aggregation from ProtonDB’s current hashed report-feed path.
- The backend now normalizes advisory data into safe env-var suggestions, copy-only raw launch strings, and grouped notes; unsupported remote launch text never mutates launch behavior directly.
- Added a thin `protondb_lookup` Tauri command and mirrored the DTOs in TypeScript with a dedicated `useProtonDbLookup` hook.
- The Profile editor now renders a `ProtonDB Guidance` card for `steam_applaunch` and `proton_run` profiles, including exact tiers, freshness/source state, refresh, copy actions, safe env-var apply actions, and per-key overwrite confirmation before profile mutation.
- Version correlation now feeds the ProtonDB card so recent game-build changes warn that the community guidance may be stale without blocking edit/save flows.
- Planning and user-facing docs were updated to lock the Steam App ID/cache namespace contract for issue `#52`, define preset-promotion/rollback rules, and document the advisory ProtonDB workflow.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run`
  - `npm exec --yes tsc -- --noEmit` in `src/crosshook-native`
- Manual regression checks still pending in a graphical/native shell:
  - empty Steam App ID shows the neutral ProtonDB state with no lookup request
  - cached-hit and stale-cache fallback states render the expected banners
  - remote timeout/unavailable behavior stays local to the ProtonDB card
  - exact-tier badges and summary metadata render cleanly in the profile editor layout
  - version-correlation warning banners appear only for `game_updated`, `both_changed`, and `update_in_progress`
  - copy/apply flows for normalized env suggestions work end to end, including per-key overwrite confirmation
- Residual risk:
  - ProtonDB’s richer report-feed path is still undocumented. The implementation is summary-first and degrades safely, but if ProtonDB changes its hashed feed contract, recommendation groups may fall back to the advisory unavailable state until the resolver is updated.
  - No live Tauri/manual UI pass was run here, so the remaining open risk is presentation/interaction polish in the real shell rather than type or compile correctness.

## 2026-03-27 - post-launch failure diagnostics

- [x] Task 1.1: Define diagnostic data models.
- [x] Task 1.2: Implement exit code analysis.
- [x] Task 1.3: Capture `ExitStatus` in `stream_log_lines`.
- [x] Task 2.1: Implement failure pattern catalog and scanner.
- [x] Task 2.2: Implement `safe_read_tail` and `sanitize_display_path`.
- [x] Task 2.3: Implement `analyze()` public API and launch module export.
- [x] Task 3.1: Wire `analyze()` into `stream_log_lines`.
- [x] Task 3.2: Wire `analyze()` into the CLI.
- [x] Task 4.1: Define TypeScript diagnostic types.
- [x] Task 4.2: Extend `useLaunchState` with diagnostic events.
- [x] Task 4.3: Render the diagnostic banner in `LaunchPanel`.
- [x] Run targeted Rust and TypeScript verification.
- [x] Add a short implementation review with outcome and residual risk.

### Implementation Review

- Added a new `crosshook_core::launch::diagnostics` module with typed models, exit-status analysis, log-pattern scanning, and a public `analyze()` API that produces a single `DiagnosticReport`.
- `src-tauri/src/commands/launch.rs` now captures child exit status, reads a bounded log tail after the final drain, runs diagnostics, sanitizes user-visible paths, emits `launch-diagnostic`, and then emits `launch-complete` in that order.
- `crosshook-cli` now runs the same diagnostics analysis after the helper exits and prints the summary plus matched patterns to stderr.
- The frontend now has mirrored diagnostic types, persistent `launch-diagnostic` / `launch-complete` listeners in `useLaunchState`, and a severity-aware `LaunchPanel` banner with collapsed details, expanded suggestions, and copy-report support.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-cli --no-run`
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run`
  - `npm exec --yes tsc -- --noEmit` in `src/crosshook-native`
- Residual risk:
  - No live Tauri/manual runtime pass was run here, so the remaining check is real launch behavior: confirm `launch-diagnostic` arrives before `launch-complete`, that the banner copy flow works in the shell, and that representative Proton/Steam failures produce the expected patterns and sanitized text.

## 2026-03-26 - route content scroll reset

- [x] Confirm which layout element owns page scroll state during route changes.
- [x] Reset content scroll position when switching between top-level pages without disturbing console scroll.
- [x] Run focused frontend verification.
- [x] Add a short implementation review with outcome and residual risk.

### Implementation Review

- The top-level route shell in `ContentArea.tsx` was reusing the same `Tabs.Content` node across route changes, which let the shared scrollable `.crosshook-content-area` keep its prior `scrollTop`.
- `Tabs.Content` is now keyed by the active route, and `ContentArea` also explicitly scrolls the active `.crosshook-content-area` back to the top on every route change.
- The fix is scoped to the main content area and does not touch the console drawer scroll behavior.
- Verification:
  - `node_modules/.bin/tsc --noEmit` in `src/crosshook-native`
- Residual risk:
  - I did not run a live graphical pass here, so the remaining check is manual confirmation that route switches now start at the top without introducing any unexpected page-state resets beyond the intended scroll reset.

## 2026-03-26 - route auto-focus scroll correction

- [x] Confirm whether controller-mode route auto-focus is re-introducing scroll on the Launch page.
- [x] Keep controller focus handoff on route changes without allowing the auto-focus step to scroll the page.
- [x] Run focused frontend verification.
- [x] Add a short implementation review with outcome and residual risk.

### Implementation Review

- The remaining `Profiles -> Launch` jump was not stale page scroll state. In controller mode, `useGamepadNav` was auto-focusing the first content control after a route change and allowing that focus handoff to call `scrollIntoView`.
- On the Launch page, the first focusable control sits low enough that this auto-focus could pull the page down on first entry even after the content wrapper was remounted.
- The route-change auto-focus now keeps the same content-focus handoff for controller navigation but does it with `preventScroll`, so the page stays at the top.
- Verification:
  - `node_modules/.bin/tsc --noEmit` in `src/crosshook-native`
- Residual risk:
  - I still did not run a live graphical/controller pass here, so the remaining manual check is that controller navigation still lands in page content correctly while route changes no longer scroll the Launch page downward.

## 2026-03-26 - shared route scroll state hardening

- [x] Re-check the route shell for any ineffective remount/reset wiring.
- [x] Clear shared inertial scroll state when switching top-level tabs.
- [x] Run focused frontend verification.
- [x] Add a short implementation review with outcome and residual risk.

### Implementation Review

- The earlier route-remount change had a real bug: `key` was placed inside a spread object passed to `Tabs.Content`, and React ignores `key` in spread props. `ContentArea.tsx` now applies `key={route}` directly on each `Tabs.Content`, so the route panel can actually remount.
- `ContentArea.tsx` still explicitly scrolls the active `.crosshook-content-area` to the top on every route change.
- `useScrollEnhance.ts` was also holding shared inertial scroll state (`activeContainer` plus velocity) across the whole shell. It now cancels momentum when a route tab is pressed and drops stale containers if they disconnect.
- Verification:
  - `node_modules/.bin/tsc --noEmit` in `src/crosshook-native`
- Residual risk:
  - The remaining manual check is in the real Tauri shell at a small viewport. If any scroll leak still exists after this, it is likely coming from a late browser/WebKit focus behavior rather than the shared React shell state.

## 2026-03-26 - actionable launch validation errors

- [x] Add structured launch validation metadata in `crosshook-core` with per-error help text and severity.
- [x] Update the Tauri launch validation command to return structured validation issues.
- [x] Replace string-only launch errors in the frontend with preflight validation feedback and a severity-aware LaunchPanel surface.
- [x] Run focused Rust and TypeScript verification.
- [x] Add a short implementation review with outcome and residual risk.

### Implementation Review

- `ValidationError` now exposes a serializable `LaunchValidationIssue` with `message`, `help`, and `severity`, and every current launch validation variant provides actionable fix guidance while remaining `fatal` in the existing fail-fast flow.
- The Tauri `validate_launch` command now returns the structured validation payload, while `launch_game` and `launch_trainer` keep their runtime string-error behavior unchanged.
- `useLaunchState` now performs a typed preflight validation before either launch step. Validation failures stop the launch and surface structured guidance; runtime spawn/build failures still appear as generic launch errors.
- `LaunchPanel` now renders a severity-aware feedback card with the validation message and help text instead of a single raw error line.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run`
  - `node_modules/.bin/tsc --noEmit` in `src/crosshook-native`
- Residual risk:
  - No graphical Tauri/manual UI pass was run here, so the new feedback card still needs an in-app check for spacing, tone, and phase behavior during real launch failures.

## 2026-03-26 - trainer source-mode support

- [x] Add a persisted trainer loading mode to shared Rust/TypeScript profile and launch contracts.
- [x] Update Steam and Proton trainer launch paths to support `source_directory` and `copy_to_prefix`.
- [x] Update exported standalone trainer launchers to honor the same loading mode.
- [x] Expose the loading mode in the profile editor and propagate it into launch/export requests.
- [x] Run focused Rust and TypeScript verification.
- [x] Add a short implementation review with outcome and residual risk.

### Implementation Review

- Profiles now persist `trainer.loading_mode` with `source_directory` as the default, and launch/export requests carry the same choice through Rust, Tauri, the CLI, and the frontend types.
- `proton_run` trainer launches no longer stage by default. In `source_directory` mode, CrossHook runs the canonical trainer path directly and keeps the working directory anchored to the trainer bundle. In `copy_to_prefix` mode, the previous staging behavior remains intact.
- Steam helper scripts now accept `--trainer-loading-mode`, skip compatdata staging for source-directory launches, and still support explicit copy mode for compatibility edge cases.
- Exported standalone trainer launchers now honor the same loading mode as the profile, and launcher staleness checks compare generated script content so a mode change is detected even when the launcher name stays the same.
- The profile editor now exposes `Trainer Loading Mode` with `Run from current directory` and `Copy into prefix`, and the export panel surfaces the selected mode in its metadata summary.
- Verification:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run`
  - `npm exec --yes tsc -- --noEmit` in `src/crosshook-native`
- Residual risk:
  - Real Proton/Steam runtime validation for Aurora and other bundle-based trainers still needs a manual end-to-end pass in a graphical session.
  - `git diff --check` still reports `indent with spaces` because the local Git whitespace configuration treats this repo’s normal space indentation as an error; I did not rewrite the feature diff to tabs.

## 2026-03-25 - proton launcher icon parity

- [x] Confirm how launcher metadata is stored and consumed for Steam and Proton profiles.
- [x] Update the profile form so Proton mode exposes launcher export metadata, including `Launcher Icon`.
- [x] Run focused verification for the affected frontend paths.
- [x] Add a short review note with outcome and residual risk.

### Review

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

### Review

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

### Implementation Review

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

### Review

- `App.tsx` now exposes a dedicated `Logs` tab for `ConsoleView`, which removes the console from the `Main` workspace and keeps the runtime log stream available without competing with launch controls.
- The `Launch Optimizations` panel now renders in the full-width row below the main two-column layout, which preserves the right-column space for `LaunchPanel` and `LauncherExport` and gives the option grid much more horizontal room.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`

## 2026-03-25 - optimization conflict feedback

- [x] Add a frontend conflict matrix for mutually exclusive launch optimizations.
- [x] Block incompatible selections immediately in the optimization panel instead of waiting for launch-time validation.
- [x] Run focused frontend verification for the new conflict feedback path.

### Review

- The launch-optimization contract now exposes a conflict matrix helper so the UI and hook use the same mutually-exclusive option relationships as the panel metadata.
- The panel now disables blocked options, labels them with the active blocker, and renders conflict warnings inside each affected option group instead of only at the top of the panel.
- `useProfile.ts` now guards toggle attempts and surfaces a panel-local warning state instead of letting a conflicting selection persist until launch.
- Verification:
  - `npm exec --yes tsc -- --noEmit` passed in `src/crosshook-native`

## 2026-03-25 - ui enhancements tracking issue

- [x] Review `docs/plans/ui-enhancements/` and closed issues `#24` and `#28` for issue structure, scope, and labels.
- [x] Draft a tracking issue body that matches the existing planning-driven feature issue format.
- [x] Create the GitHub issue with the selected labels and record the result here.

### Review

- Created [Issue #33](https://github.com/yandy-r/crosshook/issues/33): `feat: Sidebar navigation, dedicated views, and persistent console drawer`.
- The issue body follows the same planning-driven structure used by `#24` and `#28`: summary, planning artifacts, architecture, critical path, key design decisions, planned phases, success criteria, and validation expectations.
- Applied labels:
  - `type:feature`
  - `area:ui`
  - `area:profiles`
  - `platform:steam-deck`
  - `platform:linux`
  - `platform:proton`
  - `priority:medium`
  - `feat:platform-native-ui`

## 2026-03-25 - ui enhancements implementation

- [x] Validate `docs/plans/ui-enhancements/parallel-plan.md` prerequisites and rebuild the real dependency batches.
- [x] Batch 1: complete Tasks `1.1`, `1.3`, and `1.4`.
- [x] Batch 2: complete Task `1.2`.
- [x] Batch 3: complete Tasks `1.5`, `1.6`, and `1.7`.
- [x] Batch 4: complete Task `1.8`.
- [x] Batch 5: complete Tasks `2.1`, `2.3`, `2.4`, `2.5`, `2.6`, `2.7`, and `2.8`.
- [x] Batch 6: complete Task `2.2`.
- [x] Batch 7: complete Tasks `2.9` and `2.10`.
- [x] Batch 8: complete Tasks `3.1`, `3.2`, `3.3`, `3.4`, `3.5`, `3.6`, `3.7`, `3.8`, and `3.9`.
- [x] Batch 9: complete Task `3.10`.
- [x] Run integration verification, repair any cross-task conflicts, and write the closeout review.

### Review

- The horizontal tab shell was replaced with a provider-backed vertical navigation layout using `Sidebar`, `ContentArea`, `ConsoleDrawer`, and a persistent controller prompt bar.
- Profile, launch, install, community, compatibility, and settings flows now live in dedicated page shells under `src/crosshook-native/src/components/pages/`, and the old `ProfileEditor.tsx` composition was removed.
- Shared frontend state now lives in `ProfileContext` and `PreferencesContext`, with Steam-path helpers moved into `src/crosshook-native/src/utils/steam.ts`.
- Console, launch, export, settings, community, compatibility, and auto-populate surfaces were migrated away from component-local inline styles to class-driven styling in `theme.css` and the new layout CSS files.
- Gamepad navigation now understands sidebar/content focus zones, remembers focus per zone, supports left/right zone switching, and cycles sidebar views with `LB` / `RB`.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed
- Residual risk:
  - No manual Tauri or controller session was run here, so focus behavior, drawer ergonomics, responsive collapse, and install-review save handoff still need a real GUI pass.
  - `git diff --check` still reports whitespace failures because the local Git whitespace configuration expects tabs globally while this repo’s frontend files use spaces; I did not normalize the feature diff into tabs.

## 2026-03-25 - ui shell layout regression

- [x] Trace the collapsed shell layout against the current app wrapper and panel-group composition.
- [x] Fix the shell container so the resizable panel group spans the full app viewport instead of a single grid cell.
- [x] Re-run frontend verification and record the correction.

### Review

- The regression came from wrapping the panel-group shell in a two-column CSS grid without explicitly spanning the child across the grid, which collapsed the entire app into the first grid track.
- `layout.css` now treats `.crosshook-app-layout` as a flex wrapper and forces nested panel groups to fill the available width and height, which restores the intended sidebar + content layout.
- `Sidebar.tsx` now also marks the sidebar with `data-crosshook-focus-zone="sidebar"` so the newer gamepad zoning has an explicit shell anchor.

## 2026-03-25 - ui shell resize correction

- [x] Inspect the current shell against the resized screenshot and trace why the panes still do not resize correctly.
- [x] Replace the shell separators with explicit resize handles, loosen the sidebar/content constraints, and let the log drawer honor the panel height instead of a fixed internal height.
- [x] Re-run frontend verification and record the correction.

### Review

- The second regression came from treating `Separator` as a resize handle without giving it explicit sizing/styling, keeping the sidebar width too tightly capped, and letting the console drawer body keep a fixed internal height that fought the vertical panel size.
- `App.tsx` now uses explicit shell panel classes and resize-handle classes, the sidebar can grow wider, the content pane has a real minimum size, and the log panel now uses panel sizing rather than disappearing below a fixed drawer body.
- `layout.css` now defines visible vertical and horizontal resize handles plus bounded content width, and `ContentArea.tsx` keeps the content focus zone marker without re-wrapping the page shells.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed

## 2026-03-25 - ui shell sizing correction

- [x] Inspect the follow-up screenshot and trace why the app still reads as edge-to-edge, why the sidebar stays collapsed-looking, and why log resizing still behaves like the shell height is unbounded.
- [x] Restore a centered bounded shell width, lock the shell to viewport height for true internal resizing, and relax the sidebar collapse threshold so desktop/tablet widths stay expanded by default.
- [x] Re-run frontend verification and record the correction.

### Review

- The remaining issues came from three shell-sizing mistakes: the app shell no longer respected the previous centered max-width, it used `min-height` instead of a fixed viewport-bounded height so vertical resizing pushed content offscreen, and the sidebar auto-collapse threshold was too aggressive for normal desktop/tablet layouts.
- `layout.css` now restores a centered `var(--crosshook-content-width)` shell, gives the shell a fixed viewport-bounded height, and keeps the content area internally scrollable instead of letting the whole shell grow vertically.
- `App.tsx` now loosens the panel min/max constraints so the sidebar can actually grow and the main pane can shrink further, while `Sidebar.tsx` only auto-collapses at much smaller widths.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed

## 2026-03-25 - ui shell breakpoint adjustment

- [x] Inspect the latest screenshot and verify whether the sidebar is still collapsing too early and whether the centered shell is still too narrow for the current form density.
- [x] Widen the centered shell and push the sidebar collapse breakpoint down so normal app widths keep the full sidebar visible.
- [x] Re-run frontend verification and record the correction.

### Review

- The latest screenshot showed the shell was directionally better but still using an over-aggressive sidebar collapse breakpoint and a content width that was too close to the older compact layout for the denser form rows.
- `variables.css` now widens `--crosshook-content-width` to `1440px`, and the sidebar collapse behavior only kicks in below `560px` instead of collapsing at ordinary app widths.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed

## 2026-03-25 - ui shell no-collapse decision

- [x] Remove automatic sidebar collapse from the current shell.
- [x] Cap the sidebar resize range at about 40% of the shell width.
- [x] Keep the current widened centered shell and re-run frontend verification.

### Review

- The final correction turns the sidebar behavior into an explicit product decision instead of a guessed breakpoint: the sidebar now stays expanded by default, and no responsive auto-collapse logic remains in the current shell.
- `App.tsx` now limits the sidebar to roughly `14%`-`40%` with a `20%` default, and `Sidebar.tsx` no longer manages `matchMedia` collapse state.
- `variables.css` keeps the current `1440px` centered shell width without any automatic sidebar-width collapse override.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed

## 2026-03-25 - ui shell collapse removal follow-through

- [x] Verify the current shell code actually reflects the no-auto-collapse decision rather than assuming the last visual complaint was covered.
- [x] Remove the remaining sidebar collapse state and keep the sidebar resize range aligned with the agreed `~40%` max.
- [x] Re-run frontend verification and record the follow-through.

### Review

- The missing follow-through was that the previous visual complaint was still hitting live auto-collapse behavior; the current correction removes that behavior entirely instead of just adjusting its threshold.
- `Sidebar.tsx` now always renders the expanded sidebar, `App.tsx` keeps the agreed `20%` default / `40%` max sidebar range, and `variables.css` no longer includes any automatic sidebar-width collapse override.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed

## 2026-03-25 - ui shell panel sizing bug

- [x] Verify the actual panel-library size semantics instead of continuing to infer them from screenshots.
- [x] Replace the shell panel numeric size props with explicit percentage strings for sidebar/content/log sizing.
- [x] Re-run frontend verification and record the correction.

### Review

- The real resize bug was that `react-resizable-panels` treats numeric `defaultSize` / `minSize` / `maxSize` values as pixels, not percentages. The sidebar and content/log panes were therefore being constrained to tens of pixels instead of percentage-based shell space.
- `App.tsx` now uses explicit percentage strings for the resizable shell panels, while the log drawer `collapsedSize` stays pixel-based to match the visible handle height.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed

## 2026-03-25 - launch optimization panels restored

- [x] Trace whether the Proton launch-optimizations panel and Steam launch-options command UI were deleted or merely dropped from the render tree.
- [x] Reattach both surfaces to the dedicated `Launch` page using the existing profile-context contract.
- [x] Re-run frontend verification and record the correction.

### Review

- The optimization features were not removed from the codebase; they were orphaned during the shell/page refactor because the new `LaunchPage` only rendered `LaunchPanel`.
- `LaunchPage.tsx` now renders `LaunchOptimizationsPanel` for `proton_run` and `steam_applaunch` profiles, and renders `SteamLaunchOptionsPanel` beneath it for `steam_applaunch`.
- Verification:
  - `cd src/crosshook-native && npm exec --yes tsc -- --noEmit` passed
  - `cd src/crosshook-native && npm run build` passed
- Task `3.5` completed: `CommunityBrowser` and `CompatibilityViewer` now use class-based layouts and rating badges instead of inline style objects, and the frontend TypeScript check passed after the change.
