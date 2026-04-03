# Pattern Research: proton-app-id

## Overview

This document catalogs the concrete code patterns in use across `crosshook-core`, `src-tauri`, and the React frontend that are directly relevant to implementing the proton-app-id and tri-art features. Each section includes the exact file locations developers must touch or mirror.

---

## Architectural Patterns

**Thin Tauri Commands, Logic in Core**
All non-trivial backend logic lives in `crosshook-core`. Tauri command handlers in `src-tauri/src/commands/` are thin: they resolve managed `State<'_>` handles, call a core function, and map errors to `String`. The handler itself almost never does computation.

- Example: `src/crosshook-native/src-tauri/src/commands/game_metadata.rs:20-48`
- Pattern: `State<'_, MetadataStore>` injected as parameter, `download_and_cache_image(&store, ...)` does all work

**Profile Section Structs with `is_empty()` Gate**
Every optional section on `GameProfile` (e.g. `RuntimeSection`, `LocalOverrideSection`) implements `is_empty()` and is annotated `#[serde(skip_serializing_if = "…::is_empty")]`. This keeps TOML files minimal — sections are omitted entirely when empty.

- `RuntimeSection::is_empty()` at `src/crosshook-native/crates/crosshook-core/src/profile/models.rs:272-277`
- New `steam_app_id` field must be checked in `is_empty()` — a profile with only `steam_app_id` set must still emit `[runtime]`

**Three-Profile Representation: effective / storage / portable**
`GameProfile` has three derived views, each computed by dedicated methods:
- `effective_profile()` — merges `local_override.*` into base fields (used at launch and in summaries)
- `storage_profile()` — moves machine-local paths into `local_override` and clears base fields (written to disk)
- `portable_profile()` — calls `storage_profile()` then resets `local_override` entirely (used for community export)

All three are in `src/crosshook-native/crates/crosshook-core/src/profile/models.rs:407-468`.

New `custom_portrait_art_path` and `custom_background_art_path` must be wired in all three methods, following the exact same pattern as `custom_cover_art_path`.

**`sanitize_profile_for_community_export` function**
Community export calls `portable_profile()` then additionally clears DLL paths and icon paths. New custom art fields are machine-local paths and must be cleared here too.

- `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs:257-261`

**`GameImageType` Enum + `Display` impl**
The enum in `game_images/models.rs` drives both the cache DB `image_type TEXT` column and the `build_download_url` / `filename_for` / `build_endpoint` match arms. Adding `Background` requires updating all four match sites.

- Enum definition: `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs:8-13`
- `build_download_url`: `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:354-377`
- `filename_for`: `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:387-399`
- `build_endpoint` (SteamGridDB): `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs:100-116`

**`ProfileSummary` DTO with `#[serde(rename_all = "camelCase")]`**
The IPC summary struct is defined in the commands layer (not in core) and uses `rename_all = "camelCase"` so field names match TypeScript. Core types use `snake_case`; the DTO layer translates.

- `src/crosshook-native/src-tauri/src/commands/profile.rs:232-239`
- Frontend mirrors at `src/crosshook-native/src/hooks/useLibrarySummaries.ts:6-11`

---

## Code Conventions

**Rust: `snake_case`, modules as `mod.rs` files**
`game_images/mod.rs` re-exports the public surface; individual modules (`client.rs`, `models.rs`, `import.rs`, `steamgriddb.rs`) are internal.

- `src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs`

**Rust: `#[serde(rename = "…", default, skip_serializing_if = "String::is_empty")]`**
All new TOML profile fields follow this three-attribute pattern. `default` enables backward-compatible deserialization (old profiles without the field just get `String::new()`). `skip_serializing_if = "String::is_empty"` keeps TOML minimal.

- Canonical example: `custom_cover_art_path` at `src/crosshook-native/crates/crosshook-core/src/profile/models.rs:193-195`

**Rust: Error types as custom enums, not `anyhow`**
`game_images` uses a bespoke `GameImageError` enum with `From` impls for `std::io::Error` and `reqwest::Error`. This is the established pattern for the module; the feature must extend it (e.g. new `AuthFailure` variant for SteamGridDB 401/403).

- `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs:44-106`

**Rust: Tauri commands use `async fn` only when needed**
`fetch_game_cover_art` is `async` (network I/O). `import_custom_cover_art` is sync (file I/O is blocking but fast). Follow this split: `import_custom_art` stays sync; any future SteamGridDB fetch commands are `async`.

**TypeScript: Interfaces mirroring Rust serde output**
Frontend types in `src/crosshook-native/src/types/profile.ts` mirror the Rust `GameProfile` field structure verbatim. New fields on `RuntimeSection` and `GameSection` (and `local_override.game`) must be added here.

