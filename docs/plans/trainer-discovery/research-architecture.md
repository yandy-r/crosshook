# Architecture Research: Trainer Discovery Phase B

## System Overview

Phase A of trainer discovery is fully implemented: the `discovery/` module with LIKE-based search over `trainer_sources` (migration v17→v18), a sync `discovery_search_trainers` IPC command, and a React `TrainerDiscoveryPanel`. Phase B extends this with an async HTTP client (`discovery/client.rs`) following the `protondb/client.rs` OnceLock pattern, token-based scoring from `install/discovery.rs`, external result caching via the existing `external_cache_entries` table, and a version compatibility check powered by `metadata/version_store.rs`. The DB schema stays at v18 for Phase B — no new migration is needed unless a new cache namespace convention requires a table change.

---

## Relevant Components

### Rust — crosshook-core

- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: **Primary Phase B reference** — the complete OnceLock HTTP client pattern, cache-first lookup, stale fallback, TTL calculation, and `external_cache_entries` persistence. All four patterns Phase B replicates are here.
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry()`, `put_cache_entry()`, `evict_expired_cache_entries()` — the three public functions Phase B will call. Already used by ProtonDB.
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `compute_correlation_status()` (pure fn, no I/O) and `lookup_latest_version_snapshot()` — the two functions Phase B version check will use.
- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: `tokenize()` and `token_hits()` — the scoring primitives Phase B `discovery/matching.rs` will re-use or copy.
- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs`: Phase A module root (`mod models; mod search; pub use models::*; pub use search::search_trainer_sources`). Phase B adds `pub mod client; pub mod matching;`.
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs`: Defines `TrainerSearchQuery`, `TrainerSearchResult`, `TrainerSearchResponse`, `TrainerSourceRow`, `TrainerSourcesManifest`, `TrainerSourceEntry`, `VersionMatchStatus`, `VersionMatchResult`. Phase B types (`ExternalLookupResult`, trust-badge enum) go in this file.
- `src/crosshook-native/crates/crosshook-core/src/discovery/search.rs`: Phase A LIKE search; returns `relevance_score: 0.0` as placeholder column. Phase B scoring upgrades this field via `discovery/matching.rs`.
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` — Arc<Mutex<Connection>> facade. Phase B uses `metadata_store.get_cache_entry()` (line 523), `metadata_store.put_cache_entry()` (line 529), and `metadata_store.with_sqlite_conn()` (line 138) indirectly via `version_store`.
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: `migrate_17_to_18` (line 782) — created `trainer_sources` and, in the same migration, `external_cache_entries` (line 264). No new migration needed for Phase B.

### Tauri IPC Layer

- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: **Reference for async IPC pattern.** `protondb_lookup` (line 50) shows: `pub async fn`, `metadata_store: State<'_, MetadataStore>`, and `let metadata_store = metadata_store.inner().clone();` before `.await`. Phase B's `discovery_search_external` and `discovery_check_version_compatibility` must use this exact pattern.
- `src/crosshook-native/src-tauri/src/commands/discovery.rs`: Phase A sync command. Phase B adds async commands alongside `discovery_search_trainers` in this same file.
- `src/crosshook-native/src-tauri/src/commands/mod.rs`: Command module registry — no changes needed if Phase B commands are added to the existing `discovery.rs`.

### Frontend — React/TypeScript

- `src/crosshook-native/src/hooks/useTrainerDiscovery.ts`: Phase A hook — debounced `invoke('discovery_search_trainers')`, request-ID race guard, loading/error state. Phase B extends this or adds a sibling hook for external results.
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`: **Reference for progressive async loading** — `requestIdRef` race guard (line 37), `forceRefresh` parameter, optimistic dismiss (line 92). Phase B's external hook mirrors this pattern.
- `src/crosshook-native/src/types/discovery.ts`: `TrainerSearchResult`, `TrainerSearchResponse`, `VersionMatchStatus`, `VersionMatchResult` already defined. Phase B adds `ExternalTrainerResult` and trust-badge types here.
- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`: Phase A UI component with consent gate. Phase B adds external results section, trust badges, offline banner, and progressive loading state.

