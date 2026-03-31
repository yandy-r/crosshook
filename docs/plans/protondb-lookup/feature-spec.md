# Feature Spec: ProtonDB Lookup

## Executive Summary

CrossHook should enrich the profile editor with ProtonDB guidance whenever a profile already has a Steam App ID. The clean implementation is a backend-owned, cache-backed lookup service in `crosshook-core`, a thin Tauri IPC command, and a dedicated editor card that shows exact ProtonDB tiers plus normalized recommendation actions. The main risks are ProtonDB’s undocumented richer report feed and CrossHook’s lossy `CompatibilityRating` enum, so the design must preserve exact tiers, keep remote guidance advisory, and degrade gracefully to summary-only results when necessary.

## External Dependencies

### APIs and Services

#### ProtonDB Summary Endpoint

- **Documentation**: no published API documentation was found; live endpoint verified March 31, 2026 at `https://www.protondb.com/api/v1/reports/summaries/1245620.json`
- **Authentication**: none
- **Key Endpoints**:
  - `GET https://www.protondb.com/api/v1/reports/summaries/{steamAppId}.json`: fetch exact compatibility tier, confidence, score, and report count
- **Rate Limits**: not documented publicly
- **Pricing**: free

#### ProtonDB Report Feed

- **Documentation**: none found; live path discovered from the ProtonDB app page network trace
- **Authentication**: none
- **Key Endpoints**:
  - `GET https://www.protondb.com/data/reports/all-devices/app/996607738.json`: returns `page`, `perPage`, `total`, and recent report rows including `launchOptions`, `concludingNotes`, and `protonVersion`
- **Rate Limits**: not documented publicly
- **Pricing**: free
- **Important constraint**: the discovered path is not keyed directly by Steam App ID, so richer aggregation must be treated as lower-confidence than the summary endpoint

### Libraries and SDKs

| Library | Version | Purpose | Installation |
| --- | --- | --- | --- |
| `reqwest` | current stable Rust release at implementation time | backend HTTP client with TLS, JSON decode, timeout handling | add to `/src/crosshook-native/crates/crosshook-core/Cargo.toml` |
| `serde_json` | already present | normalize/cached ProtonDB payloads | already installed |

### External Documentation

