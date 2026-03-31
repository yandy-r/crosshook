# Recommendations Research: protondb-lookup

## Executive Summary

The safest high-confidence implementation is to build the feature around the stable ProtonDB summary endpoint first, while encapsulating richer recommendation aggregation behind a fallback-aware backend module so the editor still works when the undocumented report feed shifts. CrossHook should not expand `CompatibilityRating` to impersonate ProtonDB’s exact scale; it should add a dedicated exact-tier contract and map only when a lossy compatibility grouping is truly needed. The resulting UX should be advisory and explicit: cache-first, copy/apply based, and resilient to remote failures.

### Recommended Implementation Strategy

- Create a feature-local `crosshook-core::protondb` module with a normalized lookup result.
- Reuse the metadata store’s generic external cache table instead of creating a new schema up front.
- Keep the Tauri surface thin and typed.
- Use a dedicated UI component in the profile editor so the feature can evolve without destabilizing the rest of the form.
- Treat recommendation application as a controlled merge into existing profile fields, not as raw command injection.

### Phased Rollout Suggestion

- Phase 1: exact tier contract, stable summary fetch, cache behavior, and thin IPC
- Phase 2: profile-editor card, freshness states, and source-link UX
- Phase 3: normalized suggestion apply/copy actions with explicit per-key overwrite confirmation
- Phase 4: version-correlation context integration (`#41`) and stale-guidance messaging
- Phase 5: metadata handoff readiness (`#52`) with Steam App ID and cache namespace guarantees
- Phase 6: preset-promotion lifecycle for stable accepted recommendations

### Quick Wins

- Reuse `external_cache_entries` immediately instead of inventing a ProtonDB-specific table.
- Add exact-tier badge styles without touching the existing community compatibility badge semantics.
- Use existing `SteamLaunchOptionsPanel` and `CustomEnvironmentVariablesSection` patterns for copy/apply actions.

### Integrated Enhancement Tracks

- **Issue `#41` track**: ProtonDB panel state includes version-correlation context and stale-guidance messaging as a first-class behavior.
- **Issue `#52` track**: metadata/cover-art integration reuses `#53` Steam App ID and cache provenance contracts without duplicating lookup logic.
- **Preset track**: repeated safe recommendation acceptance flows into reviewable preset candidates with explicit rollback guidance.

### Risk Mitigations

- Build summary lookup so it still succeeds when richer report scraping is unavailable.
- Never execute or auto-apply raw ProtonDB launch strings; whitelist supported `KEY=value` env suggestions and keep the rest copy-only.
- Enforce explicit timeouts and stale-cache fallback so remote instability remains a soft failure.
- Keep raw payloads small or normalize aggressively to stay within the metadata cache size cap.

### Resolved Decision Summary

- Summary-first behavior is mandatory; richer report aggregation cannot block core exact-tier lookup.
- `launch.custom_env_vars` collisions require explicit per-key overwrite confirmation.
- ProtonDB panel display is exact-tier-first; any legacy compatibility grouping is derived/internal only.
