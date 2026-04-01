# UI Enhancements — Technical Architecture Research

## Executive Summary

The Profiles page currently funnels all editable form fields through a single collapsed "Advanced" section (`CollapsibleSection` with `defaultOpen={false}`), hiding the entire editing surface by default. The monolithic `ProfileFormSections.tsx` (1144 lines) renders all profile fields inline with complex conditional logic per launch method. The proposed solution decomposes `ProfileFormSections` into focused section components and replaces the single collapsed section with a hybrid sub-tab navigation system using existing CSS primitives (`crosshook-subtab-row`, `crosshook-subtab`) already defined but unused in theme.css. Form state is safely preserved across sub-tabs because `ProfileContext` (app-root level) owns the single `GameProfile` state object.

**Second pass additions (issue #52)**: Game metadata and cover art integration via Steam Store API + SteamGridDB. New `game_image_cache` SQLite table tracks filesystem-cached images. Steam metadata JSON cached in existing `external_cache_entries` (key: `steam:appdetails:v1:{app_id}`, within 512 KiB cap). New Rust modules for image fetching/caching follow the established ProtonDB client pattern. New frontend components (`GameCoverArt`, `GameMetadataBar`) integrate into the proposed ProfilesPage hierarchy alongside the Figma-concept-informed sidebar + main content layout.

---

## Current Architecture Analysis

### Component Tree

```
App
  ProfileProvider          <- owns profile CRUD, selection, dirty flag
    ProfileHealthProvider
      PreferencesProvider
        LaunchStateProvider
          Tabs.Root (Radix)
            Sidebar          <- route = 'profiles' | 'launch' | ... (7 routes)
            ContentArea
              ProfilesPage   <- 973 lines
                PageBanner
                CollapsibleSection("Advanced", defaultOpen=false)   <- EVERYTHING hidden
                  ProfileFormSections   <- 1144 lines, ALL form fields
                    ProfileIdentity (name, selector)
                    GameSection (name, path)
                    RunnerMethodSection (launch method select)
                    CustomEnvironmentVariablesSection
                    TrainerSection (conditional: launchMethod !== 'native')
                    RuntimeSection (conditional per launchMethod)
                      AutoPopulate
                      ProtonDbLookupCard
                  HealthIssues (conditional)
                ProfileActions (Save, Delete, Duplicate, Rename, Preview, Export, History)
                LauncherExport (CollapsibleSection, conditional)
              -- modals: delete confirm, rename, preview, history panel, onboarding wizard
```

### State Flow

```
ProfileContext (app root)
  +-- profile: GameProfile          <- single state object
  +-- updateProfile(updater)        <- immutable updater pattern
  +-- dirty: boolean                <- tracks unsaved changes
  +-- saving/loading/deleting       <- operation flags
  +-- selectProfile(name)           <- loads from disk via IPC
  +-- saveProfile()                 <- writes to disk via IPC

ProfilesPage reads from useProfileContext()
  +-- passes props to ProfileFormSections:
        profileName, profile, launchMethod, protonInstalls,
        onProfileNameChange, onUpdateProfile
```

Key: `onUpdateProfile` accepts `(current: GameProfile) => GameProfile`. Every field change produces a new immutable profile object. State lives in context, not in component local state, so sub-tab switching cannot lose data.

### CSS Patterns

- **Panels**: `crosshook-panel` — rounded dark container with border + shadow + blur
- **Cards**: `crosshook-card` — same as panel but with more padding
- **Section titles**: `crosshook-install-section-title` — uppercase eyebrow headings within forms
- **Sub-tabs (unused)**: `crosshook-subtab-row` + `crosshook-subtab` + `crosshook-subtab--active` — pill-shaped buttons in rounded container, defined in `theme.css:104-135` and `variables.css:45-46`
- **Collapsible**: `crosshook-collapsible` — `<details>` element with chevron, title, meta area
- **Controller mode**: `:root[data-crosshook-controller-mode='true']` overrides touch targets, padding, and grid columns

### Critical Pain Points in Current Layout

1. **All form fields hidden by default** — The "Advanced" section's `defaultOpen={false}` means profile name, game path, trainer config, runtime config, and environment variables are ALL invisible until the user clicks to expand.
2. **Actions bar buried** — Save, Delete, Duplicate, Rename buttons live inside the collapsed section, requiring users to expand Advanced before they can perform any profile operation.
3. **Monolithic form component** — `ProfileFormSections.tsx` at 1144 lines handles three launch methods with deeply nested conditional rendering, making it difficult to maintain.
4. **Section boundaries unclear** — Within the expanded Advanced section, sections are separated only by `crosshook-install-section-title` eyebrow headings with no visual container boundaries.
5. **No game visual identity** — Profiles show only text (game name, path) with no cover art, genres, or visual metadata to help users identify games at a glance.

---

## Architecture Design

### Proposed Component Hierarchy (Updated with #52)

```
ProfilesPage
  PageBanner
  ProfileSelectorBar (always visible)
    +-- ThemedSelect (profile dropdown)
    +-- HealthBadge, OfflineStatusBadge, VersionStatusBadge
    +-- Refresh button
  ProfileSubTabRow (always visible)
    +-- SubTab "General" (default)
    +-- SubTab "Runtime"
    +-- SubTab "Environment"
    +-- SubTab "Health" (conditional: only when issues exist)
  SubTabContent (renders active tab)
    +-- General:
    |     GameCoverArt                     <- NEW (#52): Portrait card with cover art + gradient overlay
    |     GameMetadataBar                  <- NEW (#52): Genres, developer, description from Steam API
    |     ProfileIdentitySection + GameSection + RunnerMethodSection
    +-- Runtime: TrainerSection + RuntimeSection (Steam/Proton/Native)
    +-- Environment: CustomEnvironmentVariablesSection + ProtonDbLookupCard
    +-- Health: HealthSummary + HealthIssuesList
  ProfileActionsBar (always visible, outside sub-tabs)
    +-- Save, Duplicate, Rename, Preview, Export, History, Delete
    +-- Dirty indicator
  LauncherExportPanel (CollapsibleSection, conditional)
  -- modals: delete confirm, rename, preview, history panel, onboarding wizard
```

### New/Modified Components

| Component                | Status        | File Path                                                | Responsibility                                               |
| ------------------------ | ------------- | -------------------------------------------------------- | ------------------------------------------------------------ |
| `ProfileSubTabs`         | **New**       | `components/ProfileSubTabs.tsx`                          | Sub-tab row + content routing                                |
| `ProfileIdentitySection` | **New**       | `components/profile-sections/ProfileIdentitySection.tsx` | Profile name field                                           |
| `GameSection`            | **New**       | `components/profile-sections/GameSection.tsx`            | Game name + executable path                                  |
| `RunnerMethodSection`    | **New**       | `components/profile-sections/RunnerMethodSection.tsx`    | Launch method selector                                       |
| `TrainerSection`         | **New**       | `components/profile-sections/TrainerSection.tsx`         | Trainer path, type, loading mode, version                    |
| `RuntimeSection`         | **New**       | `components/profile-sections/RuntimeSection.tsx`         | Steam/Proton/Native runtime fields                           |
| `GameCoverArt`           | **New (#52)** | `components/GameCoverArt.tsx`                            | Cover art display with loading/error states                  |
| `GameMetadataBar`        | **New (#52)** | `components/GameMetadataBar.tsx`                         | Genres, developer, description strip                         |
| `GameCard`               | **New (#52)** | `components/GameCard.tsx`                                | Cover art card with action overlays (launch, favorite, edit) |
| `ProfileGameCardGrid`    | **New (#52)** | `components/ProfileGameCardGrid.tsx`                     | Grid/list view of all profile game cards                     |
| `FieldRow`               | **Extract**   | `components/ui/FieldRow.tsx`                             | Generic labeled input + browse button                        |
| `ProfileFormSections`    | **Modify**    | Keep as thin re-export or remove                         | Backward-compat for OnboardingWizard                         |
| `ProfilesPage`           | **Modify**    | Existing file                                            | Add sub-tab state, restructure layout                        |
| `ProfileActions`         | **No change** | Existing file                                            | Moved outside sub-tab content area                           |

---

## Data Flow Design

### Form State Across Sub-Tabs

```
ProfileContext (persists across ALL tabs)
  |
  +-- General Tab
  |     GameCoverArt           <- reads: profile.steam.app_id (triggers image fetch)
  |     GameMetadataBar        <- reads: profile.steam.app_id (triggers metadata fetch)
  |     ProfileIdentitySection <- reads: profileName, profileExists
  |     GameSection            <- reads: profile.game
  |     RunnerMethodSection    <- reads: profile.launch.method
  |     All call: onUpdateProfile((current) => ({ ...current, ... }))
  |
  +-- Runtime Tab
  |     TrainerSection         <- reads: profile.trainer, launchMethod
  |     RuntimeSection         <- reads: profile.steam | profile.runtime
  |     All call: onUpdateProfile((current) => ({ ...current, ... }))
  |
  +-- Environment Tab
  |     CustomEnvVarsSection   <- reads: profile.launch.custom_env_vars
  |     ProtonDbLookupCard     <- reads: profile.steam.app_id
  |     ProtonDB merge logic   <- calls: onUpdateProfile to merge env vars
  |
  +-- Health Tab
        HealthSummary          <- reads from useProfileHealthContext()
        HealthIssuesList       <- reads from useProfileHealthContext()
```

### Game Metadata & Cover Art Data Flow (New — #52)

```
Profile steam.app_id (from ProfileContext)
  |
  +-- useGameMetadata(appId)
  |     +-- invoke('fetch_game_metadata', { appId })
  |     +-- Returns: SteamAppDetails (name, genres, description, developers, header_image URL)
  |     +-- State: idle -> loading -> ready | stale | unavailable
  |     +-- Cache: external_cache_entries (key: steam:appdetails:v1:{app_id})
  |
  +-- useGameCoverArt(appId)
        +-- invoke('fetch_game_cover_art', { appId })
        +-- Returns: GameCoverArtResult (local file path, source, dimensions)
        +-- State: idle -> loading -> ready | unavailable
        +-- Cache: filesystem (~/.local/share/crosshook/cache/images/{app_id}/)
        +-- Tracking: game_image_cache SQLite table
```

### Validation Across Sections

- **Cross-section validation**: The `canSave` check (`profileName.trim().length > 0 && profile.game.executable_path.trim().length > 0`) spans General tab fields. This check stays in ProfilesPage.
- **Per-field validation**: Custom env var key validation stays in `CustomEnvironmentVariablesSection`. Reserved key checks, duplicate detection unchanged.
- **Health validation**: Profile health is computed by the Rust backend and accessed via `useProfileHealthContext()`. No frontend cross-field validation needed.

### ProtonDB State Management

Currently `pendingProtonDbOverwrite`, `applyingProtonDbGroupId`, and `protonDbStatusMessage` are local state in `ProfileFormSections`. Two options:

**Option A (Recommended)**: Move these states to `RuntimeSection` or the Environment tab component. They are only relevant when the ProtonDB card is visible.

**Option B**: Lift to ProfilesPage. Overkill since these states are only consumed by ProtonDB-related UI.

---

## Data Models

### Game Image Cache — Table Schema (New — #52)

### Migration: v13 to v14

```sql
CREATE TABLE IF NOT EXISTS game_image_cache (
    cache_id            TEXT PRIMARY KEY,
    steam_app_id        TEXT NOT NULL,
    image_type          TEXT NOT NULL DEFAULT 'cover',
    source              TEXT NOT NULL DEFAULT 'steam_cdn',
    file_path           TEXT NOT NULL,
    file_size           INTEGER NOT NULL DEFAULT 0,
    content_hash        TEXT NOT NULL DEFAULT '',
    mime_type           TEXT NOT NULL DEFAULT 'image/jpeg',
    width               INTEGER,
    height              INTEGER,
    source_url          TEXT NOT NULL DEFAULT '',
    preferred_source    TEXT NOT NULL DEFAULT 'auto',
    expires_at          TEXT,
    fetched_at          TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_game_image_cache_app_type_source
    ON game_image_cache(steam_app_id, image_type, source);
CREATE INDEX IF NOT EXISTS idx_game_image_cache_expires
    ON game_image_cache(expires_at);
```

### Column Definitions

| Column             | Type    | Constraints          | Description                                                 |
| ------------------ | ------- | -------------------- | ----------------------------------------------------------- |
| `cache_id`         | TEXT    | PK                   | UUID v4 (via `db::new_id()`)                                |
| `steam_app_id`     | TEXT    | NOT NULL             | Steam app ID (e.g., "1245620"). Validated numeric-only.     |
| `image_type`       | TEXT    | NOT NULL, DEFAULT    | One of: `cover`, `header`, `hero`, `icon`, `logo`           |
| `source`           | TEXT    | NOT NULL, DEFAULT    | One of: `steam_cdn`, `steamgriddb`                          |
| `file_path`        | TEXT    | NOT NULL             | Absolute path to cached image on filesystem                 |
| `file_size`        | INTEGER | NOT NULL, DEFAULT 0  | File size in bytes                                          |
| `content_hash`     | TEXT    | NOT NULL, DEFAULT '' | SHA-256 of image file contents                              |
| `mime_type`        | TEXT    | NOT NULL, DEFAULT    | `image/jpeg`, `image/png`, or `image/webp`                  |
| `width`            | INTEGER | nullable             | Image width in pixels (populated after download)            |
| `height`           | INTEGER | nullable             | Image height in pixels (populated after download)           |
| `source_url`       | TEXT    | NOT NULL, DEFAULT '' | URL the image was fetched from                              |
| `preferred_source` | TEXT    | NOT NULL, DEFAULT    | `auto`, `steam_cdn`, `steamgriddb` — user override per game |
| `expires_at`       | TEXT    | nullable             | RFC 3339 timestamp; NULL = never expires                    |
| `fetched_at`       | TEXT    | NOT NULL             | When the image was last downloaded                          |
| `created_at`       | TEXT    | NOT NULL             | Row creation time                                           |
| `updated_at`       | TEXT    | NOT NULL             | Last modification time                                      |

### Unique Index

`(steam_app_id, image_type, source)` — At most one `cover` image from `steam_cdn` per app. Upsert on conflict.

### Relationship to `external_cache_entries`

- **Steam metadata JSON** goes into `external_cache_entries` using cache key `steam:appdetails:v1:{app_id}` with the existing `put_cache_entry` / `get_cache_entry` API. This follows the identical pattern used by the ProtonDB client (`protondb:{app_id}`).
- **Cover art images** go to filesystem + `game_image_cache` table. Images exceed the 512 KiB `MAX_CACHE_PAYLOAD_BYTES` limit on `external_cache_entries`.
- Both share `steam_app_id` as the join dimension. No FK between tables — they are independent caches with independent TTLs.

### Filesystem Layout

```
~/.local/share/crosshook/
  metadata.db                           <- existing SQLite database
  cache/
    images/
      {steam_app_id}/                   <- directory per game
        cover_steam_cdn.jpg             <- primary cover art from Steam CDN
        cover_steamgriddb.jpg           <- higher-quality art from SteamGridDB (optional)
        header_steam_cdn.jpg            <- header banner (460x215)
```

Path construction: `BaseDirs::data_local_dir().join("crosshook/cache/images/{app_id}/{type}_{source}.{ext}")`. The `app_id` component is validated as numeric-only before path construction to prevent traversal.

---

## Rust Module Design (New — #52)

### Module: `steam_metadata` (new in crosshook-core)

Follows the exact patterns established by the `protondb/` module.

```
crosshook-core/src/
  steam_metadata/
    mod.rs          <- public re-exports
    client.rs       <- Steam Store API client + cache-first logic
    models.rs       <- Data structures for API responses and lookup results
```

#### `steam_metadata/models.rs`

```rust
use serde::{Deserialize, Serialize};

pub const STEAM_METADATA_CACHE_NAMESPACE: &str = "steam:appdetails:v1";
pub const STEAM_METADATA_CACHE_TTL_HOURS: i64 = 24;

pub fn metadata_cache_key(app_id: &str) -> String {
    format!("{STEAM_METADATA_CACHE_NAMESPACE}:{}", app_id.trim())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SteamMetadataLookupState {
    #[default]
    Idle,
    Loading,
    Ready,
    Stale,
    Unavailable,
}

/// Subset of Steam Store API appdetails response, normalized for CrossHook use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SteamAppDetails {
    #[serde(default)]
    pub steam_appid: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub short_description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub developers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<SteamGenre>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub header_image: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub capsule_image: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<SteamReleaseDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metacritic: Option<SteamMetacritic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<SteamCategory>,
    #[serde(default)]
    pub is_free: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamGenre {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamReleaseDate {
    #[serde(default)]
    pub coming_soon: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamMetacritic {
    #[serde(default)]
    pub score: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SteamCategory {
    #[serde(default)]
    pub id: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Top-level lookup result returned by IPC to the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SteamMetadataLookupResult {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    #[serde(default)]
    pub state: SteamMetadataLookupState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<SteamAppDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    #[serde(default)]
    pub from_cache: bool,
    #[serde(default)]
    pub is_stale: bool,
}
```

#### `steam_metadata/client.rs` — Pattern Mirror of `protondb/client.rs`

```rust
// Key function signature — follows protondb::lookup_protondb exactly
pub async fn fetch_steam_metadata(
    metadata_store: &MetadataStore,
    app_id: &str,
    force_refresh: bool,
) -> SteamMetadataLookupResult {
    // 1. normalize_app_id (reuse from protondb::models)
    // 2. Check cache: get_cache_entry(metadata_cache_key(app_id))
    // 3. If cache hit and not force_refresh: return cached result
    // 4. Fetch from Steam Store API
    // 5. Persist to external_cache_entries via put_cache_entry
    // 6. On network failure: return stale cache if available, else Unavailable
}
```

Steam Store API endpoint: `https://store.steampowered.com/api/appdetails?appids={app_id}&l=english`

Response wrapper to handle (outer wrapper differs from inner data):

```rust
// Steam API returns: { "{app_id}": { "success": bool, "data": { ... } } }
#[derive(Debug, Deserialize)]
struct SteamApiAppDetailsWrapper {
    success: bool,
    data: Option<SteamAppDetails>,
}
```

### Module: `game_images` (new in crosshook-core)

```
crosshook-core/src/
  game_images/
    mod.rs          <- public re-exports
    client.rs       <- Image download from Steam CDN + SteamGridDB
    cache.rs        <- Filesystem cache operations (write, read, evict)
    models.rs       <- ImageSource, GameImageCacheEntry, etc.
```

#### `game_images/models.rs`

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const IMAGE_CACHE_TTL_DAYS: i64 = 7;
pub const MAX_IMAGE_FILE_SIZE: usize = 2 * 1024 * 1024; // 2 MiB

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSource {
    SteamCdn,
    SteamGridDb,
}

impl Default for ImageSource {
    fn default() -> Self {
        Self::SteamCdn
    }
}

impl ImageSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SteamCdn => "steam_cdn",
            Self::SteamGridDb => "steamgriddb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageType {
    Cover,
    Header,
    Hero,
    Icon,
    Logo,
}

impl Default for ImageType {
    fn default() -> Self {
        Self::Cover
    }
}

impl ImageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Header => "header",
            Self::Hero => "hero",
            Self::Icon => "icon",
            Self::Logo => "logo",
        }
    }
}

/// Preferred image source strategy for a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreferredImageSource {
    #[default]
    Auto,
    SteamCdn,
    SteamGridDb,
}

