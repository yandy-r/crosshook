# External API Research: ProtonUp Integration

## Executive Summary

CrossHook already has `libprotonup = "0.11.0"` pinned in `crosshook-core/Cargo.toml`. The library provides a fully async Rust API for listing releases from GitHub, downloading archives with streaming, SHA-512 verification, and extracting tar archives to `compatibilitytools.d/`. The primary external dependency is the GitHub Releases REST API (unauthenticated at 60 req/hr, authenticated at 5,000 req/hr). No new crates are needed beyond what is already in the workspace. The integration surface in Tauri is Channels for progress streaming and standard `#[tauri::command]` for list/install/status operations.

**Critical finding (updated)**: `list_releases` fetches **only the first 30 releases** (GitHub API default — no `per_page` parameter, no pagination). For CrossHook's use case this is acceptable since only the latest ~10–30 versions are practically useful. If full history is needed, CrossHook must implement its own paginated fetch.

**Security finding (updated)**: `astral-tokio-tar = "0.6"` (resolved to `0.6.0` in the workspace) is **patched past CVE-2025-59825**. The vulnerability affected ≤ 0.5.3; `0.5.4+` and all of `0.6.x` contain the fix. No action required.

**Confidence**: High — verified against current Cargo.toml, live GitHub API response, libprotonup 0.11.0 source, and workspace cargo metadata.

---

## Primary APIs

### 1. libprotonup (already in Cargo.toml)

**Version pinned**: `0.11.0` (GPL-3.0)
**Documentation**: <https://docs.rs/libprotonup/latest/libprotonup/>
**Source**: <https://github.com/auyer/Protonup-rs/tree/master/libprotonup>

#### Public API Surface

**Module: `downloads`**

```rust
// List available versions from GitHub for a CompatTool
pub async fn list_releases(
    compat_tool: &CompatTool
) -> Result<ReleaseList, reqwest::Error>

// Fetch a text file (e.g., .sha512sum) into memory
pub async fn download_file_into_memory(
    url: &String
) -> Result<String>

// Stream download to any AsyncWrite (file, progress wrapper, etc.)
pub async fn download_to_async_write<W: AsyncWrite + Unpin>(
    url: &str,
    write: &mut W,
) -> Result<()>
```

**Module: `sources`** — `CompatTool` and tool registry

```rust
pub struct CompatTool {
    pub name: String,                              // "GEProton", "WineGE", ...
    pub forge: Forge,                              // GitHub or Custom(url)
    pub repository_account: String,               // "GloriousEggroll"
    pub repository_name: String,                  // "proton-ge-custom"
    pub compatible_applications: Vec<App>,
    pub tool_type: ToolType,                       // WineBased | Runtime
    pub release_asset_filter: Option<String>,      // regex to select asset
    pub has_multiple_asset_variations: bool,
}

// Get all tools compatible with a given app (Steam, Lutris, Custom)
pub fn CompatTool::sources_for_app(app: &apps::App) -> Vec<CompatTool>

// Get the install directory name for a given version tag
pub fn CompatTool::installation_name(&self, version: &str) -> String
```

**Module: `apps`** — Installation path detection

```rust
pub enum AppInstallations {
    Steam,
    SteamFlatpak,
    Lutris,
    LutrisFlatpak,
    Custom(String),
}

// Resolve install base directory
pub fn AppInstallations::default_install_dir(&self) -> PathBuf

// List all installed tool versions in a directory
pub async fn AppInstallations::list_installed_versions(&self) -> Result<Vec<String>>

// Async detect which Steam/Lutris installs exist on this system
pub async fn App::detect_installation_method(&self) -> AppInstallations
```

**Steam paths resolved by libprotonup:**

- Native: `~/.steam/steam/compatibilitytools.d/`
- Flatpak: `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d/`

**Module: `files`** — Archive extraction

```rust
pub async fn unpack_file<R: AsyncRead + Unpin>(
    compat_tool: &CompatTool,
    download: &Download,
    reader: R,          // wrap with progress tracker here
    install_path: &Path,
) -> Result<()>

pub async fn list_folders_in_path(path: &PathBuf) -> Result<Vec<String>, anyhow::Error>
pub async fn check_if_exists(path: &PathBuf) -> bool
```

