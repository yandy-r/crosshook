# Technical Research: ProtonUp Integration

## Executive Summary

CrossHook already carries `libprotonup = "0.11.0"` as a dependency in `crosshook-core` and has
complete infrastructure for async download streaming (reqwest, tokio), archive extraction (tar,
flate2, async-compression), external API caching (existing `external_cache_entries` table), and
filesystem-derived Proton discovery (`steam/proton.rs`). The full integration requires adding one
new module (`protonup`) to `crosshook-core`, one new command file to `src-tauri/src/commands/`,
two new fields to `AppSettingsData`, one new Tauri-managed state struct for install progress, and
zero new DB tables. The existing `external_cache_entries` table is sufficient for caching
available version lists with TTL.

---

## Architecture Design

### Component Diagram

```
                     ┌───────────────────────────────────────────┐
                     │             crosshook-core                 │
                     │                                            │
                     │  ┌─────────────────────────────────────┐  │
                     │  │   src/protonup/mod.rs               │  │
                     │  │                                     │  │
                     │  │  VersionFetcher   (async, cached)   │  │
                     │  │  FsScanner        (sync, runtime)   │  │
                     │  │  Installer        (async, streaming)│  │
                     │  │  ProfileAdvisor   (sync)            │  │
                     │  │  models.rs        (Serde types)     │  │
                     │  │  error.rs         (typed errors)    │  │
                     │  └───────────┬─────────────────────────┘  │
                     │              │ uses                        │
                     │   metadata::MetadataStore  (cache_store)   │
                     │   settings::AppSettingsData (TOML)         │
                     │   steam::proton   (existing FsScanner)     │
                     │   libprotonup     (GitHub API + extract)   │
                     └──────────────┬────────────────────────────┘
                                    │ pub fn / pub async fn
                     ┌──────────────▼────────────────────────────┐
                     │   src-tauri/src/commands/protonup.rs       │
                     │                                            │
                     │   ProtonUpInstallState  (Arc<Mutex>)       │
                     │   #[tauri::command] list_available_...     │
                     │   #[tauri::command] install_proton_...     │
                     │   #[tauri::command] get_installed_...      │
                     │   #[tauri::command] get_proton_install_... │
                     │   #[tauri::command] suggest_proton_...     │
                     └──────────────┬────────────────────────────┘
                                    │ Tauri IPC / Emitter
                     ┌──────────────▼────────────────────────────┐
                     │   React frontend (new ProtonUpManager UI)  │
                     └───────────────────────────────────────────┘
```

### New Components

#### `crosshook-core/src/protonup/mod.rs`

The public API surface for the module. Re-exports all public types and functions.

#### `crosshook-core/src/protonup/models.rs`

All Serde types that cross module or IPC boundaries:

- `AvailableProtonVersion` — a single listable version with tag, size, published_at
- `InstalledProtonVersion` — filesystem-discovered install with path and name
- `ProtonInstallRequest` — input for the install command
- `ProtonInstallProgress` — emitted event payload for streaming progress
- `ProtonVersionSuggestion` — returned by the advisor for a given profile
- `ProtonVersionListCache` — the JSON payload structure stored in `external_cache_entries`

#### `crosshook-core/src/protonup/fetcher.rs`

Wraps `libprotonup::downloads::list_releases` + `libprotonup::sources::CompatTool`. Handles
caching via `metadata::cache_store`. Cache key convention: `protonup:versions:v1:{tool_slug}` (e.g.
`protonup:versions:v1:ge-proton`).

#### `crosshook-core/src/protonup/scanner.rs`

Wraps the existing `steam::proton::discover_compat_tools` to enumerate installed GE-Proton/Wine-GE
versions at runtime. Returns `Vec<InstalledProtonVersion>` — purely runtime state, not persisted.

#### `crosshook-core/src/protonup/installer.rs`

Uses `libprotonup::downloads::download_to_async_write` + `libprotonup::files::unpack_file` with a
custom progress-reporting wrapper. Sends byte-count progress via a `tokio::sync::mpsc` channel.
The command handler bridges that channel to Tauri `app.emit()` events.