/// Result returned to frontend by IPC command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameCoverArtResult {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<PathBuf>,
    #[serde(default)]
    pub source: ImageSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default)]
    pub from_cache: bool,
    #[serde(default)]
    pub available: bool,
}
```

#### `game_images/client.rs`

```rust
/// Fetch cover art for a Steam game. Cache-first with filesystem storage.
pub async fn fetch_game_cover_art(
    metadata_store: &MetadataStore,
    app_id: &str,
    steamgriddb_api_key: Option<&str>,
    force_refresh: bool,
) -> GameCoverArtResult {
    // 1. Validate app_id (numeric only — prevent path traversal)
    // 2. Check game_image_cache table for existing entry
    // 3. If cached and not expired and not force_refresh: return file_path
    // 4. Determine source:
    //    a. If steamgriddb_api_key is Some and preferred_source != steam_cdn:
    //       try SteamGridDB first (600x900 portrait grid)
    //    b. Fallback to Steam CDN: library_600x900 URL
    //       https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg
    // 5. Download image, validate (magic bytes, max size), write to filesystem
    // 6. Upsert game_image_cache row with file metadata
    // 7. On failure: return stale cache if available, else unavailable
}

/// Steam CDN cover art URL for library grid (600x900, portrait)
fn steam_cdn_cover_url(app_id: &str) -> String {
    format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
    )
}

