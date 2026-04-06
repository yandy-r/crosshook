# Trainer Discovery Phase B Implementation Plan

Phase B adds async FLiNG RSS external trainer lookup to the fully-implemented Phase A tap-local discovery. The implementation clones `protondb/client.rs` into `discovery/client.rs` (OnceLock HTTP singleton, 3-stage cache→live→stale-fallback via `external_cache_entries` with `trainer:source:v1:{key}` namespace), adds token scoring and advisory version matching in `discovery/matching.rs`, wires two new IPC commands (async `discovery_search_external` + sync `discovery_check_version_compatibility`), and extends `TrainerDiscoveryPanel.tsx` with progressive loading, trust badges, and offline banners. No new DB migration — schema stays at v18. The only new crate dependency is `quick-xml` for RSS parsing.

## Persistence and Usability

- **Data classification**
  - **External trainer search results** (`discovery_search_external`): ephemeral **SQLite metadata** in `external_cache_entries`, keys under the `trainer:source:v1:*` namespace (e.g. `trainer:source:v1:fling_rss_index`), **1 hour TTL**. Serialized JSON only; not written to the `trainer_sources` tap table.
  - **User preferences**: **`settings.discovery_enabled`** in **TOML** (`AppSettingsData`); consent and gating continue in **`TrainerDiscoveryPanel.tsx`**.
  - **Runtime-only**: **OnceLock** HTTP client singleton for external fetch, plus hook-level **request deduplication** (e.g. `requestIdRef` in a dedicated external-search hook — independent from the local tap search hook).

- **Migrations / backward compatibility**: **No new DB migration** for Phase B — metadata schema stays at **v18**. Phase B is **additive-only** (new discovery modules, cache namespace usage, IPC commands).

- **Offline behavior**: When live HTTP fails or the user is offline, **`discovery_search_external`** should return **stale cache** when an expired row still exists (`is_stale: true`), and set **`offline: true`** when there is no usable cache. **`TrainerDiscoveryPanel.tsx`** should surface an **offline banner** (and trust/offline copy) consistent with progressive loading states.

- **Degraded fallbacks** (external path): **HTTP timeout / network error** → try **stale `external_cache_entries`** → else **empty result + `offline: true`**. **Oversized RSS**, **XML parse errors** (see **`quick-xml`** usage): avoid poisoning cache — e.g. **NULL payload** where the 512 KiB cap applies, **log warnings**, return best-effort or empty per client rules; truncate before serialize (e.g. max items) so normal feeds stay cacheable.

- **User visibility / editability**: Cached external rows and merged discovery results are **read-only** in the UI (not user-editable data). **Trust badges** are **informational** only (Community vs external indicators); they do not block opening links.

- **Version advisory** (`discovery_check_version_compatibility`): **SQLite-only**, uses **`MetadataStore::lookup_profile_id`** then **`lookup_latest_version_snapshot`** (see Advice — no **`ProfileStore`** on the IPC boundary).

## Critically Relevant Files and Documentation

- `docs/plans/trainer-discovery/feature-spec.md`: Authoritative Phase B spec — task list (lines 602–616), Decision 3 (FLiNG RSS only, line 640), IPC signatures (lines 389–401), security findings (lines 555–577). **Start here.**
- `docs/plans/trainer-discovery/shared.md`: Canonical file-path and pattern reference for Phase B — every relevant file, table, pattern, and constraint.
- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: **PRIMARY Phase B template** — OnceLock singleton (line 26), cache-first 3-stage flow (lines 85–130), stale fallback (line 111), `persist_lookup_result` (line 318), `load_cached_lookup_row` with `allow_expired` (line 346).
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry()`, `put_cache_entry()`, `evict_expired_cache_entries()` — the only cache API.
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `compute_correlation_status()` (line 185, pure fn), `lookup_latest_version_snapshot()` (line 75).
- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: `tokenize()` (line 292), `token_hits()` (line 272) — scoring primitives for `matching.rs`.
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs`: Phase A types — Phase B adds `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery`, `DiscoveryCacheState`.
- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: Async IPC command reference — `.inner().clone()` before `await` (line 55).
- `src/crosshook-native/src-tauri/src/commands/discovery.rs`: Phase A sync command + IPC contract test block (lines 19–31).
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`: Frontend hook reference — `requestIdRef` race guard, `{ data, loading, error, refresh }` shape.
- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`: Phase A panel — Phase B adds external results section, trust badges, offline banner.
- `AGENTS.md`: Architecture rules, Tauri IPC conventions, scroll container requirement, persistence classification.

