# Practices Research: proton-app-id

## Executive Summary

The codebase has a mature, well-structured art pipeline that already supports multiple `GameImageType` variants (Cover, Hero, Capsule, Portrait) through a single generic `download_and_cache_image` function backed by a SQLite cache table. Adding Background is a pure extension of an existing enum — the hardest engineering work is the model change to expose `steam_app_id` on `RuntimeSection` and updating `ProfileSummary`/`LibraryCardData` to carry per-type custom art paths. The tri-art system is not over-engineering; the generic machinery already exists and adding `Background` costs one enum arm and one CDN URL. Per-type custom art upload does require a small new column pattern in the model or a dedicated map, but can reuse `import_custom_cover_art`'s implementation via a renamed/generalized function.

---

## Existing Reusable Code

| Module/Utility                          | Location                                                       | Purpose                                                                                                    | How to Reuse for This Feature                                                                                                                                                        |
| --------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `download_and_cache_image`              | `crates/crosshook-core/src/game_images/client.rs:146`          | Downloads, validates, caches an image keyed by `(app_id, GameImageType)`                                   | Call for each art type; pass the proton `steam_app_id` from `RuntimeSection` just as `steam.app_id` is passed today                                                                  |
| `GameImageType` enum                    | `crates/crosshook-core/src/game_images/models.rs:8`            | Enum of all art types (`Cover`, `Hero`, `Capsule`, `Portrait`)                                             | Add `Background` arm here; update `build_download_url` and `build_endpoint` to map it                                                                                                |
| `fetch_steamgriddb_image`               | `crates/crosshook-core/src/game_images/steamgriddb.rs:39`      | Fetches raw bytes from SteamGridDB by `(app_id, GameImageType)`                                            | `Background` maps to `"heroes"` endpoint in SteamGridDB; extend `build_endpoint`                                                                                                     |
| `import_custom_cover_art`               | `crates/crosshook-core/src/game_images/import.rs:32`           | Validates, content-addresses, and copies a user-selected image to `~/.local/share/crosshook/media/covers/` | Generalize to `import_custom_art(source_path, art_type)` that writes to a subdirectory named by art type (`covers/`, `portraits/`, `backgrounds/`); existing logic is fully reusable |
| `is_in_managed_media_dir`               | `crates/crosshook-core/src/game_images/import.rs:17`           | Detects whether a path is already inside the managed media dir (prevents re-import)                        | Works as-is for any subdirectory under `~/.local/share/crosshook/media/`                                                                                                             |
| `validate_image_bytes`                  | `crates/crosshook-core/src/game_images/client.rs:55`           | Magic-byte detection + size/MIME allow-list                                                                | Reuse unchanged for all art types                                                                                                                                                    |
| `safe_image_cache_path`                 | `crates/crosshook-core/src/game_images/client.rs:94`           | Path-traversal-safe cache path construction                                                                | Reuse unchanged; cache paths are already segmented by `app_id/` and `image_type` in the filename                                                                                     |
| `game_image_cache` SQLite table         | `crates/crosshook-core/src/metadata/migrations.rs:645`         | Stores `(steam_app_id, image_type, source)` rows with file path, hash, TTL                                 | Already supports arbitrary `image_type` strings via TEXT column; no schema migration needed to add `background`                                                                      |
| `upsert_game_image` / `get_game_image`  | `crates/crosshook-core/src/metadata/game_image_store.rs:24,70` | DB persistence for cached images                                                                           | Pass `"background"` as `image_type`; fully reusable                                                                                                                                  |
| `fetch_game_cover_art` Tauri command    | `src-tauri/src/commands/game_metadata.rs:20`                   | Dispatches to `download_and_cache_image` with `image_type` param                                           | Already accepts `image_type: Option<String>` — adding `"background"` requires one new `match` arm                                                                                    |
| `import_custom_cover_art` Tauri command | `src-tauri/src/commands/game_metadata.rs:46`                   | Thin wrapper around core import                                                                            | Generalize to accept `art_type` param once core function is generalized                                                                                                              |
| `useGameCoverArt` hook                  | `src/hooks/useGameCoverArt.ts:13`                              | Fetches and resolves art URL (custom → auto-downloaded → null); already accepts `imageType` param          | Already generalized by `imageType` string. Rename to `useGameArt` or keep as-is and call with `'portrait'`, `'background'`, etc.                                                     |
| `GameCoverArt` component                | `src/components/profile-sections/GameCoverArt.tsx:10`          | Renders a single art image with loading/error states                                                       | Parameterize to accept `imageType` prop; delegates to `useGameCoverArt` which already accepts it                                                                                     |
| `MediaSection` component                | `src/components/profile-sections/MediaSection.tsx:13`          | Single custom cover art input field + browse                                                               | Extend to render per-art-type fields; the browse/invoke/import pattern can be repeated for portrait/background                                                                       |
| `ProfileSummary` DTO                    | `src-tauri/src/commands/profile.rs:233`                        | IPC summary sent to library                                                                                | Add per-type custom art paths if library needs to show non-cover art; currently only portrait is used in library cards                                                               |
| `LibraryCardData` type                  | `src/types/library.ts:3`                                       | Frontend library card data                                                                                 | Add `customPortraitArtPath` if portrait custom override is needed in the grid                                                                                                        |