/// Steam CDN header image URL (460x215, landscape)
fn steam_cdn_header_url(app_id: &str) -> String {
    format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"
    )
}
```

#### `game_images/cache.rs`

```rust
use std::path::{Path, PathBuf};
use directories::BaseDirs;

/// Returns the base cache directory: ~/.local/share/crosshook/cache/images/
pub fn image_cache_base_dir() -> Result<PathBuf, String> {
    BaseDirs::new()
        .ok_or("home directory not found")?
        .data_local_dir()
        .join("crosshook/cache/images");
    // Verify: not a symlink, create with 0o700 permissions
}

/// Returns the path for a specific cached image.
/// app_id MUST be validated as numeric-only before calling.
pub fn image_cache_path(app_id: &str, image_type: &str, source: &str, ext: &str) -> PathBuf {
    image_cache_base_dir()
        .unwrap_or_default()
        .join(app_id)
        .join(format!("{image_type}_{source}.{ext}"))
}

/// Validates downloaded image bytes: checks magic bytes, enforces max size.
pub fn validate_image_bytes(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.len() > MAX_IMAGE_FILE_SIZE {
        return Err(format!("image exceeds {} byte limit", MAX_IMAGE_FILE_SIZE));
    }
    // Check magic bytes: JPEG (FF D8 FF), PNG (89 50 4E 47), WebP (52 49 46 46 ... 57 45 42 50)
    match bytes.get(..4) {
        Some([0xFF, 0xD8, 0xFF, _]) => Ok("image/jpeg"),
        Some([0x89, 0x50, 0x4E, 0x47]) => Ok("image/png"),
        Some([0x52, 0x49, 0x46, 0x46]) if bytes.get(8..12) == Some(b"WEBP") => Ok("image/webp"),
        _ => Err("unrecognized image format".to_string()),
    }
}
```

### Module: `metadata/game_image_store.rs` (new submodule in metadata/)

SQLite CRUD for the `game_image_cache` table. Follows the exact pattern of `metadata/cache_store.rs`.

```rust
use super::{db, MetadataStoreError};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

pub struct GameImageCacheRow {
    pub cache_id: String,
    pub steam_app_id: String,
    pub image_type: String,
    pub source: String,
    pub file_path: String,
    pub file_size: i64,
    pub content_hash: String,
    pub mime_type: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub source_url: String,
    pub preferred_source: String,
    pub expires_at: Option<String>,
    pub fetched_at: String,
}

pub fn get_cached_image(
    conn: &Connection,
    steam_app_id: &str,
    image_type: &str,
    source: &str,
) -> Result<Option<GameImageCacheRow>, MetadataStoreError> {
    let now = Utc::now().to_rfc3339();
    conn.query_row(
        "SELECT cache_id, steam_app_id, image_type, source, file_path,
                file_size, content_hash, mime_type, width, height,
                source_url, preferred_source, expires_at, fetched_at
         FROM game_image_cache
         WHERE steam_app_id = ?1 AND image_type = ?2 AND source = ?3
           AND (expires_at IS NULL OR expires_at > ?4)",
        params![steam_app_id, image_type, source, now],
        |row| { /* map to GameImageCacheRow */ },
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "query game image cache",
        source,
    })
}

pub fn upsert_cached_image(
    conn: &Connection,
    row: &GameImageCacheRow,
) -> Result<(), MetadataStoreError> {
    let now = Utc::now().to_rfc3339();
    let cache_id = db::new_id();
    conn.execute(
        "INSERT INTO game_image_cache (
            cache_id, steam_app_id, image_type, source, file_path,
            file_size, content_hash, mime_type, width, height,
            source_url, preferred_source, expires_at, fetched_at,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        ON CONFLICT(steam_app_id, image_type, source) DO UPDATE SET
            file_path = excluded.file_path,
            file_size = excluded.file_size,
            content_hash = excluded.content_hash,
            mime_type = excluded.mime_type,
            width = excluded.width,
            height = excluded.height,
            source_url = excluded.source_url,
            preferred_source = excluded.preferred_source,
            expires_at = excluded.expires_at,
            fetched_at = excluded.fetched_at,
            updated_at = excluded.updated_at",
        params![
            cache_id, row.steam_app_id, row.image_type, row.source,
            row.file_path, row.file_size, row.content_hash, row.mime_type,
            row.width, row.height, row.source_url, row.preferred_source,
            row.expires_at, row.fetched_at, now, now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert game image cache entry",
        source,
    })?;
    Ok(())
}

pub fn evict_expired_images(conn: &Connection) -> Result<Vec<String>, MetadataStoreError> {
    // Returns file_path list of expired entries (caller deletes filesystem files)
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT file_path FROM game_image_cache WHERE expires_at IS NOT NULL AND expires_at < ?1"
    ).map_err(|source| MetadataStoreError::Database {
        action: "query expired game image cache entries",
        source,
    })?;
    let paths: Vec<String> = stmt.query_map(params![now], |row| row.get(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "read expired image paths",
            source,
        })?
        .filter_map(|r| r.ok())
        .collect();
    conn.execute(
        "DELETE FROM game_image_cache WHERE expires_at IS NOT NULL AND expires_at < ?1",
        params![now],
    ).map_err(|source| MetadataStoreError::Database {
        action: "evict expired game image cache entries",
        source,
    })?;
    Ok(paths)
}
```

### MetadataStore Integration

Add to `metadata/mod.rs`:

```rust
mod game_image_store;
pub use game_image_store::GameImageCacheRow;

impl MetadataStore {
    pub fn get_cached_image(
        &self, steam_app_id: &str, image_type: &str, source: &str,
    ) -> Result<Option<GameImageCacheRow>, MetadataStoreError> {
        self.with_conn("get a cached game image", |conn| {
            game_image_store::get_cached_image(conn, steam_app_id, image_type, source)
        })
    }

    pub fn upsert_cached_image(
        &self, row: &GameImageCacheRow,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("upsert a cached game image", |conn| {
            game_image_store::upsert_cached_image(conn, row)
        })
    }

