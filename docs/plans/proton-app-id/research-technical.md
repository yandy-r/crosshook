# Technical Specification: Proton App ID & Tri-Art System

## Executive Summary

This document specifies the architecture for adding optional Steam App ID support to `proton_run` game profiles and extending art management to a tri-art system (cover, portrait, background) with custom art upload and mix-and-match resolution. The design preserves full backward compatibility with existing profiles, requires no SQLite schema migration, and follows established codebase patterns for TOML serialization, IPC commands, and frontend hooks.

**Key design decisions:**

1. New `steam_app_id` field on `RuntimeSection` (not reusing `steam.app_id`) for clean semantic separation.
2. Flat per-type custom art path fields on `GameSection` matching the existing `custom_cover_art_path` pattern.
3. Frontend-driven art resolution via a `resolveArtAppId()` utility, extending the existing `useGameCoverArt` hook.
4. Generalized `import_custom_art` backend function replacing the current cover-only import.

---

## Architecture Design

### Component Diagram

```
                          +-----------------------+
                          |   Frontend (React)    |
                          |                       |
                          |  useGameCoverArt()    |
                          |  useGameArt()  [new]  |
                          |  resolveArtAppId()    |
                          +----------+------------+
                                     |  invoke()
                          +----------v------------+
                          |  Tauri IPC Commands   |
                          |                       |
                          |  fetch_game_cover_art |
                          |  import_custom_art    |
                          +----------+------------+
                                     |
                          +----------v------------+
                          |   crosshook-core      |
                          |                       |
                          |  game_images/         |
                          |    client.rs          |
                          |    import.rs          |
                          |    steamgriddb.rs     |
                          |  profile/models.rs    |
                          |  metadata/ (SQLite)   |
                          +----------+------------+
                                     |
                     +---------------+---------------+
                     |                               |
              +------v------+              +---------v--------+
              | Steam CDN   |              | SteamGridDB API  |
              | (cover,     |              | (grids, heroes)  |
              | portrait,   |              |                  |
              | hero)       |              |                  |
              +-------------+              +------------------+
```

### New Components

None. All changes fit within existing modules. The art system extension is additive.

### Integration Points

1. **Profile model** (`profile/models.rs`): `RuntimeSection` gains `steam_app_id`; `GameSection` gains two new custom art path fields.
2. **Art import pipeline** (`game_images/import.rs`): Generalized from cover-only to any art type.
3. **Art download pipeline** (`game_images/client.rs`, `steamgriddb.rs`): `GameImageType::Background` variant added.
4. **Profile summary** (`commands/profile.rs`): Resolves effective steam_app_id from both `steam.app_id` and `runtime.steam_app_id`.
5. **Frontend art hook** (`hooks/useGameCoverArt.ts`): Accepts resolved app_id from new utility.
6. **Media section UI** (`components/profile-sections/MediaSection.tsx`): Expands to three art type fields.

---

## Data Models

### Rust Struct Changes

#### `RuntimeSection` (profile/models.rs)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeSection {
    #[serde(rename = "prefix_path", default)]
    pub prefix_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
    #[serde(rename = "working_directory", default)]
    pub working_directory: String,
    // NEW: Optional Steam App ID for media/metadata lookup only.
    // Does NOT affect launch behavior.
    #[serde(rename = "steam_app_id", default, skip_serializing_if = "String::is_empty")]
    pub steam_app_id: String,
}

impl RuntimeSection {
    pub fn is_empty(&self) -> bool {
        self.prefix_path.trim().is_empty()
            && self.proton_path.trim().is_empty()
            && self.working_directory.trim().is_empty()
            // NOTE: steam_app_id is deliberately EXCLUDED from is_empty().
            // is_empty() gates skip_serializing_if for TOML output. steam_app_id is
            // portable metadata (not a path), so a profile with only steam_app_id set
            // must still emit the [runtime] section to persist the value.
    }
}
```

#### `GameSection` (profile/models.rs)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameSection {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
    #[serde(rename = "custom_cover_art_path", default, skip_serializing_if = "String::is_empty")]
    pub custom_cover_art_path: String,
    // NEW: Per-type custom art paths
    #[serde(rename = "custom_portrait_art_path", default, skip_serializing_if = "String::is_empty")]
    pub custom_portrait_art_path: String,
    #[serde(rename = "custom_background_art_path", default, skip_serializing_if = "String::is_empty")]
    pub custom_background_art_path: String,
}
```

