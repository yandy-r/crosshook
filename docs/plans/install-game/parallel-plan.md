# Install Game Implementation Plan

The `install-game` feature should be implemented as a new workflow layered onto the existing native stack rather than as a branch inside the current launch commands. The core change is a dedicated Rust `install` domain that reuses Proton discovery, direct `proton run` runtime setup, and TOML profile persistence while adding install-specific validation, prefix provisioning, and bounded executable discovery. On the frontend, the feature belongs as a sibling sub-tab inside `ProfileEditor`, backed by its own hook and result types so installer execution, status reporting, and final profile review remain isolated from the current save/load path. The highest-risk integration point is the final review/save boundary: installer media must never be saved as `game.executable_path`, so the plan treats ranked executable discovery and explicit confirmation as first-class work, not polish.

## Critically Relevant Files and Documentation

- /src/crosshook-native/src/components/ProfileEditor.tsx: Main insertion point for the install sub-tab and profile review handoff.
- /src/crosshook-native/src/hooks/useProfile.ts: Existing profile lifecycle logic that must remain the single save/load source of truth.
- /src/crosshook-native/src/App.tsx: Root orchestration and current state wiring between profile UI and runtime actions.
- /src/crosshook-native/src/types/profile.ts: Frontend `GameProfile` contract that install output must satisfy.
- /src/crosshook-native/src/types/launch.ts: Existing typed command/result pattern to mirror for install contracts.
- /src/crosshook-native/src/styles/theme.css: Existing tab-row and panel styling utilities to reuse for the new sub-tab shell.
- /src/crosshook-native/src-tauri/src/lib.rs: Tauri command registration and managed-store injection boundary.
- /src/crosshook-native/src-tauri/src/commands/mod.rs: Domain command export surface.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Existing final profile persistence command surface to preserve.
- /src/crosshook-native/src-tauri/src/commands/steam.rs: Existing Proton discovery command reused directly by the install UI.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Best reference for async process execution and log-event streaming.
- /src/crosshook-native/crates/crosshook-core/src/lib.rs: Shared Rust module export surface where the new install domain plugs in.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Existing typed validation/error pattern to mirror for install validation.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Current direct `proton run` environment assembly and extraction target.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Canonical persisted profile model that should remain stable.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: Profile persistence and name validation.
- /src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: Compat-tool discovery and Proton resolution behavior.
- /docs/plans/install-game/feature-spec.md: Settled product and architecture decisions for this feature.
- /docs/plans/install-game/shared.md: Condensed shared context for implementation.
- /docs/features/steam-proton-trainer-launch.doc.md: Existing Proton/profile behavior the new install flow must stay consistent with.

## Implementation Plan

### Phase 1: Contracts And Shared Runtime Setup

#### Task 1.1: Create Install-Domain Contracts And Export Surface Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/install-game/feature-spec.md
- /docs/plans/install-game/shared.md
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/install/mod.rs
- /src/crosshook-native/crates/crosshook-core/src/install/models.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/lib.rs

Define the install-domain request/result contracts and the typed validation/result enums needed across Rust, Tauri, and the frontend. The request shape should reflect the settled decisions: direct `proton run`, no draft persistence, default prefix resolution in the backend, optional trainer, and post-install ranked executable candidates returned for review. Keep the final artifact aligned with the existing `GameProfile` schema instead of adding a new persisted profile type. Export the new domain from `crosshook-core` without embedding business logic in `mod.rs`; the goal of this task is a stable contract surface that unblocks Tauri wiring, frontend typing, and service implementation.

#### Task 1.2: Extract Reusable Direct-Proton Runtime Helpers Depends on [none]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- /src/crosshook-native/src-tauri/src/commands/launch.rs

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/launch/mod.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs

Extract the direct-Proton runtime pieces that install execution should share with normal `proton_run`: host environment propagation, runtime Proton environment assembly, working-directory selection, and log file/stdout-stderr attachment. Keep Steam-specific helper-script behavior where it is; the goal is not a launch refactor, only enough reuse so install execution does not fork its own environment semantics. Preserve existing launch behavior and tests while creating small helper functions that the new install service can call directly.

#### Task 1.3: Add Frontend Install Types And Hook Scaffolding Depends on [1.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/types/profile.ts
- /src/crosshook-native/src/hooks/useProfile.ts
- /docs/plans/install-game/feature-spec.md

**Instructions**

Files to Create

- /src/crosshook-native/src/types/install.ts
- /src/crosshook-native/src/hooks/useInstallGame.ts

Files to Modify

- /src/crosshook-native/src/types/index.ts