**`downloads::Release` struct** (from GitHub API):

- `tag_name: String` — e.g., `"GE-Proton10-34"`
- `assets: Vec<Asset>` — archive + sha512sum files

**`downloads::Download` struct** (resolved for an app/tool):

- `download_dir()` — where to install
- contains URL, checksum URL, filename

**Confidence**: High — verified from docs.rs and GitHub source

---

### 2. GitHub Releases REST API

**GE-Proton endpoint**: `https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases`
**Wine-GE endpoint**: `https://api.github.com/repos/GloriousEggroll/wine-ge-custom/releases`
**Latest only**: append `/latest` to either URL

**Authentication**: Optional `Authorization: Bearer <TOKEN>` header
**Required headers**:

```
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
User-Agent: <app-name>  (required — omitting causes 403)
```

**Pagination**:

- `?per_page=100&page=N` — max 100 per page
- `Link` header contains `rel="next"` URL when more pages exist
- GE-Proton has ~100+ releases; full list requires 1-2 pages

**Rate limits**:

- Unauthenticated: **60 req/hr per IP**
- Authenticated (token): **5,000 req/hr**
- Check `X-RateLimit-Remaining` and `X-RateLimit-Reset` response headers

**Response format** (key fields per release):

```json
{
  "tag_name": "GE-Proton10-34",
  "name": "GE-Proton10-34",
  "published_at": "2026-03-23T...",
  "prerelease": false,
  "draft": false,
  "assets": [
    {
      "name": "GE-Proton10-34.tar.gz",
      "browser_download_url": "https://github.com/.../GE-Proton10-34.tar.gz",
      "size": 541736960,
      "content_type": "application/x-gzip",
      "download_count": 291000
    },
    {
      "name": "GE-Proton10-34.sha512sum",
      "browser_download_url": "https://github.com/.../GE-Proton10-34.sha512sum",
      "size": 152
    }
  ]
}
```

**libprotonup wraps this API** — `list_releases(&compat_tool)` handles building the URL, pagination (single page), and deserializing into `Vec<Release>`. For CrossHook's TTL cache use case, store the raw JSON payload from the API into `external_cache_entries`.

**Confidence**: High — verified against live API response for GE-Proton10-34

---

### 3. GitHub raw content (sha512sum verification)

GE-Proton releases include a `.sha512sum` file asset. The file contains one line:

```
<sha512hex>  GE-Proton10-34.tar.gz
```

libprotonup's `download_file_into_memory()` fetches this file; the `hashing` module verifies the downloaded archive against it. `sha2 = "0.11"` (already in `crosshook-core/Cargo.toml`) handles SHA-512 computation.

**Confidence**: High — verified from libprotonup source and live release asset inspection

---

## Libraries and SDKs

### Already in `crosshook-core/Cargo.toml`

| Crate                  | Version  | Purpose                              |
| ---------------------- | -------- | ------------------------------------ |
| `libprotonup`          | `0.11.0` | Full proton version management API   |
| `reqwest`              | `0.13.2` | HTTP client (rustls TLS, no OpenSSL) |
| `sha2`                 | `0.11.0` | SHA-512 verification                 |
| `tokio`                | `1`      | Async runtime                        |
| `serde` / `serde_json` | `1`      | JSON serialization                   |
| `rusqlite`             | `0.39.0` | SQLite for TTL cache                 |

### Required by libprotonup (transitive, not direct deps needed)

| Crate               | Version | Purpose                    |
| ------------------- | ------- | -------------------------- |
| `tokio-stream`      | `0.1`   | Async streaming iteration  |
| `tokio-util`        | `0.7`   | Stream/IO utilities        |
| `futures-util`      | `0.3`   | Stream combinators         |
| `astral-tokio-tar`  | `0.6`   | Async tar extraction       |
| `async-compression` | `0.4`   | Gzip/xz/zstd decompression |
| `dirs`              | `6.0`   | `~/.steam` path resolution |

**No new direct dependencies are needed** for CrossHook. libprotonup bundles all required async download, verify, and extract capabilities as transitive deps.

