# Integration Research: Library Home

## Overview

The library-home feature integrates with five existing systems: Tauri IPC commands (profile and cover art), SQLite metadata DB (schema v14), the Rust cover art pipeline (`game_images` crate), the `ProfileStore` TOML layer, and the frontend ProfileContext/invoke pattern. All infrastructure exists; the main gap is one new Rust IPC command (`profile_list_summaries`) and a new `GameImageType::Portrait` enum variant with updated `build_download_url` fallback chain.

---

## IPC Commands

### Existing Profile Commands (No Changes)

All registered in `src/crosshook-native/src-tauri/src/lib.rs:189–279`.

| Command | Rust Signature | File |
|---|---|---|
| `profile_list` | `(store: State<ProfileStore>) -> Result<Vec<String>, String>` | `commands/profile.rs:222` |
| `profile_load` | `(name: String, store: State<ProfileStore>) -> Result<GameProfile, String>` | `commands/profile.rs:227` |
| `profile_set_favorite` | `(name: String, favorite: bool, app: AppHandle, metadata_store: State<MetadataStore>) -> Result<(), String>` | `commands/profile.rs:626` |
| `profile_list_favorites` | `(metadata_store: State<MetadataStore>) -> Result<Vec<String>, String>` | `commands/profile.rs:640` |
| `fetch_game_cover_art` | `(app_id: String, image_type: Option<String>, metadata_store: State<MetadataStore>, settings_store: State<SettingsStore>) -> Result<Option<String>, String>` | `commands/game_metadata.rs:18` |

#### `fetch_game_cover_art` detail

- `image_type` arg: `"hero"` → `GameImageType::Hero`, `"capsule"` → `GameImageType::Capsule`, anything else (incl. `"cover"`, `None`) → `GameImageType::Cover`
- Reads `steamgriddb_api_key` from `SettingsStore` (non-fatal on miss); passes as `Option<&str>` to `download_and_cache_image`
- Returns absolute disk path; frontend wraps in `convertFileSrc()` to get `asset://` URL

#### `profile_set_favorite` side-effects

- Calls `MetadataStore::set_profile_favorite` → SQLite UPDATE on `profiles.is_favorite`
- Emits `"profiles-changed"` Tauri event with payload `"favorite-updated"`
- `useProfile` hook re-fetches favorites on this event (`profiles-changed` listener at `useProfile.ts:1271`)

### New IPC Command Required

**`profile_list_summaries`** — not yet registered. Add to:

1. `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` or a new method on `ProfileStore`
2. `src/crosshook-native/src-tauri/src/commands/profile.rs` as `#[tauri::command]`
3. `src/crosshook-native/src-tauri/src/lib.rs` invoke handler list

Suggested Rust signature:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,
    pub custom_cover_art_path: String,
}

