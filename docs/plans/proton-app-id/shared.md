# Proton App ID & Tri-Art System

CrossHook's art pipeline is fully built in `crosshook-core/src/game_images/` (download, cache, validate, import) with a SQLite `game_image_cache` table (schema v14, TEXT `image_type` column) and two Tauri IPC commands (`fetch_game_cover_art`, `import_custom_cover_art`). The feature adds `steam_app_id` to `RuntimeSection` so `proton_run` profiles can resolve art without a `[steam]` section, extends custom art from cover-only to three slots (cover, portrait, background) by adding fields to `GameSection`/`LocalOverrideGameSection` and generalizing the import function, and adds `GameImageType::Background` to the existing enum — all wired through the existing download/cache pipeline with no new dependencies or SQLite migrations. A `resolve_art_app_id()` helper resolves the effective media app ID (`steam.app_id` first, then `runtime.steam_app_id`) and feeds `profile_list_summaries` so the frontend receives the correct ID for `proton_run` profiles. Security mitigations (redirect policy, API key exposure, export path leak, unknown image_type default, auth failure handling) must be addressed before ship.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile`, `RuntimeSection`, `GameSection`, `LocalOverrideGameSection`, `effective_profile()`, `storage_profile()`, `portable_profile()` — all profile section structs and three-layer merge logic
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs: `sanitize_profile_for_community_export` — must clear all custom art paths (security S-03)
- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs: `GameImageType` enum (add `Background`), `GameImageError`, `GameImageSource`
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs: `download_and_cache_image` pipeline, `build_download_url`, `filename_for`, HTTP singleton, stale fallback, 5MB streaming cap
- src/crosshook-native/crates/crosshook-core/src/game_images/import.rs: `import_custom_cover_art` — generalize to `import_custom_art(source_path, art_type)` routing to type-segregated subdirs
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs: `build_endpoint` — add `Background => ("heroes", None)` arm
- src/crosshook-native/crates/crosshook-core/src/game_images/mod.rs: Public re-exports for the `game_images` module
- src/crosshook-native/crates/crosshook-core/src/metadata/game_image_store.rs: SQLite CRUD (`upsert_game_image`, `get_game_image`, `evict_expired_images`) — no changes needed, `"background"` TEXT fits existing schema
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `AppSettingsData` with `steamgriddb_api_key` — must change IPC to return `has_steamgriddb_api_key: bool` only (security S-02)
- src/crosshook-native/src-tauri/src/commands/game_metadata.rs: `fetch_game_cover_art` (add `"background"` arm), `import_custom_cover_art` (generalize to `import_custom_art`)
- src/crosshook-native/src-tauri/src/commands/profile.rs: `profile_list_summaries` (use `resolve_art_app_id`), `profile_save` (extend auto-import to all 3 art types), `ProfileSummary` DTO
- src/crosshook-native/src-tauri/src/commands/settings.rs: `settings_load` — filter API key at IPC boundary
- src/crosshook-native/src-tauri/src/lib.rs: Tauri command registration (`invoke_handler!` macro)
- src/crosshook-native/src/types/profile.ts: `GameProfile` TS interface — add `runtime.steam_app_id`, `game.custom_portrait_art_path`, `game.custom_background_art_path`, mirror in `local_override.game`
- src/crosshook-native/src/types/library.ts: `LibraryCardData` — add `customPortraitArtPath` when tri-art lands
- src/crosshook-native/src/hooks/useGameCoverArt.ts: Art fetch hook — already accepts `imageType` param, prioritizes custom path, handles loading/error
- src/crosshook-native/src/hooks/useLibrarySummaries.ts: Invokes `profile_list_summaries`, maps to `LibraryCardData`
- src/crosshook-native/src/components/library/LibraryCard.tsx: Library grid card — `useGameCoverArt(steamAppId, customCoverArtPath, 'portrait')` with IntersectionObserver
- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx: Profile editor header art — null-gate bug (returns null when steamAppId missing even if customCoverArtPath set)
- src/crosshook-native/src/components/profile-sections/MediaSection.tsx: Custom art upload UI — currently single cover slot, expand to three slots
- src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx: `proton_run` App ID field — currently bound to `steam.app_id`, rebind to `runtime.steam_app_id`

## Relevant Tables

- game_image_cache: `(steam_app_id TEXT, image_type TEXT, source TEXT)` unique key — stores cached art file paths, TTL (24h), content hash, MIME type. `"background"` as image_type works without migration.

## Relevant Patterns

**Thin Tauri Commands, Logic in Core**: All non-trivial backend logic lives in `crosshook-core`. Tauri command handlers resolve managed `State<'_>` handles, call a core function, and map errors to `String`. See [src/crosshook-native/src-tauri/src/commands/game_metadata.rs](src/crosshook-native/src-tauri/src/commands/game_metadata.rs).