#### `LocalOverrideGameSection` (profile/models.rs)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalOverrideGameSection {
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
    #[serde(rename = "custom_cover_art_path", default)]
    pub custom_cover_art_path: String,
    // NEW
    #[serde(rename = "custom_portrait_art_path", default)]
    pub custom_portrait_art_path: String,
    #[serde(rename = "custom_background_art_path", default)]
    pub custom_background_art_path: String,
}

impl LocalOverrideGameSection {
    pub fn is_empty(&self) -> bool {
        self.executable_path.trim().is_empty()
            && self.custom_cover_art_path.trim().is_empty()
            && self.custom_portrait_art_path.trim().is_empty()   // NEW
            && self.custom_background_art_path.trim().is_empty() // NEW
    }
}
```

#### `GameImageType` (game_images/models.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameImageType {
    Cover,
    Hero,
    Capsule,
    Portrait,
    Background,  // NEW
}

impl fmt::Display for GameImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cover => write!(f, "cover"),
            Self::Hero => write!(f, "hero"),
            Self::Capsule => write!(f, "capsule"),
            Self::Portrait => write!(f, "portrait"),
            Self::Background => write!(f, "background"),
        }
    }
}
```

#### `GameProfile::effective_profile()` updates (profile/models.rs)

The `effective_profile()`, `storage_profile()`, and `portable_profile()` methods must include the new custom art path fields (`custom_portrait_art_path`, `custom_background_art_path`) in their override/clear logic. Pattern matches existing `custom_cover_art_path` handling.

**Important**: `runtime.steam_app_id` is portable metadata, NOT a machine-local path. It stays in the base `[runtime]` section and is NOT moved to `local_override`. The `effective_profile()` / `storage_profile()` / `portable_profile()` methods do NOT need to handle it — it is preserved as-is across all profile representations.

#### Helper: `resolve_art_app_id` (profile/models.rs or new utility)

```rust
/// Returns the effective Steam App ID for art/metadata lookup.
/// Priority: steam.app_id (for steam_applaunch) > runtime.steam_app_id (for proton_run).
pub fn resolve_art_app_id(profile: &GameProfile) -> &str {
    let steam = profile.steam.app_id.trim();
    if !steam.is_empty() {
        return steam;
    }
    profile.runtime.steam_app_id.trim()
}
```

### TOML Profile Format

Existing profiles remain unchanged. New fields are optional and omitted when empty.

```toml
[game]
name = "Elden Ring"
executable_path = ""
custom_cover_art_path = ""
custom_portrait_art_path = ""       # NEW (omitted when empty)
custom_background_art_path = ""     # NEW (omitted when empty)

[steam]
enabled = false
app_id = ""

[runtime]
prefix_path = "/path/to/prefix"
proton_path = "/path/to/proton"
working_directory = ""
steam_app_id = "1245620"            # NEW (omitted when empty)

[launch]
method = "proton_run"
```

### SQLite Schema

**No migration required.** The existing `game_image_cache` table (migration v13->v14) uses `image_type TEXT` which already accepts arbitrary type strings. The `Background` type will be stored as `"background"` and keyed by `(steam_app_id, image_type, source)`.

Existing table definition (for reference):

```sql
CREATE TABLE game_image_cache (
    cache_id         TEXT PRIMARY KEY,
    steam_app_id     TEXT NOT NULL,
    image_type       TEXT NOT NULL DEFAULT 'cover',   -- 'cover', 'portrait', 'hero', 'background'
    source           TEXT NOT NULL DEFAULT 'steam_cdn',
    file_path        TEXT NOT NULL,
    file_size        INTEGER NOT NULL DEFAULT 0,
    content_hash     TEXT NOT NULL DEFAULT '',
    mime_type        TEXT NOT NULL DEFAULT 'image/jpeg',
    width            INTEGER,
    height           INTEGER,
    source_url       TEXT NOT NULL DEFAULT '',
    preferred_source TEXT NOT NULL DEFAULT 'auto',
    expires_at       TEXT,
    fetched_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
-- Unique index: (steam_app_id, image_type, source)
```