---

## Modularity Design

### Recommended Module Boundaries

**`game_images` (crates/crosshook-core/src/game_images/)** is the correct home for all art logic. It currently has:

- `models.rs` — `GameImageType`, `GameImageSource`, `GameImageError`
- `client.rs` — download/cache orchestration
- `steamgriddb.rs` — SteamGridDB API
- `import.rs` — custom art import

The feature requires changes to all four files but no new files. No separate "art manager" abstraction is needed.

**`profile/models.rs`** is where `steam_app_id` belongs on `RuntimeSection`. This is a TOML-persisted field addition.

**Tauri command layer** (`src-tauri/src/commands/game_metadata.rs`) stays thin: extend `fetch_game_cover_art` with a new `"background"` branch and generalize `import_custom_cover_art` to accept an `art_type` parameter.

**`profile.rs` command** handles auto-import at save time. It will need to replicate the existing cover-art import logic for portrait and background custom art paths if those are added to the profile model.

### Shared vs. Feature-Specific Code

**Shared (do not duplicate):**

- `validate_image_bytes` — one function, called everywhere
- `safe_image_cache_path` — one function, used for all `GameImageType` variants
- `http_client()` singleton — already shared
- `game_image_cache` DB table — `image_type` column is already a free-text key

**Feature-specific additions:**

- `GameImageType::Background` arm + CDN URL + SteamGridDB endpoint mapping
- `RuntimeSection.steam_app_id: Option<String>` field
- `import_custom_art(source_path, art_type)` replacing the cover-only `import_custom_cover_art`
- Per-type custom art path fields on `GameSection` (or a `BTreeMap<GameImageType, String>`)

---

## KISS Assessment

**Is the tri-art system over-engineering?** No. The core download/cache machinery is already generic over `GameImageType`. Adding `Background` costs:

- One enum arm in `GameImageType`
- One URL match arm in `build_download_url`
- One endpoint match arm in `build_endpoint` in steamgriddb.rs
- One `"background"` string in the Tauri command dispatcher

The only place tri-art adds meaningful new code is in the profile model (per-type custom art paths) and the `MediaSection` UI. Both are bounded and straightforward.

**Should background wait?** Reasonable to ship cover + portrait first and add background in a follow-up. The pipeline cost is low, but if there is no UI surface that uses background art yet, deferring it is the simpler path. Ship what is consumed.

**Simplest approach that satisfies requirements:**

1. Add `steam_app_id: Option<String>` to `RuntimeSection` in Rust + TypeScript.
2. Extend `GameImageType` with `Background` (or defer if no UI consumer).
3. Generalize `import_custom_cover_art` → `import_custom_art(path, art_type)`.
4. Add per-type custom art path fields to `GameSection` only for the types actually used in UI.
5. `fetch_game_cover_art` Tauri command already works once the caller passes the right `app_id`.

