# Research: Integration Points for proton-app-id

## Overview

All primary infrastructure for the proton-app-id feature is already live in the codebase. The `game_images` subsystem in `crosshook-core` handles Steam CDN and SteamGridDB downloads, caches results in a dedicated SQLite table (`game_image_cache`, schema v14), and exposes two Tauri IPC commands (`fetch_game_cover_art`, `import_custom_cover_art`). The implementation work is additive: add `steam_app_id` to `RuntimeSection`, add two new custom art path fields to `GameSection`, add `GameImageType::Background`, generalize `import_custom_cover_art` to accept an art-type parameter, and wire the frontend.

---

## API Endpoints (Tauri IPC Commands)

### Registered commands relevant to this feature

| Command | File | Signature |
|---------|------|-----------|
| `fetch_game_cover_art` | `src-tauri/src/commands/game_metadata.rs:20` | `async fn(app_id: String, image_type: Option<String>, ...) -> Result<Option<String>, String>` |
| `import_custom_cover_art` | `src-tauri/src/commands/game_metadata.rs:46` | `fn(source_path: String) -> Result<String, String>` |
| `profile_list_summaries` | `src-tauri/src/commands/profile.rs:242` | `fn(store) -> Result<Vec<ProfileSummary>, String>` |
| `profile_save` | `src-tauri/src/commands/profile.rs:270` | `fn(name, data: GameProfile, ...) -> Result<(), String>` |
| `settings_load` | `src-tauri/src/commands/settings.rs:16` | `fn(store) -> Result<AppSettingsData, String>` |
| `fetch_game_metadata` | `src-tauri/src/commands/game_metadata.rs:9` | `async fn(app_id, force_refresh, ...) -> Result<SteamMetadataLookupResult, String>` |

### `fetch_game_cover_art` — routing logic

Located at `src-tauri/src/commands/game_metadata.rs:27`, `image_type` dispatch:

```rust
match image_type.as_deref().unwrap_or("cover") {
    "hero"     => GameImageType::Hero,
    "capsule"  => GameImageType::Capsule,
    "portrait" => GameImageType::Portrait,
    _          => GameImageType::Cover,  // default + unknown strings
}
```

`"background"` is **not yet** an arm — it silently falls to `Cover`. This is the S-05 security warning in `research-security.md`. Adding the arm is one of the required Phase 3 changes.

### `ProfileSummary` DTO — current shape

```rust
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,          // currently steam.app_id only
    pub custom_cover_art_path: Option<String>,
}
```

`steam_app_id` today reflects only `steam.app_id`. For Phase 1, this field must be updated to the result of `resolve_art_app_id()` (BR-9: `steam.app_id` first, then `runtime.steam_app_id`).

### `settings_load` — API key exposure (security gap S-02)

`settings_load` returns the full `AppSettingsData` struct, which includes `steamgriddb_api_key: Option<String>` in plaintext. This exposes the raw API key to the frontend. The feature spec requires this to be changed to `has_steamgriddb_api_key: bool` before shipping SGDB integration.

### Command registration

All commands registered in `src-tauri/src/lib.rs:196-290`. New commands (`import_custom_art`) must be added to the `invoke_handler!` macro list there.

---

## Database Schema (game_image_cache and related tables)

### `game_image_cache` table — schema v14

Created in migration 13→14 (`metadata/migrations.rs:642`):

```sql
CREATE TABLE game_image_cache (
    cache_id         TEXT PRIMARY KEY,           -- randomblob(16) hex
    steam_app_id     TEXT NOT NULL,              -- e.g. "1245620"
    image_type       TEXT NOT NULL DEFAULT 'cover', -- "cover"|"hero"|"portrait"|"capsule"|"background"
    source           TEXT NOT NULL DEFAULT 'steam_cdn', -- "steam_cdn"|"steamgriddb"
    file_path        TEXT NOT NULL,              -- absolute local path
    file_size        INTEGER NOT NULL DEFAULT 0,
    content_hash     TEXT NOT NULL DEFAULT '',   -- SHA-256 hex
    mime_type        TEXT NOT NULL DEFAULT 'image/jpeg',
    width            INTEGER,                    -- NULL unless explicitly set
    height           INTEGER,                    -- NULL unless explicitly set
    source_url       TEXT NOT NULL DEFAULT '',   -- origin URL
    preferred_source TEXT NOT NULL DEFAULT 'auto',
    expires_at       TEXT,                       -- NULL = never expires; RFC3339 or "YYYY-MM-DDTHH:MM:SS"
    fetched_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_game_image_cache_app_type_source
    ON game_image_cache(steam_app_id, image_type, source);
CREATE INDEX idx_game_image_cache_expires ON game_image_cache(expires_at);
```