---

## Async HTTP Client Architecture

**Reference file**: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`

The entire pattern Phase B must replicate:

```
static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();  // line 26
```

Initialization (line 175–190):

```rust
fn protondb_http_client() -> Result<&'static reqwest::Client, ProtonDbError> {
    if let Some(client) = PROTONDB_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))  // 6 seconds
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(ProtonDbError::Network)?;
    let _ = PROTONDB_HTTP_CLIENT.set(client);  // discard Err: concurrent threads may race; both built equivalent clients
    Ok(PROTONDB_HTTP_CLIENT.get().expect("..."))
}
```

The `let _ = ...set(client)` discard is intentional: `OnceLock::set` returns `Err` when another thread raced and won, but both threads built equivalent clients, so discarding the losing client is safe. The subsequent `.get().expect()` always succeeds after either thread wins.

For `discovery/client.rs`, Phase B creates `static FLING_HTTP_CLIENT: OnceLock<reqwest::Client>` with the same builder. Key constants from ProtonDB to mirror:

- `CACHE_TTL_HOURS: i64 = 6` — Phase B should use a **longer** TTL (e.g. 24h for RSS, since FLiNG updates are infrequent)
- `REQUEST_TIMEOUT_SECS: u64 = 6` — same or slightly longer for RSS parsing

Cache-first flow in `lookup_protondb()` (lines 85–130):

1. Normalize input (clean `app_id`)
2. Check `external_cache_entries` for valid (non-expired) row → return if hit
3. Call `fetch_live_lookup()` → async HTTP
4. On success: `attach_cache_state()` + `persist_lookup_result()` → cache write
5. On failure: load stale row (allow_expired=true) → return stale result with `is_stale: true`
6. On both miss: return `Unavailable` state

Phase B must replicate all 5 steps. The stale fallback (step 5) is critical for offline behavior.

---

## Cache Infrastructure

**Reference file**: `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`

### `external_cache_entries` Table Schema (migration v4, line 264 in migrations.rs)

```sql
CREATE TABLE IF NOT EXISTS external_cache_entries (
    cache_id        TEXT PRIMARY KEY,    -- uuid v4 from db::new_id()
    source_url      TEXT NOT NULL,       -- source URL for the data (informational)
    cache_key       TEXT NOT NULL UNIQUE,-- lookup key, UNIQUE constraint for upsert
    payload_json    TEXT,                -- NULL if payload > MAX_CACHE_PAYLOAD_BYTES (512 KiB)
    payload_size    INTEGER NOT NULL DEFAULT 0,
    fetched_at      TEXT NOT NULL,       -- RFC3339 timestamp
    expires_at      TEXT,                -- RFC3339 or NULL for no expiry
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
```

### Cache Key Format for Phase B

ProtonDB uses `cache_key_for_app_id()` producing e.g. `"protondb:summary:v1:1234567"`. Phase B must use a **distinct namespace** to avoid collision: `"trainer:source:v1:{normalized_key}"` where `normalized_key` is the lowercased, whitespace-collapsed game name or RSS URL hash.

### Three Cache Operations (cache_store.rs)

**`get_cache_entry(conn, cache_key)`** (line 6): returns `Option<String>` — None if row missing or `expires_at` has passed. Uses `expires_at IS NULL OR expires_at > ?2` guard.

**`put_cache_entry(conn, source_url, cache_key, payload, expires_at)`** (line 29): upserts via `ON CONFLICT(cache_key) DO UPDATE SET`. Silently stores `NULL` for `payload_json` if payload exceeds `MAX_CACHE_PAYLOAD_BYTES` (512 KiB) — Phase B must check for null payload on read-back. Takes `expires_at: Option<&str>` as RFC3339 string.

**`evict_expired_cache_entries(conn)`** (line 91): deletes rows where `expires_at < now`. Called during background maintenance. Phase B does not need to call this directly.

### How ProtonDB Accesses the Cache (load_cached_lookup_row, lines 346–393)

ProtonDB bypasses the `cache_store` public API and queries `external_cache_entries` directly in `client.rs` via `with_sqlite_conn()` (line 362) — it needs `fetched_at` and `expires_at` in addition to `payload_json` for stale-fallback state computation. Phase B cache helpers must do the same.

**`with_conn` vs `with_sqlite_conn` — choose the right accessor:**

- **`with_conn`** (private, `mod.rs` line 98): requires `T: Default`. When `MetadataStore` is unavailable, silently returns `T::default()` instead of an error. Use for operations where a silent empty result is acceptable (e.g. returning empty `TrainerSearchResponse` or `None` from cache).
- **`with_sqlite_conn`** (public, `mod.rs` line 138): no `Default` bound. Returns `Err` when the store is unavailable. Use when the caller must distinguish unavailable-store from empty-result — e.g. in `load_cached_lookup_row` where that distinction drives stale-fallback logic.

Phase B `discovery/client.rs` queries `external_cache_entries` directly via `with_sqlite_conn()` (same as ProtonDB) to get the full row for stale-fallback state.

---

## Async IPC Pattern

**Reference file**: `src/crosshook-native/src-tauri/src/commands/protondb.rs`

The critical pattern (line 50–57):

```rust
#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();   // MUST clone before await
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}
```

**Why `inner().clone()` before `await`**: `State<'_>` holds a reference with a lifetime tied to the Tauri app handle. Futures must be `'static` (no borrowed lifetimes). Cloning the inner `MetadataStore` (which is `Arc<Mutex<Connection>>` under the hood, making clone cheap) produces an owned value that satisfies `'static`. **Never pass `&MetadataStore` across an await boundary.**