### TypeScript Type Changes

#### `types/profile.ts`

```typescript
export interface GameProfile {
  game: {
    name: string;
    executable_path: string;
    custom_cover_art_path?: string;
    custom_portrait_art_path?: string; // NEW
    custom_background_art_path?: string; // NEW
  };
  // ...
  runtime: {
    prefix_path: string;
    proton_path: string;
    working_directory: string;
    steam_app_id?: string; // NEW
  };
  // ...
  local_override?: {
    game: {
      executable_path: string;
      custom_cover_art_path?: string;
      custom_portrait_art_path?: string; // NEW
      custom_background_art_path?: string; // NEW
    };
    // ...
  };
}
```

#### `types/library.ts`

```typescript
export interface LibraryCardData {
  name: string;
  gameName: string;
  steamAppId: string; // Now resolves from steam.app_id OR runtime.steam_app_id
  customCoverArtPath?: string;
  customPortraitArtPath?: string; // NEW (optional, for future grid art selection)
  isFavorite: boolean;
}
```

---

## API Design

### Modified IPC Commands

#### `fetch_game_cover_art` (existing, no change needed)

Already accepts `app_id: String` and `image_type: Option<String>`. The frontend is responsible for passing the resolved app_id. Add `"background"` to the image_type match arm.

```rust
// commands/game_metadata.rs - update image_type matching
let image_type = match image_type.as_deref().unwrap_or("cover") {
    "hero" => GameImageType::Hero,
    "capsule" => GameImageType::Capsule,
    "portrait" => GameImageType::Portrait,
    "background" => GameImageType::Background,  // NEW
    _ => GameImageType::Cover,
};
```

#### `import_custom_cover_art` -> `import_custom_art` (generalized)

**Current signature:**

```rust
#[tauri::command]
pub fn import_custom_cover_art(source_path: String) -> Result<String, String>
```

**New signature:**

```rust
#[tauri::command]
pub fn import_custom_art(
    source_path: String,
    art_type: Option<String>,  // "cover" | "portrait" | "background"; defaults to "cover"
) -> Result<String, String>
```

**Request:** `{ sourcePath: string, artType?: string }`
**Response:** `string` (absolute path to imported file)
**Errors:** `string` (validation failure, I/O error)

The old `import_custom_cover_art` command should be kept as a thin wrapper for backward compatibility during the transition, then deprecated.

#### `profile_save` — save-time validation for `runtime.steam_app_id`

The `runtime.steam_app_id` field must be validated at save time (not just download time) to surface errors in the UI early. If the field is non-empty and not a pure decimal integer, `profile_save` should return an error.

```rust
// In profile_save command handler, before the store.save() call:
let runtime_app_id = data.runtime.steam_app_id.trim();
if !runtime_app_id.is_empty()
    && !runtime_app_id.chars().all(|c| c.is_ascii_digit())
{
    return Err(format!(
        "runtime.steam_app_id must be a decimal integer, got: {runtime_app_id:?}"
    ));
}
```

#### `profile_save` (existing, extend auto-import)

Currently auto-imports `custom_cover_art_path` if it's outside the managed media dir. Extend to also auto-import `custom_portrait_art_path` and `custom_background_art_path`.

```rust
// In profile_save command handler, after existing cover art import:
for (field_getter, field_setter, art_type) in [
    (|p: &GameProfile| p.game.custom_cover_art_path.clone(),
     |p: &mut GameProfile, v: String| p.game.custom_cover_art_path = v, "cover"),
    (|p: &GameProfile| p.game.custom_portrait_art_path.clone(),
     |p: &mut GameProfile, v: String| p.game.custom_portrait_art_path = v, "portrait"),
    (|p: &GameProfile| p.game.custom_background_art_path.clone(),
     |p: &mut GameProfile, v: String| p.game.custom_background_art_path = v, "background"),
] {
    let path = field_getter(&data).trim().to_string();
    if !path.is_empty() && !is_in_managed_media_dir(&path) {
        match import_custom_art(&path, art_type) {
            Ok(imported) => field_setter(&mut data, imported),
            Err(e) => tracing::warn!(...),
        }
    }
}
```

