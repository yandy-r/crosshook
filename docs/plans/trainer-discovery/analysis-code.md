# Trainer Discovery — Code Pattern Analysis

> **Correction note**: An earlier draft of this document incorrectly described adding `source_url`/`source_name` columns to `community_profiles`. The authoritative `feature-spec.md` (Decision 1, Option B) establishes a **separate `trainer_sources` table** with an associated `trainer-sources.json` file per game directory in each tap. This document reflects the correct architecture.

## Executive Summary

The trainer-discovery feature (Phase A) requires: one new SQLite table (`trainer_sources`), a new `trainer-sources.json` file format per tap game directory, a tap directory walker extension, a new indexer function, a new `discovery/` module in `crosshook-core`, one thin IPC command file, and a frontend hook + panel. All architectural patterns are established by existing code — no new dependencies. Phase B adds an HTTP client cloned from `protondb/client.rs`. The change surface is moderate but well-contained because the `trainer_sources` table is fully independent of `community_profiles`.

---

## Existing Code Structure

### Rust — crosshook-core module layout

```
src/crosshook-native/crates/crosshook-core/src/
├── lib.rs                          — crate root; add `pub mod discovery;` here
├── metadata/
│   ├── mod.rs                      — MetadataStore facade + pub re-exports
│   ├── migrations.rs               — sequential migration chain (currently v0→v17 in-tree; trainer-discovery adds v18 / `trainer_sources`)
│   ├── community_index.rs          — transactional DELETE+INSERT indexer, A6 bounds
│   ├── models.rs                   — CommunityProfileRow, MetadataStoreError, consts
│   ├── cache_store.rs              — get_cache_entry / put_cache_entry (TTL cache)
│   └── version_store.rs            — compute_correlation_status (pure)
├── protondb/
│   ├── client.rs                   — OnceLock HTTP client, cache-first fetch, stale fallback
│   └── models.rs                   — Serde conventions reference
├── profile/
│   └── community_schema.rs         — CommunityProfileMetadata, CommunityProfileManifest
└── install/
    └── discovery.rs                — Candidate struct, tokenize/token_hits (reusable)
```

### Tauri IPC layer

```
src/crosshook-native/src-tauri/src/
├── lib.rs                          — managed state registration + invoke_handler! list
└── commands/
    ├── mod.rs                      — pub mod registry; add `pub mod discovery;`
    ├── community.rs                — sync IPC reference; IPC contract test block at lines 311–353
    └── protondb.rs                 — async IPC reference; `.inner().clone()` for await boundary
```

### Frontend

```
src/crosshook-native/src/
├── hooks/
│   ├── useProtonDbSuggestions.ts   — requestIdRef race guard pattern, full hook shape
│   ├── useCommunityProfiles.ts     — community data hook; type mirroring reference
│   └── useScrollEnhance.ts        — SCROLLABLE selector; new containers must be added here
├── components/
│   ├── CommunityBrowser.tsx        — client-side matchesQuery() search reference
│   └── pages/CommunityPage.tsx     — host page; TrainerDiscoveryPanel goes here
└── types/index.ts                  — type barrel; add `export * from './discovery'`
```

---

## Implementation Patterns

### 1. Migration Chain (`metadata/migrations.rs`)

Every migration follows the identical waterfall guard pattern:

```rust
if version < N {
    migrate_N_minus_1_to_N(conn)?;
    conn.pragma_update(None, "user_version", N_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "set user_version to N",
            source,
        })?;
}
```

The v18 migration creates the `trainer_sources` table (exact SQL from `feature-spec.md:221–239`):

```rust
fn migrate_17_to_18(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
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
        CREATE INDEX IF NOT EXISTS idx_trainer_sources_game
            ON trainer_sources(game_name);
        CREATE INDEX IF NOT EXISTS idx_trainer_sources_app_id
            ON trainer_sources(steam_app_id);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 17 to 18",
        source,
    })
}
```

Migration tests use `MetadataStore::open_in_memory()` → `run_migrations(&conn)` then assert table structure via `sqlite_master`. The `migration_16_to_17_creates_suggestion_dismissals_table` test at line 961 is the direct template — verify table existence, verify indexes, optionally verify FK cascade.

### 2. Community Index Extension — Directory Walker

The existing tap walker in `community/index.rs` finds `community-profile.json` files. Phase A must also discover `trainer-sources.json` files in the same game directories. The directory structure is:

```
tap-repo/profiles/elden-ring/
  community-profile.json    ← existing
  trainer-sources.json      ← new, optional alongside profile
```

The walker extension must:

1. Check if a `trainer-sources.json` exists alongside (or independent of) `community-profile.json`
2. Parse it into `TrainerSourcesManifest`
3. Return source entries for indexing

This is a modification to `crosshook-core/src/community/index.rs`, not `metadata/community_index.rs`.

### 3. Trainer Sources Indexer (`metadata/community_index.rs`)

A new `index_trainer_sources()` function follows `index_community_tap_result()` exactly:

**Key structural differences from `index_community_tap_result`:**

- The `UNIQUE(tap_id, relative_path, source_url)` constraint on `trainer_sources` allows `INSERT OR REPLACE` (upsert-style) instead of DELETE+INSERT — multiple sources per file, different URLs
- But to stay consistent with the existing pattern and avoid stale entries on tap re-index, DELETE+INSERT on `(tap_id)` remains correct
- A6 bounds apply to all text fields — add constants alongside the existing `MAX_GAME_NAME_BYTES` etc.

```rust
// New constants for trainer_sources field bounds
const MAX_SOURCE_NAME_BYTES: usize = 512;
const MAX_SOURCE_URL_BYTES: usize = 2_048;
const MAX_NOTES_BYTES: usize = 4_096;
```

**URL validation** (S2 security requirement — HTTPS-only at index time):

```rust
fn is_valid_https_url(url: &str) -> bool {
    url.starts_with("https://")
        && url.len() <= MAX_SOURCE_URL_BYTES
        && !url.contains('\n')
        && !url.contains('\r')
}
```

Entries with invalid URLs should be skipped with `tracing::warn!` (same pattern as A6 violations in `check_a6_bounds`).

The DELETE+INSERT transaction for trainer sources should run within the **same transaction** as the community profiles re-index — both are keyed on `tap_id`. The watermark skip logic (comparing `last_head_commit`) already covers both in `index_community_tap_result`.

### 4. MetadataStore Facade Pattern (`metadata/mod.rs:97–159`)

Three accessor patterns; choose based on return type needs:

| Method                        | Returns                              | Use when                                 |
| ----------------------------- | ------------------------------------ | ---------------------------------------- |
| `with_conn(action, f)`        | `Result<T, E>` where `T: Default`    | T: Default (Vec, bool, etc.)             |
| `with_conn_mut(action, f)`    | `Result<T, E>` where `T: Default`    | Needs `&mut Connection` for transactions |
| `with_sqlite_conn(action, f)` | `Result<R, E>` — no Default required | R is non-Default (e.g., custom struct)   |

For `MetadataStore::disabled()` path: `with_conn` and `with_conn_mut` return `Ok(T::default())` silently. `with_sqlite_conn` returns `Err(Corrupt("unavailable"))`. Discovery search must use `with_conn` so it returns an empty `Vec` when the store is disabled — never panic, never unwrap.

New public methods to add on `MetadataStore`:

```rust
pub fn search_trainer_sources(
    &self,
    query: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrainerSearchResult>, MetadataStoreError> {
    self.with_conn("search trainer sources for discovery", |conn| {
        discovery::search::search_trainer_sources(conn, query, limit, offset)
    })
}

pub fn count_trainer_sources(
    &self,
    query: &str,
) -> Result<i64, MetadataStoreError> {
    self.with_conn("count trainer sources for discovery", |conn| {
        discovery::search::count_trainer_sources(conn, query)
    })
}
```

### 5. IPC Command Pattern — Sync vs. Async Split

**Sync (fast SQLite query) — follow `community.rs` pattern:**

```rust
#[tauri::command]
pub fn discovery_search_trainers(
    query: TrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<TrainerSearchResponse, String> {
    let q = query.query.trim();
    if q.is_empty() {
        return Err("search query cannot be empty".to_string());
    }
    let limit = query.limit.unwrap_or(20) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let results = metadata_store
        .search_trainer_sources(q, limit, offset)
        .map_err(|e| e.to_string())?;
    let total_count = metadata_store
        .count_trainer_sources(q)
        .map_err(|e| e.to_string())?;

    Ok(TrainerSearchResponse { results, total_count })
}
```

