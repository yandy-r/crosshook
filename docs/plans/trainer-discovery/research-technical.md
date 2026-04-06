# Trainer Discovery: Technical Specification

## Executive Summary

Trainer discovery indexes **trainer source links** (not binaries) from community taps via **`trainer-sources.json`** into a new SQLite relational table **`trainer_sources`** (**`migrate_17_to_18`**; SQLite **`user_version`** is **17** in-tree today per `metadata/migrations.rs`). Phase A uses **parameterized LIKE** on **`trainer_sources`**. **Phase C** adds an **FTS5 virtual table** (search-only index, e.g. SQLite FTS5 `content=` sync against `trainer_sources`) — this is **not** an additional business table. Existing **`community_profiles`** and **`external_cache_entries`** remain the profile store and HTTP JSON cache, respectively.

**`crosshook-core::discovery/`** holds models, Phase A **`search.rs`**, Phase B **`client.rs`**, optional **`version_match.rs`**, exposed through Tauri **`commands/discovery.rs`** and React hooks. See **`feature-spec.md`** for authoritative DDL, IPC names, and legal/opt-in defaults (`discovery_enabled` **false**).

**Phases:** **A** = `trainer_sources` + LIKE + UI; **B** = HTTP + `external_cache_entries` + version IPCs; **C** = `rusqlite` **`bundled-full`** + **`migrate_18_to_19`** FTS + **`discovery_rebuild_index`** + ranking.

---

## Implementation Phases

### Phase A: `trainer_sources` + LIKE (MVP)

**Scope:**

- **`migrate_17_to_18`:** `CREATE TABLE trainer_sources` + indexes (copy DDL from `feature-spec.md`).
- Tap walk + **`index_trainer_sources()`** with A6 bounds and HTTPS-only URLs.
- **`discovery/`** module: `models.rs`, `search.rs` (LIKE on **`trainer_sources`**).
- Sync IPC **`discovery_search_trainers`**; frontend hook + panel.

**Note:** **Do not** add `source_url` / `source_name` to `CommunityProfileMetadata` for this feature (Option B).

### Phase B: External cache + HTTP

**Scope:**

- **`discovery/client.rs`**: `OnceLock<reqwest::Client>`, cache → live → stale fallback.
- **`external_cache_entries`** keys **`trainer:source:v1:{normalized_game_key}`**.
- Async IPC: **`discovery_get_trainer_sources`**, **`discovery_search_external`**, **`discovery_check_version_compatibility`** (see `research-architecture.md` for request/response sketches).

### Phase C: FTS5 virtual table + rebuild

**Scope:**

- Enable **`rusqlite` `bundled-full`** (or project-approved FTS-capable features).
- **`migrate_18_to_19`:** FTS5 **virtual** table + triggers/rebuild strategy targeting **`trainer_sources`** text columns used in search.
- **`discovery/search.rs`:** `MATCH` + BM25; **`discovery_rebuild_index`**; LIKE fallback if FTS fails to open.

**Documentation split:** This file stays the technical companion; **`feature-spec.md`** owns acceptance criteria and phase timelines.

---

## Architecture Design

### Component Diagram

```
Community Taps (git repos)    External Trainer Sources (web)
        |                              |
        v                              v
  CommunityTapStore            DiscoveryClient (HTTP)
        |                              |
        v                              v
CommunityProfileIndex        external_cache_entries (SQLite)
        |                              |
        +----------+-------------------+
                   v
        TrainerDiscoveryService (crosshook-core::discovery)
           |              |              |
           v              v              v
     search_trainers  version_match  get_sources
           |              |              |
           +----------+-------------------+
                      v
          MetadataStore (SQLite queries)
                      v
            Tauri IPC Commands
           (#[tauri::command])
                      v
         useTrainerDiscovery (React hook)
                      v
        TrainerDiscoveryPanel (React component)
```

### New Components

#### Rust (`crosshook-core`)