#[tauri::command]
pub fn profile_list_summaries(store: State<'_, ProfileStore>) -> Result<Vec<ProfileSummary>, String> {
    // iterate store.list(), load() each, extract fields
}
```

Implementation note: `ProfileStore::load()` applies `effective_profile()` (merges `local_override`), which is what the frontend should see for cover art resolution.

---

## Database Schema

### Current Schema Version: v14

The feature spec references "schema v13" — this is outdated. Migration 13→14 (file: `metadata/migrations.rs:642`) creates `game_image_cache`. **The current schema is v14.**

### `profiles` Table (created in migration 0→1, amended through v2)

```sql
CREATE TABLE profiles (
    profile_id       TEXT PRIMARY KEY,
    current_filename TEXT NOT NULL UNIQUE,   -- profile name (no extension)
    current_path     TEXT NOT NULL,
    game_name        TEXT,
    launch_method    TEXT,
    content_hash     TEXT,
    is_favorite      INTEGER NOT NULL DEFAULT 0,
    source_profile_id TEXT REFERENCES profiles(profile_id),
    source           TEXT,                  -- added v2
    deleted_at       TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE INDEX idx_profiles_current_filename ON profiles(current_filename);
```

**Key fields for library-home:**
- `current_filename` — profile name (React key, IPC name param)
- `is_favorite` — toggled by `profile_set_favorite`; queried by `profile_list_favorites`
- `game_name` — denormalized copy; **not used for cover art** (not authoritative, TOML is)
- `steam_app_id` — **NOT stored in SQLite profiles table**; only in TOML

**Favorites queries:**

```sql
-- set_profile_favorite
UPDATE profiles SET is_favorite = ?1, updated_at = ?2
WHERE current_filename = ?3 AND deleted_at IS NULL;

-- list_favorite_profiles
SELECT current_filename FROM profiles
WHERE is_favorite = 1 AND deleted_at IS NULL
ORDER BY current_filename;
```

Source: `metadata/collections.rs:169–205`

### `game_image_cache` Table (created in migration 13→14)

```sql
CREATE TABLE game_image_cache (
    cache_id         TEXT PRIMARY KEY,           -- random hex blob
    steam_app_id     TEXT NOT NULL,
    image_type       TEXT NOT NULL DEFAULT 'cover',  -- 'cover'|'hero'|'capsule' (string of GameImageType)
    source           TEXT NOT NULL DEFAULT 'steam_cdn', -- 'steam_cdn'|'steamgriddb'
    file_path        TEXT NOT NULL,              -- absolute disk path
    file_size        INTEGER NOT NULL DEFAULT 0,
    content_hash     TEXT NOT NULL DEFAULT '',   -- SHA-256 hex
    mime_type        TEXT NOT NULL DEFAULT 'image/jpeg',
    width            INTEGER,
    height           INTEGER,
    source_url       TEXT NOT NULL DEFAULT '',
    preferred_source TEXT NOT NULL DEFAULT 'auto',
    expires_at       TEXT,                       -- ISO datetime; 24h TTL
    fetched_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_game_image_cache_app_type_source
    ON game_image_cache(steam_app_id, image_type, source);
CREATE INDEX idx_game_image_cache_expires ON game_image_cache(expires_at);
```

**Lookup key:** `(steam_app_id, image_type)` — note: multiple rows possible per app_id (one per source). `get_game_image` returns `LIMIT 1` without `source` filter, so the first matching source wins.

**No SQLite migration needed for Phase 1** — `GameImageType::Portrait` writes to the same table under the key `"portrait"`. The `image_type` column is a free-form text field with no CHECK constraint.

---

## Cover Art Pipeline

### Data Flow

```
useGameCoverArt(steamAppId, customCoverArtPath)
  → customCoverArtPath?.trim() → convertFileSrc(path) [returns immediately, no IPC]
  → invoke('fetch_game_cover_art', { appId, imageType: 'cover' })
    → fetch_game_cover_art command (game_metadata.rs:18)
      → download_and_cache_image(&store, &app_id, image_type, api_key)
        → (a) cache hit: return file_path from game_image_cache
        → (b) SteamGridDB fetch (if api_key present)
        → (c) Steam CDN via build_download_url()
        → (d) stale cache fallback
        → (e) None
      → returns Option<String> (absolute disk path)
    → frontend: convertFileSrc(path) → asset:// URL
```

Source: `game_images/client.rs`, `hooks/useGameCoverArt.ts`

### `GameImageType` Enum

File: `crosshook-core/src/game_images/models.rs:8`

```rust
pub enum GameImageType {
    Cover,    // → "cover"   → https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/header.jpg
    Hero,     // → "hero"    → .../library_hero.jpg
    Capsule,  // → "capsule" → .../capsule_616x353.jpg
}
```

**`Portrait` variant does not yet exist.** Adding it requires:

1. `models.rs` — add `Portrait` variant, update `fmt::Display` and `filename_for`
2. `client.rs:build_download_url` — add Portrait arm:
   ```rust
   GameImageType::Portrait => format!(
       "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
   )
   ```
   With 404 fallback to `library_600x900.jpg` → `header.jpg` (requires multi-attempt logic or secondary URL)
3. `commands/game_metadata.rs:26` — add `"portrait"` match arm

### `build_download_url` (current, private function)

```rust
fn build_download_url(app_id: &str, image_type: GameImageType) -> String {
    match image_type {
        GameImageType::Cover   => ".../header.jpg",         // landscape 460×215
        GameImageType::Hero    => ".../library_hero.jpg",   // 1920×620
        GameImageType::Capsule => ".../capsule_616x353.jpg", // landscape
    }
}
```

**Critical:** Current `Cover` type fetches `header.jpg` (landscape). Library home needs portrait (`library_600x900_2x.jpg`). The feature spec resolves this by adding `Portrait` variant — existing `Cover` behavior is unchanged.

### Portrait URL Fallback Chain

Steam CDN may 404 on `library_600x900_2x.jpg`. The `download_image_bytes` function already returns `Err` on non-2xx. Implementing the fallback chain requires either:
- Multiple sequential `download_image_bytes` calls in `build_download_url` (changes the function signature from pure URL builder to async)
- Or a URL list tried in order (simpler, matches existing retry pattern)

Recommended approach (matches the feature spec decision):
```
library_600x900_2x.jpg → 404 → library_600x900.jpg → 404 → header.jpg
```

### Cache Constants

- **TTL:** 24 hours (`CACHE_TTL_HOURS = 24`, `client.rs:19`)
- **Max size:** 5 MB (`MAX_IMAGE_BYTES = 5 * 1024 * 1024`)
- **Allowed MIME:** `image/jpeg`, `image/png`, `image/webp`
- **Cache base dir:** `~/.local/share/crosshook/cache/images/{app_id}/{type}_{source}.{ext}`
- **Stale fallback:** expired cache entries with existing files are served as fallback

### SteamGridDB Integration

File: `game_images/steamgriddb.rs`

- Fetches via `GET https://www.steamgriddb.com/api/v2/grids/steam/{app_id}` with Bearer auth
- Returns first image URL from `data[0].url`; downloads bytes via `read_limited_response`
- API key sourced from `settings_store.load().steamgriddb_api_key` (optional, non-fatal if missing)
- Already integrated; `Portrait` variant will be passed through to SteamGridDB endpoint automatically (if SteamGridDB is queried first)

---

## Profile Store

### `ProfileStore` Struct

File: `crosshook-core/src/profile/toml_store.rs:12`

```rust
pub struct ProfileStore {
    pub base_path: PathBuf,  // ~/.config/crosshook/profiles/
}
```

Initialized via `ProfileStore::try_new()` using `BaseDirs::config_dir()`.

### `profile_list` Implementation

```rust
pub fn list(&self) -> Result<Vec<String>, ProfileStoreError> {
    fs::create_dir_all(&self.base_path)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(&self.base_path)? {
        // filter *.toml only, strip extension
        names.push(stem.to_string());
    }
    names.sort_unstable();  // alphabetical
    Ok(names)
}
```

- Returns filenames without `.toml` extension (these are the profile "names" used in all IPC calls)
- Sorted alphabetically — same order LibraryGrid will use by default

### `profile_load` + effective_profile

`ProfileStore::load()` applies `effective_profile()` which merges `local_override` fields. The returned `GameProfile` has `steam.app_id` (string), `game.name` (string), and `game.custom_cover_art_path` (Option<String>). These are the three fields `profile_list_summaries` needs to extract.

### Profile TOML Location

- Base: `~/.config/crosshook/profiles/{name}.toml`
- No schema for `steam_app_id` in SQLite — **always read from TOML**

---

## Frontend Integration

### Invoke Pattern (standard)

All IPC calls use `invoke<ReturnType>(commandName, params)` from `@tauri-apps/api/core`. Pattern from `useProfile.ts`:

```typescript
const names = await invoke<string[]>('profile_list');
const profile = await invoke<GameProfile>('profile_load', { name });
await invoke('profile_set_favorite', { name, favorite });
const favorites = await invoke<string[]>('profile_list_favorites');
const path = await invoke<string | null>('fetch_game_cover_art', {
  appId: normalizedAppId,
  imageType: 'cover',
});
```

Error handling pattern: `catch (err) { console.error(...); return fallback; }` — hooks absorb errors and return null/empty rather than throw.

### `useGameCoverArt` Hook

File: `src/crosshook-native/src/hooks/useGameCoverArt.ts`

```typescript
export function useGameCoverArt(
  steamAppId: string | undefined,
  customCoverArtPath?: string,
): UseGameCoverArtResult  // { coverArtUrl: string | null; loading: boolean }
```

- Priority: `customCoverArtPath` > IPC fetch > null
- Custom path: `convertFileSrc(customCoverArtPath)` — immediate, no loading state
- IPC path: `invoke('fetch_game_cover_art', { appId, imageType: 'cover' })`
- Uses `requestIdRef` to cancel stale in-flight requests (race condition guard)
- **For library-home, pass `imageType: 'portrait'`** once the Rust variant is added; the hook currently hardcodes `'cover'`

### `useImageDominantColor` Hook

File: `src/crosshook-native/src/hooks/useImageDominantColor.ts`

```typescript
export function useImageDominantColor(imageUrl: string | null): [number, number, number] | null
```

- Canvas-based; down-samples to 32×32 for performance
- Weighted average favouring top third of image (banner-appropriate color)
- Boosts dark colors (luminance < 80) for visibility on dark backgrounds
- Returns `null` on load failure or null input

### `ProfileContext`

File: `src/crosshook-native/src/context/ProfileContext.tsx`

Wraps `useProfile()` hook. Library page consumes via `useProfileContext()`:

```typescript
const {
  profiles,          // string[] — profile names, alphabetical
  favoriteProfiles,  // string[] — from profile_list_favorites
  selectProfile,     // async (name: string) => void — loads profile via IPC
  refreshProfiles,   // async () => void — re-fetches profile_list
  toggleFavorite,    // async (name: string, favorite: boolean) => void
} = useProfileContext();
```

### `AppRoute` Type

File: `src/crosshook-native/src/components/layout/Sidebar.tsx:13`

```typescript
export type AppRoute = 'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings' | 'health';
```

**`'library'` is not yet in this union.** Must add to:
1. `Sidebar.tsx:13` — type union
2. `Sidebar.tsx:58` — `ROUTE_LABELS` record
3. `Sidebar.tsx` — `SIDEBAR_ITEMS` array (add icon + item)
4. `App.tsx:19` — `VALID_APP_ROUTES` record
5. `App.tsx:43` — change `useState<AppRoute>('profiles')` → `'library'`
6. `ContentArea.tsx` — add `case 'library':` to switch

### Navigation Pattern (`onNavigate`)

Pages with navigation receive `onNavigate?: (route: AppRoute) => void` prop. Examples:
- `InstallPage` (`install-page.tsx:26`): `onNavigate?.('profiles')`
- `HealthDashboardPage` (`health-dashboard-page.tsx:826`): `onNavigate?.('profiles')`

**LibraryPage pattern:**
```typescript
async function handleLaunch(name: string) {
  await selectProfile(name);
  onNavigate?.('launch');
}
async function handleEdit(name: string) {
  await selectProfile(name);
  onNavigate?.('profiles');
}
```

### `contentArea` `forceMount` Behavior

`ContentArea` renders all route content using a tabbed panel. Pages stay mounted when inactive (the `forceMount` pattern from Radix). LibraryPage effects should be gated on `route === 'library'` if they trigger side effects when inactive.

---

## Gotchas & Edge Cases

- **Schema version mismatch**: Feature spec says "schema v13" but `game_image_cache` is created in migration 13→14, making the current schema **v14**. No migration needed for Portrait — `image_type` is free-form text.
- **`get_game_image` returns first source, not best source**: Query uses `LIMIT 1` without ordering by source. If both `steam_cdn` and `steamgriddb` rows exist for the same `(app_id, 'portrait')`, the result is non-deterministic. For portrait art this is unlikely to be an issue but worth noting.
- **`profiles.game_name` vs TOML**: SQLite `game_name` is denormalized and may lag behind TOML edits. Never use it for display — always read from `profile_load` or `profile_list_summaries`.
- **`steam_app_id` not in SQLite profiles table**: Only stored in TOML. `profile_list_summaries` must read TOML files, not the DB, for app_id.
- **`profile_list_favorites` returns current filename**: Matches `profile_list` naming convention; safe to use as React key and IPC param.
- **`toggleFavorite` followed by `loadFavorites` (not optimistic in hook)**: The hook calls `invoke` then re-fetches favorites. Library home must implement its own optimistic UI on top (flip heart immediately, revert on error from the underlying IPC).
- **`profiles-changed` event**: Emitted after `profile_set_favorite`. `useProfile` re-fetches both `profile_list` and `profile_list_favorites` on this event (line 1271). LibraryPage benefits automatically if it uses `useProfileContext`.
- **Portrait URL 404s**: `library_600x900_2x.jpg` returns HTTP 404 for many older titles. `download_image_bytes` calls `error_for_status()` which surfaces as `Err(Network(...))`. The fallback chain must handle this gracefully and try `library_600x900.jpg`, then `header.jpg`.
- **Custom cover art path security**: `convertFileSrc(customCoverArtPath)` passes raw user path to Tauri asset protocol. Tauri's `fs:allow-read-file` scope may block paths outside allowed dirs (feature spec security warning — validate or broker via IPC).
- **`useGameCoverArt` hardcodes `imageType: 'cover'`**: The hook will need a parameter or variant to pass `'portrait'` for the library grid.

---

## Relevant Files

### Rust Backend

- `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs` — `GameImageType` enum (add `Portrait` here)
- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs` — `build_download_url`, `download_and_cache_image`
- `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs` — SteamGridDB client
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` — `ProfileStore`, `list()`, `load()`
- `src/crosshook-native/crates/crosshook-core/src/metadata/game_image_store.rs` — `upsert_game_image`, `get_game_image`
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — schema history (v14 current)
- `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs` — `set_profile_favorite`, `list_favorite_profiles`
- `src/crosshook-native/src-tauri/src/commands/profile.rs` — all `profile_*` commands
- `src/crosshook-native/src-tauri/src/commands/game_metadata.rs` — `fetch_game_cover_art`
- `src/crosshook-native/src-tauri/src/commands/mod.rs` — command module registration
- `src/crosshook-native/src-tauri/src/lib.rs:189` — invoke handler list

### Frontend

- `src/crosshook-native/src/hooks/useGameCoverArt.ts` — cover art hook
- `src/crosshook-native/src/hooks/useImageDominantColor.ts` — dominant color hook
- `src/crosshook-native/src/hooks/useProfile.ts` — all profile IPC calls, favorites, selectProfile
- `src/crosshook-native/src/context/ProfileContext.tsx` — context provider
- `src/crosshook-native/src/components/layout/Sidebar.tsx:13` — `AppRoute` type definition
- `src/crosshook-native/src/components/layout/ContentArea.tsx` — route switch
- `src/crosshook-native/src/App.tsx:19,43` — `VALID_APP_ROUTES`, default route state
- `src/crosshook-native/src/types/profile.ts` — `GameProfile` TypeScript interface

---

## Other Docs

- [Feature Spec](./feature-spec.md) — full feature requirements
- [Steamworks Standard Assets](https://partner.steamgames.com/doc/store/assets/standard) — Steam CDN URL patterns
- [SteamGridDB API v2](https://www.steamgriddb.com/api/v2) — already integrated