## Implementation Plan

### Phase 1: Foundation (Models + Dependency)

#### Task 1.1: Add quick-xml Dependency + Phase B Model Types

Depends on [none]

**READ THESE BEFORE TASK**

- `docs/plans/trainer-discovery/feature-spec.md` (lines 602–616 for Phase B scope)
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` (existing Phase A types)
- `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs` (Serde conventions)

**Instructions**

Files to Modify

- `src/crosshook-native/crates/crosshook-core/Cargo.toml` — add `quick-xml = "0.37"` under `[dependencies]`
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` — add Phase B types
- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs` — add forward declarations for `pub mod client; pub mod matching;` (commented out until files exist, or as empty stub modules)

Add `quick-xml` to `Cargo.toml`. This is the **only new crate dependency** for Phase B.

Add these types to `discovery/models.rs`:

- `ExternalTrainerResult` — `#[serde(rename_all = "camelCase")]`; fields: `game_name: String`, `source_name: String` (e.g. `"FLiNG"`), `source_url: String` (trainer page URL — never a direct download), `pub_date: Option<String>`, `source: String` (e.g. `"fling_rss"`), `relevance_score: f64`.
- `ExternalTrainerSearchQuery` — `#[serde(rename_all = "camelCase")]`; fields: `game_name: String`, `steam_app_id: Option<String>`, `force_refresh: Option<bool>`.
- `ExternalTrainerSearchResponse` — `#[serde(rename_all = "camelCase")]`; fields: `results: Vec<ExternalTrainerResult>`, `source: String`, `cached: bool`, `cache_age_secs: Option<i64>`, `is_stale: bool`, `offline: bool`.
- `DiscoveryCacheState` — `#[derive(Default)]` + `#[serde(rename_all = "snake_case")]`; variants: `Fresh`, `Stale`, `#[default] Unavailable`.

Apply Serde conventions: `#[serde(default, skip_serializing_if = "Option::is_none")]` on optional fields. Follow `TrainerSearchResult` as the direct template.

### Phase 2: Core Logic (HTTP Client + Matching)

#### Task 2.1: FLiNG RSS HTTP Client

Depends on [1.1]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` (full file — this is the template)
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` (cache API)
- `docs/plans/trainer-discovery/research-integration.md` (FLiNG RSS XML structure)

**Instructions**

Files to Create

- `src/crosshook-native/crates/crosshook-core/src/discovery/client.rs`

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs` — add `pub mod client;` and `pub use client::search_external_trainers;`

**Clone `protondb/client.rs` as the starting point**, then adapt:

1. **OnceLock HTTP singleton**: Declare `static FLING_HTTP_CLIENT: OnceLock<reqwest::Client>` at the top. Create `fn fling_http_client() -> Result<&'static reqwest::Client, DiscoveryError>` following the exact `protondb_http_client()` pattern. Set timeout to 10s (FLiNG may be slower). **Must set `CrossHook/{version}` user-agent** or FLiNG returns 403.

2. **Constants**: `const FLING_RSS_URL: &str = "https://flingtrainer.com/category/trainer/feed/";`, `const CACHE_TTL_HOURS: i64 = 1;`, `const REQUEST_TIMEOUT_SECS: u64 = 10;`, `const CACHE_NAMESPACE: &str = "trainer:source:v1";`, `const MAX_CACHED_ITEMS: usize = 200;`.

3. **Private error type**: `enum DiscoveryError { Network(reqwest::Error), ParseError(String), Store(MetadataStoreError) }` with manual `fmt::Display` impl. Never `pub`. Never `anyhow`. Never `thiserror`.

4. **Public async function**: `pub async fn search_external_trainers(metadata_store: &MetadataStore, query: &ExternalTrainerSearchQuery) -> Result<ExternalTrainerSearchResponse, DiscoveryError>`.