#### `crosshook-core/src/protonup/advisor.rs`

Reads `community_profiles.proton_version` (via `MetadataStore`) for a given profile and cross-
references with installed versions to produce a `ProtonVersionSuggestion`.

#### `crosshook-core/src/protonup/error.rs`

Typed `ProtonUpError` enum covering: `CacheUnavailable`, `GitHubApiError { message }`,
`NetworkUnavailable`, `InstallFailed { reason }`, `ExtractionFailed { reason }`,
`BinaryNotFound { name }`, `IoError { path, source }`, `MetadataError(MetadataStoreError)`.

### Integration Points

- **`metadata::cache_store`** — `get_cache_entry` / `put_cache_entry` used directly by
  `fetcher.rs` with a 24-hour TTL for available version lists.
- **`steam::proton::discover_compat_tools`** — re-used by `scanner.rs` to avoid duplicating
  filesystem enumeration logic.
- **`settings::AppSettingsData`** — two new fields added (see Data Models).
- **`libprotonup`** — `downloads::list_releases`, `downloads::download_to_async_write`,
  `files::unpack_file`, `sources::CompatTool::from_str("GEProton")` and `"WineGE"` are the
  primary entry points.
- **Tauri `Emitter`** — progress and completion events are emitted over the existing Tauri event
  bus, following the same pattern as `update.rs` and `prefix_deps.rs`.

---

## Data Models

### New `AppSettingsData` Fields

> **Note on existing field**: `AppSettingsData` already has `default_proton_path: String` (settings/mod.rs:141).
> That field stores a **filesystem path** to the Proton executable (e.g. `/home/user/.steam/steam/compatibilitytools.d/GE-Proton9-27/proton`)
> and is applied to the `runtime.proton_path` of newly created profiles via `creation_defaults.rs`.
> It is semantically different from a version tag and serves a different purpose.
> The new `preferred_proton_version` field below stores a version tag string (e.g. `"GE-Proton9-27"`)
> used for UI pre-selection in the install dialog. Both fields are needed; they do not overlap.

Add two new fields to `AppSettingsData` in
`/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`:

```rust
/// Path override for protonup-rs / protonup-qt binary; empty = auto-detect from PATH.
/// When populated, the binary is used for install operations.
/// When empty, CrossHook uses libprotonup directly without invoking a binary.
#[serde(default, skip_serializing_if = "String::is_empty")]
pub protonup_binary_path: String,

/// User-preferred Proton version tag for new profiles (e.g. "GE-Proton9-27").
/// Empty = no preference. Shown as a pre-selection in the install UI.
#[serde(default, skip_serializing_if = "String::is_empty")]
pub preferred_proton_version: String,
```

These serialize into `~/.config/crosshook/settings.toml` with no migration needed (TOML `#[serde(default)]` backward-compat is automatic).

### `external_cache_entries` Cache Key Pattern

```
protonup:versions:v1:{tool_slug}
```

The `v1` segment allows cache key evolution without collisions if the payload schema changes.
`{tool_slug}` is the lowercase slug form of the tool name.

Examples:

- `protonup:versions:v1:ge-proton`
- `protonup:versions:v1:wine-ge`

**TTL**: 24 hours (`expires_at = now() + 24h`).

**`payload_json` Structure** (the `ProtonVersionListCache` model):

```json
{
  "tool_name": "GEProton",
  "fetched_at": "2026-04-06T12:00:00Z",
  "versions": [
    {
      "tag_name": "GE-Proton9-27",
      "published_at": "2024-10-15T18:00:00Z",
      "download_url": "https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton9-27/GE-Proton9-27.tar.gz",
      "size_bytes": 412000000,
      "checksum_url": "https://github.com/.../GE-Proton9-27.sha512sum",
      "checksum_type": "sha512"
    }
  ]
}
```

