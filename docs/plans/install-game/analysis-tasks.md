# Task Analysis: install-game

## Executive Summary

The cleanest task structure is four phases: backend contract foundation, core installer/runtime logic, frontend integration and review flow, then polish/verification. The codebase boundaries are strong enough that several tasks can run in parallel if they stay scoped to distinct files: install-domain models, runtime-helper extraction, and UI shell work do not need to block each other immediately. The highest-risk ordering concern is making sure the final save boundary stays explicit, so executable discovery and review/save wiring should be treated as first-class work rather than as a small detail after installer launch.

## Recommended Phase Structure

- **Phase 1 - Contracts And Scaffolding**: establish install-domain types, Tauri module wiring, and UI shell boundaries.
- **Phase 2 - Runtime And Generation Core**: implement direct `proton run` installer execution, prefix provisioning, and generated-profile assembly.
- **Phase 3 - Review And Editor Integration**: add ranked executable discovery, install-status UI, and final save/review handoff into existing profile state.
- **Phase 4 - Polish And Verification**: warnings, docs, recents if included, and focused Rust/build validation.

## Proposed Task Breakdown

- **Task A: Create install-domain models and module wiring**
  - Files to create: `src/crosshook-native/crates/crosshook-core/src/install/mod.rs`, `src/crosshook-native/crates/crosshook-core/src/install/models.rs`
  - Files to modify: `src/crosshook-native/crates/crosshook-core/src/lib.rs`
  - Purpose: define `InstallGameRequest`, `InstallGameResult`, validation/result enums, and export surface.
  - Dependency boundary: foundational; other backend tasks should depend on it.

- **Task B: Extract reusable direct-Proton runtime helpers**
  - Files to modify: `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - Purpose: isolate environment assembly, working-directory, and log-attachment helpers so install logic reuses current runtime conventions without cloning code.
  - Dependency boundary: independent from Task A at the file level; should finish before core install execution task.

- **Task C: Add Tauri install command scaffolding**
  - Files to create: `src/crosshook-native/src-tauri/src/commands/install.rs`
  - Files to modify: `src/crosshook-native/src-tauri/src/commands/mod.rs`, `src/crosshook-native/src-tauri/src/lib.rs`
  - Purpose: register install IPC endpoints and keep the command surface thin.
  - Dependency boundary: depends on Task A for contract shape, but not on full runtime implementation.

- **Task D: Implement backend install service and prefix resolution**
  - Files to create: `src/crosshook-native/crates/crosshook-core/src/install/service.rs`
  - Files to modify: `src/crosshook-native/crates/crosshook-core/src/install/mod.rs`
  - Purpose: default prefix resolution under `~/.local/share/crosshook/prefixes/<slug>`, install validation, prefix creation, direct `proton run` spawn, and log result generation.
  - Dependency boundary: depends on Tasks A and B.

- **Task E: Implement executable discovery and generated profile assembly**
  - Files to create: `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`
  - Files to modify: `src/crosshook-native/crates/crosshook-core/src/install/service.rs`, `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` only if absolutely necessary
  - Purpose: rank likely runtime executables, exclude obvious installer/uninstaller/updater binaries, and build the reviewable `GameProfile`.
  - Dependency boundary: depends on Task A; can proceed in parallel with late parts of Task D once result shape is stable.

- **Task F: Add frontend install types and hook**
  - Files to create: `src/crosshook-native/src/types/install.ts`, `src/crosshook-native/src/hooks/useInstallGame.ts`
  - Files to modify: none initially
  - Purpose: encode install request/result types and async UI state separate from `useProfile`.
  - Dependency boundary: depends on Task A and should complete before full UI integration.

- **Task G: Add install sub-tab UI shell**
  - Files to create: `src/crosshook-native/src/components/InstallGamePanel.tsx`
  - Files to modify: `src/crosshook-native/src/components/ProfileEditor.tsx`, optionally `src/crosshook-native/src/styles/theme.css`
  - Purpose: mount the new tab, reuse browse controls and detected-Proton selector behavior, and present status/review sections.
  - Dependency boundary: can start after Task F and partial Task C; should not wait on full executable discovery.

- **Task H: Wire review/save handoff into existing profile flow**
  - Files to modify: `src/crosshook-native/src/hooks/useProfile.ts`, `src/crosshook-native/src/components/ProfileEditor.tsx`, `src/crosshook-native/src/App.tsx` if needed
  - Purpose: show ranked executable candidates, let the user confirm the final runtime target, and save only from the final review step.
  - Dependency boundary: depends on Tasks D, E, F, and G.

- **Task I: Add focused Rust tests and optional recent-path integration**
  - Files to modify: `src/crosshook-native/crates/crosshook-core/src/install/*.rs`, `src/crosshook-native/crates/crosshook-core/src/settings/recent.rs` only if installer recents are included
  - Purpose: verify prefix resolution, validation, executable ranking, and generated profile correctness.
  - Dependency boundary: depends on core backend tasks but can run in parallel with late UI polish.

- **Task J: Update docs and task log**
  - Files to modify: `README.md`, `docs/getting-started/quickstart.md`, `docs/features/steam-proton-trainer-launch.doc.md`, `tasks/todo.md`
  - Purpose: document the new install flow and storage path split only after implementation details settle.
  - Dependency boundary: should trail the core feature integration.

## Dependency Notes

- Task A is the primary backend prerequisite because nearly everything else wants stable install request/result shapes.
- Task B is deliberately separated because runtime-helper extraction reduces duplication and de-risks Task D.
- Task H is the main integration bottleneck: it is where backend result shape, UI review state, and final save semantics all converge.
- Task J should stay late to avoid churn in user-facing docs before the UX settles.
- `ProfileEditor.tsx` is the main shared frontend conflict file, so only one UI-heavy task should own it at a time.

## Parallelization Strategy

- **Early parallel work**:
  - Task A and Task B can run together.
  - Task F can start once Task A stabilizes even if backend execution is not complete.
- **Mid-phase parallel work**:
  - Task D and Task E can overlap after models are settled, as long as `service.rs` ownership is coordinated.
  - Task G can start once Task F exists and the command surface is stable enough.
- **Late parallel work**:
  - Task I can begin once backend behavior lands.
  - Task J can begin once save/review UX is no longer moving.

## Risk-First Ordering

- Solve the save-boundary problem before polish: the plan should treat “installer path must not become runtime path” as a primary requirement, not a validation footnote.
- Build runtime helper extraction early so install execution does not fork its own environment semantics.
- Keep direct `proton run` as the only runtime path in v1 to avoid branching the plan around an unselected future compatibility option.
- Defer installer recents and doc polish until the core backend and review flow are stable.

## Suggested Task Granularity

- Keep backend tasks to 1-3 files whenever possible.
- Treat `ProfileEditor.tsx` integration as separate from `useInstallGame.ts` creation so the UI shell and async state logic do not become one oversized task.
- Keep executable discovery in its own backend task/module; it has distinct heuristics, tests, and risk profile from raw installer execution.
- Keep doc updates separate from code tasks so implementation sequencing stays technical first and user-facing wording follows the final behavior.
