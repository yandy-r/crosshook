# Architecture Research: proton-app-id

## System Overview

CrossHook is a native Linux Tauri v2 desktop app (AppImage). The stack is layered: `crosshook-core` (Rust) holds all business logic; `src-tauri` is a thin IPC layer exposing `#[tauri::command]` handlers; the frontend is React/TypeScript communicating exclusively via `invoke()`. Profile data is persisted as TOML files on disk; operational metadata (including art cache) lives in a SQLite database via `crosshook-core/src/metadata/`.

---

## Relevant Components

### Rust — crosshook-core

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: Defines `GameProfile`, `RuntimeSection`, `GameSection`, `LocalOverrideSection`, and all TOML-mapped structs. Contains `effective_profile()`, `storage_profile()`, `portable_profile()`, and `resolve_launch_method()`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/models.rs`: `GameImageType` enum (`Cover`, `Hero`, `Capsule`, `Portrait`), `GameImageSource`, `GameImageError`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`: `download_and_cache_image()` — the main image download+cache pipeline. Handles TTL, stale fallback, Steam CDN and SteamGridDB, safe path construction, magic-byte validation.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/import.rs`: `import_custom_cover_art()` — copies user-selected art file into `~/.local/share/crosshook/media/covers/` with content-addressed naming.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs`: `fetch_steamgriddb_image()` + `build_endpoint()` — SteamGridDB API client. Maps `GameImageType` variants to API path/query.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs`: Public re-exports for the `game_images` module.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/game_image_store.rs`: SQLite helpers for `game_image_cache` table: `upsert_game_image()`, `get_game_image()`, `evict_expired_images()`. Uses `(steam_app_id, image_type, source)` as the unique key.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`: Community export/import. `sanitize_profile_for_community_export()` calls `portable_profile()` then clears specific machine paths — currently does **not** clear `custom_cover_art_path` (security finding S-03).

### Rust — src-tauri (IPC layer)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/game_metadata.rs`: Tauri commands `fetch_game_cover_art`, `import_custom_cover_art`. `fetch_game_cover_art` dispatches to `download_and_cache_image`; currently matches `"hero"`, `"capsule"`, `"portrait"`, defaulting all others to `Cover`. `import_custom_cover_art` is a thin wrapper around core.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`: `profile_list_summaries` builds `ProfileSummary { name, game_name, steam_app_id, custom_cover_art_path }` — currently sets `steam_app_id` from `effective.steam.app_id` only. `profile_save` auto-imports cover art from unmanaged paths.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: `invoke_handler` registration for all Tauri commands.

### TypeScript / React — Frontend

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts`: `GameProfile` TS type. `runtime` section currently has `{ prefix_path, proton_path, working_directory }` — no `steam_app_id` yet. `game` has `custom_cover_art_path?: string` (cover only).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/library.ts`: `LibraryCardData { name, gameName, steamAppId, customCoverArtPath?, isFavorite }`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGameCoverArt.ts`: Stateful hook wrapping `fetch_game_cover_art` invoke. Accepts `(steamAppId, customCoverArtPath?, imageType?)`. Returns custom URL immediately if present; otherwise fetches via IPC. **Current bug**: if `steamAppId` is empty/undefined it sets `coverArtUrl = null` and returns — even when `customCoverArtPath` exists, the custom URL is served via `customUrl ?? coverArtUrl`, so the bug is that `null` app ID prevents the IPC call, which is correct. However, the hook skips the IPC call entirely when `customUrl` is truthy (correct behavior).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useLibrarySummaries.ts`: Calls `profile_list_summaries`; maps `steamAppId` and `customCoverArtPath` to `LibraryCardData`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/library/LibraryCard.tsx`: Renders portrait art card. Uses `IntersectionObserver` for lazy loading; calls `useGameCoverArt(visible ? profile.steamAppId : undefined, profile.customCoverArtPath, 'portrait')`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/library/GameCoverArt.tsx`: Smaller art component used outside the grid.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`: Renders `proton_run` App ID field bound to `profile.steam.app_id` (line ~188). This is the field the feature spec says must be **rebound** to `profile.runtime.steam_app_id`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/profile-sections/MediaSection.tsx`: Single "Custom Cover Art" slot; calls `import_custom_cover_art` IPC. Must be expanded to three art slots.

---

## Data Flow

### Library Grid Art Resolution (current)

```
LibraryCard (IntersectionObserver visible)
  -> useGameCoverArt(steamAppId, customCoverArtPath, "portrait")
    -> if customCoverArtPath: return convertFileSrc(customCoverArtPath)
    -> else if steamAppId: invoke("fetch_game_cover_art", { appId, imageType })
         -> game_metadata.rs: match image_type -> GameImageType::Portrait
         -> download_and_cache_image(store, app_id, Portrait, api_key)
              -> check game_image_cache (SQLite) for non-expired row
              -> if miss: SteamGridDB (if key) -> Steam CDN -> stale cache -> None
              -> write file to ~/.local/share/crosshook/cache/images/{app_id}/portrait_*.jpg
              -> upsert game_image_cache row
              -> return absolute path
         -> frontend: convertFileSrc(path) -> tauri asset protocol URL
