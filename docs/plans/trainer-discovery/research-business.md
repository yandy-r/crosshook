# Trainer Discovery — Business Analysis

CrossHook is a native Linux desktop tool that orchestrates launching Windows games via Proton/Wine. Trainer discovery gives users a searchable index of known trainer sources per game, shows version compatibility between available trainers and installed game builds, and links to external download sources. CrossHook never hosts or redistributes trainer files.

---

## Executive Summary

Linux gamers who use game trainers currently must know trainer sources in advance, manually track version compatibility, and configure CrossHook profiles without any in-app guidance on where to find compatible trainers. Trainer discovery closes this gap by surfacing trainer metadata from community-maintained tap repositories (already fully implemented) and, optionally, from external sources when online. The core constraint is strict: CrossHook is a metadata aggregator and link provider only. No files are fetched, hosted, or cached beyond JSON metadata blobs. Version matching is advisory — it informs the user but does not gate access.

**Strategic context**: Trainer discovery is issue #67, classified as Phase 7 / P3 in `docs/research/additional-features/implementation-guide.md`. It is explicitly unscheduled with the trigger condition "When community taps reach 50+ profiles and users still struggle to find trainers." The deep research report also flags "Trainer download marketplace" as a 3/8-perspective anti-pattern (legal liability, security concerns, maintenance burden). This research covers the full feature scope; the MVP scope definition below identifies the minimum viable slice that avoids the anti-pattern risk.

---

## Strategic Context & MVP Scope

### Phase positioning

Per `docs/research/additional-features/implementation-guide.md`, issue #67 is Phase 7 (P3, unscheduled). The trigger to revisit is: community taps reaching 50+ profiles while users still struggle to find trainers. This research covers the full feature design. Implementation sequencing is a separate decision.

### Anti-pattern boundary

The deep research report (3/8 perspectives) flags "Trainer download marketplace" as an anti-pattern due to legal liability, security concerns, and maintenance burden. The boundary rule is:

> CrossHook guides, it does not host. CrossHook links, it does not download.

Any scope that moves toward fetching, storing, or forwarding trainer binary content crosses this boundary and should be rejected regardless of implementation convenience.

### MVP definition (feature-complete plan — no deferred core)

The feature has three engineering slices: **Phase A** (tap-sourced discovery), **Phase B** (external metadata via HTTP + cache), **Phase C** (FTS5 + rebuild + ranking hardening). All are specified in `feature-spec.md` and are **in scope** for a complete trainer-discovery release track — not parked as “Phase 2 someday.”

**Phase A — Tap-sourced discovery (MVP gate)**

- **Data flow:** Tap repos ship **`trainer-sources.json`** beside `community-profile.json` → sync indexes **`trainer_sources`** (`migrate_17_to_18` in `migrations.rs`) → **`discovery_search_trainers`** runs LIKE queries → UI panel.
- **Interfaces:** `TrainerSourcesManifest` / `TrainerSourceEntry` (Rust + TS mirrors); `MetadataStore::search_trainer_sources` (name TBD) delegating to `discovery/search.rs`.
- **Success criteria:** Search returns within 500ms local; HTTPS-only URLs; A6 bounds; empty safe `MetadataStore::disabled()` path.
- **Minimal acceptance tests:** `MetadataStore::open_in_memory()` tests for migration 18, indexer, and search; IPC contract test block in `commands/discovery.rs`.

**Phase B — External trainer-source client + aggregation**

- **Approach:** `discovery/client.rs` with `OnceLock<reqwest::Client>`, same cache discipline as `protondb/client.rs`; **`external_cache_entries`** key `trainer:source:v1:{normalized_game_key}`; FLiNG RSS per Decision 3; merge external rows with tap rows in UI (tap-first ordering preserved).
- **Success criteria:** Offline banner when network fails; stale cache fallback; no trainer binaries fetched (JSON only).
- **Minimal tests:** Unit tests with mocked HTTP or trait-injected client; cache hit/miss/expiry paths.

**Phase C — Bundled/catalog + search scale (FTS5)**

