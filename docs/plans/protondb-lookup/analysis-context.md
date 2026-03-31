# Context Analysis: protondb-lookup

## Executive Summary

The feature is a narrow but non-trivial cross-layer addition: it starts from an existing Steam App ID on the selected profile, resolves cached-or-live ProtonDB guidance in `crosshook-core`, exposes that through a thin Tauri command, and renders it in the profile editor as advisory UI. The stable value path is the summary endpoint and metadata cache; the riskier value path is richer recommendation aggregation from an undocumented ProtonDB report feed. Planning should therefore sequence exact-tier and fetch/cache work before UI composition so the editor never depends on brittle upstream behavior.

## Architecture Context

- **System Structure**: CrossHook’s business logic lives in `crosshook-core`, Tauri commands in `src-tauri` stay thin, and the profile editor composes typed hooks/components in the React frontend.
- **Data Flow**: `GameProfile.steam.app_id` in the selected profile becomes the ProtonDB lookup key; the backend resolves a normalized snapshot through the metadata cache and returns a typed DTO to the frontend.
- **Integration Points**: `MetadataStore`, `ProfileFormSections`, `ProfilesPage`, and the Tauri invoke registry are the main touch points.

## Critical Files Reference

- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: existing reusable remote-cache path
- /src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: existing compatibility enum mismatch that must not be ignored
- /src/crosshook-native/src-tauri/src/commands/version.rs: best thin-command precedent for the new lookup command
- /src/crosshook-native/src/components/ProfileFormSections.tsx: where the new advisory UI should be mounted
- /src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx: reusable copy-action pattern for launch-setting suggestions

## Patterns to Follow

- **Backend-First Remote Integration**: put fetch/normalization/cache logic in `crosshook-core`; expose only DTOs over IPC.
- **Soft Advisory State**: model missing app ID, stale cache, and unavailable upstream as non-fatal UI states.
- **Feature-Local Contracts**: create a dedicated `ProtonDbTier` contract instead of mutating `CompatibilityRating`.

## Cross-Cutting Concerns

- Exact-tier styling and copy/apply UI need to stay consistent across Steam and Proton runtime launch methods.
- Recommendation application touches user-owned `launch.custom_env_vars`, so overwrite behavior must be explicit.
- The richer report feed is an external fragility; summary-only fallback has to remain intact.

## Security Constraints

- Never auto-apply raw ProtonDB `launchOptions` strings into launch commands.
- Keep browser CORS out of scope by fetching only in the backend.
- Treat all free-form notes as plain text.

## Reuse Opportunities

- /src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: cache access and TTL plumbing
- /src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: apply path for supported env vars
- /src/crosshook-native/src/hooks/useProfileHealth.ts: typed hook and soft-state precedent

## Parallelization Opportunities

- Exact-tier CSS/styling can start once the backend DTO names are fixed.
- Frontend TS type/hook wiring can proceed in parallel with core parser tests after the IPC contract is defined.
- Shared files that need tight coordination are `/src/crosshook-native/src-tauri/src/lib.rs`, `/src/crosshook-native/src/components/ProfileFormSections.tsx`, and `/src/crosshook-native/src/styles/theme.css`.

## Implementation Constraints

- No frontend test runner exists; verification must rely on Rust tests, `tsc`, and a manual Tauri pass.
- `CompatibilityRating` cannot represent exact ProtonDB tiers, so the plan must preserve both concepts separately.
- `external_cache_entries` has a 512 KiB cap, so cached payloads must be normalized and compact.

## Key Recommendations

- Burn down the external-integration risk first: exact tiers, summary fetch, cache, and fallback.
- Reuse the generic metadata cache instead of adding a migration unless the first implementation proves it insufficient.
- Keep recommendation apply flows narrow and safe; if a suggestion cannot be merged sanely, leave it copy-only.
