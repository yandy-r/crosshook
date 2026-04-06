# Integration Research: protonup-integration

## API Endpoints

### Existing Related Endpoints

- `list_proton_installs` (Tauri command in `/src/crosshook-native/src-tauri/src/commands/steam.rs`): returns locally discovered Proton installs.
- `get_app_settings` / `update_app_settings` (commands in `/src/crosshook-native/src-tauri/src/commands/settings.rs`): persistence entry point for ProtonUp-related preference flags.

### Route Organization

CrossHook uses Tauri command handlers instead of HTTP routes. Commands are grouped by domain under `/src/crosshook-native/src-tauri/src/commands/`, then registered centrally in `/src/crosshook-native/src-tauri/src/lib.rs`.

## Database

### Relevant Tables

- `external_cache_entries`: cache layer for remote Proton catalog data.
- `community_profiles`: includes community metadata used for version suggestion context.
- `profiles`: local profile identity and metadata linkage.
- `version_snapshots` (optional advisory correlation input): latest known game/trainer version information.

### Schema Details

- `external_cache_entries` is the preferred storage for bounded JSON cache payloads with TTL (`expires_at`) and namespace keys.
- No new schema is required for baseline ProtonUp rollout if install history remains runtime-only or implicit.
- If install operation history is added later, it should be a new operational metadata table and migration.

## External Services

- `protonup-rs` / `libprotonup`: primary provider candidate for release catalog and installation orchestration.
- GitHub Releases endpoints for GE-Proton and Wine-GE metadata when direct source listing is needed:
  - `https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases`
  - `https://api.github.com/repos/GloriousEggroll/wine-ge-custom/releases`
- Steam compatibility tools filesystem layout references from Valve Proton docs.

## Internal Services

- `steam/proton.rs` and `steam/discovery.rs`: install root/runtime discovery and normalization.
- `metadata/cache_store.rs`: cache read/write and expiry handling.
- `profile/community_schema.rs`: `proton_version` metadata used for recommendation matching.
- Frontend hooks/pages:
  - `/src/crosshook-native/src/hooks/useProtonInstalls.ts`
  - `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`
  - `/src/crosshook-native/src/components/pages/CompatibilityPage.tsx`

## Configuration

- Existing `settings.toml` model supports extension via `AppSettingsData` in `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`.
- Candidate new settings:
  - `protonup.auto_suggest`
  - `protonup.binary_path` (when CLI adapter path override is required).