Phase B commands to add in `commands/discovery.rs`:

```rust
#[tauri::command]
pub async fn discovery_search_external(
    game_name: String,
    steam_app_id: Option<u32>,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ExternalLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    crosshook_core::discovery::search_external(
        &metadata_store, &game_name, steam_app_id, force_refresh.unwrap_or(false)
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn discovery_check_version_compatibility(
    profile_name: String,
    trainer_game_version: Option<String>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<VersionMatchResult, String> {
    let metadata_store = metadata_store.inner().clone();
    // ...
}
```

---

## Version Store Integration

**Reference file**: `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`

### `compute_correlation_status()` (line 185–211) — Pure Function

```rust
pub fn compute_correlation_status(
    current_build_id: &str,
    snapshot_build_id: Option<&str>,
    current_trainer_hash: Option<&str>,
    snapshot_trainer_hash: Option<&str>,
    state_flags: Option<u32>,
) -> VersionCorrelationStatus
```

Returns: `UpdateInProgress` | `Untracked` | `BothChanged` | `GameUpdated` | `TrainerChanged` | `Matched`

For Phase B version check, this function answers "does the trainer's `game_version` string match the installed Steam build?". However, it compares build IDs and file hashes, not human-readable version strings. The `discovery_check_version_compatibility` IPC command will need to do a **string comparison** between `TrainerSourceEntry::game_version` and the `human_game_ver` from `VersionSnapshotRow`, returning a `VersionMatchResult` with `VersionMatchStatus` (defined in `discovery/models.rs`).

### `lookup_latest_version_snapshot()` (line 75–111)

```rust
pub fn lookup_latest_version_snapshot(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<VersionSnapshotRow>, MetadataStoreError>
```

Returns the most recent `version_snapshots` row for a profile. `VersionSnapshotRow` contains: `steam_build_id`, `trainer_version`, `trainer_file_hash`, `human_game_ver`, `status`, `checked_at`.

Phase B version check flow:

1. Resolve `profile_id` from `profile_name` via `MetadataStore::lookup_profile_id()`
2. Load snapshot via `lookup_latest_version_snapshot(conn, &profile_id)`
3. Compare snapshot `human_game_ver` against trainer's `game_version` string
4. Return `VersionMatchResult` — **advisory only, no blocking behavior**