**Size constraint**: `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB). GE-Proton release lists with
50 versions and full metadata are approximately 80–150 KiB JSON — well within the limit.

### Runtime State Struct (Tauri-managed)

```rust
// In src-tauri/src/commands/protonup.rs
pub struct ProtonUpInstallState {
    /// install_key -> (progress_tx, cancel_tx)
    active: Mutex<HashMap<String, (mpsc::Sender<ProtonInstallProgress>, oneshot::Sender<()>)>>,
}
```

This is ephemeral — it lives in Tauri state and is not persisted. It allows `get_proton_install_progress` to poll and `cancel_proton_install` (future command) to send a cancellation signal.

### Rust Model Types (crosshook-core/src/protonup/models.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableProtonVersion {
    pub tag_name: String,
    pub published_at: String,
    pub download_url: String,
    pub size_bytes: u64,
    pub checksum_url: Option<String>,
    pub checksum_type: Option<String>, // "sha512" | "sha256"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledProtonVersion {
    /// Display name (e.g. "GE-Proton9-27")
    pub name: String,
    /// Absolute path to the `proton` executable
    pub proton_executable_path: String,
    pub is_official: bool,
    pub source: InstalledProtonSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledProtonSource {
    SteamappsCommon,
    CompatibilityToolsD,
    SystemShared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonInstallRequest {
    pub tool_name: String,   // "GEProton" | "WineGE"
    pub version_tag: String, // e.g. "GE-Proton9-27"
    pub install_dir: Option<String>, // None = default Steam compatibilitytools.d
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonInstallProgress {
    pub tool_name: String,
    pub version_tag: String,
    /// Phase: "downloading" | "verifying" | "extracting" | "complete" | "error"
    pub phase: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    /// Percentage 0–100, computed from bytes_downloaded/total_bytes
    pub percent: u8,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonVersionSuggestion {
    pub profile_name: String,
    /// The proton_version string from community_profiles.proton_version
    pub required_version: String,
    /// True if that version is currently installed
    pub is_installed: bool,
    /// The closest installed version if not installed (heuristic match)
    pub closest_installed: Option<String>,
    /// The exact AvailableProtonVersion for the required version, if found in the cache
    pub available_version: Option<AvailableProtonVersion>,
}

/// JSON payload stored in external_cache_entries.payload_json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonVersionListCache {
    pub tool_name: String,
    pub fetched_at: String,
    pub versions: Vec<AvailableProtonVersion>,
}
```

---

## API Design

All new commands follow the `snake_case` naming convention and are registered in
`src-tauri/src/lib.rs` `invoke_handler`.

### `list_available_proton_versions`

Lists available GE-Proton or Wine-GE versions from GitHub via libprotonup, with SQLite cache TTL.

**Request:**

```rust
#[tauri::command]
pub async fn list_available_proton_versions(
    tool_name: String,        // "GEProton" | "WineGE"
    force_refresh: bool,      // bypass cache
    state: State<'_, SettingsStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<AvailableProtonVersion>, String>
```

**Response:** `Vec<AvailableProtonVersion>` (sorted newest first)

**Errors:**

- `"offline_mode_enabled"` if `settings.offline_mode == true`
- `"cache_only_stale"` if offline and cache exists but is stale (frontend shows age indicator)
- `"network_error: {detail}"` if GitHub API fails and no cache available
- `"invalid_tool_name"` if `tool_name` is not recognized by libprotonup

**Cache behavior:**

1. If `!force_refresh`, check `external_cache_entries` for `protonup:versions:v1:{tool_slug}`.
2. If valid (not expired), return parsed `ProtonVersionListCache.versions`.
3. If expired or missing, call `libprotonup::downloads::list_releases(&compat_tool).await`.
4. On success, serialize to JSON and upsert via `metadata::cache_store::put_cache_entry` with 24h TTL.
5. On network failure with stale cache present, return stale data with a client-side indicator
   (the frontend checks `fetched_at` in a separate `get_proton_cache_metadata` call or we include it
   in a wrapper response — see open questions).

### `install_proton_version`

Downloads and installs a Proton version. Emits streaming progress events.

**Request:**

