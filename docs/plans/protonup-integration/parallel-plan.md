# ProtonUp Integration Implementation Plan

ProtonUp integration adds in-app Proton/Wine version management to CrossHook by activating the declared `libprotonup = "0.11.0"` crate within the established three-layer architecture: new `crosshook-core/src/protonup/` domain module (fetcher, installer, scanner, advisor), thin Tauri IPC wrappers in `src-tauri/src/commands/protonup.rs`, and a React `ProtonVersionManager` component embedded in SettingsPanel. The implementation follows the existing `PrefixDepsPanel` + `prefix_deps.rs` event-streaming pattern for download progress, caches GitHub release data in the existing `external_cache_entries` SQLite table (schema v18, no migration needed), and embeds three ship-blocking security mitigations (path traversal validation, archive bomb caps, Cargo.lock verification) directly in `installer.rs`. A GPL-3.0 licensing gate must be resolved before Phase 1 begins -- Option B (direct `reqwest`/`flate2`/`tar`, all MIT-clean) is the fallback.

## Critically Relevant Files and Documentation

- docs/plans/protonup-integration/feature-spec.md: Authoritative spec -- business rules BR-1 through BR-13, data models, 5 Tauri command contracts, 3-phase breakdown, resolved decisions
- docs/plans/protonup-integration/research-security.md: 3 CRITICAL findings (CVE chain, path traversal, archive bomb) with verified mitigation code patterns -- ship-blocking
- docs/plans/protonup-integration/research-external.md: Complete `libprotonup 0.11.0` API surface, GitHub Releases API spec, integration code patterns
- docs/plans/protonup-integration/research-practices.md: Reusable module inventory with exact file paths and line numbers; KISS assessment; rule to copy (not abstract) the cache-first pattern
- docs/plans/protonup-integration/research-recommendations.md: GPL-3.0 blocker analysis, Option A vs B vs C, risk table with mitigations
- docs/plans/protonup-integration/research-technical.md: Rust type definitions, full API contracts, architecture decisions
- docs/plans/protonup-integration/research-ux.md: Component vocabulary, user flows, API-to-UX binding, accessibility requirements
- AGENTS.md: Binding architecture rules, IPC naming, directory map, persistence classification
- CLAUDE.md: `useScrollEnhance` scroll registration, commit/PR conventions, label taxonomy
- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs: Cache-first fetch pattern (lines 85-130) -- copy verbatim for fetcher.rs
- src/crosshook-native/src-tauri/src/commands/prefix_deps.rs: AppHandle::emit streaming install pattern (lines 234-320) -- structural template for install command
- src/crosshook-native/crates/crosshook-core/src/install/models.rs: Request/Result/Error triple pattern -- structural template for models.rs
- src/crosshook-native/src/hooks/useUpdateGame.ts: Frontend listen-before-invoke stage machine -- template for useInstallProtonVersion.ts
- src/crosshook-native/src/components/PrefixDepsPanel.tsx: UI install pattern with live progress -- template for ProtonVersionManager.tsx

## Implementation Plan

### Pre-Phase Gate: GPL-3.0 License Resolution

Before any Phase 1 coding begins, resolve whether `libprotonup`'s GPL-3.0 license is compatible with CrossHook's MIT license. If incompatible, all Phase 1 code must use Option B (direct `reqwest`/`flate2`/`tar`). This changes `fetcher.rs` and `installer.rs` implementations entirely. Block Phase 1 on this decision.

### Phase 1: Backend Foundation

#### Task 1A-1: Module declarations and stub `mod.rs` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/lib.rs
- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/crates/crosshook-core/src/install/mod.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/lib.rs
- src/crosshook-native/src-tauri/src/commands/mod.rs