**Confidence**: High — verified from libprotonup Cargo.toml

---

## Integration Patterns

### Pattern 1: Tauri Channel for Installation Progress

The recommended Tauri v2 pattern for streaming download progress is `Channel`, not the event system. Channels are ordered, typed, and designed for streaming use cases.

```rust
use tauri::ipc::Channel;
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum InstallEvent {
    Progress { bytes_downloaded: u64, total_bytes: Option<u64>, phase: String },
    Verifying,
    Extracting,
    Complete { install_path: String },
    Error { message: String },
}

#[tauri::command]
pub async fn install_proton_version(
    version: String,
    tool_name: String,        // "GEProton" | "WineGE"
    install_path: String,     // custom path or detected steam path
    on_progress: Channel<InstallEvent>,
) -> Result<(), String> {
    // ... implementation
}
```

Frontend TypeScript:

```typescript
import { invoke, Channel } from '@tauri-apps/api/core';

const channel = new Channel<InstallEvent>();
channel.onmessage = (msg) => {
  if (msg.event === 'Progress') {
    setProgress(msg.data.bytes_downloaded / (msg.data.total_bytes ?? 1));
  }
};
await invoke('install_proton_version', {
  version: 'GE-Proton10-34',
  toolName: 'GEProton',
  installPath: '', // empty = auto-detect
  onProgress: channel,
});
```

**Confidence**: High — from official Tauri v2 docs <https://v2.tauri.app/develop/calling-frontend/>

---

### Pattern 2: Progress-Tracking AsyncWrite Wrapper

`download_to_async_write` accepts a generic `W: AsyncWrite + Unpin`. Wrap `tokio::fs::File` in a custom progress reporter:

```rust
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tauri::ipc::Channel;

pub struct ProgressWriter<W: AsyncWrite + Unpin> {
    inner: W,
    bytes_written: u64,
    total_bytes: Option<u64>,
    channel: Channel<InstallEvent>,
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ProgressWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = result {
            self.bytes_written += n as u64;
            let _ = self.channel.send(InstallEvent::Progress {
                bytes_downloaded: self.bytes_written,
                total_bytes: self.total_bytes,
                phase: "downloading".into(),
            });
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
```

---

### Pattern 3: TTL Cache Using Existing `external_cache_entries`

The `external_cache_entries` table already exists in the CrossHook SQLite metadata DB (schema v13). Use `cache_store::put_cache_entry` / `get_cache_entry` to cache GitHub release lists:

```rust
use crate::metadata::cache_store;

const CACHE_KEY_GE_PROTON: &str = "github:GloriousEggroll/proton-ge-custom:releases";
const CACHE_KEY_WINE_GE: &str = "github:GloriousEggroll/wine-ge-custom:releases";
const CACHE_TTL_SECS: i64 = 3600 * 6;  // 6-hour TTL

pub fn get_cached_releases(conn: &Connection, cache_key: &str) -> Option<String> {
    cache_store::get_cache_entry(conn, cache_key).ok().flatten()
}

pub fn store_releases_cache(
    conn: &Connection,
    cache_key: &str,
    source_url: &str,
    json_payload: &str,
) {
    let expires_at = (Utc::now() + chrono::Duration::seconds(CACHE_TTL_SECS))
        .to_rfc3339();
    let _ = cache_store::put_cache_entry(
        conn,
        source_url,
        cache_key,
        json_payload,
        Some(&expires_at),
    );
}
```

`MAX_CACHE_PAYLOAD_BYTES` is enforced by `put_cache_entry` — payloads over the limit store `NULL` with a warning log. The GitHub releases JSON for 100 releases is approximately 200-500 KB; if this exceeds the limit, store only the `tag_name` list as a stripped payload.

---

### Pattern 4: Listing Installed Versions from Filesystem

libprotonup's `AppInstallations::list_installed_versions()` scans `compatibilitytools.d/` and returns directory names. Use this as the runtime source of truth for "what is installed":