    pub fn evict_expired_images(&self) -> Result<Vec<String>, MetadataStoreError> {
        self.with_conn("evict expired game images", |conn| {
            game_image_store::evict_expired_images(conn)
        })
    }
}
```

### Settings Update

Add SteamGridDB API key to `AppSettingsData` in `settings/mod.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub steamgriddb_api_key: String,  // NEW: optional, empty = disabled
}
```

Backward compatible: `#[serde(default)]` means existing `settings.toml` without this field deserializes fine.

---

## IPC Commands (New — #52)

### `commands/game_metadata.rs`

```rust
use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;
use crosshook_core::steam_metadata::{fetch_steam_metadata, SteamMetadataLookupResult};
use crosshook_core::game_images::{fetch_game_cover_art, GameCoverArtResult};
use tauri::State;

#[tauri::command]
pub async fn fetch_game_metadata(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamMetadataLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(fetch_steam_metadata(
        &metadata_store,
        &app_id,
        force_refresh.unwrap_or(false),
    ).await)
}

#[tauri::command]
pub async fn fetch_game_cover_art(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<GameCoverArtResult, String> {
    let metadata_store = metadata_store.inner().clone();
    let api_key = settings_store.load()
        .map(|s| s.steamgriddb_api_key)
        .unwrap_or_default();
    let api_key_ref = if api_key.is_empty() { None } else { Some(api_key.as_str()) };
    Ok(fetch_game_cover_art(
        &metadata_store,
        &app_id,
        api_key_ref,
        force_refresh.unwrap_or(false),
    ).await)
}

#[tauri::command]
pub async fn get_cached_image_path(
    app_id: String,
    image_type: Option<String>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Option<String>, String> {
    let image_type = image_type.unwrap_or_else(|| "cover".to_string());
    let metadata_store = metadata_store.inner().clone();
    // Check game_image_cache for any source, prefer steam_cdn, fall back to steamgriddb
    for source in &["steam_cdn", "steamgriddb"] {
        if let Ok(Some(row)) = metadata_store.get_cached_image(&app_id, &image_type, source) {
            if std::path::Path::new(&row.file_path).exists() {
                return Ok(Some(row.file_path));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub async fn clear_image_cache(
    app_id: Option<String>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<usize, String> {
    // If app_id provided: clear only that game's images
    // If None: clear all expired images
    // Returns count of files deleted
}
```

### Registration in `lib.rs`

Add to `invoke_handler`:

```rust
commands::game_metadata::fetch_game_metadata,
commands::game_metadata::fetch_game_cover_art,
commands::game_metadata::get_cached_image_path,
commands::game_metadata::clear_image_cache,
```

---

## Frontend Components (New — #52)

### TypeScript Types: `types/game-metadata.ts`

```typescript
export type SteamMetadataLookupState = 'idle' | 'loading' | 'ready' | 'stale' | 'unavailable';

export interface SteamGenre {
  id: string;
  description: string;
}

export interface SteamReleaseDate {
  coming_soon: boolean;
  date: string;
}

export interface SteamMetacritic {
  score: number;
  url: string;
}

export interface SteamCategory {
  id: number;
  description: string;
}

export interface SteamAppDetails {
  steam_appid: number;
  name: string;
  short_description: string;
  developers: string[];
  publishers: string[];
  genres: SteamGenre[];
  header_image: string;
  capsule_image: string;
  release_date: SteamReleaseDate | null;
  metacritic: SteamMetacritic | null;
  categories: SteamCategory[];
  is_free: boolean;
}

export interface SteamMetadataLookupResult {
  app_id: string;
  state: SteamMetadataLookupState;
  details: SteamAppDetails | null;
  cache_key: string | null;
  from_cache: boolean;
  is_stale: boolean;
}

export interface GameCoverArtResult {
  app_id: string;
  file_path: string | null;
  source: 'steam_cdn' | 'steamgriddb';
  width: number | null;
  height: number | null;
  from_cache: boolean;
  available: boolean;
}
```

### Hook: `hooks/useGameMetadata.ts`

Mirrors `useProtonDbLookup.ts` pattern exactly:

```typescript
import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { SteamMetadataLookupResult, SteamMetadataLookupState, SteamAppDetails } from '../types/game-metadata';

export interface UseGameMetadataResult {
  appId: string;
  state: SteamMetadataLookupState;
  loading: boolean;
  details: SteamAppDetails | null;
  fromCache: boolean;
  isStale: boolean;
  isUnavailable: boolean;
  refresh: () => Promise<void>;
}

export function useGameMetadata(appId: string): UseGameMetadataResult {
  // Same pattern as useProtonDbLookup:
  // - normalizeAppId, requestIdRef for race condition protection
  // - invoke('fetch_game_metadata', { appId, forceRefresh })
  // - idle/loading/ready/stale/unavailable state machine
}
```

### Hook: `hooks/useGameCoverArt.ts`

````typescript
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { GameCoverArtResult } from '../types/game-metadata';

export interface UseGameCoverArtResult {
  appId: string;
  loading: boolean;
  available: boolean;
  /** Tauri asset URL suitable for <img src> */
  imageSrc: string | null;
  source: 'steam_cdn' | 'steamgriddb' | null;
  dimensions: { width: number; height: number } | null;
  refresh: () => Promise<void>;
}

export function useGameCoverArt(appId: string): UseGameCoverArtResult {
  // 1. invoke('fetch_game_cover_art', { appId })
  // 2. Convert result.file_path to Tauri asset URL via convertFileSrc()
  // 3. Track loading/available state
  // Key: convertFileSrc converts absolute file path to asset:// protocol URL
  //      that Tauri's webview can load securely
}
### Component: `GameCoverArt.tsx`

```tsx
interface GameCoverArtProps {
  appId: string;
  gameName: string;
  /** 'portrait' = 3:4 (card grid), 'landscape' = header aspect (editor) */
  aspect?: 'portrait' | 'landscape';
  className?: string;
}

// States:
// 1. No appId: show placeholder with game initial letter
// 2. Loading: animated skeleton pulse (crosshook-color-bg-elevated)
// 3. Loaded: <img> with fade-in transition, gradient overlay at bottom
// 4. Error: fallback to genre-color gradient with game name text

// CSS classes (BEM, follows existing crosshook-* pattern):
// .crosshook-game-cover-art             — container with overflow:hidden, border-radius
// .crosshook-game-cover-art--portrait   — aspect-ratio: 3/4
// .crosshook-game-cover-art--landscape  — aspect-ratio: 460/215
// .crosshook-game-cover-art__image      — object-fit:cover, full bleed
// .crosshook-game-cover-art__overlay    — bottom gradient for text readability
// .crosshook-game-cover-art__placeholder — centered initial letter or game name
// .crosshook-game-cover-art__skeleton   — animated pulse background
````

### Component: `GameCard.tsx` (New — Figma concept)

```tsx
interface GameCardProps {
  profileName: string;
  appId: string;
  gameName: string;
  launchMethod: string;
  isFavorite: boolean;
  isSelected: boolean;
  healthStatus?: 'healthy' | 'warning' | 'broken';
  onSelect: () => void;
  onLaunch: () => void;
  onToggleFavorite: (favorite: boolean) => void;
}

// Composition:
//   GameCoverArt (portrait, background)
//   GameCardOverlay (gradient + game name + badges)
//   GameCardActions (hover-reveal: favorite, edit, launch buttons)
//
// The card is the primary interactive unit in the Figma concept.
// Favorite/launch/edit actions are directly on the card, not in a separate panel.
//
// CSS classes:
// .crosshook-game-card                    — container, relative positioning
// .crosshook-game-card--selected          — accent border (crosshook-color-accent)
// .crosshook-game-card--favorite          — subtle star indicator
// .crosshook-game-card__actions           — absolute positioned, opacity:0 by default
// .crosshook-game-card:hover .crosshook-game-card__actions — opacity:1 on hover
// .crosshook-game-card__action-btn        — icon button with min-size from --crosshook-touch-target-compact
// Controller mode: actions always visible (no hover), larger touch targets
```

### Component: `GameMetadataBar.tsx`

```tsx
interface GameMetadataBarProps {
  appId: string;
  /** Compact mode shows only genres as pills; full mode adds description */
  compact?: boolean;
  className?: string;
}

// Renders: genre pills, developer name, short description (truncated)
// Uses useGameMetadata(appId) hook
// Compact mode: just genre pills inline (for card headers)
// Full mode: genres + developer + one-line description (for General tab)

