# Architecture Research: install-game

## System Overview

CrossHook Native is split into three layers that matter for this feature: React UI in `src/crosshook-native/src/`, Tauri IPC commands in `src/crosshook-native/src-tauri/src/commands/`, and shared Rust domain logic in `src/crosshook-native/crates/crosshook-core/src/`. The current install-game feature fits most cleanly as a new backend domain parallel to `launch`, `profile`, and `steam`, with a new Tauri command module exposing it and a new Profile-panel sub-tab in the frontend consuming it. The important architectural constraint is that installer execution is close to current `proton_run` behavior, but not identical: it needs prefix creation, executable discovery, and a final review handoff before saving a normal `GameProfile`.

## Relevant Components

- `/src/crosshook-native/src/App.tsx`: Root app composition, top-level tabs, and launch request assembly.
- `/src/crosshook-native/src/components/ProfileEditor.tsx`: Current profile creation/edit panel and best insertion point for the install sub-tab.
- `/src/crosshook-native/src/hooks/useProfile.ts`: Frontend state machine for profile list/load/save/delete and metadata sync.
- `/src/crosshook-native/src/types/profile.ts`: TypeScript shape for persisted profiles and runtime fields.
- `/src/crosshook-native/src/types/launch.ts`: Frontend launch request contract and launch result types.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri builder, command registration, and store injection.
- `/src/crosshook-native/src-tauri/src/commands/mod.rs`: Domain-oriented command module boundaries.
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Thin Tauri wrappers over `ProfileStore`.
- `/src/crosshook-native/src-tauri/src/commands/steam.rs`: Proton discovery and Steam path detection commands already consumed by the frontend.
- `/src/crosshook-native/src-tauri/src/commands/launch.rs`: Async process-launch pattern with log streaming and typed command results.
- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Shared Rust domain module exports; new install domain will plug in here.
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: Canonical persisted `GameProfile` shape used by frontend and backend.
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: File-backed profile persistence under XDG config.
- `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: Launch request shape and validation rules that install logic should reference but not overload.
- `/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Direct `proton run` execution, environment assembly, log wiring, and helper patterns to reuse.
- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: Filesystem-based Proton discovery and compat-tool resolution.
- `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: App settings persistence pattern.
- `/src/crosshook-native/crates/crosshook-core/src/settings/recent.rs`: Recent-path persistence pattern, useful if installer media recents are added later.

## Data Flow

The current relevant flow starts in `ProfileEditor.tsx`, which renders mode-specific form fields and uses `invoke()` plus `useProfile` to mutate or persist profiles. `useProfile.ts` owns the edit-state lifecycle: it normalizes loaded profile data, saves via `profile_save`, and synchronizes `settings.toml` and `recent.toml` after successful operations. Tauri is intentionally thin: `src-tauri/src/commands/*.rs` translate IPC payloads into shared Rust domain types and return `Result<_, String>` for frontend consumption.

For direct Proton execution, `App.tsx` derives a `LaunchRequest`, `commands/launch.rs` validates and spawns a process, and `crosshook-core/src/launch/` builds the actual command and environment. The install-game feature should follow the same overall shape but with a different mid-layer contract: the frontend gathers install inputs, a new `install` command validates and provisions the prefix, the backend launches the installer via direct `proton run`, then the result is turned into a reviewable `GameProfile` that the frontend loads into the existing profile editor before final save.

## Integration Points

- Frontend UI integration should happen inside `ProfileEditor.tsx` by adding a shallow sub-tab or segmented switch between the existing profile editor and the new install flow.
- Frontend orchestration should live in a new install-specific hook rather than bloating `useProfile.ts` with installer process state.
- Tauri integration should follow the current domain pattern by adding `/src/crosshook-native/src-tauri/src/commands/install.rs` and registering it in `/src/crosshook-native/src-tauri/src/lib.rs`.
- Shared Rust logic should live in a new `/src/crosshook-native/crates/crosshook-core/src/install/` module, exported from `/src/crosshook-native/crates/crosshook-core/src/lib.rs`.
- Installer runtime execution should reuse extracted helpers from `launch/script_runner.rs` for host environment, Proton environment, working directory selection, and log file setup.
- Final persistence should reuse `ProfileStore` and existing `GameProfile` schema rather than inventing a second durable profile format.
- Executable discovery should be a post-install backend step that returns ranked candidates to the UI for review, not a frontend-only heuristic.

## Key Dependencies

- React 18 component/hook patterns in `src/crosshook-native/src/`.
- Tauri v2 command registration and async runtime patterns in `src/crosshook-native/src-tauri/`.
- `@tauri-apps/api/core` and `@tauri-apps/plugin-dialog` for typed IPC and native file pickers.
- `serde` and shared Rust data models for frontend/backend contract stability.
- `tokio::process::Command` for process spawning and streaming installer logs.
- `directories::BaseDirs` for canonical XDG path resolution.
- Existing Proton discovery in `crosshook_core::steam` and direct Proton runtime assembly in `crosshook_core::launch`.
