# Architecture Research: protonup-integration

## System Overview

CrossHook is a Tauri v2 native Linux desktop app with a strict three-layer architecture: business logic in `crosshook-core` (Rust crate), a thin Tauri IPC layer in `src-tauri` that wraps core logic as `#[tauri::command]` handlers, and a React 18 + TypeScript frontend that calls `invoke()` wrapped in custom hooks. The `libprotonup = "0.11.0"` dependency is already declared in `crosshook-core/Cargo.toml` but unused — this is the primary integration point. No new crate dependencies are needed for the feature.

## Relevant Components

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml`: Declares `libprotonup = "0.11.0"` — already present but unused; also `reqwest`, `sha2`, `nix`, `rusqlite`, `tokio` (all needed)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Module registry — new `protonup` module must be declared here (`pub mod protonup;` is absent despite the directory existing)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: `discover_compat_tools()` / `discover_compat_tools_with_roots()` — the installer scan function that protonup-integration must call after installation to refresh the available Proton list
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/mod.rs`: Exports `ProtonInstall`, `discover_compat_tools`, `discover_steam_root_candidates` — types and discovery used by both the existing `list_proton_installs` command and the new protonup feature
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Currently at schema version 18 (v18); the external_cache_entries table (added in migration 3-to-4) is the TTL cache store for version lists — no new tables needed for phase 1
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` struct wrapping `Arc<Mutex<Connection>>`; exposes `get_cache_entry` / `put_cache_entry` (delegates to `cache_store::`); must be injected as `State<'_, MetadataStore>` in new commands
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `AppSettingsData` struct (TOML-backed) — steam client path is NOT stored here; add `protonup_install_directory` field only if a user-configurable install path override is required
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: Tauri `Builder` — all new `#[tauri::command]` functions must be registered in `invoke_handler!(...)` here; all managed state registered via `.manage(...)` at startup
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs`: Command module registry — new `protonup.rs` command file must be declared here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs`: `list_proton_installs` command and `default_steam_client_install_path()` helper — steam path resolution reads `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var or discovers from filesystem; protonup commands must call this function or accept path as parameter, not read from settings
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/prefix_deps.rs`: **Closest structural parallel** — background async install with progress streaming via `app_handle.emit()`, managed state lock (`PrefixDepsInstallState`), `#[tauri::command]` pattern with `AppHandle + State` injection
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/update.rs`: `UpdateProcessState` — PID-tracking pattern (`Mutex<Option<u32>>`) for cancellable processes; use this if protonup needs cancel support
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonInstalls.ts`: Frontend hook for installed Proton versions — must expose a `reload()` trigger that protonup feature can call after successful install
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/proton.ts`: Currently only `ProtonInstallOption` — new types for `ProtonUpRelease`, `ProtonUpInstallProgress`, etc. go here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/settings.ts`: `AppSettingsData` / `SettingsSaveRequest` frontend mirrors — update when Rust settings struct changes
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/context/PreferencesContext.tsx`: App-wide settings context — `settings.default_proton_path` is the primary field the protonup installer will write to on success
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/Sidebar.tsx`: `AppRoute` union type and `SIDEBAR_SECTIONS` array — protonup panel is likely embedded in Settings or as a subsection, not a new top-level route
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/ContentArea.tsx`: Route-to-page mapping — only modified if a new top-level route is added (unlikely for protonup)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useScrollEnhance.ts`: **Critical gotcha** — any new scroll container with `overflow-y: auto` MUST be added to the `SCROLLABLE` selector here or dual-scroll jank occurs
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/SettingsPanel.tsx`: 49k file, primary settings UI component — most likely home for a ProtonUp management section (as a sub-panel similar to `PrefixDepsPanel`)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/PrefixDepsPanel.tsx`: **Closest UI structural parallel** — background install with live log streaming, managed install lock, progress feedback — study for protonup UI pattern

## Data Flow

**Version list fetch (TTL-cached):**