5. **3-stage cache-first flow** (clone from `protondb/client.rs:85-130`):
   - Build cache key: `format!("{CACHE_NAMESPACE}:fling_rss_index")`
   - Stage 1: Check `external_cache_entries` for valid (non-expired) row via `metadata_store.get_cache_entry(&cache_key)`. On hit: deserialize payload, filter by query tokens using `matching::score_fling_result()`, return with `cached: true`.
   - Stage 2: HTTP GET `FLING_RSS_URL`, call `.text().await?` (not `.json()`), parse XML with `quick-xml` event reader. Extract `<item>` elements: `<title>` (strip trainer suffix via `matching::strip_trainer_suffix()`), `<link>` (canonical source URL), `<pubDate>` (raw string). Truncate to `MAX_CACHED_ITEMS` before serializing. Call `metadata_store.put_cache_entry(FLING_RSS_URL, &cache_key, &json_payload, Some(&expires_at))`.
   - Stage 3: On HTTP/parse error: `tracing::warn!(...)` then query `external_cache_entries` directly via `metadata_store.with_sqlite_conn(...)` with `allow_expired=true` SQL (no `expires_at > NOW` filter — same pattern as `protondb/client.rs:346-394`). Return stale result with `is_stale: true`.
   - Stage 4: On total failure (no stale row): return `ExternalTrainerSearchResponse { results: vec![], source: "fling_rss".into(), cached: false, cache_age_secs: None, is_stale: false, offline: true }`.

6. **Content-Type validation** (S3 cache poisoning mitigation): Before parsing, check `Content-Type` header contains `xml` or `rss`. Reject HTML/JSON captive portal responses.

7. **Response size guard**: Abort if response body exceeds 1 MB before reading fully.

Add minimal compile-check tests: `cache_hit_returns_without_fetch`, `empty_query_returns_empty`. Full test suite in Task 3.2.

#### Task 2.2: Token Scoring + Advisory Version Matching

Depends on [1.1]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs` (lines 272–304: `tokenize()`, `token_hits()`)
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` (lines 185–211: `compute_correlation_status()`)
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` (VersionMatchStatus, VersionMatchResult)

**Instructions**

Files to Create

- `src/crosshook-native/crates/crosshook-core/src/discovery/matching.rs`

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs` — add `pub mod matching;` and re-export public functions

This file contains **only pure functions — zero I/O, zero MetadataStore dependency**. This purity enables it to be developed and tested in parallel with `client.rs`.

1. **Token scoring primitives** (duplicate from `install/discovery.rs` — do not create cross-domain import):
   - `pub(crate) fn tokenize(value: &str) -> Vec<String>` — split on non-alphanumeric, lowercase, filter < 2 chars. Identical to `install/discovery.rs:292-304`.
   - `pub(crate) fn token_hits(value: &str, target_tokens: &[String]) -> usize` — count substring matches. Identical to `install/discovery.rs:272-277`.

2. **RSS title normalization**:
   - `pub fn strip_trainer_suffix(title: &str) -> String` — removes `" Trainer"` suffix and optional preceding version string (e.g. `"Elden Ring v1.12 +DLC Trainer"` → `"Elden Ring"`, `"Elden Ring Trainer"` → `"Elden Ring"`). Case-insensitive. Strip from last occurrence of `Trainer` (case-insensitive), then optionally strip version prefix like `v1.12` or `v1.12 +DLC` from the end.

3. **Relevance scoring**:
   - `pub fn score_fling_result(query: &str, rss_game_name: &str) -> f64` — tokenize both, compute `token_hits(rss_game_name_lower, &query_tokens) as f64 / query_tokens.len().max(1) as f64`. Returns `0.0..=1.0`. Results scoring `< 0.1` should be filtered by the caller.

4. **Advisory version matching**:
   - `pub fn match_trainer_version(trainer_game_version: Option<&str>, installed_human_game_ver: Option<&str>) -> VersionMatchResult` — pure function, no I/O. Returns `VersionMatchStatus::Unknown` when either input is `None`. String equality check for `Exact`. Contains check for `Compatible`. Otherwise returns `Outdated`. Always advisory — never blocking. Populate `detail` field with a human-readable explanation.

Add comprehensive `#[cfg(test)]` module:

- `strip_trainer_suffix` tests: `"Elden Ring Trainer"` → `"Elden Ring"`, `"Elden Ring v1.12 Trainer"` → `"Elden Ring"`, `"Elden Ring v1.12 +DLC Trainer"` → `"Elden Ring"`, already-stripped title unchanged, empty string handled
- `score_fling_result` tests: exact match → `1.0`, partial match `> 0.0`, unrelated → `< 0.1`, empty strings
- `match_trainer_version` tests: matching → `Exact`, mismatch → `Outdated`, `None` inputs → `Unknown`
- `tokenize` tests: basic splitting, lowercase, single-char filtering

### Phase 3: IPC Integration