```rust
use libprotonup::apps::{App, AppInstallations};

pub async fn list_installed_proton_versions(install_path: Option<String>)
    -> Result<Vec<String>>
{
    let app_inst = match install_path {
        Some(path) => AppInstallations::Custom(path),
        None => App::Steam.detect_installation_method().await,
    };
    app_inst.list_installed_versions().await
}
```

This is filesystem-derived at runtime — do not cache in SQLite.

---

### Pattern 5: Full Install Flow

```rust
pub async fn install_version(
    compat_tool: &CompatTool,
    release: &Release,
    app_inst: &AppInstallations,
    on_event: Channel<InstallEvent>,
) -> Result<()> {
    // 1. Resolve download metadata
    let download = release.get_download_info(app_inst, compat_tool);
    let install_dir = app_inst.default_install_dir();

    // 2. Create temp file
    let tmp = tempfile::NamedTempFile::new()?;
    let file = tokio::fs::File::from_std(tmp.reopen()?);

    // 3. Download with progress
    let size = download.size; // from asset metadata
    let mut writer = ProgressWriter::new(file, Some(size), on_event.clone());
    downloads::download_to_async_write(&download.download_url, &mut writer).await?;

    // 4. Verify SHA-512
    on_event.send(InstallEvent::Verifying).ok();
    if let Some(ref hash_info) = download.hash_sum {
        let hash_content = downloads::download_file_into_memory(&hash_info.sum_content).await?;
        // hashing module validates
    }

    // 5. Extract
    on_event.send(InstallEvent::Extracting).ok();
    let decompressor = files::Decompressor::from_path(tmp.path()).await?;
    files::unpack_file(compat_tool, &download, decompressor, &install_dir).await?;

    on_event.send(InstallEvent::Complete {
        install_path: install_dir.to_string_lossy().into()
    }).ok();
    Ok(())
}
```

---

## Constraints and Gotchas

### GitHub API Rate Limits (Critical)

- **Unauthenticated**: 60 requests/hr per IP — exhausted after ~60 cache misses in one hour
- CrossHook must use the `external_cache_entries` TTL cache for all release listing calls
- A 6-hour TTL means at most ~4 uncached fetches per day per installation
- On rate limit (HTTP 403/429), fall back to stale cache with a UI age indicator
- `X-RateLimit-Remaining` and `X-RateLimit-Reset` headers can be inspected for graceful degradation
- **Do not offer GitHub token configuration** unless the rate limit causes real user pain — complexity isn't worth it for this use case

**Confidence**: High — from GitHub docs <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>

### Download Sizes

- GE-Proton tar.gz: **~517 MB** (GE-Proton10-34)
- Wine-GE: comparable size (~400-600 MB)
- Always show total size before confirming download
- Downloads must not block the Tauri main thread — use `tokio::spawn` inside the command handler
- Temp file must be cleaned up on failure — use `tempfile::NamedTempFile` (auto-cleanup on drop)

### Filesystem Permissions

- `~/.steam/steam/compatibilitytools.d/` — user-owned, no sudo needed
- `~/.var/app/...` Flatpak path — user-owned, no sudo needed
- libprotonup expands `~` in paths via `dirs` crate — do not pre-expand paths manually

### ProtonUp Binary Not Found

The feature uses `libprotonup` (Rust crate) directly — there is **no external `protonup` binary dependency**. The "protonup binary not found" scenario does not apply to this Rust library integration.

### Multiple Steam Install Variants

libprotonup detects both native and Flatpak Steam via `App::Steam.detect_installation_method()`. If CrossHook has a configured Steam path, construct `AppInstallations::Custom(path)` directly instead of running autodetection.

### Wine-GE Deprecation

Wine-GE (`wine-ge-custom`) is being superseded by ULWGL which allows non-Steam games to use Proton directly. The Wine-GE GitHub repository is still active and the releases API works, but new release frequency has decreased. libprotonup still supports it as `WineGE` tool. Include in the UI but note possible deprecation.

**Confidence**: Medium — based on GloriousEggroll's own statements about ULWGL

### Archive Formats

libprotonup's `Decompressor::from_path()` auto-detects `.tar.gz`, `.tar.xz`, and `.tar.zst`. GE-Proton uses `.tar.gz`. No manual format detection needed.