#### `profile_list_summaries` (existing, extend resolution)

Update to resolve the effective steam_app_id for art lookup:

```rust
// In ProfileSummary construction:
let effective_app_id = {
    let steam = effective.steam.app_id.trim();
    if steam.is_empty() {
        effective.runtime.steam_app_id.trim().to_string()
    } else {
        steam.to_string()
    }
};
summaries.push(ProfileSummary {
    name,
    game_name: effective.game.name.clone(),
    steam_app_id: effective_app_id,
    custom_cover_art_path: ...,
});
```

### Backend Core Changes

#### `game_images/import.rs` - Generalized import

```rust
/// Import a custom art image into the managed media directory.
/// `art_type` determines the subdirectory: "cover" -> media/covers,
/// "portrait" -> media/portraits, "background" -> media/backgrounds.
pub fn import_custom_art(source_path: &str, art_type: &str) -> Result<String, String> {
    let source = Path::new(source_path.trim());
    if !source.exists() {
        return Err(format!("source file does not exist: {}", source.display()));
    }

    let bytes = std::fs::read(source)
        .map_err(|e| format!("failed to read source file: {e}"))?;

    let mime = validate_image_bytes(&bytes)
        .map_err(|e| format!("image validation failed: {e}"))?;
    let ext = mime_extension(mime);

    let subdir = match art_type {
        "portrait" => "portraits",
        "background" => "backgrounds",
        _ => "covers",  // default and "cover"
    };
    let dest_dir = media_base_dir()?.join(subdir);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("failed to create media directory: {e}"))?;

    let hash = sha256_hex(&bytes);
    let dest_path = dest_dir.join(format!("{}.{ext}", &hash[..16]));

    if !dest_path.exists() {
        std::fs::write(&dest_path, &bytes)
            .map_err(|e| format!("failed to write imported art: {e}"))?;
    }

    dest_path
        .to_str()
        .ok_or_else(|| "media path contains non-UTF-8 characters".to_string())
        .map(String::from)
}

/// Backward-compatible wrapper.
pub fn import_custom_cover_art(source_path: &str) -> Result<String, String> {
    import_custom_art(source_path, "cover")
}
```

#### `game_images/client.rs` - Background URL builder

```rust
fn build_download_url(app_id: &str, image_type: GameImageType) -> String {
    match image_type {
        GameImageType::Cover => format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"
        ),
        GameImageType::Hero => format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero.jpg"
        ),
        GameImageType::Capsule => format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/capsule_616x353.jpg"
        ),
        GameImageType::Portrait => format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
        ),
        // NEW: Background uses the hero/library background image
        GameImageType::Background => format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero.jpg"
        ),
    }
}

fn filename_for(image_type: GameImageType, source: GameImageSource, extension: &str) -> String {
    let type_prefix = match image_type {
        GameImageType::Cover => "cover",
        GameImageType::Hero => "hero",
        GameImageType::Capsule => "capsule",
        GameImageType::Portrait => "portrait",
        GameImageType::Background => "background",  // NEW
    };
    // ... rest unchanged
}
```

#### `game_images/steamgriddb.rs` - Background endpoint

```rust
fn build_endpoint(app_id: &str, image_type: &GameImageType) -> String {
    let (path_segment, dimensions) = match image_type {
        GameImageType::Cover => ("grids", Some("460x215,920x430")),
        GameImageType::Hero => ("heroes", None),
        GameImageType::Capsule => ("grids", Some("342x482,600x900")),
        GameImageType::Portrait => ("grids", Some("342x482,600x900")),
        GameImageType::Background => ("heroes", None),  // NEW: same as hero
    };
    // ... rest unchanged
}
```

### Frontend Changes

#### `utils/art.ts` (NEW utility)