**Profile Section Structs with `is_empty()` Gate**: Every optional section implements `is_empty()` and is annotated `#[serde(skip_serializing_if = "…::is_empty")]`. New TOML fields use `#[serde(rename = "…", default, skip_serializing_if = "String::is_empty")]`. See `custom_cover_art_path` at [src/crosshook-native/crates/crosshook-core/src/profile/models.rs](src/crosshook-native/crates/crosshook-core/src/profile/models.rs).

**Three-Profile Representation**: `effective_profile()` merges local_override into base (for display/launch), `storage_profile()` moves machine-local paths into local_override (for disk), `portable_profile()` calls storage then clears local_override (for export). New custom art paths must follow the exact same pattern as `custom_cover_art_path`. See [src/crosshook-native/crates/crosshook-core/src/profile/models.rs:407-468](src/crosshook-native/crates/crosshook-core/src/profile/models.rs).

**GameImageType Enum + Match Arms**: The enum drives the cache DB column value, `build_download_url`, `filename_for`, and `build_endpoint` (SteamGridDB). Adding a variant requires updating all four match sites. See [src/crosshook-native/crates/crosshook-core/src/game_images/models.rs](src/crosshook-native/crates/crosshook-core/src/game_images/models.rs).

**Content-Addressed Import**: `import_custom_cover_art` validates magic bytes + size, SHA-256 hashes, writes to `media/covers/{hash[..16]}.{ext}`. Idempotent. Generalize to route by art_type to `media/portraits/` and `media/backgrounds/`. See [src/crosshook-native/crates/crosshook-core/src/game_images/import.rs](src/crosshook-native/crates/crosshook-core/src/game_images/import.rs).

**ProfileSummary DTO (camelCase)**: IPC summary struct uses `#[serde(rename_all = "camelCase")]` — Rust snake_case maps to TS camelCase. See [src/crosshook-native/src-tauri/src/commands/profile.rs:232-239](src/crosshook-native/src-tauri/src/commands/profile.rs).

**Frontend Art Hook**: `useGameCoverArt(steamAppId, customArtPath, imageType)` prioritizes custom path over IPC fetch, uses `convertFileSrc` for local paths, handles loading/error states with request race protection. See [src/crosshook-native/src/hooks/useGameCoverArt.ts](src/crosshook-native/src/hooks/useGameCoverArt.ts).

**Error Types as Custom Enums**: `GameImageError` with `From` impls for `io::Error` and `reqwest::Error`. Non-fatal metadata errors logged via `tracing::warn!`. Download failures return `Ok(None)`, config errors return `Err`. See [src/crosshook-native/crates/crosshook-core/src/game_images/models.rs:44-106](src/crosshook-native/crates/crosshook-core/src/game_images/models.rs).

**Unit Tests Co-Located**: `#[cfg(test)] mod tests` in the same file. `MetadataStore::open_in_memory()` for DB tests, `tempfile::tempdir()` for filesystem tests. Run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

## Relevant Docs

**docs/plans/proton-app-id/feature-spec.md**: You _must_ read this before writing any code. Authoritative contract: business rules (BR-1 through BR-14), data models, API signatures, file-by-file modification list, 4-phase rollout, storage classification, resolved decisions.

**AGENTS.md**: You _must_ read this before any implementation. Hard architectural constraints: `crosshook-core` owns business logic, IPC-thin `src-tauri`, persistence classification rules, CSS layout contracts, build commands.

**docs/plans/proton-app-id/research-security.md**: You _must_ read this before touching `game_images/client.rs`, `settings.rs`, or community export. 5 WARNING-level findings with Rust code snippets: redirect policy (S-01/S-06), API key leak (S-02), export path disclosure (S-03), silent image_type default (S-05), auth failure handling (S-12).

**docs/plans/proton-app-id/research-technical.md**: You _must_ read this before modifying Rust data models or React components. Full struct change specs, `is_empty()` exclusion note, `ProfileSummary` DTO design.

**docs/plans/proton-app-id/research-business.md**: You _must_ read this before implementing profile-save logic, portability rules, or art resolution chain. Business rules for art priority (BR-1), `steam_app_id` is media-only (BR-2), custom art is machine-local (BR-6), per-type independence (BR-10).

**docs/plans/proton-app-id/research-practices.md**: You _must_ read this before creating new functions. Complete reuse inventory prevents duplication. Shows generalization pattern for `import_custom_art`.

**docs/plans/proton-app-id/research-ux.md**: You _must_ read this before implementing `MediaSection.tsx` or the App ID field. Three-slot media section design, source badge pattern, thumbnail preview flow.

**docs/plans/ui-enhancements/research-technical.md**: You _must_ read this before touching `game_images/` Rust modules. Established the baseline art infrastructure this feature extends.

**docs/plans/proton-app-id/research-recommendations.md**: Read this for phasing rationale, risk assessment, and 3 quick-win pre-implementation tests.

**docs/plans/proton-app-id/research-external.md**: Read this before modifying `client.rs` or `steamgriddb.rs`. Confirmed CDN fallback chains, codebase state inventory, redirect allow-list domains.