#### Task 3.1: Async IPC Commands + Registration

Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src-tauri/src/commands/protondb.rs` (async command pattern — `.inner().clone()` at line 55)
- `src/crosshook-native/src-tauri/src/commands/discovery.rs` (existing Phase A sync command + contract test)
- `src/crosshook-native/src-tauri/src/lib.rs` (command registration in `generate_handler![]` macro)

**Instructions**

Files to Modify

- `src/crosshook-native/src-tauri/src/commands/discovery.rs` — add two new commands + extend contract test block
- `src/crosshook-native/src-tauri/src/lib.rs` — register new commands in `tauri::generate_handler![]`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — add MetadataStore facade methods for Phase B

Add these MetadataStore public methods in `metadata/mod.rs`:

```rust
pub fn lookup_latest_version_snapshot_for_profile(&self, profile_id: &str) -> Result<Option<VersionSnapshotRow>, MetadataStoreError> {
    self.with_conn("lookup latest version snapshot", |conn| {
        version_store::lookup_latest_version_snapshot(conn, profile_id)
    })
}
```

Add two commands to `commands/discovery.rs`:

**`discovery_search_external`** (async — follows `protondb_lookup` pattern exactly):

```rust
#[tauri::command]
pub async fn discovery_search_external(
    query: ExternalTrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ExternalTrainerSearchResponse, String> {
    let metadata_store = metadata_store.inner().clone(); // MANDATORY before .await
    crosshook_core::discovery::search_external_trainers(&metadata_store, &query)
        .await
        .map_err(|e| e.to_string())
}
```

**`discovery_check_version_compatibility`** (sync — SQLite only, no HTTP; **no** `metadata_store.inner().clone()` — there is no `.await`):

```rust
#[tauri::command]
pub fn discovery_check_version_compatibility(
    profile_name: String,
    trainer_game_version: Option<String>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<VersionMatchResult, String> {
    // 1. Resolve display name → profile_id via MetadataStore (profiles table in SQLite).
    let Some(profile_id) = metadata_store
        .lookup_profile_id(&profile_name)
        .map_err(|e| e.to_string())?
    else {
        return Err(format!("unknown profile: {profile_name}"));
    };
    // 2. Latest row from version_snapshots (version_store).
    let snapshot = metadata_store
        .lookup_latest_version_snapshot(&profile_id)
        .map_err(|e| e.to_string())?;
    // 3. Compare trainer_game_version with snapshot (e.g. human_game_ver) using matching::match_trainer_version()
    // Return VersionMatchResult
}
```

**Profile identity**: Local profiles are recorded in metadata **`profiles`** (`profile_id`, `current_filename`, …). **`MetadataStore::lookup_profile_id`** is the supported resolver (same role as a hypothetical `lookup_profile_by_name` / `resolve_profile_id`). **`ProfileStore`** is **not** required on this command — avoid an extra Tauri `State` unless a future use case needs on-disk TOML profile paths not reflected in metadata.

**MANDATORY**: Extend the `#[cfg(test)]` contract test block to include function-pointer casts for both new commands. Follow `commands/discovery.rs:19-31` exactly:

```rust
let _ = discovery_search_external
    as fn(ExternalTrainerSearchQuery, State<'_, MetadataStore>) -> Result<ExternalTrainerSearchResponse, String>;

let _ = discovery_check_version_compatibility
    as fn(String, Option<String>, State<'_, MetadataStore>) -> Result<VersionMatchResult, String>;
```

Register both new commands in `src-tauri/src/lib.rs` `tauri::generate_handler![]` in the `// Trainer discovery` block:

```rust
commands::discovery::discovery_search_trainers,                 // Phase A (existing)
commands::discovery::discovery_search_external,                 // Phase B new
commands::discovery::discovery_check_version_compatibility,     // Phase B new
```

Run verification: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Phase 4: Frontend

#### Task 4.1: TypeScript Types + External Search Hook

Depends on [3.1]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/types/discovery.ts` (existing Phase A types)
- `src/crosshook-native/src/hooks/useTrainerDiscovery.ts` (Phase A hook — pattern reference)
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` (requestIdRef race guard reference)

**Instructions**

Files to Create

- `src/crosshook-native/src/hooks/useExternalTrainerSearch.ts`

Files to Modify

- `src/crosshook-native/src/types/discovery.ts` — add Phase B TypeScript interfaces

Add these interfaces to `types/discovery.ts`:

```typescript
export interface ExternalTrainerSearchQuery {
  gameName: string;
  steamAppId?: string;
  forceRefresh?: boolean;
}

export interface ExternalTrainerResult {
  gameName: string;
  sourceName: string;
  sourceUrl: string;
  pubDate?: string;
  source: string;
  relevanceScore: number;
}

export interface ExternalTrainerSearchResponse {
  results: ExternalTrainerResult[];
  source: string;
  cached: boolean;
  cacheAgeSecs?: number;
  isStale: boolean;
  offline: boolean;
}
```

Create `useExternalTrainerSearch.ts` hook:

- Signature: `useExternalTrainerSearch(gameName: string, options?: { steamAppId?: string })` returning `{ data: ExternalTrainerSearchResponse | null; loading: boolean; error: string | null; search: (forceRefresh?: boolean) => Promise<void> }`
- Uses `requestIdRef.current` race guard pattern (separate ref from the local search hook)
- Does **NOT** auto-fire on every keystroke — triggered manually by the "Search Online" button via `search()` callback
- `invoke<ExternalTrainerSearchResponse>('discovery_search_external', { query: { gameName, steamAppId, forceRefresh } })`
- Empty `gameName` guard: return null data without IPC call

#### Task 4.2: TrainerDiscoveryPanel Phase B Integration

Depends on [4.1]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` (existing Phase A panel)
- `docs/plans/trainer-discovery/research-ux.md` (trust badges, progressive loading, offline banner)
- `src/crosshook-native/src/hooks/useScrollEnhance.ts` (SCROLLABLE selector)
- `src/crosshook-native/src/styles/variables.css` (CSS custom properties)

**Instructions**

Files to Modify

- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` — add external results section
- `src/crosshook-native/src/hooks/useScrollEnhance.ts` — register new scroll container if needed
- `src/crosshook-native/src/styles/theme.css` — add trust badge CSS classes

Add to `TrainerDiscoveryPanel.tsx`:

1. **"Search Online" button**: Visible only when `settings.discovery_enabled` and query is non-empty. Triggers `useExternalTrainerSearch.search()`. Positioned next to the search input.

2. **External results section**: Rendered below the existing local results section. Shows a spinner while `externalData.loading`. Uses the same `TrainerResultCard` component — external results get `crosshook-discovery-badge--external` badge class (chain-link icon, muted) vs local results which keep `crosshook-discovery-badge--community` (filled badge, accent color). External source label: `"{sourceName} (External)"` in the badge.

3. **Trust badges**: Two-tier model:
   - Community tap results: filled badge, accent color, "Community" label (already implemented in Phase A)
   - External (FLiNG) results: muted chain-link icon, "{sourceName}" label. Never blocks link opening — informational only.

4. **Offline banner**: When `externalData?.offline === true`, show persistent inline banner above external results: `"Online search unavailable. Showing local results only."` with "Retry" button calling `search(true)`. Use `crosshook-muted` styling.

5. **Cache age indicator**: When `externalData?.cached === true`, show `"Results from cache, {N} min ago"` using `cacheAgeSecs`.

6. **Stale indicator**: When `externalData?.isStale === true`, show `"Results may be outdated"` with muted styling.

7. **Progressive loading**: Local results render immediately (Phase A `useTrainerDiscovery`). External results load independently — external spinner does NOT block local results. Local and external use separate `requestIdRef` instances.

**CRITICAL**: If a new `overflow-y: auto` container is added for the external results list, it MUST be registered in the `SCROLLABLE` selector in `useScrollEnhance.ts`. Use `overscroll-behavior: contain` on inner containers. Missing this causes dual-scroll jank under WebKitGTK.

Keep `TrainerDiscoveryPanel.tsx` under 400 lines. Extract `ExternalResultsSection.tsx` as a separate component if the file exceeds this. External links: always `shellOpen()` via `@tauri-apps/plugin-shell`, never `<a href>`, never `dangerouslySetInnerHTML`.

#### Task 4.3: Phase B Rust Unit Tests (Full Suite)

Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/discovery/search.rs` (test patterns at lines 105–322)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (MetadataStore::open_in_memory)

**Instructions**

This task writes the **complete test suite** for Phase B, expanding the minimal compile-check tests added in Tasks 2.1 and 2.2. Do not add a second `#[cfg(test)]` module — extend the existing ones.

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/discovery/client.rs` — expand `#[cfg(test)]` module
- `src/crosshook-native/crates/crosshook-core/src/discovery/matching.rs` — expand `#[cfg(test)]` module (if not already comprehensive from Task 2.2)

Test cases for `client.rs` (expand existing module):

- `cache_hit_returns_without_http_fetch` — pre-populate `MetadataStore::open_in_memory()` with a valid (non-expired) cache entry, verify `search_external_trainers` returns the cached result without HTTP
- `cache_miss_with_disabled_store_returns_offline` — `MetadataStore::disabled()` returns `offline: true`
- `stale_fallback_returns_is_stale_true` — pre-populate with an expired cache entry, verify stale result has `is_stale: true`
- `cache_key_uses_correct_namespace` — verify stored key matches `"trainer:source:v1:fling_rss_index"`
- `truncation_keeps_under_512kib` — verify serialization of 200 items stays under `MAX_CACHE_PAYLOAD_BYTES`

Test cases for `matching.rs` (verify Task 2.2 coverage, add if missing):

- `strip_trainer_suffix` edge cases: mixed case, no suffix, version with plus modifiers
- `score_fling_result` edge cases: empty query, single-character tokens, non-ASCII
- `match_trainer_version` edge cases: both `None`, only one `None`, exact match, partial match

All tests use `MetadataStore::open_in_memory()` for store-dependent tests. Pure function tests in `matching.rs` need no store at all.

Run verification: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

## Advice

- **Clone `protondb/client.rs` as `discovery/client.rs` starting point** — the patterns are identical. Rename constants, swap the static name, change the URL and XML parsing logic. This is explicitly the intended approach and reduces risk.
- **FLiNG RSS endpoint needs live verification first** — `curl -H "User-Agent: CrossHook/0.1" https://flingtrainer.com/category/trainer/feed/` before writing the XML parser. If it returns 403 or HTML instead of XML, the `scraper` HTML fallback becomes mandatory.
- **`discovery_check_version_compatibility` and `MetadataStore` only** — `profile_name` → `profile_id` is resolved with **`MetadataStore::lookup_profile_id`** (backed by the SQLite `profiles` table). Then call **`lookup_latest_version_snapshot(&profile_id)`**. **`ProfileStore`** is **not** part of this IPC surface. The command is **synchronous** (no HTTP, no `.await`), so **do not** use `metadata_store.inner().clone()` here; reserve **`.inner().clone()`** for **`discovery_search_external`** (async, before the first `.await`), same as `protondb_lookup`.
- **FLiNG RSS title format variations**: Titles follow `"Game Name Trainer"`, `"Game Name v{ver} Trainer"`, or `"Game Name v{ver} (+N Trainer)"`. The `strip_trainer_suffix` function must handle all three. Test with real RSS data.
- **FLiNG download links are NOT stable** — OneDrive/Google Drive URLs from `<description>` expire. Store only the trainer page URL from `<link>` field. Never extract URLs from `<content:encoded>`.
- **Oversized RSS → NULL cache**: If the FLiNG RSS payload exceeds 512 KiB after serialization, `put_cache_entry` silently stores NULL. Truncate to `MAX_CACHED_ITEMS` (200) before serializing to prevent this.
- **Two separate `requestIdRef` instances**: The local search hook and external search hook must use independent request ID refs. A new local search must not cancel an in-flight external search (different lifecycles).
- **`compute_correlation_status()` from `version_store.rs` is NOT directly reusable** for trainer version comparison — it compares Steam build IDs and file hashes, not human-readable version strings. Write new advisory logic in `matching.rs`.
- **FTS5 is NOT available** — `rusqlite` uses `features = ["bundled"]` only. Do not use FTS5 queries or `MATCH` syntax in Phase B. LIKE is the only correct approach.
- **IPC contract test is non-optional** — the `#[cfg(test)]` function-pointer cast block in `commands/discovery.rs` must include all three commands (Phase A sync + Phase B async + Phase B sync). Missing this breaks the established convention.
- **External results are ephemeral** — they live only in `external_cache_entries` JSON blob. They are NEVER written to the `trainer_sources` table. The frontend distinguishes them via the `source` field and trust badge.
- **Cache strategy: single key for full index** — use `trainer:source:v1:fling_rss_index` (1h TTL) for the full RSS feed. Filter results in-memory by query tokens after deserialization. This is simpler than per-game keys and matches the spec.
- **`useScrollEnhance.ts` registration is a WebKitGTK requirement** — missing it causes dual-scroll jank visible only in the AppImage, not in dev browser. Register any new scrollable container on first commit.