// CSS: reuses crosshook-panel or crosshook-card background
// Genre pills: small rounded spans with crosshook-color-accent-soft background
```

---

## Navigation Design

### Sub-Tab Routing Approach: Local State (Recommended)

```tsx
// In ProfilesPage
type ProfileSubTab = 'general' | 'runtime' | 'environment' | 'health';
const [activeSubTab, setActiveSubTab] = useState<ProfileSubTab>('general');
```

**Rationale**:

- The app uses Radix Tabs for top-level routing (sidebar). No URL router exists.
- Sub-tabs are purely visual navigation within a single page context.
- Profile form state persists in ProfileContext regardless of which tab is rendered.
- Tab resets to 'general' when navigating away and back — acceptable behavior, matches user expectation.

**Rejected alternatives**:

- URL hash routing: No URL router in the app; adding one is a disproportionate change.
- Nested Radix Tabs: Possible but constrains layout flexibility. Plain buttons with conditional rendering are simpler and equally accessible with proper ARIA.

### Accessibility

```tsx
<div className="crosshook-subtab-row" role="tablist" aria-label="Profile sections">
  <button
    role="tab"
    aria-selected={activeSubTab === 'general'}
    aria-controls="profile-tab-general"
    className={`crosshook-subtab ${activeSubTab === 'general' ? 'crosshook-subtab--active' : ''}`}
    onClick={() => setActiveSubTab('general')}
  >
    General
  </button>
  {/* ... more tabs */}
</div>

<div id="profile-tab-general" role="tabpanel" aria-labelledby="...">
  {activeSubTab === 'general' && <GeneralTabContent />}
</div>
```

### Integration with Sidebar Navigation

No changes to Sidebar.tsx or ContentArea.tsx. The sub-tabs are entirely within ProfilesPage. The sidebar route remains `'profiles'` — sub-tab state is local to the page component.

### Controller Mode / Gamepad

---

## Figma-Concept: Cover Art Card Grid

The Figma concept is specifically about the **library grid system with game cover art cards** — where favorite, edit, and launch actions are all accessible directly from each cover art card. This does not change the existing CrossHook theme (dark glassmorphism panels, BEM `crosshook-*` classes, CSS variables, controller mode). The grid is a new browsing surface that sits alongside or replaces the current `ThemedSelect` profile dropdown.

### Cover Art Card Grid Pattern

The primary Figma concept is a responsive grid of portrait game cards with cover art as the dominant visual. Each card provides direct access to core actions without opening a separate editor.

```
ProfilesPage
  PageBanner
  ProfileLibraryBar (always visible)
    +-- Grid/List view toggle
    +-- Sort controls (name, last launched, favorite)
    +-- Search/filter input
  ProfileGameCardGrid (grid or list view)
    +-- GameCard (per profile)
    |     GameCoverArt(portrait, 3:4)      <- cover art background
    |     GameCardOverlay                   <- gradient overlay for text readability
    |       GameCardTitle                   <- game name
    |       GameCardBadges                  <- launch method badge, health indicator
    |     GameCardActions (hover/focus)     <- action buttons revealed on interaction
    |       FavoriteToggle                  <- star/heart icon (uses profile_set_favorite IPC)
    |       EditButton                      <- opens profile editor (selects profile + switches to edit view)
    |       LaunchButton                    <- direct launch (invokes launch_game IPC)
  -- OR when a profile is selected for editing:
  ProfileEditorPanel
    ProfileSubTabRow + SubTabContent + ProfileActionsBar
```

### Card Action Overlay

Each game card exposes three actions directly, visible on hover (desktop) or always visible (controller mode):

| Action       | Icon/Control     | IPC Command            | Behavior                                    |
| ------------ | ---------------- | ---------------------- | ------------------------------------------- |
| **Favorite** | Star toggle      | `profile_set_favorite` | Toggles `is_favorite` in profiles table     |
| **Edit**     | Pencil/gear icon | `selectProfile(name)`  | Selects profile, transitions to editor view |
| **Launch**   | Play triangle    | `launch_game`          | Launches game directly from card            |

Action positioning: bottom of card over gradient overlay, or top-right corner as icon-only buttons. Controller mode should make actions accessible via gamepad buttons (A = launch, X = edit, Y = favorite) using existing `data-crosshook-focus-zone` pattern.

### Grid View Layout

```css
/* Responsive portrait card grid */
.crosshook-game-card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: var(--crosshook-grid-gap);
  padding: var(--crosshook-panel-padding);
}
```

- Cards fill available width responsively; minimum 180px ensures readable text at small sizes
- Portrait aspect ratio (3:4) matches Steam library grid and SteamGridDB grid art dimensions
- Existing `--crosshook-grid-gap` variable provides consistent spacing
- Controller mode: `--crosshook-layout-main-columns: 1fr` already collapses to single column — card grid naturally adapts

### List View Layout

```css
/* Compact horizontal list view */
.crosshook-game-card-list {
  display: flex;
  flex-direction: column;
  gap: calc(var(--crosshook-grid-gap) / 2);
}

.crosshook-game-card-list__item {
  display: grid;
  grid-template-columns: 64px 1fr auto;
  gap: var(--crosshook-grid-gap);
  align-items: center;
  min-height: var(--crosshook-touch-target-min);
  padding: 8px var(--crosshook-panel-padding);
}
```

- Thumbnail (64x48, landscape header crop) + game name/info + action buttons
- Uses `--crosshook-touch-target-min` for accessible row height

### Grid/List View Toggle

- **State**: `localStorage` key `crosshook.profilesViewMode` (`'grid' | 'list'`)
- **Toggle control**: Button pair in `ProfileLibraryBar`, matching existing `crosshook-subtab` button styling
- **Default**: Grid view (showcases cover art, primary value proposition of #52)

### Integration with Profile Editor

Two navigation patterns for transitioning from the card grid to the profile editor:

**Option A (Recommended): In-place transition**

- Card grid replaces the current profile selector dropdown as the primary browse surface
- Clicking "Edit" on a card selects the profile and reveals the sub-tab editor below the grid (or replaces the grid with a back-to-grid button)
- Matches how the current `ThemedSelect` dropdown works — selecting a profile shows its editor

**Option B: Sidebar + editor split**

- Card grid in a sidebar column (~300px), editor in main column
- More screen-efficient for frequent editing, but significant layout change
- Deferred: requires careful responsive breakpoint work and controller mode testing

### Relationship to Existing Components

| Existing Component        | Integration                                                         |
| ------------------------- | ------------------------------------------------------------------- |
| `PinnedProfilesStrip`     | Favorited cards get visual treatment (border glow, sort-to-top)     |
| `ThemedSelect` (dropdown) | May be replaced by grid or retained as compact fallback             |
| `HealthBadge`             | Rendered as small icon on card overlay (reuse existing component)   |
| `OfflineStatusBadge`      | Rendered as small icon on card overlay (reuse existing component)   |
| `ProfileActions`          | Launch action promoted to card; edit/delete remain in editor panel  |
| `ProfileContext`          | Card click calls `selectProfile(name)` — same as dropdown selection |

---

## Component Specifications

### ProfileSubTabs

```tsx
interface ProfileSubTabsProps {
  activeTab: ProfileSubTab;
  onTabChange: (tab: ProfileSubTab) => void;
  showHealthTab: boolean; // only show when health issues exist
}
```

Renders the `crosshook-subtab-row` with tab buttons. Uses existing CSS classes.

### ProfileIdentitySection

```tsx
interface ProfileIdentitySectionProps {
  profileName: string;
  profileExists: boolean;
  onProfileNameChange: (value: string) => void;
  // Optional: profile selector (used by OnboardingWizard)
  profileSelector?: ProfileFormSectionsProfileSelector;
}
```

Extracted from `ProfileFormSections.tsx` lines 665-702.

### GameSection

```tsx
interface GameSectionProps {
  profile: GameProfile;
  launchMethod: LaunchMethod;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 704-737. Includes game name + executable path with browse.

### RunnerMethodSection

```tsx
interface RunnerMethodSectionProps {
  launchMethod: LaunchMethod;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 739-766. Launch method dropdown (steam_applaunch / proton_run / native).

### TrainerSection

```tsx
interface TrainerSectionProps {
  profile: GameProfile;
  launchMethod: LaunchMethod;
  profileName: string;
  profileExists: boolean;
  reviewMode: boolean;
  trainerVersion: string | null;
  onVersionSet?: () => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 775-897. Only rendered when `launchMethod !== 'native'`. Includes trainer path, type selector, loading mode, version display, and manual version set.

### RuntimeSection

```tsx
interface RuntimeSectionProps {
  profile: GameProfile;
  launchMethod: LaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  reviewMode: boolean;
  profileExists: boolean;
  profileName: string;
  trainerVersion: string | null;
  versionStatus: VersionCorrelationStatus | null;
  onVersionSet?: () => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 899-1138. Handles all three launch method variants (steam_applaunch, proton_run, native). Includes ProtonPathField, LauncherMetadataFields, AutoPopulate, and ProtonDbLookupCard.

### FieldRow (extracted to ui/)

```tsx
interface FieldRowProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  helperText?: string;
  error?: string | null;
  browseLabel?: string;
  onBrowse?: () => Promise<void>;
}
```

Currently a local function component inside ProfileFormSections (lines 124-163). Used 10+ times across sections. Should be extracted to `components/ui/FieldRow.tsx` for reuse.

---

## CSS / Styling Strategy

### Existing Classes to Use

| Class                             | Source          | Purpose                              |
| --------------------------------- | --------------- | ------------------------------------ |
| `crosshook-subtab-row`            | `theme.css:104` | Container for sub-tab pills          |
| `crosshook-subtab`                | `theme.css:115` | Individual sub-tab button            |
| `crosshook-subtab--active`        | `theme.css:131` | Active sub-tab with accent gradient  |
| `crosshook-panel`                 | `theme.css:137` | Dark panel container                 |
| `crosshook-install-section-title` | `theme.css`     | Section eyebrow headings within form |
| `crosshook-install-grid`          | `theme.css`     | Grid layout for form fields          |

### New CSS Needed

```css
/* Container for sub-tab content to provide consistent padding */
.crosshook-subtab-content {
  padding: var(--crosshook-card-padding);
  /* No border-top since sub-tab-row provides visual separation */
}

/* Sticky actions bar at bottom of profile panel */
.crosshook-profile-actions-bar {
  padding: var(--crosshook-card-padding);
  border-top: 1px solid var(--crosshook-color-border);
  position: sticky;
  bottom: 0;
  background: inherit; /* match panel background for scroll cover */
}

/* --- NEW: Game cover art and metadata (#52) --- */

.crosshook-game-cover-art {
  position: relative;
  overflow: hidden;
  border-radius: var(--crosshook-radius-md);
  background: var(--crosshook-color-surface);
}

.crosshook-game-cover-art--portrait {
  aspect-ratio: 3 / 4;
}

.crosshook-game-cover-art--landscape {
  aspect-ratio: 460 / 215;
}

.crosshook-game-cover-art__image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  opacity: 0;
  transition: opacity var(--crosshook-transition-standard) ease;
}

