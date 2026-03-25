# Task Analysis: proton-optimizations

## Executive Summary

This feature breaks cleanly into four implementation phases: typed model and persistence foundations, backend Proton launch resolution, frontend panel and autosave UX, and final integration/docs/verification. The highest-leverage split is to stabilize the shared data contract first, then let Rust launch work and frontend panel work proceed in parallel on top of that contract. The main sequencing risk is autosave scope: the implementation should not reuse the current full-profile save flow for checkbox toggles, so a section-specific persistence path needs to exist before the panel is wired to save automatically.

## Proposed Phase Structure

### Phase 1

- Goal
  - Establish the typed optimization contract and a narrow persistence path for `launch.optimizations`.
- Why first
  - Both backend launch resolution and frontend autosave depend on a stable shared schema and a non-destructive write path.

### Phase 2

- Goal
  - Add backend launch-directive resolution for `proton_run` and the frontend panel/catalog UI in parallel.
- Why next
  - Once the contract is fixed, Rust launch behavior and React presentation can move independently with limited file overlap.

### Phase 3

- Goal
  - Integrate panel state into the main app flow, finalize autosave/status behavior, and connect the launch request to the new resolver.
- Why next
  - This phase joins the parallel tracks and is where cross-layer edge cases become visible.

### Phase 4

- Goal
  - Complete verification, documentation updates, and plan-closeout cleanup.
- Why next
  - Only after the integration path is stable can tests, manual verification, and docs be updated accurately.

## Proposed Task Breakdown

### Task Group: Shared Model And Catalog Foundation

- Scope
  - Add `launch.optimizations.enabled_option_ids` to the frontend and Rust profile/request models, and introduce a typed frontend option catalog.
- Candidate files
  - `src/crosshook-native/src/types/profile.ts`
  - `src/crosshook-native/src/types/launch.ts`
  - `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
  - `src/crosshook-native/src/types/launch-optimizations.ts`
- Dependencies
  - none
- Parallelizable? yes

### Task Group: Section-Only Optimization Persistence

- Scope
  - Add a dedicated Tauri/Rust persistence path that loads an existing profile, updates only `launch.optimizations`, and writes it back without triggering the current full refresh/reload side effects.
- Candidate files
  - `src/crosshook-native/src-tauri/src/commands/profile.rs`
  - `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`
  - `src/crosshook-native/src/hooks/useProfile.ts`
- Dependencies
  - Shared Model And Catalog Foundation
- Parallelizable? no

### Task Group: Backend Proton Launch Resolver

- Scope
  - Add a backend-owned resolver that validates optimization IDs, turns them into env vars and wrapper prefixes, and applies them to the `proton_run` command builders.
- Candidate files
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`
- Dependencies
  - Shared Model And Catalog Foundation
- Parallelizable? yes

### Task Group: Frontend Panel Shell And Layout

- Scope
  - Create the `LaunchOptimizationsPanel` component, option grouping, advanced disclosure, tooltip affordances, and right-column layout placement beneath `LaunchPanel`.
- Candidate files
  - `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`
  - `src/crosshook-native/src/App.tsx`
  - `src/crosshook-native/src/styles/theme.css`
- Dependencies
  - Shared Model And Catalog Foundation
- Parallelizable? yes

### Task Group: Frontend Autosave And Status UX

- Scope
  - Wire panel selections into profile state, defer autosave for unsaved profiles, debounce optimization-only saves, and surface `Saving...` / `Saved automatically` / error states.
- Candidate files
  - `src/crosshook-native/src/hooks/useProfile.ts`
  - `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`
  - `src/crosshook-native/src/components/ProfileEditor.tsx`
- Dependencies
  - Section-Only Optimization Persistence
  - Frontend Panel Shell And Layout
- Parallelizable? no

### Task Group: Launch Request Integration

- Scope
  - Extend `App.tsx` request construction and launch validation plumbing so `launch_game` and `launch_trainer` receive optimization IDs and the backend resolver is exercised end-to-end.
- Candidate files
  - `src/crosshook-native/src/App.tsx`
  - `src/crosshook-native/src/types/launch.ts`
  - `src/crosshook-native/src-tauri/src/commands/launch.rs`
- Dependencies
  - Shared Model And Catalog Foundation
  - Backend Proton Launch Resolver
- Parallelizable? partially

### Task Group: Rust Test Coverage

- Scope
  - Add unit tests for TOML round-trips, optimization ID validation, wrapper ordering, env injection, and missing/conflicting option behavior.
- Candidate files
  - `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`
- Dependencies
  - Backend Proton Launch Resolver
  - Section-Only Optimization Persistence
- Parallelizable? yes

### Task Group: User-Facing Documentation And Final Verification

- Scope
  - Update user-facing docs to mention the new panel and validate the integrated path with Rust tests, TypeScript checks, and a manual Tauri UI pass.
- Candidate files
  - `README.md`
  - `docs/getting-started/quickstart.md`
  - `docs/features/steam-proton-trainer-launch.doc.md`
  - `tasks/todo.md`
- Dependencies
  - Frontend Autosave And Status UX
  - Launch Request Integration
  - Rust Test Coverage
- Parallelizable? no

## Dependency Notes

- The data contract is the root dependency. Do not let frontend panel work invent IDs or shape independently from the Rust request/profile model.
- The autosave path should land before the panel is wired to persist changes, otherwise the feature will either reload too aggressively or silently persist unrelated edits.
- `App.tsx` is a shared integration hotspot for layout placement and request construction; keep panel-shell work and launch-request work coordinated to avoid duplicate edits colliding late.
- Rust launch resolution is mostly independent from React UI once the shared option IDs are fixed, which makes it the best parallel track after Phase 1.
- Tests should follow the Rust module boundaries rather than trying to backfill a new frontend test harness during this feature.

## Risk-Driven Ordering

- Solve persistence scope before UX polish. A visually complete panel backed by the current full save loop would create the wrong behavior and likely force rework.
- Stabilize wrapper/env resolution before integrating advanced options. The risky part is deterministic launch construction, not the presence of more checkboxes.
- Keep the first shipped catalog conservative. Advanced or community-documented options should ride on the same contract later rather than driving the initial architecture.
- Leave Steam parity out of the task graph. Treating optional Steam follow-up as part of the core plan would deepen dependencies and distract from the direct Proton path the user actually wants.

## Recommended Plan Shape

- Total phases: 4
- Likely task count: 8 to 10 implementation tasks
- Independent tasks to start with:
  - Shared Model And Catalog Foundation
  - Frontend Panel Shell And Layout
  - Backend Proton Launch Resolver
