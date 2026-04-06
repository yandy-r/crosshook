# Trainer Discovery Phase B — Code Pattern Analysis

## Executive Summary

Phase A (tap-local discovery) is fully implemented and working. Phase B adds an FLiNG RSS HTTP client, token-based relevance scoring, advisory version matching, and progressive frontend loading. Every pattern needed for Phase B already exists in the codebase — the implementation is primarily an application of `protondb/client.rs` patterns to a new domain. The only new crate dependency is `quick-xml`. Three new Rust files are required (`discovery/client.rs`, `discovery/matching.rs`); three existing files need augmentation (`discovery/mod.rs`, `discovery/models.rs`, `metadata/mod.rs`); two IPC commands are added to `commands/discovery.rs`; two TypeScript additions complete the frontend.

---

## Existing Code Structure

### Rust — crosshook-core (Phase A in-tree layout)

```
src/crosshook-native/crates/crosshook-core/src/
├── discovery/
│   ├── mod.rs          — pub mod models; pub mod search; pub use re-exports
│   ├── models.rs       — TrainerSearchQuery, TrainerSearchResult, TrainerSearchResponse,
│   │                     VersionMatchStatus, VersionMatchResult (Phase B stubs already present)
│   └── search.rs       — search_trainer_sources() LIKE query + pagination + test suite
├── protondb/
│   └── client.rs       — PRIMARY PATTERN for Phase B HTTP client (OnceLock singleton, 3-stage cache)
├── metadata/
│   ├── mod.rs          — MetadataStore facade; search_trainer_sources(), get_cache_entry(), put_cache_entry()
│   ├── cache_store.rs  — get_cache_entry(), put_cache_entry(), evict_expired_cache_entries()
│   └── version_store.rs — lookup_latest_version_snapshot(), compute_correlation_status()
└── install/
    └── discovery.rs    — tokenize(), token_hits() — Phase B scoring primitives
```

### Tauri IPC Layer

```
src/crosshook-native/src-tauri/src/commands/
├── discovery.rs    — Phase A discovery_search_trainers; Phase B adds discovery_search_external,
│                     discovery_check_version_compatibility
├── protondb.rs     — ASYNC IPC REFERENCE (protondb_lookup pattern at line 49–57)
└── mod.rs          — pub mod discovery; already declared
```

### Frontend

```
src/crosshook-native/src/
├── types/discovery.ts               — Phase A types; Phase B extends with ExternalTrainerResult etc.
├── hooks/
│   ├── useTrainerDiscovery.ts       — Phase A debounced hook (race guard pattern)
│   └── useProtonDbSuggestions.ts    — REFERENCE for async hook + requestIdRef race guard
└── components/
    └── TrainerDiscoveryPanel.tsx    — Phase A panel; Phase B adds "Search Online" button + external section
```

---

## Implementation Patterns

### Pattern 1: OnceLock HTTP Client Singleton

**Location**: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:26,175–190`

The established pattern for HTTP clients in this codebase:

```rust
// Static OnceLock declaration — one per HTTP client domain
static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

// Lazy init function — called at first use, race-safe via OnceLock::set
fn protondb_http_client() -> Result<&'static reqwest::Client, ProtonDbError> {
    if let Some(client) = PROTONDB_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(ProtonDbError::Network)?;
    let _ = PROTONDB_HTTP_CLIENT.set(client);
    Ok(PROTONDB_HTTP_CLIENT.get().expect("..."))
}
```

**For Phase B**: Create `static FLING_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();` in `discovery/client.rs` and a matching `fling_http_client() -> Result<&'static reqwest::Client, DiscoveryError>`.

### Pattern 2: Three-Stage Cache-First Fetch

