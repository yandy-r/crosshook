# Proton Launch Optimizations Implementation Plan

`proton-optimizations` should be implemented as a `proton_run`-only feature that adds a typed `launch.optimizations` contract, a section-only autosave path, a backend launch resolver, and a new right-column UI panel with accessible per-option info tooltips. The safest sequencing is to stabilize the shared option ID catalog and profile/request schema first, then let backend launch work and frontend panel work proceed in parallel on top of that contract. The major technical risk is not UI rendering; it is preserving launch determinism and avoiding the current full-profile save/reload path for checkbox autosave. The implementation should therefore keep React responsible for human-friendly option selection and save-state feedback, while Rust owns validation, wrapper ordering, env injection, and final `proton run` command construction.

## Critically Relevant Files and Documentation

- /docs/plans/proton-optimizations/feature-spec.md: Source of truth for scope, option catalog, autosave rules, tooltip requirements, and success criteria.
- /docs/plans/proton-optimizations/shared.md: Condensed architecture, relevant files, patterns, and must-read docs.
- /docs/plans/proton-optimizations/analysis-context.md: Cross-cutting concerns, constraints, and parallelization guidance.
- /docs/plans/proton-optimizations/analysis-code.md: Code-level patterns, file impact, and integration seams.
- /docs/plans/proton-optimizations/analysis-tasks.md: Recommended phase structure and task-group dependencies.
- /src/crosshook-native/src/App.tsx: Right-column layout composition and `LaunchRequest` construction.
- /src/crosshook-native/src/components/LaunchPanel.tsx: Presentational launch card that anchors the new optimization panel.
- /src/crosshook-native/src/hooks/useProfile.ts: Existing profile mutation/save path that must gain narrow autosave support.
- /src/crosshook-native/src/types/profile.ts: Frontend `GameProfile` contract to extend with `launch.optimizations`.
- /src/crosshook-native/src/types/launch.ts: Frontend `LaunchRequest` contract to extend with optimization IDs.
- /src/crosshook-native/src-tauri/src/lib.rs: Tauri command registration point for any new profile persistence command.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Thin IPC layer and correct home for section-only optimization persistence.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Launch validation/spawn boundary for the richer request.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Rust TOML profile model mirrored to the frontend.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: Profile persistence layer and round-trip test home.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Rust request validation and method gating surface.
- /src/crosshook-native/crates/crosshook-core/src/launch/mod.rs: Launch module export surface that must expose any new resolver module.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Direct Proton command builders where wrappers/env vars must be applied.
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Shared env reconstruction helpers.
- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs: Existing env isolation invariants and test pattern.
- /src/crosshook-native/src/styles/theme.css: Shared styling surface for the new panel, tooltips, disclosures, and save-state feedback.
- /docs/features/steam-proton-trainer-launch.doc.md: Existing user-facing launch behavior doc that will need updating after the feature ships.
- /docs/getting-started/quickstart.md: Current saved-profile workflow guide that will need the new panel described.

## Implementation Plan

### Phase 1: Typed Contract and Persistence Foundation

#### Task 1.1: Define the frontend optimization catalog and TS contracts Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/proton-optimizations/feature-spec.md
- /docs/plans/proton-optimizations/shared.md
- /src/crosshook-native/src/types/profile.ts
- /src/crosshook-native/src/types/launch.ts

**Instructions**

Files to Create

- /src/crosshook-native/src/types/launch-optimizations.ts

Files to Modify

- /src/crosshook-native/src/types/profile.ts
- /src/crosshook-native/src/types/launch.ts

- Add a stable frontend option catalog keyed by saved option IDs, not by env var names or labels.
- Include the user-facing label, short description, tooltip/help copy, category, advanced/community flags, applicability, and any conflict metadata needed by the panel.
- Extend the TypeScript `GameProfile` shape with `launch.optimizations.enabled_option_ids` and extend `LaunchRequest` with the same optimization ID payload.
- Keep the v1 catalog conservative and `proton_run`-only. Steam support stays out of the contract except as future-facing metadata if truly needed.
- Expected outcome: React has a single typed source for option IDs, labels, tooltips, and request/profile shape.

#### Task 1.2: Mirror the optimization contract in Rust profile and request models Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/proton-optimizations/feature-spec.md
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- /src/crosshook-native/src/types/profile.ts

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs

- Add `LaunchOptimizationsSection` to the Rust profile model with `serde(default)` and `skip_serializing_if` so older profiles still load cleanly and empty optimization sections stay compact.
- Extend the Rust `LaunchRequest` contract with optimization IDs in the same shape as the frontend request.
- Add only the structural model changes in this task; deeper resolver logic belongs later.
- Keep the Rust and TypeScript shapes aligned exactly so Tauri serialization remains predictable.
- Expected outcome: TOML persistence and IPC can carry optimization IDs without ad hoc translation.