Add `pub mod protonup;` to `crosshook-core/src/lib.rs` alongside the existing module declarations. Add `pub mod protonup;` to `src-tauri/src/commands/mod.rs`. Create `protonup/mod.rs` as a placeholder stub (empty module body or comment) so both declarations compile. This must land first so all downstream tasks can compile independently during development. Follow the module-per-domain pattern from `install/mod.rs`.

#### Task 1A-2: Promote `normalize_alias` to `pub` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs

Change `pub(crate) fn normalize_alias` to `pub fn normalize_alias`. One-token change. This enables the `advisor.rs` module (Task 1D-2) to import it for profile version fuzzy matching. Verify the function signature and any tests that reference it still compile: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

#### Task 1A-3: Implement `protonup/models.rs` and `protonup/error.rs` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/install/models.rs
- src/crosshook-native/crates/crosshook-core/src/update/models.rs
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs
- docs/plans/protonup-integration/feature-spec.md (data models section)
- docs/plans/protonup-integration/research-technical.md (type definitions)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/protonup/models.rs
- src/crosshook-native/crates/crosshook-core/src/protonup/error.rs

Define all Serde types following the Request/Result/Error triple pattern from `install/models.rs`:

**models.rs**: `AvailableProtonVersion` (tag_name, download_url, checksum_url, size_bytes, tool_name), `InstalledProtonVersion` (name, path, aliases -- with `From<ProtonInstall>` impl), `ProtonVersionListCache` (versions vec, fetched_at, cache_key), `ProtonInstallProgress` (version, stage: InstallPhase, percent, message), `ProtonVersionSuggestion` (is_installed, closest_installed, available_version), `VersionChannel` enum (GEProton, WineGE -- Phase 1 uses GEProton only), `InstallPhase` enum (Downloading, Verifying, Extracting). All structs derive `Debug, Clone, Serialize, Deserialize` with `#[serde(default)]` on every field. Enums use `#[serde(rename_all = "snake_case")]`.

**error.rs**: `ProtonupError` enum with variants: `Validation(ProtonupValidationError)`, `NetworkFailed { message: String }`, `ExtractionFailed { message: String }`, `ChecksumMismatch`, `InstallDirPathTraversal`, `AlreadyInstalled`, `DiskSpaceInsufficient`, `AlreadyInstalling`. `ProtonupValidationError` enum: `ToolNameRequired`, `VersionRequired`, `InstallDirRequired`, `InstallDirNotDirectory`. Both implement `.message() -> String` with exhaustive match, `Display` delegating to `.message()`, `From<ProtonupValidationError> for ProtonupError`.

#### Task 1A-4: Add settings field for preferred Proton version Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- src/crosshook-native/src-tauri/src/commands/settings.rs
- src/crosshook-native/src/types/settings.ts

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- src/crosshook-native/src-tauri/src/commands/settings.rs (AppSettingsIpcData mirror)
- src/crosshook-native/src/types/settings.ts (TypeScript mirror)

Add `preferred_proton_version: String` with `#[serde(default, skip_serializing_if = "String::is_empty")]` to `AppSettingsData`. Add the matching field to `AppSettingsIpcData` in `commands/settings.rs`. Add `preferred_proton_version: string` to the TypeScript `AppSettingsData` interface. **Also update the `merge_settings_data` function in `commands/settings.rs` (~line 122) to copy the new field from saved settings to the new value -- otherwise the field is silently zeroed on every save.** Zero migration cost -- `#[serde(default)]` ensures backward compatibility with existing TOML files. This field stores the user's selected default GE-Proton version for new profiles.

#### Task 1B-1: Implement `protonup/fetcher.rs` -- cache-first GitHub fetch Depends on [1A-1, 1A-3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs (lines 85-130 cache pattern, lines 26-40 OnceLock)
- src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs
- src/crosshook-native/crates/crosshook-core/src/offline/network.rs
- docs/plans/protonup-integration/research-external.md (libprotonup API)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/protonup/fetcher.rs

Copy the cache-first fetch pattern from `protondb/client.rs:85-130` verbatim and adapt. Implement:

1. `static PROTONUP_HTTP_CLIENT: OnceLock<reqwest::Client>` singleton with `CrossHook/<version>` user-agent and 10s timeout.
2. `pub async fn list_available_versions(metadata_store: &MetadataStore, channel: &VersionChannel) -> Result<Vec<AvailableProtonVersion>, ProtonupError>`: check `is_network_available()` first; cache key `protonup:versions:v1:ge-proton` with 24h TTL; serve stale cache up to 7 days with age indicator if network fails (BR-4); call `libprotonup::downloads::list_releases(&CompatTool::from_str("GEProton"))` on cache miss; normalize `Release` to `AvailableProtonVersion`; store in `external_cache_entries` via `put_cache_entry(conn, source_url, cache_key, payload, expires_at)` where `source_url` = `"https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases"`.
3. **Stale fallback requires bypassing `expires_at` filter**: The standard `get_cache_entry` only returns non-expired entries. For stale serving, implement a second query path that ignores expiry (mirror the `load_cached_lookup_row(metadata_store, &cache_key, true)` pattern in `protondb/client.rs:111` which passes a `force_refresh` flag to skip the expiry check). Return stale data with a `fetched_at` timestamp so the frontend can display cache age.
4. `MAX_CACHE_PAYLOAD_BYTES` is 512 KiB -- GE-Proton release JSON (~80-150 KB) fits safely.
5. On any `reqwest::Error` with 403 status (rate limited), log `tracing::warn!` and serve stale cache.
6. When `MetadataStore` is disabled (`MetadataStore::disabled()`), always fetch from network -- do not hard-fail.

#### Task 1C-1: Implement `protonup/scanner.rs` -- installed version scanner Depends on [1A-1, 1A-3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs (discover_compat_tools)
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs (ProtonInstall)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/protonup/scanner.rs

Thin wrapper (~30 lines) over existing `steam::proton::discover_compat_tools`. Implement `pub fn list_installed_versions(steam_root_candidates: &[PathBuf]) -> Vec<InstalledProtonVersion>` that calls `discover_compat_tools(steam_root_candidates, &mut diagnostics)` (the public wrapper at `proton.rs:24`, which handles system roots internally) and converts each `ProtonInstall` to `InstalledProtonVersion` via the `From<ProtonInstall>` impl defined in `models.rs`. **Note**: Do NOT call `discover_compat_tools_with_roots` directly -- it takes 3 arguments (steam_root_candidates, system_compat_tool_roots, diagnostics) and is `pub(crate)`. Use the simpler `discover_compat_tools` public wrapper instead. No new filesystem logic -- this task is deliberately minimal. Do NOT replace or duplicate `discover_compat_tools` -- these serve different purposes (profile path resolution vs ProtonUp management UI).

#### Task 1D-1: Implement `protonup/installer.rs` -- download, verify, extract with security Depends on [1A-1, 1A-3]

**READ THESE BEFORE TASK**

- docs/plans/protonup-integration/research-security.md (all 3 CRITICAL mitigations)
- src/crosshook-native/src-tauri/src/commands/prefix_deps.rs (lines 234-320 event emission)
- src/crosshook-native/crates/crosshook-core/src/install/service.rs (validate-then-execute)
- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs (symlink detection)
- docs/plans/protonup-integration/research-external.md (libprotonup download/extract API)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/protonup/installer.rs

Most complex Phase 1 task (~150-200 lines). Implement:

1. `pub fn validate_install_request(request: &ProtonUpInstallRequest) -> Result<(), ProtonupValidationError>`: validate tool_name non-empty, version non-empty, install_dir exists and is a directory. **Security C-2**: canonicalize install_dir, verify it resolves within `$HOME`, verify it ends with `compatibilitytools.d` or a known Steam library path, reject paths containing `..`, use `symlink_metadata()` to verify install_dir is not a symlink (mirror `metadata/db.rs:open_at_path` pattern).