```rust
#[tauri::command]
pub async fn install_proton_version(
    request: ProtonInstallRequest,
    app: AppHandle,
    install_state: State<'_, ProtonUpInstallState>,
    settings_store: State<'_, SettingsStore>,
) -> Result<(), String>
```

**Progress events** (emitted via `app.emit("proton-install-progress", payload)`):

```json
{
  "tool_name": "GEProton",
  "version_tag": "GE-Proton9-27",
  "phase": "downloading",
  "bytes_downloaded": 104857600,
  "total_bytes": 412000000,
  "percent": 25,
  "error": null
}
```

**Completion event** emitted as `app.emit("proton-install-complete", payload)`:

```json
{
  "tool_name": "GEProton",
  "version_tag": "GE-Proton9-27",
  "phase": "complete",
  "bytes_downloaded": 412000000,
  "total_bytes": 412000000,
  "percent": 100,
  "error": null
}
```

**Error event** (same `proton-install-progress` event with `phase: "error"`):

```json
{
  "tool_name": "GEProton",
  "version_tag": "GE-Proton9-27",
  "phase": "error",
  "bytes_downloaded": 87654321,
  "total_bytes": 412000000,
  "percent": 21,
  "error": "download interrupted: connection reset"
}
```

**Errors returned from command (pre-flight):**

- `"already_installing: GE-Proton9-27"` if an install for that tag is already active
- `"invalid_tool_name"` if `request.tool_name` is not recognized
- `"version_already_installed"` if the version directory already exists in `install_dir`

### `get_installed_proton_versions`

Scans the filesystem at runtime and returns currently installed Proton versions.

**Request:**

```rust
#[tauri::command]
pub async fn get_installed_proton_versions(
    steam_client_install_path: Option<String>,
) -> Result<Vec<InstalledProtonVersion>, String>
```

**Response:** `Vec<InstalledProtonVersion>` (sorted by name)

Delegates to `steam::proton::discover_compat_tools` — same logic as existing `list_proton_installs`
but returns the richer `InstalledProtonVersion` type. May share implementation with a refactor of
`list_proton_installs` or wrap it.

### `get_proton_install_progress`

Polls current install progress (for UI reconnection after navigation or refresh).

**Request:**

```rust
#[tauri::command]
pub fn get_proton_install_progress(
    version_tag: String,
    install_state: State<'_, ProtonUpInstallState>,
) -> Option<ProtonInstallProgress>
```

**Response:** `Option<ProtonInstallProgress>` — `null` if no install is active for that tag.

### `suggest_proton_version_for_profile`

Returns a suggestion for a given profile by cross-referencing the community profile's
`proton_version` field with what is installed.

**Request:**

```rust
#[tauri::command]
pub async fn suggest_proton_version_for_profile(
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<Option<ProtonVersionSuggestion>, String>
```

**Response:** `Option<ProtonVersionSuggestion>` — `null` if no community profile is linked or no
`proton_version` requirement is found.

**Logic:**

1. Look up `community_profiles.proton_version` for the profile via `MetadataStore`.
2. If empty, return `null`.
3. Scan installed versions via `get_installed_proton_versions`.
4. Check if the required version string matches any installed version name (exact then normalized
   via the existing `steam::proton::normalize_alias` function).
5. If not installed, check the live/cached available list for an exact match.
6. Return `ProtonVersionSuggestion` with `is_installed`, `closest_installed`, `available_version`.

---

## System Constraints

### Download Size and Streaming

GE-Proton releases are 300–600 MB (tar.gz/tar.zst). The download must stream bytes to disk without
buffering the full file in memory. `libprotonup::downloads::download_to_async_write` already
accepts `AsyncWrite` — we pipe it through a counting wrapper:

```rust
struct ProgressWriter<W: AsyncWrite + Unpin> {
    inner: W,
    bytes_written: u64,
    total_bytes: u64,
    progress_tx: mpsc::Sender<u64>,
}
```

The `AsyncWrite` impl updates `bytes_written` and sends the new count over the channel after each
`poll_write`. The command handler receives on the channel and emits Tauri events.