1. Frontend calls `invoke('protonup_list_releases', { variant: 'ge-proton' | 'wine-ge' })`
2. Tauri command (`src-tauri/src/commands/protonup.rs`) calls `crosshook_core::protonup::list_releases()`
3. Core function checks `MetadataStore::get_cache_entry(cache_key, ttl)` — if fresh, returns cached JSON
4. On cache miss, calls `libprotonup::downloads::list_releases()` (wraps GitHub Releases API), serializes result, stores in `external_cache_entries` with TTL
5. Result propagates back through IPC as `Vec<ProtonUpRelease>` serialized via Serde

**MetadataStore mutex discipline (critical):**

- `MetadataStore` wraps `Arc<Mutex<Connection>>` — the lock must NOT be held while awaiting download operations
- Correct pattern (from `protondb/client.rs`): read cache → release lock → do async network work → re-acquire to write result
- Holding the mutex across an async download will deadlock other commands that need the DB

**Installation with progress streaming:**

1. Frontend calls `invoke('protonup_install_release', { tag: '...', variant: '...', install_path: '...' })`
2. Tauri command acquires `ProtonUpInstallState` lock (prevents concurrent installs)
3. Core async function spawns download stream from `libprotonup`; progress bytes emitted via `app_handle.emit("protonup-install-progress", payload)`
4. Frontend `useProtonUpInstall` hook listens via `listen('protonup-install-progress', ...)` — mirrors `prefix-dep-log` pattern
5. On completion, emits `protonup-install-complete` with `{ succeeded, tag, install_path }`
6. Frontend hook triggers `useProtonInstalls.reload()` to refresh available Proton list
7. If `settings.default_proton_path` is empty, optionally calls `persistSettings({ default_proton_path: install_path })` via `PreferencesContext`

**Steam path resolution (not from settings):**

