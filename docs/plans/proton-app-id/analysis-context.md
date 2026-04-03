# Context Analysis: proton-app-id

## Executive Summary

This feature adds an optional `runtime.steam_app_id` field to `proton_run` profiles so they can resolve art through the existing `game_images` download/cache pipeline — and extends custom art from a single cover slot to a tri-art system (cover, portrait, background) with per-type upload and mix-and-match resolution. Nearly all backend infrastructure already exists; the work is additive data model changes, generalization of one import function, and new UI surfaces.

## Architecture Context

- **System Structure**: `crosshook-core` owns all business logic (art pipeline, profile models, SQLite ops). `src-tauri` is a thin IPC layer. React/TypeScript frontend communicates exclusively via `invoke()`. Profile data persists as TOML files; art cache lives in SQLite `game_image_cache` (schema v14) with image files on disk under `~/.local/share/crosshook/cache/images/`.
- **Data Flow**: `LibraryCard` (IntersectionObserver) → `useGameCoverArt(steamAppId, customPath, 'portrait')` → `fetch_game_cover_art` IPC → `download_and_cache_image` (SQLite TTL check → SteamGridDB → Steam CDN → stale cache → None) → absolute path returned → `convertFileSrc` for WebView display. Custom art short-circuits before any IPC call.
- **Integration Points**: Phase 1 plugs into `profile_list_summaries` (resolve effective app_id) and `RuntimeSection.tsx` (rebind field). Phase 2 plugs into `import_custom_cover_art` (generalize), `profile_save` auto-import, and `MediaSection.tsx`. Phase 3 plugs into `GameImageType` enum and all four match sites that branch on it.

## Critical Files Reference

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: All profile section structs + three-layer merge logic (`effective_profile`, `storage_profile`, `portable_profile`) — primary change surface for Phase 1 and 2
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/models.rs`: `GameImageType` enum — adding `Background` requires updating all four downstream match sites
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`: `download_and_cache_image`, `build_download_url`, `filename_for`, HTTP singleton — add `Background` arms; also location of required S-01/S-06 redirect policy fix
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/import.rs`: `import_custom_cover_art` — generalize to `import_custom_art(source_path, art_type)` routing to type-segregated subdirs
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs`: `build_endpoint` — add `Background => ("heroes", None)` arm
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs`: Public re-exports — must add `import_custom_art` alongside `import_custom_cover_art` (Phase 2)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`: `sanitize_profile_for_community_export` — must explicitly clear all three custom art path fields (security S-03 gap)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/game_metadata.rs`: `fetch_game_cover_art`, `import_custom_cover_art` Tauri commands — add `"background"` arm; register new `import_custom_art` command
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`: `profile_list_summaries` (use `resolve_art_app_id()`), `profile_save` (extend auto-import to all 3 art types), `ProfileSummary` DTO
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/settings.rs`: `settings_load` — filter raw SGDB API key at IPC boundary (security S-02)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: `invoke_handler!` macro — register `import_custom_art`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts`: `GameProfile` TS interface — add `runtime.steam_app_id`, `game.custom_portrait_art_path`, `game.custom_background_art_path`; mirror in `local_override.game`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`: Rebind proton_run "Steam App ID" field from `steam.app_id` to `runtime.steam_app_id` (~line 188); `steam_applaunch` field (~line 60) must remain unchanged
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/profile-sections/MediaSection.tsx`: Expand from single cover slot to three art type slots (Browse/Clear/Preview per type)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGameCoverArt.ts`: Already accepts `imageType` param — no structural change needed; receives resolved `effective_steam_app_id` from backend

## Patterns to Follow

- **Thin Tauri Commands**: All business logic in `crosshook-core`; command handlers resolve `State<'_>`, call core function, map errors to `String`. Example: `game_metadata.rs:20-48`
- **TOML field additions**: Always use `#[serde(rename = "…", default, skip_serializing_if = "String::is_empty")]`. Reference: `custom_cover_art_path` in `models.rs:193-195`
- **`is_empty()` gate**: Sections use `is_empty()` for TOML section elision. `RuntimeSection::is_empty()` currently only checks `prefix_path`, `proton_path`, `working_directory` — it MUST be extended to also return `false` when `steam_app_id` is non-empty, otherwise a profile with only `steam_app_id` set will have `is_empty()` return `true`, suppressing the `[runtime]` TOML section and silently dropping the field. `LocalOverrideGameSection::is_empty()` MUST likewise be extended to include the new portrait and background art path fields, or profiles with only those overrides will skip the `[local_override.game]` TOML block
- **Three-profile representation**: `effective_profile()` merges local_override → base; `storage_profile()` moves machine-local paths to local_override; `portable_profile()` calls storage then clears local_override. All new custom art path fields must propagate through all three identically to `custom_cover_art_path`
- **`GameImageType` enum exhaustiveness**: Adding `Background` requires four match sites: `build_download_url`, `filename_for`, `build_endpoint` (SGDB), and `fetch_game_cover_art` IPC dispatch
- **Content-addressed import**: SHA-256 prefix for filename, type-segregated subdirs (`media/covers/`, `media/portraits/`, `media/backgrounds/`), closed enum for `art_type` to prevent directory traversal
- **`ProfileSummary` DTO**: `#[serde(rename_all = "camelCase")]` — Rust snake_case maps to TS camelCase. `effective_steam_app_id` must be computed backend-side (BR-9: `steam.app_id` first, else `runtime.steam_app_id`)
- **Unit tests co-located**: `#[cfg(test)] mod tests` in the same file; `MetadataStore::open_in_memory()` for DB tests; `tempfile::tempdir()` for FS tests