---

## Abstraction vs. Repetition

### Extract (Worth Abstracting)

- **`import_custom_art(source_path, art_type)`** — the existing `import_custom_cover_art` has all validation, hashing, and idempotent-write logic. Parameterize the subdirectory path by art type. This avoids copy-pasting three nearly-identical functions for cover/portrait/background import.

- **Art-type-to-subdirectory mapping** — a small helper `fn media_subdir_for(art_type: GameImageType) -> &'static str` keeps the `media/covers/`, `media/portraits/`, `media/backgrounds/` path derivation in one place.

- **Tauri `fetch_game_cover_art`** already dispatches over `image_type: Option<String>`; the `match` arm pattern should stay there rather than being split into separate commands.

### Repeat (Acceptable Duplication)

- **Per-type field declarations in `GameSection`** — three separate `Option<String>` fields (`custom_cover_art_path`, `custom_portrait_art_path`, `custom_background_art_path`) are clearer than a `BTreeMap<String, String>` for 3 fixed types. Rule of three applies: once you have three variants that are truly parallel and fixed, individual fields remain the most readable and type-safe representation. A map only makes sense if the types are dynamic.

- **Per-type `FieldRow` in `MediaSection`** — three separate UI rows for cover/portrait/background are more readable than a generic loop over a small fixed set. Repeat the pattern; the code is short.

- **`profile_save` auto-import logic** — repeating the import guard for each art type field is acceptable (the guard is 3 lines per type); no abstraction needed for 3 cases.

---

## Interface Design

### Public API Surfaces

**Rust (`game_images` module exports):**

```rust
// Already exists — keep
pub use client::download_and_cache_image;
pub use models::{GameImageError, GameImageSource, GameImageType};

// Generalize (breaking rename — internal only, update call sites)
pub use import::{import_custom_art, is_in_managed_media_dir};
```

**Tauri IPC commands:**

- `fetch_game_cover_art(app_id, image_type?)` — **keep as-is**, add `"background"` branch. Generic `image_type` string parameter already exists.
- `import_custom_cover_art(source_path)` — **generalize** to `import_custom_art(source_path, art_type)` or overload to `import_custom_cover_art(source_path, art_type?)` for backward compatibility with existing callers.

One generic command (`fetch_game_cover_art` + `import_custom_art`) is correct. Separate commands per type would duplicate auth/settings lookup and parameter validation.

**Frontend hooks:**

- `useGameCoverArt(appId, customPath, imageType?)` — **keep the current signature**; it already accepts `imageType`. `LibraryCard` already calls it with `'portrait'`. Call sites that need cover or background simply pass a different `imageType` string.
- No need for separate `useGamePortraitArt`, `useGameBackgroundArt` hooks — parameterizing `imageType` in the existing hook is the right approach.

### Extension Points

- `GameImageType` enum is the single extension point for adding new types; all routing (CDN URL, SteamGridDB endpoint, filename prefix, DB `image_type` column value) branches off it.
- `GameSection` in `profile/models.rs` is the extension point for custom art path fields.
- `LocalOverrideGameSection` needs matching per-type fields for custom art to participate in the local-override / portable-profile system (see `effective_profile()` and `storage_profile()` in `profile/models.rs:408`).

---

## Testability Patterns

**Existing test infrastructure to follow:**

- `MetadataStore::open_in_memory()` — used in `client.rs` tests for upsert/get round-trips. New art type tests can follow the same pattern: insert a row with `image_type = "background"`, assert retrieval.

- `tempfile::tempdir()` for path validation tests — already used in `safe_image_cache_path` tests (`client.rs:671`).

- Pure unit tests for enum mapping — `build_endpoint` in `steamgriddb.rs` has `#[test]` per `GameImageType` variant. Add a test for `Background` when the arm is added.

- `validate_image_bytes` tests for MIME/size rejection — fully reusable for any art type.

