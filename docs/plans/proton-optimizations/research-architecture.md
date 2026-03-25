# Architecture Research: proton-optimizations

## System Overview
CrossHook’s Proton launch path is split across three layers: React profile/editor state in `src/crosshook-native/src/`, Tauri IPC commands in `src/crosshook-native/src-tauri/src/commands/`, and Rust launch execution in `src/crosshook-native/crates/crosshook-core/src/launch/`. For `proton_run`, the relevant architecture already exists to carry typed profile data from the editor into a `LaunchRequest`, validate it in Rust, and spawn a direct Proton process with an explicitly reconstructed environment. The new optimization panel fits cleanly into that path by extending the profile and launch-request models, adding a narrow persistence path for `launch.optimizations`, and resolving saved option IDs to env vars and wrapper commands immediately before `build_proton_game_command()` and `build_proton_trainer_command()`.

## Relevant Components
- /docs/plans/proton-optimizations/feature-spec.md: Current source of truth for required scope, data shape, and UX placement.
- /src/crosshook-native/src/App.tsx: Builds `launchRequest`, resolves launch method, and owns the right-column layout where the panel will sit.
- /src/crosshook-native/src/components/LaunchPanel.tsx: Existing Proton launch card the optimization panel should compose with or sit beneath.
- /src/crosshook-native/src/components/ProfileEditor.tsx: Owns profile editor state and is the likely parent for optimization autosave coordination.
- /src/crosshook-native/src/hooks/useProfile.ts: Loads, normalizes, mutates, and saves `GameProfile`; current full-save path is too heavy for per-toggle autosave.
- /src/crosshook-native/src/types/profile.ts: Frontend `GameProfile` definition that needs a new `launch.optimizations` subsection.
- /src/crosshook-native/src/types/launch.ts: Frontend `LaunchRequest` type that must carry optimization IDs into launch validation/execution.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri profile load/save boundary and the correct place for a new section-specific optimization save command.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Tauri launch entry point that validates requests and selects the `proton_run` command builders.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Rust TOML-backed profile model that mirrors the frontend profile shape.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Rust launch-request model and validation logic for `proton_run`.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Direct Proton process construction for game and trainer launch.
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Low-level host env, Proton env, working directory, and stdio setup for direct Proton commands.

## Data Flow
Saved profile data enters the app through `profile_load` in `src-tauri/src/commands/profile.rs`, then `useProfile.ts` normalizes it into the editor state used by `App.tsx` and `ProfileEditor.tsx`. `App.tsx` computes `effectiveLaunchMethod`, derives the current `LaunchRequest`, and passes that request into `LaunchPanel`, whose `useLaunchState` hook calls the Tauri `validate_launch`, `launch_game`, and `launch_trainer` commands.

For the `proton_run` path, `src-tauri/src/commands/launch.rs` routes the request to `crosshook-core` validation in `launch/request.rs`, then to `build_proton_game_command()` or `build_proton_trainer_command()` in `launch/script_runner.rs`. Those builders create a direct `proton run ...` command, clear the environment, rehydrate host and Proton-specific variables through `runtime_helpers.rs`, set the working directory, and attach logs before spawning the process. The optimization feature should join this flow in two places: persistence of `launch.optimizations` through the profile store, and launch-time resolution of saved optimization IDs into additional env vars or wrapper prefixes just before the final Proton command is built.

## Integration Points
The frontend panel should integrate into the existing right column in `src/crosshook-native/src/App.tsx`, directly beneath `LaunchPanel.tsx`, because that column already groups launch-affecting controls and status. The panel’s state source should remain the shared `GameProfile` in `useProfile.ts`, but autosave should avoid the existing `persistProfileDraft()` reload cycle by adding a dedicated Tauri command in `src-tauri/src/commands/profile.rs` that loads the current TOML profile, updates only `launch.optimizations`, and writes it back via `ProfileStore`.

On the backend, the cleanest hook point is a new launch-optimization resolver module under `crates/crosshook-core/src/launch/` that accepts `LaunchRequest` optimization IDs and returns deterministic env vars and wrapper commands for `proton_run`. `launch/request.rs` should validate the IDs and applicability, `script_runner.rs` should incorporate wrapper ordering and env injection into the direct Proton command builders, and `runtime_helpers.rs` should stay the shared utility layer for reconstructed environment setup. The Rust and TypeScript profile/request models must be extended in lockstep so Tauri serialization remains consistent.

## Key Dependencies
- React 18 component state and composition in `src/crosshook-native/src/`: existing UI layer for the panel and autosave feedback.
- `@tauri-apps/api/core` invokes in `useProfile.ts` and launch hooks: current frontend-to-backend bridge for save and launch commands.
- Rust `serde` models in `profile/models.rs` and `launch/request.rs`: shared serialization contract for TOML persistence and Tauri IPC payloads.
- `ProfileStore` TOML persistence in `crates/crosshook-core/src/profile/`: durable storage boundary for optimization IDs.
- `tokio::process::Command` in `script_runner.rs` and `runtime_helpers.rs`: execution layer that can prepend wrappers and inject env vars for `proton_run`.
- Host-side Proton runtime, MangoHud, GameMode, and optional `game-performance` executables: external runtime dependencies the new resolver must model and validate when enabled.
