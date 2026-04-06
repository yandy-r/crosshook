# ProtonUp Integration

ProtonUp integration spans all three CrossHook layers: `crosshook-core` for provider orchestration, `src-tauri` for IPC surface, and React hooks/pages for user-facing install and recommendation flows. Existing runtime discovery already lives in `steam/proton.rs`, so new work should extend that path rather than building a parallel inventory model. Cached provider catalogs fit the existing SQLite `external_cache_entries` mechanism, while user defaults remain in TOML settings. The implementation should keep recommendations advisory by default, with install actions explicit, integrity-checked, and non-blocking to valid launch paths.

## Relevant Files

- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: Existing installed-runtime discovery and normalization source of truth.
- `/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs`: Steam root detection for install destination validation.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `external_cache_entries` cache reads/writes with TTL support.
- `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: TOML settings model for ProtonUp preference/path additions.
- `/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`: Community profile metadata (`proton_version`) used for suggestions.
- `/src/crosshook-native/src-tauri/src/commands/steam.rs`: Existing Proton listing IPC command pattern.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Command registration for new `protonup_*` handlers.
- `/src/crosshook-native/src/hooks/useProtonInstalls.ts`: Typed invoke wrapper pattern for list/refresh behavior.
- `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: Profile runtime UX and suggestion entry point.
- `/src/crosshook-native/src/components/pages/CompatibilityPage.tsx`: Compatibility guidance page for recommendation/install controls.
- `/docs/plans/protonup-integration/feature-spec.md`: Accepted scope, decision lock-ins, and phase sequencing.

## Relevant Tables

- `external_cache_entries`: Cached available-version catalogs and freshness metadata.
- `community_profiles`: Community metadata context for recommendation source inputs.
- `profiles`: Local profile identity and linkage for profile-level suggestion targeting.
- `version_snapshots`: Optional advisory context for version correlation messaging.

## Relevant Patterns

**Core-first service layering**: Place provider/catalog/install logic in `crosshook-core`, then expose thin commands and hooks. See [`/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`](/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs).

**Cache-live-stale fallback**: Resolve from cache first, refresh from network, and fallback to stale cache when offline/errors occur. See [`/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`](/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs).

**Typed IPC hook wrappers**: Frontend should consume typed hooks instead of direct command calls in page components. See [`/src/crosshook-native/src/hooks/useProtonInstalls.ts`](/src/crosshook-native/src/hooks/useProtonInstalls.ts).

## Relevant Docs

**`/AGENTS.md`**: You _must_ read this when working on architecture placement, IPC naming conventions, and persistence boundaries.

**`/docs/plans/protonup-integration/feature-spec.md`**: You _must_ read this when implementing accepted decisions, phased rollout, and file impact scope.

**`/docs/plans/protonup-integration/research-external.md`**: You _must_ read this when integrating provider APIs, release metadata, and integrity strategy.

**`/docs/plans/protonup-integration/research-ux.md`**: You _must_ read this when implementing launch-time prompts, install state design, and recovery messaging.

## Security Considerations

- Treat provider/version/user-selected install inputs as untrusted at the IPC boundary and validate/allowlist before execution.
- Enforce path canonicalization and root-prefix checks so installs cannot write outside intended compatibility tool directories.
- Require checksum verification prior to marking install success.
- Keep recommendation mismatch advisory by default and do not expand hard launch blocking beyond existing invalid-path validation rules.

## Reuse Opportunities

- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: Extend existing runtime discovery post-install refresh instead of duplicating local scan logic.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: Reuse cache storage/TTL semantics for available-version catalogs.
- `/src/crosshook-native/src/hooks/useProtonInstalls.ts`: Extend hook patterns for ProtonUp state instead of adding direct `invoke()` calls in components.
- Keep provider adapter implementation feature-local under a new protonup module until multiple providers require wider abstraction.
