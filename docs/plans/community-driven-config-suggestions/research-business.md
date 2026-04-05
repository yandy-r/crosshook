# ML-Assisted Configuration — Business Analysis

## Executive Summary

CrossHook already has a working ProtonDB integration: `protondb::lookup_protondb` fetches summary + report-feed data, parses env-var suggestions from raw launch strings, groups them by frequency, and surfaces them in the profile editor UI where users can one-click apply them to `launch.custom_env_vars`. The "ML-assisted configuration" feature (issue #77) extends this existing pipeline in one concrete way for v1: mapping extracted env-var suggestions to entries in the existing `optimization_catalog` so that CrossHook can suggest enabling optimization toggles—not just raw env vars. Note-text scanning of freeform `concluding_notes` fields is deliberately deferred to v2; false positive risk (negation patterns, version-specific caveats) is too high before the env-var suggestion flow is proven. No net-new storage model is needed at v1; the existing `external_cache_entries` table already handles caching at a 6-hour TTL and serves stale data offline.

---

## User Stories

### Primary Users

Linux gamers who use CrossHook to launch Windows games through Proton/Wine with trainer support. They currently:

1. Find a game that crashes or has poor performance under Proton.
2. Open ProtonDB, Reddit, or PCGamingWiki in a browser.
3. Manually read multiple reports to find env-var patterns like `PROTON_USE_WINED3D=1`, `MANGOHUD=1`, or launch flags like `PROTON_LOG=1`.
4. Copy options to a text editor, then manually enter them in the CrossHook profile editor under `custom_env_vars`.

This manual loop requires context-switching out of CrossHook, reading unstructured community text, and pattern-matching across multiple reports.

### User Stories

**US-1 — Automated suggestion delivery**
_As a gamer setting up a new profile, I want CrossHook to automatically show me the most commonly used ProtonDB launch configurations for this game, so I don't have to search Reddit or ProtonDB myself._

**US-2 — Optimization toggle suggestions**
_As a gamer, I want CrossHook to suggest enabling specific optimization catalog options (e.g., WINED3D, ESYNC, FSYNC) when community reports show those options working, so I can apply them with one click rather than manually entering env vars._

**US-3 — Conflict resolution**
_As a gamer who already has custom env vars set, I want CrossHook to detect conflicts before overwriting my values and let me choose per-key what to keep, so I don't lose settings I set intentionally._

**US-4 — Offline access to cached suggestions**
_As a gamer without internet access, I want CrossHook to show me previously fetched ProtonDB suggestions from cache and clearly indicate when those suggestions might be stale, so I can still make informed decisions offline._

**US-5 — Suggestion feedback**
_As a gamer who tried a suggestion and it didn't help, I want a way to dismiss it so it stops cluttering the UI, even if CrossHook can't always know whether the game actually launched successfully._

**US-6 — Confidence transparency**
_As a gamer, I want to see how many reports back a given suggestion so I can judge its reliability (e.g., "Seen in 12 reports" is more trustworthy than "Seen in 1 report")._

---

## Business Rules

### Core Rules

**BR-1 — Cache TTL**
ProtonDB API responses are cached in `external_cache_entries` with a 6-hour TTL (`CACHE_TTL_HOURS = 6` in `client.rs`). This single cache entry (`protondb:<app_id>`) stores the full `ProtonDbLookupResult` including all `recommendation_groups[*].env_vars`. Suggestion data is derived from this cached result in memory at display time; no separate suggestion cache entry is needed.

**BR-3 — Payload size limit**
`external_cache_entries` silently stores `NULL payload_json` for payloads exceeding 512 KiB (`MAX_CACHE_PAYLOAD_BYTES`). Aggregated suggestion JSON is small (a few KB), so this is not a concern unless raw report feed data is cached separately.

**BR-4 — Env-var safety filtering**
`safe_env_var_suggestions()` enforces: SCREAMING*SNAKE_CASE keys only, no null bytes in values, no shell-special characters (`$;\"'\\` etc.), reserved keys (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `STEAM_COMPAT*\*`) are rejected. This rule must apply to all env vars extracted by any expanded logic. (Note-text scanning is deferred to v2 — see Open Question 1.)

**BR-5 — Suggestion grouping by frequency**
Suggestions are grouped by their env-var signature (sorted `KEY=VALUE` pairs joined by newline). Groups are ranked by report count, descending. The top 3 env-var groups, top 3 copy-only launch groups, and top 4 note groups are surfaced (constants `MAX_ENV_GROUPS`, `MAX_LAUNCH_GROUPS`, `MAX_NOTE_GROUPS`).

**BR-6 — Optimization catalog matching**
An extracted env var should be cross-referenced against the `optimization_catalog` to find matching entries where `entry.env` contains `[key, value]`. The match must be on the full `[key, value]` pair, not key alone. When a match is found, the suggestion should indicate the catalog entry name and allow toggling the optimization instead of (or in addition to) setting a raw env var. The `OptimizationCatalog.allowed_env_keys` HashSet provides O(1) pre-screening before `find_by_id` is called.

**BR-7 — Conflict detection before apply**
When a user applies ProtonDB env-var suggestions, the system compares suggested keys against existing `launch.custom_env_vars`. If a key already exists with a different value, the user is shown a per-key conflict resolution UI before any change is committed. Keys that match exactly (`existingValue === envVar.value`) are treated as unchanged and silently skipped.

**BR-8 — Accepted suggestions persist to profile TOML**
Accepted env vars are written into `launch.custom_env_vars` in the profile TOML file. Accepted optimization toggles are written to `launch.optimizations.enabled_option_ids`. No intermediate "accepted suggestion" table is needed; the profile itself is the record of acceptance.

**BR-9 — Dismissed suggestions**
A user dismissing a suggestion (without applying it) should suppress it for that profile. Dismissed suggestion IDs should be stored per profile with an optional expiry (recommended: 30 days) to prevent permanent suppression of high-value suggestions as community data evolves. This is operational metadata → SQLite, not user preferences → TOML. The `protondb_dismissed_suggestions` table must use `ON DELETE CASCADE` on the `profile_id` foreign key so dismissed records are cleaned up when a profile is deleted.

**BR-10 — Offline behavior**
When the live ProtonDB API is unreachable, the system falls back to stale cache (`load_cached_lookup_row(allow_expired = true)`). The UI must clearly indicate staleness with a banner and the cache age. The feature is non-blocking; the rest of the profile editor remains fully usable when ProtonDB is unavailable.

**BR-11 — Force refresh**
Users can trigger a manual refresh (`forceRefresh = true`) which bypasses the cache and fetches live data regardless of TTL.

**BR-12 — Suggestion generation on demand**
Suggestions are generated from cached or freshly fetched report data at the time of UI display. There is no background job or scheduled aggregation step at v1.

**BR-13 — Steam App ID required**
ProtonDB lookup and suggestions are only shown when the profile has a valid `steam.app_id` (or `runtime.steam_app_id`). The UI already gates on `launchMethod === 'steam_applaunch' || launchMethod === 'proton_run'`.

**BR-14 — No auto-apply (MUST NOT)**
Suggestions MUST NEVER be applied to a profile without explicit user action (a button click). Auto-applying extracted env vars from untrusted community text is a security and correctness violation. The UI must always present suggestions as optional and require deliberate user confirmation before writing to profile state.

**BR-15 — Cached ProtonDB data must not be exported**
ProtonDB data is published under the Open Database License (ODbL). CrossHook caches raw API responses in `external_cache_entries` for local use. This cached data must never be included in profile exports, community tap distributions, or any shareable artifact. Profile TOML exports contain only user-accepted values (written to `custom_env_vars` / `enabled_option_ids`) — not the source ProtonDB payload. This constraint is already satisfied architecturally but must be preserved in any future export or sharing feature.

**BR-16 — Suggestion provenance tracking**
Accepted suggestions should have their origin recorded for auditability (so users know which keys came from ProtonDB). TOML comments are not viable for this purpose: `GameProfile` is serialized via `serde` to `BTreeMap<String, String>` and round-trips do not preserve comments. Provenance is tracked instead in SQLite via an `protondb_applied_suggestions` table (`profile_id`, `group_id`, `applied_keys: JSON`, `applied_at`). The UI can surface this in a "suggestion history" view within the profile editor. This table is optional for v1 but the constraint against TOML comments must be documented to prevent future attempts.

### Edge Cases

**EC-1 — No ProtonDB entry for game**
The summary API returns 404. The system produces an `Unavailable` state. No suggestions are shown. The UI shows a "No community data available" message.

**EC-2 — Game exists in ProtonDB but has zero reports**
Summary returns with `total_reports = 0` or null. Report feed may be empty. No suggestion groups are generated. The `degraded_recommendation_group()` fallback is shown.

**EC-3 — All reports use copy-only launch strings**
No env vars can be parsed (no `KEY=VALUE` pattern before `%command%`). Only copy-only launch groups are shown. No optimization catalog matches are attempted.

**EC-4 — Report feed hash mismatch**
The `report_feed_id` hash algorithm produces an ID that returns 404 on first try. The code retries with refreshed counts data once. If both fail, `HashResolutionFailed` error is returned and a degraded group is shown.

**EC-5 — Env-var suggestion matches reserved key**
Filtered out by `safe_env_var_suggestions()` — never shown to the user.

**EC-6 — Profile has no Steam App ID**
ProtonDB panel is hidden entirely. The user must first set a Steam App ID in the profile before suggestions can be loaded.

**EC-7 — Conflict where suggested value matches existing value**
Treated as `unchangedKeys` — not shown in conflict UI, silently skipped. No overwrite occurs.

**EC-8 — Cache payload NULL (over 512 KiB)**
The aggregated suggestion payload will never approach this limit (typically < 5 KiB). If raw report feed data is cached in future, size must be validated before storage.

**EC-9 — Optimization catalog not loaded**
If the catalog fails to load (corrupt TOML or database error), catalog matching is skipped; env-var suggestions are still shown as raw env vars. The feature degrades gracefully.

**EC-10 — Dismissed suggestion reappears after cache refresh**
Dismissed suggestion IDs are stored per profile in SQLite. After a cache refresh that changes what the top suggestions are, previously dismissed IDs may no longer match any current suggestion (the suppression entry is a no-op). If the same suggestion reappears with a new `group_id`, it will not be suppressed unless the dismissed record matches.

**EC-11 — Catalog entry removed after suggestion was matched**
If a catalog entry ID referenced by a suggestion is later removed from the catalog, the `find_by_id` lookup returns `None`. The suggestion must degrade to raw env-var apply mode rather than showing a broken "Enable [unknown]" button. Any provenance records referencing the stale catalog ID remain valid (the user did accept the suggestion at the time).

---

## Workflows

### Primary Workflow — Suggestion Display During Profile Edit

```
1. User opens a profile in the profile editor (LaunchPage / ProfileFormSections)
2. Frontend checks launchMethod ∈ {steam_applaunch, proton_run}
3. If true, ProtonDbLookupCard mounts and calls useProtonDbLookup(appId)
4. Hook invokes `protondb_lookup` Tauri command (force_refresh = false)
5. Backend: normalize_app_id → cache_key_for_app_id → load_cached_lookup_row
   a. If valid cache hit → return cached result (state = "ready", from_cache = true)
   b. If no valid cache → fetch_live_lookup → fetch_summary + fetch_recommendations
      i. normalize_report_feed() → ProtonDbRecommendationGroup[]
      ii. safe_env_var_suggestions() filters env vars per report
      iii. Groups ranked by frequency, capped at MAX_ENV/LAUNCH/NOTE_GROUPS
   c. persist_lookup_result → external_cache_entries (TTL = now + 6h)
   d. If network fails → load_cached_lookup_row(allow_expired = true) → state = "stale"
   e. If no cache and network fails → state = "unavailable"
6. Frontend renders ProtonDbLookupCard:
   a. Header: tier badge, score, confidence, total_reports, freshness
   b. Banner: staleness/offline/cache notice
   c. Community recommendations: env-var groups, copy-only groups, note groups
   d. Each env-var group has "Apply Suggested Env Vars" button and per-var "Copy" button
7. User clicks "Apply Suggested Env Vars" on a group
   → handleApplyProtonDbEnvVars → mergeProtonDbEnvVarGroup
   a. No conflicts → apply immediately to profile state, show status message
   b. Conflicts exist → show ProtonDbOverwriteConfirmation dialog
      i. User selects per-key: "Keep current" or "Use suggestion"
      ii. User clicks "Apply selected changes"
      iii. applyProtonDbGroup applies only user-confirmed keys
8. Profile state is now updated (in memory). User must save to persist to TOML.
```

### Extended Workflow — Optimization Catalog Matching (New for This Feature)

```
1. After normalize_report_feed() produces env-var suggestion groups:
2. For each ProtonDbEnvVarSuggestion in each group:
   a. Look up optimization_catalog entries where entry.env contains [key, value]
   b. If match found, attach catalog entry ID and label to the suggestion
3. In ProtonDbLookupCard, if a suggestion has a matched catalog entry:
   a. Show "Enable [Optimization Label]" button alongside "Copy"
   b. Clicking applies entry.id to launch.optimizations.enabled_option_ids
   c. Clicking does NOT add the raw env var to custom_env_vars (catalog manages it)
4. If no catalog match, fall back to raw env-var application (existing behavior)
5. If catalog lookup returns None (entry removed), fall back to raw env-var apply
```

### Error Recovery Workflow

```
1. Live fetch fails (network error / 404 / hash mismatch)
2. Backend attempts stale cache load (allow_expired = true)
3. If stale cache found:
   a. Return result with state = "stale"
   b. UI shows "Showing cached ProtonDB guidance because the live lookup failed" banner
4. If no cache exists:
   a. Return result with state = "unavailable"
   b. UI shows "ProtonDB is unavailable" banner
   c. Rest of profile editor remains usable
5. User can retry via "Refresh" button (force_refresh = true)
```

### Offline Workflow

```
1. User opens profile with no network connectivity
2. Backend: live fetch fails immediately
3. Stale cache (if any) is returned with is_offline = true, is_stale = true
4. UI banner: "Showing cached ProtonDB guidance because the live lookup failed"
5. Freshness indicator shows cache age (e.g., "Updated 3 hours ago")
6. All apply/conflict/dismiss workflows remain functional on cached data
7. "Refresh" button is available but will fail again (same offline state)
```

---

## Domain Model

### Key Entities

**ProtonDbReport** (external, read-only from ProtonDB API)

- `id: String` — unique report identifier
- `timestamp: i64` — Unix timestamp of report submission
- `responses.concluding_notes: String` — freeform text advice
- `responses.launch_options: String` — raw Steam launch options string
- `responses.proton_version: String` — Proton version used
- `responses.variant: String` — Proton variant (GE, Experimental, etc.)

**ProtonDbSnapshot** (normalized, cached in `external_cache_entries`)

- `app_id: String`
- `tier: ProtonDbTier` — Platinum/Gold/Silver/Bronze/Borked/Native/Unknown
- `best_reported_tier, trending_tier: Option<ProtonDbTier>`
- `score: Option<f32>`, `confidence: Option<String>`, `total_reports: Option<u32>`
- `recommendation_groups: Vec<ProtonDbRecommendationGroup>` — aggregated from feed
- `fetched_at: String` — RFC 3339 timestamp

**ProtonDbRecommendationGroup** (in-memory, derived from report aggregation)

- `group_id: String` — stable ID for UI tracking (e.g., "supported-env-1")
- `title, summary: String`
- `env_vars: Vec<ProtonDbEnvVarSuggestion>` — parsed, filtered, safe
- `launch_options: Vec<ProtonDbLaunchOptionSuggestion>` — copy-only raw strings
- `notes: Vec<ProtonDbAdvisoryNote>` — freeform text notes

**ProtonDbEnvVarSuggestion** (in-memory)

- `key, value: String` — validated env-var pair (SCREAMING_SNAKE_CASE key, no shell-special values)
- `source_label: String` — Proton version or variant label
- `supporting_report_count: Option<u32>` — how many reports included this pair
- `catalog_entry_id: Option<String>` — (new) references `optimization_catalog.id` if matched
- `catalog_entry_label: Option<String>` — (new) human-readable name for "Enable [X]" button

**OptimizationCatalogMatch** (new, in-memory only for this feature)

- `catalog_entry_id: String` — references `optimization_catalog.id`
- `label: String` — human-readable name for UI
- `already_enabled: bool` — whether the profile already has this ID in `enabled_option_ids`

**ProtonDbCacheState** (in-memory, surfaced to UI)

- `cache_key, fetched_at, expires_at: String`
- `from_cache: bool` — response came from local cache
- `is_stale: bool` — cache TTL expired but no live fetch succeeded
- `is_offline: bool` — no live fetch attempted or possible

**DismissedSuggestion** (new, SQLite operational metadata)

- `profile_id: String` — foreign key to `profiles.profile_id` (ON DELETE CASCADE)
- `group_id: String` — matches `ProtonDbRecommendationGroup.group_id`
- `dismissed_at: String` — RFC 3339 timestamp
- `expires_at: Option<String>` — RFC 3339; recommended 30-day TTL from `dismissed_at`

**AppliedSuggestion** (new, SQLite operational metadata — optional at v1)

- `profile_id: String` — foreign key to `profiles.profile_id` (ON DELETE CASCADE)
- `group_id: String` — source recommendation group
- `applied_keys: String` — JSON array of env-var keys accepted
- `applied_at: String` — RFC 3339 timestamp

### State Transitions

```
ProtonDbLookupState:
  idle → loading (app_id set, lookup triggered)
  loading → ready (live fetch or valid cache succeeded)
  loading → stale (live fetch failed, stale cache used)
  loading → unavailable (live fetch failed, no cache)
  ready → loading (user clicks Refresh)
  stale → loading (user clicks Refresh)
  unavailable → loading (user clicks Refresh)
```

```
Suggestion Application:
  shown → applying (user clicks "Apply Suggested Env Vars")
  applying → conflict_resolution (conflicts detected)
  applying → applied (no conflicts)
  conflict_resolution → applied (user confirms selections)
  conflict_resolution → cancelled (user cancels)
  shown → dismissed (user clicks dismiss — new state for this feature)
```

---

## Existing Codebase Integration

### What Already Exists (No Change Needed)

| Component                                   | Location                                       | Role                                                  |
| ------------------------------------------- | ---------------------------------------------- | ----------------------------------------------------- |
| `lookup_protondb()`                         | `protondb/client.rs:85`                        | Entry point: cache check → live fetch → persist       |
| `normalize_report_feed()`                   | `protondb/aggregation.rs:81`                   | Converts report feed to recommendation groups         |
| `safe_env_var_suggestions()`                | `protondb/aggregation.rs:260`                  | Safety-filters raw launch strings to env vars         |
| `external_cache_entries` table              | schema v4                                      | Generic HTTP response cache; already used by ProtonDB |
| `ProtonDbLookupCard`                        | `components/ProtonDbLookupCard.tsx`            | Full UI for showing tier + recommendations            |
| `ProtonDbOverwriteConfirmation`             | `components/ProtonDbOverwriteConfirmation.tsx` | Conflict resolution dialog (already built)            |
| `mergeProtonDbEnvVarGroup()`                | `utils/protondb.ts`                            | Merge logic with conflict detection                   |
| `useProtonDbLookup()`                       | `hooks/useProtonDbLookup.ts`                   | React hook for lookup state management                |
| `protondb_lookup` command                   | `commands/protondb.rs`                         | Tauri IPC command                                     |
| `optimization_catalog` table                | schema v12                                     | Stores optimization entries with env arrays           |
| `OptimizationCatalog` / `OptimizationEntry` | `launch/catalog.rs`                            | In-memory catalog with `allowed_env_keys` index       |
| `launch.custom_env_vars`                    | `profile/models.rs:337`                        | Profile TOML field where accepted env vars land       |
| `launch.optimizations.enabled_option_ids`   | `profile/models.rs:86`                         | Profile TOML field for active optimization toggles    |

### What Needs Extension

| Area                                                       | Change Required                                                                                                                                                                                              |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `normalize_report_feed()`                                  | Add catalog matching pass after env-var extraction (v1 scope only); attach `catalog_entry_id`/`catalog_entry_label` to matched suggestions. Note-text scanning of `concluding_notes` is out of scope for v1. |
| `ProtonDbRecommendationGroup` / `ProtonDbEnvVarSuggestion` | Add optional `catalog_entry_id` and `catalog_entry_label` fields                                                                                                                                             |
| `ProtonDbLookupCard`                                       | Render "Enable [Optimization]" button for catalog-matched suggestions                                                                                                                                        |
| Profile form / apply flow                                  | When catalog match is applied, write to `enabled_option_ids`, not `custom_env_vars`                                                                                                                          |
| New SQLite table                                           | `protondb_dismissed_suggestions` (schema v14) with `ON DELETE CASCADE`                                                                                                                                       |
| New SQLite table (optional v1)                             | `protondb_applied_suggestions` for provenance tracking                                                                                                                                                       |
| Schema migration                                           | v14 adding `protondb_dismissed_suggestions`; v14 or v15 for `protondb_applied_suggestions`                                                                                                                   |
| New Tauri commands                                         | `protondb_dismiss_suggestion(profile_id, group_id)` and `protondb_get_dismissed_suggestions(profile_id)`                                                                                                     |

### Storage Boundary Classification

| Datum                                                        | Layer                                                         | Rationale                                                                    |
| ------------------------------------------------------------ | ------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Raw ProtonDB API response (snapshot + recommendation groups) | SQLite `external_cache_entries`                               | Operational cache with TTL; already implemented                              |
| Dismissed suggestion records                                 | SQLite new table `protondb_dismissed_suggestions`             | Operational metadata tied to profile lifecycle; cascade-deleted with profile |
| Applied suggestion provenance                                | SQLite new table `protondb_applied_suggestions` (optional v1) | Audit trail; cascade-deleted with profile; NOT exported                      |
| Accepted env vars                                            | Profile TOML `launch.custom_env_vars`                         | User-editable preferences; existing field                                    |
| Accepted optimization toggles                                | Profile TOML `launch.optimizations.enabled_option_ids`        | User-editable preferences; existing field                                    |
| Active suggestion state (loading, conflicts)                 | In-memory React state                                         | Ephemeral UI state; not persisted                                            |

### Constraints

- **ODbL compliance**: Cached ProtonDB data in `external_cache_entries` must never be included in profile exports, community tap distributions, or any user-shareable artifact. Only user-accepted values (written to profile TOML fields) may be exported.
- **No TOML comments for provenance**: `GameProfile` serialization via `serde` does not preserve TOML comments. Source attribution must use SQLite (`protondb_applied_suggestions`), not inline TOML comments.

---

## Success Criteria

1. **Suggestion accuracy**: At least the top env-var pattern from ProtonDB reports is surfaced in the UI for games that have community reports (verifiable by comparing to ProtonDB web UI).
2. **Catalog coverage**: Common optimization catalog entries (ESYNC, FSYNC, WINED3D) are suggested via toggle when matching env vars appear in reports.
3. **Conflict safety**: No existing `custom_env_vars` value is silently overwritten — conflicts always prompt the user.
4. **Offline resilience**: Cached suggestions are available offline with a staleness indicator.
5. **No regression**: Existing "Apply Suggested Env Vars" flow for env-var groups continues to work exactly as before.
6. **Performance**: Suggestion generation adds no perceptible latency on top of the existing ProtonDB fetch.
7. **No auto-apply**: Zero code paths exist that write to profile state without an explicit user action.

---

## Open Questions

1. **Note-text parsing scope**: `concluding_notes` is freeform prose. **Decided (v1): deferred.** Adding regex-based extraction from note text before the env-var suggestion flow is proven would be scope expansion. Notes are already surfaced as `ProtonDbAdvisoryNote` for copy-read. Flag for v2 when frequency-ranked env-var suggestions have shipped and the false-positive risk (negation patterns, version-specific caveats) can be quantified.
2. **Dismissed suggestion TTL**: Should dismissed suggestions auto-expire (e.g., after 30 days) to allow rediscovery when community reports change, or persist indefinitely?
3. ~~**Suggestion TTL independence**~~ **Decided (v1): resolved.** No separate cache key for suggestions. The existing `protondb:<app_id>` entry already stores the full `ProtonDbLookupResult` including all `recommendation_groups[*].env_vars`. Suggestions are derived from that cached result in memory; a separate key would require dual invalidation for data already present. The 6-hour TTL applies to the single cache entry.
4. **Catalog version drift**: If a suggestion was matched to a catalog entry ID that is later removed from the catalog, how should the UI handle the stale reference? (Proposed: degrade to raw env-var apply mode.)
5. **Feedback signal**: Without a "did this help?" success signal, confidence scoring is based solely on report frequency. Is frequency sufficient for v1 trust signaling, or should a thumbs-up/down mechanism be scoped in?
6. **Launch option apply**: Copy-only launch strings (e.g., `gamemoderun %command%`) cannot be safely parsed into env vars. Should v1 surface a "Set as Steam launch options" action, or remain copy-only?
7. **Provenance at v1**: Should `protondb_applied_suggestions` be included in schema v14 or deferred to a follow-up? Including it at v1 avoids a future migration to backfill provenance for already-applied suggestions.
8. **Catalog coverage audit**: Before shipping the "Enable [Optimization]" path, the catalog entries in `assets/default_optimization_catalog.toml` should be audited against the most common ProtonDB env-var patterns to quantify how many suggestions will have catalog matches vs. raw env-var fallback.
