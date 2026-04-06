# Feature Spec: Trainer Discovery

## Executive Summary

Trainer discovery (#67) gives CrossHook users a searchable index of known trainer sources per game, showing version compatibility and linking to external download pages — without hosting or redistributing trainer files. The **Decided approach (Decision 1, Option B)** is a **community-tap-first MVP** that adds a per-game **`trainer-sources.json`** manifest beside `community-profile.json` in each tap, indexes entries into a new SQLite **`trainer_sources`** table (**`migrate_17_to_18`** after the existing SQLite `user_version` **17** in `metadata/migrations.rs`), and runs Phase A **`LIKE`** search over **`trainer_sources`** (joined to `community_taps`), surfaced in a new discovery panel. **`CommunityProfileMetadata` is not extended** with `source_url` / `source_name` for discovery. This requires **zero new Rust crates** for Phase A, reuses community tap sync, `external_cache_entries`, and version correlation infrastructure. The primary risk is legal: linking to trainer download sources carries DMCA §1201 considerations — **opt-in** (`discovery_enabled` default **false**) plus a first-run consent dialog (Decision 2).

---

## External Dependencies

### APIs and Services

#### Steam Web API / Local Manifests (Already Integrated)

- **Documentation**: <https://partner.steamgames.com/doc/webapi/ISteamApps>
- **Authentication**: Free API key via <https://steamcommunity.com/dev>
- **Key Endpoints**:
  - `GET /api/appdetails/?appids={appid}`: Game metadata (already implemented in `steam_metadata/client.rs`)
  - Local `.acf` manifest parsing: `build_id` extraction (already implemented in `steam/manifest.rs`)
- **Rate Limits**: 200 requests / 5 minutes on `store.steampowered.com`
- **Pricing**: Free

#### FLiNG Trainer Site (Phase B — External Lookup)

- **Site**: <https://flingtrainer.com/>
- **Authentication**: None required for public pages
- **Access Method**: RSS feed at `https://flingtrainer.com/category/trainer/feed/` (XenForo built-in; needs live verification)
- **Rate Limits**: Self-imposed; recommend ≥10s between requests
- **Pricing**: Free
- **Note**: No public API exists. CrossHook links to trainer pages, not file host URLs.

#### PCGamingWiki Cargo API (Phase B — Cross-Reference)

- **Documentation**: <https://www.pcgamingwiki.com/wiki/PCGamingWiki:API>
- **Authentication**: None required
- **Key Endpoint**: `GET /w/api.php?action=cargoquery&tables=Infobox_game&where=Steam_AppID HOLDS "{appid}"&format=json`
- **Rate Limits**: Undocumented; cache aggressively

#### IGDB API (Deferred — Requires OAuth Infrastructure)

- **Documentation**: <https://api-docs.igdb.com/>
- **Authentication**: OAuth 2.0 via Twitch Client Credentials
- **Note**: No auth token infrastructure exists in crosshook-core. Defer unless non-Steam game support is explicitly scoped.

### Libraries and SDKs

| Library                | Version | Purpose                             | Status                               |
| ---------------------- | ------- | ----------------------------------- | ------------------------------------ |
| `reqwest`              | 0.12+   | HTTP client (`json` + `rustls-tls`) | Already in project                   |
| `rusqlite`             | 0.39    | SQLite queries (LIKE search)        | Already in project                   |
| `serde` / `serde_json` | 1.x     | IPC serialization                   | Already in project                   |
| `sha2`                 | 0.11    | Hash verification                   | Already in project                   |
| `tokio`                | 1.x     | Async runtime                       | Already in project                   |
| `scraper`              | 0.26+   | HTML parsing (FLiNG fallback)       | **Phase B only, if RSS unavailable** |

**Zero new dependencies required for Phase A MVP.**

### External Documentation

- [Steam Web API community docs](https://steamapi.xpaw.me/): Comprehensive endpoint reference
- [FLiNG Trainer site](https://flingtrainer.com/): Primary trainer source
- [ProtonDB community API](https://protondb.max-p.me/): Already integrated for compatibility tiers
- [Tauri v2 Security — CSP](https://v2.tauri.app/security/csp/): Frontend security constraints

---

## Business Requirements

### User Stories

**Primary User: Linux Gamer**

- As a Linux gamer, I want to search for trainers by game name so that I can find compatible trainers without knowing source sites (US-1)
- As a Linux gamer, I want to see whether a trainer version matches my installed game build so I know if the trainer is likely to work (US-2)
- As a Linux gamer, I want to click a link that takes me to the trainer's download page so I can download from the original source (US-3)
- As a Linux gamer, I want to see trainer sources from my subscribed taps first so I get curated community data (US-4)
- As a Linux gamer, I want to import a discovered trainer profile directly into CrossHook so I don't manually re-enter settings (US-7)

**Secondary User: Steam Deck User**

- As a Steam Deck user, I want trainer discovery to work offline with tap data so I can find trainers at a LAN event (US-5)
- As a Steam Deck user, I want to see an "offline" notice when external search is unavailable so I understand limited results (US-6)

**Secondary User: Community Tap Maintainer**

- As a tap maintainer, I want to know which metadata fields control discoverability so I can structure my tap correctly (US-8)

### Business Rules

1. **No hosting or redistribution (BR-1)**: CrossHook must never fetch, store, or serve trainer binary files. External URLs opened via `tauri::api::shell::open`. Enforced architecturally by keeping binary fetch calls out of the discovery module.
2. **Tap-first result ordering (BR-2)**: Community tap results always appear above external results, reflecting the trust hierarchy.
3. **Version matching is advisory (BR-3)**: Shown as informational signal (Matched / GameUpdated / TrainerChanged / Untracked), not an access gate. Users can proceed regardless.
4. **SHA-256 is a trust signal, not a download gate (BR-5)**: Displayed from community metadata; enforcement happens at launch time via existing `trainer_hash.rs`.
5. **External search requires network; tap data does not (BR-6)**: Tap results come from locally cloned git workspaces. External queries need connectivity.
6. **Offline degraded mode (BR-7)**: Shows tap-only results with persistent "offline — showing tap data only" banner.
7. **Search is local-first (BR-8)**: All tap-originated discovery search is performed against the SQLite **`trainer_sources`** table (indexed from local tap checkouts). No network round-trip for Phase A tap search.
8. **Results are read-only metadata (BR-9)**: Discovery does not create, modify, or delete profiles. Import is a separate explicit action.
9. **No personal data transmitted (BR-11)**: External queries contain only game title or Steam App ID — no user identity, no installed game list.

### Edge Cases

| Scenario                                       | Expected Behavior                                                                   |
| ---------------------------------------------- | ----------------------------------------------------------------------------------- |
| Same game in multiple taps                     | All results shown, grouped by tap with version and rating                           |
| Tap entry has no `trainer_sha256`              | SHA-256 column shows "not verified by tap author"                                   |
| Source URL no longer valid                     | User discovers broken link in browser; future: background link health check         |
| Search query differs from `game_name`          | Fuzzy/substring matching across game_name, trainer_name, description, platform_tags |
| Version snapshot missing                       | Status shown as "Untracked", not as an error                                        |
| External search returns game without tap entry | Result shown with lower trust indicator, no SHA-256 or compatibility rating         |

### Success Criteria

- [ ] Users can search for trainers by game name and see results within 500ms (local SQLite query)
- [ ] Trainer source links open in system browser via Tauri `open()` — no in-app download
- [ ] Tap results always shown before external results in ranked list
- [ ] Version correlation status visible for games with known Steam build ID
- [ ] Offline: panel shows tap results with clear offline notice; no error modal
- [ ] Import from discovery result produces same outcome as importing from CommunityBrowser
- [ ] No trainer binary data stored in SQLite or filesystem by this feature

---

## Technical Specifications

### Architecture Overview

```
Community Taps (git repos)         External Sources (Phase B)
        |                                    |
        v                                    v
  CommunityTapStore                  DiscoveryClient (HTTP)
  (sync/index + trainer-sources.json)      (cache-first fetch)
        |                                    |
        v                                    v
community_profiles (SQLite)          external_cache_entries (SQLite)
trainer_sources (SQLite, Phase A)            |
        |                                    |
        +----------------+-------------------+
                         v
            discovery/search.rs
            (Phase A: LIKE on trainer_sources;
             Phase C: FTS5 + token scoring tie-ins)
                         v
               Tauri IPC Commands
              (#[tauri::command])
                         v
            useTrainerDiscovery (React hook)
                         v
           TrainerDiscoveryPanel (React component)
```

### Data Models

#### Existing Tables (No Changes for Phase A Search)

**`community_profiles`** — already indexed with game_name, trainer_name, trainer_version, game_version, proton_version, compatibility_rating, author, description, platform_tags, schema_version. Unique on `(tap_id, relative_path)`.

**`external_cache_entries`** — generic HTTP cache with source_url, cache_key (UNIQUE), payload_json (512 KiB cap), expires_at TTL. Used in Phase B for external source metadata.

#### Schema Extension (Phase A) — Separate `trainer-sources.json`

Trainer source metadata lives in a separate `trainer-sources.json` file alongside `community-profile.json` in each game directory within a community tap. This separates discovery metadata from profile configuration and allows multiple sources per game.

**Tap directory structure:**

```
tap-repo/
  profiles/
    elden-ring/
      community-profile.json    # Existing profile manifest
      trainer-sources.json      # NEW: trainer source metadata
    cyberpunk-2077/
      community-profile.json
      trainer-sources.json
```

**`trainer-sources.json` schema:**

```json
{
  "schema_version": 1,
  "game_name": "Elden Ring",
  "steam_app_id": 1245620,
  "sources": [
    {
      "name": "FLiNG Trainer",
      "url": "https://flingtrainer.com/elden-ring-trainer/",
      "trainer_version": "v1.12.3",
      "game_version": "1.12.3",
      "notes": "+25 options, known working with Proton 9.0",
      "sha256": "a1b2c3..."
    }
  ]
}
```

**Rust model:**

```rust
// In crosshook-core/src/discovery/models.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerSourcesManifest {
    pub schema_version: u32,
    pub game_name: String,
    #[serde(default)]
    pub steam_app_id: Option<u32>,
    pub sources: Vec<TrainerSourceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerSourceEntry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub trainer_version: Option<String>,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}
```

**SQLite storage:** Trainer source entries are indexed into a new **`trainer_sources`** table during tap sync (alongside existing `community_profiles` indexing). Add **`migrate_17_to_18`** in `metadata/migrations.rs` (SQLite **`user_version`** is **17** in-tree before this migration):

```sql
CREATE TABLE IF NOT EXISTS trainer_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tap_id TEXT NOT NULL REFERENCES community_taps(tap_id) ON DELETE CASCADE,
    game_name TEXT NOT NULL,
    steam_app_id INTEGER,
    source_name TEXT NOT NULL,
    source_url TEXT NOT NULL,
    trainer_version TEXT,
    game_version TEXT,
    notes TEXT,
    sha256 TEXT,
    relative_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(tap_id, relative_path, source_url)
);
CREATE INDEX idx_trainer_sources_game ON trainer_sources(game_name);
CREATE INDEX idx_trainer_sources_app_id ON trainer_sources(steam_app_id);
```

**Note**: This is a **new relational table** (Decision 1, Option B). It separates discovery links from profile launch settings and supports **multiple sources per game** without altering `community_profiles`.

#### Rust Structs

```rust
// crosshook-core/src/discovery/models.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchQuery {
    pub query: String,
    pub compatibility_filter: Option<String>,
    pub platform_filter: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchResult {
    pub community_profile_id: i64,
    pub game_name: String,
    pub game_version: String,
    pub trainer_name: String,
    pub trainer_version: String,
    pub proton_version: String,
    pub compatibility_rating: String,
    pub author: String,
    pub description: String,
    pub platform_tags: Vec<String>,
    pub tap_url: String,
    pub relative_path: String,
    pub manifest_path: String,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub relevance_score: f64,
    pub version_match: Option<VersionMatchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchResponse {
    pub results: Vec<TrainerSearchResult>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionMatchStatus {
    Exact, Compatible, NewerAvailable, Outdated, Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMatchResult {
    pub status: VersionMatchStatus,
    pub trainer_game_version: String,
    pub installed_game_version: String,
    pub detail: Option<String>,
}
```

#### TypeScript Interfaces

```typescript
// src/types/discovery.ts
export type VersionMatchStatus = 'exact' | 'compatible' | 'newer_available' | 'outdated' | 'unknown';

export interface TrainerSearchQuery {
  query: string;
  compatibilityFilter?: string;
  platformFilter?: string;
  limit?: number;
  offset?: number;
}

export interface TrainerSearchResult {
  communityProfileId: number;
  gameName: string;
  gameVersion: string;
  trainerName: string;
  trainerVersion: string;
  protonVersion: string;
  compatibilityRating: string;
  author: string;
  description: string;
  platformTags: string[];
  tapUrl: string;
  relativePath: string;
  manifestPath: string;
  sourceUrl?: string;
  sourceName?: string;
  relevanceScore: number;
  versionMatch?: VersionMatchResult;
}

export interface TrainerSearchResponse {
  results: TrainerSearchResult[];
  totalCount: number;
}
```

### API Design

#### `discovery_search_trainers` (Phase A — LIKE search)

**Request:**

```typescript
invoke<TrainerSearchResponse>('discovery_search_trainers', {
  query: { query: 'Elden Ring', compatibilityFilter: 'working', limit: 20, offset: 0 },
});
```

**Rust signature:**

```rust
#[tauri::command]
pub fn discovery_search_trainers(
    query: TrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<TrainerSearchResponse, String>
```

**Phase A SQL** (queries the new `trainer_sources` table):

```sql
SELECT ts.id, ts.game_name, ts.steam_app_id, ts.source_name,
       ts.source_url, ts.trainer_version, ts.game_version,
       ts.notes, ts.sha256, ts.relative_path,
       ct.tap_url, 0.0 AS relevance_score
FROM trainer_sources ts
JOIN community_taps ct ON ts.tap_id = ct.tap_id
WHERE (ts.game_name LIKE '%' || ?1 || '%'
    OR ts.source_name LIKE '%' || ?1 || '%'
    OR ts.notes LIKE '%' || ?1 || '%')
ORDER BY ts.game_name, ts.source_name
LIMIT ?2 OFFSET ?3
```

**Errors:** Empty query → `"search query cannot be empty"`; DB unavailable → graceful empty results.

#### `discovery_check_version_compatibility` (Phase B — On-demand)

**Request:**

```typescript
invoke<VersionMatchResult>('discovery_check_version_compatibility', {
  communityProfileId: 42,
  profileName: 'my-elden-ring',
});
```

Loads community profile `game_version`, looks up user's `version_snapshots` for the profile, computes advisory match status. Pure function pattern from `compute_correlation_status()`.

### System Integration

#### Files to Create

| File                                       | Phase | Purpose                                        |
| ------------------------------------------ | ----- | ---------------------------------------------- |
| `crosshook-core/src/discovery/mod.rs`      | A     | Module root, re-exports                        |
| `crosshook-core/src/discovery/models.rs`   | A     | Data types for search/results                  |
| `crosshook-core/src/discovery/search.rs`   | A     | LIKE query builder, result mapping, pagination |
| `crosshook-core/src/discovery/matching.rs` | B     | Version comparison (advisory), token scoring   |
| `src-tauri/src/commands/discovery.rs`      | A     | Thin IPC command handlers                      |
| `src/hooks/useTrainerDiscovery.ts`         | A     | React hook wrapping discovery IPC              |
| `src/types/discovery.ts`                   | A     | TypeScript interfaces                          |
| `src/components/TrainerDiscoveryPanel.tsx` | A     | Search UI component                            |

#### Files to Modify

| File                                             | Phase | Change                                                                             |
| ------------------------------------------------ | ----- | ---------------------------------------------------------------------------------- |
| `crosshook-core/src/lib.rs`                      | A     | Add `pub mod discovery;`                                                           |
| `crosshook-core/src/community/index.rs`          | A     | Walk tap directories for `trainer-sources.json` alongside `community-profile.json` |
| `crosshook-core/src/metadata/community_index.rs` | A     | Add `index_trainer_sources()` with A6 bounds + URL validation                      |
| `crosshook-core/src/metadata/migrations.rs`      | A     | Add migration v17→v18 for CREATE TABLE `trainer_sources`                           |
| `crosshook-core/src/metadata/mod.rs`             | A     | Expose trainer source query methods on `MetadataStore`                             |
| `src-tauri/src/commands/mod.rs`                  | A     | Add `pub mod discovery;`                                                           |

#### Configuration

- `settings.toml` — `discovery_enabled` flag (**default: `false`**, opt-in) with legal disclaimer on first enable (matches Decision 2 — user must consent before any discovery UI fetches or shows external-oriented flows)
- No new environment variables required

---

## UX Considerations

### User Workflows

#### Primary Workflow: Game-First Discovery

1. **Open Discovery Panel**
   - User: Opens "Discover Trainers" panel (from sidebar or profile TrainerSection)
   - System: Loads tap results from SQLite immediately — no spinner for local data

2. **Search**
   - User: Types game name (pre-filled from active profile if available)
   - System: Real-time LIKE filter (300ms debounce), results update reactively

3. **Review Results**
   - User: Scans result cards (game name, trainer name, compatibility badge, trust indicator)
   - System: Cards sorted tap-first by compatibility_rating desc, then game_name asc

4. **Expand Detail**
   - User: Clicks/taps a result card
   - System: Shows trainer_version, game_version, proton_version, platform_tags, SHA-256 (if present), version correlation badge (on-demand fetch)

5. **Action**
   - Has community profile: "Import Profile" (primary CTA) + "Get Trainer ↗" (secondary)
   - Link only: "Get Trainer ↗" opens source URL in system browser via Tauri `open()`
   - User downloads trainer externally, configures path in profile

#### Error Recovery Workflow

1. **Offline**: Persistent inline banner "You're offline. Showing local tap results only." — tap results fully functional
2. **No results**: "No trainers found for '[query]'. Try different terms or search online." with "Search Online" button
3. **External search failed**: "Online search unavailable. Showing local results only." with "Retry" button
4. **No taps synced**: "No community taps configured. Add a tap to discover trainers." with "Add Community Tap" CTA

### UI Patterns

| Component                 | Pattern                                                                                                | Notes                                                             |
| ------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| Search input              | Debounced (300ms), keyboard-focusable on open                                                          | Placeholder: "Search games or trainers…"                          |
| Result cards              | Progressive disclosure via CollapsibleSection                                                          | Collapsed: game name, trainer name, badges. Expanded: full detail |
| Compatibility badge       | ProtonDB tier pattern (platinum/working/partial/broken/unknown)                                        | Reuse existing `crosshook-protondb-tier-badge` color tokens       |
| Version correlation badge | Three visual tiers: green (Matched), yellow (GameUpdated/TrainerChanged/BothChanged), gray (Untracked) | Two-stage render: gray placeholder while checking                 |
| Trust indicator           | "Community" filled badge for tap results; chain-link icon for external                                 | Informational only — never blocks link opening                    |
| External link CTA         | Button with ↗ icon, opens via Tauri `open()`                                                           | Never `<a href>` navigation in WebView                            |
| Offline banner            | Persistent inline above results, `--unavailable` modifier                                              | Same pattern as ProtonDB stale cache banner                       |

### Accessibility Requirements

- All interactive elements meet `--crosshook-touch-target-min: 48px` (56px in controller mode)
- Keyboard navigation: Tab → search → filters → cards; Enter to expand; Enter/Space on CTAs
- ARIA live region on results count (`aria-live="polite"`)
- Compatibility badges include `aria-label` with full text
- Skeleton loading: `role="status"`, respects `prefers-reduced-motion`

### Performance UX

- **Loading States**: Skeleton cards on initial panel open; instant render from cache on subsequent opens
- **Search**: 300ms debounce for IPC call; immediate local filtering on cache
- **Background Refresh**: Small spinner in toolbar (non-blocking), "Updated" confirmation for 2s
- **Error Feedback**: Inline banner — never modal for non-critical errors

---

## Recommendations

### Implementation Approach

**Recommended Strategy**: Community-tap-first MVP (Phase A) using **`trainer-sources.json` → `trainer_sources`**, then external lookup (Phase B), then search scale-up (Phase C) — all scoped in this spec (no open-ended deferrals).

**Phasing:**

1. **Phase A — Tap-sourced discovery (MVP, ~3–5 engineer-days)**: **`migrate_17_to_18`** + `index_trainer_sources()` + `discovery/` (`models`, `search` LIKE). IPC `discovery_search_trainers`. UI `TrainerDiscoveryPanel` + opt-in gating. **Estimate:** 3–5 days; **rollback:** feature-flag off + migration downgrade only before ship (avoid dropping `trainer_sources` in production without backup).
2. **Phase B — External source lookup (~1–2 weeks)**: `discovery/client.rs` (`OnceLock<reqwest::Client>`), **`external_cache_entries`** with `trainer:source:v1:{key}` namespace, multi-source aggregation (FLiNG RSS per Decision 3 + optional PCGW normalization), token scoring from `install/discovery.rs`, async IPC (`discovery_search_external`, `discovery_check_version_compatibility`). **Rollback:** disable external commands; cache rows expire via TTL.
3. **Phase C — FTS5 + full `discovery/` integration (~1 week after Phase B)** — **in scope, not deferred**: Enable **`rusqlite` `bundled-full`** (or project-approved FTS-capable feature set). Add **`migrate_18_to_19`** creating an **FTS5 virtual table** (search-only, e.g. content-sync against `trainer_sources` text columns) + triggers or rebuild job. Upgrade `discovery/search.rs` to FTS **`MATCH`** + BM25 ranking; wire **`discovery_rebuild_index`** for recovery; integrate token scoring and cache freshness from Phase B. **Acceptance:** p95 local search &lt; 500ms on a 5k-row synthetic `trainer_sources` DB; rebuild command restores index after corruption; falling back to LIKE if FTS init fails (logged). **Risk:** larger SQLite build / binary size — document in release notes; **rollback:** ship with LIKE-only path via compile-time or runtime flag if FTS build is rejected.

### Technology Decisions

| Decision          | Recommendation                                                                                         | Rationale                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| Search engine     | LIKE in Phase A/B; FTS5 in Phase C                                                                     | Phase C adds `bundled-full` + FTS virtual table; Phase A/B stay on LIKE (`bundled`)                     |
| Version matching  | Advisory text, not computed semver                                                                     | Trainer versions aren't semver-compliant ("v1.0 +DLC", "Build 12345"). `semver` crate would reject most |
| New dependencies  | None for Phase A                                                                                       | All needed crates already in Cargo.toml                                                                 |
| Schema versioning | SQLite **`user_version` 17 → 18** for `trainer_sources`; optional **18 → 19** for FTS in Phase C       | Distinct from `AGENTS.md` table-inventory “schema version” doc line                                     |
| IPC commands      | Split sync/async: `discovery_search_trainers` (sync SQLite) separate from external search (async HTTP) | Prevents fast tap results from waiting on slow network                                                  |
| External cache    | Reuse `external_cache_entries` with `trainer:source:v1:{key}` namespace                                | Proven pattern; **`trainer_sources` is the only new relational table** for tap discovery                |

### Quick Wins

- **Surface existing trainer metadata in UI** (hours): `CommunityProfileMetadata` already has trainer_name, trainer_version, trainer_sha256 — display prominently in profile browsing
- **Ship `trainer-sources.json` template** (hours): Tap maintainers can add sources without changing `community-profile.json` shape
- **Version-aware compatibility badge** (1 day): Combine `version_snapshots` build_id with community `game_version`

### Future Enhancements

- **Trainer source reputation**: Track which tap contributed each source; weight by tap history
- **Aggregate version compatibility matrix**: Pure computation on existing community profile data
- **Link health monitoring**: Background HTTP HEAD checks on source URLs (privacy implications noted)
- **"Hide this source" dismissals**: Reuse `suggestion_dismissals` TTL-based pattern

---

## Risk Assessment

### Technical Risks

| Risk                                       | Likelihood | Impact | Mitigation                                                                                                                 |
| ------------------------------------------ | ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------- |
| Search complexity grows unbounded          | Medium     | High   | Constrain to LIKE search on community index. Token scorer from `install/discovery.rs` for fuzzy matching. No web scraping. |
| Version matching false positives           | Medium     | Medium | Use Steam `build_id` (numeric, exact) for game version. Advisory text for trainer versions.                                |
| Data freshness (stale trainer URLs)        | High       | Medium | TTL-based expiry on cache. Community taps update on sync. Surface staleness in UI.                                         |
| FTS5 not in default build                  | Low        | Medium | Phase A/B use LIKE. Phase C enables FTS via `bundled-full` + migration; document binary-size impact.                       |
| Solo maintainer burnout from feature scope | Medium     | High   | Community taps distribute data maintenance. Keep code surface small (reuse patterns).                                      |

### Integration Challenges

- **Community tap manifest evolution**: `trainer-sources.json` is versioned separately; missing file means no extra discovery rows for that game directory
- **IPC command split**: Sync tap search + async external search must merge client-side for progressive loading — follows existing ProtonDB pattern

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| Finding                                                         | Risk                                             | Mitigation                                                                                       | Alternatives                                          |
| --------------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| S1: DMCA §1201 trafficking risk from linking to trainer sources | Legal liability for indexing circumvention tools | Link-only architecture, legal disclaimer, single-player scope focus                              | Opt-in discovery with consent dialog; geo-restriction |
| S2: No integrity verification beyond git for tap content        | Malicious URLs from compromised taps             | URL validation at index time (HTTPS-only), A6 field-length guards, `pinned_commit` support       | Phase B: git commit signing verification              |
| S3: Cache poisoning via compromised external API                | Stale/malicious metadata persisted until TTL     | TLS validation (rustls), short TTLs, content-type validation, response size limits               | Response hash verification from trusted source        |
| S4: FTS5 query injection (Phase C)                              | Unexpected query behavior from user input        | Sanitize FTS5 operators; Phase A/B use parameterized LIKE only                                   | Quote-wrap / escape MATCH tokens in Phase C           |
| S5: URL rendering in WebKitGTK                                  | XSS via `javascript:` URIs in href               | Always open via Tauri `open()` plugin; never `<a href>` navigation; no `dangerouslySetInnerHTML` | Validate HTTPS-only before rendering                  |

#### Advisories — Best Practices

- **S6**: Enforce HTTPS-only for all trainer source URLs (deferral: none — implement in Phase A)
- **S7**: Apply A6 field-length bounds to trainer source metadata fields (deferral: safe to defer if schema fields are optional)
- **S8**: Run `cargo audit` before shipping discovery feature (deferral: can be added to CI independently)
- **S9**: Surface community tap SHA-256 hashes in discovery; enforce at launch time via existing `trainer_hash.rs` (deferral: Phase B)
- **S10**: Display trust indicator and external-navigation notice for non-tap results (deferral: Phase B when external results exist)

---

## Task Breakdown Preview

### Phase A: Community Tap Extension (MVP)

**Focus**: Searchable trainer discovery from existing community tap profiles with source linking.
**Tasks**:

- Define `TrainerSourcesManifest` / `TrainerSourceEntry` models and `trainer-sources.json` schema
- Add migration v17→v18 for CREATE TABLE `trainer_sources` with indexes
- Extend `CommunityProfileIndex` to walk tap directories for `trainer-sources.json` files
- Add `index_trainer_sources()` to persist entries with A6 field-length bounds + HTTPS-only URL validation
- Create `discovery/` module with `mod.rs`, `models.rs`, `search.rs` (LIKE query builder)
- Create `commands/discovery.rs` with `discovery_search_trainers` IPC command
- Create `useTrainerDiscovery.ts` hook (request-id race guard pattern)
- Create `TrainerDiscoveryPanel.tsx` with search, result cards, compatibility badges, source link CTAs
- Create `types/discovery.ts` TypeScript interfaces
- Unit tests for search, URL validation, field bounds (use `MetadataStore::open_in_memory()`)
- Legal disclaimer dialog on first discovery panel open

**Parallelization**: Schema/migration + UI design can proceed in parallel. Pure function tests are independent of UI.

### Phase B: External Source Lookup

**Focus**: Optional external trainer source queries with cache and degraded offline mode.
**Dependencies**: Phase A complete.
**Tasks**:

- Create `discovery/client.rs` following ProtonDB client pattern (`OnceLock` HTTP client, cache-first fetch)
- Create `discovery/matching.rs` (token scoring from `install/discovery.rs` pattern, advisory version comparison)
- Add `discovery_search_external` async IPC command
- Add `discovery_check_version_compatibility` on-demand IPC command
- Integrate external results into `TrainerDiscoveryPanel` with progressive loading
- Source trust indicators (Community vs External badges)
- Offline degraded mode with persistent banner
- Unit tests for client, cache, matching, aggregation

### Phase C: FTS5 search + discovery hardening (required closure)

**Focus**: Scale search and complete `crosshook-core::discovery/` integration with cache + ranking.
**Dependencies**: Phase B complete (HTTP client + `external_cache_entries` paths live).
**Timeline**: ~1 engineering week (can overlap lightly with Phase B bugfix window).
**Tasks**:

- Add **`rusqlite` `features = ["bundled-full"]`** (or approved equivalent) in `crosshook-core/Cargo.toml`; verify CI/release binary impact.
- Add **`migrate_18_to_19`**: FTS5 **virtual table** (not a duplicate business table) with `content='trainer_sources'` + triggers, or equivalent rebuild-from-source strategy documented in `discovery/search.rs`.
- Upgrade `discovery/search.rs`: FTS **`MATCH`** queries + BM25 ranking; merge token scores from `install/discovery.rs` where applicable.
- Implement **`discovery_rebuild_index`**: rebuilds FTS from `trainer_sources`; idempotent; covered by `MetadataStore::open_in_memory()` test.
- Ensure **`external_cache_entries`** TTL/eviction behavior is aligned with search (stale cache badges in UI).
  **Acceptance criteria**: Documented in **Recommendations → Phasing → Phase C** (p95 target, rebuild path, LIKE fallback).
  **Rollback**: Disable FTS code path; keep LIKE; or revert `bundled-full` in a follow-up PR if size regressions block release.

---

## Decisions (Resolved)

1. **Schema approach** — **Decided: Option B** — Separate `trainer-sources.json` file per game directory in community taps. More flexible than a single `source_url` field; allows multiple sources per game, separates trainer source metadata from profile metadata, and lets tap maintainers evolve the format independently.

2. **Legal posture** — **Decided: Option B** — Discovery is opt-in (disabled by default in `settings.toml` via `discovery_enabled = false`). First enable shows a consent dialog explaining that CrossHook does not host trainers, links to external sources only, and the user is responsible for legal compliance. Disclaimer notes trainers for online games may violate terms of service.

3. **External API selection (Phase B)** — **Decided: FLiNG RSS only** — FLiNG RSS as the sole Phase B external source. PCGamingWiki for cross-reference metadata only (game name normalization, not trainer sources). No WeMod API (ToS risk), no CheatHappens (subscription-gated), no IGDB (requires OAuth infrastructure).

4. **Anti-cheat scope filtering** — **Decided: Option A** — No automatic filtering. User responsibility, noted in the consent disclaimer that trainers for games with online anti-cheat may violate terms of service. Avoids maintaining an anti-cheat game list.

---

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): External API details, trainer source site analysis, Rust crate evaluation
- [research-business.md](./research-business.md): Business logic analysis, user stories, domain model, existing codebase integration
- [research-technical.md](./research-technical.md): Technical specifications, data models, API design, system constraints
- [research-ux.md](./research-ux.md): UX research, competitive analysis, accessibility, performance UX patterns
- [research-security.md](./research-security.md): Security analysis with severity-leveled findings (0 CRITICAL, 5 WARNING, 5 ADVISORY)
- [research-practices.md](./research-practices.md): Engineering practices, reusable code inventory, KISS assessment, build-vs-depend decisions
- [research-recommendations.md](./research-recommendations.md): Implementation phasing, risk assessment, alternative approaches, task breakdown