- [ProtonDB summary endpoint](https://www.protondb.com/api/v1/reports/summaries/1245620.json): live shape for exact tier lookup
- [ProtonDB game page](https://www.protondb.com/app/1245620): source page for manual verification and outbound “open in ProtonDB” links
- [ProtonDB report feed sample](https://www.protondb.com/data/reports/all-devices/app/996607738.json): live example of richer report data discovered from the app page network trace

## Business Requirements

### User Stories

**Primary User: Profile editor user**

- As a profile editor user, I want CrossHook to show the current ProtonDB tier as soon as a Steam App ID is present so that I can assess Linux compatibility without leaving the app.
- As a profile editor user, I want CrossHook to surface common ProtonDB tweaks as explicit suggestions so that I can compare them against my current launch settings.
- As a profile editor user, I want cached ProtonDB data to remain visible when I am offline so that the feature is still useful after the first lookup.

**Secondary User: Maintainer**

- As a maintainer, I want the feature to fail softly when ProtonDB changes so that external instability does not break profile editing.

### Business Rules

1. **Lookup Eligibility**: ProtonDB lookup only runs when `steam.app_id` is non-empty.
   - Validation: empty App ID returns an idle/not-configured state, not an error.
   - Exception: cached data may still be shown for a previously resolved App ID while the user is editing other fields.

2. **Advisory-Only Recommendations**: ProtonDB suggestions must never silently mutate a profile.
   - Validation: any “apply” action must be explicit and merge into existing CrossHook fields safely.
   - Exception: none; silent mutation would violate trust.

3. **Remote Failures Stay Soft**: ProtonDB unavailability must not block save/load/edit flows.
   - Validation: lookup errors surface only inside the ProtonDB panel state.
   - Exception: none.

4. **Exact Tier Fidelity**: user-facing ProtonDB tiers must preserve values such as `gold`, `silver`, `bronze`, and `borked`.
   - Validation: use a dedicated exact-tier contract instead of forcing `CompatibilityRating`.
   - Exception: lossy mapping is allowed only in derived/internal grouping logic.

### Edge Cases

| Scenario | Expected Behavior | Notes |
| --- | --- | --- |
| Empty Steam App ID | Show neutral “add a Steam App ID to enable ProtonDB” state | No network call |
| ProtonDB summary fetch fails but cache exists | Show cached result with stale label and retry action | Offline-tolerant |
| ProtonDB returns unknown/new tier | Show exact raw tier text if safe, plus generic fallback styling | Avoid hard crash |
| Recommendation includes unsupported raw command fragments | Keep the suggestion copy-only | Never inject blindly |
| Existing custom env vars conflict with suggested env vars | Require explicit overwrite/merge handling | User intent wins |

### Success Criteria

- [ ] The profile editor shows an exact ProtonDB tier for profiles with a known Steam App ID.
- [ ] Results are cached locally in the metadata DB and can be shown during offline/stale states.
- [ ] ProtonDB unavailability produces a soft panel-level failure, not a broken profile form.
- [ ] Supported recommendations can be copied or explicitly applied without unsafe raw command injection.
- [ ] The feature stays consistent with CrossHook’s existing Steam/version/profile architecture and does not move business logic into the frontend.
- [ ] ProtonDB guidance includes version-correlation context from issue `#41` so users can distinguish stable recommendations from likely stale advice after game updates.
- [ ] Steam App ID ownership, cache namespaces, and DTO boundaries are stable and documented for issue `#52` reuse without data-model duplication.
- [ ] Repeatedly accepted safe recommendations have a documented promotion path into reusable preset candidates with explicit user review.

## Technical Specifications

### Architecture Overview

```text
ProfileFormSections / ProfilesPage
            |
            v
   useProtonDbLookup hook
            |
            v
  Tauri command: protondb_lookup
            |
            v
 crosshook-core::protondb
   |                |
   |                +--> ProtonDB summary endpoint
   |
   +--> MetadataStore external_cache_entries
```

`crosshook-core::protondb` owns fetch, normalization, fallback, and cache usage. `src-tauri` only translates IPC arguments and returns typed DTOs. The frontend only consumes typed lookup state and renders advisory actions near the existing Steam metadata inputs.

### Data Models

#### `NormalizedProtonDbSnapshot`

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `app_id` | `String` | non-empty | Steam App ID used for lookup |
| `tier` | `ProtonDbTier` | exact remote tier | exact current ProtonDB tier |
| `best_reported_tier` | `Option<ProtonDbTier>` | nullable | best tier seen in remote payload |
| `trending_tier` | `Option<ProtonDbTier>` | nullable | trend indicator from remote payload |
| `score` | `Option<f32>` | nullable | aggregated ProtonDB score |
| `confidence` | `Option<String>` | nullable | confidence label |
| `total_reports` | `Option<u32>` | nullable | report count |
| `recommendations` | `Vec<ProtonDbRecommendation>` | compact | normalized apply/copy suggestions |
| `source_url` | `String` | non-empty | ProtonDB page or JSON source |
| `fetched_at` | `String` | RFC 3339 | cache freshness anchor |

**Indexes:**

- reuse existing `external_cache_entries.cache_key` unique index

**Relationships:**

- keyed by Steam App ID and stored in `external_cache_entries` rather than a dedicated ProtonDB table initially

#### `external_cache_entries`

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `cache_id` | `TEXT` | PK | row identifier |
| `source_url` | `TEXT` | required | upstream source reference |
| `cache_key` | `TEXT` | unique | namespaced ProtonDB cache key |
| `payload_json` | `TEXT` | nullable | normalized cached payload |
| `payload_size` | `INTEGER` | capped | payload size for safety |
| `fetched_at` | `TEXT` | required | last successful fetch timestamp |
| `expires_at` | `TEXT` | nullable | stale threshold |

**Indexes:**

- existing unique key on `cache_key`

**Relationships:**

- reused generic cache table already exposed through `MetadataStore`

### API Design

#### `[TAURI COMMAND] protondb_lookup`

**Purpose**: resolve cached-or-live ProtonDB data for a Steam App ID
**Authentication**: not required

**Request:**

```json
{
  "appId": "1245620",
  "forceRefresh": false
}
```

**Response (200):**

```json
{
  "app_id": "1245620",
  "state": "ready",
  "from_cache": true,
  "stale": false,
  "snapshot": {
    "tier": "gold",
    "best_reported_tier": "platinum",
    "trending_tier": "platinum",
    "score": 0.78,
    "confidence": "strong",
    "total_reports": 2004,
    "recommendations": []
  }
}
```

**Errors:**

| Status | Condition | Response |
| --- | --- | --- |
| soft-state | empty app id | idle/not-configured payload rather than hard error |
| soft-state | remote timeout/unavailable | unavailable payload, optionally with stale cache |
| soft-state | undocumented rich report feed unavailable | summary-only payload plus degraded recommendation state |

### System Integration

#### Files to Create

- `/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs`: feature-local ProtonDB module entry point
- `/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: exact-tier and normalized DTO definitions
- `/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: cache-backed fetch logic
- `/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs`: safe recommendation normalization
- `/src/crosshook-native/src-tauri/src/commands/protondb.rs`: thin IPC handler
- `/src/crosshook-native/src/types/protondb.ts`: mirrored TS contracts
- `/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: invoke-driven lookup state hook
- `/src/crosshook-native/src/components/ProtonDbLookupCard.tsx`: profile-editor UI surface

#### Files to Modify

- `/src/crosshook-native/crates/crosshook-core/Cargo.toml`: add HTTP client dependency
- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: export ProtonDB module
- `/src/crosshook-native/src-tauri/src/commands/mod.rs`: register ProtonDB command module
- `/src/crosshook-native/src-tauri/src/lib.rs`: add the Tauri command to the invoke handler
- `/src/crosshook-native/src/components/ProfileFormSections.tsx`: place the new card near Steam metadata
- `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: provide selected profile context and refresh wiring
- `/src/crosshook-native/src/styles/theme.css`: add exact-tier badge/panel styling
- `/src/crosshook-native/src/types/index.ts`: export ProtonDB frontend types

#### Configuration

- `protondb:summary:v1:{appId}`: metadata cache key for stable tier lookup
- `protondb:recommendations:v1:{appId}`: metadata cache key for normalized tweak suggestions
- `User-Agent`: explicit backend request header identifying CrossHook

## UX Considerations

### User Workflows

#### Primary Workflow: Profile Editor Guidance

1. **Steam App ID Present**
   - User: selects a profile or fills/auto-populates the Steam App ID
   - System: starts a ProtonDB lookup and shows an inline loading state

2. **Review Guidance**
   - User: reads the exact ProtonDB tier and community guidance
   - System: shows exact tier, report volume, freshness, and recommendation actions

3. **Take Action**
   - User: copies launch options, applies supported env vars, or ignores the panel
   - System: keeps all actions explicit and non-destructive

4. **Success State**
   - User: leaves the editor with clearer compatibility context and no forced workflow changes

#### Error Recovery Workflow

1. **Error Occurs**: ProtonDB times out, returns invalid JSON, or richer report aggregation is unavailable
2. **User Sees**: a soft unavailable or stale-cache state inside the ProtonDB panel
3. **Recovery**: continue editing immediately, optionally retry, optionally open the full ProtonDB page externally

### UI Patterns

| Component | Pattern | Notes |
| --- | --- | --- |
| ProtonDB lookup card | advisory info panel | keep near Steam metadata |
| Exact tier badge | dedicated ProtonDB badge styling | do not overload legacy compatibility styles |
| Recommendation actions | explicit copy/apply buttons | raw suggestions stay copy-only |
| Freshness state | muted status text plus refresh action | communicate stale cache clearly |

### Accessibility Requirements

- Badge text must include the exact ProtonDB tier name and not rely on color alone.
- Retry, copy, and apply actions must have explicit accessible names.
- Unavailable/stale/loading states must be represented by text, not only icons.
- Recommendation notes must render as plain text with readable contrast and keyboard-accessible controls.

### Performance UX

- **Loading States**: inline panel loading; never block the full editor
- **Optimistic Updates**: none for network fetch; use explicit loading and success labels
- **Error Feedback**: local to the ProtonDB card, with stale-cache fallback when available

## Recommendations

### Implementation Approach

**Recommended Strategy**: implement a summary-first, cache-first ProtonDB service in `crosshook-core`, preserve exact tiers with a dedicated `ProtonDbTier` contract, and layer richer recommendations behind safe normalization and graceful fallback. This keeps the stable value path (rating lookup) separate from the risky value path (report aggregation), while still giving the UI a single normalized payload.

**Phasing:**

1. **Phase 1 - Foundation**: exact-tier contract, metadata-cache reuse, stable summary fetch
2. **Phase 2 - Core Features**: Tauri IPC, frontend hook, profile-editor card, exact-tier styling
3. **Phase 3 - Interaction Safety**: explicit apply/copy actions with deterministic conflict resolution, docs, and verification
4. **Phase 4 - Version Correlation Join (`#41`)**: connect ProtonDB panel guidance with version-status context and stale-guidance messaging
5. **Phase 5 - Metadata Handoff Readiness (`#52`)**: lock Steam App ID/caching contracts and integration boundaries for metadata reuse
6. **Phase 6 - Preset Promotion Lifecycle**: promote stable accepted guidance into reusable preset candidates with explicit governance

### Technology Decisions

| Decision | Recommendation | Rationale |
| --- | --- | --- |
| HTTP client | `reqwest` in `crosshook-core` | backend-owned network logic with timeouts and JSON decode |
| Cache storage | reuse `external_cache_entries` | existing table already fits remote JSON snapshots |
| Tier contract | new `ProtonDbTier` enum | existing `CompatibilityRating` is lossy |
| Recommendation application | explicit copy/apply only | avoids unsafe raw launch option injection |

### Quick Wins

- Add the exact-tier rating panel even before richer recommendation aggregation is perfect.
- Reuse `SteamLaunchOptionsPanel` and `CustomEnvironmentVariablesSection` interaction patterns for copy/apply affordances.
- Surface stale cached data with a refresh action to improve offline usability immediately.

### Integrated Expansion Scope (Planned)

- **Version Correlation Integration (`#41`)**: implemented in Phase 4. ProtonDB panel copy includes version-status context so users can distinguish stable guidance from potentially stale recommendations after game updates.
- **Preset Promotion Lifecycle**: implemented in Phase 6. Stable, repeatedly accepted safe recommendations are routed through an explicit preset-candidate workflow with tracked eligibility, review, and rollback behavior.
- **Metadata Integration Boundary (`#52`)**: implemented in Phase 5. The feature defines reusable Steam App ID and cache-key contracts so game-metadata/cover-art work can integrate without schema churn.

## Risk Assessment

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| Hidden ProtonDB report feed changes or disappears | High | High | keep summary lookup independent and degrade recommendations gracefully |
| Exact ProtonDB tiers get collapsed accidentally into legacy compatibility states | Medium | High | introduce a dedicated exact-tier contract and only map intentionally |
| Unsafe raw launch strings get injected into launch builders | Medium | High | whitelist supported env vars and keep everything else copy-only |
| Remote failures degrade the whole profile editor | Low | High | isolate state/errors inside a dedicated card and use stale-cache fallback |

### Integration Challenges

- CrossHook has no direct raw launch-options field in `GameProfile`, so recommendation application must target existing `launch.custom_env_vars` or copy-only flows.
- The existing compatibility badge CSS only understands `unknown`, `broken`, `partial`, `working`, and `platinum`, so ProtonDB needs dedicated exact-tier styling.
- Frontend fetch is not viable because ProtonDB’s CORS response only allows `https://www.protondb.com`.

### Security Considerations

- Treat all ProtonDB fields, especially `launchOptions` and `concludingNotes`, as untrusted remote input.
- Render remote notes as plain text only.
- Never auto-merge remote settings without explicit user action and overwrite safeguards.

## Resolved Decisions

- **Summary-first policy**: issue `#53` is summary-first by requirement. Richer report aggregation is best-effort and cannot block exact-tier lookup or cached-result delivery.
- **Tier display policy**: the ProtonDB panel is ProtonDB-tier-first (`platinum`, `gold`, `silver`, `bronze`, `borked`, etc.). Any legacy compatibility grouping is derived/internal only and never the primary label.
- **Conflict policy for recommendation apply**: conflicts in `launch.custom_env_vars` require explicit per-key confirmation before overwrite; no silent replacement is allowed.
- **Cache policy**: persist normalized ProtonDB DTOs in `external_cache_entries`; do not persist raw report payloads as long-lived cache entries.

## Research References

- [./research-external.md](./research-external.md): live endpoints, dependency choices, and upstream constraints
- [./research-business.md](./research-business.md): user stories, business rules, and success criteria
- [./research-technical.md](./research-technical.md): architecture and data-model approach
- [./research-ux.md](./research-ux.md): placement, state design, and interaction guidance
- [./research-recommendations.md](./research-recommendations.md): phased implementation strategy and top risks
- [/docs/research/additional-features/deep-research-report.md](/docs/research/additional-features/deep-research-report.md): original backlog research that identified issue `#53`
- [/docs/research/additional-features/implementation-guide.md](/docs/research/additional-features/implementation-guide.md): current roadmap placement showing `#53` after version correlation

## Task Breakdown Preview

### Phase 1: Core Lookup and Cache

**Focus**: exact ProtonDB contract, summary fetch, and metadata-cache reuse
**Tasks**:

- define exact-tier and normalized lookup DTOs
- implement cache-backed summary fetch with stale-cache fallback
- add parser tests for tier mapping and safe recommendation normalization

**Parallelization**: exact-tier frontend styling can start once the enum names are fixed

### Phase 2: IPC and Frontend State

**Focus**: expose a single thin Tauri lookup command and frontend hook
**Dependencies**: Phase 1 backend DTO shape
**Tasks**:

- add `protondb_lookup` command and registration
- mirror DTOs in TypeScript and build `useProtonDbLookup`
- define exact-tier UI styling

### Phase 3: Profile Editor UX, Apply Safety, Docs, and Verification

**Focus**: surface the guidance card, explicit apply/copy actions, and finish validation
**Dependencies**: Phase 2 command and hook
**Tasks**:

- add the ProtonDB card to the profile editor
- wire copy/apply actions into existing profile fields with explicit per-key overwrite confirmation
- update docs and run Rust/TypeScript/manual validation

### Phase 4: Version Correlation Integration (`#41`)

**Focus**: align ProtonDB guidance with existing version-intelligence context
**Dependencies**: Phase 3 integration complete and version status surfaces available
**Tasks**:

- join ProtonDB panel state with version-correlation status near the same editor workflow
- add explicit stale-guidance messaging when game version drift suggests recommendations may be outdated
- add regression checks proving this context remains advisory and non-blocking

### Phase 5: Metadata Handoff Readiness (`#52`)

**Focus**: guarantee contract stability for downstream metadata features
**Dependencies**: Phase 2 DTOs and Phase 4 messaging contract
**Tasks**:

- document and enforce canonical Steam App ID ownership for ProtonDB and metadata consumers
- lock cache-key namespace rules and source-provenance fields for cross-feature reuse
- define explicit scope boundaries so issue `#52` can integrate without duplicating lookup or schema logic

### Phase 6: Preset Promotion Lifecycle

**Focus**: convert proven advisory guidance into reusable preset candidates safely
**Dependencies**: Phase 3 apply-flow safety and Phase 4 correlation context
**Tasks**:

- define preset-promotion eligibility criteria based on repeated safe acceptance and conflict-free application patterns
- define storage/update workflow for candidate presets without auto-enabling profile mutations
- add verification and documentation for promotion-path transparency and rollback behavior