**Async (Phase B HTTP fetch) — follow `protondb.rs` pattern:**

```rust
#[tauri::command]
pub async fn discovery_search_external(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ExternalDiscoveryResult, String> {
    let metadata_store = metadata_store.inner().clone();  // MANDATORY before .await
    Ok(fetch_external_trainers(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}
```

The `.inner().clone()` on `State<'_, MetadataStore>` before the `await` boundary is MANDATORY — `State<'_>` cannot cross `await` points without cloning the inner value (see `protondb.rs:55`).

### 6. IPC Contract Tests (MANDATORY)

Every `commands/*.rs` file ends with this compile-time type-assertion block. No exceptions.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = discovery_search_trainers
            as fn(TrainerSearchQuery, State<'_, MetadataStore>) -> Result<TrainerSearchResponse, String>;
    }
}
```

Template: `commands/community.rs:311–353`.

### 7. Phase A SQL Search (`discovery/search.rs`)

From `feature-spec.md:366–379` — queries `trainer_sources`, not `community_profiles`:

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

The `%` wildcards must be in the SQL string, not the Rust binding — rusqlite parameterized binds do not expand wildcards. Never format user input into SQL strings.

### 8. Cache-First HTTP Client (`protondb/client.rs`) — Phase B Reference

Phase B external source fetching clones this exact pattern:

```
normalize input
  → check fresh cache (expires_at > now)
  → live HTTP fetch (OnceLock<reqwest::Client>)
  → persist to external_cache_entries
  → on failure: load stale cache (allow_expired = true)
  → on both failures: return Unavailable state
```

Cache key namespace for discovery: `trainer_discovery:game:{app_id}`. Matches existing namespace convention (`protondb:{app_id}`). The `put_cache_entry` / `get_cache_entry` functions in `cache_store.rs` are the storage primitives — call them via `MetadataStore::with_sqlite_conn`.

HTTP client singleton pattern from `client.rs:175–190`:

```rust
static DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn discovery_http_client() -> Result<&'static reqwest::Client, DiscoveryError> {
    if let Some(client) = DISCOVERY_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(DiscoveryError::Network)?;
    let _ = DISCOVERY_HTTP_CLIENT.set(client);
    Ok(DISCOVERY_HTTP_CLIENT.get().expect("client initialized"))
}
```

### 9. Serde Conventions on IPC Boundary Types

These rules apply to ALL types crossing the IPC boundary (from `protondb/models.rs`):

- **IPC result structs**: `#[serde(rename_all = "camelCase")]` — frontend receives camelCase
- **State enums**: `#[serde(rename_all = "snake_case")]` with `#[default]` on the idle variant
- **Optional fields**: `#[serde(default, skip_serializing_if = "Option::is_none")]`
- **Empty collections**: `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
- **Empty strings**: `#[serde(default, skip_serializing_if = "String::is_empty")]`

The exact models to create (from `feature-spec.md:246–300`):

```rust
// crosshook-core/src/discovery/models.rs

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

### 10. Frontend Hook Pattern (`hooks/useProtonDbSuggestions.ts`)

The canonical hook shape for async IPC:

```typescript
const requestIdRef = useRef(0);

