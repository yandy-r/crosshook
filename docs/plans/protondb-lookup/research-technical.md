# Technical Research: protondb-lookup

## Executive Summary

CrossHook already has the structural pieces needed for a clean ProtonDB integration: a reusable metadata SQLite cache, thin Tauri command handlers, typed frontend invoke patterns, and a profile editor surface that already groups Steam- and launch-related guidance. The cleanest implementation is a new `crosshook-core::protondb` module that owns remote fetch, normalization, tier mapping boundaries, and cache usage; a thin Tauri command that exposes a typed lookup result; and a new profile-editor card that renders exact tiers and explicit apply/copy actions. The main technical risk is not cache or UI work but the richer ProtonDB report feed, which is observable on the live site but not documented or keyed directly by Steam App ID.

### Architecture Approach

- Add a feature-local backend module under `crosshook-core/src/protondb/`.
- Keep remote fetch, normalization, and recommendation aggregation in `crosshook-core`, not in `src-tauri`.
- Reuse `MetadataStore::get_cache_entry` and `put_cache_entry` rather than creating a dedicated schema first.
- Expose a single `#[tauri::command]` lookup entry point, for example `protondb_lookup(app_id, force_refresh)`.
- Render the resulting state in a dedicated profile-editor component near the existing Steam App ID and auto-populate section.

### Data Model Implications

- Add a dedicated exact-tier enum, for example `ProtonDbTier`, because `CompatibilityRating` only supports `unknown`, `broken`, `partial`, `working`, and `platinum`.
- Add a normalized lookup DTO containing:
  - exact tier and secondary tiers (`bestReportedTier`, `trendingTier`)
  - score, confidence, total reports
  - fetched/stale metadata
  - normalized recommendation lists such as supported env var suggestions, copy-only launch strings, and plain-text notes
- Reuse `external_cache_entries` for serialized normalized payloads with cache keys such as `protondb:summary:v1:{appId}` and `protondb:recommendations:v1:{appId}`.
- No profile TOML migration is required for a read-only advisory panel; any apply flow can target existing `launch.custom_env_vars` or existing copy surfaces.

### API Design Considerations

- Tauri command surface:
  - `protondb_lookup(app_id: String, force_refresh: bool) -> Result<ProtonDbLookupResult, String>`
- Request/response guidance:
  - empty app ID returns an idle/not-configured response instead of an error
  - stale cache should be differentiated from hard failure
  - richer report data should be optional inside the DTO so summary lookup still succeeds when only the stable endpoint is reachable
- Error model:
  - normalize timeouts, HTTP failures, invalid JSON, and unavailable richer report data into user-friendly soft failure states
  - never propagate remote errors as validation failures for the profile itself

### System Constraints

- CORS forces backend fetching; frontend direct fetch is not viable.
- The hidden report feed appears to be page-data driven, so recommendation aggregation must be implemented behind a fallback boundary.
- `external_cache_entries` enforces a 512 KiB payload cap, so the normalized cached payload should stay compact and avoid hoarding unnecessary raw history.
- CrossHook has no configured frontend test runner, so correctness needs heavy Rust coverage plus `tsc` and manual Tauri validation.
- Remote recommendation text must be treated as untrusted input and rendered as plain text, never injected into HTML or directly into launch builders.

### File-Level Impact Preview

- Likely files to create:
  - `/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs`
  - `/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`
  - `/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`
  - `/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`
  - `/src/crosshook-native/src-tauri/src/commands/protondb.rs`
  - `/src/crosshook-native/src/types/protondb.ts`
  - `/src/crosshook-native/src/hooks/useProtonDbLookup.ts`
  - `/src/crosshook-native/src/components/ProtonDbLookupCard.tsx`
- Likely files to modify:
  - `/src/crosshook-native/crates/crosshook-core/Cargo.toml`
  - `/src/crosshook-native/crates/crosshook-core/src/lib.rs`
  - `/src/crosshook-native/src-tauri/src/commands/mod.rs`
  - `/src/crosshook-native/src-tauri/src/lib.rs`
  - `/src/crosshook-native/src/components/ProfileFormSections.tsx`
  - `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`
  - `/src/crosshook-native/src/styles/theme.css`
  - `/src/crosshook-native/src/types/index.ts`