```

### Profile Save (custom art auto-import)

```
profile_save IPC
  -> if custom_cover_art_path outside media dir:
       import_custom_cover_art(path)
         -> validate magic bytes + size
         -> sha256_hex(bytes) -> content-addressed filename
         -> write to ~/.local/share/crosshook/media/covers/{hash[..16]}.{ext}
         -> return absolute path
  -> store.save(name, &data) -> write TOML
  -> metadata_store.observe_profile_write(...)
```

### Art Source Priority (per BR-1)

Custom art path (if set) → auto-downloaded art (via effective app_id) → stale cache → placeholder/initials.  
The `useGameCoverArt` hook short-circuits at step 1 (custom URL returned immediately, no IPC call). Steps 2-4 happen inside `download_and_cache_image`.

---

## Integration Points

### Where `steam_app_id` plugs in (Phase 1)

1. **`RuntimeSection` struct** (`models.rs:262`): Add `steam_app_id: String` field with `#[serde(rename = "steam_app_id", default, skip_serializing_if = "String::is_empty")]`. Update `is_empty()` at line 272 — must **not** check `steam_app_id` alone (a profile with only `steam_app_id` set should still emit the `[runtime]` section).
2. **`resolve_art_app_id()` helper** (new function in `models.rs`): Returns `steam.app_id` if non-empty, else `runtime.steam_app_id`.
3. **`profile_list_summaries`** (`profile.rs:242`): Replace `steam_app_id: effective.steam.app_id.clone()` with the result of `resolve_art_app_id(&effective)`.
4. **`RuntimeSection.tsx`** (line ~188): Rebind the `proton_run` "Steam App ID" `FieldRow` from `profile.steam.app_id` to `profile.runtime.steam_app_id`.
5. **`GameProfile` type** (`profile.ts:119`): Add `steam_app_id?: string` to `runtime` interface.

### Where tri-art custom upload plugs in (Phase 2)

1. **`GameSection` struct** (`models.rs:188`): Add `custom_portrait_art_path` and `custom_background_art_path` with `skip_serializing_if = "String::is_empty"`.
2. **`LocalOverrideGameSection`** (`models.rs:353`): Mirror the new fields; update `is_empty()`.
3. **`effective_profile()` / `storage_profile()` / `portable_profile()`** (`models.rs:410-468`): Add merge/clear/blank logic for each new art path field — follow the exact existing pattern for `custom_cover_art_path`.
4. **`import_custom_cover_art` → `import_custom_art(source_path, art_type)`** (`import.rs`): Add `art_type` parameter routing to `media/covers/`, `media/portraits/`, `media/backgrounds/` subdirs.
5. **`game_metadata.rs` / `profile.rs`**: Add `import_custom_art` Tauri command; update `profile_save` to auto-import all three art types.
6. **`MediaSection.tsx`**: Expand to three `FieldRow` slots (Cover, Portrait, Background).
7. **`profile.ts` `GameProfile`**: Add `custom_portrait_art_path?` and `custom_background_art_path?` to `game` interface and `local_override.game`.
8. **`sanitize_profile_for_community_export`** (`exchange.rs:257`): Add explicit clears for all three custom art paths (security S-03).

### Where `Background` image type plugs in (Phase 3)

1. **`GameImageType` enum** (`models.rs`): Add `Background` variant.
2. **`build_download_url()`** (`client.rs:354`): Add `Background => library_hero.jpg`.
3. **`filename_for()`** (`client.rs:387`): Add `Background => "background"` prefix.
4. **`build_endpoint()`** (`steamgriddb.rs:100`): Add `Background => ("heroes", None)`.
5. **`fetch_game_cover_art` IPC** (`game_metadata.rs:27`): Add `"background" => GameImageType::Background` arm.
6. **Frontend**: Build UI consumers (profile detail backdrop, launch page hero).

---

## Key Dependencies

### Rust crates (already present — no new deps)

| Crate | Role |
|---|---|
| `reqwest` (0.12, rustls-tls) | HTTP client singleton for CDN + SteamGridDB downloads |
| `infer` (~0.16) | Magic-byte MIME detection for image validation |
| `sha2` (0.11) | Content-addressed import filenames |
| `rusqlite` | SQLite access for `game_image_cache` and all metadata |
| `serde` / `toml` | Profile TOML serialization/deserialization |
| `directories` | Resolves `~/.local/share/crosshook/` on Linux |
| `tauri` v2 | IPC boundary, asset protocol for serving local files |