- `src/crosshook-native/src/types/profile.ts:92-153`
- `local_override.game` must also gain `custom_portrait_art_path` and `custom_background_art_path` as optional fields

**TypeScript: React hooks wrap `invoke()` for stateful data**
`useGameCoverArt` wraps `fetch_game_cover_art` with stale-while-revalidate behavior, request race protection (`requestIdRef`), and custom-art short-circuit. New art types use the same hook; the `imageType` parameter already accepts arbitrary strings.

- `src/crosshook-native/src/hooks/useGameCoverArt.ts`

**TypeScript: `convertFileSrc` for local file paths**
Local filesystem paths from the backend (cache dir or media dir) must be converted with `convertFileSrc` before use in `<img src>`. The hook does this on line 52.

- `src/crosshook-native/src/hooks/useGameCoverArt.ts:52`

**CSS: BEM-like `crosshook-*` classes**
All component CSS classes use the `crosshook-*` prefix and BEM-style suffixes (`__element`, `--modifier`). Example: `crosshook-library-card__image`, `crosshook-library-card--selected`.

- `src/crosshook-native/src/components/library/LibraryCard.tsx`

---

## Error Handling

**Non-fatal metadata store errors are `tracing::warn!`, not propagated**
`upsert_game_image` failure after a successful file write is logged but does not cause the command to return an error. The file is usable even without a DB row.

- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:324-342`

**Download failures return `Ok(None)`, hard config errors return `Err(String)`**
`download_and_cache_image` distinguishes: a bad `app_id` is `Err` (caller bug); a network failure or rejected MIME is `Ok(None)` (caller shows placeholder). Tauri commands surface `Err(String)` to the frontend as rejected promise.

- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:130-152`

**Stale-cache fallback on every download failure**
After any download error (network, SteamGridDB, CDN), `stale_fallback_path` is called before returning `Ok(None)`. If a stale file exists on disk, it is returned instead of None. This is the `stale_fallback_path` helper.

- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:461-480`

**Frontend: `onError` handler on `<img>` for broken paths**
`LibraryCard` uses `useState(false)` for `imgFailed` and resets it on `coverArtUrl` change. This handles the case where a local path is valid but the file is gone. `GameCoverArt` returns `null` on failure; `LibraryCard` shows initials.

- `src/crosshook-native/src/components/library/LibraryCard.tsx:52-53, 80-82`
- `src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx:18` — **gotcha**: returns `null` when `steamAppId` is falsy even if `customCoverArtPath` is set. This must be fixed for tri-art.

**`tracing::instrument(skip(api_key))`**
The SteamGridDB fetch function uses `#[tracing::instrument(skip(api_key))]` to prevent the bearer token from appearing in structured logs.

- `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs:38`

---

## Testing Approach

**Unit tests co-located in the same file (`#[cfg(test)] mod tests`)**
All Rust unit tests live in the file they test. `client.rs`, `models.rs`, `import.rs`, `steamgriddb.rs`, and `profile/models.rs` each have an inline `tests` module.

- Example layout: `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:508-761`

**In-memory SQLite for store tests**
`MetadataStore::open_in_memory()` creates a throwaway store for DB round-trip tests with no disk I/O.

- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:718-749`

**`tempfile::tempdir()` for filesystem tests**
Path traversal and safe-path tests use `tempfile::tempdir()` to avoid touching real directories.

- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs:671-701`

**Test coverage expected for new code:**
- `RuntimeSection::is_empty()` must still be `false` when only `steam_app_id` is set
- `effective_profile()` / `storage_profile()` / `portable_profile()` round-trips for new custom art fields
- `import_custom_art` with each `art_type` variant routes to the correct subdirectory
- `build_download_url`, `filename_for`, `build_endpoint` — one test per new `Background` arm
- `GameCoverArt` null-gate fix — test that component renders when only `customCoverArtPath` is set

**Run command**

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

---

## Patterns to Follow

### Adding a new TOML field to a section

1. Add field to Rust struct with `#[serde(rename = "…", default, skip_serializing_if = "String::is_empty")]`
2. Update `is_empty()` if applicable
3. Mirror in `LocalOverride*Section` when the field is machine-local
4. Wire `effective_profile()`, `storage_profile()`, `portable_profile()` for all machine-local fields
5. Clear in `sanitize_profile_for_community_export` for any machine-local path
6. Add matching field to TypeScript `GameProfile` interface in `profile.ts`
7. Update `ProfileSummary` DTO + `profile_list_summaries` if the field is needed in the Library view