```typescript
import type { GameProfile } from '../types';

/** Resolve the effective Steam App ID for art/metadata lookup. */
export function resolveArtAppId(profile: GameProfile): string {
  const steamAppId = profile.steam?.app_id?.trim() ?? '';
  if (steamAppId) return steamAppId;
  return profile.runtime?.steam_app_id?.trim() ?? '';
}

/** Resolve the custom art path for a given art type. */
export function resolveCustomArtPath(
  profile: GameProfile,
  artType: 'cover' | 'portrait' | 'background'
): string | undefined {
  switch (artType) {
    case 'cover':
      return profile.game.custom_cover_art_path;
    case 'portrait':
      return profile.game.custom_portrait_art_path;
    case 'background':
      return profile.game.custom_background_art_path;
  }
}
```

#### `hooks/useGameCoverArt.ts` - No structural change

The hook already accepts `steamAppId` and `customCoverArtPath` as separate params. Callers use `resolveArtAppId()` and `resolveCustomArtPath()` to provide the correct values. The hook itself does not need to change.

#### `components/library/LibraryCard.tsx`

```typescript
// Update to use resolveArtAppId if profile data is available,
// or use the pre-resolved steamAppId from ProfileSummary (backend resolves)
const { coverArtUrl, loading } = useGameCoverArt(
  visible ? profile.steamAppId : undefined, // Already resolved by backend
  profile.customCoverArtPath, // For portrait: profile.customPortraitArtPath
  'portrait'
);
```

#### `components/profile-sections/MediaSection.tsx` - Tri-art fields

Expand from one field to three. Each field has: text input, Browse button, Clear button, optional preview thumbnail.

```typescript
// Pseudocode for the three art type fields:
const ART_TYPES = [
  {
    key: 'custom_cover_art_path',
    label: 'Custom Cover Art',
    artType: 'cover',
    helperText: 'Overrides Steam/SteamGridDB for the profile header backdrop (460x215 or larger landscape).',
  },
  {
    key: 'custom_portrait_art_path',
    label: 'Custom Portrait Art',
    artType: 'portrait',
    helperText: 'Overrides Steam/SteamGridDB for the library grid card (600x900 or taller portrait).',
  },
  {
    key: 'custom_background_art_path',
    label: 'Custom Background Art',
    artType: 'background',
    helperText: 'Overrides Steam/SteamGridDB for background imagery (1920x620 or wider landscape). Future use.',
  },
] as const;
```

#### `components/profile-sections/RuntimeSection.tsx` - Steam App ID for proton_run

The existing "Steam App ID" field in the `proton_run` section currently writes to `profile.steam.app_id`. This should be changed to write to `profile.runtime.steam_app_id` instead:

```typescript
// In the proton_run section:
<FieldRow
  label="Steam App ID"
  value={profile.runtime.steam_app_id ?? ''}
  onChange={(value) =>
    onUpdateProfile((current) => ({
      ...current,
      runtime: { ...current.runtime, steam_app_id: value },
    }))
  }
  placeholder="Optional — enables art download and metadata lookup"
/>
```

**Migration note:** Existing proton_run profiles that have `steam.app_id` set (from the old UI behavior) will continue to work because `resolveArtAppId()` checks `steam.app_id` first. Users can optionally move the value to `runtime.steam_app_id`.

---

## System Constraints

### Performance

1. **Lazy art loading**: Library grid already uses IntersectionObserver (`LibraryCard.tsx:33-44`). No change needed.
2. **Cache TTL**: 24-hour expiration on `game_image_cache` entries (`CACHE_TTL_HOURS = 24` in `client.rs:19`). Stale fallback on download failure.
3. **Request deduplication**: `useGameCoverArt` uses `requestIdRef` to cancel stale requests. No change needed.
4. **Profile summary**: `profile_list_summaries` loads all profiles synchronously. Adding `runtime.steam_app_id` resolution adds negligible overhead (string comparison).
5. **Import idempotency**: Content-addressed filenames (`hash[..16].ext`) prevent duplicate writes.

### Storage

**Media directory structure** (under `~/.local/share/crosshook/`):

