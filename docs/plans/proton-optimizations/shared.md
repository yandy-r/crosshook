# Proton Launch Optimizations

`proton-optimizations` fits into CrossHook’s existing `proton_run` pipeline: profile state is edited in React, persisted through Tauri profile commands, converted into a typed `LaunchRequest` in `App.tsx`, then validated and executed in Rust launch modules. The feature adds a new `launch.optimizations` profile subsection, a dedicated right-column UI panel beneath the current launch card, and a backend resolver that turns stable option IDs into deterministic env vars and wrapper prefixes. The main architectural constraint is that checkbox-driven autosave cannot reuse the current full `persistProfileDraft()` loop because that path refreshes metadata and reloads the entire profile after every save. Implementation should therefore keep the frontend human-friendly and typed, while pushing all launch translation, conflict checks, and wrapper ordering into `crosshook-core` for the `proton_run` path only.

## Relevant Files

- /docs/plans/proton-optimizations/feature-spec.md: Source of truth for scope, option model, autosave boundary, and phased plan.
- /src/crosshook-native/src/App.tsx: Builds `LaunchRequest` and owns the right-column layout where the panel will live.
- /src/crosshook-native/src/components/LaunchPanel.tsx: Existing launch card the new optimization panel should sit beneath or compose with.
- /src/crosshook-native/src/hooks/useProfile.ts: Profile normalization, mutation, and save logic; current full-save flow is too heavy for autosave.
- /src/crosshook-native/src/types/profile.ts: Frontend `GameProfile` contract that must gain `launch.optimizations`.
- /src/crosshook-native/src/types/launch.ts: Frontend `LaunchRequest` contract that should carry optimization IDs into launch.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Thin IPC layer and correct home for a section-only optimization save command.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Launch entry points and validation boundary before process spawn.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Rust TOML profile model mirrored to the frontend schema.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: TOML persistence and round-trip test location for the new profile subsection.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Rust request validation and method gating for `proton_run`.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Direct Proton command construction and best integration point for wrappers/env injection.
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Shared host/runtime env reconstruction helpers used by `proton_run`.
- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs: Existing environment allowlist/clear-list invariants and test pattern.
- /src/crosshook-native/src/styles/theme.css: Existing layout/styles file for the new right-column card, advanced disclosure, and save-state UI.

## Relevant Tables

- none: This feature uses TOML-backed profile persistence instead of database tables.

## Relevant Patterns

**Hook-driven state with thin presentation components**: Long-lived editor and launch state lives in hooks, while surface components stay presentational. See [useProfile.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts) and [LaunchPanel.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/LaunchPanel.tsx).

**Typed frontend/backend model mirroring**: TypeScript request/profile shapes are mirrored in Rust Serde structs and evolved in lockstep. See [profile.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts), [launch.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/launch.ts), [models.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs), and [request.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs).

**Thin Tauri command layer over core logic**: Tauri commands translate IPC calls and delegate persistence or launch behavior to `crosshook-core` rather than embedding business logic in handlers. See [profile.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs) and [launch.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs).

**Backend-owned launch orchestration**: React builds typed requests, while Rust handles validation, env reconstruction, wrapper ordering, and process spawn. See [App.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx), [script_runner.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs), and [runtime_helpers.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs).

**Environment isolation and explicit allowlists**: `proton_run` starts from `env_clear()` and reconstructs only approved host and Proton variables, so new launch options must be deliberately mapped rather than inherited from shell state. See [env.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/env.rs) and [runtime_helpers.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs).

## Relevant Docs

**/docs/plans/proton-optimizations/feature-spec.md**: You _must_ read this when working on scope, option catalog, autosave rules, tooltips, and phased implementation.

**/docs/plans/proton-optimizations/research-technical.md**: You _must_ read this when working on typed persistence, Rust launch resolution, and file-level impact.

**/docs/plans/proton-optimizations/research-ux.md**: You _must_ read this when working on panel placement, grouping, save-state UI, and per-option info tooltips.

**/docs/plans/proton-optimizations/research-business.md**: You _must_ read this when working on autosave eligibility, runner applicability, and install-review boundaries.

**/docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when working on current `proton_run` launch behavior and existing launch flow semantics.

**/docs/getting-started/quickstart.md**: You _must_ read this when updating saved-profile or user-facing launch workflow guidance.

**/README.md**: You _must_ read this when updating top-level feature descriptions or launch-mode language.

**/AGENTS.md**: You _must_ read this when implementing changes that span frontend types, Tauri commands, Rust launch code, and verification workflow.