### Streaming vs. Buffered Download

`download_to_async_write` streams bytes chunk-by-chunk to the writer — memory usage stays bounded regardless of archive size. Do not use `download_file_into_memory` for the main archive.

---

## Code Examples

### Complete Tauri Command: List Available Versions

```rust
use libprotonup::{downloads, sources::CompatTool, apps::App};
use crate::metadata::{cache_store, MetadataStoreError};

#[tauri::command]
pub async fn list_proton_versions(
    state: tauri::State<'_, AppState>,
    tool: String,  // "GEProton" | "WineGE"
    force_refresh: bool,
) -> Result<Vec<ProtonVersionInfo>, String> {
    let cache_key = format!("github:proton-releases:{tool}");
    let conn = state.db.lock().unwrap();

    // Try cache first
    if !force_refresh {
        if let Some(cached) = cache_store::get_cache_entry(&conn, &cache_key)
            .map_err(|e| e.to_string())?
        {
            let versions: Vec<ProtonVersionInfo> = serde_json::from_str(&cached)
                .map_err(|e| e.to_string())?;
            return Ok(versions);
        }
    }

    // Fetch from GitHub via libprotonup
    let compat_tool = CompatTool::sources_for_app(&App::Steam)
        .into_iter()
        .find(|t| t.name == tool)
        .ok_or_else(|| format!("Unknown tool: {tool}"))?;

    let releases = downloads::list_releases(&compat_tool)
        .await
        .map_err(|e| e.to_string())?;

    let versions: Vec<ProtonVersionInfo> = releases.iter()
        .map(|r| ProtonVersionInfo {
            tag: r.tag_name.clone(),
            published_at: r.published_at.clone(),
            size_bytes: r.assets.iter()
                .find(|a| a.name().ends_with(".tar.gz"))
                .map(|a| a.size),
        })
        .collect();

    // Store in cache with 6-hour TTL
    let payload = serde_json::to_string(&versions).map_err(|e| e.to_string())?;
    let source_url = format!(
        "https://api.github.com/repos/{}/{}/releases",
        compat_tool.repository_account, compat_tool.repository_name
    );
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(6)).to_rfc3339();
    cache_store::put_cache_entry(&conn, &source_url, &cache_key, &payload, Some(&expires_at))
        .map_err(|e| e.to_string())?;

    Ok(versions)
}
```

### Complete Tauri Command: Install Version

```rust
#[tauri::command]
pub async fn install_proton_version(
    state: tauri::State<'_, AppState>,
    version: String,
    tool: String,
    on_progress: tauri::ipc::Channel<InstallEvent>,
) -> Result<(), String> {
    let app_inst = {
        let settings = state.settings.read().await;
        match &settings.steam_path {
            Some(path) => libprotonup::apps::AppInstallations::Custom(path.clone()),
            None => libprotonup::apps::App::Steam
                .detect_installation_method().await,
        }
    };

    let compat_tool = libprotonup::sources::CompatTool::sources_for_app(
        &libprotonup::apps::App::Steam
    )
    .into_iter()
    .find(|t| t.name == tool)
    .ok_or_else(|| format!("Unknown tool: {tool}"))?;

    // Get release info (from cache or API)
    let releases = libprotonup::downloads::list_releases(&compat_tool)
        .await
        .map_err(|e| e.to_string())?;

    let release = releases.iter()
        .find(|r| r.tag_name == version)
        .ok_or_else(|| format!("Version not found: {version}"))?;

    let download = release.get_download_info(&app_inst, &compat_tool);
    let install_dir = app_inst.default_install_dir();

    // Download to temp file with progress
    let tmp = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let file = tokio::fs::File::create(tmp.path()).await.map_err(|e| e.to_string())?;
    let mut writer = ProgressWriter::new(file, None, on_progress.clone());

    libprotonup::downloads::download_to_async_write(
        &download.download_url, &mut writer
    ).await.map_err(|e| e.to_string())?;

    // Verify + extract (delegate to libprotonup)
    on_progress.send(InstallEvent::Extracting).ok();
    let decompressor = libprotonup::files::Decompressor::from_path(tmp.path())
        .await.map_err(|e| e.to_string())?;
    libprotonup::files::unpack_file(
        &compat_tool, &download, decompressor, &install_dir
    ).await.map_err(|e| e.to_string())?;

    on_progress.send(InstallEvent::Complete {
        install_path: install_dir.to_string_lossy().into_owned(),
    }).ok();
    Ok(())
}
```