const fetchData = useCallback(
  async (forceRefresh = false) => {
    const id = ++requestIdRef.current;
    setLoading(true);
    setError(null);

    try {
      const result = await invoke<T>('command_name', { params });
      if (requestIdRef.current !== id) return; // stale request guard
      setData(result);
    } catch (err) {
      if (requestIdRef.current !== id) return;
      setError(err instanceof Error ? err.message : String(err));
      setData(null);
    } finally {
      if (requestIdRef.current === id) {
        setLoading(false);
      }
    }
  },
  [deps]
);
```

Return shape: `{ data, loading, error, refresh }`. All hooks return a named interface, not a tuple. Guard early on empty query strings (parallel to `!appId || !profileName` check in `useProtonDbSuggestions.ts:30–36`). Include a 300ms debounce before invoking the IPC command.

---

## Integration Points

### Files to CREATE (new)

| File                                       | Phase | Purpose                                                                                                                                    |
| ------------------------------------------ | ----- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `crosshook-core/src/discovery/mod.rs`      | A     | Module root; re-exports public API                                                                                                         |
| `crosshook-core/src/discovery/models.rs`   | A     | `TrainerSourcesManifest`, `TrainerSourceEntry`, `TrainerSearchQuery`, `TrainerSearchResult`, `TrainerSearchResponse`, `VersionMatchResult` |
| `crosshook-core/src/discovery/search.rs`   | A     | `search_trainer_sources()` and `count_trainer_sources()` — pure SQL functions, directly testable                                           |
| `src-tauri/src/commands/discovery.rs`      | A     | IPC command handlers + mandatory contract test block                                                                                       |
| `src/hooks/useTrainerDiscovery.ts`         | A     | Frontend hook wrapping `discovery_search_trainers` with requestIdRef race guard                                                            |
| `src/types/discovery.ts`                   | A     | TypeScript type mirrors for Rust structs                                                                                                   |
| `src/components/TrainerDiscoveryPanel.tsx` | A     | Search input, result cards, compatibility badges, source link CTAs                                                                         |
| `crosshook-core/src/discovery/client.rs`   | B     | HTTP client (clone of `protondb/client.rs`)                                                                                                |
| `crosshook-core/src/discovery/matching.rs` | B     | Token scoring from `install/discovery.rs`, advisory version comparison                                                                     |

### Files to MODIFY (existing)

| File                                             | Phase | Change                                                                             |
| ------------------------------------------------ | ----- | ---------------------------------------------------------------------------------- |
| `crosshook-core/src/lib.rs`                      | A     | Add `pub mod discovery;`                                                           |
| `crosshook-core/src/community/index.rs`          | A     | Walk tap directories for `trainer-sources.json` alongside `community-profile.json` |
| `crosshook-core/src/metadata/migrations.rs`      | A     | Add `migrate_17_to_18()` + guard block + test                                      |
| `crosshook-core/src/metadata/community_index.rs` | A     | Add `index_trainer_sources()` function with A6 bounds + HTTPS URL validation       |
| `crosshook-core/src/metadata/mod.rs`             | A     | Add `search_trainer_sources` and `count_trainer_sources` methods                   |
| `src-tauri/src/commands/mod.rs`                  | A     | Add `pub mod discovery;`                                                           |
| `src-tauri/src/lib.rs`                           | A     | Register discovery commands in `invoke_handler!`                                   |
| `src/hooks/useScrollEnhance.ts`                  | A     | Add `TrainerDiscoveryPanel` scroll container to `SCROLLABLE` constant              |
| `src/types/index.ts`                             | A     | Add `export * from './discovery'`                                                  |

**Note**: `community_profiles`, `profile/community_schema.rs`, and `metadata/models.rs` are **NOT modified**. The trainer-sources data is fully separate.

---

## Code Conventions

**Rust:**

- Error action strings use lowercase imperative: `"search trainer sources"`, `"run metadata migration 17 to 18"`
- `tracing::warn!` for non-fatal per-entry failures (A6 violations, invalid URLs)
- `tracing::info!` for successful operations with count summaries
- `db::new_id()` for new row primary keys (UUID generation, consistent across all stores)
- All SQL uses `params![]` binds — never string-format user input into SQL
- `nullable_text()` helper in `community_index.rs:330–337` — reuse for optional text fields in `index_trainer_sources`

**TypeScript:**

- Component names: `PascalCase` (`TrainerDiscoveryPanel`)
- Hook names: `useCamelCase` (`useTrainerDiscovery`)
- IPC call parameters: `camelCase` (Tauri serializes snake_case Rust args to camelCase)
- No `any` type; mirror Rust structs exactly with TypeScript interfaces
- CSS classes: `crosshook-*` BEM-like prefix

---

## Dependencies and Services

**Phase A — zero new dependencies.** Reuses:

- `rusqlite` (existing, `bundled` feature — FTS5 NOT available)
- `chrono` (timestamps)
- `serde` / `serde_json` (IPC boundary)
- `tracing` (logging)

**Phase B additions** (already in `Cargo.toml` via `protondb`):

- `reqwest` (HTTP client, already present)
- `tokio` (async runtime, already present)

**FTS5 constraint**: `rusqlite` is `features = ["bundled"]` only. LIKE-based search is the only valid Phase A approach. FTS5 (Phase C) requires a `bundled-full` feature flag change + separate issue.

---

## Gotchas

- **`trainer_sources` is a new table, `community_profiles` is unchanged.** The search SQL queries `trainer_sources JOIN community_taps`, not `community_profiles`. A `TrainerSearchResult` in the IPC response includes `source_url` and `source_name` natively from `trainer_sources` — no join to `community_profiles` needed for Phase A results.

- **`community/index.rs` walker must be extended, not just `metadata/community_index.rs`.** The tap directory walker that discovers `community-profile.json` files lives in `community/index.rs`. It must also discover `trainer-sources.json` files. This is a two-place change: (1) the walker that finds files, (2) the indexer that persists them.

- **`trainer-sources.json` is optional alongside `community-profile.json`.** A game directory may have a profile but no sources file, or a sources file but no profile. Both cases are valid. The walker must not fail when one is absent.

- **`index_trainer_sources()` should run inside the same transaction as `index_community_tap_result()`.** Both are keyed on `tap_id`. The watermark skip already gates both. Combining them in one `TransactionBehavior::Immediate` transaction prevents partial updates.

- **Watermark reset is NOT needed for v18 migration.** Unlike a column addition to `community_profiles`, the new `trainer_sources` table starts empty. Taps will populate it on the next sync without any watermark reset. Do not null `last_head_commit` in the v18 migration.

- **Async command requires `.inner().clone()` before `.await`**: `State<'_, MetadataStore>` is not `Send` across await points. The clone at `protondb.rs:55` is not optional. Phase B async discovery commands must do the same.