2. `pub async fn install_version(request: &ProtonUpInstallRequest, progress_tx: impl Fn(ProtonInstallProgress)) -> Result<ProtonUpInstallResult, ProtonupError>`: orchestrate the full install flow:
   - Call `validate_install_request` first (validate-then-execute pattern).
   - Idempotency check: if version directory already exists in install_dir, return `AlreadyInstalled`.
   - Pre-flight disk space check via `nix::sys::statvfs` on install_dir -- warn (non-blocking) if available space < 2x tarball size.
   - Download archive using `libprotonup::downloads::download_to_async_write`, emitting `ProtonInstallProgress` via `progress_tx` callback at each phase (Downloading with byte progress, Verifying, Extracting).
   - SHA-512 verification: fetch `.sha512sum` via `download_file_into_memory`, compare against downloaded archive.
   - **Security C-3**: Extraction with archive bomb caps -- 8 GB total extracted size, 50,000 file entry limit. Wrap `libprotonup::files::unpack_file` and count entries/bytes during extraction, aborting if caps exceeded.
   - Atomic installation: extract to `<version>.tmp` directory, rename atomically on success, clean up `.tmp` on failure.
   - Return `ProtonUpInstallResult` with `succeeded`, `installed_path`, `message`.

3. `pub async fn delete_version(version_dir: &Path) -> Result<(), ProtonupError>`: validate path is within `compatibilitytools.d`, then `tokio::fs::remove_dir_all`. Refuse to delete official Proton versions (check `is_official` field).

4. **Security C-1**: Verify `Cargo.lock` shows `astral-tokio-tar 0.6.x` and no `tokio-tar` -- this is a manual verification step documented in the task, not runtime code.

#### Task 1D-2: Implement `protonup/advisor.rs` -- profile version matching Depends on [1A-1, 1A-2, 1A-3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs (normalize_alias function)
- docs/plans/protonup-integration/feature-spec.md (advisor behavior)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/protonup/advisor.rs

Pure logic module (~60 lines), no filesystem or network I/O. Implement:

1. `pub fn suggest_version_for_profile(requested_version: &str, installed: &[InstalledProtonVersion], available: &[AvailableProtonVersion]) -> ProtonVersionSuggestion`: normalize `requested_version` via `steam::proton::normalize_alias`, then search installed versions for exact match or closest fuzzy match. If not installed, search available versions. Return `ProtonVersionSuggestion` with `is_installed`, `closest_installed: Option<String>`, `available_version: Option<String>`.

2. `pub fn find_orphan_versions(installed: &[InstalledProtonVersion], profile_proton_paths: &[String]) -> Vec<InstalledProtonVersion>`: identify installed GE-Proton versions not referenced by any profile's proton path. Accept a simple `&[String]` of proton paths extracted from profiles (the caller in the Tauri command layer can extract paths from `GameProfile` structs). This avoids coupling `advisor.rs` to the `profile` module. Used by Phase 3 cleanup UI.

Test with varied version string formats: `GE-Proton9-20`, `Proton-9.20-GE-1`, `ge-proton9-20` (case variants).

#### Task 1E-1: Tauri command layer and state registration Depends on [1A-1, 1B-1, 1C-1, 1D-1, 1D-2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/prefix_deps.rs (PrefixDepsInstallState, AppHandle::emit pattern)
- src/crosshook-native/src-tauri/src/commands/install.rs (thin wrapper pattern)
- src/crosshook-native/src-tauri/src/lib.rs (lines 197-322: .manage() and invoke_handler!)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/protonup.rs

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Create `commands/protonup.rs` with `ProtonupInstallState` struct (wrapping `tokio::sync::Mutex<Option<tokio::task::AbortHandle>>`) and 5 `#[tauri::command]` functions:

1. `protonup_list_versions(metadata_store: State<'_, MetadataStore>) -> Result<Vec<AvailableProtonVersion>, String>`: call `fetcher::list_available_versions`. Thin wrapper with `.map_err(|e| e.to_string())`.

2. `protonup_list_installed(metadata_store: State<'_, MetadataStore>) -> Result<Vec<InstalledProtonVersion>, String>`: call `scanner::list_installed_versions` with steam root candidates from `discover_steam_root_candidates`.

3. `protonup_install_version(version: String, tool_name: String, install_dir: String, app: AppHandle, metadata_store: State<'_, MetadataStore>, install_state: State<'_, ProtonupInstallState>) -> Result<(), String>`: acquire install lock via `try_lock()` (return error if already installing). Spawn background task via `tauri::async_runtime::spawn`. In the spawned task: call `installer::install_version` with a progress callback that calls `app.emit("protonup-install-progress", payload)`. On completion, emit `app.emit("protonup-install-complete", payload)`. Store `AbortHandle` in `ProtonupInstallState` for cancellation. Return `Ok(())` immediately.

4. `protonup_cancel_install(install_state: State<'_, ProtonupInstallState>) -> Result<(), String>`: acquire mutex, take the `AbortHandle`, call `.abort()`.

5. `protonup_delete_version(version_dir: String, metadata_store: State<'_, MetadataStore>) -> Result<(), String>`: call `installer::delete_version`.

6. `protonup_suggest_version(requested_version: String, metadata_store: State<'_, MetadataStore>) -> Result<ProtonVersionSuggestion, String>`: call `advisor::suggest_version_for_profile` with installed and available version lists. Used by Phase 3 community profile integration.

7. `protonup_find_orphans(metadata_store: State<'_, MetadataStore>) -> Result<Vec<InstalledProtonVersion>, String>`: call `advisor::find_orphan_versions` with installed versions and profile proton paths. Used by Phase 3 cleanup UI.

Modify `src-tauri/src/lib.rs`: add `.manage(commands::protonup::ProtonupInstallState::new())` after existing `.manage()` calls. Register all 7 commands in the `invoke_handler!` array. Event names: `protonup-install-progress`, `protonup-install-complete` (kebab-case, consistent with `update-log`, `prefix-dep-log`).

#### Task 1E-2: Module root `mod.rs` and unit tests Depends on [1A-1, 1B-1, 1C-1, 1D-1, 1D-2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/install/mod.rs (re-export pattern)
- src/crosshook-native/crates/crosshook-core/src/install/service.rs (lines 334-507 test pattern)
- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs (open_in_memory for test fixtures)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs

Populate `protonup/mod.rs` with module declarations and `pub use` re-exports following the `install/mod.rs` pattern. Re-export all public types from `models.rs`, `error.rs`, and all public functions from `fetcher.rs`, `scanner.rs`, `installer.rs`, `advisor.rs`.

Add inline `#[cfg(test)] mod tests` in the appropriate module files (or a dedicated `tests.rs`). Required tests:

1. **Cache round-trip**: write `ProtonVersionListCache` to `MetadataStore::open_in_memory()`, read back, verify content and TTL expiry logic.
2. **Stale fallback**: verify that expired cache entry is still served when network probe fails.
3. **`From<ProtonInstall>` conversion**: verify all `ProtonInstall` fields map correctly to `InstalledProtonVersion`.
4. **Path traversal guard**: verify `validate_install_request` rejects `..` components, absolute paths outside `$HOME`, symlink targets.
5. **Idempotency**: verify install returns `AlreadyInstalled` when version directory exists (use `tempfile::tempdir()`).
6. **Version string normalization**: verify `suggest_version_for_profile` matches across case variants and alias formats.

Run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Phase 2: Core UI

#### Task 2A-1: TypeScript types and `useProtonVersions` hook Depends on [1E-1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/proton.ts
- src/crosshook-native/src/types/update.ts (TS mirror pattern)
- src/crosshook-native/src/hooks/useProtonInstalls.ts (reload counter pattern)

