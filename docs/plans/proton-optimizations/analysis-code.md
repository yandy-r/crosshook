# Code Analysis: proton-optimizations

## Executive Summary

The implementation sits cleanly on top of CrossHook’s existing `proton_run` path: React owns typed profile state and layout composition, Tauri commands provide thin IPC boundaries, and `crosshook-core` owns validation plus process construction. The most important split is between persistence and launch translation: the frontend needs a narrow autosave path for `launch.optimizations`, while the backend needs a new resolver that maps stable option IDs to env vars and wrapper prefixes before `build_proton_game_command()` or `build_proton_trainer_command()` run.

## Existing Code Structure

### Related Components

- /src/crosshook-native/src/App.tsx: Composes the right-column layout and builds `LaunchRequest` from the current profile.
- /src/crosshook-native/src/components/LaunchPanel.tsx: Presentational launch card and the natural visual anchor for the new panel.
- /src/crosshook-native/src/hooks/useProfile.ts: Owns profile normalization, mutation, and explicit save behavior.
- /src/crosshook-native/src/types/profile.ts: Frontend `GameProfile` contract that must gain the optimization subsection.
- /src/crosshook-native/src/types/launch.ts: Frontend `LaunchRequest` contract that must carry optimization IDs.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Thin Tauri persistence layer and best place for a section-only save command.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Launch entry point that validates requests and selects the `proton_run` builder.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Rust TOML-backed profile schema mirroring the TS types.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: TOML save/load boundary and round-trip test home.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Rust request model and fail-fast validation surface.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Direct Proton process construction and wrapper/env injection seam.
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Shared env reconstruction and log attachment helpers.
- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs: Existing env allowlist/clear-list invariants and test pattern.

### File Organization Pattern

The repo separates concerns by layer, not by feature folder. Frontend React state and view code live in `src/crosshook-native/src/`, Tauri IPC wrappers live in `src-tauri/src/commands/`, and durable models plus launch logic live in `crates/crosshook-core/src/`. Similar cross-boundary changes are implemented by updating TypeScript types, Rust Serde models, Tauri commands, and core logic in lockstep instead of introducing ad hoc shims or UI-owned execution rules.

## Implementation Patterns

### Pattern: Typed contract mirroring

**Description**: Frontend and backend data shapes are mirrored explicitly, then normalized centrally rather than inferred at call sites.
**Example**: See `/src/crosshook-native/src/types/profile.ts` lines 16-48, `/src/crosshook-native/src/types/launch.ts` lines 11-29, `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` lines 31-115, and `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs` lines 12-54.
**Apply to**: Add `launch.optimizations` and any launch-request optimization payload in both TypeScript and Rust together.

### Pattern: Thin Tauri commands over core business logic

**Description**: Tauri commands only expose IPC-safe functions and delegate persistence or launch behavior to `crosshook-core`.
**Example**: See `/src/crosshook-native/src-tauri/src/commands/profile.rs` lines 55-103 and `/src/crosshook-native/src-tauri/src/commands/launch.rs` lines 22-90.
**Apply to**: Add a focused optimization-save command in `profile.rs`, but keep merge/save and launch-resolution logic in Rust core modules.

### Pattern: Hook-owned editor state with presentational panels

**Description**: Long-lived domain state lives in hooks, while surface components receive typed props and avoid direct persistence logic.
**Example**: See `/src/crosshook-native/src/hooks/useProfile.ts` lines 204-260 and `/src/crosshook-native/src/components/LaunchPanel.tsx` lines 18-233.
**Apply to**: Implement the new panel as its own component, with autosave state coming from a hook or narrow extension of `useProfile.ts`.

### Pattern: Backend-owned launch construction

**Description**: React builds a typed `LaunchRequest`; Rust validates it and constructs the final command.
**Example**: See `/src/crosshook-native/src/App.tsx` lines 102-126, `/src/crosshook-native/src-tauri/src/commands/launch.rs` lines 27-45 and 59-79, and `/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` lines 58-102.
**Apply to**: Keep option ID -> env/wrapper mapping in Rust, not in React.

### Pattern: Explicit environment reconstruction

**Description**: `proton_run` starts from `env_clear()` and rehydrates only known-safe host and Proton variables, so launch options must be deliberately allowlisted.
**Example**: See `/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` lines 11-31 and 34-63, plus `/src/crosshook-native/crates/crosshook-core/src/launch/env.rs` lines 1-53.
**Apply to**: Add optimization env vars through a dedicated resolver and preserve the current isolation model.

## Integration Points

### Files to Create

- /src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx: Dedicated UI for grouped toggles, tooltips, autosave state, and preview.
- /src/crosshook-native/src/types/launch-optimizations.ts: Frontend option catalog and shared metadata for labels, tooltips, groups, and applicability.
- /src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs: Backend registry and resolver from stable option IDs to env vars and wrappers.