- Steam client path is NOT stored in `AppSettingsData`
- Protonup commands must call `default_steam_client_install_path()` internally (reads `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var, then filesystem discovery) or accept it as a command parameter
- Install target (`~/.steam/root/compatibilitytools.d/`) is derived from the resolved Steam root, not from any setting

**Installed versions scan:**

- `discover_compat_tools` (steam module) → rich `ProtonInstall` structs with aliases — used for profile path resolution
- `libprotonup::list_installed_versions()` → `Vec<String>` — used for ProtonUp management UI display
- These two scanners serve different purposes and should remain separate

## Integration Points

**New Rust module (`crosshook-core`):**

- Directory already exists: `src/crosshook-native/crates/crosshook-core/src/protonup/` (empty)
- Add `mod.rs` (and optionally `models.rs`, `service.rs`, `client.rs` per domain module topology)
- Declare in `src/crosshook-native/crates/crosshook-core/src/lib.rs`: `pub mod protonup;`
- Module owns: release listing (TTL-cached via MetadataStore), disk space pre-check (nix statvfs), streaming install/verify/extract via `libprotonup`

**New Tauri command file:**

- Create `src/crosshook-native/src-tauri/src/commands/protonup.rs`
- Declare in `src/crosshook-native/src-tauri/src/commands/mod.rs`: `pub mod protonup;`
- Register commands in `src/crosshook-native/src-tauri/src/lib.rs` `invoke_handler!` array

**Managed state:**

- Add `ProtonUpInstallState` to `src-tauri/src/lib.rs` `.manage(...)` chain
- For simple lock-only (no cancel): `Mutex<bool>` or `PrefixDepsInstallLock` pattern
- For cancellable installs: `Mutex<Option<tokio::sync::oneshot::Sender<()>>>` pattern (see `UpdateProcessState`)

**Frontend new files:**

- `src/crosshook-native/src/components/ProtonUpManagerPanel.tsx` — list releases, install controls, progress display
- `src/crosshook-native/src/hooks/useProtonUpReleases.ts` — invoke wrapper for release list + loading state
- `src/crosshook-native/src/hooks/useProtonUpInstall.ts` — invoke + event listener for install progress (mirrors `usePrefixDeps.ts`)
- `src/crosshook-native/src/types/protonup.ts` — `ProtonUpRelease`, `ProtonUpInstallProgress`, `ProtonUpInstallResult` types

**Existing files modified:**

- `src/crosshook-native/src/components/SettingsPanel.tsx`: Add `<ProtonUpManagerPanel />` section
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: Add any new scroll containers to `SCROLLABLE` selector
- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: Add `pub mod protonup;`
- `src/crosshook-native/src-tauri/src/commands/mod.rs`: Add `pub mod protonup;`
- `src/crosshook-native/src-tauri/src/lib.rs`: Register new commands + manage new state

**DB schema decision:**

- Phase 1: No new migration needed — `external_cache_entries` (migration 3→4) is sufficient for TTL-caching release lists
- Cache key convention (to standardize): `github:proton-releases:GEProton` / `github:proton-releases:WineGE`
- Optional schema v19: Add `installed_proton_versions` table only if install history/dates UX is required without filesystem I/O; defer to later phase

## Key Dependencies

**Rust (all already in crosshook-core/Cargo.toml):**

- `libprotonup = "0.11.0"` — release listing, streaming download, SHA-512 verification, tar extraction
- `reqwest = "0.13.2"` — HTTP client with rustls (used by libprotonup internally)
- `sha2 = "0.11.0"` — SHA-512 verification (used by libprotonup)
- `nix = "0.31.2"` — `statvfs()` for pre-download disk space check
- `rusqlite = "0.39.0"` — MetadataStore for `external_cache_entries` TTL cache
- `tokio = "1"` with `sync` feature — async Mutex for install lock

**Frontend (all already in package.json):**

- `@tauri-apps/api` — `invoke()` and `listen()` for IPC and event streaming
- React 18 hooks — `useState`, `useEffect`, `useCallback` for install state management

**Internal modules consumed:**

- `crosshook_core::steam::discover_compat_tools` — post-install scan to verify installation
- `crosshook_core::steam::default_steam_client_install_path` — steam root resolution for install target
- `crosshook_core::metadata::MetadataStore` — TTL cache for release lists (avoid redundant GitHub API calls)
- `crosshook_core::settings::AppSettingsData` — `default_proton_path` field updated on install

## Architectural Gotchas

- **`protonup/` directory exists but is empty and undeclared** — `crosshook-core/src/protonup/` exists as an empty directory; `pub mod protonup;` is absent from `lib.rs`; first implementation commit must add both `mod.rs` content and the `lib.rs` declaration together or the build will fail
- **Steam client path is NOT in settings** — `AppSettingsData` has no steam path field; protonup commands must call `default_steam_client_install_path()` (from `commands/steam.rs`) or accept path as parameter — do not attempt to read it from the settings store
- **MetadataStore mutex must not span async awaits** — pattern is: lock → read cache → unlock → await network → lock → write cache → unlock; holding across download will deadlock other commands
- **`discover_compat_tools` and `libprotonup::list_installed_versions` serve different purposes** — keep them separate; the steam module's rich `ProtonInstall` structs (with aliases) are for profile path resolution; libprotonup's version list is for the ProtonUp management UI
- **No protonup frontend component or hook exists** — `src/components/protonup/` does not exist; `useProtonUpInstall` / `useProtonUpReleases` do not exist; this is entirely net-new frontend work
- **`PrefixDepsPanel` + `prefix_deps.rs` command are the implementation template** — the progress streaming pattern (emit log lines, emit complete event, acquire lock) is established there and should be followed exactly
- **Schema v18 is current** — any SQLite addition requires a new `migrate_17_to_18` → `migrate_18_to_19` function; for phase 1, no migration is needed
- **Scroll containers require `useScrollEnhance` registration** — any `overflow-y: auto` div in the protonup panel must be added to the `SCROLLABLE` selector in `useScrollEnhance.ts`
- **SettingsPanel is 49k** — it is the most likely integration point for the protonup UI section; adding a new section follows the existing pattern of sub-panel components like `PrefixDepsPanel`
- **`AppRoute` is a discriminated union** — adding a new top-level route requires updating `Sidebar.tsx`, `ContentArea.tsx`, and `App.tsx`; embedding in Settings avoids this
- **MetadataStore can be disabled** — it initializes with `MetadataStore::disabled()` on SQLite failure; protonup cache must gracefully degrade to always-fetch when `metadata_store.available` is false
- **`libprotonup` install target directory** — the library installs to `~/.steam/root/compatibilitytools.d/` by default; this directory is already scanned by `discover_compat_tools_with_roots()` so installed versions will appear automatically after `useProtonInstalls.reload()`