**Throttle events**: emit at most 1 event per 0.5% progress change or 1 second, whichever comes
first, to avoid flooding the frontend (a 500 MB file at 10 MB/s would produce ~100 events/s
otherwise). Use a `last_emit_percent: u8` and `last_emit_instant: Instant` guard in the async
reader loop.

### Filesystem Permissions and Install Path

Default install target: `~/.steam/steam/compatibilitytools.d/` (Steam native).
Flatpak Steam: `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d/`.

`libprotonup::apps::AppInstallations::Steam.default_install_dir()` returns the correct path with
tilde expansion via `libprotonup::utils::expand_tilde`. `unpack_file` calls `fs::create_dir_all`
before extraction, so the directory is created if absent.

Permission constraint: CrossHook runs as the user, so only user-owned `compatibilitytools.d` is
writable. System paths (`/usr/share/steam/compatibilitytools.d`) are read-only at runtime. The
installer should only target the user's Steam `compatibilitytools.d`.

### tar.gz / tar.zst Extraction

`libprotonup::files::unpack_file` + `Decompressor::from_path` handles gz, xz, and zst via
async-compression. The archive renames the top-level directory to the `CompatTool.installation_name`
output, which is the standard GE-Proton naming convention
(`GE-Proton9-27` → extracts as `GE-Proton9-27/`).

### GitHub API Rate Limits

GitHub unauthenticated rate limit: 60 requests/hour per IP. The 24-hour cache means a normal user
makes at most 1–2 API calls per day per tool. Even if the user force-refreshes, the debounce in the
UI (disable the refresh button for 60 seconds after a fetch) is sufficient.

**No GitHub token is required** for listing public releases. However, if the user is behind a NAT
sharing an IP with many other users (university, corporate VPN), rate limits could be hit. The
fallback to stale cache data handles this gracefully.

### Error Recovery for Interrupted Downloads

The installer downloads to a `.tmp` path (e.g. `GE-Proton9-27.tar.gz.tmp`) in a temp directory
(`libprotonup::utils::create_download_temp_dir`). On success, the archive is unpacked and the temp
file is deleted. On error or cancellation, the temp file is deleted in a `Drop` guard. **No partial
extraction is left behind** because `unpack_file` extracts to a new directory; if it fails mid-way,
the partial directory is left. A cleanup step removes the partial dir if the version tag does not
appear in `get_installed_proton_versions()` after the fact.

**Retry**: not implemented in v1. The user can re-trigger from the UI. An interrupted download
starts from zero (no resume/range-request support in libprotonup 0.11.0).

### Offline Behavior

When `settings.offline_mode == true`:

- `list_available_proton_versions` returns cached data (even if stale) with a `stale: true`
  indicator in the response wrapper, or returns an error if no cache exists.
- `install_proton_version` returns `"offline_mode_enabled"` immediately.
- `get_installed_proton_versions` always works (filesystem-only).
- `suggest_proton_version_for_profile` works against installed versions only; sets
  `available_version: null`.

---

## Codebase Changes

### Files to Create

> **Note**: The directory `src/crosshook-native/crates/crosshook-core/src/protonup/` does **not yet exist**
> (confirmed: no files found under that path). It must be created as part of implementation.
> Rust's module system requires only the `.rs` files; `cargo build` will create no directory itself,
> but the files listed below must be placed in a new `protonup/` subdirectory.