| Component                    | Location                        | Phase              | Purpose                                                                                 |
| ---------------------------- | ------------------------------- | ------------------ | --------------------------------------------------------------------------------------- |
| `discovery/mod.rs`           | `crosshook-core/src/discovery/` | A                  | Module root, re-exports                                                                 |
| `discovery/models.rs`        | same                            | A                  | `TrainerSearchQuery`, `TrainerSearchResult`, `TrainerSource`, `VersionMatch` structs    |
| `discovery/search.rs`        | same                            | A (LIKE), C (FTS5) | Search logic: query parsing, SQL generation, result mapping                             |
| `discovery/version_match.rs` | same                            | B                  | Advisory comparison using `version_snapshots` + `compute_correlation_status`            |
| `discovery/client.rs`        | same                            | B                  | HTTP + `external_cache_entries` (ProtonDB-style)                                        |
| `metadata/migrations.rs`     | `crosshook-core/src/metadata/`  | A, C               | **`migrate_17_to_18`** (`trainer_sources`); **`migrate_18_to_19`** (FTS5 virtual table) |

#### Tauri Commands

| Command                                 | Location                              | Phase | Purpose                                          |
| --------------------------------------- | ------------------------------------- | ----- | ------------------------------------------------ |
| `discovery_search_trainers`             | `src-tauri/src/commands/discovery.rs` | A     | LIKE search on **`trainer_sources`**             |
| `discovery_get_trainer_sources`         | same                                  | B     | Async cached external resolution                 |
| `discovery_check_version_compatibility` | same                                  | B     | Advisory version compare via `version_snapshots` |
| `discovery_rebuild_index`               | same                                  | C     | Rebuild FTS5 index over `trainer_sources`        |

#### React/TypeScript

| Component                   | Location          | Phase | Purpose                                              |
| --------------------------- | ----------------- | ----- | ---------------------------------------------------- |
| `useTrainerDiscovery.ts`    | `src/hooks/`      | 1     | Hook wrapping IPC calls with loading/error state     |
| `TrainerDiscoveryPanel.tsx` | `src/components/` | 1     | Search UI with results, version badges, source links |
| `types/discovery.ts`        | `src/types/`      | 1     | TypeScript interfaces mirroring Rust Serde models    |

### Integration Points

- **Community tap sync** (`community_sync` command): After `community_sync` indexes profiles into `community_profiles`, trigger FTS re-index for the affected tap (Phase 2).
- **ProtonDB lookup** (existing): Cross-reference ProtonDB tier with discovered trainers to show compatibility context.
- **Version snapshots** (`version_snapshots` table): Compare installed game build ID against community profile `game_version` for version matching (Phase 2).
- **Profile import** (`community_import_profile`): Discovery results link directly to the existing community import flow.
- **External cache** (`external_cache_entries`): Phase 2 uses the same `MetadataStore::put_cache_entry`/`get_cache_entry` pattern as ProtonDB for external source lookups. Key constraint: `MAX_CACHE_PAYLOAD_BYTES` (512 KiB) cap means individual cache entries must stay small. Aggregate indexes would need a dedicated table (like `game_image_cache` does for images).

---

## Data Models

### Existing tables (profile + cache — unchanged for Option B)

#### `community_profiles` (migration v4, rebuilt v5)

| Column                 | Type       | Notes                                                 |
| ---------------------- | ---------- | ----------------------------------------------------- |
| `id`                   | INTEGER PK | Auto-increment                                        |
| `tap_id`               | TEXT FK    | References `community_taps(tap_id)` ON DELETE CASCADE |
| `relative_path`        | TEXT       | Path within tap workspace                             |
| `manifest_path`        | TEXT       | Absolute path on disk                                 |
| `game_name`            | TEXT       | Searchable                                            |
| `game_version`         | TEXT       | For version matching                                  |
| `trainer_name`         | TEXT       | Searchable                                            |
| `trainer_version`      | TEXT       | For version matching                                  |
| `proton_version`       | TEXT       | Filter criterion                                      |
| `compatibility_rating` | TEXT       | `unknown\|broken\|partial\|working\|platinum`         |
| `author`               | TEXT       | Searchable                                            |
| `description`          | TEXT       | Searchable                                            |
| `platform_tags`        | TEXT       | Space-joined tags, filterable                         |
| `schema_version`       | INTEGER    | Must equal `COMMUNITY_PROFILE_SCHEMA_VERSION` (1)     |
| `created_at`           | TEXT       | RFC3339 timestamp                                     |

Index: `UNIQUE(tap_id, relative_path)`

#### `community_taps` (migration v4)

