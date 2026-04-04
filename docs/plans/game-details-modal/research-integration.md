# Integration Research: game-details-modal

## API Endpoints

### Existing Related Endpoints

- `profile_list_summaries`: summary data already used by library cards (`useLibrarySummaries.ts`).
- `profile_load`: full profile payload available when details need richer profile fields.
- `fetch_game_metadata`: Steam metadata lookup used by `useGameMetadata.ts`.
- `fetch_game_cover_art`: local/remote cover art lookup used by `useGameCoverArt.ts`.
- `protondb_lookup`: ProtonDB enrichment used by `useProtonDbLookup.ts`.
- `get_profile_health`: profile health integration point for optional diagnostics details.
- `check_offline_readiness`: optional offline readiness enrichment.

All commands are registered in `src/crosshook-native/src-tauri/src/lib.rs`.

### Route Organization

Library features are route-state driven (not URL-driven). `AppRoute` includes `library`, and `ContentArea.tsx` renders `LibraryPage.tsx`, which composes `LibraryGrid.tsx` and `LibraryCard.tsx`.

## Database

### Relevant Tables

- `external_cache_entries`: HTTP cache used for Steam metadata and ProtonDB lookup responses.
- `health_snapshots`: health data used by profile health surfaces.
- `offline_readiness_snapshots`: cached readiness status for offline behavior.
- `version_snapshots`: version/correlation diagnostics that may be referenced in advanced details.

### Schema Details

Metadata storage is SQLite-backed in `crosshook-core` and accessed via `MetadataStore` initialized in `src-tauri/src/lib.rs`. For this modal scope, existing tables are read through existing commands; no new schema is required for v1.

## External Services

- Steam Store appdetails API (`crosshook-core/src/steam_metadata/client.rs`).
- ProtonDB API surface and app pages (`crosshook-core/src/protondb/client.rs`).
- SteamGridDB image API (optional, via configured API key) in `crosshook-core/src/game_images/steamgriddb.rs`.

## Internal Services

- Profile read/list services via `ProfileStore` in `src-tauri/src/commands/profile.rs`.
- Game metadata and image services in `src-tauri/src/commands/game_metadata.rs`.
- ProtonDB integration in `src-tauri/src/commands/protondb.rs`.
- Health/offline/version commands in `src-tauri/src/commands/health.rs`, `offline.rs`, `version.rs`.

## Configuration

- `AppSettingsData.steamgriddb_api_key` controls optional SteamGridDB resolution (`crosshook-core/src/settings/mod.rs`).
- UI view preferences such as `crosshook.library.viewMode` are currently stored in browser `localStorage` in `LibraryPage.tsx`.
- Command names and invoke wiring must stay aligned (`snake_case` command names, matching frontend `invoke` strings).