Mirror the new install-domain contracts in TypeScript and create a dedicated hook skeleton for install form state, validation state, async command status, and result handling. Keep the hook separate from `useProfile.ts` so installer execution does not pollute the current profile save/load state machine. This task should establish a strongly typed frontend boundary and expose the state primitives the UI shell will need later, but it should not yet take ownership of final profile save/review logic.

### Phase 2: Backend Install Pipeline

#### Task 2.1: Add Tauri Install Command Surface Depends on [1.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src-tauri/src/lib.rs
- /src/crosshook-native/src-tauri/src/commands/mod.rs
- /src/crosshook-native/src-tauri/src/commands/launch.rs
- /src/crosshook-native/src-tauri/src/commands/profile.rs

**Instructions**

Files to Create

- /src/crosshook-native/src-tauri/src/commands/install.rs

Files to Modify

- /src/crosshook-native/src-tauri/src/commands/mod.rs
- /src/crosshook-native/src-tauri/src/lib.rs

Introduce a thin Tauri command module for install behavior and register it in the central handler list. Follow the existing command shape: typed inputs, narrow glue code, `Result<_, String>` outputs, and `spawn_blocking` for any blocking filesystem/process work. This task should expose backend-resolved default prefix path and the install command entry point without embedding business logic into the command file itself.

#### Task 2.2: Implement Install Service, Prefix Resolution, And Direct Proton Execution Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs
- /docs/plans/install-game/feature-spec.md

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/install/service.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/install/mod.rs
- /src/crosshook-native/crates/crosshook-core/src/install/models.rs

Implement the core install service that resolves the default prefix root under `~/.local/share/crosshook/prefixes/<slug>`, validates installer inputs, provisions the prefix if missing, launches the installer through direct `proton run`, and returns a structured install result with log path metadata. Keep persistence out of this task except for preparing the result that later review/save logic will consume; v1 should not persist drafts. Use the extracted runtime helpers instead of duplicating environment construction, and keep user-facing error strings explicit and actionable.
Keep install log-path creation in the Tauri command layer, matching the current launch-command pattern, and pass the resolved log path into the service so process/log ownership does not split awkwardly across layers.

#### Task 2.3: Implement Executable Discovery Ranking And Generated Profile Assembly Depends on [1.1, 2.2]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/install/models.rs
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- /docs/plans/install-game/feature-spec.md

**Instructions**

Files to Create

- /src/crosshook-native/crates/crosshook-core/src/install/discovery.rs

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/install/service.rs
- /src/crosshook-native/crates/crosshook-core/src/install/mod.rs

Add bounded post-install executable discovery that scans the prefix for likely game executables, ranks plausible candidates, and de-ranks obvious setup, uninstaller, updater, or redistributable binaries. Build the reviewable `GameProfile` result from install inputs plus the ranked candidate list, but leave final save authority to the review step so installer media cannot leak into `game.executable_path`. Keep the heuristics assistive rather than authoritative: the best candidate can be preselected, but the result should still require confirmation before persistence.

### Phase 3: UI Integration And Review Flow

#### Task 3.1: Add Install Game Panel And Profile Sub-Tab Shell Depends on [1.3]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/styles/theme.css
- /docs/plans/install-game/research-ux.md
- /tasks/lessons.md

**Instructions**

Files to Create

- /src/crosshook-native/src/components/InstallGamePanel.tsx

Files to Modify

- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/styles/theme.css

Add a shallow `Profile` / `Install Game` sub-tab shell inside the existing profile panel and mount a dedicated install panel component rather than growing `ProfileEditor.tsx` further. Reuse the current visual/tab styling patterns and keep the detected-Proton selector contract intact: selection should fill an editable path field, not lock it. This task should establish the install form layout, helper copy, and space for status/review sections, but it should not yet own the final review/save integration.

#### Task 3.2: Wire Install Hook To IPC, Status, Logs, And Candidate Mapping Depends on [2.1, 2.3, 3.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useInstallGame.ts
- /src/crosshook-native/src/components/InstallGamePanel.tsx
- /src/crosshook-native/src-tauri/src/commands/install.rs
- /src/crosshook-native/src-tauri/src/commands/launch.rs

**Instructions**

Files to Create

- [none]

Files to Modify

- /src/crosshook-native/src/hooks/useInstallGame.ts
- /src/crosshook-native/src/components/InstallGamePanel.tsx
- /src/crosshook-native/src/types/install.ts