- **`useScrollEnhance` SCROLLABLE selector**: The `SCROLLABLE` constant at `useScrollEnhance.ts:8–9` is a CSS selector string. Any new `overflow-y: auto` container in `TrainerDiscoveryPanel` must be added here as a new comma-separated selector. Missing this causes dual-scroll jank.

- **Import CTA wires into existing `community_import_profile`**: The "Import Profile" button on a discovery result that has a community profile calls `community_import_profile` — the existing IPC command. Do not build a parallel import mechanism.

- **Trainer versions are not semver**: Display all version strings as-is. Do not attempt to parse with `semver` crate.

- **`discovery_enabled` opt-in guard**: The `discovery_search_trainers` command should check the settings flag. Return empty results (not an error) when disabled. The legal disclaimer is UI-layer only.

- **HTTPS-only URL validation at index time**: Source URLs that do not start with `https://` must be rejected at `index_trainer_sources()` time with `tracing::warn!` and skipped. Never store or display `http://` or `javascript:` URLs.

---

## Task-Specific Guidance

### Phase A task order (dependency-enforced)

1. **`TrainerSourcesManifest` + `TrainerSourceEntry` models** — defines the `trainer-sources.json` schema; `community/index.rs` walker depends on this
2. **v18 migration** — `trainer_sources` table; all storage depends on this
3. **`community/index.rs` walker extension** — discovers `trainer-sources.json` alongside profile manifests; `index_trainer_sources` depends on this
4. **`index_trainer_sources()` in `community_index.rs`** — persists entries with A6 bounds + URL validation
5. **`discovery/search.rs` + `MetadataStore` methods** — LIKE query functions; IPC command depends on this
6. **`commands/discovery.rs`** — thin IPC handler; registration depends on this
7. **`lib.rs` + `commands/mod.rs` + `invoke_handler!`** — wires everything together
8. **Frontend types + hook + panel** — can start in parallel with steps 5–7

### In-memory test pattern for store functions

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::db;
    use crate::metadata::migrations::run_migrations;

    #[test]
    fn search_returns_empty_for_no_sources() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let results = search_trainer_sources(&conn, "elden ring", 20, 0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_finds_inserted_source_by_game_name() {
        let conn = db::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        // insert a community_taps row first (FK constraint)
        // insert a trainer_sources row
        // assert search finds it
    }
}
```

Use `db::open_in_memory()` + `run_migrations()` — same pattern used in every `metadata/` test module. The `run_migrations` call will now run through v18, creating `trainer_sources` automatically.

### Module layout for `discovery/`

Mirror `protondb/` directory structure:

```
crosshook-core/src/discovery/
├── mod.rs          — pub use models::*, pub fn search/count via MetadataStore
├── models.rs       — TrainerSourcesManifest, TrainerSearchResult, TrainerSearchResponse, etc.
└── search.rs       — search_trainer_sources(conn, query, limit, offset) — pure SQL, testable
```

Phase B adds `client.rs` (HTTP) and `matching.rs` (token scoring, version advisory).