.crosshook-game-cover-art__image--loaded {
  opacity: 1;
}

.crosshook-game-cover-art__overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(transparent 40%, var(--crosshook-color-surface) 100%);
  pointer-events: none;
}

.crosshook-game-cover-art__placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  font-size: 2rem;
  font-weight: 600;
  color: var(--crosshook-color-text-muted);
  background: var(--crosshook-color-bg-elevated);
}

.crosshook-game-cover-art__skeleton {
  width: 100%;
  height: 100%;
  background: linear-gradient(
    90deg,
    var(--crosshook-color-bg-elevated) 25%,
    var(--crosshook-color-surface) 50%,
    var(--crosshook-color-bg-elevated) 75%
  );
  background-size: 200% 100%;
  animation: crosshook-skeleton-pulse 1.5s ease-in-out infinite;
}

@keyframes crosshook-skeleton-pulse {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

/* Genre pill tags */
.crosshook-genre-pill {
  display: inline-flex;
  padding: 2px 10px;
  border-radius: 100px;
  background: var(--crosshook-color-accent-soft);
  color: var(--crosshook-color-text);
  font-size: 0.75rem;
  line-height: 1.5;
}

/* Game card grid — cover art library (Figma concept) */
.crosshook-game-card {
  position: relative;
  cursor: pointer;
  border-radius: var(--crosshook-radius-md);
  overflow: hidden;
  border: 1px solid var(--crosshook-color-border);
  transition: border-color var(--crosshook-transition-fast) ease;
}

.crosshook-game-card--selected {
  border-color: var(--crosshook-color-accent);
  box-shadow: 0 0 0 1px var(--crosshook-color-accent);
}

.crosshook-game-card--favorite {
  border-color: var(--crosshook-color-warning);
}

.crosshook-game-card:hover {
  border-color: var(--crosshook-color-border-strong);
}

/* Card action overlay — revealed on hover, always visible in controller mode */
.crosshook-game-card__actions {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  padding: 8px;
  opacity: 0;
  transition: opacity var(--crosshook-transition-fast) ease;
  z-index: 1;
}

.crosshook-game-card:hover .crosshook-game-card__actions,
.crosshook-game-card:focus-within .crosshook-game-card__actions {
  opacity: 1;
}

:root[data-crosshook-controller-mode='true'] .crosshook-game-card__actions {
  opacity: 1; /* always visible in controller mode — no hover */
}

.crosshook-game-card__action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: var(--crosshook-touch-target-compact);
  min-height: var(--crosshook-touch-target-compact);
  border: none;
  border-radius: var(--crosshook-radius-sm);
  background: rgba(0, 0, 0, 0.6);
  color: var(--crosshook-color-text);
  cursor: pointer;
  backdrop-filter: blur(4px);
  transition: background var(--crosshook-transition-fast) ease;
}

.crosshook-game-card__action-btn:hover {
  background: rgba(0, 0, 0, 0.8);
}

.crosshook-game-card__action-btn--launch {
  background: var(--crosshook-color-accent);
  color: #fff;
}

.crosshook-game-card__action-btn--launch:hover {
  background: var(--crosshook-color-accent-strong);
}

/* Game card text overlay at bottom */
.crosshook-game-card__info {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 32px 12px 12px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.8));
}