- **Approach:** Enable **`rusqlite` `bundled-full`**; **`migrate_18_to_19`** FTS5 virtual table over `trainer_sources` (search index only); `discovery_rebuild_index`; token scoring tie-in from `install/discovery.rs`.
- **“Bundled source catalog”:** Static allow-list JSON in-repo (e.g. known RSS endpoints / site metadata) versioned with app — read-only, signed-off in PR; not a remote “store.”
- **Link health monitoring:** Optional background `HEAD`/`GET` metadata check with strict rate limits, user-toggle off by default, privacy note in settings — ships in same milestone as Phase B or C per capacity (documented in `feature-spec.md` Phase C acceptance if included).

**Milestone timeline (indicative):** Week 1 — Phase A vertical slice; Week 2–3 — Phase B client + UI merge; Week 4 — Phase C FTS + rebuild + performance acceptance.

### Community tap sustainability model

Trainer **links** live in **`trainer-sources.json`**, maintained by tap authors alongside profiles. CrossHook does not host binaries; it indexes metadata and opens the user’s browser. This distributes maintenance cost and keeps legal posture aligned with BR-1.

---

## User Stories

### Primary Actors

- **Linux gamer** — plays Windows games via Steam/Proton, wants trainers to work correctly without deep technical knowledge.
- **Steam Deck user** — constrained controller UI, offline use common, storage-sensitive.
- **Community tap maintainer** — publishes trainer metadata and compatibility profiles for games they test.

### Stories

| ID   | As a…                    | I want to…                                                    | So that…                                                        |
| ---- | ------------------------ | ------------------------------------------------------------- | --------------------------------------------------------------- |
| US-1 | Linux gamer              | Search for trainers by game name                              | I can find compatible trainers without knowing source sites     |
| US-2 | Linux gamer              | See whether a trainer version matches my installed game build | I know before downloading if the trainer is likely to work      |
| US-3 | Linux gamer              | Click a link that takes me to the trainer's download page     | I can download from the original source directly                |
| US-4 | Linux gamer              | See trainer sources from taps I already subscribe to first    | I trust curated community data more than generic search results |
| US-5 | Steam Deck user          | Use trainer discovery offline with tap data                   | I can still find known-good trainer sources at a LAN event      |
| US-6 | Linux gamer              | See an "offline" notice when external search is unavailable   | I understand why results are limited without being confused     |
| US-7 | Linux gamer              | Import a discovered trainer profile directly into CrossHook   | I don't have to manually re-enter trainer and Proton settings   |
| US-8 | Community tap maintainer | Know which metadata fields control discoverability            | I can structure my tap so games surface correctly in search     |

---

## Business Rules

### Core Rules

**BR-1 — No hosting or redistribution**
CrossHook must never fetch, store, or serve trainer binary files. The feature produces links only. Every external URL opened in the user's browser is opened via `tauri::api::shell::open` or equivalent — CrossHook does not download on the user's behalf.

**BR-2 — Tap-first result ordering**
Search results from subscribed community taps are always displayed above external source results. This reflects trust hierarchy: curated tap profiles have been author-reviewed and include SHA-256 hashes, compatibility ratings, and Proton-specific launch settings.

**BR-3 — Version matching is advisory**
Version compatibility is shown as an informational signal (matched / game-updated / trainer-changed / untracked), not an access gate. Users can proceed with any result regardless of version status. Existing `VersionCorrelationStatus` enum in `metadata/models.rs` already models these states.

**BR-4 — Version correlation uses existing snapshot infrastructure**
Version matching for tap-sourced trainers reuses the existing `version_snapshots` table (`metadata/version_store.rs`). Game build ID comes from the Steam manifest (already resolved at launch time). Trainer version comes from `CommunityProfileMetadata.trainer_version`. The `VersionCorrelationStatus` computed by `compute_version_correlation_status()` provides the match signal.

**BR-5 — SHA-256 is a trust signal, not a download gate**
The optional `trainer_sha256` field in `CommunityProfileMetadata` is displayed to the user as a known-good digest. It is not used to approve or reject a link. The existing launch-time hash verification (in `launch/trainer_hash.rs`) is a separate enforcement layer that operates on the local trainer binary after the user downloads and configures it.

**BR-6 — External search requires network, tap data does not**
Tap data is available offline via the locally cloned git workspace. External source queries (ProtonDB-style or any future third-party API) require an active network connection. The offline fallback (BR-7) applies automatically.

**BR-7 — Offline degraded mode**
When no network is available, the discovery panel shows tap-sourced results only and displays a persistent "offline — showing tap data only" notice. This reuses the existing `is_tap_available_offline()` check in `CommunityTapStore`.

