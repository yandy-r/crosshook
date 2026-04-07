# Feature Spec: ProtonUp Integration

## Executive Summary

CrossHook currently dead-ends when a profile references a Proton version not installed on disk, showing a path-not-found error with no remediation. This feature adds first-class Proton version management: listing available GE-Proton/Wine-GE releases, installing them in-app with real-time progress, and auto-suggesting versions from community profiles. The implementation leverages `libprotonup = "0.11.0"` (already declared in `crosshook-core/Cargo.toml` but unused) as the download/verify/extract engine, the existing `external_cache_entries` SQLite table for TTL-cached version lists, and the existing `discover_compat_tools` scanner for installed versions. No new DB tables, no new crate dependencies. One CRITICAL security finding — tar path traversal via CVE-2025-62518 in abandoned `tokio-tar` — is resolved by pinning `libprotonup >= 0.9.1` (which migrated to patched `astral-tokio-tar`).

## External Dependencies

### APIs and Services

#### GitHub Releases REST API

- **Documentation**: <https://docs.github.com/en/rest/releases/releases>
- **Authentication**: Optional `Authorization: Bearer <TOKEN>` (unauthenticated by default)
- **Key Endpoints**:
  - `GET /repos/GloriousEggroll/proton-ge-custom/releases`: List all GE-Proton releases
  - `GET /repos/GloriousEggroll/wine-ge-custom/releases`: List all Wine-GE releases
  - `GET /repos/{owner}/{repo}/releases/latest`: Latest release only
- **Rate Limits**: 60 req/hr unauthenticated; 5,000 req/hr with token
- **Pricing**: Free (public repositories)
- **Note**: `libprotonup::downloads::list_releases` wraps this API — CrossHook does not call it directly

#### GE-Proton Release Assets

- **SHA-512 Checksums**: Each release includes a `.sha512sum` file alongside the `.tar.gz` tarball
- **Archive Format**: `.tar.gz` (300-600 MB per version); `libprotonup` auto-detects `.tar.gz`, `.tar.xz`, `.tar.zst`
- **No GPG Signing**: GloriousEggroll does not provide GPG-signed releases; SHA-512 is the practical trust anchor

### Libraries and SDKs

All already in `crosshook-core/Cargo.toml` — no new dependencies:

| Library       | Version     | Purpose                                                                   |
| ------------- | ----------- | ------------------------------------------------------------------------- |
| `libprotonup` | `0.11.0`    | Version listing, streaming download, SHA-512 verification, tar extraction |
| `reqwest`     | `0.13.2`    | HTTP client (rustls TLS, no OpenSSL) — used by libprotonup                |
| `sha2`        | `0.11.0`    | SHA-512 hash computation — used by libprotonup                            |
| `tokio`       | `1`         | Async runtime with `sync` feature for `Mutex`/`mpsc`                      |
| `rusqlite`    | `0.39.0`    | SQLite for `external_cache_entries` TTL cache                             |
| `nix`         | (workspace) | `statvfs` for pre-download disk space check                               |

### External Documentation