.crosshook-game-card__title {
  font-size: 0.85rem;
  font-weight: 600;
  color: #fff;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.crosshook-game-card__badges {
  display: flex;
  gap: 4px;
  margin-top: 4px;
}
```

### Controller Mode

All existing `crosshook-subtab-*` variables already have controller-mode overrides in `variables.css:87-88`:

```css
:root[data-crosshook-controller-mode='true'] {
  --crosshook-subtab-min-height: 48px;
  --crosshook-subtab-padding-inline: 20px;
}
```

No additional controller-mode CSS changes needed. Game cover art cards should use `--crosshook-touch-target-min` for minimum hit area in controller mode.

---

## Migration Path

### Phase 1 — Extract Shared UI Components (Low Risk)

**Goal**: Reduce `ProfileFormSections.tsx` line count without changing behavior.

1. Extract `FieldRow` to `components/ui/FieldRow.tsx`
2. Reconcile `ProtonPathField` — the local version in ProfileFormSections (lines 166-231) vs the existing `components/ui/ProtonPathField.tsx` file. Consolidate into one.
3. Extract `OptionalSection` to `components/ui/OptionalSection.tsx` (small, ~15 lines)
4. Extract `TrainerVersionSetField` to `components/profile-sections/TrainerVersionSetField.tsx`
5. Extract `LauncherMetadataFields` to `components/profile-sections/LauncherMetadataFields.tsx`

**Verification**: All form fields still render and update profile correctly. `cargo test -p crosshook-core` passes (no backend change). Visual diff: none.

### Phase 2 — Split ProfileFormSections into Section Components (Medium Risk)

**Goal**: Each logical form section becomes its own component file.

1. Create `components/profile-sections/` directory
2. Create `ProfileIdentitySection.tsx`, `GameSection.tsx`, `RunnerMethodSection.tsx`
3. Create `TrainerSection.tsx` (conditional on launch method)
4. Create `RuntimeSection.tsx` (conditional per launch method variant)
5. Reduce `ProfileFormSections.tsx` to a thin composition:

   ```tsx
   export function ProfileFormSections(props) {
     return (
       <div className="crosshook-profile-shell">
         <ProfileIdentitySection {...} />
         <GameSection {...} />
         <RunnerMethodSection {...} />
         <CustomEnvironmentVariablesSection {...} />
         {supportsTrainer && <TrainerSection {...} />}
         <RuntimeSection {...} />
       </div>
     );
   }
   ```

6. Keep `ProfileFormSections` export for backward compatibility (OnboardingWizard uses it in review mode).

**Verification**: Same as Phase 1. No visible change to users.

### Phase 3 — Add Sub-Tab Navigation (Medium Risk)

**Goal**: Replace collapsed "Advanced" section with sub-tab layout.

1. Add `ProfileSubTabs.tsx` component
2. In `ProfilesPage.tsx`:
   - Remove the outer `CollapsibleSection("Advanced")` wrapper
   - Add `useState<ProfileSubTab>('general')` for active tab
   - Render `ProfileSubTabs` row below the profile selector
   - Conditionally render section components based on active tab
   - Move `ProfileActions` OUTSIDE the sub-tab content area (always visible)
3. Profile selector bar, health badges, and actions bar remain always-visible
4. Launcher Export panel stays as a separate `CollapsibleSection` below the main profile panel

**Verification**: All form fields accessible via sub-tabs. Profile save/load/delete still works. Dirty indicator reflects changes from any tab. Health issues visible in Health tab.

### Phase 4 — Game Metadata & Cover Art Backend (Medium Risk — #52)

**Goal**: Rust infrastructure for fetching and caching game metadata and cover art.

1. Add `game_image_cache` table via migration v13-to-v14 in `metadata/migrations.rs`
2. Create `metadata/game_image_store.rs` with CRUD operations
3. Create `steam_metadata/` module (client.rs, models.rs, mod.rs)
4. Create `game_images/` module (client.rs, cache.rs, models.rs, mod.rs)
5. Add `steamgriddb_api_key` field to `AppSettingsData`
6. Add MetadataStore integration methods
7. Create `commands/game_metadata.rs` with IPC handlers
8. Register commands in `lib.rs` invoke_handler

**Verification**: `cargo test -p crosshook-core` passes. Migration test for v14 validates table creation. Unit tests for `validate_image_bytes`, `metadata_cache_key`, `image_cache_path`.

### Phase 5 — Game Metadata & Cover Art Frontend (#52)

**Goal**: Frontend components consuming the backend image/metadata infrastructure.

1. Create `types/game-metadata.ts` with TypeScript interfaces
2. Create `hooks/useGameMetadata.ts` and `hooks/useGameCoverArt.ts`
3. Create `components/GameCoverArt.tsx` with loading/error/placeholder states
4. Create `components/GameMetadataBar.tsx` with genre pills and description
5. Add new CSS classes to theme/component CSS files
6. Integrate `GameCoverArt` and `GameMetadataBar` into General tab content
7. Add SteamGridDB API key field to SettingsPage

**Verification**: Cover art loads for profiles with `steam.app_id`. Placeholder shown when no app_id. Error state graceful. Stale cache served when offline.

### Phase 6 — CSS and Layout Polish (Low Risk)

1. Apply `crosshook-subtab-row` / `crosshook-subtab` / `crosshook-subtab--active` classes
2. Add any needed spacing/container CSS
3. Implement Figma-concept sidebar layout (if approved for this iteration)
4. Test controller mode (larger touch targets)
5. Test responsive breakpoints (max-width: 900px, max-height: 820px)

---

## Technical Decisions

### Decision 0: Overall Layout Strategy

Four approaches were evaluated for decluttering the Profiles page Advanced section:

| Approach                                                                                  | Scroll Reduction                | Visual Clarity                                 | Random-Access Editing    | Effort | Recommendation  |
| ----------------------------------------------------------------------------------------- | ------------------------------- | ---------------------------------------------- | ------------------------ | ------ | --------------- |
| **A. Sub-tabs** (replace collapsed section with tabbed sections)                          | High — only active tab rendered | High — each tab is focused                     | Full — click any tab     | Medium | Good            |
| **B. Card-based containers** (each logical group gets its own `crosshook-panel`)          | None — all content visible      | Medium — visual boundaries help but still long | Full — scroll to section | Low    | Complement only |
| **C. Hybrid: promote + sub-tabs** (always-visible essentials + sub-tabs for form content) | High                            | High                                           | Full                     | Medium | **Recommended** |
| **D. Progressive disclosure stepper** (inline step-by-step flow like OnboardingWizard)    | High                            | Medium                                         | Poor — sequential only   | Medium | Rejected        |

**Recommendation**: **Approach C (Hybrid promote + sub-tabs)**. Promote the profile selector, wizard access, health badges, and actions bar to always-visible positions outside the sub-tab content area. Use sub-tabs for the form sections (General, Runtime, Environment, Health). Within each tab, use `CollapsibleSection` for optional/advanced subsections (matching the existing LaunchPage pattern where each concern — Gamescope, MangoHud, Optimizations, Steam Launch Options — is its own `CollapsibleSection` panel).

**Why not pure cards (B)?** Cards alone do not reduce the vertical scroll length — the Advanced section content is too long when all fields are expanded for Steam or Proton launch methods. Cards are a good complement within sub-tabs (visual grouping inside a tab) but insufficient as the sole strategy.

**Why not stepper (D)?** Power users need random access to any field at any time. The `OnboardingWizard` already provides a guided linear flow for first-time setup as a modal overlay — duplicating that pattern inline would confuse the two use cases and block experienced users who want to jump directly to, say, environment variables.

**Why hybrid (C) over pure sub-tabs (A)?** The key insight is that the profile selector, wizard buttons, health badges, and save/delete actions should never be hidden behind a tab boundary. By promoting these to always-visible positions, the sub-tabs only organize the form fields themselves — which is where the clutter actually lives. This matches existing patterns: the profile selector and wizard area already sit above the Advanced section, and the LaunchPage places its profile selector and launch controls outside its collapsible panels.

### Decision 1: Sub-Tab Implementation

| Option                   | Complexity | Accessibility      | Layout Flexibility          | Recommendation       |
| ------------------------ | ---------- | ------------------ | --------------------------- | -------------------- |
| Local useState + buttons | Low        | Manual ARIA needed | High                        | **Recommended**      |
| Nested Radix Tabs        | Medium     | Built-in           | Constrained by Tabs.Content | Viable alternative   |
| URL hash routing         | High       | N/A                | N/A                         | Rejected (no router) |

**Recommendation**: Local `useState` with plain buttons. The app has no URL router; Radix Tabs constrains layout. Manual ARIA attributes (`role="tablist"`, `role="tab"`, `role="tabpanel"`, `aria-selected`, `aria-controls`) are straightforward.

**Note on Radix Tabs**: `@radix-ui/react-tabs` is already a dependency (used for sidebar routing in App.tsx). A nested `Tabs.Root` inside the profile panel would provide built-in keyboard navigation (arrow keys between tabs) and ARIA roles for free. The trade-off is that Radix Tabs enforces a `Tabs.List` + `Tabs.Content` structure that may constrain layout flexibility if the actions bar or health badges need to sit between the tab list and tab content. If accessibility is prioritized over layout flexibility, Radix sub-tabs are a strong alternative.

### Decision 2: Content Rendering Strategy

| Option                                | DOM Weight | State Preservation              | Re-fetch Behavior     |
| ------------------------------------- | ---------- | ------------------------------- | --------------------- |
| Conditional render (unmount inactive) | Light      | Via ProfileContext              | Hooks re-run on mount |
| Hidden render (CSS display:none)      | Heavy      | Component-local state preserved | No re-fetch           |

**Recommendation**: **Conditional render** (unmount inactive tabs). ProfileContext preserves all form state regardless. ProtonDB lookup will re-fetch when the Environment tab mounts, but the hook handles caching. This keeps the DOM minimal.

### Decision 3: Where to Place Actions Bar

| Option                                | Discoverability              | UX              |
| ------------------------------------- | ---------------------------- | --------------- |
| Inside sub-tab content (current)      | Poor — hidden when collapsed | Bad             |
| Fixed below sub-tabs (always visible) | Excellent                    | **Recommended** |
| Floating/sticky at bottom of scroll   | Good                         | Complex CSS     |

**Recommendation**: Actions bar fixed below the sub-tab content area, inside the main panel but outside the sub-tab switching logic. Always visible regardless of which tab is selected.

### Decision 4: OnboardingWizard Compatibility

The `OnboardingWizard` imports and uses `ProfileFormSections` with `reviewMode={true}` and a `profileSelector` prop. Two paths:

| Option                                   | Effort | Risk        |
| ---------------------------------------- | ------ | ----------- |
| Keep ProfileFormSections as thin wrapper | Low    | None        |
| Create separate ReviewFormSections       | Medium | Duplication |

**Recommendation**: Keep `ProfileFormSections` as a thin composition of the new section components. The wizard passes `reviewMode` which collapses optional sections — this behavior transfers naturally to the section components. The wizard's modal overlay operates independently of the sub-tab layout; it renders its own copy of `ProfileFormSections` within the modal, not within the page's sub-tab content area.

### Decision 5: Image Storage Strategy (#52)

| Option              | DB Impact     | Performance     | Cleanup          | Recommendation  |
| ------------------- | ------------- | --------------- | ---------------- | --------------- |
| Filesystem + SQLite | No bloat      | OS file caching | Two-system coord | **Recommended** |
| SQLite BLOB         | Massive bloat | Slow for >100KB | Atomic           | Rejected        |
| Filesystem only     | No tracking   | Fast            | Orphan risk      | Rejected        |

**Recommendation**: Filesystem for image binaries, SQLite `game_image_cache` table for metadata tracking. This matches the architectural decision in issue #52: images exceed the `external_cache_entries` 512 KiB cap, so they cannot go in the existing cache table. The SQLite table tracks path, checksum, source, and expiry; the filesystem holds the actual bytes. Eviction deletes filesystem files first, then removes SQLite rows.

### Decision 6: Image Fetch Strategy (#52)

| Option                | Startup Cost | First-View UX         | Bandwidth    | Recommendation  |
| --------------------- | ------------ | --------------------- | ------------ | --------------- |
| Lazy (on demand)      | None         | Loading spinner once  | Minimal      | **Recommended** |
| Eager (background)    | N API calls  | Instant after startup | All profiles | Rejected        |
| Hybrid (visible only) | Some         | Good                  | Moderate     | Future option   |

**Recommendation**: Lazy fetch on demand, matching ProtonDB's cache-first pattern. The `useGameCoverArt` hook triggers a fetch when the component mounts and no valid cache exists. 7-day TTL with stale fallback on network failure.

### Decision 7: Image Source Priority (#52)

| Strategy                        | Auth Required | Art Quality    | Coverage  | Recommendation  |
| ------------------------------- | ------------- | -------------- | --------- | --------------- |
| Steam CDN default               | No            | Good (600x900) | All Steam | **Recommended** |
| SteamGridDB default             | API key       | Better         | ~80%      | Rejected        |
| Steam CDN + SteamGridDB upgrade | Optional      | Best available | Full      | **Recommended** |

**Recommendation**: Steam CDN as primary source (free, no auth, reliable for all Steam games). SteamGridDB as optional upgrade when user provides API key in settings. The `preferred_source` column in `game_image_cache` allows per-game override.

---

## Updated Files to Create/Modify List

### Files to Create (Backend — #52)

| File                                                     | Purpose                                   |
| -------------------------------------------------------- | ----------------------------------------- |
| `crates/crosshook-core/src/steam_metadata/mod.rs`        | Module root, public re-exports            |
| `crates/crosshook-core/src/steam_metadata/client.rs`     | Steam Store API client, cache-first fetch |
| `crates/crosshook-core/src/steam_metadata/models.rs`     | SteamAppDetails, lookup result structs    |
| `crates/crosshook-core/src/game_images/mod.rs`           | Module root, public re-exports            |
| `crates/crosshook-core/src/game_images/client.rs`        | Image download (Steam CDN + SteamGridDB)  |
| `crates/crosshook-core/src/game_images/cache.rs`         | Filesystem cache ops, image validation    |
| `crates/crosshook-core/src/game_images/models.rs`        | ImageSource, GameCoverArtResult structs   |
| `crates/crosshook-core/src/metadata/game_image_store.rs` | SQLite CRUD for game_image_cache table    |
| `src-tauri/src/commands/game_metadata.rs`                | IPC command handlers                      |

### Files to Create (Frontend — #52 + restructuring)

| File                                                          | Purpose                                             |
| ------------------------------------------------------------- | --------------------------------------------------- |
| `src/components/ProfileSubTabs.tsx`                           | Sub-tab row + content routing                       |
| `src/components/profile-sections/ProfileIdentitySection.tsx`  | Profile name field                                  |
| `src/components/profile-sections/GameSection.tsx`             | Game name + executable path                         |
| `src/components/profile-sections/RunnerMethodSection.tsx`     | Launch method selector                              |
| `src/components/profile-sections/TrainerSection.tsx`          | Trainer config fields                               |
| `src/components/profile-sections/RuntimeSection.tsx`          | Steam/Proton/Native runtime fields                  |
| `src/components/profile-sections/LauncherMetadataSection.tsx` | Launcher name + icon                                |
| `src/components/GameCoverArt.tsx`                             | Cover art display with states                       |
| `src/components/GameMetadataBar.tsx`                          | Genres, developer, description                      |
| `src/components/GameCard.tsx`                                 | Cover art card with action overlays (Figma concept) |
| `src/components/ProfileGameCardGrid.tsx`                      | Grid/list library view of profile cards             |
| `src/hooks/useGameMetadata.ts`                                | Steam metadata fetch hook                           |
| `src/hooks/useGameCoverArt.ts`                                | Cover art fetch + asset URL hook                    |
| `src/types/game-metadata.ts`                                  | TypeScript interfaces                               |

### Files to Modify

| File                                               | Changes                                                   |
| -------------------------------------------------- | --------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/migrations.rs` | Add `migrate_13_to_14` for `game_image_cache` table       |
| `crates/crosshook-core/src/metadata/mod.rs`        | Add `game_image_store` submodule, MetadataStore methods   |
| `crates/crosshook-core/src/settings/mod.rs`        | Add `steamgriddb_api_key` to `AppSettingsData`            |
| `crates/crosshook-core/src/lib.rs`                 | Add `pub mod steam_metadata; pub mod game_images;`        |
| `src-tauri/src/commands/mod.rs`                    | Add `pub mod game_metadata;`                              |
| `src-tauri/src/lib.rs`                             | Register 4 new IPC commands in `invoke_handler`           |
| `src/components/pages/ProfilesPage.tsx`            | Remove Advanced wrapper, add sub-tab state, integrate art |
| `src/components/ProfileFormSections.tsx`           | Reduce to thin composition of section components          |
| `src/components/ui/InstallField.tsx`               | Add `id` prop for FieldRow replacement                    |
| `src/components/pages/SettingsPage.tsx`            | Add SteamGridDB API key input field                       |
| `src/types/index.ts`                               | Re-export game metadata types                             |
| `src/types/settings.ts`                            | Add `steamgriddb_api_key` to settings type                |