**Location**: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:85–130`

```rust
pub async fn lookup_protondb(metadata_store: &MetadataStore, ...) -> ProtonDbLookupResult {
    // Stage 1: Valid cache hit → return immediately
    if !force_refresh {
        if let Some(valid_cache) = load_cached_lookup_row(metadata_store, &cache_key, false) {
            if let Some(result) = cached_result_from_row(..., false) {
                return result;
            }
        }
    }
    // Stage 2: Live fetch → parse → persist → return
    match fetch_live_lookup(&app_id).await {
        Ok(mut result) => {
            attach_cache_state(&mut result, &cache_key, false, false);
            persist_lookup_result(metadata_store, &cache_key, &result);
            result
        }
        // Stage 3: Stale fallback on error → return with is_stale=true
        Err(error) => {
            tracing::warn!(app_id, %error, "ProtonDB live lookup failed");
            if let Some(stale_cache) = load_cached_lookup_row(metadata_store, &cache_key, true) {
                if let Some(result) = cached_result_from_row(..., true) {
                    return result;
                }
            }
            // Stage 4: Total failure → Unavailable
            ProtonDbLookupResult { state: ProtonDbLookupState::Unavailable, ... }
        }
    }
}
```

**For Phase B**: The `search_external_trainers()` function in `discovery/client.rs` follows this exact flow with `DiscoveryCacheState` instead of `ProtonDbCacheState`.

### Pattern 3: Domain Error Types

**Location**: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:28–53`

Private enums with manual `fmt::Display` — never exposed at the IPC boundary:

```rust
#[derive(Debug)]
enum ProtonDbError {
    NotFound,
    Network(reqwest::Error),
    InvalidAppId(String),
    ...
}

impl fmt::Display for ProtonDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "ProtonDB summary not found for this Steam App ID"),
            Self::Network(error) => write!(f, "network error: {error}"),
            ...
        }
    }
}
```

**For Phase B**: Declare `enum DiscoveryError { Network(reqwest::Error), ParseError(String), CacheKeyEmpty, }` in `discovery/client.rs`. Never add `pub` — it stays private to the client module.

### Pattern 4: Cache Read/Write via MetadataStore

**Location**: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:523–537`
**Implementation**: `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`

The MetadataStore exposes two facade methods:

```rust
// Valid-only read (expired rows ignored)
pub fn get_cache_entry(&self, cache_key: &str) -> Result<Option<String>, MetadataStoreError>

// Upsert — ON CONFLICT(cache_key) DO UPDATE SET
pub fn put_cache_entry(
    &self,
    source_url: &str,
    cache_key: &str,
    payload: &str,
    expires_at: Option<&str>,
) -> Result<(), MetadataStoreError>
```

`protondb/client.rs` calls these at lines 332–343 and 346–394. The client bypasses the facade for stale reads (directly calling `with_sqlite_conn`) to get expired rows — Phase B should use the same approach.

**Cache key namespace for Phase B**: `trainer:source:v1:{normalized_game_name}` — avoids collisions with ProtonDB's `protondb:app:v1:{app_id}` namespace.

**Payload cap**: `MAX_CACHE_PAYLOAD_BYTES` = 512 KiB (enforced in `put_cache_entry`). RSS responses are well under this. The function stores NULL payload and logs a warning if exceeded — this is handled automatically, no special casing needed.

### Pattern 5: Async IPC Command

**Location**: `src/crosshook-native/src-tauri/src/commands/protondb.rs:49–57`

```rust
#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    // CRITICAL: Clone before first `.await` — State<> is not Send
    let metadata_store = metadata_store.inner().clone();
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}
```

The `.inner().clone()` before `await` is mandatory — `State<'_, T>` is not `Send`. `map_err(|e| e.to_string())` at the IPC boundary is the only error serialization needed.

**For Phase B**: `discovery_search_external` follows this exact shape. `discovery_check_version_compatibility` can be sync (no HTTP call — it's a pure lookup + computation).

### Pattern 6: IPC Contract Test

**Location**: `src/crosshook-native/src-tauri/src/commands/discovery.rs:19–31`

Every commands file has a compile-time signature verification block:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = discovery_search_trainers
            as fn(
                TrainerSearchQuery,
                State<'_, MetadataStore>,
            ) -> Result<TrainerSearchResponse, String>;
    }
}
```

**For Phase B**: Add a matching cast for `discovery_search_external` and `discovery_check_version_compatibility` in the same `tests` block.

### Pattern 7: Token Scoring Primitives

**Location**: `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs:272–304`

```rust
// Splits on non-alphanumeric, lowercases, filters tokens < 2 chars
fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.len() >= 2 { Some(token) } else { None }
        })
        .collect()
}