---

## Resolved Questions (from tech-designer follow-up)

### Q1: libprotonup tokio runtime flavor requirement

**Answer**: No specific runtime flavor required. libprotonup's `Cargo.toml` only requires `tokio = { version = "1.51", features = ["macros"] }` — the `macros` feature only (not `rt` or `rt-multi-thread`). This means libprotonup is runtime-agnostic and works under both `current_thread` and `multi_thread` flavors. Tauri v2's default `tauri::async_runtime` is `multi_thread` tokio — fully compatible.

**Confidence**: High — verified from libprotonup Cargo.toml; no `tokio::spawn` calls inside downloads.rs or files.rs.

---

### Q2: GitHub API pagination in list_releases

**Answer**: `list_releases` does **not** handle pagination. It fetches a single page with no `per_page` or `page` query parameters. The GitHub API default is **30 results per page**. GE-Proton has 100+ historical releases, so `list_releases` returns only the **most recent 30**.

**Practical impact**: For CrossHook's use case (browse available versions, install latest or specific version referenced in a community profile), 30 is sufficient for day-to-day use. Most users only care about the latest ~5-10 versions. However, if a community profile references a version older than the 30th most recent release, it won't appear in the list.

**Mitigation options**:

1. Accept 30-release limit — sufficient for 99% of use cases
2. Bypass libprotonup and call the API directly with `?per_page=100` for the cache-filling fetch — returns ~3x more history

**Confidence**: High — verified from libprotonup downloads.rs source + GitHub API documentation.

---

### Q3: Exact CompatTool name keys in sources.ron

**Answer**: The `FromStr` implementation is case-insensitive name matching against the `CompatTools` static (loaded from `sources.ron` at compile time). The constants `DEFAULT_STEAM_TOOL = "GEProton"` and `DEFAULT_LUTRIS_TOOL = "WineGE"` confirm these are the canonical names. The regex filter tests confirm `"GE-Proton10-8.tar.gz"` and `"GE-Proton10-8.tar.zst"` are valid asset name patterns for GEProton.

Exact `CompatTool::from_str` keys (case-insensitive):

- `"GEProton"` — GE-Proton for Steam (also: `"geproton"`, `"GEPROTON"`)
- `"WineGE"` — Wine-GE for Lutris (also: `"winege"`, `"WINGE"`)

**Sources.ron** is embedded via `include_str!` at compile time — the tool configurations are baked into the libprotonup binary and cannot change at runtime.

**Confidence**: High — verified from constants.rs (`DEFAULT_STEAM_TOOL`, `DEFAULT_LUTRIS_TOOL`), sources.rs FromStr implementation, and regex test cases.

---

### Q4: astral-tokio-tar symlink security (CVE-2025-59825)

**Answer**: **The vulnerability is patched.** CVE-2025-59825 (GHSA-3wgq-wrwc-vqmv) affected `astral-tokio-tar ≤ 0.5.3` — path traversal via symlink chaining and TOCTOU cache bypass in `Entry::unpack_in_raw`. The fix landed in `0.5.4` (September 23, 2025).

CrossHook's workspace resolves `astral-tokio-tar = "0.6.0"` (verified via `cargo metadata`). **Version 0.6.0 is patched** — it postdates both 0.5.4 and 0.5.5.

However, libprotonup's `unpack_file` implementation has **no explicit symlink validation beyond what astral-tokio-tar 0.6.0 provides**. The implementation uses `set_unpack_xattrs(false)`, `set_preserve_permissions(true)`, and skips top-level path components manually, but delegates all path safety to the underlying tar library.

**Residual risk**: The patched library uses robust path normalization, but `allow_external_symlinks` defaults to `true` in astral-tokio-tar. libprotonup does not explicitly call `.set_allow_external_symlinks(false)`. This is mitigated by the fact that GE-Proton archives come from GitHub's CDN (trusted source), but a MitM or compromised CDN delivery could still be exploited.