**Instructions**

Files to Create

- src/crosshook-native/src/types/protonup.ts
- src/crosshook-native/src/hooks/useProtonVersions.ts

**types/protonup.ts**: Mirror all Rust IPC types with `snake_case` field names. Include `AvailableProtonVersion`, `InstalledProtonVersion`, `ProtonInstallProgress`, `ProtonVersionSuggestion`, `ProtonUpInstallStage` type union (`'idle' | 'preparing' | 'installing' | 'complete' | 'failed'`), `PROTONUP_VALIDATION_MESSAGES` const (synced with Rust `.message()` output).

**useProtonVersions.ts**: Wrap `protonup_list_versions` and `protonup_list_installed` commands. Expose: `availableVersions: AvailableProtonVersion[]`, `installedVersions: InstalledProtonVersion[]`, `isLoading: boolean`, `error: string | null`, `isStale: boolean`, `cacheAge: string | null`, `isOffline: boolean`, `reload: () => void`, `refresh: () => Promise<void>`. Follow `useProtonInstalls.ts` reload counter pattern (`setReloadVersion(c => c + 1)`). Initial load on mount via `useEffect`.

#### Task 2A-2: `useInstallProtonVersion` hook Depends on [1E-1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useUpdateGame.ts (lines 192-263: listen-before-invoke, stage machine, completedBeforeInvoke race guard)
- src/crosshook-native/src/hooks/useProtonInstalls.ts (reload pattern)

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/useInstallProtonVersion.ts

Implement install lifecycle hook following `useUpdateGame.ts` exactly. Expose: `stage: ProtonUpInstallStage`, `progress: ProtonInstallProgress | null`, `error: string | null`, `canInstall: boolean`, `isInstalling: boolean`, `install: (version: string, toolName: string, installDir: string) => Promise<void>`, `cancel: () => Promise<void>`, `reset: () => void`.

Critical patterns to replicate:

1. **Listen before invoke**: subscribe to `protonup-install-progress` and `protonup-install-complete` events BEFORE calling `invoke('protonup_install_version')`.
2. **Race guard**: `let completedBeforeInvoke = false` flag prevents regression to `'installing'` if backend completes before invoke resolves.
3. **unlistenRef**: store cleanup function in ref, call on unmount and reset.
4. **Event filtering**: filter events by `event.payload.version !== targetVersion`.
5. **Post-install**: on success, call `protonInstalls.reload()` from the hook consumer (expose callback or accept reload function as parameter).
6. **Normalize errors**: `error instanceof Error ? error.message : String(error)` in catch blocks.

#### Task 2B-1: `ProtonVersionManager` component Depends on [2A-1, 2A-2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/PrefixDepsPanel.tsx (UI install pattern)
- docs/plans/protonup-integration/research-ux.md (component vocabulary, user flows)
- src/crosshook-native/src/styles/variables.css (CSS custom properties)

**Instructions**

Files to Create

- src/crosshook-native/src/components/ProtonVersionManager.tsx

Primary UI component (~300 lines). Structure following `PrefixDepsPanel.tsx`:

1. **Installed section** (`CollapsibleSection`): list installed GE-Proton versions from `useProtonVersions().installedVersions`. Each row shows version name, path. Delete button (with confirmation modal) calls `invoke('protonup_delete_version')`. Official Proton versions have delete disabled.

2. **Available section** (`CollapsibleSection`): list available versions from `useProtonVersions().availableVersions`. Text filter (debounced 200ms), sort dropdown. Each row shows tag name, size. Install button disabled while `isInstalling`. Active install row shows inline progress bar with stage text (`Downloading... 45%` -> `Verifying checksum...` -> `Extracting...`).

3. **Cache-age banner**: show `crosshook-community-browser__cache-banner`-style indicator when serving stale data. Offline state: show cached list with age, disable install buttons with "No network" explanation.

