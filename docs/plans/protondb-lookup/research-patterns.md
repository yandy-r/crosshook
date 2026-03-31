# Pattern Research: protondb-lookup

## Architectural Patterns

**Core-Owned Business Logic**: Feature logic lives in `crosshook-core`, while `src-tauri` only exposes thin commands.

- Example: /src/crosshook-native/src-tauri/src/commands/version.rs delegates version work to `crosshook_core::metadata` and `crosshook_core::steam`

**Reusable Metadata Store Helpers**: feature state is persisted through shared `MetadataStore` methods rather than feature-local DB code in Tauri.

- Example: /src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

**Invoke + Hook Frontend Pattern**: the frontend wraps Tauri commands in dedicated hooks and typed TS contracts instead of calling `invoke()` ad hoc inside many components.

- Example: /src/crosshook-native/src/hooks/useProfileHealth.ts

**Feature-Local UI Surface**: new functionality is usually introduced as a dedicated component that is then composed into pages/forms.

- Example: /src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx

**Shared Badge Styling via Theme Classes**: semantic badges use dedicated modifier classes in `theme.css`.

- Example: /src/crosshook-native/src/components/CompatibilityViewer.tsx and /src/crosshook-native/src/styles/theme.css

## Code Conventions

- Rust modules use `snake_case` paths and keep DTOs Serde-ready when they cross IPC boundaries.
- Tauri commands use `#[tauri::command]` with `snake_case` names matching frontend `invoke()` calls.
- React components are `PascalCase`, hooks are `camelCase`, and typed frontend contracts live under `src/types/`.
- Shared CSS classes follow the `crosshook-*` naming scheme with modifier suffixes such as `--working`.

## Error Handling

- Backend layers return typed `Result<_, String>` at the Tauri boundary and richer project errors inside `crosshook-core`.
- Soft-failure UI states are preferred for advisory or background information.
- Existing remote-like flows avoid crashing the page when data is unavailable and instead render degraded states or cached snapshots.

## Testing Approach

- Rust feature logic is usually tested directly inside the module or adjacent integration tests.
- Parser/IO helpers use `tempfile` and deterministic fixtures rather than external live dependencies.
- The frontend relies on TypeScript compile validation and manual UI verification because there is no configured frontend test runner.

## Patterns to Follow

- Reuse `MetadataStore::get_cache_entry` / `put_cache_entry` before considering a new table or migration.
- Keep exact ProtonDB tiers in a dedicated feature contract rather than retrofitting `CompatibilityRating`.
- Model remote lookup state as `idle / loading / ready / stale / unavailable` instead of treating it like validation failure.
- Keep recommendation application explicit and merge into existing profile fields instead of inventing a raw “launch options” field on the first pass.