---

## Codebase Pattern Reuse Analysis

### ProtonDB Client as Template

The `protondb/` module (3 files, ~380 lines in client.rs) provides a complete template for the Steam metadata and image clients:

| ProtonDB Pattern                   | Reuse in Steam Metadata              | Reuse in Game Images                    |
| ---------------------------------- | ------------------------------------ | --------------------------------------- |
| `normalize_app_id()`               | Direct reuse (same validation)       | Direct reuse                            |
| `cache_key_for_app_id()` format    | `metadata_cache_key()` same pattern  | N/A (uses SQLite table, not cache key)  |
| `lookup_protondb()` signature      | `fetch_steam_metadata()` same shape  | `fetch_game_cover_art()` similar shape  |
| `load_cached_lookup_row()` pattern | Reuse `get_cache_entry()` from store | New: `get_cached_image()` from store    |
| `persist_lookup_result()` pattern  | Reuse `put_cache_entry()` from store | New: `upsert_cached_image()` + fs write |
| Stale cache fallback on failure    | Same pattern                         | Same pattern (return stale file path)   |
| `reqwest::Client` with timeout     | Same builder pattern                 | Same + streaming for large image files  |
| User-Agent header                  | Same `CrossHook/{version}` format    | Same format                             |

### MetadataStore Integration Pattern

Every existing store module follows the same pattern:

1. Private module file (e.g., `cache_store.rs`) with free functions taking `&Connection`
2. Public methods on `MetadataStore` that call `self.with_conn("action description", |conn| ...)`
3. Error mapping to `MetadataStoreError::Database { action, source }`

The new `game_image_store.rs` follows this exactly.

### Frontend Hook Pattern

`useProtonDbLookup.ts` (170 lines) provides the exact template for both `useGameMetadata.ts` and `useGameCoverArt.ts`:

- `requestIdRef` for race condition protection across async invoke calls
- Normalized app ID memoization
- Loading/ready/stale/unavailable state machine
- `refresh()` callback for force-refresh
- Effect that re-triggers on `normalizedAppId` change

## Open Questions

1. **Tab naming**: Should the "Runtime" tab be named differently based on launch method (e.g., "Steam Runtime" vs "Proton Runtime" vs "Native Runtime")? The current section title in ProfileFormSections already does this.

2. **Health tab visibility**: Should the Health sub-tab always appear, or only when issues are detected? Showing it always provides consistency but may confuse users with healthy profiles.

3. **Launcher Export placement**: Currently a separate `CollapsibleSection` below the profile panel. Should it become a sub-tab, or stay as a separate panel? It's conditionally shown only for steam_applaunch and proton_run methods.

4. **ProtonDB card placement**: Currently in the Runtime section. Moving it to the Environment tab (alongside custom env vars) makes logical sense since its primary action is merging env vars. But it also reads `steam.app_id` which is a Runtime concept. Which tab grouping is more intuitive?

5. **Wizard setup flow**: The OnboardingWizard guides users through fields linearly. If fields are now in sub-tabs, should the wizard auto-navigate between tabs, or keep its own linear flow (modal overlay)?

6. **Alternative sub-tab groupings**: An alternative grouping of "Profile" / "Trainer" / "Runtime" / "Export" was considered, which collapses launcher metadata and community export into an "Export" tab. This may be worth evaluating against the "General" / "Runtime" / "Environment" / "Health" grouping proposed here — particularly whether export-related fields (launcher name, icon) belong with their associated runtime fields or in a dedicated export tab.

7. **Card grid vs. dropdown coexistence (#52)**: Should the cover art card grid fully replace the `ThemedSelect` profile dropdown, or coexist as an alternative browsing mode? The dropdown is compact and familiar; the grid is visual but takes more space. Options: grid as primary with dropdown as compact fallback, or grid as a new "Library" sub-view.

8. **SteamGridDB API key onboarding (#52)**: Should users be prompted to provide a SteamGridDB API key during onboarding, or only when they navigate to Settings? The key is optional — Steam CDN provides adequate cover art without it.

9. **Card action placement (#52)**: Should card actions (favorite, edit, launch) appear as a bottom bar below the game name, or as corner icon buttons? Bottom bar is more discoverable; corner icons save space. Controller mode needs always-visible actions regardless.

10. **Direct launch from card safety (#52)**: Launching a game directly from a card skips the profile editor review. Should there be a confirmation step, or is direct launch appropriate for profiles that have been successfully launched before? The `launch_operations` table tracks launch history — could gate direct launch on prior success.

11. **Profiles without steam_app_id (#52)**: Many profiles (especially `proton_run` and `native`) may not have a `steam.app_id`. These cannot fetch cover art from Steam. Options: always show placeholder with game name, allow manual image assignment, or parse executable name for fuzzy Steam search.
