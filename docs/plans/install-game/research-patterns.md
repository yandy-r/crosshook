# Pattern Research: install-game

## Architectural Patterns

**Domain-oriented Rust modules**: Shared backend code is organized by feature/domain under `crosshook-core/src/` with one top-level module per area such as `launch`, `profile`, `settings`, and `steam`. Example: `/src/crosshook-native/crates/crosshook-core/src/lib.rs`.

**Thin Tauri command adapters**: IPC command files stay narrow and delegate business logic to shared Rust code, returning `Result<T, String>` to the frontend. Examples: `/src/crosshook-native/src-tauri/src/commands/profile.rs`, `/src/crosshook-native/src-tauri/src/commands/steam.rs`, `/src/crosshook-native/src-tauri/src/commands/launch.rs`.

**Hook-owned frontend state**: React components stay mostly declarative while hooks own normalization, side effects, and persistence calls. Example: `/src/crosshook-native/src/hooks/useProfile.ts`.

**Mode-driven conditional UI**: The UI renders sections based on the resolved launch mode rather than splitting each mode into separate screens. Example: `/src/crosshook-native/src/components/ProfileEditor.tsx`.

**Shared typed contracts across layers**: Frontend types mirror Rust models closely, especially for profiles and launch requests, keeping the Tauri boundary explicit and easy to reason about. Examples: `/src/crosshook-native/src/types/profile.ts`, `/src/crosshook-native/src/types/launch.ts`, `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`, `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`.

## Code Conventions

- Rust modules use `snake_case` filenames and feature-domain grouping, with one Tauri command file per domain. Examples: `/src/crosshook-native/src-tauri/src/commands/steam.rs`, `/src/crosshook-native/src-tauri/src/commands/profile.rs`.
- React components use `PascalCase` one-component-per-file naming. Examples: `/src/crosshook-native/src/components/ProfileEditor.tsx`, `/src/crosshook-native/src/components/AutoPopulate.tsx`.
- Hooks use `camelCase` and the `use*` prefix. Example: `/src/crosshook-native/src/hooks/useProfile.ts`.
- TypeScript avoids `any` and prefers explicit interfaces and union types. Examples: `/src/crosshook-native/src/types/profile.ts`, `/src/crosshook-native/src/types/launch.ts`.
- UI styling currently favors colocated `CSSProperties` objects for complex components and shared class-based theme/layout utilities for app-level structure. Examples: `/src/crosshook-native/src/components/ProfileEditor.tsx`, `/src/crosshook-native/src/styles/theme.css`.
- Repeated small UI primitives are extracted as inner helpers when they are truly local to a component. Examples: `FieldRow`, `ProtonPathField`, `chooseFile`, and `chooseDirectory` inside `/src/crosshook-native/src/components/ProfileEditor.tsx`.

## Error Handling

- Tauri commands return `Result<_, String>` and map domain errors with `.to_string()` or local `map_err` helpers. Examples: `/src/crosshook-native/src-tauri/src/commands/profile.rs`, `/src/crosshook-native/src-tauri/src/commands/launch.rs`.
- Shared Rust code uses typed error enums with `Display` implementations and specific validation cases rather than generic strings. Examples: `ValidationError` in `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`, `ProfileStoreError` in `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`.
- Frontend hooks and components store a single user-visible error string and preserve form state on failure instead of resetting fields. Example: `/src/crosshook-native/src/hooks/useProfile.ts`.
- Async/background work uses explicit logging through `tracing` or streamed log lines rather than silent failures. Examples: `/src/crosshook-native/src-tauri/src/commands/launch.rs`, `/src/crosshook-native/src-tauri/src/commands/steam.rs`.
- Validation errors are phrased as actionable user messages, not low-level exception text. Example: `ValidationError::message()` in `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`.

## Testing Approach

- Rust domain code is tested inline with focused unit tests colocated under `#[cfg(test)]`. Examples: `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`, `/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`, `/src/crosshook-native/crates/crosshook-core/src/settings/recent.rs`.
- File-backed persistence is tested with `tempfile::tempdir()` and isolated per-test base paths. Example: `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`.
- Runtime/process code is tested by asserting command construction, environment variables, and staged file outputs rather than doing heavyweight end-to-end execution. Example: `/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`.
- Frontend has no dedicated test framework in the repo right now, so confidence currently comes from typed contracts plus build verification. That means install-game should push as much logic as possible into Rust or pure hook/helpers where behavior can be reasoned about and checked.

## Patterns to Follow

- **Add a new feature domain instead of extending unrelated modules**: Create `/src/crosshook-native/crates/crosshook-core/src/install/` and `/src/crosshook-native/src-tauri/src/commands/install.rs` rather than overloading `launch.rs` or `profile.rs`. Follow the module boundary pattern in `/src/crosshook-native/crates/crosshook-core/src/lib.rs` and `/src/crosshook-native/src-tauri/src/commands/mod.rs`.
- **Keep Tauri commands thin**: Put validation, prefix resolution, executable discovery, and profile generation in shared Rust code. Use `/src/crosshook-native/src-tauri/src/commands/launch.rs` as the command-shape pattern, not as a place to embed install business logic.
- **Reuse the editable detected-path selector pattern**: The install tab should follow the same detection-plus-manual-edit contract as `ProtonPathField` in `/src/crosshook-native/src/components/ProfileEditor.tsx`.
- **Return reviewable typed results, not ambiguous side effects**: Mirror the launch command pattern by returning structured results with status and log paths. Use `/src/crosshook-native/src-tauri/src/commands/launch.rs` as the result-shape reference.
- **Use typed validation errors in Rust first**: Create install-specific validation errors with human-readable messages following `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`.
- **Favor backend path resolution for canonical defaults**: Default prefix root and slug rules should live in Rust, similar to how profile storage roots are determined in `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`.
- **Test install logic at the domain layer**: Add inline Rust tests around prefix creation, executable ranking, and generated profile output rather than trying to cover the feature first through frontend-only checks.