#### Task 1.3: Add a section-only optimization persistence path Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- /docs/plans/proton-optimizations/feature-spec.md
- /src/crosshook-native/src-tauri/src/commands/profile.rs
- /src/crosshook-native/src-tauri/src/lib.rs
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src-tauri/src/commands/profile.rs
- /src/crosshook-native/src-tauri/src/lib.rs
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs

- Add a focused command such as `profile_save_launch_optimizations` that loads an existing profile, updates only `launch.optimizations`, and writes the merged TOML document back to disk.
- Keep this path restricted to existing saved profiles so checkbox toggles do not silently create new profile files.
- Implement the merge/write behavior close to `ProfileStore` rather than embedding file-manipulation logic directly in the Tauri handler.
- Register the new command in `src-tauri/src/lib.rs` without disturbing unrelated profile commands.
- Expected outcome: the app has a narrow persistence seam that avoids the existing full-profile save/reload side effects.

### Phase 2: Backend Launch Resolution

#### Task 2.1: Build the backend optimization resolver and validation rules Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- /docs/plans/proton-optimizations/feature-spec.md
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/mod.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/launch/mod.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs

- Create a backend-owned resolver that maps stable option IDs to env vars and wrapper directives for `proton_run`.
- Extend `ValidationError` and request validation to reject unknown IDs, incompatible combinations, and method-incompatible usage early.
- Keep advanced/community options represented in the resolver even if the UI hides some of them behind later phases; the resolver should be the single authority for semantics.
- Preserve env isolation discipline by treating optimization env vars as explicit launch directives, not as passthrough shell state.
- Expected outcome: Rust owns option meaning, applicability, and failure messages before any process is spawned.

#### Task 2.2: Integrate optimization directives into direct Proton launch builders Depends on [2.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs
- /src/crosshook-native/src-tauri/src/commands/launch.rs
- /docs/plans/proton-optimizations/analysis-code.md

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs

- Apply resolved env vars and wrapper ordering to both `build_proton_game_command()` and `build_proton_trainer_command()` without changing native or Steam behavior.
- Keep wrapper ordering deterministic and centralized. Do not let the frontend assemble command strings.
- Add any small helper necessary to compose wrappers around the existing `proton run ...` command while preserving logging and working-directory behavior.
- Update Rust-side env invariants only where the direct Proton path genuinely needs it, and preserve the existing isolation model documented in `env.rs`.
- Expected outcome: `proton_run` launches honor the saved optimization IDs end-to-end.

#### Task 2.3: Add Rust tests for persistence, validation, and command construction Depends on [1.3, 2.1, 2.2]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs
- /docs/plans/proton-optimizations/feature-spec.md

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs

- Add unit tests for TOML round-tripping of `launch.optimizations.enabled_option_ids`.
- Add validation tests for unknown IDs, duplicate IDs, incompatible wrapper combinations, and method gating.
- Add resolver-focused tests that verify env var emission and wrapper ordering for representative `proton_run` cases.
- Keep tests colocated with the Rust modules that own the behavior rather than inventing a new test harness.
- Expected outcome: the feature’s risky persistence and launch semantics are covered by fast Rust tests.

### Phase 3: Frontend Panel and Autosave UX

#### Task 3.1: Build the launch optimizations panel UI and theme support Depends on [1.1]

**READ THESE BEFORE TASK**

- /docs/plans/proton-optimizations/research-ux.md
- /src/crosshook-native/src/App.tsx
- /src/crosshook-native/src/components/LaunchPanel.tsx
- /src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Create

- /src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx

Files to Modify

- /src/crosshook-native/src/styles/theme.css

- Create a dedicated panel component beneath the existing launch card rather than adding more logic to `LaunchPanel.tsx`.
- Implement grouped checkboxes for the v1 option set, an advanced disclosure for non-default items, and a small summary/status row.
- Add a keyboard-focusable info (`i`) affordance for every visible option with accessible tooltip/popover content describing what it does, when it helps, and its main caveat.
- Keep the panel presentational and prop-driven: no IPC calls or command-string generation inside the component.
- Expected outcome: the UI shell, tooltip affordances, and styling exist independently of autosave and launch integration.

#### Task 3.2: Compose the panel into the app layout and launch request flow Depends on [1.1, 2.2, 3.1, 3.3]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/App.tsx
- /src/crosshook-native/src/components/LaunchPanel.tsx
- /docs/plans/proton-optimizations/shared.md

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/App.tsx