### Files to Modify

- /src/crosshook-native/src/App.tsx: Add the new panel to the right-column stack and extend `launchRequest` with optimization IDs.
- /src/crosshook-native/src/hooks/useProfile.ts: Normalize optimization state and add a narrow autosave path that does not reload the whole profile.
- /src/crosshook-native/src/types/profile.ts: Extend `GameProfile.launch`.
- /src/crosshook-native/src/types/launch.ts: Extend `LaunchRequest`.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Add section-only optimization persistence for existing profiles.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Accept the richer request and keep the validation/spawn boundary intact.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Persist `launch.optimizations` in TOML.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: Add save/load round-trip coverage for the new subsection.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Validate optimization IDs and `proton_run` applicability.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Apply env vars and wrapper ordering before the final `proton run` command.
- /src/crosshook-native/src/styles/theme.css: Add card layout, advanced disclosure, tooltip, and autosave-status styles.

## Code Conventions

### Naming

React components use `PascalCase`, hooks and helpers use `camelCase`, and Rust uses `snake_case`. Existing types prefer explicit nested interfaces/structs and string unions over loose objects, so the new optimization surface should use named interfaces and stable string IDs instead of free-form records.

### Error Handling

Frontend hooks catch `invoke()` failures and reduce them into local string state rather than throwing through the tree. Rust uses explicit validation enums with `Display`-backed messages, then returns `Result<_, String>` through Tauri commands. The new feature should follow the same fail-fast pattern: invalid option IDs, incompatible wrapper combinations, or missing host binaries should become explicit validation or autosave errors.

### Testing

The strongest existing test pattern is colocated Rust unit tests inside the affected modules. There is no established frontend unit-test harness, so correctness should come from Rust tests for TOML round-trips, request validation, and command construction, plus manual Tauri UI verification for the panel and autosave feedback.

## Dependencies and Services

### Available Utilities

- `resolveLaunchMethod` in `/src/crosshook-native/src/App.tsx`: current method-normalization logic to preserve when extending request construction.
- `normalizeProfileForEdit` / `normalizeProfileForSave` in `/src/crosshook-native/src/hooks/useProfile.ts`: central normalization hooks for new profile fields.
- `ProfileStore` in `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: existing TOML persistence boundary to reuse for section updates.
- `apply_host_environment`, `apply_runtime_proton_environment`, and `attach_log_stdio` in `/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: reusable building blocks for the new launch resolver.
- `ValidationError` in `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: current user-facing validation model to extend.

### Required Dependencies

- Existing `@tauri-apps/api/core` invoke path for frontend autosave and launch requests.
- Existing Rust `serde` model mirroring for TOML persistence and IPC.
- Existing `tokio::process::Command` launch path for wrapper composition.
- No new npm or cargo dependency is obviously required for the core implementation; host tools like MangoHud and GameMode remain optional runtime dependencies, not bundled libraries.

## Gotchas and Warnings

- `useProfile.persistProfileDraft()` is intentionally heavy: it saves, syncs settings/recent files, refreshes the profile list, and reloads the profile. Reusing it for checkbox autosave will churn state and can silently persist unrelated editor changes.
- `LaunchPanel.tsx` is intentionally presentational. Do not bury autosave or env translation logic inside it.
- `proton_run` currently constructs the direct command as `proton run <path>` in `script_runner.rs` lines 58-102. Wrapper ordering must be inserted carefully so the final process shape stays deterministic.
- `runtime_helpers.rs` starts from `env_clear()` and re-adds only approved variables. If new env vars are needed, they must be added explicitly; ambient shell state will not carry through.
- `env.rs` documents a Rust/shell sync invariant for Proton/WINE environment handling. Even though this feature is `proton_run`-only, any future spillover into Steam helpers must respect that invariant.
- The current profile types do not have an obvious “existing file on disk” flag beyond `profileExists` and selected name. Autosave eligibility for brand-new profiles needs deliberate handling, not guesswork.

## Task-Specific Guidance

- **For UI tasks**: keep the new panel separate from `LaunchPanel.tsx` and feed it typed props plus a narrow autosave surface; use grouped checkboxes, per-option tooltip metadata, and save-state feedback rather than inline launch logic.
- **For Rust launch tasks**: add a dedicated `launch/optimizations.rs` resolver and keep all ID validation, wrapper ordering, and env mapping in Rust before `build_proton_game_command()` / `build_proton_trainer_command()` spawn.
- **For persistence tasks**: add a section-only IPC command in `src-tauri/src/commands/profile.rs` and update `ProfileStore`-adjacent code to merge `launch.optimizations` into the existing TOML document without persisting unrelated dirty editor fields.
