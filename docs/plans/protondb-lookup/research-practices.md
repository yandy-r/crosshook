# Practices Research: protondb-lookup

## Reuse Opportunities

- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: reuse the generic external cache instead of introducing a ProtonDB-only table
- /src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: keep cache access behind `MetadataStore`, not direct SQLite calls in new code
- /src/crosshook-native/src-tauri/src/commands/version.rs: copy the thin-command pattern and typed DTO style
- /src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx: reuse copy-to-clipboard and “derived from current settings” UX patterns
- /src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: reuse the existing profile update path when applying supported env-var suggestions
- /src/crosshook-native/src/hooks/useProfileHealth.ts: reuse the invoke-driven hook shape for idle/loading/loaded/error state management

## Modularity Guidance

- Keep all remote-fetch and normalization logic feature-local in `crosshook-core::protondb`.
- Do not widen `CompatibilityRating` just to satisfy this feature; exact ProtonDB tiers belong in a new contract.
- Do not create a general-purpose “external service” abstraction until there is a second real consumer; the cache helper already gives enough shared infrastructure.
- Keep UI surface local to the profile editor until another page proves it needs the same ProtonDB card.

## Build-vs-Depend Guidance

- Add one explicit HTTP dependency in `crosshook-core` rather than pushing the feature into the frontend or shelling out to external tools.
- Reuse existing Serde, SQLite, and TS typing infrastructure; no new state library or frontend data framework is necessary.
- Avoid a new database migration on the first pass because the generic cache table already solves persistence, TTL, and offline fallback.

## KISS Notes

- Summary lookup is the stable value path; build it so it stands alone.
- Recommendation aggregation should be an additive layer that can fail independently.
- If a suggestion cannot be mapped safely into existing profile fields, keep it informational instead of inventing a new launch-setting model prematurely.
