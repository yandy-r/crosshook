# Trainer Discovery: Research Recommendations

## Executive Summary

Trainer discovery (#67) is a P3 feature rated "Very High" effort with 3/8 perspective support from the deep research analysis. The codebase has substantial existing infrastructure that can dramatically reduce the effort if the feature is scoped correctly. The recommended approach is a **community-tap-first MVP** that extends the existing community profile schema with trainer source metadata, rather than building a standalone search engine. This keeps the feature aligned with CrossHook's sustainability model (community-distributed maintenance) and avoids the "trainer download marketplace" anti-pattern flagged by the research report.

The biggest risk is not technical but scope: building too much. The MVP should be achievable in 1-2 weeks by leveraging existing community tap indexing, the `external_cache_entries` cache layer, and version correlation infrastructure. Zero new crate dependencies are required -- `reqwest`, `sha2`, `serde_json`, `rusqlite`, and `tokio` are already in `Cargo.toml`.

---

## Implementation Recommendations

### Approach: Community Tap Extension + Optional External Lookup

**Phase A (MVP)**: Extend community taps with trainer source metadata. No new external APIs. No new database tables.

**Phase B (Enhancement)**: Add optional external source lookups using the established `external_cache_entries` pattern, following the ProtonDB/Steam metadata client architecture.

### Technology Choices

All technology choices should reuse existing codebase patterns:

- **Data model**: Extend `CommunityProfileMetadata` or add a sibling `trainer-sources.json` file per game directory in community taps
- **Indexing**: Reuse `community/index.rs` directory-walking and JSON parsing pattern; apply `check_a6_bounds` field-length guards from `community_index.rs` before SQLite insert
- **Caching**: Reuse `external_cache_entries` with TTL-based expiry (same pattern as `protondb/client.rs:318-344` and `steam_metadata/client.rs:204-223`)
- **HTTP client**: `OnceLock<reqwest::Client>` with 6-second timeout and CrossHook user-agent (established pattern)
- **Name matching**: Reuse the tokenize/score/rank pattern from `install/discovery.rs` (~50 lines of pure functions: `tokenize()`, `token_hits()`, `score_candidate()`) for fuzzy game-name and trainer-name matching
- **Version matching**: Advisory text string from tap metadata, not computed semver comparison -- trainer versions are not semver-compliant (e.g., "v1.0 +DLC", "2024.12.05"). Pure function on `version_snapshots` data (follows `version_store.rs:185-211` compute pattern)
- **SHA-256 verification**: Reuse `offline/hash.rs` functions (`verify_and_cache_trainer_hash`, `normalize_sha256_hex`) for hash chain continuity
- **Dismissals**: Reuse `metadata/suggestion_store.rs` TTL-based dismissal pattern for "hide this source" user actions
- **Security validation**: Reuse `community/taps.rs` utilities (URL allow-list, branch name validation, git SHA validation, git env isolation)
- **Serde models**: All IPC-crossing types need `Serialize`/`Deserialize` derives (Tauri convention)

### Build vs. Depend Decisions

| Concern            | Decision                                  | Rationale                                                                                         |
| ------------------ | ----------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Full-text search   | **Build** custom token scorer (~50 lines) | Same pattern as `install/discovery.rs`. Do NOT add tantivy or FTS5 -- overkill for this use case. |
| Version comparison | **Build** simple normalizer               | The `semver` crate rejects most real trainer version strings. Advisory text is sufficient.        |
| HTTP client        | **Reuse** reqwest                         | Already in Cargo.toml, already patterned across two modules.                                      |
| Hash verification  | **Reuse** sha2 + existing functions       | `offline/hash.rs` already does this.                                                              |
| All other concerns | **Reuse** existing crates                 | Zero new dependencies needed for MVP.                                                             |

### Phasing Strategy

#### Phase A: Community Tap Trainer Sources (MVP)

**What**: Add trainer source metadata to community taps so users can discover where to get trainers for games that already have community profiles.

**Scope**:

- New optional `trainer_sources` array in `CommunityProfileMetadata` containing `{ name, url, notes }` entries
- OR a separate `trainer-sources.json` file alongside `community-profile.json` in tap directories
- Extend `CommunityProfileIndex` to surface trainer source data during tap sync
- Surface trainer sources in the profile detail view / onboarding flow
- URL validation using the same scheme-allowlist pattern as `validate_tap_url()`

**Why this is the MVP**: Zero new external dependencies. Zero new database tables. Community tap maintainers curate the data. Works offline via existing tap cache. Aligns with the deep research report's recommendation that community taps distribute maintenance burden.

**Effort**: ~3-5 days

#### Phase B: External Source Lookup (Enhancement)

**What**: Optional network lookup to augment community tap data with broader trainer source information.

**Scope**:

- New `trainer_discovery/` module in `crosshook-core` (following `protondb/` module layout: `mod.rs`, `client.rs`, `models.rs`, `matching.rs`)
- HTTP client with stale-while-revalidate using `external_cache_entries`
- Cache key pattern: `trainer:source:v1:{normalized_game_name}`
- Integration with version correlation to show version-compatible sources
- Degraded mode: community tap data only when network unavailable

**Module structure**:

```
crosshook-core/src/trainer_discovery/
  mod.rs          # Public re-exports
  client.rs       # HTTP fetch + cache (mirrors protondb/client.rs)
  models.rs       # TrainerSource, TrainerSourceLookupResult, etc.
  matching.rs     # Pure scoring/ranking functions (mirrors install/discovery.rs)
```

**IPC layer**: New `src-tauri/src/commands/trainer_discovery.rs` with thin handlers following `commands/protondb.rs` pattern. Frontend: new `useTrainerDiscovery.ts` hook + injection point in `TrainerSection.tsx`.

**Effort**: ~1-2 weeks

### Quick Wins That Can Ship Independently

1. **Surface existing community profile trainer metadata in UI** (~hours): `CommunityProfileMetadata` already has `trainer_name`, `trainer_version`, and `trainer_sha256`. Display these prominently when browsing community profiles so users know what trainer to look for.

2. **Add `trainer_source_url` field to community profile schema** (~hours): A single optional URL field is the smallest possible extension. Tap maintainers can start populating it immediately.

3. **Version-aware trainer compatibility badge** (~1 day): Combine `version_snapshots` build ID tracking with community profile `game_version` metadata to show "This trainer profile was verified with game version X; your game is version Y."

---

## Improvement Ideas

### Connection to Related Features

1. **Version Correlation (P1, #41 -- Done)**: Trainer discovery's highest-value feature is showing which trainers work with which game versions. The `version_snapshots` table already tracks `steam_build_id`, `trainer_version`, and `trainer_file_hash` per profile. Discovery should query this: "for app ID X at build Y, which trainer versions have `status = 'matched'` in community data?"

2. **Onboarding Guidance (P0, #37 -- Done)**: The onboarding flow already guides users through profile creation. Trainer discovery slots naturally into the "where do I get a trainer?" gap identified in the deep research report. When a user creates a profile for a game, show known trainer sources from community taps.

3. **Profile Health Dashboard (P0, #38 -- Done)**: When a profile's trainer is missing or its hash doesn't match, the health dashboard could suggest: "Find a replacement trainer" with links from discovery data.

4. **ProtonDB Suggestions (#77 -- Done)**: The ProtonDB suggestion system derives actionable configuration suggestions from community data. Trainer discovery could follow the same pattern: derive trainer recommendations from community profile metadata aggregated across taps.

5. **Trainer Hash Verification (#63 -- Done)**: The `trainer_sha256` field in `CommunityProfileMetadata` and the `trainer_hash_cache` table create a verification chain. Discovery should surface expected hashes so users can verify downloaded trainers before importing.

### Enhancements Beyond MVP

- **Trainer source reputation**: Track which community tap contributed each source. Taps with more profiles and longer history are more trustworthy.
- **Aggregate version compatibility matrix**: Across all community profiles, build a matrix of game versions vs. trainer versions with compatibility ratings. Pure computation on existing data.
- **Trainer source freshness**: When a community tap's trainer source URL returns 404, mark it as stale. Surface freshness in UI.
- **"Hide this source" dismissals**: Reuse the `suggestion_dismissals` table pattern from `metadata/suggestion_store.rs` with TTL-based expiry so dismissed sources reappear after a configurable period.

---

## Risk Assessment

### Technical Risks

| Risk                                | Severity | Likelihood | Mitigation                                                                                                                                                                               |
| ----------------------------------- | -------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Search complexity grows unbounded   | High     | Medium     | Constrain to community tap index lookup + optional single-API external lookup. No web scraping. Use `install/discovery.rs` tokenize/score pattern (~50 lines), not FTS5.                 |
| Version matching false positives    | Medium   | Medium     | Use Steam `build_id` (numeric, exact) not human game version strings. `compute_correlation_status()` already handles this. For trainer versions, use advisory text, not computed semver. |
| Data freshness (stale trainer URLs) | Medium   | High       | TTL-based expiry on `external_cache_entries`. Community taps update on sync. Surface staleness in UI.                                                                                    |
| Cache payload size exceeds 512 KiB  | Low      | Low        | Trainer source metadata per game is small (~1-5 KiB). Only a risk if building a full search index (which we shouldn't).                                                                  |
| Community tap schema version break  | Medium   | Low        | `COMMUNITY_PROFILE_SCHEMA_VERSION` is currently 1. New fields with `serde(default)` are backward-compatible without bumping.                                                             |

### Legal Risks

| Risk                                                    | Severity | Likelihood | Mitigation                                                                                                                    |
| ------------------------------------------------------- | -------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Linking to trainer download sources implies endorsement | High     | Medium     | Clear disclaimer that CrossHook does not host, redistribute, or verify trainer legality. Community-contributed metadata only. |
| DMCA/takedown for linking to trainers                   | Medium   | Low        | Links are community-contributed via git repos (community taps), not hosted by CrossHook. Tap maintainers own their content.   |
| Trademark issues from displaying trainer/game names     | Low      | Low        | Factual use of names for identification. Same as displaying game names from Steam manifests (already done).                   |

### Security Risks

| Risk                                         | Severity | Likelihood | Mitigation                                                                                                                                                    |
| -------------------------------------------- | -------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Malicious URLs in community taps             | High     | Medium     | URL scheme validation (HTTPS only for trainer sources). Community tap trust model: users choose which taps to subscribe to.                                   |
| Malicious trainer executables                | Critical | Medium     | SHA-256 hash verification already exists at launch (`trainer_hash.rs`). Surface expected hashes from community data. **Never auto-download or auto-execute.** |
| Cache poisoning via compromised external API | Medium   | Low        | Validate response structure. Cap payload size. Stale-while-revalidate means poisoned data expires.                                                            |
| Tap force-push changes discovery metadata    | Medium   | Medium     | `pinned_commit` support already exists for taps. Encourage pinning for security-sensitive taps.                                                               |

### Scope Risks

| Risk                                       | Severity | Likelihood | Mitigation                                                                                                                                                                    |
| ------------------------------------------ | -------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Feature creep toward "trainer marketplace" | Critical | High       | Hard scope boundary: display links only. No hosting, no downloads, no ratings, no accounts.                                                                                   |
| Very High effort for P3 priority           | High     | High       | MVP via community tap extension reduces effort to Medium. Phase B only if community demand justifies it.                                                                      |
| Solo maintainer burnout                    | High     | Medium     | Community taps distribute data maintenance. Keep the code surface small (reuse existing patterns).                                                                            |
| Premature optimization of search           | Medium   | Medium     | Start with linear scan of community index entries (already sorted by game name). Custom token scorer from `install/discovery.rs` pattern handles fuzzy matching without FTS5. |

---

## Alternative Approaches

### Option 1: Community Tap Extension Only (Recommended MVP)

**Description**: Add trainer source metadata to community profile manifests. No external APIs. No new database tables.

**Pros**:

- Lowest effort (~3-5 days)
- Works offline via existing tap cache
- Community-maintained data (sustainable)
- No legal risk from external API integration
- Reuses all existing infrastructure
- Backward-compatible schema extension
- Zero new crate dependencies

**Cons**:

- Limited to games with community profiles
- Data quality depends on tap maintainers
- No automated discovery for new games

**Effort**: Low-Medium

### Option 2: Community Taps + External API Lookup

**Description**: Option 1 plus an optional external lookup service for games without community tap data.

**Pros**:

- Broader coverage than community taps alone
- Stale-while-revalidate provides resilience
- Follows established ProtonDB/Steam metadata patterns

**Cons**:

- Higher effort (~2-3 weeks total)
- External API dependency (availability, rate limits, data quality)
- Legal complexity of linking to external trainer sources programmatically
- Maintenance burden for API integration

**Effort**: Medium-High

### Option 3: Full Search Index with FTS

**Description**: Build a searchable trainer database with SQLite FTS5, aggregating multiple sources.

**Pros**:

- Best search UX
- Can aggregate community taps + external APIs + user submissions

**Cons**:

- Very High effort (matches original estimate)
- New database table and migration required
- Search relevance tuning is ongoing work
- Over-engineered for the current user base
- Maintenance burden far exceeds value at current maturity

**Effort**: Very High

### Recommendation

**Start with Option 1.** It delivers 80% of the value at 20% of the effort. If community feedback demonstrates demand for broader coverage, graduate to Option 2. Option 3 is not justified at CrossHook's current maturity level.

---

## Reusable Infrastructure Inventory

The following existing codebase patterns directly apply to trainer discovery implementation. This inventory informs the effort estimates above.

| Infrastructure                         | File                                             | Reuse For                                          |
| -------------------------------------- | ------------------------------------------------ | -------------------------------------------------- |
| TTL-based cache with stale fallback    | `metadata/cache_store.rs`                        | All external metadata fetches                      |
| SHA-256 verification with SQLite cache | `offline/hash.rs`                                | Trainer hash chain continuity                      |
| Tokenize/score/rank matching           | `install/discovery.rs`                           | Game name and trainer name fuzzy matching          |
| URL allow-list validation              | `community/taps.rs`                              | Trainer source URL validation                      |
| Field-length guards for SQLite         | `metadata/community_index.rs`                    | Bound-checking trainer source fields before insert |
| TTL-based dismissal                    | `metadata/suggestion_store.rs`                   | "Hide this source" user action                     |
| HTTP client singleton                  | `protondb/client.rs`, `steam_metadata/client.rs` | External API fetch (Phase B)                       |
| Community tap offline state            | `community_tap_offline_state` table              | Offline availability tracking                      |
| Version correlation                    | `metadata/version_store.rs`                      | Game/trainer version matching                      |

---

## Design Trade-offs (Resolved)

These trade-offs were evaluated by the tech designer and recommendations agent. The resolution column reflects the team's consensus position.

### Search Strategy: FTS5 vs. Token Scorer

| Dimension                       | FTS5                                                      | Token Scorer (Recommended)                                  |
| ------------------------------- | --------------------------------------------------------- | ----------------------------------------------------------- |
| Performance at <1000 profiles   | Sub-ms                                                    | Sub-ms (equivalent)                                         |
| Performance at 10,000+ profiles | Sub-ms                                                    | ~5-10ms (acceptable)                                        |
| Schema impact                   | New virtual table + 3 triggers (migration v18)            | Zero schema changes                                         |
| Code complexity                 | ~80 lines for FTS setup + sync triggers                   | ~50 lines of pure functions (already patterned in codebase) |
| Relevance ranking               | Built-in BM25                                             | Custom scoring via token hits + penalties                   |
| MVP alignment                   | Requires migration; conflicts with "zero new tables" goal | Fully aligned with MVP scope                                |

**Resolution**: Use the `install/discovery.rs` tokenize/score pattern for Phase A. Graduate to FTS5 as a Phase B optimization **only if** the community tap index exceeds ~1000 profiles and search latency becomes measurable. FTS5 is a valid upgrade path, not a starting point. The tech designer's concern about scale is acknowledged but the threshold is speculative at current maturity.

### Version Matching: Computed vs. Advisory

| Dimension    | Computed semver                                  | Advisory text (Recommended)            |
| ------------ | ------------------------------------------------ | -------------------------------------- |
| Accuracy     | High for semver-compliant strings                | Displays community-provided text as-is |
| Coverage     | Rejects "v1.0 +DLC", "Build 12345", "2024.12.05" | Handles all formats (no parsing)       |
| Dependency   | Requires `semver` crate                          | Zero new dependencies                  |
| Failure mode | Silent rejection of non-compliant versions       | Always displays something useful       |

**Resolution**: Use advisory text for trainer version display. Use Steam `build_id` (numeric, exact match) for game version correlation via `compute_correlation_status()`. Do not attempt semver parsing of trainer version strings.

### Search Result Enrichment: Batch vs. On-Demand

**Resolution**: On-demand enrichment, matching the ProtonDB pattern where `lookup_protondb()` and `derive_suggestions()` are separate calls. Search returns lightweight results; version correlation data is fetched per selected result. This keeps search fast and avoids loading version data for results the user never inspects.

### Cross-Tap Deduplication

**Resolution**: Show all results with tap attribution. Deduplicate only within the same tap (already enforced by the unique index on `(tap_id, relative_path)` in `community_profiles`). Users chose which taps to subscribe to; hiding results from a subscribed tap would be confusing. Cross-tap duplicates are informational -- they show community consensus.

### Version Strings in Search

**Resolution**: Exclude `game_version` and `trainer_version` from text search. They pollute results (e.g., searching "1.0" matches hundreds of profiles). Version filtering should be a separate facet or on-demand enrichment, not a text search dimension.

---

## Task Breakdown Preview

### Phase A: Community Tap Trainer Sources (MVP)

| Task Group                 | Tasks                                                                                                                                                                              | Complexity | Dependencies     |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ---------------- |
| Schema Extension           | Add `trainer_sources` to community profile schema; update `CommunityProfileMetadata` Serde model; add URL validation                                                               | Low        | None             |
| Index Extension            | Extend `CommunityProfileIndex` to surface trainer source entries during tap sync; apply field-length guards                                                                        | Low        | Schema Extension |
| Tauri Commands             | Add `get_trainer_sources_for_game` command wrapping index lookup                                                                                                                   | Low        | Index Extension  |
| UI: Profile Detail         | Show trainer sources when viewing a community profile or game detail                                                                                                               | Medium     | Tauri Commands   |
| UI: Onboarding Integration | Surface trainer sources in the onboarding/profile creation flow                                                                                                                    | Medium     | Tauri Commands   |
| Version Matching           | Advisory text comparison (not semver): match community trainer sources against user's game `build_id` from `version_snapshots`                                                     | Low        | Index Extension  |
| Tests                      | Unit tests for schema, index, version matching (pure functions, no I/O); integration test for tap sync with trainer sources; use `MetadataStore::open_in_memory()` for cache tests | Low        | All above        |

**Estimated total**: 3-5 days

### Phase B: External Source Lookup (Enhancement)

| Task Group                 | Tasks                                                                                                                | Complexity | Dependencies                        |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------- | ---------- | ----------------------------------- |
| Models                     | Define `TrainerSourceLookupResult`, `TrainerSource`, `TrainerSourceCacheState`                                       | Low        | None                                |
| HTTP Client                | `trainer_discovery/client.rs` following ProtonDB pattern: `OnceLock` client, fetch, cache, stale fallback            | Medium     | Models                              |
| Cache Integration          | Reuse `external_cache_entries` with `trainer:source:v1:{key}` pattern                                                | Low        | HTTP Client                         |
| Name Matching              | `trainer_discovery/matching.rs` with tokenize/score/rank from `install/discovery.rs` pattern; pure functions, no I/O | Low        | Models                              |
| Aggregation                | Merge community tap sources + external API results, deduplicate, rank                                                | Medium     | HTTP Client, Phase A, Name Matching |
| Source Dismissal           | Reuse `suggestion_dismissals` table pattern for "hide this source"                                                   | Low        | Models                              |
| Tauri Commands             | Expose aggregated results via IPC; thin handlers following `commands/protondb.rs`                                    | Low        | Aggregation                         |
| UI: Search                 | Add search/filter UI for trainer sources; `useTrainerDiscovery.ts` hook + `TrainerSection.tsx` injection             | Medium     | Tauri Commands                      |
| FTS5 Upgrade (conditional) | Add FTS5 virtual table + triggers if community tap index exceeds ~1000 profiles                                      | Medium     | Phase A complete, scale data        |
| Tests                      | Unit tests for client, cache, matching, aggregation; mock-based tests for external API                               | Medium     | All above                           |

**Estimated total**: 1-2 weeks

### Parallelizable Work

- Schema extension and UI design can proceed in parallel
- Unit tests for version matching and name matching are independent of UI work (pure functions)
- Phase B HTTP client can be prototyped before Phase A UI is complete (but should not ship before Phase A)
- `matching.rs` pure functions can be developed and tested independently of all I/O concerns

### Dependencies on P0/P1 Features

| Dependency                      | Status | Impact                                             |
| ------------------------------- | ------ | -------------------------------------------------- |
| Version correlation (#41)       | Done   | Required for version-aware trainer matching        |
| Onboarding guidance (#37)       | Done   | Required for onboarding integration                |
| Profile health dashboard (#38)  | Done   | Required for "find replacement trainer" suggestion |
| Community taps infrastructure   | Done   | Required for Phase A (core infrastructure)         |
| Trainer hash verification (#63) | Done   | Required for hash verification chain               |

All dependencies are already complete. Phase A can begin immediately.

---

## Architectural Invariant

**Discovery results must be link-only.** CrossHook must never auto-fetch or auto-execute trainer binaries from discovered URLs. This is enforced architecturally by keeping `reqwest` fetch calls out of the discovery indexer -- the indexer produces metadata (URLs, names, hashes), never binary content. Any future Phase B external API client fetches metadata JSON only, never trainer executables.

---

## Key Decisions Needed

1. **Schema approach**: Add `trainer_sources` array to `CommunityProfileMetadata` (simpler, one file) vs. separate `trainer-sources.json` per game directory (more flexible, separates concerns)? Initial step: add a single optional `source_url` field via `serde(default)` (backward-compatible, no schema version bump required). Full `trainer_sources` array in a future schema v2 if one URL per profile is insufficient.

2. **URL allowlist**: HTTPS only for trainer source URLs, matching the `validate_tap_url()` security posture. **Resolved.**

3. **Schema version bump**: Not needed for optional `serde(default)` fields. Bump to v2 only when adding non-optional structural changes. **Resolved.**

4. **External API selection (Phase B only)**: Which external trainer source APIs, if any, are reliable enough to integrate? This needs community input and legal review before committing.

5. **Scope boundary enforcement**: Hard rule that CrossHook never fetches trainer binaries, only metadata and links. Enforced architecturally by keeping binary fetch calls out of the discovery module. **Resolved.**

---

## Open Questions

1. How many games in the current community tap ecosystem have profiles? This determines whether community-tap-only discovery is sufficient for initial launch.

2. Are there existing community-maintained trainer metadata databases (JSON/YAML/etc.) that could be imported as community taps without building a new data pipeline?

3. What is the legal precedent for linking to trainer download sources in open-source Linux gaming tools? (Lutris links to game installers; is the trainer analogy defensible?)

4. Should trainer discovery results be exportable as part of the diagnostic bundle (#49), or is that scope creep?

5. What is the minimum viable UI surface? A section in the profile detail view, or a dedicated discovery panel? The onboarding flow integration may be sufficient for MVP.

6. Should Phase B external lookup be opt-in (user must enable in settings) or opt-out? Security and privacy considerations favor opt-in.