- Place the new panel beneath `LaunchPanel` in the right-column stack, preserving the existing `LauncherExport` layout logic.
- Extend `launchRequest` construction so the selected optimization IDs are included whenever the effective method is `proton_run`.
- Keep `LaunchPanel.tsx` presentational; the composition and request-building logic should remain in `App.tsx`.
- Ensure native profiles do not render or feed Proton-only optimization data into the request path.
- Expected outcome: the panel is visible in the correct place and the launch path receives optimization IDs.

#### Task 3.3: Wire autosave, profile normalization, and panel status feedback Depends on [1.3, 3.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useProfile.ts
- /src/crosshook-native/src/components/ProfileEditor.tsx
- /tasks/lessons.md

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/hooks/useProfile.ts
- /src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx
- /src/crosshook-native/src/components/ProfileEditor.tsx

- Add the missing `launch.optimizations` defaulting and normalization in `useProfile.ts` so `createEmptyProfile()`, `normalizeProfileForEdit()`, and `normalizeProfileForSave()` always expose a stable optimization shape before the panel is mounted into `App.tsx`.
- Add a narrow autosave helper in `useProfile.ts` that calls the new section-only persistence command with debounce and returns save-state feedback suitable for the panel.
- Do not reuse `persistProfileDraft()` for checkbox autosave; keep unrelated dirty profile fields out of this write path.
- Gate autosave with the existing saved-profile signals in `useProfile.ts` such as `profileExists` and the current name, and defer autosave entirely for brand-new unsaved profiles with a clear panel message such as `Save profile first to enable autosave`.
- Surface panel-local `Saving...`, `Saved automatically`, and failure states without pushing errors into a global banner.
- Expected outcome: the panel can mutate local state immediately, older profiles load safely, and only optimization changes persist when the profile is eligible.

### Phase 4: Docs and Verification

#### Task 4.1: Update user-facing documentation for the new panel Depends on [3.2, 3.3]

**READ THESE BEFORE TASK**

- /README.md
- /docs/getting-started/quickstart.md
- /docs/features/steam-proton-trainer-launch.doc.md
- /docs/plans/proton-optimizations/feature-spec.md

**Instructions**

Files to Create

- None.

Files to Modify

- /README.md
- /docs/getting-started/quickstart.md
- /docs/features/steam-proton-trainer-launch.doc.md

- Update the docs to describe the new `Launch Optimizations` panel, its `proton_run` scope, autosave behavior for existing profiles, and the presence of per-option info tooltips.
- Avoid describing Steam parity as shipped behavior; keep the docs aligned with the actual required scope.
- Keep the v1 option catalog description conservative and note that some advanced options are gated or experimental.
- Expected outcome: user-facing docs no longer imply manual raw-variable editing as the main workflow.

#### Task 4.2: Run final verification and record the planning closeout Depends on [2.3, 3.3, 4.1]

**READ THESE BEFORE TASK**

- /docs/plans/proton-optimizations/feature-spec.md
- /tasks/todo.md
- /src/crosshook-native/src/hooks/useProfile.ts
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs

**Instructions**

Files to Create

- None.

Files to Modify

- /tasks/todo.md

- Run the concrete verification pass for the implemented feature:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
  - `npm exec --yes tsc -- --noEmit` in `src/crosshook-native`
  - manual Tauri UI checks for panel placement, tooltip accessibility, autosave states, unsaved-profile deferral, advanced disclosure, and `proton_run` launch behavior
- Record results, residual risks, and any deferred manual checks in `tasks/todo.md`.
- Verify that wrapper/tool failures surface actionable validation messages and do not produce silent no-op launches.
- Expected outcome: the feature has an explicit verification record and the task log reflects the final state.

## Advice

- Keep the option ID catalog as the single source of truth for both UI copy and backend semantics. If IDs drift between React and Rust, the rest of the plan becomes fragile.
- Do not route checkbox autosave through the current full-profile save loop. That helper is correct for manual Save, but it is the wrong behavior for a narrow launch-settings panel.
- Keep `LaunchPanel.tsx` presentational. The new feature should live in its own panel and pass typed data through `App.tsx` rather than accreting launch-logic UI state inside the existing card.
- Centralize wrapper ordering and conflict handling in Rust. This is where correctness lives for MangoHud, GameMode, and optional `game-performance`.
- Treat advanced/community-only options as a later enablement layer on top of the same contract. Do not let optional HDR/Wayland/upscaler switches complicate the initial persistence and launch plumbing.