| File                                                                   | Purpose                         |
| ---------------------------------------------------------------------- | ------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs`       | Module root, public re-exports  |
| `src/crosshook-native/crates/crosshook-core/src/protonup/models.rs`    | Serde data types                |
| `src/crosshook-native/crates/crosshook-core/src/protonup/fetcher.rs`   | Version list fetcher with cache |
| `src/crosshook-native/crates/crosshook-core/src/protonup/scanner.rs`   | Filesystem install scanner      |
| `src/crosshook-native/crates/crosshook-core/src/protonup/installer.rs` | Download + extract pipeline     |
| `src/crosshook-native/crates/crosshook-core/src/protonup/advisor.rs`   | Profile version suggestion      |
| `src/crosshook-native/crates/crosshook-core/src/protonup/error.rs`     | Typed error enum                |
| `src/crosshook-native/src-tauri/src/commands/protonup.rs`              | Tauri command handlers          |

### Files to Modify

| File                                                             | Change                                                                               |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`          | Add `pub mod protonup;`                                                              |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` | Add 2 new fields to `AppSettingsData`                                                |
| `src/crosshook-native/src-tauri/src/commands/mod.rs`             | Add `pub mod protonup;`                                                              |
| `src/crosshook-native/src-tauri/src/lib.rs`                      | Register new commands in `invoke_handler`, add `ProtonUpInstallState` to `.manage()` |

### Dependency Additions

None. `libprotonup = "0.11.0"` is already in `crosshook-core/Cargo.toml`. `reqwest`, `tokio`,
`serde`, `chrono` are also already present. `tokio::sync::mpsc` and `oneshot` are in the existing
`tokio` dependency's `sync` feature (currently enabled via `tokio = { features = ["fs", "process", "rt", "sync", "time"] }`).

---

## Technical Decisions

### Decision 1: Use `libprotonup` directly vs. invoking `protonup-rs` binary

**Option A — Use `libprotonup` directly (Rust API)**

- No subprocess overhead
- Download progress is native async/channel — no log scraping
- No dependency on user having protonup-rs or protonup-qt installed
- Already a Cargo dependency

**Option B — Shell out to `protonup-rs` / `protonup-qt` binary**

- Simpler install implementation (one command)
- Progress is log-scraping or exit-code based — brittle
- User must have the binary installed (additional setup step)
- Binary discovery adds failure surface

**Recommendation: Option A.** `libprotonup` is already in Cargo.toml and provides all needed
primitives. The binary path setting (`protonup_binary_path`) is retained as a settings field only
as a future escape hatch for users who want to invoke their installed binary for advanced operations
(e.g. Luxtorpeda installs), but is **not used in v1 installation flow**.

If the user does not have `libprotonup`-compatible connectivity or wants to use a different tool, the
settings field enables them to point to their installed binary and we can add a `shell-out`
installer path in a later phase.

### Decision 2: Cache payload format — raw release JSON vs. normalized model

**Option A — Store `libprotonup::downloads::ReleaseList` JSON verbatim**

- No transformation on write, deserialization-on-read
- Couples the cache format to libprotonup's internal `Release` struct (which is not `pub`)
- If libprotonup changes the struct, old cache entries fail to deserialize

**Option B — Normalize to `ProtonVersionListCache` (our own model)**

- Decouples from libprotonup internals
- Easy to add fields (e.g., `is_installed` hint) in future
- One extra serialization pass on fetch

**Recommendation: Option B.** The normalization cost is negligible and protects from libprotonup
churn. The `ProtonVersionListCache` struct owns the `AvailableProtonVersion` elements which are a
projection of `Release` fields we control.

### Decision 3: Progress event transport — Tauri event bus vs. polling endpoint

**Option A — `app.emit("proton-install-progress", payload)`** (Tauri event bus)

- Real-time push to frontend
- No polling loop
- Consistent with how `update.rs` and `prefix_deps.rs` stream progress

**Option B — `get_proton_install_progress` polling (IPC command)**

- Simpler state management
- Higher latency (depends on poll interval)
- Unnecessary overhead for a long-running operation

**Recommendation: Option A** for real-time download progress (same pattern as existing features).
Option B (`get_proton_install_progress` command) is retained as a supplemental reconnection
mechanism for cases where the frontend navigates away and re-opens the install modal — it can read
the last known state from `ProtonUpInstallState.active` map.

### Decision 4: Install directory — user-configurable vs. hardcoded

**Option A — Always install to `~/.steam/steam/compatibilitytools.d/`**

- Simple
- Fails for Flatpak Steam users (different path)

**Option B — Detect installed Steam variant, use its `compatibilitytools.d`**

- `libprotonup::apps::AppInstallations::detect_installation_method()` can enumerate
- Install goes to the right place for both native and Flatpak Steam

**Option C — Allow user override in `ProtonInstallRequest.install_dir`**

- Maximally flexible
- Combined with Option B as the default when `install_dir` is `None`

**Recommendation: Option C** (default to detected path, allow override). This matches what the
existing `list_proton_installs` / `steam::proton::discover_compat_tools` already does for discovery.
The `ProtonInstallRequest.install_dir` field (optional) lets power users override if needed.

---

## Open Questions

1. **Stale cache age indicator in response**: Should `list_available_proton_versions` return a
   wrapper struct `{ versions: Vec<...>, fetched_at: String, is_stale: bool }` or should the
   frontend call a separate `get_proton_cache_metadata(tool_name)` endpoint? Recommendation: include
   `fetched_at` and `is_stale` in a wrapper struct returned by `list_available_proton_versions` to
   reduce round trips.

2. **Cancel install**: Should `cancel_proton_install(version_tag: String)` be included in v1?
   Pattern exists in `update.rs` (cancel_update) and `prefix_deps.rs` (via lock). Recommendation:
   include it — use the `oneshot::Sender<()>` in `ProtonUpInstallState` to signal cancellation; the
   installer checks `oneshot::Receiver` periodically via `try_recv`.

3. **Hash verification**: `libprotonup` downloads a `.sha512sum` or `.sha256sum` file and compares
   it against the archive. The verification step is provided by `libprotonup::hashing`. Should
   CrossHook expose a separate `phase: "verifying"` event, or fold verification into the
   `downloading` phase? Recommendation: emit a distinct `phase: "verifying"` event (< 1 second
   duration) to give users confidence the integrity check ran.

4. **ProtonUp binary detection**: If `protonup_binary_path` is set, should CrossHook validate the
   binary exists on startup (like the `protontricks_binary_path` pattern)? Recommendation: validate
   lazily (on first use), not at startup, since the binary path is optional and not on the critical
   path for profile launching.

5. **`list_proton_installs` vs. `get_installed_proton_versions` overlap**: The existing command
   `list_proton_installs` returns `Vec<ProtonInstall>` from `crosshook-core`. The new command
   returns `Vec<InstalledProtonVersion>`. Consider whether to refactor `list_proton_installs` to
   return the new richer type or keep both. Recommendation: keep both for v1 (avoid breaking the
   existing command's callers), deprecate `list_proton_installs` in a follow-up.

---

## Relevant Files

- `/src/crosshook-native/crates/crosshook-core/Cargo.toml` — libprotonup = "0.11.0" already present
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry` / `put_cache_entry` / `evict_expired_cache_entries`
- `/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — migration 3→4 defines `external_cache_entries` schema; current version is 18
- `/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — `MAX_CACHE_PAYLOAD_BYTES = 524_288` limit for cache payloads
- `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData` struct; pattern for adding backward-compat fields
- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` — `discover_compat_tools`, `normalize_alias`, `resolve_compat_tool_by_name`; re-use for scanner
- `/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` — `CommunityProfileMetadata.proton_version: String` field
- `/src/crosshook-native/src-tauri/src/commands/update.rs` — reference pattern for spawn_blocking + Tauri event streaming
- `/src/crosshook-native/src-tauri/src/commands/prefix_deps.rs` — reference pattern for managed state + mpsc streaming + lock guard
- `/src/crosshook-native/src-tauri/src/lib.rs` — invoke_handler registration; `.manage()` pattern for new state
- `/src/crosshook-native/src-tauri/src/commands/steam.rs` — `list_proton_installs` (existing, to be superseded)
- `~/.cargo/registry/src/.../libprotonup-0.11.0/src/downloads.rs` — `list_releases`, `download_to_async_write`
- `~/.cargo/registry/src/.../libprotonup-0.11.0/src/files.rs` — `unpack_file`, `Decompressor`
- `~/.cargo/registry/src/.../libprotonup-0.11.0/src/sources.rs` — `CompatTool`, `CompatTools` static list
- `~/.cargo/registry/src/.../libprotonup-0.11.0/src/apps.rs` — `AppInstallations`, `default_install_dir`