### External services

| Service | Auth | Used for |
|---|---|---|
| Steam CDN (`cdn.cloudflare.steamstatic.com`) | None | Cover, Portrait (3-URL fallback), Hero/Background |
| SteamGridDB API (`www.steamgriddb.com/api/v2`) | Bearer token (optional) | Cover, Portrait, Hero/Background |

### Internal module dependencies for the feature

```
src-tauri/commands/game_metadata.rs
  -> crosshook-core::game_images::{download_and_cache_image, import_custom_cover_art, GameImageType}
  -> crosshook-core::metadata::MetadataStore
  -> crosshook-core::settings::SettingsStore

src-tauri/commands/profile.rs
  -> crosshook-core::game_images::{import_custom_cover_art, is_in_managed_media_dir}
  -> crosshook-core::profile::{GameProfile, ProfileStore, effective_profile()}

crosshook-core::game_images::client
  -> crosshook-core::metadata (game_image_store functions via MetadataStore)
  -> crosshook-core::game_images::steamgriddb

Frontend hooks/components:
  useLibrarySummaries -> profile_list_summaries IPC
  useGameCoverArt -> fetch_game_cover_art IPC
  LibraryCard -> useGameCoverArt
  MediaSection -> import_custom_cover_art IPC
  RuntimeSection -> profile data binding (steam_app_id field)
```

---

## Gotchas and Edge Cases

- **`RuntimeSection::is_empty()` must NOT gate on `steam_app_id` alone**: The current guard (`prefix_path.is_empty() && proton_path.is_empty() && working_directory.is_empty()`) omits `steam_app_id`. The spec says a profile with only `steam_app_id` set must still emit the `[runtime]` section — this is already the desired behavior since `is_empty()` does not check the new field. Do NOT add `&& steam_app_id.is_empty()` to the guard.

- **`useGameCoverArt` null gate**: When `steamAppId` is falsy, the hook returns `null` immediately even if `customCoverArtPath` is set. However, the hook's return value is `customUrl ?? coverArtUrl` — so if `customUrl` is non-null, it is always returned. The null gate only prevents the IPC fetch, which is correct. The "fix" mentioned in the feature spec (Phase 1) is that the component using the hook must pass the effective app_id (resolved backend-side), not just `steam.app_id`.

- **`profile_list_summaries` only resolves `steam.app_id`**: The `steamAppId` field sent to the frontend is currently `effective.steam.app_id`, which is always empty for `proton_run` profiles. Phase 1 requires calling `resolve_art_app_id()` here to include `runtime.steam_app_id` in the resolution.

- **`game_image_cache` unique key is `(steam_app_id, image_type, source)`**: Adding `"background"` as a new `image_type` string works without migration because `image_type` is TEXT and the conflict key already accommodates any string. No SQL migration needed.

- **`sanitize_profile_for_community_export` does not clear `custom_cover_art_path`** (security S-03): `portable_profile()` clears local override fields and the base `game.custom_cover_art_path` via `storage_profile()`. But the implementation in `exchange.rs:257` calls `portable_profile()` and then manually clears `dll_paths`, `launcher.icon_path`, `runtime.proton_path`, `runtime.working_directory` — it does not clear `custom_cover_art_path`. If `custom_cover_art_path` was stored in the base `game` section (not local_override), it survives the export. All three custom art paths must be explicitly cleared after the `portable_profile()` call.

- **`proton_run` "Steam App ID" in `RuntimeSection.tsx` is currently bound to `steam.app_id`** (line ~188): This field must be rebound to `runtime.steam_app_id`. The `steam_applaunch` "Steam App ID" field (line ~60) must remain bound to `steam.app_id` — they are different fields for different launch methods.

- **`import_custom_cover_art` destination is hardcoded to `media/covers/`**: Generalizing to `import_custom_art(source_path, art_type)` requires routing to `media/portraits/` and `media/backgrounds/` based on the type parameter. The type must be validated against a closed set to prevent directory traversal.

- **`profile_save` auto-import currently covers only `custom_cover_art_path`**: When Phase 2 adds portrait and background art paths, `profile_save` must auto-import all three art types — following the existing pattern but using the generalized `import_custom_art(path, type)`.

- **`GameCoverArt.tsx` component in `library/` is separate from `LibraryCard`**: `GameCoverArt.tsx` (894 bytes) appears to be a standalone component. Verify it also uses `useGameCoverArt` and will benefit from the resolved app_id without separate changes.