| Column             | Type    | Notes                       |
| ------------------ | ------- | --------------------------- |
| `tap_id`           | TEXT PK | UUID                        |
| `tap_url`          | TEXT    | Git remote URL              |
| `tap_branch`       | TEXT    | Branch name (default `""`)  |
| `local_path`       | TEXT    | On-disk workspace path      |
| `last_head_commit` | TEXT    | HEAD SHA for watermark skip |
| `profile_count`    | INTEGER | Cached count                |
| `last_indexed_at`  | TEXT    | RFC3339 timestamp           |
| `created_at`       | TEXT    |                             |
| `updated_at`       | TEXT    |                             |

Index: `UNIQUE(tap_url, tap_branch)`

#### `external_cache_entries` (migration v4)

Used in Phase 2 to cache external trainer source metadata (e.g., page scrape summaries, version lists from known trainer sites).

| Column         | Type        | Notes                                                                         |
| -------------- | ----------- | ----------------------------------------------------------------------------- |
| `cache_id`     | TEXT PK     | UUID                                                                          |
| `source_url`   | TEXT        | Origin URL                                                                    |
| `cache_key`    | TEXT UNIQUE | Namespace-prefixed key (e.g., `trainer:source:v1:{game_name}:{trainer_name}`) |
| `payload_json` | TEXT        | JSON payload (max 512 KiB per `MAX_CACHE_PAYLOAD_BYTES`)                      |
| `payload_size` | INTEGER     | Byte count                                                                    |
| `fetched_at`   | TEXT        | RFC3339                                                                       |
| `expires_at`   | TEXT        | TTL boundary                                                                  |
| `created_at`   | TEXT        |                                                                               |
| `updated_at`   | TEXT        |                                                                               |

### Schema changes (SQLite `user_version` 17 → 18 → 19)

#### Phase A: `trainer_sources` relational table (`migrate_17_to_18`)

Use the **`CREATE TABLE trainer_sources`** statement from **`feature-spec.md`** (Decision 1 / data models). Index from **`trainer-sources.json`** during tap sync; do **not** `ALTER TABLE community_profiles` for discovery URLs.

#### Phase C: FTS5 **virtual** table (`migrate_18_to_19`)

Add **`rusqlite` features** enabling FTS5 (`bundled-full`). Create a **virtual** table, e.g. `trainer_sources_fts`, using FTS5 **`content='trainer_sources'`** and **`content_rowid='id'`** (or equivalent) so SQLite maintains the index against existing rows — **no duplicate authoritative source rows**. Example shape (finalize column list in implementation):

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS trainer_sources_fts USING fts5(
    game_name,
    source_name,
    notes,
    content='trainer_sources',
    content_rowid='id'
);
-- Triggers or `INSERT INTO trainer_sources_fts(trainer_sources_fts) VALUES('rebuild')` per SQLite docs; see also discovery_rebuild_index.
```

**Maintenance:** **`discovery_rebuild_index`** repopulates or rebuilds the FTS segment; **`discovery/search.rs`** uses **`MATCH`** in Phase C and **parameterized LIKE** in Phase A/B fallback.

### Rust Structs

```rust
// crosshook-core/src/discovery/models.rs

use serde::{Deserialize, Serialize};

/// Search query from the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchQuery {
    pub query: String,
    #[serde(default)]
    pub game_name_filter: Option<String>,
    #[serde(default)]
    pub compatibility_filter: Option<String>,
    #[serde(default)]
    pub platform_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// Single search result returned to the frontend.
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
    /// Source URL from the community profile manifest, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Source name from the community profile manifest, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// Relevance score from FTS5 rank (Phase C); 0.0 in Phase A/B when using LIKE.
    pub relevance_score: f64,
    /// Version match status against the user's installed game, if known (Phase 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_match: Option<VersionMatchResult>,
}

/// Paginated search response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchResponse {
    pub results: Vec<TrainerSearchResult>,
    pub total_count: i64,
    pub query: TrainerSearchQuery,
}

/// Resolved download source for a trainer.
/// CrossHook does NOT host trainers; this points to original source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSource {
    /// Display label (e.g., "FLiNG Trainer", "WeMod Community").
    pub source_name: String,
    /// External URL where the trainer can be obtained.
    pub download_url: String,
    /// Whether this source was resolved from a community tap manifest.
    pub from_community_tap: bool,
    /// Tap URL if from a community tap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tap_url: Option<String>,
    /// SHA-256 of the trainer executable, if known from the manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_sha256: Option<String>,
}

