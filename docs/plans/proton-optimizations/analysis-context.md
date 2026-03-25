# Context Analysis: proton-optimizations

## Executive Summary

This feature adds a typed, profile-scoped `launch.optimizations` model and a new right-column UI panel for `proton_run` profiles only. The implementation should keep the frontend responsible for human-friendly option selection, autosave state, and tooltips, while Rust owns launch translation, conflict validation, and final Proton command construction. The key architectural risk is persistence scope: optimization toggles need lightweight section-only autosave for existing profiles, not the current full profile save/reload loop.

## Architecture Context

- **System Structure**: React editor/layout code in `src/crosshook-native/src/` feeds typed profile and launch-request data into Tauri commands in `src-tauri/src/commands/`, which delegate persistence and launch orchestration to `crosshook-core`.
- **Data Flow**: `profile_load` hydrates `GameProfile` into `useProfile`, `App.tsx` derives `LaunchRequest`, `validate_launch` gates the request, and `script_runner.rs` builds the direct `proton run` command with explicit env reconstruction.
- **Integration Points**: Add the new panel under `LaunchPanel`, extend TS/Rust profile and request models with `launch.optimizations`, add a narrow optimization save command in `profile.rs`, and introduce a Rust launch-optimization resolver used by `script_runner.rs`.

## Critical Files Reference

- /docs/plans/proton-optimizations/feature-spec.md: Scope, option catalog, autosave rules, and required `proton_run` focus.
- /docs/plans/proton-optimizations/shared.md: Consolidated file map, patterns, and must-read docs.
- /src/crosshook-native/src/App.tsx: Right-column composition and `LaunchRequest` construction.
- /src/crosshook-native/src/components/LaunchPanel.tsx: Launch card the new panel should sit beneath or coordinate with.
- /src/crosshook-native/src/hooks/useProfile.ts: Existing profile mutation/save path that must not be reused directly for checkbox autosave.
- /src/crosshook-native/src/types/profile.ts: Frontend `GameProfile` schema to extend with `launch.optimizations`.
- /src/crosshook-native/src/types/launch.ts: Frontend `LaunchRequest` schema to extend with optimization IDs.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Best place for section-only optimization persistence IPC.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Launch validation/spawn entry point that must accept the extended request.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Rust TOML profile model that must mirror TS changes.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: Profile persistence layer and round-trip test location.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Rust validation model for method gating and option ID checks.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Direct Proton command builder where wrappers/env vars should be injected.
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Existing env reconstruction helpers that preserve launch determinism.
- /src/crosshook-native/src/styles/theme.css: Existing styling/layout surface for the new panel and autosave feedback.

## Patterns to Follow

- **Hook-driven editor state**: Keep long-lived profile and save state in hooks, with panel UI staying presentational. Example: `/src/crosshook-native/src/hooks/useProfile.ts`.
- **Typed TS/Rust model mirroring**: Update TypeScript and Rust structs in lockstep whenever profile/request shapes change. Example: `/src/crosshook-native/src/types/profile.ts` and `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`.
- **Thin Tauri command layer**: Put IPC handlers in `src-tauri`, but keep merge/save and launch resolution in `crosshook-core`. Example: `/src/crosshook-native/src-tauri/src/commands/profile.rs`.
- **Backend-owned launch orchestration**: Send stable IDs from React and let Rust resolve wrappers/env vars before spawning Proton. Example: `/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`.
- **Environment allowlist discipline**: New env-based options must be explicitly introduced into the direct Proton path because launches start from `env_clear()`. Example: `/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`.

## Cross-Cutting Concerns

- Autosave must persist only the optimization subtree so unrelated dirty profile edits are not silently saved.
- Tooltips are a required UX element for every visible option and must be accessible to keyboard and screen-reader users.
- Wrapper availability and conflicts need explicit validation, especially for MangoHud, GameMode, and optional `game-performance`.
- Community-only or hardware-specific flags such as HDR, NTSync, FSR4, DLSS, and `SteamDeck=1` must stay clearly labeled and likely behind an advanced disclosure.
- Rust tests are the primary verification surface; there is no established frontend unit-test harness.

## Parallelization Opportunities

- Frontend panel creation and TS type updates can run in parallel with Rust profile-model and request-model extensions once the option ID catalog is fixed.
- Backend persistence work in `profile.rs`/`toml_store.rs` can run independently from panel styling and layout.
- Launch resolver work in `launch/request.rs` and `script_runner.rs` can proceed alongside UI tooltip/status implementation, but both depend on the same stable option catalog.
- Documentation updates for quickstart/README can be deferred until the functional implementation is settled.

## Implementation Constraints

- Required scope is `proton_run`; Steam parity is optional future work and should not influence the core task breakdown.
- New unsaved profiles must stage optimization selections locally until first manual save; no silent profile creation.
- Existing `persistProfileDraft()` is too heavy for this interaction pattern because it reloads profile state and rewrites metadata.
- The launch path must remain deterministic: no raw shell strings, no arbitrary env editing, and no implicit reliance on ambient shell state.
- The option model needs stable IDs rather than persisted labels or raw env var names so UI copy can evolve without breaking saved profiles.

## Key Recommendations

- Build the plan around four layers: shared option catalog, profile/request model changes, section-only persistence, and backend launch resolution.
- Make a dedicated `LaunchOptimizationsPanel` and keep its state/update path narrow instead of embedding extra logic in `App.tsx`.
- Add Rust-side validation for unknown IDs, incompatible combinations, and missing host wrappers before process spawn.
- Treat advanced options as a later phase; keep the first implementation focused on the conservative `proton_run` set plus tooltip/accessibility requirements.
