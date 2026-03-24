# Code Analysis: install-game

## Executive Summary

The current codebase already contains most of the primitives install-game needs, but they are split across different domains: Proton discovery in `steam`, runtime process assembly in `launch`, and persistence/state handling in `profile` plus `useProfile`. The missing code is a dedicated install service and a matching UI surface, not a broad refactor. The plan should therefore create one new backend domain, one new Tauri command module, one new frontend panel/hook pair, and a small number of precise modifications to wire the generated profile back into the existing editor flow.

## Existing Code Structure

### Related Components

- /src/crosshook-native/src/components/ProfileEditor.tsx: Mode-aware form rendering, browse helpers, Proton selector, and current profile actions.
- /src/crosshook-native/src/hooks/useProfile.ts: Profile normalization, persistence, settings sync, and list refresh behavior.
- /src/crosshook-native/src/App.tsx: Top-level tab shell and current profile-to-launch request derivation.
- /src/crosshook-native/src/types/profile.ts: Frontend `GameProfile` definition.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Async spawn pattern and log-event streaming.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Thin save/load/list/delete commands over `ProfileStore`.
- /src/crosshook-native/src-tauri/src/commands/steam.rs: Existing Proton discovery command used by the frontend.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Direct `proton run` environment setup and command construction.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Validation structure and user-facing error messaging.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: File-backed profile persistence and name validation.

### File Organization Pattern

The backend is organized by feature domain, with shared Rust logic in `crosshook-core/src/<domain>/` and thin Tauri adapters in `src-tauri/src/commands/<domain>.rs`. The frontend follows a component-plus-hook split, with leaf components in `src/components/`, stateful logic in `src/hooks/`, and shared type definitions in `src/types/`. Similar features are integrated by adding one domain module, one Tauri command file, and one UI surface rather than by growing a central utility layer.

## Implementation Patterns

### Pattern: New Domain Module

**Description**: New behavior belongs in its own Rust feature domain with a narrow export surface.
**Example**: See `/src/crosshook-native/crates/crosshook-core/src/lib.rs` and `/src/crosshook-native/crates/crosshook-core/src/steam/`.
**Apply to**: install request/result models, validation, executable ranking, and profile generation.

### Pattern: Thin Tauri Adapter

**Description**: Command files translate IPC payloads into shared Rust calls and map domain errors to `String`.
**Example**: See `/src/crosshook-native/src-tauri/src/commands/profile.rs` and `/src/crosshook-native/src-tauri/src/commands/launch.rs`.
**Apply to**: new `commands/install.rs`.

### Pattern: Hook-Owned Async UI State

**Description**: Components render controls while hooks own async work, derived state, and error strings.
**Example**: See `/src/crosshook-native/src/hooks/useProfile.ts`.
**Apply to**: new `useInstallGame.ts` or equivalent install-state hook.

### Pattern: Mode-Specific Conditional UI

**Description**: The profile UI renders sections based on current mode rather than creating whole-screen route changes.
**Example**: See `/src/crosshook-native/src/components/ProfileEditor.tsx`.
**Apply to**: shallow sub-tab or segmented switch between `Profile` and `Install Game`.

### Pattern: Rust Validation Error Enum

**Description**: Validation is implemented as an explicit enum with `Display`/message mapping, not scattered string checks.
**Example**: See `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`.
**Apply to**: install request validation and executable-discovery edge cases.

## Integration Points

### Files to Create

- /src/crosshook-native/crates/crosshook-core/src/install/mod.rs: Export install-domain entry points.
- /src/crosshook-native/crates/crosshook-core/src/install/models.rs: `InstallGameRequest`, `InstallGameResult`, and validation/result types.
- /src/crosshook-native/crates/crosshook-core/src/install/service.rs: Prefix provisioning, installer execution, and profile-generation orchestration.
- /src/crosshook-native/crates/crosshook-core/src/install/discovery.rs: Ranked executable discovery heuristics after install exit.
- /src/crosshook-native/src-tauri/src/commands/install.rs: Tauri IPC surface for install flow.
- /src/crosshook-native/src/components/InstallGamePanel.tsx: Install sub-tab UI surface.
- /src/crosshook-native/src/hooks/useInstallGame.ts: Install form and async state orchestration.
- /src/crosshook-native/src/types/install.ts: Frontend contract for install request/result.

### Files to Modify

- /src/crosshook-native/src/components/ProfileEditor.tsx: Add sub-tab navigation and mount the install panel.
- /src/crosshook-native/src/hooks/useProfile.ts: Add a helper or flow to load/generated-profile results into existing profile state.
- /src/crosshook-native/src/App.tsx: Support install-result handoff if app-level state is the cleanest boundary.
- /src/crosshook-native/src-tauri/src/lib.rs: Register install commands.
- /src/crosshook-native/src-tauri/src/commands/mod.rs: Export install command module.
- /src/crosshook-native/crates/crosshook-core/src/lib.rs: Export install domain.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Extract reusable direct-Proton runtime helpers.
- /src/crosshook-native/crates/crosshook-core/src/settings/recent.rs: Optional follow-on if installer media recents are included.

## Code Conventions

### Naming

- Rust files and modules: `snake_case`.
- Tauri command files: one file per feature domain under `src-tauri/src/commands/`.
- React components: `PascalCase`, one component per file.
- Hooks: `camelCase` with `use` prefix.
- TypeScript data contracts: explicit interfaces/types, no `any`.

### Error Handling

- Shared Rust returns typed errors internally, mapped to `String` only at the Tauri boundary.
- Frontend stores one visible error string and preserves form state after failures.
- Long-running process work logs through `tracing` and/or streamed log file lines instead of silent failures.

### Testing

- Rust unit tests are colocated with domain code.
- File-backed behaviors use `tempfile::tempdir()`.
- Process-heavy runtime code is tested via command/environment/output assertions rather than full end-to-end launches.

## Dependencies and Services

### Available Utilities

- Proton discovery via `list_proton_installs` and `crosshook_core::steam::discover_compat_tools`.
- Profile persistence via `ProfileStore`.
- Recent file persistence via `RecentFilesStore`.
- Log creation and async process spawn patterns in `commands/launch.rs`.
- Working-directory and environment assembly helpers in `launch/script_runner.rs`.

### Required Dependencies

- No new package dependency is obviously required for v1.
- `directories` already covers XDG path resolution.
- `tokio::process` already covers process spawning and log piping.

## Gotchas and Warnings

- `launch/request.rs` assumes the runtime target is already known; install cannot be modeled as a normal `LaunchRequest`.
- `ProfileEditor.tsx` is already large, so the install panel should be split into its own component rather than expanding the file unchecked.
- Do not make the detected-Proton field non-editable after selection; that is a recorded project lesson.
- Gamepad/global key handling must not break typing inside the new install form controls.
- The final save boundary must stay clear: installer media cannot leak into `game.executable_path`.

## Task-Specific Guidance

- **For backend tasks**: prefer moving shared direct-Proton logic into small reusable helpers before building the install service.
- **For Tauri tasks**: keep install commands narrow and typed; avoid embedding heuristics or persistence logic in `commands/install.rs`.
- **For UI tasks**: build the install tab as a sibling component mounted by `ProfileEditor.tsx`, with its own hook for async/install state.
- **For review/save tasks**: route the generated profile back through existing profile state machinery so the app keeps one source of truth for saved profiles.
