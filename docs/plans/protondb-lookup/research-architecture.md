# Architecture Research: protondb-lookup

## System Overview

CrossHook’s native architecture is split cleanly across three layers: `crosshook-core` for business logic, `src-tauri` for thin IPC/desktop orchestration, and the React frontend in `src/crosshook-native/src` for stateful editor UI. ProtonDB lookup belongs in the same path as Steam discovery and version correlation: fetch and cache logic in `crosshook-core`, a thin Tauri command in `src-tauri`, and a profile-editor card driven by a frontend hook. The feature plugs into the existing profile editor and metadata SQLite store rather than introducing a second data path or a browser-side network client.

## Relevant Components

- /src/crosshook-native/crates/crosshook-core/src/lib.rs: top-level module exports for new core feature registration
- /src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: shared metadata-store API surface, including cache helpers
- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: reusable external JSON cache keyed by stable strings
- /src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: existing compatibility enum that currently collapses ProtonDB-like states
- /src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs: current Steam App ID/build lookup path and parsing conventions
- /src/crosshook-native/src-tauri/src/commands/steam.rs: thin Steam-related command pattern to mirror
- /src/crosshook-native/src-tauri/src/commands/version.rs: thin metadata-backed Tauri command pattern to mirror
- /src/crosshook-native/src-tauri/src/commands/mod.rs: command module registration surface
- /src/crosshook-native/src-tauri/src/lib.rs: global invoke handler and shared app-state registration
- /src/crosshook-native/src/components/ProfileFormSections.tsx: profile-editor form sections, including Steam App ID and custom env editing
- /src/crosshook-native/src/components/pages/ProfilesPage.tsx: selected-profile wiring and health/version context
- /src/crosshook-native/src/styles/theme.css: compatibility badge and panel styling surface

## Data Flow

The selected profile flows from `useProfile` into `ProfilesPage`, then down into `ProfileFormSections` alongside derived Steam context such as the selected Steam App ID and trainer version metadata. Existing remote-like intelligence already follows a backend-first path: `src-tauri` commands call `crosshook-core`, while `MetadataStore` persists version and cache snapshots in SQLite. ProtonDB lookup should follow the same route: the frontend sends only the App ID and refresh intent to a Tauri command, `crosshook-core` resolves cached-or-live data, and the frontend renders a typed advisory state without owning any network or persistence logic.

## Integration Points

- New `crosshook-core::protondb` module for remote fetch, normalization, and cache reuse
- New Tauri command, likely `protondb_lookup`, registered alongside existing `steam` and `version` commands
- New frontend hook mirroring the invoke-driven state style used by existing profile-health and version workflows
- New editor card placed near Steam App ID / Auto-Populate inside `ProfileFormSections`
- Existing `launch.custom_env_vars` and `SteamLaunchOptionsPanel` flows for explicit recommendation apply/copy actions

## Key Dependencies

- Metadata SQLite store (`MetadataStore`) for cache persistence and offline fallback
- Steam App ID already stored in `GameProfile.steam.app_id`
- Existing React profile state in `useProfile` and `ProfileContext`
- Potential new Rust HTTP dependency in `crosshook-core`
- Existing CSS class system using `crosshook-*` prefixes for panel/badge theming