**Key constraint**: `(steam_app_id, image_type, source)` is the natural primary key for upserts. One row per app_id + type + source combination.

**`"background"` type fits without migration**: `image_type TEXT` accepts any string. Inserting `"background"` alongside `"cover"`, `"portrait"`, `"hero"` requires no schema change.

### SQLite access pattern

`MetadataStore` is the façade over the SQLite connection. It delegates to free functions in `metadata/game_image_store.rs`:

- `upsert_game_image(conn, steam_app_id, image_type, source, file_path, ...)` — ON CONFLICT upserts the row
- `get_game_image(conn, steam_app_id, image_type)` — fetches first match (LIMIT 1); returns `Option<GameImageCacheRow>`
- `evict_expired_images(conn)` — deletes rows where `expires_at < now()`, returns deleted `file_path`s for disk cleanup

`MetadataStore` exposes public wrappers: `store.get_game_image(app_id, type)`, `store.upsert_game_image(...)`, and `store.with_sqlite_conn("label", |conn| ...)` for raw operations (used by the stale-row delete in `client.rs:484`).

### `GameImageCacheRow` Rust struct

Defined at `metadata/game_image_store.rs:5`. All fields mapped directly from the SQL columns above.

---

## External Services (Steam CDN, SteamGridDB, reqwest)

### Steam CDN

- **No authentication** required
- Base: `https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/`
- Implemented in `client.rs:354` (`build_download_url`):

| `GameImageType` | CDN file |
|----------------|----------|
| `Cover` | `header.jpg` |
| `Hero` | `library_hero.jpg` |
| `Capsule` | `capsule_616x353.jpg` |
| `Portrait` | `library_600x900_2x.jpg` (with 3-URL fallback chain) |
| `Background` (new) | `library_hero.jpg` — same as `Hero` |

- Portrait has a 3-URL fallback chain at `client.rs:379` (`portrait_candidate_urls`): `_2x.jpg` → `library_600x900.jpg` → `header.jpg`

### SteamGridDB API

- **Bearer auth required**: `Authorization: Bearer <api_key>`
- API key stored in `settings.toml` as `steamgriddb_api_key` (plaintext — S-15 advisory)
- Key is read in `fetch_game_cover_art` via `settings_store.load().ok().and_then(|s| s.steamgriddb_api_key)` (non-fatal on load failure)
- API key excluded from tracing via `#[tracing::instrument(skip(api_key))]` in `steamgriddb.rs:38`
- Implemented in `steamgriddb.rs:100` (`build_endpoint`):

| `GameImageType` | SGDB path | Dimensions filter |
|----------------|-----------|-------------------|
| `Cover` | `grids/steam/{app_id}` | `460x215,920x430` |
| `Hero` | `heroes/steam/{app_id}` | none |
| `Capsule` | `grids/steam/{app_id}` | `342x482,600x900` |
| `Portrait` | `grids/steam/{app_id}` | `342x482,600x900` |
| `Background` (new) | `heroes/steam/{app_id}` | none — same as `Hero` |

- Response shape: `{ success: bool, data: [{ url: String, ... }] }` — only `url` is used; first item wins
- Image is downloaded from `cdn2.steamgriddb.com` (the URL in `data[0].url`)

### reqwest HTTP client

Singleton `OnceLock<reqwest::Client>` at `client.rs:22` (`GAME_IMAGES_HTTP_CLIENT`):

- Timeout: 15 seconds
- User-Agent: `CrossHook/{CARGO_PKG_VERSION}`
- TLS: `rustls-tls` (webpki-roots)
- **No redirect policy configured** — security gap S-01/S-06 requires adding `.redirect(Policy::custom(...))` restricting to 4 allowed domains (CDN + SGDB) over HTTPS only
- 5 MB streaming cap enforced chunk-by-chunk in `read_limited_response` (`client.rs:440`)
- Magic-byte MIME validation via `infer` crate: allow-list is `["image/jpeg", "image/png", "image/webp"]`

### Download fallback order

With `api_key = Some(key)`: SteamGridDB → Steam CDN → stale cache → `None`

With `api_key = None`: Steam CDN → stale cache → `None`

The switch happens at `client.rs:199`. Portrait type has special handling that calls `try_portrait_candidates` instead of the single CDN URL.

---

## Internal Services (Profile System, Image Pipeline, Cache)

### Profile loading and saving

`ProfileStore` at `crosshook-core/src/profile/toml_store.rs`. Files stored as TOML at `~/.local/share/crosshook/profiles/{name}.toml`.

- `store.load(name)` → `GameProfile`
- `store.save(name, &profile)` → persists TOML
- `profile.effective_profile()` → merges `local_override.*` fields over base fields (machine-specific paths take precedence)
- `profile.storage_profile()` → moves machine paths into `local_override`, clears base portable fields
- `profile.portable_profile()` → calls `storage_profile()`, then clears all `local_override` fields