4. **Accessibility**: all rows keyboard-navigable, `aria-label` on Install/Cancel/Delete buttons, `role="progressbar"` with `aria-valuenow`/`aria-valuemin`/`aria-valuemax` on progress bar, `role="status"` on cache-age banner.

5. **CSS**: use BEM classes `crosshook-protonup`, `crosshook-protonup__releases`, `crosshook-protonup__log`. Use CSS variables from `variables.css`. Compositor-friendly animations only (`transform`, `opacity`).

6. **No resume support**: document in UI that cancelled downloads restart from scratch (libprotonup limitation).

#### Task 2B-2: Settings panel integration Depends on [1A-4, 2A-1, 2A-2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/SettingsPanel.tsx
- src/crosshook-native/src/context/PreferencesContext.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/SettingsPanel.tsx

Add `ProtonVersionManager` as a `CollapsibleSection` within `SettingsPanel.tsx`. Import the component and place it in the appropriate settings group (near Proton/compatibility settings if they exist, or as a new section). Add a preferred version selector dropdown bound to `settings.preferred_proton_version` via `PreferencesContext`. Add stale-preference warning when the preferred version is no longer installed. Be surgical -- SettingsPanel is 49KB; add only the import, component placement, and preference binding without touching existing sections.

#### Task 2B-3: Scroll container registration Depends on [2B-1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useScrollEnhance.ts
- CLAUDE.md (scroll registration requirement)

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useScrollEnhance.ts

Add the `ProtonVersionManager`'s scrollable container selector(s) to the `SCROLLABLE` constant. Identify the exact CSS selector(s) for any `overflow-y: auto` containers in `ProtonVersionManager.tsx` (e.g., the releases list, log output). This is a ship-blocking requirement -- missing it causes dual-scroll jank on WebKitGTK. Also add `overscroll-behavior: contain` to the inner scroll container's CSS.

### Phase 3: Polish and Community Integration

#### Task 3-1: Community profile auto-suggest Depends on [2B-1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/CommunityBrowser.tsx
- src/crosshook-native/crates/crosshook-core/src/protonup/advisor.rs
- docs/plans/protonup-integration/feature-spec.md (community integration section)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/CommunityBrowser.tsx (or relevant community profile component)

Cross-reference community profile `proton_version` field against installed versions via `suggest_version_for_profile` (exposed as a Tauri command wrapping `advisor.rs`). On community profile cards where `proton_version` is non-null and not installed, show a `[Not Installed]` chip. In the profile import wizard, show "Required Proton version: GE-Proton9-27 [Not Installed]" with an optional install checkbox. Profile import is NEVER blocked by missing Proton version (BR-1: launch must never be gated by ProtonUp state).

#### Task 3-2: Post-install profile path suggestion Depends on [2B-1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/migration.rs (check_proton_migrations, apply_proton_migration)
- src/crosshook-native/src/context/PreferencesContext.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ProtonVersionManager.tsx (or relevant profile components)

After a successful ProtonUp install, check two settings fields: (1) `settings.default_proton_path` (existing, stores the filesystem path to the default Proton binary) -- if empty or pointing to a non-existent path, prompt to set it to the newly installed version's path; (2) `settings.preferred_proton_version` (new from Task 1A-4, stores the version name like "GE-Proton9-20") -- if empty, prompt to set it. These are distinct: `default_proton_path` is the runtime binary path used by launch commands; `preferred_proton_version` is the preferred version name shown in the UI. Optionally trigger `check_proton_migrations` to identify profiles with stale Proton paths and offer to update them via `apply_proton_migration`. No new backend code -- uses existing migration commands.