---

## Token Scoring

**Reference file**: `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`

Two pure functions Phase B `discovery/matching.rs` will adapt:

### `tokenize(value: &str) -> Vec<String>` (line 292–304)

Splits on non-alphanumeric characters, lowercases, filters tokens shorter than 2 chars. Example: `"Elden Ring"` → `["elden", "ring"]`.

```rust
fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.len() >= 2 { Some(token) } else { None }
        })
        .collect()
}
```

### `token_hits(value: &str, target_tokens: &[String]) -> usize` (line 272–277)

Counts how many target tokens appear as substrings in `value`:

```rust
fn token_hits(value: &str, target_tokens: &[String]) -> usize {
    target_tokens.iter().filter(|token| value.contains(token.as_str())).count()
}
```

### Scoring Application in `score_candidate()` (line 214–270)

The full scoring function in `install/discovery.rs` shows how `token_hits` produces a score:

- Stem token hits: `+40 + (hits * 12)` (high weight — game name match in executable stem)
- Path segment token hits: `+hits * 4` (lower weight — directory name matches)
- Penalty for suspicious terms: `-120` (file), `-90` (path)

For Phase B `discovery/matching.rs`, the token scoring of RSS results against the user query should use a simplified version: tokenize the RSS `game_name`, count token hits against the search query tokens, normalize to `relevance_score: f64` in `[0.0, 1.0]`.

---

## Data Flow

### Phase B External Lookup Flow

```
User searches → discovery_search_trainers (Phase A, sync) → local results shown immediately
                                                              ↓
              → discovery_search_external (Phase B, async) → external results shown progressively

discovery_search_external:
    ↓  MetadataStore.inner().clone()  [before first await]
    ↓  check external_cache_entries for "trainer:source:v1:{key}"
         if hit (not expired) → return cached payload with is_stale=false
    ↓  if miss → fling_http_client() → GET FLiNG RSS URL
    ↓  parse RSS XML → Vec<ExternalTrainerEntry>
    ↓  score against query tokens (matching.rs)
    ↓  put_cache_entry(source_url, cache_key, payload_json, expires_at)
    ↓  return ExternalLookupResult { entries, is_stale: false }
         if HTTP error → load stale cache row (allow_expired=true)
         if stale exists → return with is_stale=true
         if no stale → return ExternalLookupResult { entries: [], is_stale: false, offline: true }
```

### Phase B Version Check Flow

```
User clicks version check on a trainer result:
    ↓  discovery_check_version_compatibility IPC (async)
    ↓  MetadataStore.inner().clone()
    ↓  lookup_profile_id(profile_name)
    ↓  with_sqlite_conn → lookup_latest_version_snapshot(profile_id)
    ↓  compare snapshot.human_game_ver vs trainer's game_version string
    ↓  return VersionMatchResult { status, trainer_game_version, installed_game_version, detail }
```

---

## Integration Points

Phase B connects to existing Phase A infrastructure at four seams:

1. **`discovery/mod.rs`** — add `pub mod client; pub mod matching;` alongside existing `pub mod models; pub mod search;`

2. **`commands/discovery.rs`** — add two async commands (`discovery_search_external`, `discovery_check_version_compatibility`) alongside existing sync `discovery_search_trainers`. IPC contract test block must include casts for all three commands.
   - `discovery_search_external` injects: `metadata_store: State<MetadataStore>` only
   - `discovery_check_version_compatibility` injects: `metadata_store: State<MetadataStore>` + `profile_store: State<ProfileStore>` (needed to resolve `profile_id` for version snapshot lookup)

3. **`src-tauri/src/lib.rs` line 318** — register new commands in the existing `// Trainer discovery` block inside `tauri::generate_handler![]`. Current entry is `commands::discovery::discovery_search_trainers`; Phase B appends `commands::discovery::discovery_search_external` and `commands::discovery::discovery_check_version_compatibility` to that block.