## Cross-Cutting Concerns

- **Security — must fix before ship**: S-01/S-06 (add redirect-policy domain allow-list to `GAME_IMAGES_HTTP_CLIENT`); S-02 (`settings_load` must return `has_steamgriddb_api_key: bool` only); S-03 (`sanitize_profile_for_community_export` calls `portable_profile()` which already resets all `local_override` — new portrait/background art paths are covered automatically since they live in `LocalOverrideGameSection`. The residual gap is only if a custom art path ever lands in base `game` section rather than local_override; verify this cannot happen via `profile_save` enforcement); S-05 (return error for unknown `image_type` instead of silently defaulting to Cover — IPC string dispatch at `game_metadata.rs:27` is NOT compiler-checked, must be updated manually when adding `"background"`); S-12 (detect SGDB 401/403 separately, fall to CDN not stale cache)
- **Backward compatibility**: All new TOML fields use `#[serde(default)]` — existing profiles unaffected. No SQLite migration needed (`game_image_cache.image_type TEXT` accepts `"background"` without schema change)
- **Launch pipeline isolation**: `runtime.steam_app_id` is media-only (BR-2). Launch request construction must never read it. A test asserting this isolation is required
- **Local path portability**: Custom art paths (cover, portrait, background) are machine-local — must live in `local_override.game.*`, move to local_override in `storage_profile()`, cleared wholesale in `portable_profile()`. Omitting any of the four update sites (`LocalOverrideGameSection` struct, `is_empty()`, `effective_profile()`, `storage_profile()`) silently breaks portability. `runtime.steam_app_id` is explicitly NOT machine-local — it stays only in `RuntimeSection` base, does not go into `LocalOverrideRuntimeSection`, and intentionally survives portable export unchanged
- **Custom art files not deleted on clear**: Only the profile reference is removed (BR-14). Content-addressed files may be shared across profiles

## Parallelization Opportunities

- **Phase 1**: Rust model changes (`RuntimeSection.steam_app_id`, `resolve_art_app_id()`, `profile_list_summaries` update) can run in parallel with frontend utility (`resolveArtAppId()` in `src/utils/art.ts`) and `RuntimeSection.tsx` rebind
- **Phase 2**: Backend import generalization (`import_custom_art`) + `profile_save` auto-import extension can run in parallel with `MediaSection.tsx` three-slot UI expansion + `profile.ts` type updates
- **Security fixes** (S-01/S-02/S-03/S-05/S-12): Each targets a distinct file and can be implemented independently in any phase order; S-02 and S-05 are lowest-effort and can be done early
- **Phase 3**: `GameImageType::Background` backend work (enum + four match sites) can be done independently of any frontend UI consumer

## Implementation Constraints

- **No new dependencies**: All required crates (`reqwest`, `infer`, `sha2`, `rusqlite`, `directories`) already in `Cargo.toml`. No new npm packages
- **No SQLite migration**: `image_type TEXT` column accepts `"background"` without schema change
- **Art type is a closed set**: `import_custom_art(source_path, art_type)` must validate `art_type` against a closed enum/set — arbitrary strings risk directory traversal. No free-form strings allowed for routing to subdirectories
- **Background art has no UI consumer in Phase 1-2**: Add data model fields (they serialize as empty/omitted by default), but do NOT add Background slot to `MediaSection.tsx` until a display surface exists (Phase 3)
- **`steam_applaunch` App ID field untouched**: `RuntimeSection.tsx` line ~60 binds to `steam.app_id` for `steam_applaunch` — this field must remain on `steam.app_id`. Only the `proton_run` field (~line 188) gets rebound
- **Backend resolves effective app_id, not frontend**: `profile_list_summaries` must compute and return a single `effective_steam_app_id` — frontend receives one value and does not implement the fallback logic itself

## Key Recommendations

- **Phase 1 first**: Ship `runtime.steam_app_id` + art normalization as a standalone deliverable. It is independent of tri-art custom upload and unblocks proton_run art display immediately
- **Quick wins before any code**: Test whether proton_run profiles with a manually-set `steam.app_id` already show Library art (the pipeline may already work end-to-end). If yes, Phase 1 is smaller than estimated
- **Fix `GameCoverArt` null gate** (Phase 1): `src/components/profile-sections/GameCoverArt.tsx` returns null when `steamAppId` is falsy even when `customCoverArtPath` is set — small fix with high visibility impact
- **`resolve_art_app_id` placement**: Add the Rust helper to `profile/models.rs` alongside `resolve_launch_method` (line ~511). Add the TypeScript utility to the new `src/utils/art.ts` following the pattern in `utils/steam.ts`
- **Validate `steam_app_id` at save time**: Apply `validate_steam_app_id()` in `profile_save`, not only at fetch time. Frontend should mirror the same `chars().all(is_ascii_digit)` + max 12-digit check
- **Keep `import_custom_cover_art` as backward-compat wrapper**: The generalized `import_custom_art(path, art_type)` is the new canonical function; `import_custom_cover_art` becomes a thin wrapper calling it with `art_type = "cover"` for existing call sites
- **Task ordering within Phase 2**: Backend model changes (`GameSection`, `LocalOverrideGameSection`, three-profile propagation) must complete before frontend can wire new art paths — this is the only hard sequential dependency within a phase