#### Task 3-3: Wine-GE support Depends on [2B-1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/protonup/fetcher.rs
- docs/plans/protonup-integration/research-external.md (WineGE section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/protonup/fetcher.rs
- src/crosshook-native/src/hooks/useProtonVersions.ts
- src/crosshook-native/src/components/ProtonVersionManager.tsx

Add `VersionChannel::WineGe` variant to the enum (if not already present from 1A-3). Add second cache key `protonup:versions:v1:wine-ge` in `fetcher.rs`. Update `fetcher::list_available_versions` to accept `VersionChannel` parameter and select the appropriate `libprotonup::CompatTool` (`"WineGE"`). Add channel toggle UI (chips or dropdown) in `ProtonVersionManager`. Note: Wine-GE installs to `~/.local/share/lutris/runners/wine/`, NOT `compatibilitytools.d/` -- clearly label this in the UI as "Lutris Wine-GE" and use the correct install path.

#### Task 3-4: Cleanup UI -- orphan detection and deletion Depends on [2B-1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/protonup/advisor.rs (find_orphan_versions)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ProtonVersionManager.tsx

Add an "Orphan Detection" section or button to `ProtonVersionManager`. Call `find_orphan_versions` (exposed as a Tauri command) to identify installed GE-Proton versions not referenced by any profile. Display orphans with storage size and a batch-delete option. Confirm before deletion. This is a convenience feature -- do not gate any other functionality on it.

## Advice

- **Copy, don't abstract**: `protondb/client.rs:85-130` (cache-first fetch) and `commands/prefix_deps.rs:234-320` (event streaming) should be copied verbatim and adapted. Two call sites do not warrant a shared abstraction. `research-practices.md` explicitly prohibits abstracting the cache pattern.
- **Security mitigations are embedded, not separate tasks**: The three CRITICAL security items (path traversal, archive bomb, Cargo.lock verification) are acceptance criteria for Task 1D-1, not standalone tasks. Do not split them out -- doing so creates a window of insecure code.
- **Pin `libprotonup` exactly**: Use `= "0.11.0"` (exact pin) in `Cargo.toml`, not `"0.11.0"` (which allows patch upgrades). After any `Cargo.toml` change, verify `Cargo.lock` shows `astral-tokio-tar 0.6.x` and no `tokio-tar`.
- **`listen-before-invoke` race guard is non-negotiable**: The `completedBeforeInvoke` flag in `useUpdateGame.ts:227` prevents the UI from regressing to `'installing'` if the backend completes before the invoke promise resolves. Fast local installs (from SSD) can trigger this race.
- **MetadataStore Mutex must never be held across `.await`**: The read-release-reacquire sequence from `protondb/client.rs` is the only safe pattern. Holding the `Arc<Mutex<Connection>>` during async download will deadlock.
- **`SettingsPanel.tsx` is 49KB**: Import and place `<ProtonVersionManager />` surgically. Do not inline panel code into SettingsPanel. Keep the component self-contained.
- **`useScrollEnhance` registration is a one-liner but ship-blocking**: Missing it causes dual-scroll jank on WebKitGTK. Must be done as soon as the component's DOM structure is finalized (Task 2B-3).
- **File naming follows `feature-spec.md`**: Use `fetcher.rs` (not `client.rs`), `installer.rs` (not `service.rs`), `scanner.rs`, `advisor.rs`. The feature-spec naming takes precedence over older research docs.
- **GE-Proton only in Phase 1**: Wine-GE is deferred to Phase 3 Task 3-3 to avoid scope creep. The `VersionChannel` enum should be defined in models.rs but only `GEProton` is implemented in Phase 1.
- **No resume support**: `libprotonup 0.11.0` does not support HTTP range requests. Cancelled downloads restart from scratch. Document this in the UI rather than attempting a workaround.
- **BR-1 is unconditional**: Profile launch must NEVER be gated by ProtonUp state. The feature is install/suggestion only -- it never intercepts or blocks game launching.
- **Test the cache round-trip first**: The most important unit test is cache write/read with `MetadataStore::open_in_memory()`. This catches serialization bugs and cache key mismatches that would be painful to debug through the full Tauri stack.
