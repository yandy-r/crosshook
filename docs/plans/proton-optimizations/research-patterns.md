# Pattern Research: proton-optimizations

## Architectural Patterns

**Hook-driven state with thin presentation components**: CrossHook keeps long-lived editor and launch state in hooks, then passes compact props into presentational components.

- Example: /src/crosshook-native/src/hooks/useProfile.ts
- Example: /src/crosshook-native/src/hooks/useLaunchState.ts
- Example: /src/crosshook-native/src/components/LaunchPanel.tsx

**Typed frontend/backend model mirroring**: Profile and launch request shapes are defined in TypeScript and mirrored in Rust with Serde-backed structs, which is the right pattern for adding `launch.optimizations` without ad hoc serialization.

- Example: /src/crosshook-native/src/types/profile.ts
- Example: /src/crosshook-native/src/types/launch.ts
- Example: /src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- Example: /src/crosshook-native/crates/crosshook-core/src/launch/request.rs

**Thin Tauri command layer over core business logic**: IPC commands in `src-tauri` mostly validate inputs, translate errors to strings, and delegate to `crosshook-core` or persistence types.

- Example: /src/crosshook-native/src-tauri/src/commands/profile.rs
- Example: /src/crosshook-native/src-tauri/src/commands/launch.rs

**Backend-owned launch orchestration**: Actual process construction happens in Rust launch modules, not in React. The frontend builds a typed `LaunchRequest`, and the backend resolves the launch method, validation, and command invocation.

- Example: /src/crosshook-native/src/App.tsx
- Example: /src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- Example: /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs

**Shared form section extraction for large editors**: Reusable field groups are pulled into helper components/functions instead of duplicating form logic across screens.

- Example: /src/crosshook-native/src/components/ProfileFormSections.tsx

**Environment isolation and explicit allowlists**: Proton and runtime launch behavior is intentionally built from `env_clear()` plus explicit rehydration, so new launch options need deliberate backend mapping instead of relying on ambient shell state.

- Example: /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs
- Example: /src/crosshook-native/crates/crosshook-core/src/launch/env.rs

## Code Conventions

TypeScript uses `PascalCase` for components and `camelCase` for hooks/functions, with narrow interfaces rather than loose objects. Nested state updates are done immutably with updater callbacks, as seen in `useProfile.ts` and `ProfileFormSections.tsx`. The codebase prefers explicit string unions like `LaunchMethod` over loose string handling, then normalizes unresolved values centrally (`resolveLaunchMethod`) rather than scattering fallback logic.

Frontend composition favors one hook per domain concern and one component per surface area. `LaunchPanel.tsx` is intentionally presentational and reads everything it needs from `useLaunchState`, while `App.tsx` handles layout-level composition and passes the typed `LaunchRequest`. New UI for proton optimizations should follow that split: a dedicated panel component plus either a focused autosave hook or a minimal extension of `useProfile.ts`, not extra logic embedded in `App.tsx`.

Rust follows `snake_case`, small modules, and Serde-first IPC types. Tauri command functions are named to match frontend `invoke()` calls, return `Result<_, String>`, and keep mapping logic thin. Core structs use `#[serde(default)]` and selective `skip_serializing_if` rules, which is the right convention for making `launch.optimizations` backward-compatible and TOML-compact.

## Error Handling

Frontend hooks generally catch `invoke()` failures, convert them into displayable strings, and keep them in local state instead of throwing through the tree.

- `useProfile.ts` stores `error: string | null` and returns `{ ok: false, error }` from persistence helpers.
- `useLaunchState.ts` reduces launch failures into state transitions with explicit fallback phases.

This means proton-optimizations should follow the existing pattern: autosave and validation failures should become panel-local state such as `saveStatus` or `errorMessage`, not global exceptions.

Backend Rust uses explicit validation enums and `Display` implementations for user-facing messages.

- Example: /src/crosshook-native/crates/crosshook-core/src/launch/request.rs defines `ValidationError` and a `message()` method.
- Example: /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs defines `ProfileStoreError` and converts IO/TOML failures cleanly.

The codebase does not silently swallow launch-time correctness issues. Validation happens before process spawn (`validate_launch` in `/src/crosshook-native/src-tauri/src/commands/launch.rs`), and command-construction errors are surfaced immediately with contextual strings such as `failed to build Proton game launch: ...`. For this feature, unknown optimization IDs, incompatible wrapper combinations, and missing host tools should follow that same fail-fast model.

## Testing Approach

Most meaningful tests for this feature belong in Rust unit tests, not the frontend. The repo already has a pattern of colocated unit tests inside the affected Rust modules.

- Example: /src/crosshook-native/crates/crosshook-core/src/launch/env.rs tests environment allowlists.
- Example: /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs tests save/load/delete and legacy import round trips.
- Example: /src/crosshook-native/src-tauri/src/commands/profile.rs tests launcher cleanup behavior tied to profile lifecycle.

There is no established frontend unit-test harness in the repo, so implementation should lean on pure helper functions that can be tested in Rust or manually verified in the Tauri UI. For proton-optimizations, the strongest test targets are:

- TOML round-trip of `launch.optimizations.enabled_option_ids`
- validation of unknown or incompatible optimization IDs
- wrapper ordering and command construction in the `proton_run` path
- preservation of explicit environment isolation when adding new env vars

## Patterns to Follow

Follow the existing typed mirror pattern when extending the model: update `/src/crosshook-native/src/types/profile.ts`, `/src/crosshook-native/src/types/launch.ts`, `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`, and `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs` together so the IPC contract stays aligned.

Follow the thin-command pattern in Tauri: add a narrow IPC command for section-only optimization persistence in `/src/crosshook-native/src-tauri/src/commands/profile.rs`, but keep merge/save logic in `ProfileStore`-adjacent Rust code instead of embedding business rules directly in the command handler.

Follow the hook-plus-panel composition pattern on the frontend: the new optimization UI should be a dedicated component placed alongside `LaunchPanel`, while autosave timing and persistence should live in a hook or narrowly scoped state helper rather than inside the JSX tree.

Follow the existing immutable nested-update style from `/src/crosshook-native/src/components/ProfileFormSections.tsx` when mutating profile state, but avoid piggybacking on the current heavy `persistProfileDraft()` loop for checkbox autosave because that helper refreshes metadata, reloads profiles, and is optimized for explicit Save, not rapid panel interaction.

Follow the backend-owned launch resolution pattern: React should send stable option IDs only, and a new Rust launch module should translate them into env vars and wrappers before `build_proton_game_command()` / `build_proton_trainer_command()` spawn anything. That matches current separation of concerns and keeps quoting, ordering, validation, and host-tool checks out of the UI.