Connect the install hook and panel to the new Tauri commands, including default-prefix resolution, async status transitions, installer log visibility, and ranked executable candidate mapping. Keep the UI state explicit: preparing, running installer, review required, failed, and ready-to-save should each have distinct behavior and messages. Do not save the profile in this task; the goal is a complete install execution and review-state handoff surface.

#### Task 3.3: Integrate Final Review And Save With Existing Profile Flow Depends on [2.3, 3.1, 3.2]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useProfile.ts
- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/components/InstallGamePanel.tsx

**Instructions**

Files to Create

- [none]

Files to Modify

- /src/crosshook-native/src/hooks/useProfile.ts
- /src/crosshook-native/src/components/ProfileEditor.tsx

Wire the generated install result back into the existing profile state machinery so the user reviews a standard `GameProfile`, confirms or edits the final executable, and saves through the established persistence path. Keep `useProfile.ts` as the single source of truth for persisted profiles, but add only the minimal helpers needed to hydrate or replace current edit state from the install result. Keep this handoff inside `ProfileEditor.tsx` plus `useProfile.ts`; do not involve `App.tsx` unless an unavoidable event or ownership boundary appears during implementation. Make the save boundary unambiguous: installation completes first, then explicit profile review/save happens, with no draft persistence in between.

### Phase 4: Tests, Docs, And Final Verification

#### Task 4.1: Add Focused Rust Tests For Install Validation And Discovery Depends on [2.2, 2.3]

**READ THESE BEFORE TASK**

- /src/crosshook-native/crates/crosshook-core/src/install/models.rs
- /src/crosshook-native/crates/crosshook-core/src/install/service.rs
- /src/crosshook-native/crates/crosshook-core/src/install/discovery.rs
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs

**Instructions**

Files to Create

- [none]

Files to Modify

- /src/crosshook-native/crates/crosshook-core/src/install/models.rs
- /src/crosshook-native/crates/crosshook-core/src/install/service.rs
- /src/crosshook-native/crates/crosshook-core/src/install/discovery.rs

Add inline Rust tests for the install-domain behaviors that are easiest to regress: default prefix resolution, validation errors, prefix creation assumptions, executable candidate ranking, and generated profile assembly. Keep these tests deterministic and filesystem-local using `tempfile`, following current domain-test patterns. This task is the main correctness backstop for the new backend behavior and should land before final docs and verification.

#### Task 4.2: Update User-Facing Documentation For Install Flow And Storage Paths Depends on [3.3]

**READ THESE BEFORE TASK**

- /README.md
- /docs/getting-started/quickstart.md
- /docs/features/steam-proton-trainer-launch.doc.md
- /docs/plans/install-game/feature-spec.md

**Instructions**

Files to Create

- [none]

Files to Modify

- /README.md
- /docs/getting-started/quickstart.md
- /docs/features/steam-proton-trainer-launch.doc.md

Document the new install flow only after the behavior is stable in code. Update launch-mode and user-state wording to reflect that profiles remain under config while default install prefixes live under `~/.local/share/crosshook/prefixes/<slug>`, and explain the post-install review step so users understand why the profile is shown for confirmation before save. Keep the docs aligned with the direct `proton run` decision and do not mention `umu-run` as an active v1 path.

#### Task 4.3: Run Final Native Verification And Record Planning Follow-Through Depends on [4.1, 4.2]

**READ THESE BEFORE TASK**

- /tasks/todo.md
- /docs/plans/install-game/parallel-plan.md
- /src/crosshook-native/Cargo.toml
- /src/crosshook-native/package.json

**Instructions**

Files to Create

- [none]

Files to Modify

- /tasks/todo.md

Run the final implementation verification sequence after the feature lands: targeted Rust tests for the new install domain, `cargo check` for the native workspace, and the frontend production build. Record the implementation outcome, residual risks, and any manual validation gaps in `tasks/todo.md` using the existing task-log pattern. If verification reveals structural issues in task ownership or dependency boundaries, adjust the implementation sequencing before closing out the feature.

## Advice

- Extracting runtime helpers from `launch/script_runner.rs` early is the highest-leverage move in the plan; it keeps install execution from silently diverging from normal `proton_run`.
- Only one task at a time should own `/src/crosshook-native/src/components/ProfileEditor.tsx`, because it is the main shared frontend conflict file.
- Keep executable discovery assistive and backend-owned. If the frontend grows its own heuristics, save-boundary bugs will be harder to reason about and test.
- Resist adding draft persistence or `umu-run` branching in v1. Both expand the support surface without helping the core review/save requirement.
- The feature is complete only when the user can review and save a normal `GameProfile`; a successful installer process without a safe handoff is not a shippable endpoint.