```
media/
  covers/       # Custom cover art (existing)
  portraits/    # Custom portrait art (NEW)
  backgrounds/  # Custom background art (NEW)
cache/
  images/
    <app_id>/
      cover_steam_cdn.jpg
      cover_steamgriddb.png
      portrait_steam_cdn.jpg
      portrait_steamgriddb.webp
      background_steam_cdn.jpg   # NEW
      background_steamgriddb.jpg # NEW
      hero_steam_cdn.jpg         # Existing
```

### Persistence Classification

| Datum                        | Storage                                                              | Notes                                              |
| ---------------------------- | -------------------------------------------------------------------- | -------------------------------------------------- |
| `runtime.steam_app_id`       | TOML base section (portable)                                         | NOT a machine-local path; survives portable export |
| `custom_cover_art_path`      | TOML `local_override.game` (machine-local)                           | Stripped on portable export                        |
| `custom_portrait_art_path`   | TOML `local_override.game` (machine-local)                           | Stripped on portable export                        |
| `custom_background_art_path` | TOML `local_override.game` (machine-local)                           | Stripped on portable export                        |
| Downloaded art files         | Filesystem cache (`~/.local/share/crosshook/cache/images/{app_id}/`) | Ephemeral, re-downloadable                         |
| Art metadata (cache entries) | SQLite `game_image_cache`                                            | Operational metadata, auto-managed                 |
| Custom art files             | Filesystem (`~/.local/share/crosshook/media/{type}/`)                | Content-addressed, idempotent                      |

### Backward Compatibility

| Scenario                                       | Impact                                                                                                                             |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Old profile TOML opened by new app             | New fields default to empty string; behavior unchanged.                                                                            |
| New profile TOML opened by old app             | Unknown TOML keys (`steam_app_id`, `custom_portrait_art_path`, `custom_background_art_path`) ignored by serde `#[serde(default)]`. |
| Existing `steam.app_id` on proton_run profiles | Continues to work; `resolveArtAppId` checks `steam.app_id` first.                                                                  |
| Existing `custom_cover_art_path`               | Unchanged; field position and behavior preserved.                                                                                  |
| SQLite game_image_cache                        | No migration; new `"background"` type stored alongside existing types.                                                             |

### Security

1. **App ID validation**: Existing `app_id.chars().all(|c| c.is_ascii_digit())` check applies to `runtime.steam_app_id` via the same download pipeline. Additionally, `profile_save` validates `runtime.steam_app_id` at save time to surface errors in the UI before any network call.
2. **Custom art import**: All custom art goes through `validate_image_bytes()` (magic bytes, 5MB limit, MIME allow-list) and content-addressed storage.
3. **Path safety**: `safe_image_cache_path()` validates all constructed cache paths. `is_in_managed_media_dir()` prevents re-importing already-managed files.
4. **Art type parameter**: The `art_type` parameter in `import_custom_art` is matched against a fixed set (`"cover"`, `"portrait"`, `"background"`) — no arbitrary subdirectory creation.

---

## Codebase Changes

### Files to Modify

