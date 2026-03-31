# Integration Research: protondb-lookup

## API Endpoints

### Existing Related Endpoints

- `auto_populate_steam`: derives Steam App ID, compatdata, and Proton path from local Steam installs
- `check_version_status`: compares live Steam build state against cached trainer/game version metadata
- `build_steam_launch_options_command`: renders a copyable `%command%`-style launch options line from current profile settings
- `profile_load` / `profile_save`: current profile editor load/save path

### Route Organization

Tauri commands are grouped by feature under `src-tauri/src/commands/` and registered centrally in `src-tauri/src/lib.rs`. Each command receives shared app state through `State<'_, ...>` and converts typed backend results into Serde-friendly IPC DTOs.

## Database

### Relevant Tables

- `external_cache_entries`: generic remote JSON cache with TTL support
- `profiles`: stable profile identity and filename mapping used by the metadata layer
- `version_snapshots`: existing Steam App ID and trainer-version intelligence keyed by profile
- `community_profiles`: imported community compatibility metadata that already uses the legacy `CompatibilityRating` scale

### Schema Details

- `external_cache_entries` already stores `source_url`, `cache_key`, `payload_json`, `payload_size`, `fetched_at`, and `expires_at`
- `cache_key` is unique, which is enough for namespaced ProtonDB summary/recommendation entries
- `MAX_CACHE_PAYLOAD_BYTES` is enforced at 512 KiB, so normalized cached payloads should stay compact
- No ProtonDB-specific migration is required if the feature reuses the generic cache table

## External Services

- ProtonDB summary endpoint: stable-enough live JSON keyed by Steam App ID
- ProtonDB report feed: richer live JSON observed from the web app, but currently undocumented and not keyed directly by Steam App ID
- ProtonDB Steam proxy: relevant for issue `#52` metadata integration, but out of scope for issue `#53` lookup implementation

## Internal Services

- `MetadataStore`: cache persistence and existing SQLite lifecycle
- `ProfileStore`: existing Steam App ID source of truth for the profile editor
- `ProfileContext` / `useProfile`: selected-profile and update flow for the form
- `useProfileHealth`: existing soft-failure metadata-driven hook pattern

## Configuration

- No secrets or OAuth configuration are required.
- The new backend HTTP client should set an explicit timeout and user agent.
- Cache TTL should be conservative because ProtonDB does not publish rate-limit guidance.