/// Version comparison between available trainer and installed game.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionMatchStatus {
    /// Trainer version matches installed game version exactly.
    Exact,
    /// Trainer is for a compatible (older) game version.
    Compatible,
    /// Trainer is for a newer game version than installed.
    NewerAvailable,
    /// Trainer is for an older game version; game has been updated.
    Outdated,
    /// Cannot determine match (missing version data).
    Unknown,
}

/// Result of comparing a trainer version against an installed game version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMatchResult {
    pub status: VersionMatchStatus,
    pub trainer_game_version: String,
    pub installed_game_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
```

### TypeScript Interfaces

```typescript
// src/types/discovery.ts

export type VersionMatchStatus = 'exact' | 'compatible' | 'newer_available' | 'outdated' | 'unknown';

export interface TrainerSearchQuery {
  query: string;
  gameNameFilter?: string;
  compatibilityFilter?: string;
  platformFilter?: string;
  limit?: number;
  offset?: number;
}

export interface VersionMatchResult {
  status: VersionMatchStatus;
  trainerGameVersion: string;
  installedGameVersion: string;
  detail?: string;
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

export interface TrainerSource {
  sourceName: string;
  downloadUrl: string;
  fromCommunityTap: boolean;
  tapUrl?: string;
  trainerSha256?: string;
}

export interface TrainerSearchResponse {
  results: TrainerSearchResult[];
  totalCount: number;
  query: TrainerSearchQuery;
}
```

---

## API Design

### IPC Command: `discovery_search_trainers`

**Phase:** 1 (LIKE), upgraded to FTS5 in Phase 2

**Direction:** Frontend -> Backend

**Request:**

```typescript
invoke<TrainerSearchResponse>('discovery_search_trainers', {
  query: {
    query: 'Elden Ring',
    compatibilityFilter: 'working',
    platformFilter: 'linux',
    limit: 20,
    offset: 0,
  },
  installedAppId: '1245620', // optional: enables version matching (Phase 2)
});
```

**Rust signature:**

```rust
#[tauri::command]
pub fn discovery_search_trainers(
    query: TrainerSearchQuery,
    installed_app_id: Option<String>,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<TrainerSearchResponse, String>
```

**Response:**

```json
{
  "results": [
    {
      "communityProfileId": 42,
      "gameName": "Elden Ring",
      "gameVersion": "1.12.3",
      "trainerName": "FLiNG Trainer",
      "trainerVersion": "v1.12.3",
      "protonVersion": "9.0-4",
      "compatibilityRating": "working",
      "author": "crosshook-user",
      "description": "Known-good launch profile for FLiNG trainer",
      "platformTags": ["linux", "steam-deck"],
      "tapUrl": "https://github.com/community/crosshook-taps.git",
      "relativePath": "profiles/elden-ring/community-profile.json",
      "manifestPath": "/home/user/.local/share/crosshook/community/taps/...",
      "sourceUrl": "https://flingtrainer.com/elden-ring",
      "sourceName": "FLiNG Trainer",
      "relevanceScore": 12.5,
      "versionMatch": {
        "status": "exact",
        "trainerGameVersion": "1.12.3",
        "installedGameVersion": "1.12.3"
      }
    }
  ],
  "totalCount": 3,
  "query": { "query": "Elden Ring", "limit": 20, "offset": 0 }
}
```

**Errors:** Empty query -> `"search query cannot be empty"`, DB unavailable -> graceful empty results.

**Phase A SQL (LIKE on `trainer_sources`)** — canonical copy in `feature-spec.md`:

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

**Phase C SQL (FTS5 on `trainer_sources_fts`)** — virtual table; column list must match migration:

```sql
SELECT ts.id, ts.game_name, ts.steam_app_id, ts.source_name,
       ts.source_url, ts.trainer_version, ts.game_version,
       ts.notes, ts.sha256, ts.relative_path,
       ct.tap_url,
       trainer_sources_fts.rank AS relevance_score
FROM trainer_sources_fts
JOIN trainer_sources ts ON trainer_sources_fts.rowid = ts.id
JOIN community_taps ct ON ts.tap_id = ct.tap_id
WHERE trainer_sources_fts MATCH ?1
ORDER BY relevance_score
LIMIT ?2 OFFSET ?3
```

### IPC Command: `discovery_get_trainer_sources` (Phase 2)

**Request:**

```typescript
invoke<TrainerSource[]>('discovery_get_trainer_sources', {
  communityProfileId: 42,
});
```

**Rust signature:**

```rust
#[tauri::command]
pub fn discovery_get_trainer_sources(
    community_profile_id: i64,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<TrainerSource>, String>
```

**Response:** Array of `TrainerSource` objects. For community tap entries, the source is derived from the manifest `source_url` field, falling back to the tap URL. For external cached sources, the `download_url` comes from `external_cache_entries.source_url`.

**Errors:** Invalid/missing ID -> `"community profile not found: {id}"`.

### IPC Command: `discovery_check_version_compatibility` (Phase 2)

**Request:**

```typescript
invoke<VersionMatchResult>('discovery_check_version_compatibility', {
  communityProfileId: 42,
  profileName: 'my-elden-ring',
});
```

**Rust signature:**

```rust
#[tauri::command]
pub fn discovery_check_version_compatibility(
    community_profile_id: i64,
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<VersionMatchResult, String>
```

**Logic:**

1. Load community profile row by ID to get `game_version` and `trainer_version`.
2. Load user's profile by name to get `steam.app_id`.
3. Look up latest `version_snapshots` row for the user's profile_id to get `human_game_ver` or `steam_build_id`.
4. Run version comparison algorithm (see System Constraints).
5. Return `VersionMatchResult`.

### IPC Command: `discovery_rebuild_index` (Phase C)

**Request:**

```typescript
invoke<{ ok: boolean; rowsIndexed: number }>('discovery_rebuild_index', {});
```

**Rust signature:**

```rust
#[tauri::command]
pub fn discovery_rebuild_index(
    metadata_store: State<'_, MetadataStore>,
) -> Result<DiscoveryRebuildIndexResponse, String>
```

Rebuilds the **FTS5** segment(s) for **`trainer_sources`** (not `community_profiles`). Used after corruption, FTS upgrade, or manual repair; idempotent.

---

## System Constraints

### Performance

| Concern                       | Target                         | Implementation                                                           |
| ----------------------------- | ------------------------------ | ------------------------------------------------------------------------ |
| Search latency (Phase A)      | < 200ms for typical queries    | LIKE with LIMIT on `trainer_sources`                                     |
| Search latency (Phase C)      | < 50ms typical (target)        | FTS5 `MATCH` + rank on `trainer_sources_fts`                             |
| Index rebuild                 | < 2s for 10k source rows       | Rebuild/populate FTS from `trainer_sources`                              |
| Cache TTL (external, Phase B) | 24h fresh / 7d stale (tunable) | `external_cache_entries.expires_at`; align with `discovery/client.rs`    |
| FTS index size                | ~10–20% of indexed text        | `content='trainer_sources'` avoids duplicating business columns          |
| Max results per page          | 50                             | Capped in Rust; `MAX_DISCOVERY_RESULTS_PER_PAGE: usize = 50`             |
| External cache payload        | 512 KiB per entry              | `MAX_CACHE_PAYLOAD_BYTES` constraint; large indexes need dedicated table |

### Version Matching Algorithm (Phase 2)

```
1. Normalize both versions: strip leading "v"/"V", trim whitespace
2. Parse as semver (major.minor.patch) if possible
3. If both parse:
   - Exact: all components match -> Exact
   - Trainer game_version < installed -> Outdated
   - Trainer game_version > installed -> NewerAvailable
   - Same major.minor, different patch -> Compatible
4. If either fails to parse:
   - String equality check -> Exact or Unknown
5. If trainer has no game_version -> Unknown
```

### Offline Behavior

| Scenario                         | Behavior                                                                   |
| -------------------------------- | -------------------------------------------------------------------------- |
| Taps synced, network down        | Search works from local index built from last sync                         |
| Taps never synced                | Empty results, prompt to add/sync a community tap                          |
| External cache expired (Phase 2) | Stale results returned with `is_stale: true` flag                          |
| FTS index corrupt (Phase C)      | `discovery_rebuild_index`; optional LIKE fallback if virtual table missing |

### Search Query Safety

- All queries use parameterized SQL (`?` bind params), not string interpolation.
- Query text is trimmed and limited to 500 bytes before search.
- Phase A LIKE queries: `%` / `_` wrapping is server-side only; bind user text as a single parameter.
- Phase C FTS5: sanitize or quote user tokens before `MATCH` to avoid operator injection.
- Empty/whitespace-only queries return an error, not unbounded results.

### Business Rules and Validation

The following rules are derived from business analysis of existing data model constraints:

#### Result Type Architecture

- `TrainerDiscoveryResult` (the `TrainerSearchResult` struct in the spec) is a **runtime-only aggregated view type** -- never persisted. It merges `CommunityProfileRow` data (tap sources) with external cache payload entries.
- Discovery results for games the user has not imported yet have **no version correlation**. Version correlation requires a known `profile_id` (the game must exist as a local profile with a `version_snapshots` row). The `discovery_check_version_compatibility` command must validate that the named profile exists before attempting correlation.
- The existing `VersionCorrelationStatus` enum at `metadata/models.rs` (`Untracked`, `Matched`, `GameUpdated`, `TrainerChanged`, `BothChanged`, `Unknown`, `UpdateInProgress`) and the pure function `compute_correlation_status` at `metadata/version_store.rs:185` provide the established pattern. Discovery version matching should adapt this pattern, not duplicate the enum.

#### Source Trust Levels (Runtime, Not Persisted)

Source trust is computed at query time based on origin, and affects result sort order:

| Trust Level        | Origin                                                   | Sort Priority |
| ------------------ | -------------------------------------------------------- | ------------- |
| `community_tap`    | `community_profiles` rows from synced taps               | Highest       |
| `external_indexed` | `external_cache_entries` from known, curated source APIs | Medium        |
| `external_search`  | `external_cache_entries` from ad-hoc search queries      | Lowest        |

#### Validation Rules

| Field                | Rule                                                                                                                                       | Existing Code Reference                                                      |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| `trainer_sha256`     | When present, must be exactly 64 hex characters (SHA-256).                                                                                 | `offline::hash::normalize_sha256_hex` at `offline/hash.rs:84-95`             |
| External source URLs | Must use `https://` only.                                                                                                                  | `community::taps::validate_tap_url` at `taps.rs:485-491`                     |
| Search query strings | Trimmed, capped at 512 chars before any external API call.                                                                                 | New validation in `discovery/search.rs`                                      |
| Field length bounds  | Enforced on INSERT to `community_profiles`: game_name 512B, trainer_name 512B, description 4096B, platform_tags 2048B, versions 256B each. | `metadata::community_index::check_a6_bounds` at `community_index.rs:259-328` |

#### Cache Key Namespacing

External discovery cache entries use namespaced keys in `external_cache_entries`:

- `trainer_discovery:game:{steam_app_id}` -- cached source data for a specific game
- `trainer_discovery:search:{query_slug}` -- cached search results from external APIs

This follows the ProtonDB pattern (`protondb:{app_id}` cache keys) and avoids collisions with other cache consumers.

---

## Codebase Changes

### Files to Create

| File                                             | Phase | Purpose                                                                                   |
| ------------------------------------------------ | ----- | ----------------------------------------------------------------------------------------- |
| `crosshook-core/src/discovery/mod.rs`            | 1     | Module root: re-exports                                                                   |
| `crosshook-core/src/discovery/models.rs`         | 1     | Data types: `TrainerSearchQuery`, `TrainerSearchResult`, `TrainerSource`, `VersionMatch*` |
| `crosshook-core/src/discovery/search.rs`         | 1     | LIKE query builder (Phase 1), FTS5 query builder (Phase 2), result mapping, pagination    |
| `crosshook-core/src/discovery/version_match.rs`  | 2     | Version comparison algorithm                                                              |
| `crosshook-core/src/discovery/service.rs`        | 2     | `TrainerDiscoveryService` orchestration                                                   |
| `crosshook-core/src/metadata/discovery_index.rs` | 2     | FTS5 table operations: search, rebuild, count                                             |
| `src-tauri/src/commands/discovery.rs`            | 1     | Tauri IPC command handlers                                                                |
| `src/hooks/useTrainerDiscovery.ts`               | 1     | React hook wrapping discovery IPC                                                         |
| `src/types/discovery.ts`                         | 1     | TypeScript interfaces                                                                     |
| `src/components/TrainerDiscoveryPanel.tsx`       | 1     | Search UI component                                                                       |

### Files to Modify

| File                                             | Phase | Change                                                                         |
| ------------------------------------------------ | ----- | ------------------------------------------------------------------------------ |
| `crosshook-core/src/lib.rs`                      | 1     | Add `pub mod discovery;`                                                       |
| `crosshook-core/src/profile/community_schema.rs` | 1     | Add optional `source_url`, `source_name` to `CommunityProfileMetadata`         |
| `crosshook-core/src/metadata/community_index.rs` | 1     | Persist `source_url`, `source_name` during indexing                            |
| `crosshook-core/src/metadata/models.rs`          | 1     | Add `source_url`, `source_name` to `CommunityProfileRow`                       |
| `crosshook-core/src/metadata/migrations.rs`      | 1     | Add `migrate_17_to_18` for ALTER TABLE + FTS5                                  |
| `crosshook-core/src/metadata/mod.rs`             | 1     | Add `mod discovery_index;` (Phase 2), expose search methods on `MetadataStore` |
| `src-tauri/src/commands/mod.rs`                  | 1     | Add `pub mod discovery;`                                                       |
| `src-tauri/src/commands/community.rs`            | 2     | After `community_sync` index sync, trigger FTS rebuild for the synced tap      |
| `src/types/index.ts`                             | 1     | Re-export discovery types                                                      |
| `src/hooks/useCommunityProfiles.ts`              | 1     | Update `CommunityProfileMetadata` interface with `source_url?`, `source_name?` |

### Dependencies

No new crate dependencies required. The implementation uses:

- `rusqlite` (existing) for LIKE/FTS5 queries
- `serde` (existing) for IPC serialization
- `chrono` (existing) for timestamps
- `reqwest` (existing, Phase 2) for external source HTTP client via `OnceLock` pattern
- Standard library `semver` parsing can be done manually (avoid adding `semver` crate for a single function)

### Reusable Code Inventory

Existing code that should be reused directly or adapted -- do not reimplement:

#### Use Directly (No Modification)

| Module                  | Functions                                                           | Purpose for Discovery                                                                                                                        |
| ----------------------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `metadata::cache_store` | `get_cache_entry`, `put_cache_entry`, `evict_expired_cache_entries` | Phase 2 external source caching. TTL, stale fallback, and 512 KiB size guards are already implemented.                                       |
| `offline::hash`         | `normalize_sha256_hex`, `verify_and_cache_trainer_hash`             | Cross-check community-published `trainer_sha256` digests against local trainer files. The SQLite-backed hash cache avoids redundant hashing. |
| `protondb::client`      | `OnceLock<reqwest::Client>` pattern, `PROTONDB_HTTP_CLIENT`         | Phase 2 HTTP client for external source lookups. Clone the pattern with a `DISCOVERY_HTTP_CLIENT` static.                                    |

#### Adapt (Same Pattern, Different Data)

| Module                      | Functions                                                   | Adaptation                                                                                                                                                                                                                                                                            |
| --------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `install::discovery`        | `tokenize`, `token_hits`, `contains_any`, `target_tokens`   | Adapt for trainer name matching in search scoring. `tokenize` splits on non-alphanumeric chars, filters tokens < 2 chars. `token_hits` counts substring matches against target tokens. These can replace or supplement LIKE queries in Phase 1 for client-side re-ranking of results. |
| `metadata::version_store`   | `compute_correlation_status`                                | Pure function pattern to follow for version matching. Trainer versions are not semver, so the algorithm needs adaptation, but the function structure (normalize, compare, classify) should be mirrored.                                                                               |
| `metadata::community_index` | `index_community_tap_result`, `list_community_tap_profiles` | The transactional DELETE+INSERT watermark pattern for tap re-indexing. Discovery search queries follow the same JOIN pattern (`community_profiles JOIN community_taps`).                                                                                                              |

#### Frontend Patterns to Reuse

| File                              | Pattern                                         | Purpose for Discovery                                                                                                                                                                         |
| --------------------------------- | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CommunityBrowser.tsx:36`         | `matchesQuery(entry, query)`                    | Client-side substring filter across game_name, trainer_name, author, description. Reuse in `TrainerDiscoveryPanel` for initial filtering or as a fallback when backend search is unavailable. |
| `useProtonDbSuggestions.ts:27-67` | Request-ID cancellation pattern                 | Stale-request protection for `useTrainerDiscovery` hook. Increment `requestIdRef` on each search, discard responses for stale IDs.                                                            |
| `useProtonDbLookup.ts:54-60`      | `normalizeLookupResult` defensive normalization | Normalize backend discovery responses to handle missing optional fields from Serde.                                                                                                           |

---

## Technical Decisions

### Decision 1: Phased approach (trainer_sources → HTTP cache → FTS)

| Slice       | Deliverable                                                     |
| ----------- | --------------------------------------------------------------- |
| **Phase A** | `trainer_sources` + LIKE + `discovery/` + IPC + UI              |
| **Phase B** | `discovery/client.rs` + `external_cache_entries` + async IPC    |
| **Phase C** | `bundled-full` + FTS5 virtual table + `discovery_rebuild_index` |

**Recommendation:** Ship A before B; C follows B (see `feature-spec.md` timelines). Row counts in `trainer_sources` drive FTS need, not `community_profiles` row count alone.

### Decision 2: FTS5 vs LIKE

| Option     | Use when                                       |
| ---------- | ---------------------------------------------- |
| LIKE (A/B) | Default build (`bundled`); moderate row counts |
| FTS5 (C)   | `bundled-full`; large catalogs; BM25 ranking   |

**Recommendation:** FTS5 targets **`trainer_sources`** via **`content=`** sync — not a second relational table.

### Decision 3: FTS maintenance (Phase C)

**Recommendation:** Prefer triggers keeping `trainer_sources_fts` aligned with `trainer_sources` INSERT/DELETE/UPDATE; expose **`discovery_rebuild_index`** for recovery after corruption or migration.

### Decision 4: Version matching location (Phase 2)

| Option                    | Pros                                                | Cons                                     |
| ------------------------- | --------------------------------------------------- | ---------------------------------------- |
| **Backend (recommended)** | Consistent, testable, access to `version_snapshots` | Slightly more IPC data                   |
| Frontend                  | Less backend code                                   | Version logic duplicated, harder to test |

**Recommendation:** Backend. Version snapshots are in SQLite; the comparison needs profile_id resolution and snapshot lookup, both of which are backend-only operations. The ProtonDB suggestions pattern (backend-side `derive_suggestions`) validates this approach.

### Decision 5: Search result enrichment (version match on search vs. on demand)

| Option                      | Pros                                                         | Cons                                                                           |
| --------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| On search (batch)           | Single round-trip, all results enriched                      | Slower search if many results; version data may not be available for all games |
| **On demand (recommended)** | Fast search response, version match only for selected result | Extra IPC call per result the user inspects                                    |

**Recommendation:** On-demand via `discovery_check_version_compatibility`. Search results include raw `game_version` from community profiles; version matching is invoked when the user selects a specific result. This follows the ProtonDB pattern where `protondb_lookup` and `protondb_get_suggestions` are separate commands. For a future optimization, the search command can accept an optional `installed_app_id` to batch-enrich the first page.

### Decision 6: External source caching pattern (Phase B)

**Recommendation:** Reuse **`external_cache_entries`** with keys **`trainer:source:v1:{normalized_key}`** for HTTP-fetched JSON (512 KiB cap). **Tap-originated rows** live in **`trainer_sources`** (relational).

---

## Resolved follow-ups

1. **Community profile schema**: Option B — **no** `source_url` / `source_name` on `CommunityProfileMetadata` for discovery; use **`trainer-sources.json`**.
2. **Cross-tap duplicates**: Show all with tap attribution (BR-2 tap-first ordering).
3. **External scope**: FLiNG RSS + PCGW normalization per **`feature-spec.md` Decision 3**; legal review before adding sources.
4. **FTS query UX**: Plain-text queries in UI; sanitize before `MATCH` (no raw FTS operator exposure).
5. **Search scope**: Default = all subscribed taps; optional filter by `tap_id` in later UI iteration.