**`profile_save` auto-import** (`profile.rs:279`): At save time, if `game.custom_cover_art_path` is non-empty and not already in the managed media directory, it is automatically imported via `import_custom_cover_art`. The generalized `import_custom_art` command will need to extend this pattern to portrait and background.

### Custom art import pipeline

`game_images/import.rs`. Current function: `import_custom_cover_art(source_path: &str) -> Result<String, String>`.

- Reads file from disk
- Validates via `validate_image_bytes` (magic bytes, 5 MB limit, MIME allow-list)
- Destination: `~/.local/share/crosshook/media/covers/{sha256_16_chars}.{ext}`
- Content-addressed: idempotent — same file re-uploaded returns same path
- `is_in_managed_media_dir(path)` helper checks if a path is already under `~/.local/share/crosshook/media/`

The generalized `import_custom_art(source_path, art_type)` will route to type-segregated subdirectories: `media/covers/`, `media/portraits/`, `media/backgrounds/`.

### Image cache management

Cache base directory: `~/.local/share/crosshook/cache/images/{app_id}/{filename}`

Filename pattern: `{type_prefix}_{source_suffix}.{extension}` (e.g. `portrait_steam_cdn.jpg`, `cover_steamgriddb.webp`). Defined in `client.rs:387` (`filename_for`).

TTL: 24 hours from fetch time. Stale entries are served as fallback but expired rows are eventually evicted. If a cached file is missing from disk (e.g. cleaned externally), the DB row is deleted immediately and `None` is returned.

### Community export sanitization

`exchange.rs:257` (`sanitize_profile_for_community_export`) calls `profile.portable_profile()` then additionally clears DLL paths, icon path, and runtime paths. Custom art paths are cleared by `portable_profile()` → `storage_profile()` which moves them to `local_override` — then `portable_profile()` clears all `local_override`.

**Note**: The feature spec (S-03) flags that `custom_cover_art_path` currently survives community export in a subtle way. The `sanitize_profile_for_community_export` function should explicitly call `.clear()` on all custom art path fields for safety. New `custom_portrait_art_path` and `custom_background_art_path` must also be cleared here.

---

## Configuration (TOML Profile Fields, local_override)

### Current TOML profile structure (relevant sections)

```toml
[game]
name = "Elden Ring"
executable_path = "/path/to/eldenring.exe"
custom_cover_art_path = ""          # machine-local (stored in local_override)

[steam]
enabled = false
app_id = "1245620"                   # used for both launch + art today

[runtime]
prefix_path = "/path/to/prefix"
proton_path = "/path/to/proton"
working_directory = ""
# steam_app_id not yet present — MUST ADD

[local_override.game]
executable_path = "/path/to/eldenring.exe"
custom_cover_art_path = "/home/user/.local/share/crosshook/media/covers/abc123def.jpg"
```

### Fields to add

**`RuntimeSection`** (`profile/models.rs:261`):
```rust
pub struct RuntimeSection {
    pub prefix_path: String,
    pub proton_path: String,
    pub working_directory: String,
    // ADD:
    pub steam_app_id: String,   // skip_serializing_if = "String::is_empty"
}
```

`is_empty()` at `models.rs:271` currently returns `true` when all three fields are empty. The new field must be **excluded** from `is_empty()` — a profile with only `steam_app_id` set (and no paths) must still serialize the `[runtime]` section.

**`GameSection`** (`profile/models.rs:187`):
```rust
pub struct GameSection {
    pub name: String,
    pub executable_path: String,
    pub custom_cover_art_path: String,
    // ADD:
    pub custom_portrait_art_path: String,    // skip_serializing_if = "String::is_empty"
    pub custom_background_art_path: String,  // skip_serializing_if = "String::is_empty"
}
```

**`LocalOverrideGameSection`** (`profile/models.rs:353`):
```rust
pub struct LocalOverrideGameSection {
    pub executable_path: String,
    pub custom_cover_art_path: String,
    // ADD (same pattern as cover):
    pub custom_portrait_art_path: String,
    pub custom_background_art_path: String,
}
```

`is_empty()` at `models.rs:361` must also include the new fields.

### `effective_profile()` / `storage_profile()` propagation

Both methods at `profile/models.rs:410` and `440` must be extended for the two new custom art path fields, following the identical pattern used for `custom_cover_art_path`.

### App settings TOML (SteamGridDB key)

Located at `~/.config/crosshook/settings.toml`. The `steamgriddb_api_key` field is an `Option<String>` in `AppSettingsData` (`settings/mod.rs:27`). It is stored in plaintext (S-15 advisory — acceptable for single-user desktop, document `chmod 600`).

### `resolve_art_app_id` helper (to add)