| File                                                   | Change                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models.rs`          | Add `steam_app_id` to `RuntimeSection` (portable, NOT in local_override, excluded from `is_empty()`); add `custom_portrait_art_path`, `custom_background_art_path` to `GameSection` and `LocalOverrideGameSection`; update `LocalOverrideGameSection::is_empty()`, `effective_profile()`, `storage_profile()`, `portable_profile()` for new art path fields only. Add `resolve_art_app_id()` helper. |
| `crates/crosshook-core/src/game_images/models.rs`      | Add `Background` variant to `GameImageType`, update `Display`.                                                                                                                                                                                                                                                                                                                                       |
| `crates/crosshook-core/src/game_images/client.rs`      | Add `Background` to `build_download_url()` and `filename_for()`.                                                                                                                                                                                                                                                                                                                                     |
| `crates/crosshook-core/src/game_images/steamgriddb.rs` | Add `Background` to `build_endpoint()`.                                                                                                                                                                                                                                                                                                                                                              |
| `crates/crosshook-core/src/game_images/import.rs`      | Generalize `import_custom_cover_art` to `import_custom_art(source_path, art_type)`. Keep backward-compat wrapper.                                                                                                                                                                                                                                                                                    |
| `crates/crosshook-core/src/game_images/mod.rs`         | Update public exports for `import_custom_art`.                                                                                                                                                                                                                                                                                                                                                       |
| `crates/crosshook-core/src/profile/mod.rs`             | Re-export `resolve_art_app_id` if placed in models.                                                                                                                                                                                                                                                                                                                                                  |
| `src-tauri/src/commands/game_metadata.rs`              | Add `"background"` to `image_type` match. Add `import_custom_art` command (generalized).                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/commands/profile.rs`                    | Update `profile_list_summaries` to resolve effective art app_id. Update `profile_save` to auto-import all three art types.                                                                                                                                                                                                                                                                           |
| `src-tauri/src/lib.rs`                                 | Register new `import_custom_art` command if created as separate from existing.                                                                                                                                                                                                                                                                                                                       |
| `src/types/profile.ts`                                 | Add `steam_app_id` to runtime, `custom_portrait_art_path` and `custom_background_art_path` to game.                                                                                                                                                                                                                                                                                                  |
| `src/types/library.ts`                                 | Optionally add `customPortraitArtPath` for future grid art selection.                                                                                                                                                                                                                                                                                                                                |
| `src/hooks/useGameCoverArt.ts`                         | No structural change; callers provide resolved values.                                                                                                                                                                                                                                                                                                                                               |
| `src/components/profile-sections/RuntimeSection.tsx`   | Change proton_run Steam App ID field to write `runtime.steam_app_id`. Update placeholder text.                                                                                                                                                                                                                                                                                                       |
| `src/components/profile-sections/MediaSection.tsx`     | Expand to three art type fields with per-type browse and clear.                                                                                                                                                                                                                                                                                                                                      |
| `src/components/library/LibraryCard.tsx`               | No change if backend resolves steamAppId in summary.                                                                                                                                                                                                                                                                                                                                                 |

### Files to Create

| File               | Purpose                                                              |
| ------------------ | -------------------------------------------------------------------- |
| `src/utils/art.ts` | `resolveArtAppId()` and `resolveCustomArtPath()` frontend utilities. |

### Dependencies

No new crate or npm dependencies required. All functionality uses existing libraries:

- `reqwest` (HTTP), `infer` (magic bytes), `rusqlite` (SQLite), `serde`/`toml` (serialization)
- Frontend: `@tauri-apps/api/core` (invoke), React hooks

---

## Technical Decisions

### Decision 1: steam_app_id field placement

| Option                                      | Pros                                                                                    | Cons                                                                                        |
| ------------------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| **A: `runtime.steam_app_id`** (recommended) | Clean semantic separation; `steam.*` stays launch-specific; matches issue #142 proposal | Requires migration logic for existing proton_run profiles that set `steam.app_id`           |
| B: Reuse `steam.app_id`                     | No new field; existing UI already writes there                                          | Conflates launch config with media lookup; `steam.enabled = false` semantics become unclear |

**Recommendation: Option A.** The `resolveArtAppId()` fallback chain (`steam.app_id` -> `runtime.steam_app_id`) provides natural migration: existing profiles continue to work, and users can optionally move the value.

### Decision 2: Custom art field structure

| Option                           | Pros                                                                                             | Cons                                                                                            |
| -------------------------------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| **A: Flat fields** (recommended) | Matches existing `custom_cover_art_path` pattern; simple TOML layout; easy `skip_serializing_if` | Three fields on GameSection instead of one                                                      |
| B: Nested struct                 | Grouping; extensible                                                                             | Different pattern from existing field; TOML nesting adds complexity; `#[serde(flatten)]` issues |

**Recommendation: Option A.** Consistency with existing code patterns outweighs the minor field count increase.

### Decision 3: Background vs Hero art type

| Option                                        | Pros                                                                                          | Cons                                                  |
| --------------------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| **A: Add `Background` variant** (recommended) | Clear semantic distinction for UI use (hero = internal, background = user-facing); extensible | Same CDN/SteamGridDB endpoint as Hero                 |
| B: Repurpose Hero as Background               | Fewer enum variants                                                                           | Breaks existing `Hero` usage if any; confusing naming |