- [libprotonup docs.rs](https://docs.rs/libprotonup/latest/libprotonup/): Rust API reference
- [protonup-rs GitHub](https://github.com/auyer/Protonup-rs): Source repository
- [GE-Proton releases](https://github.com/GloriousEggroll/proton-ge-custom/releases): Release assets
- [Tauri v2 Channels](https://v2.tauri.app/develop/calling-frontend/): Progress streaming pattern

## Business Requirements

### User Stories

**Primary User: Linux/Steam Deck gamer**

- As a gamer, I want to see available GE-Proton and Wine-GE releases so that I can choose the right version for my game
- As a gamer, I want to install a Proton version directly in CrossHook so that I never need a terminal or ProtonUp-Qt
- As a gamer, I want real-time download/install progress so that I know when installation will complete
- As a gamer, I want auto-suggestions when a community profile requires a missing version so that I install exactly the right one
- As a gamer, I want cached version lists when offline so that I can plan installs when connectivity returns
- As a gamer, I want profile launch to never be blocked by ProtonUp integration so that I can always play
- As a Steam Deck user, I want a preferred Proton version setting so that new profiles default to my choice

### Business Rules

1. **BR-1 — Never block profile launch.** Profile launch proceeds if `runtime.proton_path` is valid on disk. ProtonUp is an install/suggestion layer — it must not intercept or gate the launch command. Unconditional.

2. **BR-2 — Installed versions from filesystem.** The authoritative list of installed Proton versions is derived at runtime by scanning `compatibilitytools.d/` via existing `discover_compat_tools`. Never persisted to SQLite or TOML.

3. **BR-3 — Available versions via external cache (TTL = 6h).** Fetched from GitHub via `libprotonup` and cached in `external_cache_entries` with 6-hour TTL. Cache key: `protonup:versions:v1:{tool_slug}`.

4. **BR-4 — Stale-fallback with no hard limit.** On fetch failure, serve stale cache with visible age indicator. No expiry ceiling — Steam Deck users on airplane mode must see a useful cached list indefinitely.

5. **BR-5 — Preferred Proton version in TOML.** New `preferred_proton_version` field in `AppSettingsData` with `#[serde(default)]` for backward compatibility.

6. **BR-6 — Auto-suggest from community profiles.** When `community_profiles.proton_version` is non-null and non-empty, and the version is not installed, surface a non-blocking suggestion. Never block import or use of the profile.

7. **BR-7 — No external binary required.** `libprotonup` (Rust crate) provides all capabilities. No `protonup` or `protonup-qt` binary needed at runtime.

8. **BR-8 — Download/install are ephemeral state.** Progress lives in React state / Tauri events only. Not persisted.

9. **BR-9 — One active install at a time.** Backend `ProtonupInstallState` mutex (modeled after `PrefixDepsInstallLock`) prevents concurrent installs. UI additionally disables buttons.

10. **BR-10 — Offline mode respects settings.** When `offline_mode == true`, no network calls. Serve cache or show offline message.

11. **BR-11 — Disk space warning before download.** GE-Proton tarballs are 500 MB-1.5 GB. Query available disk space and warn if insufficient. Warning is non-blocking (user may proceed).

### Edge Cases

| Scenario                                 | Expected Behavior                                        | Notes                                                  |
| ---------------------------------------- | -------------------------------------------------------- | ------------------------------------------------------ |
| Version already installed                | No-op with positive feedback ("already installed")       | Install button disabled for installed versions         |
| Partial/interrupted download             | Restart from zero; temp files cleaned up                 | No resume support in libprotonup 0.11.0                |
| Multiple Steam library roots             | Install to primary root; user override via `install_dir` | Multi-library selection deferred to Phase 3            |
| Community `proton_version` is null/empty | No suggestion shown                                      | Field is nullable, free-form, advisory                 |
| Steam Deck read-only system paths        | Graceful error; never attempt system-level install       | Only target user-owned `compatibilitytools.d/`         |
| GitHub rate limit exhausted              | Serve stale cache with age indicator; retry timer        | 60 req/hr with 6h TTL = at most 4 uncached fetches/day |
| `libprotonup` unrecoverable error        | Show guidance, not crash; profile launch unaffected      | Feature degraded, not broken                           |

### Success Criteria

- [ ] Users can browse available GE-Proton/Wine-GE versions in CrossHook
- [ ] Users can install any listed version with real-time progress
- [ ] After install, new version appears in Proton path dropdown without app restart
- [ ] Community profile missing-version suggestion appears as non-blocking badge
- [ ] Offline mode shows cached list with age indicator; install disabled with explanation
- [ ] Profile launch never blocked by ProtonUp integration
- [ ] Disk space warning shown before download when space is insufficient
- [ ] Already-installed versions produce clear feedback, not errors

## Technical Specifications

### Architecture Overview

```
                     +-------------------------------------------+
                     |             crosshook-core                 |
                     |                                            |
                     |  +-------------------------------------+  |
                     |  |   src/protonup/                      |  |
                     |  |                                      |  |
                     |  |  fetcher.rs   (async, cached)        |  |
                     |  |  scanner.rs   (sync, filesystem)     |  |
                     |  |  installer.rs (async, streaming)     |  |
                     |  |  advisor.rs   (sync, profile match)  |  |
                     |  |  models.rs    (Serde types)          |  |
                     |  |  error.rs     (typed errors)         |  |
                     |  +------------------+------------------+  |
                     |                     | uses                 |
                     |   metadata::cache_store  (TTL cache)       |
                     |   settings::AppSettingsData (TOML)         |
                     |   steam::proton (filesystem scanner)       |
                     |   libprotonup (GitHub API + extract)       |
                     +--------------------+----------------------+
                                          | pub fn / pub async fn
                     +--------------------v----------------------+
                     |   src-tauri/commands/protonup.rs           |
                     |                                            |
                     |   ProtonupInstallState (Arc<Mutex>)        |
                     |   5 #[tauri::command] handlers             |
                     +--------------------+----------------------+
                                          | Tauri IPC + Events
                     +--------------------v----------------------+
                     |   React frontend                           |
                     |   ProtonVersionManager + hooks             |
                     +-------------------------------------------+
```

### Data Models

#### Cache: `ProtonVersionListCache` (stored in `external_cache_entries.payload_json`)

```json
{
  "tool_name": "GEProton",
  "fetched_at": "2026-04-06T12:00:00Z",
  "versions": [
    {
      "tag_name": "GE-Proton10-34",
      "published_at": "2026-03-23T...",
      "download_url": "https://github.com/.../GE-Proton10-34.tar.gz",
      "size_bytes": 541736960,
      "checksum_url": "https://github.com/.../GE-Proton10-34.sha512sum",
      "checksum_type": "sha512"
    }
  ]
}
```

Cache key: `protonup:versions:v1:ge-proton` (or `wine-ge`). TTL: 6 hours. Size: ~80-150 KB for 100 versions (within 512 KiB `MAX_CACHE_PAYLOAD_BYTES` limit).

#### Rust Types (`protonup/models.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableProtonVersion {
    pub tag_name: String,
    pub published_at: String,
    pub download_url: String,
    pub size_bytes: u64,
    pub checksum_url: Option<String>,
    pub checksum_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledProtonVersion {
    pub name: String,
    pub proton_executable_path: String,
    pub is_official: bool,
    pub source: InstalledProtonSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonInstallProgress {
    pub tool_name: String,
    pub version_tag: String,
    pub phase: String,  // "downloading" | "verifying" | "extracting" | "complete" | "error"
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percent: u8,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonVersionSuggestion {
    pub profile_name: String,
    pub required_version: String,
    pub is_installed: bool,
    pub closest_installed: Option<String>,
    pub available_version: Option<AvailableProtonVersion>,
}
```

#### Settings Addition (`AppSettingsData`)

```rust
#[serde(default, skip_serializing_if = "String::is_empty")]
pub preferred_proton_version: String,
```

Zero-migration: `#[serde(default)]` ensures backward compatibility with existing `settings.toml` files.

### API Design (Tauri IPC Commands)

#### `list_available_proton_versions`

**Purpose**: List available GE-Proton or Wine-GE versions from cache/GitHub

```rust
#[tauri::command]
pub async fn list_available_proton_versions(
    tool_name: String,       // "GEProton" | "WineGE"
    force_refresh: bool,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<VersionListResponse, String>
```

Response includes `versions`, `fetched_at`, `is_stale`, `is_offline` for cache-age banner rendering.

**Errors**: `offline_mode_enabled`, `network_error`, `invalid_tool_name`

#### `install_proton_version`

**Purpose**: Download, verify, and extract a Proton version

```rust
#[tauri::command]
pub async fn install_proton_version(
    request: ProtonInstallRequest,
    app: AppHandle,
    install_state: State<'_, ProtonupInstallState>,
) -> Result<(), String>
```

Emits `protonup-install-progress` events with phase-by-phase progress. Pre-flight checks: already installing, version already installed, disk space.

#### `get_installed_proton_versions`

**Purpose**: Scan filesystem for installed Proton versions

```rust
#[tauri::command]
pub async fn get_installed_proton_versions(
    steam_client_install_path: Option<String>,
) -> Result<Vec<InstalledProtonVersion>, String>
```

Delegates to `discover_compat_tools` via `From<ProtonInstall> for InstalledProtonVersion`.

#### `get_proton_install_progress`

**Purpose**: Poll current install state (for UI reconnection after navigation)

```rust
#[tauri::command]
pub fn get_proton_install_progress(
    version_tag: String,
    install_state: State<'_, ProtonupInstallState>,
) -> Option<ProtonInstallProgress>
```

#### `suggest_proton_version_for_profile`

**Purpose**: Cross-reference community profile `proton_version` with installed versions

```rust
#[tauri::command]
pub async fn suggest_proton_version_for_profile(
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Option<ProtonVersionSuggestion>, String>
```

Uses `normalize_alias` (promoted from `pub(crate)` to `pub`) for fuzzy matching.

### System Integration

#### Files to Create

| File                                       | Purpose                                       |
| ------------------------------------------ | --------------------------------------------- |
| `crosshook-core/src/protonup/mod.rs`       | Module root, public re-exports                |
| `crosshook-core/src/protonup/models.rs`    | Serde data types                              |
| `crosshook-core/src/protonup/fetcher.rs`   | Version list fetcher with cache-first pattern |
| `crosshook-core/src/protonup/scanner.rs`   | Thin wrapper over `discover_compat_tools`     |
| `crosshook-core/src/protonup/installer.rs` | Download + verify + extract pipeline          |
| `crosshook-core/src/protonup/advisor.rs`   | Profile version suggestion                    |
| `crosshook-core/src/protonup/error.rs`     | Typed `ProtonupError` enum                    |
| `src-tauri/src/commands/protonup.rs`       | Tauri command handlers                        |

#### Files to Modify

| File                                 | Change                                               |
| ------------------------------------ | ---------------------------------------------------- |
| `crosshook-core/src/lib.rs`          | Add `pub mod protonup;`                              |
| `crosshook-core/src/settings/mod.rs` | Add `preferred_proton_version` to `AppSettingsData`  |
| `crosshook-core/src/steam/proton.rs` | Promote `normalize_alias` from `pub(crate)` to `pub` |
| `src-tauri/src/commands/mod.rs`      | Add `pub mod protonup;`                              |
| `src-tauri/src/lib.rs`               | Register commands + `.manage(ProtonupInstallState)`  |

## UX Considerations

### User Workflows

#### Primary: Browse and Install

1. **Open Proton Manager** — User navigates to Settings > "Proton Versions" `CollapsibleSection`
2. **View Lists** — Installed versions (filesystem, instant) shown first; available versions (cache/network) load with skeleton rows
3. **Search/Filter** — Live text filter (debounced 200ms), sort dropdown (Latest/Oldest/A-Z), type filter chips (GE-Proton/Wine-GE)
4. **Install** — Click [Install] on version row; row transitions to inline progress bar with percentage, speed, ETA
5. **Phase Feedback** — `Downloading... 45% (12.3 MB/s)` → `Verifying checksum...` → `Extracting...` → `Installed` chip
6. **Complete** — Toast notification; version appears in installed section; Proton path dropdown updates reactively

#### Error Recovery: Missing Proton Path

1. Profile health check detects missing `proton_path`
2. Enhanced remediation: "Proton version 'GE-Proton9-27' may not be installed. [Open Proton Manager]"
3. User installs version, re-selects path in profile

#### Auto-Suggest from Community Profile

1. Community profile card shows `proton_version` with [Not Installed] chip if missing
2. Import wizard shows "Required Proton version: GE-Proton9-27 [Not Installed]" with optional install checkbox
3. Profile import never blocked regardless of install decision

### UI Patterns

| Component        | Pattern                                               | Notes                                   |
| ---------------- | ----------------------------------------------------- | --------------------------------------- |
| Version list     | Two `CollapsibleSection` groups (Installed/Available) | Matches CommunityBrowser pattern        |
| Status chips     | `crosshook-status-chip` (green = Installed)           | Matches HealthBadge vocabulary          |
| Progress bar     | Inline on version row, determinate with phase label   | Cancel visible during download only     |
| Cache-age banner | `crosshook-community-browser__cache-banner` pattern   | "Offline - cached list from 3h ago"     |
| Search/filter    | Live text + `ThemedSelect` sort dropdown              | Matches CommunityBrowser `matchesQuery` |
| Empty state      | Instructional text with action link                   | "No versions installed. Install below." |

### Accessibility Requirements

- All list rows keyboard-navigable (Tab/Enter)
- [Install]/[Delete] buttons: `aria-label="Install GE-Proton10-34"`
- Progress bar: `role="progressbar"` with `aria-valuenow/min/max`
- Cache-age banners: `role="status"` `aria-live="polite"`
- Respect `prefers-reduced-motion` for skeleton shimmer and progress animation

### Performance UX

- **Loading**: Serve cache immediately (zero perceived load); revalidate in background if TTL expired (stale-while-revalidate)
- **Progress**: Tauri event streaming at ~1 event per 0.5% (max 200 events per install)
- **Installed list**: Synchronous filesystem scan — no skeleton needed
- **Background downloads**: React context holds install state; mini badge/count indicator persists across navigation

## Recommendations

### Implementation Approach

**Recommended Strategy**: Use `libprotonup = "0.11.0"` (Option A) as the primary engine. It provides native async streaming download, SHA-512 verification, and tar extraction. CrossHook builds only the coordination layer: cache management, progress event emission, install lock, and Tauri IPC surface.

**Phasing:**

1. **Phase 1 — Foundation**: Backend `protonup/` module in `crosshook-core` + Tauri commands. No UI. Includes: models, fetcher (cache-first), installer (download/verify/extract), advisor (profile matching), settings field, install lock, unit tests, security audit of extraction.

2. **Phase 2 — Core UI**: `ProtonVersionManager` component in Settings panel, `useProtonVersions` and `useInstallProtonVersion` hooks, error states, offline fallback display, settings panel additions.

3. **Phase 3 — Polish**: Community profile auto-suggest integration, post-install profile path suggestion, Wine-GE support (second cache key + channel toggle), installed version cleanup UI with orphan detection.

### Technology Decisions

| Decision           | Recommendation                                             | Rationale                                              |
| ------------------ | ---------------------------------------------------------- | ------------------------------------------------------ |
| Engine             | `libprotonup` directly (not shell out)                     | Already a dep; native async; no external binary needed |
| Cache format       | Normalized `ProtonVersionListCache` (not raw release JSON) | Decouples from libprotonup struct churn                |
| Progress transport | Tauri event bus + poll command for reconnect               | Real-time push; consistent with `update.rs` pattern    |
| Install directory  | Auto-detect + user override                                | `libprotonup` resolves native/Flatpak Steam paths      |
| Cache TTL          | 6 hours, fixed                                             | Matches ProtonDB pattern; 60 req/hr limit is ample     |

### Quick Wins

- `protonup_list_installed` reuses `discover_compat_tools` via `From<ProtonInstall>` — near-zero new logic
- `cancel_proton_install` adds ~20 lines following `cancel_update` pattern
- Stale-cache fallback path copies directly from `protondb/client.rs:109-129`
- `preferred_proton_version` slots alongside existing `default_proton_path` with serde default

### Future Enhancements

- **Download resume**: HTTP range request support (requires upstream libprotonup or custom download)
- **Auto-update notifications**: Background check for newer releases on startup
- **Version pinning per profile**: Store locked version in profile TOML
- **Cleanup old versions**: Orphan detection with size-reclaim estimates

## Risk Assessment

### Technical Risks

| Risk                                                | Likelihood | Impact   | Mitigation                                                        |
| --------------------------------------------------- | ---------- | -------- | ----------------------------------------------------------------- |
| Tar path traversal in extraction (CVE-2025-62518)   | Medium     | Critical | Pin `libprotonup >= 0.9.1`; add CrossHook path validation guard   |
| No download resume (500MB-1.5GB restarts from zero) | Medium     | High     | Document as known limitation; track for future; show UX warning   |
| `libprotonup` 0.x API breaking change               | Medium     | High     | Pin `=0.11.0`; integration tests on API surface                   |
| GitHub rate limit under shared NAT                  | Low-Med    | High     | 6h TTL cache; serve stale up to 7 days; force_refresh escape      |
| Disk space exhaustion during download/extraction    | Medium     | High     | `statvfs` pre-check; warn at < 2x tarball size                    |
| Concurrent install race via two Tauri calls         | Low        | High     | Backend `Mutex<Option<AbortHandle>>`; UI guards are supplementary |

### Integration Challenges

- `normalize_alias` visibility: promote from `pub(crate)` to `pub` (one-line change)
- `list_proton_installs` overlap: implement `From<ProtonInstall>` conversion; keep both commands in v1
- Frontend scroll container: new panel must be added to `SCROLLABLE` selector in `useScrollEnhance.ts`
- Settings IPC DTO: `AppSettingsIpcData` must stay in sync with `AppSettingsData` manually

### Security Considerations

#### Critical -- Hard Stops

| Finding                                                                                | Risk                                                                                                      | Required Mitigation                                                                                                                                               |
| -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Three-CVE chain in `astral-tokio-tar` (CVE-2025-62518, CVE-2025-59825, CVE-2026-32766) | Arbitrary file writes outside install dir via PAX header desync, symlink TOCTOU, PAX extension validation | Pin `libprotonup = "0.11.0"` (uses `astral-tokio-tar = "0.6"`); verify `Cargo.lock` shows `astral-tokio-tar 0.6.x`; verify no `tokio-tar` (abandoned) in lockfile |
| `install_dir` user-supplied path traversal                                             | Extraction can target any user-writable directory                                                         | Validate `install_dir` is within home or approved Steam library path; reject paths with `..`; reject symlinks                                                     |
| Archive bomb (no decompression limits)                                                 | Crafted 300 MB tar.gz fills disk                                                                          | Enforce max extracted size (10x compressed or 4 GB hard cap) and max file count (10,000); abort and clean up if exceeded                                          |

#### Warnings -- Must Address

| Finding                                       | Risk                                    | Mitigation                                                                  | Alternatives                  |
| --------------------------------------------- | --------------------------------------- | --------------------------------------------------------------------------- | ----------------------------- |
| No rate limit handling for GitHub API         | Cache-miss storms could trigger IP bans | Exponential backoff; respect `X-RateLimit-*` headers; enforce TTL cache     | Optional GitHub PAT in future |
| Partial downloads leave temp files on SIGKILL | Disk space leak                         | Use OS-backed `tempfile()`; SIGINT/SIGTERM cleanup via tokio::signal        | Accept for abnormal exits     |
| Checksum from same server as binary           | Single trust anchor                     | Document accepted risk; SHA-512 is industry standard for this class of tool | GPG if upstream adds it       |
| No path validation on extracted entries       | Escape even with patched tar library    | Validate each entry path against install root before writing                | Defense-in-depth              |

#### Advisories -- Best Practices

- GPG verification: not available upstream; defer
- `#[serde(deny_unknown_fields)]` on GitHub API response structs
- Version string allowlist validation: `^[A-Za-z0-9][A-Za-z0-9.\-_]{0,99}$`
- HTTPS-only URL validation before any download request
- Download URL hostname allowlist: `github.com`, `objects.githubusercontent.com`

## Task Breakdown Preview

### Phase 1: Foundation

**Focus**: Backend module + Tauri commands, no UI

**Tasks**:

- Promote `normalize_alias` to `pub` in `steam/proton.rs`
- Implement `protonup/models.rs` (all Serde types, `From<ProtonInstall>`)
- Implement `protonup/fetcher.rs` (cache-first with stale fallback, 6h TTL)
- Implement `protonup/installer.rs` (async download, SHA-512 verify, extract, disk check, path validation)
- Implement `protonup/advisor.rs` (fuzzy version matching via `normalize_alias`)
- Implement `protonup/error.rs` (typed error enum)
- Add `preferred_proton_version` to `AppSettingsData`
- Implement `ProtonupInstallState` (install lock + abort handle)
- Register 5 Tauri commands in `src-tauri`
- Security audit: verify `libprotonup >= 0.9.1` extraction behavior; add path guard
- Unit tests: cache logic, `From<ProtonInstall>`, path validation, idempotency, stale fallback

**Parallelization**: `models.rs`, settings field, `normalize_alias` promotion can run in parallel. `fetcher.rs` and `installer.rs` depend on models. `advisor.rs` depends on `normalize_alias`.

### Phase 2: Core UI

**Focus**: Proton Manager component + hooks

**Dependencies**: Phase 1 complete
**Tasks**:

- `useProtonVersions.ts` hook (list + offline/stale state + cache age)
- `useInstallProtonVersion.ts` hook (phase streaming, cancel, reconnect on mount)
- `ProtonVersionManager` component (browse, filter, install, progress, offline banner)
- Settings panel integration (CollapsibleSection, preferred version selector)
- Profile health warning enhancement (link to Proton Manager)
- Add scroll container to `SCROLLABLE` selector

### Phase 3: Polish

**Focus**: Community integration + Wine-GE + cleanup

**Tasks**:

- Community profile auto-suggest via `advisor.rs` (CommunityBrowser chip + import wizard)
- Post-install profile path suggestion
- Wine-GE support (second cache key, `VersionChannel` toggle)
- Installed version cleanup UI (orphan detection, delete with size reclaim)
- Download resume investigation (upstream libprotonup or Option B)

## Decisions (Resolved)

All decisions resolved prior to implementation planning:

1. **GE-Proton only in Phase 1** — Wine-GE deferred to Phase 3. Keeps Phase 1 scope tight; Wine-GE adds a `VersionChannel` enum variant and second cache key with low additional effort but increases testing surface.

2. **Fixed 6h cache TTL** — No user-configurable TTL. `force_refresh` serves as the manual escape hatch. Configure only if users report issues.

3. **Primary Steam root in Phase 1** — Multi-library install target selector deferred to Phase 3. Default to the detected primary Steam root via `libprotonup::apps::AppInstallations`.

4. **2x tarball size as soft disk space warning** — Non-blocking: user may proceed after seeing the warning. On Steam Deck with constrained `~/.var` partition, the warning is especially useful but never a hard block.

5. **License: update CrossHook if needed** — `libprotonup` is GPL-3.0. CrossHook will update its license to be compatible if needed and makes sense. Don't blindly update, and bring it up to the team for discussion if you're unsure.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): libprotonup API surface, GitHub releases API, integration patterns
- [research-business.md](./research-business.md): User stories, business rules, workflows, domain model
- [research-technical.md](./research-technical.md): Architecture, data models, API contracts, codebase changes
- [research-ux.md](./research-ux.md): UX workflows, competitive analysis, component patterns, accessibility
- [research-security.md](./research-security.md): Severity-leveled findings (1 CRITICAL, 4 WARNING, 5 ADVISORY)
- [research-practices.md](./research-practices.md): Code reuse, modularity, KISS assessment, build-vs-depend
- [research-recommendations.md](./research-recommendations.md): Phasing, alternatives, risk assessment, task breakdown

## Implementation Notes

These notes record concrete implementation realities that differ from the original spec assumptions. The confirmed decisions above remain unchanged.

### Actual file structure

The implementation created the following files under `src/crosshook-native/crates/crosshook-core/src/protonup/`:

- `mod.rs` — shared DTOs and service interface (as spec'd)
- `catalog.rs` — catalog retrieval with cache-live-stale fallback
- `install.rs` — install orchestration with security guardrails
- `matching.rs` — pure advisory match logic

The file `protonup/service.rs` specified in the original technical spec was not created. Catalog, install, and matching responsibilities are distributed across the three module files above instead of being unified in a single service module.

### Provider integration

The implementation uses the GitHub Releases API directly (`https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases`) rather than the `libprotonup` library. The `libprotonup` dependency was not added to `Cargo.toml`. The spec listed `libprotonup` as the primary integration path with GitHub Releases as a fallback/direct-catalog mode; in v1 the GitHub Releases path is the sole and primary integration.

### `version_snapshots` status

The `version_snapshots` table is not used by the ProtonUp integration. All catalog data is stored through the existing `external_cache_entries` table in the SQLite metadata DB. `version_snapshots` remains deferred and optional in v1 — no migration or schema change was introduced.

### Settings fields added

Two fields were added to `AppSettingsData` in `settings/mod.rs` and exposed through `AppSettingsIpcData` in the settings command layer:

- `protonup_auto_suggest` (`bool`, default `true`) — controls whether advisory recommendation banners appear in profile and compatibility views.
- `protonup_binary_path` (`String`, default empty string) — reserved for a future provider binary override path. This field is defined and persisted but is not actively consumed in v1; it is included for forward compatibility without requiring a future migration.

Both fields deserialize with defaults when absent from `settings.toml`, preserving backward compatibility with existing installations.

### Dependencies added

`futures-util = { version = "0.3", default-features = false, features = ["alloc"] }` was added to `crates/crosshook-core/Cargo.toml`, and `reqwest` enables the `stream` feature, to support streaming download chunks during archive installation.