4. **Frontend `TrainerDiscoveryPanel.tsx`** — add a second data source (external results) alongside the existing Phase A local results. The `useTrainerDiscovery` hook (Phase A) and the new `useExternalTrainerSearch` hook (Phase B) run in parallel; UI renders trust badges (Community vs External) to distinguish result provenance. External results are **not** written to `trainer_sources` — they are ephemeral, sourced from the `external_cache_entries` blob.

---

## Key Dependencies

All already present in `Cargo.toml` — no new crate additions for Phase B:

| Crate                  | Role in Phase B                   | Location in Cargo.toml                                                                              |
| ---------------------- | --------------------------------- | --------------------------------------------------------------------------------------------------- |
| `reqwest 0.12`         | HTTP client for FLiNG RSS         | line 22, `features = ["json", "rustls-tls"]` — add `"text"` feature if needed for RSS text response |
| `serde` / `serde_json` | IPC serialization, cache payload  | lines 10–11                                                                                         |
| `tokio`                | Async runtime                     | line 14, `features = ["fs", "process", "rt", "sync", "time"]`                                       |
| `chrono`               | TTL timestamps (RFC3339)          | line 7                                                                                              |
| `rusqlite 0.39`        | `external_cache_entries` queries  | line 21, `features = ["bundled"]`                                                                   |
| `sha2`                 | Trainer file hash (version store) | line 23                                                                                             |

**Note on `reqwest` features**: `"json"` enables `.json::<T>()`. FLiNG RSS is XML. Phase B will need to either add a RSS/XML parsing crate (`roxmltree`, `quick-xml`) or parse the RSS as text and use a minimal extractor. This is the **only potential new dependency** for Phase B.

---

## Edge Cases

- **`payload_json` is NULL after oversized payload**: `put_cache_entry` stores `NULL` for payloads > 512 KiB (line 37–47 in cache_store.rs). Phase B must handle `get_cache_entry()` returning `Some("")` or `None` for oversized cached entries and treat them as cache misses.
- **MetadataStore disabled path**: `MetadataStore::disabled()` returns `available: false`. `get_cache_entry` returns `Ok(None)` for disabled stores (tested at line 2499 in mod.rs tests). Phase B must handle this gracefully — skip cache, attempt live fetch.
- **`inner().clone()` is mandatory before `await`**: Forgetting this will cause a compile error about `State<'_>` not being `'static`. Do not attempt to work around this with `Arc::clone` on internal fields.
- **`external_cache_entries` is shared**: ProtonDB and Phase B both write to this table with distinct key namespaces. Cache eviction via `evict_expired_cache_entries()` is namespace-agnostic — it deletes all expired rows. Phase B TTL should be long enough (24h+) to survive routine maintenance cycles.
- **Version snapshot may not exist**: `lookup_latest_version_snapshot()` returns `Ok(None)` for profiles that have never been launched. The version check IPC must return `VersionMatchStatus::Unknown` in this case, not an error.
- **`useScrollEnhance.ts` constraint**: Any new scrollable container in the Phase B UI section of `TrainerDiscoveryPanel` must be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts` to avoid dual-scroll jank under WebKitGTK.
- **RSS XML vs JSON**: `reqwest` with `features = ["json"]` provides `.json::<T>()`. FLiNG RSS returns XML — this requires a separate XML parser. The feature flag choice affects whether a new crate is needed.

---

## Other Docs

- `docs/plans/trainer-discovery/feature-spec.md` — business rules, FLiNG RSS URL format, trust badge semantics
- `docs/plans/trainer-discovery/research-integration.md` — external API details (FLiNG RSS structure)
- `docs/plans/trainer-discovery/research-patterns.md` — IPC contract test patterns, error handling conventions
- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — the authoritative reference implementation for Phase B's HTTP client, cache-first flow, and stale fallback
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — cache store API
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` — version snapshot API
- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs` — token scoring source functions
