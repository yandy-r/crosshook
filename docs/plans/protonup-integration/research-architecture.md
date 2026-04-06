# Architecture Research: protonup-integration

## System Overview

CrossHook follows a layered architecture: `crosshook-core` owns business logic, `src-tauri` exposes thin IPC commands, and the React frontend consumes typed hooks/components. Proton runtime discovery already exists in `crosshook-core/src/steam/proton.rs`, while profile/community metadata and settings are split across SQLite metadata and TOML settings. ProtonUp integration fits as a new core service module that plugs into existing Steam discovery, metadata cache, and profile/community recommendation surfaces.

## Relevant Components

- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: Proton install discovery and runtime normalization logic.
- `/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs`: Steam root candidate detection used for install target resolution.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `external_cache_entries` cache read/write API for available-version catalogs.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` facade and connection handling.
- `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: TOML settings model (`AppSettingsData`) for user-editable preferences.
- `/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`: community profile metadata including `proton_version`.
- `/src/crosshook-native/src-tauri/src/commands/steam.rs`: existing proton-list command pattern (`list_proton_installs`).
- `/src/crosshook-native/src-tauri/src/lib.rs`: command registration surface for new protonup IPC handlers.
- `/src/crosshook-native/src/hooks/useProtonInstalls.ts`: frontend hook pattern for Proton list retrieval and refresh.
- `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: profile runtime selection and missing-version UX integration point.
- `/src/crosshook-native/src/components/pages/CompatibilityPage.tsx`: compatibility guidance/suggestion UI integration point.

## Data Flow

User actions originate in frontend pages/hooks and invoke Tauri commands. Tauri commands call core services that discover local runtimes (filesystem scan), fetch/cache available releases (remote metadata to SQLite cache), and compute recommendation/match status using community metadata plus local installs. Results flow back as Serde DTOs to hooks, then UI renders advisory status, install controls, progress, and recovery messaging.

## Integration Points

Primary integration belongs in a new `crosshook-core` protonup domain module (catalog, install, match). Existing proton discovery (`steam/proton.rs`) should remain source of truth for installed runtime enumeration. Tauri should gain new `protonup_*` command wrappers, while frontend extends existing Proton hooks/UI rather than introducing an isolated parallel state system.

## Key Dependencies

- Existing Rust deps: `serde`, `reqwest`, `rusqlite` usage patterns in core.
- Candidate external dependency: `libprotonup` (or fallback CLI adapter strategy).
- Existing persistence surfaces: SQLite `external_cache_entries` and TOML settings.
- Existing architecture constraints from `AGENTS.md`: core-first business logic, snake_case command naming, Serde boundary types.