**BR-8 — Search is local-first**
Tap-originated **discovery** search runs in-process against the indexed **`trainer_sources`** SQLite table (from `trainer-sources.json`). **Profile browsing** still uses `community_profiles`. No network round-trip is required for Phase A tap discovery results. External source queries (Phase B) fire only when the user opts in and triggers online lookup paths.

**BR-9 — Results are read-only metadata**
The discovery index does not create, modify, or delete user profiles. Import is a separate, explicit user action. Browsing results is zero-side-effect.

**BR-10 — Source trust levels are enumerated, not user-defined**
Trust levels are: `community_tap` (highest — curated), `external_indexed` (medium — known third-party source listed in CrossHook's bundled catalog), `external_search` (lowest — matched from search). Users cannot add custom trust levels.

**BR-11 — No personal data transmitted**
External discovery queries must not include the user's Steam ID, username, installed game list, or system information. Query payloads are restricted to the game title or Steam App ID.

**BR-12 — Tap results are independent of external source availability**
Tap results MUST be displayable without waiting for external source availability. The UI MUST NOT gate tap result rendering on an external fetch completing. When both sources are queried, the frontend issues two separate IPC commands — (synchronous, SQLite) and (async, network) — and renders tap results immediately while external results load progressively. This matches the ProtonDB progressive-load pattern already in use.

### Edge Cases

- A game appears in multiple taps with conflicting trainer versions → all results are shown, grouped by tap with version and compatibility rating visible.
- A tap entry has no `trainer_sha256` → SHA-256 column shows "not verified by tap author."
- A tap entry links to a trainer that is no longer hosted at the listed URL → CrossHook cannot validate link liveness; the user discovers the broken link in their browser. Future: link health check as an optional background task.
- The user searches for a game name that differs from the tap's `game_name` field → fuzzy/substring matching applies across `game_name`, `trainer_name`, `description`, and `platform_tags` (mirrors existing `matchesQuery` logic in `CommunityBrowser.tsx`).
- External search returns a game that has no tap entry → result is shown with `external_indexed` or `external_search` trust level and no SHA-256 or compatibility rating.
- A tap is pinned to a specific commit → discovery still works from that pinned snapshot, which may lag behind the latest trainer releases.
- Version snapshot is missing (trainer never launched) → status shown as `Untracked`, not as an error.

---

## Workflows

### Primary Workflow — Discovery and Import

```
1. User opens "Discover Trainers" panel
2. System loads **`trainer_sources`** from SQLite (tap-sourced, offline-safe), optionally joining **`community_profiles`** for import/compatibility context
3. System checks network availability
   - If offline: show tap results only + offline notice (skip step 4)
   - If online: proceed to step 4
4. System optionally fetches external source results (user-triggered or auto)
   - Fetched results are cached in external_cache_entries (existing table)
   - Cache TTL: configurable, default 6 hours (same as ProtonDB cache)
5. User types a game name or trainer name in the search field
6. Results filter in real-time across all sources, sorted:
   a. Tap results (highest trust), sorted by compatibility_rating desc, then game_name asc
   b. External results (lower trust), sorted by source trust level, then relevance
7. User selects a result to expand detail view
   - Shows: game_name, trainer_name, trainer_version, game_version, proton_version,
     platform_tags, compatibility_rating, author, description, trainer_sha256 (if present),
     version correlation status (if game is installed and build ID is known)
8. User clicks "Get Trainer" → system opens source URL in browser via shell::open
   (CrossHook does not fetch the file)
9. User downloads trainer externally, notes local path
10. If result is tap-sourced and has a community profile:
    a. User clicks "Import Profile" → existing community import workflow activates
    b. Profile is imported with trainer path left blank for user to fill in
    c. Version snapshot is seeded from manifest metadata (existing BR-8/W3 pattern)
```

### Error Recovery Workflows

**Network failure during external search:**
System sets external search state to `failed`, retains tap results, shows "External search unavailable — showing tap data" notice. User can retry explicitly.

**Tap not synced (no local clone):**
Tap entries with no local clone return empty results for that tap with a diagnostic message. User is prompted to sync taps. `is_tap_available_offline()` returns false; the tap is highlighted in the tap management UI.

**Import fails (path validation, schema version mismatch):**
Import workflow delegates to existing `community_prepare_import` → `community_import_profile` error paths. Errors are surfaced in the same modal pattern as existing community import.

**External cache stale or corrupt:**
`evict_expired_cache_entries()` runs on session start. A corrupt payload_json (NULL sentinel) causes the external result to be skipped silently with a debug log.

---

## Domain Model

### Entities

| Entity                     | Storage                          | Notes                                           |
| -------------------------- | -------------------------------- | ----------------------------------------------- |
| `TrainerDiscoveryResult`   | Runtime only                     | Aggregated view: tap entries + external entries |
| `CommunityProfileRow`      | `community_profiles` SQLite      | Tap-sourced trainer metadata, already indexed   |
| `CommunityTapSubscription` | TOML settings (`community_taps`) | User's subscribed tap repositories              |
| `ExternalCacheEntry`       | `external_cache_entries` SQLite  | Cached external source query results            |
| `VersionSnapshotRow`       | `version_snapshots` SQLite       | Trainer/game version correlation history        |
| `TrainerSourceLink`        | Runtime only                     | URL linking result to external download page    |
| `DiscoverySearchState`     | Runtime only                     | Current query, filters, selected result         |

### State Transitions — Discovery Result Lifecycle

```
[No results]
    → tap sync completes → [Tap results available (offline-safe)]
        → user triggers external search → [Fetching external]
            → success → [All results available]
            → failure → [Tap results only + offline notice]

[Tap result selected]
    → version correlation computed → [Match | Untracked | GameUpdated | TrainerChanged]
    → user clicks "Get Trainer" → [Browser opens source URL]
    → user clicks "Import Profile" → [Community import workflow]
```

### State Transitions — Version Correlation per Result

```
Untracked → Matched (after first successful launch with this profile)
Matched → GameUpdated (Steam updates game build ID)
Matched → TrainerChanged (trainer binary hash changes)
Matched → BothChanged (both change)
Any → Unknown (manifest or hash missing)
```

---

## Existing Codebase Integration

### Community Tap Infrastructure (fully implemented)

The entire tap sync, indexing, and import pipeline is already in production:

- **`community/taps.rs`** — `CommunityTapStore`: clones/updates tap git repos, validates URLs/branches/pinned commits. Security hardening: `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`, low-speed abort timeouts.
- **`community/index.rs`** — `CommunityProfileIndex`: walks tap filesystem, parses `community-profile.json` manifests, sorts by `game_name`.
- **`metadata/community_index.rs`** — `index_community_tap_result()`: upserts `community_taps` + transactional DELETE+INSERT into `community_profiles`. Watermark skip on unchanged HEAD commit.
- **`metadata/cache_store.rs`** — `get_cache_entry()` / `put_cache_entry()` / `evict_expired_cache_entries()`: general-purpose TTL cache on top of `external_cache_entries`. Already used by ProtonDB client. **Trainer discovery external search can reuse this directly.**
- **`profile/community_schema.rs`** — `CommunityProfileManifest` / `CommunityProfileMetadata`: schema v1, includes `trainer_sha256` (optional).
- **`profile/exchange.rs`** — `import_community_profile()` / `preview_community_profile_import()`: full import pipeline with validation and SHA-256 propagation.
- **`metadata/version_store.rs`** — `compute_version_correlation_status()`: produces `VersionCorrelationStatus`. Already connected to launch flow.
- **`src-tauri/src/commands/community.rs`** — IPC surface: `community_add_tap`, `community_list_profiles`, `community_sync`, `community_prepare_import`, `community_import_profile`, `community_list_indexed_profiles`.
- **`hooks/useCommunityProfiles.ts`** — React hook: full state management for taps, sync, import, and profile listing. `matchesQuery` in `CommunityBrowser.tsx` implements substring search across all metadata fields.

### External Cache (reusable for external search)

`external_cache_entries` table with `cache_key` uniqueness, `expires_at` TTL, 512 KiB payload cap (`MAX_CACHE_PAYLOAD_BYTES`). ProtonDB client (`protondb/client.rs`) demonstrates the full fetch-then-cache pattern with 6-hour TTL and stale-fallback behavior. Trainer discovery external search should follow the same pattern with a namespace prefix (e.g., `trainer_discovery:game:{app_id}`).

### Version Correlation (already linked to trainer metadata)

`CommunityProfileMetadata.trainer_version` and `.game_version` are already seeded into `version_snapshots` on community import (see `community.rs:community_import_profile`, lines 122–150). The `VersionCorrelationStatus` enum is already fully defined and computed. Discovery UI only needs to read this status and present it.

### Missing Infrastructure (scope of this feature)

1. **External source search**: No external trainer-source API client exists yet. The pattern to follow is `protondb/client.rs` (cache namespace, HTTP client, TTL, stale fallback).
2. **Discovery-specific IPC command**: A `trainer_discovery_search` command is needed to aggregate tap results + external results. `community_list_indexed_profiles` is close but lacks external result aggregation and search-query filtering on the backend.
3. **Frontend discovery panel**: No dedicated trainer-discovery React component exists. `CommunityBrowser.tsx` is the closest precedent (search, filtering, import modal). The new panel can share `useCommunityProfiles` for tap state and add a parallel `useTrainerDiscovery` hook for external results.

---

## Persistence Classification

| Datum                  | Storage                                            | Notes                                                                |
| ---------------------- | -------------------------------------------------- | -------------------------------------------------------------------- |
| Subscribed tap URLs    | TOML settings (`community_taps`)                   | User-editable                                                        |
| Indexed tap profiles   | `community_profiles` SQLite                        | Rebuilt on each tap sync; not directly user-editable                 |
| Trainer source index   | `trainer_sources` SQLite                           | Rebuilt from `trainer-sources.json` on tap sync (`migrate_17_to_18`) |
| External search cache  | `external_cache_entries` SQLite                    | Runtime cache, TTL-evicted; not user-editable                        |
| Version snapshots      | `version_snapshots` SQLite                         | Operational history; user-visible, not directly editable             |
| Discovery search state | Runtime only                                       | No persistence; resets on panel close                                |
| Source link URLs       | `trainer_sources` (+ optional external cache JSON) | Persisted as indexed metadata rows + cache payloads                  |

**Offline behavior**: Tap results always available when taps are synced (local git clone present). External results require network; degraded fallback shows tap-only results with explicit notice.

**Migration/backward compatibility**: Trainer-discovery adds **`trainer_sources`** (**SQLite `user_version` 17 → 18** per `migrations.rs`). Optional Phase C adds an **FTS5 virtual table** (18 → 19). Missing `trainer_sha256` or version fields in manifests are treated as absent/unknown at display time.

---

## Success Criteria

- A user can type a game name and see matching trainer entries from their subscribed taps within 500 ms (local SQLite query, no network).
- A user can click a trainer source link and have it open in their browser without CrossHook fetching the file.
- Tap results are always shown before external results in the ranked list.
- Version correlation status is visible for any game whose Steam build ID is known to CrossHook.
- When offline, the panel shows tap results with a clear offline notice; no error modal fires.
- Import from a discovery result produces the same outcome as importing from `CommunityBrowser`.
- No trainer binary data is stored in the app's SQLite database or local filesystem by this feature.

---

## Open Questions

1. **Which external trainer sources should be indexed?** The business rule requires explicit enumeration of trusted source URLs (e.g., FLiNG Trainers, WeMod). This list needs agreement on inclusion criteria and how it is updated (bundled catalog vs. tap-delivered).
2. **External search query shape**: Should external search be triggered automatically when the user types, or only on explicit "Search Online" button? Auto-trigger risks unexpected network calls; explicit-trigger adds friction.
3. **Link health monitoring**: Should CrossHook perform background HTTP HEAD checks on trainer source URLs to detect broken links? Adds complexity and potential privacy concerns (discloses game interest to source sites).
4. **Version string normalization**: `trainer_version` and `game_version` are free-form strings. How does version matching handle `v1.0` vs `1.0` vs `1.0.0`? Currently compared as raw strings.
5. **Search scope for external results**: Should external search query by game name (string match) or Steam App ID? App ID is unambiguous but requires the game to be in the user's Steam library.
6. **Tap-contributed external source links**: Should tap manifests be able to include a `download_url` field pointing to the trainer's source page? This would let tap authors encode the link per game version, making the "Get Trainer" workflow more precise.
7. **MVP scope decision**: Should the initial implementation be restricted to tap-sourced discovery only (zero new backend infrastructure, no external API client) and external source support treated as a follow-on increment? The implementation guide trigger condition (50+ tap profiles) suggests yes.