**Recommendation: Option A.** `Background` maps to the same CDN endpoint (`library_hero.jpg`) and SteamGridDB endpoint (`/heroes/`) as `Hero`, but has distinct UI semantics. The `hero` and `capsule` types remain for future internal use.

### Decision 4: Art resolution location

| Option                                  | Pros                                                         | Cons                                                                  |
| --------------------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------- |
| **A: Frontend resolves** (recommended)  | Matches existing hook pattern; no new IPC round-trip; simple | Resolution logic in two places (frontend + backend summary)           |
| B: Backend resolves via new IPC command | Single source of truth; pure backend logic                   | Extra IPC call per art request; latency; breaks existing hook pattern |

**Recommendation: Option A.** A lightweight `resolveArtAppId()` utility on the frontend, plus backend resolution in `profile_list_summaries`, covers all cases without new IPC overhead.

---

## Open Questions

1. **Background art CDN mapping**: Steam CDN's `library_hero.jpg` is 1920x620. Is this the right asset for "background art"? Or should it map to a different Steam asset (e.g., `page_bg_generated_v6b.jpg`)? Decision: use `library_hero.jpg` as it's the most reliably available large landscape image.

2. **proton_run Steam App ID migration**: Should the app automatically migrate `steam.app_id` -> `runtime.steam_app_id` for existing proton_run profiles? Or let the fallback chain handle it? Decision: fallback chain is sufficient; no automatic migration needed.

3. **Background art UI timing**: Background art is described as "future use." Should the `custom_background_art_path` field be visible in the UI now, or hidden behind a feature flag? Recommendation: include in MediaSection now with "(future use)" helper text, so the data model is exercised even if no UI consumer displays it yet.

4. **SteamGridDB API background endpoint**: The SteamGridDB `heroes` endpoint returns large landscape images. Are the dimensions appropriate for background use? The typical hero image is 1920x620 or similar. Verification with actual SteamGridDB responses is recommended.

5. **Library grid art type**: The library grid currently always requests `portrait` type art. Should there be a user preference to switch between cover/portrait for grid display? This is a UX question beyond the scope of this feature.

---

## Relevant Files

- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — Core profile data models (RuntimeSection, GameSection, LocalOverrideGameSection)
- `/src/crosshook-native/crates/crosshook-core/src/game_images/client.rs` — Image download and cache pipeline
- `/src/crosshook-native/crates/crosshook-core/src/game_images/models.rs` — GameImageType enum, GameImageError, GameImageSource
- `/src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs` — SteamGridDB API client and endpoint builder
- `/src/crosshook-native/crates/crosshook-core/src/game_images/import.rs` — Custom art import (file validation, content-addressed storage)
- `/src/crosshook-native/crates/crosshook-core/src/metadata/game_image_store.rs` — SQLite game_image_cache CRUD
- `/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — Schema migration chain (v14 = game_image_cache)
- `/src/crosshook-native/src-tauri/src/commands/game_metadata.rs` — Tauri IPC: fetch_game_cover_art, import_custom_cover_art
- `/src/crosshook-native/src-tauri/src/commands/profile.rs` — Tauri IPC: profile_save, profile_list_summaries
- `/src/crosshook-native/src-tauri/src/lib.rs` — Tauri command registration
- `/src/crosshook-native/src/types/profile.ts` — TypeScript GameProfile type
- `/src/crosshook-native/src/types/library.ts` — LibraryCardData type
- `/src/crosshook-native/src/hooks/useGameCoverArt.ts` — Art loading hook with custom art priority
- `/src/crosshook-native/src/hooks/useGameMetadata.ts` — Steam metadata lookup hook
- `/src/crosshook-native/src/hooks/useLibrarySummaries.ts` — Library grid data fetching
- `/src/crosshook-native/src/components/profile-sections/MediaSection.tsx` — Custom cover art UI field
- `/src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx` — proton_run runtime fields including Steam App ID
- `/src/crosshook-native/src/components/library/LibraryCard.tsx` — Library grid card with lazy art loading
- `/src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx` — Profile editor cover art display
- `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — AppSettingsData with steamgriddb_api_key