**Art resolution priority chain testing:**

- The priority chain is: custom art path (non-empty, exists on disk) → cached image (non-expired) → stale cache → `None`.
- This logic lives entirely in `download_and_cache_image` and `useGameCoverArt`. Test the Rust function with an in-memory `MetadataStore` and temp paths. Test the hook behavior with mocked `invoke` (Tauri test utilities or a simple stub).

**Custom art override logic testing:**

- `import_custom_art` (after generalization) can be unit-tested with a real tempdir containing valid JPEG/PNG bytes.
- `is_in_managed_media_dir` is already tested and covers the boundary.

**No new test framework needed** — the existing Rust `#[test]` pattern, in-memory DB, and `tempfile` cover all scenarios for the backend. The frontend has no configured test framework; test via dev/build scripts as per CLAUDE.md.

---

## Build vs. Depend

**Existing dependencies that cover all requirements:**

| Crate                        | Already in Cargo.toml | Use for this feature                  |
| ---------------------------- | --------------------- | ------------------------------------- |
| `reqwest` (0.12, rustls-tls) | Yes                   | HTTP downloads for all art types      |
| `infer` (~0.16)              | Yes                   | Magic-byte image validation           |
| `sha2` (0.11)                | Yes                   | Content-addressed import filenames    |
| `directories` (6.0.0)        | Yes                   | `~/.local/share/crosshook/` base path |
| `tempfile` (3)               | Yes                   | Test helpers                          |
| `rusqlite`                   | Yes                   | `game_image_cache` DB operations      |

**No new dependencies required.** The existing `infer` crate handles JPEG, PNG, and WebP detection. No image resizing is needed (images are stored as-is and displayed via CSS `object-fit`). No new image processing crates should be added.

---

## Open Questions

1. **`steam_app_id` on `RuntimeSection` vs. reusing `steam.app_id`**: The `proton_run` form already renders a "Steam App ID" field that writes to `profile.steam.app_id` (see `RuntimeSection.tsx:182`). It is labeled "Optional for ProtonDB lookup". If that field already carries the app ID for `proton_run`, should `RuntimeSection.steam_app_id` be a separate TOML field or should the existing `steam.app_id` path be the canonical source for all launch methods? Adding a dedicated `runtime.steam_app_id` field avoids semantic overlap but adds redundancy; reusing `steam.app_id` is simpler but may confuse the intent for `proton_run` (which does not use Steam to launch). Decision impacts whether `ProfileSummary.steam_app_id` needs to change its source field.

2. **`LocalOverrideSection` coverage for per-type custom art**: Custom cover art is already part of `LocalOverrideGameSection` (`custom_cover_art_path`). Portrait and background custom art paths are machine-local paths that need the same treatment. Should they be added to `LocalOverrideGameSection` in parallel? Required for the `effective_profile()` / `storage_profile()` / `portable_profile()` semantics to work correctly for community-shared profiles.

3. **`ProfileSummary` and library card for portrait custom art**: `LibraryCard` uses `profile.customCoverArtPath` passed through `ProfileSummary`. If portrait has a separate custom art override, `ProfileSummary` must also carry `customPortraitArtPath` for the library grid to respect it. Is the library always portrait, or could it be configurable?

4. **Background art UI consumer**: Is there a defined UI surface that will consume background art (e.g., a fullscreen backdrop on the launch page)? If not yet designed, shipping background type in the pipeline without a UI consumer is low-risk (enum arm + DB row), but the `GameSection` model fields and `MediaSection` UI rows for background can be deferred until a consumer exists.

5. **SteamGridDB `Background` endpoint**: SteamGridDB's `heroes` endpoint returns hero/background images. `GameImageType::Hero` already maps to `"heroes"` in `steamgriddb.rs:104`. If `Background` is logically a separate concept from `Hero`, should `Background` map to the same `heroes` endpoint or a different one? If they are the same API endpoint but different display contexts, the two enum variants would share a URL — a potential source of confusion.