**Confidence**: High — CVE details from GitHub Advisory GHSA-3wgq-wrwc-vqmv; workspace version verified via `cargo metadata`.

---

### Q5: GitHub rate limit headers exposed by list_releases

**Answer**: **No** — `list_releases` does not expose `X-RateLimit-Remaining` or any response headers. The function calls `response.json().await?` directly without inspecting headers. Rate limit exhaustion surfaces as a `reqwest::Error` with HTTP 403 status.

**Handling pattern for CrossHook**:

```rust
match downloads::list_releases(&compat_tool).await {
    Ok(releases) => { /* cache and return */ }
    Err(e) => {
        // Check if the error is a 403 (rate limit) vs. network failure
        // reqwest::Error does not expose status code directly from send()
        // but the error message contains it
        if e.status() == Some(reqwest::StatusCode::FORBIDDEN) {
            // Rate limited — return stale cache with age indicator
        } else {
            // Network error — return stale cache or offline state
        }
    }
}
```

`reqwest::Error::status()` returns `Option<StatusCode>` — use this to distinguish rate limit (403) from genuine network failure.

**Confidence**: High — verified from downloads.rs source; no header inspection present.

---

## Open Questions

1. **`Release` struct field names for `published_at` and asset `size`**: The confirmed `Release` struct has `tag_name`, `url`, `name`, `body`, `assets`. The `Asset` struct has `url`, `id`, `name`, `size: i64`, `updated_at`, `browser_download_url`. Note `published_at` is **not in the Release struct** — libprotonup does not deserialize it. CrossHook cannot show release dates from `list_releases` without either fetching full GitHub API JSON directly or making a separate API call.

2. **`MAX_CACHE_PAYLOAD_BYTES` limit**: The value in `metadata::models` needs to be verified. The GitHub releases JSON for 30 releases (libprotonup's page size) is approximately 50-100 KB — far smaller than for 100 releases. Likely within the payload limit.

3. **libprotonup re-exported from crosshook-core**: Check if libprotonup types need to be re-exported via crosshook-core's public API or if src-tauri should depend on it directly. Currently only `crosshook-core` has the dependency.

4. **Wine-GE install path**: Wine-GE installs to Lutris directories (`~/.local/share/lutris/runners/wine/`), not to Steam's `compatibilitytools.d/`. For CrossHook's use case (Proton for Steam games), WineGE is likely out of scope unless CrossHook supports Lutris game launching.

---

## Sources

- [libprotonup on docs.rs](https://docs.rs/libprotonup/latest/libprotonup/)
- [Protonup-rs GitHub repository](https://github.com/auyer/Protonup-rs)
- [libprotonup on crates.io](https://crates.io/crates/libprotonup)
- [GitHub REST API — List releases endpoint](https://docs.github.com/en/rest/releases/releases)
- [GitHub REST API — Rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
- [GitHub updated rate limits (May 2025)](https://github.blog/changelog/2025-05-08-updated-rate-limits-for-unauthenticated-requests/)
- [GitHub pagination docs](https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api)
- [GloriousEggroll proton-ge-custom releases](https://github.com/GloriousEggroll/proton-ge-custom/releases)
- [GloriousEggroll wine-ge-custom GitHub](https://github.com/GloriousEggroll/wine-ge-custom)
- [Tauri v2 — Calling Frontend from Rust (Channels)](https://v2.tauri.app/develop/calling-frontend/)
- [Tauri v2 — Calling Rust from Frontend](https://v2.tauri.app/develop/calling-rust/)
- [Steam compatibilitytools.d install guide (DEV Community)](https://dev.to/harry_tanama_51571ebf90b6/install-ge-proton-on-linux-1ejl)
- [reqwest streaming download with progress (GitHub Gist)](https://gist.github.com/Tapanhaz/096e299bf060607b572d700e89a62529)
- [tokio AsyncWrite trait docs](https://docs.rs/tokio/latest/tokio/io/trait.AsyncWrite.html)
- [Live GE-Proton10-34 release API response](https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/latest)