// Counts substring matches of tokens in a value
fn token_hits(value: &str, target_tokens: &[String]) -> usize {
    target_tokens
        .iter()
        .filter(|token| value.contains(token.as_str()))
        .count()
}
```

These are private functions in `install/discovery.rs`. Phase B's `discovery/matching.rs` must duplicate or re-lift them — they cannot be imported directly. The spec calls for lifting them into `discovery/matching.rs` as `pub(crate)` so they can be tested independently.

**Scoring formula reference** from `install/discovery.rs:238–249`:

- `stem_token_hits > 0` → `score += 40 + (hits * 12)`
- `path_token_hits > 0` → `score += hits * 4`

Phase B adapts this for RSS title matching against the user's search query.

### Pattern 8: Version Store Advisory Check

**Location**: `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs:185–211`

`compute_correlation_status` is a **pure function** — no I/O, no MetadataStore dependency:

```rust
pub fn compute_correlation_status(
    current_build_id: &str,
    snapshot_build_id: Option<&str>,
    current_trainer_hash: Option<&str>,
    snapshot_trainer_hash: Option<&str>,
    state_flags: Option<u32>,
) -> VersionCorrelationStatus
```

Phase B's advisory version check (`discovery/matching.rs`) uses a simpler comparison: it compares `trainer_game_version` from the external result against `human_game_ver` from the version snapshot, returning `VersionMatchStatus` (already defined in `discovery/models.rs`). The lookup itself uses:

```rust
pub fn lookup_latest_version_snapshot(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<VersionSnapshotRow>, MetadataStoreError>
```

### Pattern 9: MetadataStore Facade — with_conn vs with_sqlite_conn

**Location**: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:98–160`

Two access patterns:

- `with_conn(action, f)` — returns `T::default()` when store unavailable; for optional reads where degraded response is acceptable
- `with_sqlite_conn(action, f)` — returns `Err(MetadataStoreError::Corrupt)` when unavailable; use when unavailability must be surfaced (e.g., stale cache reads that are load-bearing)

Phase B cache methods added to the facade:

```rust
// For Phase B — add to MetadataStore in metadata/mod.rs:
pub fn get_external_trainer_cache(&self, cache_key: &str) -> Result<Option<String>, MetadataStoreError> {
    self.with_conn("get external trainer cache entry", |conn| {
        cache_store::get_cache_entry(conn, cache_key)
    })
}

pub fn put_external_trainer_cache(&self, source_url: &str, cache_key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError> {
    self.with_conn("put external trainer cache entry", |conn| {
        cache_store::put_cache_entry(conn, source_url, cache_key, payload, expires_at)
    })
}

pub fn lookup_latest_version_snapshot_for_profile(&self, profile_id: &str) -> Result<Option<VersionSnapshotRow>, MetadataStoreError> {
    self.with_conn("lookup latest version snapshot", |conn| {
        version_store::lookup_latest_version_snapshot(conn, profile_id)
    })
}
```

Note: `protondb/client.rs` uses `with_sqlite_conn` directly for stale reads (bypasses the facade to access expired rows). Phase B's `discovery/client.rs` should do the same for the stale fallback path — see `load_cached_lookup_row()` pattern at `protondb/client.rs:346–394`.

### Pattern 10: Frontend Hook Race Guard

**Location**: `src/crosshook-native/src/hooks/useTrainerDiscovery.ts:38–51` and `useProtonDbSuggestions.ts:37`

```typescript
const requestIdRef = useRef(0);

const fetchResults = useCallback(async (searchQuery: string) => {
    const id = ++requestIdRef.current;  // Increment before await
    setLoading(true);

    try {
        const response = await invoke<T>('command_name', { ... });
        if (requestIdRef.current !== id) return;  // Discard stale responses
        setData(response);
    } catch (err) {
        if (requestIdRef.current !== id) return;
        setError(...);
    } finally {
        if (requestIdRef.current === id) setLoading(false);
    }
}, [deps]);
```

**For Phase B**: The external search hook (`useExternalTrainerSearch` or inline in the panel) must use the same `requestIdRef` pattern. The local and external searches use separate `requestIdRef` instances so a new local search doesn't cancel an in-flight external search (different lifecycles).

### Pattern 11: Serde IPC Boundary Conventions

**Location**: `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs:1–122`

```rust
// Result structs — camelCase for TypeScript consumers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSearchResult { ... }

// State enums — snake_case matching TypeScript string literals
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VersionMatchStatus {
    Exact, Compatible, NewerAvailable, Outdated,
    #[default] Unknown,
}

// Optional fields — never serialize nulls
#[serde(default, skip_serializing_if = "Option::is_none")]
pub trainer_version: Option<String>,
```

Phase B adds `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery`, and `DiscoveryCacheState` to `discovery/models.rs` following these same conventions.

---

## Integration Points

### Files to Create

| File                                       | Purpose                                    | Based On                                   |
| ------------------------------------------ | ------------------------------------------ | ------------------------------------------ |
| `crosshook-core/src/discovery/client.rs`   | FLiNG RSS HTTP client, 3-stage cache       | `protondb/client.rs` verbatim pattern      |
| `crosshook-core/src/discovery/matching.rs` | Token scoring, advisory version comparison | `install/discovery.rs:tokenize/token_hits` |

### Files to Modify

| File                                       | Change                                                                                                                       |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| `crosshook-core/src/discovery/mod.rs`      | Add `pub mod client; pub mod matching;` + re-exports                                                                         |
| `crosshook-core/src/discovery/models.rs`   | Add `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery`, `DiscoveryCacheState`            |
| `crosshook-core/src/metadata/mod.rs`       | Add facade methods: `get_external_trainer_cache`, `put_external_trainer_cache`, `lookup_latest_version_snapshot_for_profile` |
| `crosshook-core/Cargo.toml`                | Add `quick-xml = "0.37"` (or current version) under `[dependencies]`                                                         |
| `src-tauri/src/commands/discovery.rs`      | Add async `discovery_search_external`, sync `discovery_check_version_compatibility`, extend contract test                    |
| `src-tauri/src/lib.rs`                     | Register new commands in the `// Trainer discovery` block (~line 318)                                                        |
| `src/types/discovery.ts`                   | Add `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `DiscoveryCacheState` interfaces                               |
| `src/components/TrainerDiscoveryPanel.tsx` | Add "Search Online" button, external results section, trust badges, offline banner                                           |
| `src/hooks/useScrollEnhance.ts`            | Register external results scroll container in `SCROLLABLE` selector if a new overflow container is added                     |

### New IPC Commands

```rust
// Async — follows protondb_lookup pattern exactly
#[tauri::command]
pub async fn discovery_search_external(
    query: ExternalTrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ExternalTrainerSearchResponse, String> {
    let metadata_store = metadata_store.inner().clone();
    client::search_external_trainers(&metadata_store, &query).await.map_err(|e| e.to_string())
}

// Sync — no HTTP, pure DB lookup + computation
#[tauri::command]
pub fn discovery_check_version_compatibility(
    community_profile_id: i64,
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<VersionMatchResult, String> {
    matching::check_version_compatibility(&metadata_store, community_profile_id, &profile_name)
        .map_err(|e| e.to_string())
}
```

---

## Code Conventions

### Rust Module Visibility

- `pub(crate)` for internal types; `pub` only for IPC boundary types (those used in Tauri commands)
- Error enums in client files are always private (`enum DiscoveryError`, not `pub enum`)
- Cache key construction functions follow `protondb/models.rs:cache_key_for_app_id` pattern — keep as `pub(crate)` in `discovery/client.rs`

### Error Propagation

- Inside `crosshook-core`: use `MetadataStoreError` for DB errors; use private error types for HTTP errors
- At IPC boundary in `commands/*.rs`: always `.map_err(|e| e.to_string())`
- Never return internal error types through `#[tauri::command]` — always `Result<T, String>`

### Logging

- Use `tracing::warn!()` for degraded paths (stale fallback, cache miss, HTTP errors) — matches `protondb/client.rs:110`
- Use structured fields: `tracing::warn!(cache_key, %error, "message")`

### Testing Pattern

- Unit tests in `#[cfg(test)] mod tests {}` within the same file
- DB tests use `MetadataStore::open_in_memory().unwrap()` + `with_sqlite_conn("test ...", |conn| { ... })`
- See `discovery/search.rs:105–322` for the complete test structure to replicate

---

## Dependencies and Services

### Existing (no change needed)

- `reqwest` — already in Cargo.toml with `json` + `rustls-tls` features
- `serde_json` — serialization/deserialization for cache payloads
- `chrono` — TTL calculations (`Utc::now().to_rfc3339()`, `ChronoDuration::hours(TTL)`)
- `rusqlite` — direct SQL access via `with_sqlite_conn`
- `tracing` — structured logging

### New Dependency

- `quick-xml` — RSS parsing for FLiNG feed. Add to `crosshook-core/Cargo.toml` under `[dependencies]`. Version: check crates.io for current stable; the feature-spec doesn't pin a version.

### Services (Tauri State injection)

- `MetadataStore` — the only state needed; injected via `State<'_, MetadataStore>` in all commands
- No new state types required for Phase B

---

## Gotchas and Warnings

### CRITICAL: State<> is not Send

`State<'_, MetadataStore>` cannot cross an `.await` point. Always call `.inner().clone()` before the first `await` in any `async` command. Forgetting this causes a compile error that can be confusing to diagnose. This is the single most common mistake with async Tauri commands.

### CRITICAL: Scroll Container Registration

`useScrollEnhance.ts` manages scroll in WebKitGTK by targeting elements matching the `SCROLLABLE` selector. Any new `overflow-y: auto` container in the Phase B UI additions to `TrainerDiscoveryPanel.tsx` **must** be registered there or dual-scroll jank will occur. Use `overscroll-behavior: contain` on inner containers.

### Stale Cache Bypass

`get_cache_entry()` in `cache_store.rs` filters out expired rows (`expires_at > NOW`). For the stale fallback path, you must query `external_cache_entries` directly via `with_sqlite_conn` without the expiry filter — exactly as `protondb/client.rs:load_cached_lookup_row(allow_expired=true)` does. Do not call `get_cache_entry()` for stale reads.

### tokenize() is Private

`tokenize()` and `token_hits()` in `install/discovery.rs` are private module functions. They cannot be imported from `discovery/matching.rs`. The correct approach is to re-implement them (they are small — 15 lines each) or extract them to a shared module. The spec calls for lifting them into `discovery/matching.rs` as `pub(crate)`.

### No New Schema Migration

The `external_cache_entries` table (migration v4) already exists. Cache key namespace `trainer:source:v1:{key}` is sufficient to avoid collisions. Do not add a new migration for Phase B cache storage.

### Cache Key Normalization

The cache key must be derived from a normalized form of the search query (e.g., lowercased, trimmed, truncated) to maximize cache hits. FLiNG RSS is fetched once per game name lookup, not once per raw search string. See `protondb/models.rs:cache_key_for_app_id` for the pattern.

### FLiNG RSS Reliability

The FLiNG RSS endpoint is not guaranteed uptime. The three-stage cache pattern (live → stale → Unavailable) is essential. The stale fallback must surface `is_stale: true` to the frontend so the UI can show the offline banner. Do not suppress the stale state.

### TypeScript camelCase ↔ Rust snake_case

Tauri's Serde bridge applies the crate-level `rename_all` but only for the struct/enum annotated. Verify each new type has the correct `#[serde(rename_all = "camelCase")]` annotation. Missing this causes silent deserialization failures where fields arrive as `undefined` in TypeScript.

### Async Command Registration

`discovery_search_external` is async — it must be registered in `tauri::generate_handler![]` in `src-tauri/src/lib.rs` just like synchronous commands. Tauri handles both uniformly from the macro's perspective.

---

## Task-Specific Guidance

### For `discovery/client.rs` (Phase B core task)

1. Declare `static FLING_HTTP_CLIENT: OnceLock<reqwest::Client>` at the top.
2. Copy `protondb_http_client()` function verbatim, rename to `fling_http_client()`, switch `PROTONDB_HTTP_CLIENT` reference.
3. Implement `pub async fn search_external_trainers(metadata_store: &MetadataStore, query: &ExternalTrainerSearchQuery) -> ExternalTrainerSearchResponse`.
4. Inside, build the cache key, run the three-stage flow (check cache → fetch RSS → stale fallback).
5. For the RSS fetch: call `client.get(FLING_RSS_URL).send().await`, read response text, parse with `quick-xml`.
6. For persist: call `metadata_store.put_external_trainer_cache(source_url, &cache_key, &json_payload, expires_at)` — this calls `cache_store::put_cache_entry` internally.
7. For stale reads: use `metadata_store.with_sqlite_conn(...)` directly with `allow_expired=true` SQL (no `expires_at > NOW` filter).

### For `discovery/matching.rs` (Phase B scoring task)

1. Lift `tokenize()` and `token_hits()` as `pub(crate)` functions — identical implementation to `install/discovery.rs`.
2. Implement `pub(crate) fn score_external_result(game_name: &str, query_tokens: &[String]) -> f64` using `token_hits`.
3. Implement `pub fn check_version_compatibility(metadata_store: &MetadataStore, community_profile_id: i64, profile_name: &str) -> Result<VersionMatchResult, MetadataStoreError>`:
   - Look up the trainer's `game_version` from `trainer_sources` by `community_profile_id`
   - Look up the profile's `profile_id` from MetadataStore
   - Call `metadata_store.lookup_latest_version_snapshot_for_profile(&profile_id)`
   - Compare `trainer_game_version` vs `snapshot.human_game_ver` to produce `VersionMatchStatus`
   - Return `VersionMatchResult` with advisory text in `detail`

### For `commands/discovery.rs` (IPC additions)

1. Add `use crosshook_core::discovery::{ExternalTrainerSearchQuery, ExternalTrainerSearchResponse, VersionMatchResult, client, matching};` imports.
2. Implement `discovery_search_external` as `pub async fn` following the protondb_lookup pattern exactly.
3. Implement `discovery_check_version_compatibility` as `pub fn` (sync — no HTTP).
4. Extend the `#[cfg(test)] mod tests` block with function-pointer casts for both new commands.

### For `TrainerDiscoveryPanel.tsx` (Phase B UI task)

1. Add "Search Online" button next to the search field — only visible when `settings.discovery_enabled`.
2. Add a second `requestIdRef` for the external search (independent from the local search ref).
3. External results load asynchronously — show a spinner in the external section while loading.
4. Trust badges: local results use `crosshook-discovery-badge--community` (already implemented); external results use a new `crosshook-discovery-badge--external` class.
5. Offline banner: when `response.cacheState?.isStale === true`, show a persistent banner using the existing `crosshook-muted` style.
6. If the external results list introduces a new `overflow-y: auto` container, register it in `useScrollEnhance.ts`.

---

## File-to-Task Mapping (for task-structurer)

| File                                 | Phase B Task                   | Blocks                     |
| ------------------------------------ | ------------------------------ | -------------------------- |
| `discovery/client.rs` (new)          | HTTP client + cache            | IPC command, frontend hook |
| `discovery/matching.rs` (new)        | Token scoring + version check  | IPC command                |
| `discovery/models.rs` (extend)       | New types                      | All downstream             |
| `discovery/mod.rs` (extend)          | Re-exports                     | All downstream             |
| `metadata/mod.rs` (extend)           | Cache + version facade methods | `discovery/client.rs`      |
| `Cargo.toml` (extend)                | Add quick-xml                  | `discovery/client.rs`      |
| `commands/discovery.rs` (extend)     | New IPC commands               | Frontend                   |
| `src-tauri/src/lib.rs` (extend)      | Register commands              | Frontend invocations       |
| `types/discovery.ts` (extend)        | TS types                       | Frontend hook + panel      |
| `TrainerDiscoveryPanel.tsx` (extend) | Phase B UI                     | —                          |