```rust
pub fn resolve_art_app_id(profile: &GameProfile) -> &str {
    let steam = profile.steam.app_id.trim();
    if !steam.is_empty() { return steam; }
    profile.runtime.steam_app_id.trim()
}
```

Called from `profile_list_summaries` to populate the `steam_app_id` field in `ProfileSummary`, and from any future frontend-facing IPC that needs the effective media app_id.

---

## Gotchas and Edge Cases

- **`image_type` unknown string defaults to Cover silently** (S-05): `fetch_game_cover_art` uses `_ => GameImageType::Cover` as catch-all. After adding `"background"`, callers passing a typo still silently get Cover. The spec recommends returning a typed error for unrecognized `image_type` values.

- **`download_url` field records grid endpoint for Hero/Portrait** (`client.rs:309-312`): The `source_url` stored in `game_image_cache` always records `/grids/steam/{id}` for SGDB sources, regardless of actual image type. For Hero images, the correct endpoint would be `/heroes/steam/{id}`. Minor data quality issue; no functional impact.

- **`is_empty()` on `RuntimeSection` must exclude `steam_app_id`**: If the check includes `steam_app_id`, a proton_run profile with only the Steam App ID set (and no prefix/proton paths yet) would skip serializing the `[runtime]` section, silently losing the field.

- **`settings_load` exposes raw SGDB API key** (S-02): Currently `AppSettingsData` is serialized as-is to the frontend. Before shipping SGDB features, either filter the key at the IPC boundary or return a `has_steamgriddb_api_key: bool` field.

- **Community export leaks local art paths** (S-03): `sanitize_profile_for_community_export` relies on `portable_profile()` to clear `local_override`, but the base `game.custom_cover_art_path` field can hold a value (it is cleared by `storage_profile()` only when also populated in `local_override`). An explicit `.clear()` after `portable_profile()` is safer.

- **Portrait CDN fallback chain is type-specific** (`client.rs:213`): The 3-URL portrait candidate chain is only triggered when `image_type == GameImageType::Portrait`. The Background type must use a single CDN URL (`library_hero.jpg`) — no fallback chain needed for background.

- **LibraryCard uses `profile.steamAppId` from `ProfileSummary`** (`LibraryCard.tsx:46`): If `steamAppId` is empty (proton_run profiles today), `useGameCoverArt` bails early and no art fetch occurs. Phase 1 is specifically to fix this by resolving the effective app_id in `profile_list_summaries`.

- **`useGameCoverArt` null gate for custom art** (`useGameCoverArt.ts:31`): When `customCoverArtPath` is set but `steamAppId` is empty, the hook correctly returns the custom URL. However if `steamAppId` is empty AND `customCoverArtPath` is also empty, the hook immediately returns `null` without attempting a fetch. This is the existing behavior — correct but worth noting for proton_run profiles.

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/client.rs` — HTTP singleton, CDN URL construction, fallback chain, cache write/read, all validation
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs` — SteamGridDB fetch, endpoint builder
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/import.rs` — custom art import (currently cover-only)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/models.rs` — `GameImageType`, `GameImageSource`, `GameImageError`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/game_image_store.rs` — SQLite CRUD for `game_image_cache`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — migration 13→14 defines the `game_image_cache` table schema
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile`, `RuntimeSection`, `GameSection`, `LocalOverrideGameSection`, `effective_profile()`, `storage_profile()`, `portable_profile()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs` — `sanitize_profile_for_community_export`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData` with `steamgriddb_api_key`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/game_metadata.rs` — `fetch_game_cover_art`, `import_custom_cover_art` Tauri commands
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs` — `profile_save` (auto-import at save), `profile_list_summaries` (ProfileSummary DTO)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/settings.rs` — `settings_load` exposes raw API key
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — Tauri command registration (invoke_handler!)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGameCoverArt.ts` — frontend hook; resolves custom path or invokes `fetch_game_cover_art`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts` — TypeScript `GameProfile` interface (needs `runtime.steam_app_id`, new custom art path fields)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/library.ts` — `LibraryCardData` interface (has `steamAppId`)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/profile-sections/MediaSection.tsx` — current cover-only art upload UI
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx` — proton_run section (has "Steam App ID" field bound to `steam.app_id` — must rebind to `runtime.steam_app_id`)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/library/LibraryCard.tsx` — calls `useGameCoverArt` with `profile.steamAppId` and `'portrait'` type

---

## Other Docs

- [feature-spec.md](./feature-spec.md) — authoritative feature specification, phasing, and decisions
- [research-external.md](./research-external.md) — Steam CDN and SteamGridDB API reference, codebase reality section (§11)
- [research-security.md](./research-security.md) — S-01 through S-15 security findings with required mitigations
- [research-technical.md](./research-technical.md) — data model specs, Rust struct changes, API design decisions
