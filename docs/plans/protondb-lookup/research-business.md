# Business Research: protondb-lookup

## Executive Summary

The core user value is reducing context switching during profile editing: if a profile already knows its Steam App ID, the user should immediately see how Linux and Steam Deck players rate the game and what common tweaks are being recommended. The feature must remain advisory, non-blocking, and offline-tolerant; a ProtonDB outage cannot become a new reason that profile editing or saving fails. Because CrossHook already tracks version intelligence and launch optimizations, ProtonDB guidance should complement those systems rather than bypass them with opaque raw command strings.

### User Stories

- As a profile editor user, I want to see the current ProtonDB tier for a game after I enter or auto-populate its Steam App ID so that I can decide whether the profile needs extra attention.
- As a Steam Deck or Linux user, I want to see the most common ProtonDB tweak guidance without leaving CrossHook so that I can compare community advice against my current launch settings.
- As an offline or bandwidth-limited user, I want cached ProtonDB data to remain visible when the network is unavailable so that the feature is still useful after the first lookup.
- As a maintainer, I want the integration to degrade gracefully when ProtonDB changes so that CrossHook keeps working even if the external service is unstable.

### Business Rules

- Lookup eligibility: ProtonDB lookup is only meaningful when `steam.app_id` is present and non-empty.
- Non-blocking behavior: missing network, invalid remote payloads, or ProtonDB downtime must not block `profile_save`, `profile_load`, or normal form editing.
- Advisory behavior: ProtonDB tweaks are suggestions. CrossHook must not silently apply launch options or mutate the profile without explicit user action.
- Exact terminology: user-facing rating output should preserve ProtonDB’s exact tier names where available instead of collapsing everything into `working` or `partial`.
- Offline behavior: cached results may be shown when stale, but stale/unavailable states must be communicated clearly.
- Existing user intent wins: if CrossHook offers an “apply recommendation” action, it must not overwrite explicit user `launch.custom_env_vars` without confirmation.

### Workflows

- Primary flow:
  - User selects or edits a profile with a known Steam App ID.
  - CrossHook loads cached ProtonDB data or fetches it on demand.
  - The profile editor shows the exact tier, freshness, and community recommendations.
  - The user either leaves the advice as informational or explicitly copies/applies a supported suggestion.
- Error recovery flow:
  - ProtonDB lookup fails or times out.
  - CrossHook surfaces a soft unavailable state and preserves the rest of the form.
  - The user can continue editing immediately and retry via explicit panel refresh.

### Domain Concepts

- Steam App ID: the stable identifier that connects a CrossHook profile to ProtonDB.
- ProtonDB tier: exact compatibility label from ProtonDB, distinct from CrossHook’s existing community `CompatibilityRating`.
- Recommendation snapshot: normalized, cacheable summary of remote rating plus aggregated suggestions.
- Fresh vs stale vs unavailable: user-facing state categories that determine whether the UI shows live results, cached results, or a soft failure message.

### Success Criteria

- A profile with a known Steam App ID shows an exact ProtonDB tier and freshness state in the editor.
- Cached ProtonDB data remains visible when the user is offline or ProtonDB is temporarily unavailable.
- Recommendation apply/copy flows never overwrite explicit user settings without clear intent.
- Remote failures remain informational and do not break save/load/edit flows.

### Resolved Scope and Product Decisions

- **Community-tweak depth**: issue `#53` uses recent/common normalized suggestions and does not implement an exhaustive historical report browser.
- **Panel eligibility**: the panel appears for any profile with a meaningful non-empty Steam App ID, not only `steam_applaunch` / `proton_run`.
- **Issue boundary**: issue `#53` is scoped to editor/runtime advisory guidance; TOML/community-profile import extensions are out of scope for this plan and are tracked separately.
- **Conflict handling**: recommendation apply actions require explicit per-key overwrite confirmation when `launch.custom_env_vars` keys collide.