Reference: `custom_cover_art_path` end-to-end in `profile/models.rs`, `commands/profile.rs`, `types/profile.ts`

### Adding a new `GameImageType` variant

1. Add variant to `GameImageType` enum in `game_images/models.rs`
2. Add `Display` arm: `Self::Background => write!(f, "background")`
3. Add `build_download_url` arm returning the Steam CDN URL
4. Add `filename_for` arm returning the cache filename prefix
5. Add `build_endpoint` arm in `steamgriddb.rs`
6. Add `"background"` match arm in `fetch_game_cover_art` Tauri command

Reference: how `Portrait` was added alongside `Cover`, `Hero`, `Capsule`

### Generalizing `import_custom_cover_art`

The existing function in `game_images/import.rs` hardcodes `media_base_dir().join("covers")`. The generalized version takes an `art_type` enum (or closed string set), routes to `media/portraits/` or `media/backgrounds/`, and reuses all existing validation and hashing logic. The backward-compat wrapper `import_custom_cover_art` calls `import_custom_art(path, "cover")`.

Reference: `src/crosshook-native/crates/crosshook-core/src/game_images/import.rs:32-65`

### Adding a Tauri command

1. Write the `#[tauri::command]` function in the relevant `commands/*.rs` file
2. Add it to the `invoke_handler!` list in `src-tauri/src/lib.rs`
3. Add the frontend `invoke<ReturnType>('command_name', { camelCaseArgs })` call (in a hook or directly)

Registration location: `src/crosshook-native/src-tauri/src/lib.rs:200-280`

### Frontend art consumption pattern

The `useGameCoverArt(steamAppId, customArtPath, imageType)` hook already accepts `imageType`. The hook prioritizes `customArtPath` over remote fetch and handles loading/error states. Components subscribe to `coverArtUrl` and `loading` from this hook.

- For Library grid: always request `'portrait'` (one IPC call per visible card via IntersectionObserver)
- For profile editor header: request `'cover'` (existing `GameCoverArt` component)
- For background backdrop: request `'background'` (new consumer)

The `LibraryCardData` type in `library.ts` will need `customPortraitArtPath` (alongside `customCoverArtPath`) once tri-art lands, mirroring the existing pattern.

### `resolveArtAppId` helper placement

The Rust helper `resolve_art_app_id(profile: &GameProfile) -> &str` belongs in `profile/models.rs` alongside the existing `resolve_launch_method` helper function (line 511).

The TypeScript utility `resolveArtAppId(profile: GameProfile): string` goes in the new `src/crosshook-native/src/utils/art.ts` file, matching the convention in `utils/steam.ts` (pure functions that derive values from profile data).

---

## Relevant Files

| File | Role |
|------|------|
| `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs` | `GameImageType` enum, `GameImageError`, `GameImageSource` |
| `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs` | Download pipeline, cache, validation, URL building |
| `src/crosshook-native/crates/crosshook-core/src/game_images/import.rs` | Custom art import (validation, content-addressed copy) |
| `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs` | SteamGridDB API call + endpoint construction |
| `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` | All profile section structs, `effective_profile`, `storage_profile`, `portable_profile` |
| `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs` | `sanitize_profile_for_community_export` |
| `src/crosshook-native/src-tauri/src/commands/game_metadata.rs` | `fetch_game_cover_art`, `import_custom_cover_art` Tauri commands |
| `src/crosshook-native/src-tauri/src/commands/profile.rs` | `profile_list_summaries`, `profile_save`, `ProfileSummary` DTO |
| `src/crosshook-native/src-tauri/src/lib.rs` | Command registration in `invoke_handler!` |
| `src/crosshook-native/src/types/profile.ts` | `GameProfile` TypeScript interface |
| `src/crosshook-native/src/types/library.ts` | `LibraryCardData` interface |
| `src/crosshook-native/src/hooks/useGameCoverArt.ts` | Art fetch hook (request dedup, custom-art short-circuit, `convertFileSrc`) |
| `src/crosshook-native/src/hooks/useLibrarySummaries.ts` | Invokes `profile_list_summaries`, maps to `LibraryCardData` |
| `src/crosshook-native/src/components/library/LibraryCard.tsx` | Library grid card — IntersectionObserver + `useGameCoverArt` |
| `src/crosshook-native/src/components/profile-sections/MediaSection.tsx` | Custom art browse/import UI (currently cover-only) |
| `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx` | proton_run App ID field (currently reads `steam.app_id`) |
| `src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx` | Profile editor header cover art (null-gate bug to fix) |
| `src/crosshook-native/src/utils/steam.ts` | Reference for utility function conventions |
