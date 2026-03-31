# ProtonDB Lookup

CrossHook already has the key pieces needed for a backend-owned ProtonDB integration: Steam App IDs live on profiles, the metadata SQLite store already has a generic external-cache table, and the profile editor already groups Steam discovery, version hints, and launch-setting controls in one place. The new feature should slot into that path by adding a `crosshook-core::protondb` module for cache-backed lookup and normalization, a thin Tauri IPC command, and a dedicated profile-editor card that renders exact ProtonDB tiers plus explicit copy/apply actions. The main architectural wrinkle is that CrossHook’s existing `CompatibilityRating` enum is not an exact ProtonDB scale, so the feature needs a dedicated exact-tier contract instead of forcing `gold`, `silver`, `bronze`, and `borked` into `working` or `partial`. Because ProtonDB’s richer report feed is undocumented and CORS blocks browser-side fetches, network logic must stay in the backend and degrade gracefully to summary-only results when the unstable feed cannot be resolved.

## Relevant Files

- /src/crosshook-native/crates/crosshook-core/src/lib.rs: exports core modules like `steam`, `metadata`, and `protondb`
- /src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: shared cache/store API and SQLite lifecycle
- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: generic external JSON cache with TTL support
- /src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: existing legacy compatibility enum and community metadata surface
- /src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs: Steam App ID/build parsing conventions
- /src/crosshook-native/src-tauri/src/commands/steam.rs: thin Steam command pattern for local metadata lookup
- /src/crosshook-native/src-tauri/src/commands/version.rs: thin metadata-backed command pattern and typed DTO boundary
- /src/crosshook-native/src-tauri/src/commands/mod.rs: command module registry
- /src/crosshook-native/src-tauri/src/lib.rs: invoke handler registration and shared state wiring
- /src/crosshook-native/src/components/ProfileFormSections.tsx: profile-editor Steam/App ID/custom env layout
- /src/crosshook-native/src/components/pages/ProfilesPage.tsx: selected profile context and existing health/version badges
- /src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx: copyable launch-options UX pattern to reuse
- /src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: existing safe mutation path for environment-variable apply actions
- /src/crosshook-native/src/styles/theme.css: existing compatibility badge and panel class definitions
- /docs/plans/protondb-lookup/feature-spec.md: synthesized feature-research output for this plan

## Relevant Tables

- external_cache_entries: reusable cache table for remote ProtonDB summary/recommendation snapshots
- profiles: stable profile identity mapping used by metadata and the selected editor surface
- version_snapshots: existing Steam App ID and trainer-version intelligence that sits adjacent to this feature
- community_profiles: current community compatibility metadata that still uses the legacy `CompatibilityRating` scale

## Relevant Patterns

**Core-Owned Feature Logic**: put the actual lookup/cache/normalization logic in `crosshook-core`, then keep Tauri thin. Example: [/src/crosshook-native/src-tauri/src/commands/version.rs](/src/crosshook-native/src-tauri/src/commands/version.rs).

**Metadata Cache Reuse**: use `MetadataStore` and `external_cache_entries` before adding new schema. Example: [/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs](/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs).

**Invoke + Hook Frontend State**: wrap Tauri commands in typed hooks with soft failure states instead of scattering `invoke()` calls. Example: [/src/crosshook-native/src/hooks/useProfileHealth.ts](/src/crosshook-native/src/hooks/useProfileHealth.ts).

**Feature-Local UI Composition**: build a dedicated component, then compose it into the page/form that owns the workflow. Example: [/src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx](/src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx).

**Semantic Badge Styling**: define dedicated modifier classes in `theme.css` instead of inline one-off color rules. Example: [/src/crosshook-native/src/styles/theme.css](/src/crosshook-native/src/styles/theme.css).

## Relevant Docs

**/AGENTS.md**: You _must_ read this when working on new backend modules, Tauri commands, and repo planning conventions.

**/docs/getting-started/quickstart.md**: You _must_ read this when working on the profile editor, Steam App ID auto-populate flow, or user-facing docs for the editor.

**/docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when working on Steam metadata, launch optimizations, or copy/apply launch-setting UX.

**/docs/research/additional-features/deep-research-report.md**: You _must_ read this when working on issue `#53` in the context of the original feature-priority research.

**/docs/research/additional-features/implementation-guide.md**: You _must_ read this when sequencing `#53` relative to version correlation and adjacent backlog work.

**/docs/plans/protondb-lookup/feature-spec.md**: You _must_ read this when implementing `#53`; it captures external API constraints and resolved planning decisions for execution.

## Resolved Planning Defaults

- Summary-first lookup is mandatory; richer report aggregation is best-effort and cannot block exact-tier output.
- ProtonDB tier labels are exact-tier-first in UI; legacy compatibility grouping is derived/internal only.
- Recommendation-apply conflicts use explicit per-key overwrite confirmation.
- Cache persistence is normalized DTO-only in `external_cache_entries` (no raw report payload cache rows).

## Cross-Issue Boundary

- Issue `#53` owns ProtonDB lookup, normalization, advisory rendering, and apply/copy safety behavior.
- Issue `#41` integration is included as version-correlation context inside ProtonDB panel state.
- Issue `#52` may reuse Steam App ID and cache provenance contracts from `#53`, but must not duplicate ProtonDB fetch/cache logic.

## Security Considerations

- ProtonDB `launchOptions`, notes, and tier strings are untrusted remote input; normalize them in `crosshook-core`, render them as plain text, and never inject raw launch strings directly into CrossHook’s launch pipeline.
- Backend-only fetching is mandatory because ProtonDB’s CORS response allows `https://www.protondb.com`, not the CrossHook webview origin; keep network logic and remote failure handling out of the frontend.
- Resolve the exact-tier contract and stale/unavailable state model before wiring UI mutations so ProtonDB outages stay advisory and do not become profile-validation failures.

## Reuse Opportunities

- /src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: reuse cache-keyed JSON persistence rather than creating a ProtonDB table immediately
- /src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx: reuse copy-to-clipboard launch-option presentation patterns
- /src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: reuse the existing profile mutation path for any supported env-var apply action
- /src/crosshook-native/src-tauri/src/commands/version.rs: mirror the thin command pattern and DTO boundary
- Keep the feature local to `crosshook-core::protondb` and a single profile-editor card; do not introduce a repo-wide “external service” abstraction until a second feature needs it
