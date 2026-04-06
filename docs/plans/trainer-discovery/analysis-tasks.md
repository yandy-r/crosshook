# Task Structure Analysis: trainer-discovery

## Executive Summary

Phase A (MVP) introduces a new `trainer_sources` table (v17→v18 migration), a `discovery/` module in `crosshook-core`, a thin IPC command file, and a `TrainerDiscoveryPanel` React component. All foundation tasks (schema, models, tap indexer extension) are serialized; the UI and tests can proceed in parallel once the IPC layer is complete. Phase B adds external HTTP lookup following the ProtonDB client pattern and is gated on Phase A shipping. The only new crate dependency for Phase B is `quick-xml` for RSS parsing — all other crates (`reqwest`, `tokio`, `chrono`, `serde`) are already present.

---

## Recommended Phase Structure

### Phase A: Community Tap MVP

Tasks ordered from foundation to integration; parallel groups are noted explicitly.

#### A1 — Schema Migration (BLOCKER for A2, A3, A4)

**Files (2):**

- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — add `migrate_17_to_18()` creating `trainer_sources` table + two indexes
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — add `TrainerSourceRow` struct mapping to the new table columns

**Details:**

- Schema v17→v18: `CREATE TABLE trainer_sources (id, tap_id FK, game_name, steam_app_id, source_name, source_url, trainer_version, game_version, notes, sha256, relative_path, created_at)` + `UNIQUE(tap_id, relative_path, source_url)` + indexes on `game_name` and `steam_app_id`
- Clears `last_head_commit` watermark on all taps to force re-index (same pattern as previous migrations that extended `community_profiles`)
- `TrainerSourceRow` follows existing `CommunityProfileRow` field layout in `models.rs`

---

#### A2 — Discovery Domain Models (BLOCKER for A3, A4; can start in parallel with A1 after migration SQL is finalized)

**Files (2):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs` — new module root, `pub use` re-exports
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` — `TrainerSourcesManifest`, `TrainerSourceEntry`, `TrainerSearchQuery`, `TrainerSearchResult`, `TrainerSearchResponse`, `VersionMatchStatus`, `VersionMatchResult`

**Details:**

- All IPC-crossing types: `#[serde(rename_all = "camelCase")]` on result structs; `#[serde(rename_all = "snake_case")]` on `VersionMatchStatus` enum; `#[serde(default, skip_serializing_if = "Option::is_none")]` on optional fields
- `TrainerSourcesManifest` / `TrainerSourceEntry` are the JSON-deserialization types for `trainer-sources.json` tap files
- `TrainerSearchQuery` / `TrainerSearchResponse` / `TrainerSearchResult` are the IPC boundary types
- Register module in `src/crosshook-native/crates/crosshook-core/src/lib.rs` — add `pub mod discovery;`

---

#### A3 — Search Logic (BLOCKER for A5; depends on A1 + A2)

**Files (1):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/search.rs` — `search_trainers()` free function taking `&Connection`, returns `Result<TrainerSearchResponse, MetadataStoreError>`

**Details:**

- LIKE query on `trainer_sources` JOIN `community_taps` — fields: `game_name`, `source_name`, `notes`
- Pagination via `LIMIT ?2 OFFSET ?3`
- Error on empty query: return `Err` with message `"search query cannot be empty"`
- Handle `MetadataStore::disabled()` path: return empty `TrainerSearchResponse` (never panic)
- This is a pure function (no I/O beyond the connection) — directly unit-testable with `MetadataStore::open_in_memory()`

---

#### A4 — Tap Indexer Extension (BLOCKER for A5; depends on A1 + A2)

**Files (2):**

- `src/crosshook-native/crates/crosshook-core/src/community/index.rs` — walk tap directories for `trainer-sources.json` alongside `community-profile.json`
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — add `index_trainer_sources()` free function; apply A6 field-length bounds + HTTPS-only URL validation before INSERT

**Details:**

- `index_trainer_sources()` follows the `index_community_tap_result()` transactional pattern: `Immediate` transaction, DELETE WHERE `tap_id = ?` then INSERT per source entry
- URL validation: HTTPS-only scheme check (same allow-list as `validate_tap_url()` in `community/taps.rs`)
- A6 field bounds applied to `source_name`, `source_url`, `notes`, `game_name` before INSERT
- `trainer-sources.json` parse failures are logged as diagnostics (non-fatal), not errors — mirrors `community-profile.json` parse failure handling

---

#### A5 — MetadataStore Facade Method + IPC Commands (BLOCKER for A6, A7; depends on A3 + A4)

**Files (3):**

- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — expose `search_trainer_sources()` public method on `MetadataStore` (delegates to `discovery/search.rs` via `with_conn`)
- `src/crosshook-native/src-tauri/src/commands/discovery.rs` — new file: `discovery_search_trainers` sync IPC command; IPC contract test block (`#[cfg(test)]` casting each handler to function-pointer type — MANDATORY)
- `src/crosshook-native/src-tauri/src/commands/mod.rs` — add `pub mod discovery;`

**Details:**

- `discovery_search_trainers` is a **sync** `fn` (SQLite, no network) — do NOT use `async fn`
- Inject `State<'_, MetadataStore>`, delegate to `metadata_store.search_trainer_sources(query)`, map errors with `.map_err(|e| e.to_string())`
- Register command in `src/crosshook-native/src-tauri/src/lib.rs` `invoke_handler!`
- IPC contract test block is mandatory — see `commands/community.rs:311–353` for pattern

---

#### A6 + A7 — Frontend (can run in parallel once A5 ships; A6 and A7 are independent of each other)

**A6 — TypeScript Types + Hook**

**Files (2):**

- `src/crosshook-native/src/types/discovery.ts` — `VersionMatchStatus`, `TrainerSearchQuery`, `TrainerSearchResult`, `TrainerSearchResponse` TypeScript interfaces
- `src/crosshook-native/src/hooks/useTrainerDiscovery.ts` — React hook wrapping `invoke<TrainerSearchResponse>('discovery_search_trainers', ...)` with `requestIdRef.current` race guard, loading/error state, `forceRefresh` pattern

**Details:**

- Add `export * from './discovery'` in `src/crosshook-native/src/types/index.ts`
- Hook signature: `useTrainerDiscovery(query: string, options?: { limit?: number; offset?: number })` returning `{ data, loading, error, refresh }`
- 300ms debounce on query changes before IPC call (matches UX spec)
- Model after `useProtonDbSuggestions.ts` pattern exactly

---

**A7 — TrainerDiscoveryPanel Component**

**Files (1):**

- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` — search input (debounced), result cards with progressive disclosure via `CollapsibleSection`, compatibility badge reusing `crosshook-protondb-tier-badge` tokens, source link CTA opening via Tauri `open()`, offline banner, empty state, legal disclaimer dialog on first open

**Details:**

- Any `overflow-y: auto` container MUST be added to the `SCROLLABLE` selector in `useScrollEnhance.ts` (critical — see `AGENTS.md`)
- Inner scroll containers: `overscroll-behavior: contain`
- External links: never `<a href>` — always `invoke('tauri_open', ...)` or the Tauri shell `open()` API
- No `dangerouslySetInnerHTML` — XSS prevention (S5 from security spec)
- ARIA live region on results count (`aria-live="polite"`)
- Legal disclaimer: shown once on first panel open (keyed to `discovery_enabled` settings flag), matches the opt-in consent design decision

---

#### A8 — Unit Tests (can start in parallel with A6/A7 once A3 is complete)

**Files (1):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/search.rs` — `#[cfg(test)]` module with tests using `MetadataStore::open_in_memory()`

**Test cases:**

- Empty query returns error
- LIKE match on `game_name`, `source_name`, `notes`
- Pagination (limit/offset)
- `MetadataStore::disabled()` path returns empty results
- URL validation: HTTPS accepted, HTTP rejected, `javascript:` rejected
- A6 field bounds: overlong fields rejected

---

### Phase B: External Source Lookup

All Phase B tasks depend on Phase A shipping. Within Phase B, the dependency chain is: B1 (models) → B2 (HTTP client) ∥ B3 (matching.rs) → B4 (IPC layer) → B5 (frontend) ∥ B6 (tests).

#### B1 — Cargo Dependency + Phase B Models (BLOCKER for B2, B3)

**Files (2):**

- `src/crosshook-native/crates/crosshook-core/Cargo.toml` — add `quick-xml = { version = "0.36", features = ["serialize"] }`
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` — extend with Phase B types

**Details:**

- `quick-xml` is the **only new crate dependency** for Phase B. All other crates (`reqwest`, `tokio`, `chrono`, `serde_json`, `rusqlite`) are already present in `Cargo.toml`. `reqwest` already has `rustls-tls`; add `"text"` feature to `reqwest` if not present (FLiNG RSS is XML, not JSON — use `.text().await?` not `.json()`).
- New types to add in `discovery/models.rs`:
  - `ExternalTrainerResult` — `#[serde(rename_all = "camelCase")]`; fields: `game_name`, `source_name` (`"FLiNG"`), `source_url` (trainer page URL), `pub_date: Option<String>`, `source` (`"fling_rss"`)
  - `ExternalTrainerSearchResponse` — fields: `results: Vec<ExternalTrainerResult>`, `source: String`, `cached: bool`, `cache_age_secs: Option<i64>`, `is_stale: bool`, `offline: bool`
  - `ExternalTrainerSearchQuery` — fields: `game_name: String`, `steam_app_id: Option<String>`
  - `DiscoveryCacheState` enum — `Fresh`, `Stale`, `Unavailable`; `#[serde(rename_all = "snake_case")]`
- **Rationale for B1 as blocker**: `client.rs` imports `ExternalTrainerResult` and `ExternalTrainerSearchResponse`; `matching.rs` imports `ExternalTrainerResult`. Both B2 and B3 cannot compile until B1 types are defined.

---

#### B2 — FLiNG RSS HTTP Client (depends on B1; parallel with B3)

**Files (1):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/client.rs` — new file: OnceLock HTTP client singleton, 3-stage cache-first fetch, RSS XML parsing, stale fallback

**Details:**

- `static FLING_HTTP_CLIENT: OnceLock<reqwest::Client>` initialized with `CrossHook/{version}` user-agent and 10s timeout. **Mandatory**: without the `User-Agent` header, FLiNG returns HTTP 403.
- **3-stage cache-first flow** (clone from `protondb/client.rs:85–130`):
  1. `metadata_store.get_cache_entry("trainer:source:v1:fling_rss_index")` — return on non-expired hit
  2. HTTP GET `https://flingtrainer.com/category/trainer/feed/` → `response.text().await?` → parse XML with `quick-xml`
  3. On parse success: `put_cache_entry(source_url, cache_key, payload, expires_at_1h)` → return result with `cached: false`
  4. On HTTP/parse error: query `external_cache_entries` directly via `with_sqlite_conn` with `allow_expired=true` (same direct-table-query pattern as `protondb/client.rs:346–394`) → return stale result with `is_stale: true`
  5. On total failure (no stale row): return `ExternalTrainerSearchResponse { results: vec![], offline: true }`
- **RSS parsing**: `quick-xml` event reader over `<item>` elements; extract `<title>` (strip `" Trainer"` / `" v{ver} Trainer"` suffix, case-insensitive), `<link>` (canonical source_url), `<pubDate>` (raw RFC 822 string). Skip `<description>` and `<content:encoded>` — never parse HTML from trainer pages.
- **Cache key**: `trainer:source:v1:fling_rss_index` for the full feed index (1h TTL). Per-game filtered results are not separately cached at this stage — the full index is fetched once and filtering happens in-memory.
- **Oversized payload guard**: Truncate to top 200 items before JSON serialization to stay well under the 512 KiB `MAX_CACHE_PAYLOAD_BYTES` limit. The FLiNG RSS feed (~400+ entries) may approach the cap if serialized without truncation.
- **Domain error type**: Private `enum TrainerDiscoveryError { Network(reqwest::Error), ParseError(String), Store(MetadataStoreError) }` with `fmt::Display`. Never exposed at IPC boundary — map to `String` at `commands/discovery.rs`.
- **`discovery/mod.rs` update**: Add `pub mod client;` and `pub use client::search_external;` alongside existing Phase A declarations.

---

#### B3 — Shared Text Utils + Token Scoring and Version Matching (depends on B1; parallel with B2)

**Files (2):**

- `src/crosshook-native/crates/crosshook-core/src/text_utils.rs` — new file: lift `tokenize()` and `token_hits()` from `install/discovery.rs` into a shared crate-level module
- `src/crosshook-native/crates/crosshook-core/src/discovery/matching.rs` — new file: token scoring, RSS title normalization, advisory version comparison (imports from `text_utils`)

**Details:**

- **`text_utils.rs` first** — create `crosshook-core/src/text_utils.rs` as a `pub(crate)` module. Lift `tokenize()` (line 292 of `install/discovery.rs`) and `token_hits()` (line 272) verbatim as `pub(crate) fn`. Register in `lib.rs` as `pub(crate) mod text_utils;`. Update `install/discovery.rs` to import via `use crate::text_utils::{tokenize, token_hits};` — do not duplicate the implementation. This is a resolved design decision from `research-practices.md` to avoid a cross-domain dependency from `discovery/` directly into `install/`. The entire `text_utils.rs` is ~20 lines.
- **Token scoring in `matching.rs`**: Import `tokenize` and `token_hits` from `crate::text_utils`. These are pure functions (no I/O) — no `MetadataStore` dependency. `tokenize()` splits on non-alphanumeric, lowercases, filters tokens < 2 chars. `token_hits()` counts substring matches.
- **`score_fling_result(query: &str, rss_title: &str) -> f64`**: Tokenize both query and stripped RSS title; compute token hit ratio normalized to `[0.0, 1.0]`. Results with `score < 0.1` are filtered from the response.
- **RSS title normalization**: `strip_trainer_suffix(title: &str) -> String` — removes `Trainer` suffix (case-insensitive) and version suffixes like `" v1.12 Trainer"` or `" v1.12 +DLC Trainer"` (case-insensitive, strip from last occurrence of `v` before `Trainer` or from `Trainer` directly). Version strings are NOT semver — never use `semver` crate.
- **Advisory version matching**: `check_version_advisory(trainer_game_version: Option<&str>, snapshot_human_game_ver: Option<&str>) -> VersionMatchStatus` — string equality check only. Returns `VersionMatchStatus::Unknown` when either field is `None` (snapshot may not exist for unplayed profiles). No blocking behavior — purely advisory.
- **`discovery/mod.rs` update**: Add `pub mod matching;` and `pub use matching::{score_fling_result, strip_trainer_suffix, check_version_advisory};`.

---

#### B4 — MetadataStore Phase B Methods + Async IPC Commands (depends on B2 + B3; BLOCKER for B5, B6)

**Files (3):**

- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — add `search_external_trainers()` and `check_version_compatibility()` public methods on `MetadataStore`
- `src/crosshook-native/src-tauri/src/commands/discovery.rs` — add `discovery_search_external` async IPC command and `discovery_check_version_compatibility` sync IPC command; update IPC contract test block
- `src/crosshook-native/src-tauri/src/lib.rs` — register both new commands in the `// Trainer discovery` block of `tauri::generate_handler![]`

**Details:**

- **`discovery_search_external`** (async):

  ```rust
  #[tauri::command]
  pub async fn discovery_search_external(
      query: ExternalTrainerSearchQuery,
      metadata_store: State<'_, MetadataStore>,
  ) -> Result<ExternalTrainerSearchResponse, String> {
      let metadata_store = metadata_store.inner().clone(); // MANDATORY before .await
      crosshook_core::discovery::search_external(&metadata_store, &query)
          .await
          .map_err(|e| e.to_string())
  }
  ```

  The `.inner().clone()` before the first `.await` is MANDATORY — see `commands/protondb.rs:55`. `State<'_>` cannot cross await boundaries without cloning the inner value.

- **`discovery_check_version_compatibility`** (sync — SQLite only, no network):

  ```rust
  #[tauri::command]
  pub fn discovery_check_version_compatibility(
      profile_name: String,
      trainer_game_version: Option<String>,
      metadata_store: State<'_, MetadataStore>,
      profile_store: State<'_, ProfileStore>,
  ) -> Result<VersionMatchResult, String> { ... }
  ```

  Inject `profile_store` to resolve `profile_id` from `profile_name`. Look up `version_snapshots` via `lookup_latest_version_snapshot(conn, &profile_id)`. Compare `snapshot.human_game_ver` against `trainer_game_version` using `matching::check_version_advisory()`. Return `VersionMatchStatus::Unknown` when snapshot is `None` — never an error.

- **IPC contract test block update**: The existing `#[cfg(test)]` block in `commands/discovery.rs` must be extended to cast `discovery_search_external` and `discovery_check_version_compatibility` to their explicit function-pointer types — same mandatory compile-time IPC validation pattern as `commands/community.rs:311–353`.
- **`lib.rs` registration**: Add to the `// Trainer discovery` block:

  ```rust
  commands::discovery::discovery_search_trainers,     // Phase A (existing)
  commands::discovery::discovery_search_external,     // Phase B new
  commands::discovery::discovery_check_version_compatibility, // Phase B new
  ```

---

#### B5 — Frontend Types + Hook + Panel Integration (depends on B4; parallel with B6)

**Files (3):**

- `src/crosshook-native/src/types/discovery.ts` — add Phase B TypeScript interfaces
- `src/crosshook-native/src/hooks/useExternalTrainerSearch.ts` — new hook wrapping `discovery_search_external`
- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` — integrate external results section, "Search Online" button, trust badges, offline banner, cache age indicator

**Details:**

- **TypeScript types** to add in `discovery.ts`:

  ```typescript
  export interface ExternalTrainerSearchQuery {
    gameName: string;
    steamAppId?: string;
  }
  export interface ExternalTrainerResult {
    gameName: string;
    sourceName: string;
    sourceUrl: string;
    pubDate?: string;
    source: string;
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

- **`useExternalTrainerSearch` hook**: Follows `useProtonDbSuggestions.ts` pattern exactly — `requestIdRef.current` increment for stale cancellation, `useState<ExternalTrainerSearchResponse | null>`, return `{ data, loading, error, refresh }`. Does NOT debounce — external search is triggered manually by "Search Online" button or on game context change (not on every keystroke).
- **`TrainerDiscoveryPanel.tsx` additions**:
  - "Search Online" button (visible only when `settings.discovery_enabled === true` and a non-empty query is present)
  - External results section (below local results) with spinner while `useExternalTrainerSearch.loading` is true — does NOT block local results display
  - Trust badges: `crosshook-discovery-badge--community` (local tap results) and `crosshook-discovery-badge--external` (FLiNG RSS results)
  - Offline banner: persistent (not dismissible) when `data.offline === true`
  - Cache age indicator: shown when `data.cached === true`, e.g. "Results from cache, 23 min ago"
  - External result cards use the same `TrainerResultCard` component — `result.sourceUrl` opens via `shellOpen()` (never `<a href>`)
- **Scroll container registration**: Any new `overflow-y: auto` container added in the external results section must be added to `SCROLLABLE` in `src/crosshook-native/src/hooks/useScrollEnhance.ts`. Add `overscroll-behavior: contain` to inner scroll containers.
- **CSS variables**: Add badge and banner CSS custom properties to `src/crosshook-native/src/styles/variables.css`.

---

#### B6 — Phase B Unit Tests (depends on B2 + B3; parallel with B5)

**Files (2):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/client.rs` — `#[cfg(test)]` module
- `src/crosshook-native/crates/crosshook-core/src/discovery/matching.rs` — `#[cfg(test)]` module

**Test cases — `client.rs`:**

- Cache hit returns result without HTTP fetch (inject pre-populated `MetadataStore::open_in_memory()`)
- Cache miss triggers live path (mock the HTTP layer or test the cache-only path)
- Stale fallback: when cache entry is expired and HTTP fails, return `is_stale: true`
- Oversized payload: truncation to 200 items before serialization stays under 512 KiB
- `MetadataStore::disabled()` path: skips cache, returns offline result gracefully
- Cache key namespace: verify stored key matches `"trainer:source:v1:fling_rss_index"` pattern

**Test cases — `matching.rs`:**

- `strip_trainer_suffix`: `"Elden Ring Trainer"` → `"Elden Ring"`, `"Elden Ring v1.12 +DLC Trainer"` → `"Elden Ring"`, already-stripped title unchanged
- `score_fling_result`: exact match → `1.0`; partial match returns `> 0.0`; unrelated title returns `< 0.1`
- `check_version_advisory`: matching version strings → `VersionMatchStatus::Exact`; mismatch → `VersionMatchStatus::Outdated` or `NewerAvailable`; `None` input → `VersionMatchStatus::Unknown`
- Token scoring edge cases: empty string, single-character tokens (filtered out), non-ASCII input

**Test infrastructure**: All tests use `#[cfg(test)]` inline modules. Rust tests only — no Jest/Vitest. Use `MetadataStore::open_in_memory()` + `run_migrations()` for any store-dependent tests. Pure function tests in `matching.rs` need no store at all.

---

### Phase C: FTS5 Search Optimization (Deferred)

Gated on community tap ecosystem reaching ~1000 profiles. Requires switching `rusqlite` feature from `bundled` to `bundled-full` in `Cargo.toml`. Separate issue; do not implement in Phase A or B.

---

## Task Granularity Recommendations

- Each task touches 1–3 files maximum
- Foundation tasks (A1, A2, B1) are the only true serial blockers before parallel work can begin
- **Phase B respects the same Foundation → Core Logic → IPC → Frontend → Tests pattern as Phase A**
- B2 and B3 are split into separate tasks because `client.rs` (async HTTP, cache integration) and `matching.rs` (pure synchronous functions) have entirely different complexity profiles and can be developed independently by different implementers
- B5 is split from B6 because frontend work (TypeScript, React) and Rust unit tests have no shared implementation dependency once B4's IPC is registered

---

## Dependency Analysis

```
A1 (schema migration)
  └── A2 (discovery models) ──┐
        ├── A3 (search.rs)    │  [A3 and A4 are parallel]
        └── A4 (tap indexer)  │
              └─────A5 (MetadataStore facade + IPC commands)
                        ├── A6 (TS types + hook)       ─┐
                        ├── A7 (TrainerDiscoveryPanel)   ├── all parallel
                        └── A8 (unit tests)             ─┘

Phase B (all gates on Phase A complete):
  B1 (Cargo.toml + Phase B models)
    ├── B2 (HTTP client / client.rs)  ─┐  [B2 and B3 are parallel]
    └── B3 (matching.rs)              ─┤
              └── B4 (MetadataStore Phase B methods + async IPC)
                    ├── B5 (frontend: types + hook + panel)  ─┐ [parallel]
                    └── B6 (Phase B unit tests)               ─┘
```

**Key blocking relationships (Phase B):**

- B1 blocks B2 and B3: `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, and `DiscoveryCacheState` types must exist before `client.rs` can compile. `matching.rs` scoring functions return `ExternalTrainerResult`-shaped data. Both are also blocked on `quick-xml` being in `Cargo.toml`.
- B2 and B3 both block B4: The IPC command `discovery_search_external` delegates to `discovery::search_external()` which calls `client.rs` fetch + `matching.rs` scoring pipeline. Neither command can be implemented until both B2 and B3 are stable.
- B4 blocks B5: The frontend hook `useExternalTrainerSearch` invokes `discovery_search_external` — the hook cannot be tested or the panel cannot integrate it until the IPC command is registered.
- B4 blocks B6 (partially): Cache-interaction tests in `client.rs` need the `MetadataStore` facade methods from B4. Pure function tests in `matching.rs` are independent and can be written alongside B3.

**Non-blocking relationships (Phase B):**

- B3 (`matching.rs`) pure function tests can be written as part of B3 itself, before B4 ships.
- B5 (frontend TypeScript types) can be drafted in parallel with B1 — TypeScript interfaces mirror the Rust structs being defined in B1.
- B6 `matching.rs` tests are fully independent of B4 and B5 — they test pure functions only.

---

## File-to-Task Mapping

### Phase A

| File                                             | Task                | Phase |
| ------------------------------------------------ | ------------------- | ----- |
| `crosshook-core/src/metadata/migrations.rs`      | A1                  | A     |
| `crosshook-core/src/metadata/models.rs`          | A1                  | A     |
| `crosshook-core/src/discovery/mod.rs`            | A2                  | A     |
| `crosshook-core/src/discovery/models.rs`         | A2 (extended in B1) | A / B |
| `crosshook-core/src/lib.rs`                      | A2                  | A     |
| `crosshook-core/src/discovery/search.rs`         | A3                  | A     |
| `crosshook-core/src/community/index.rs`          | A4                  | A     |
| `crosshook-core/src/metadata/community_index.rs` | A4                  | A     |
| `crosshook-core/src/metadata/mod.rs`             | A5 (extended in B4) | A / B |
| `src-tauri/src/commands/discovery.rs`            | A5 (extended in B4) | A / B |
| `src-tauri/src/commands/mod.rs`                  | A5                  | A     |
| `src-tauri/src/lib.rs`                           | A5 (extended in B4) | A / B |
| `src/types/discovery.ts`                         | A6 (extended in B5) | A / B |
| `src/types/index.ts`                             | A6                  | A     |
| `src/hooks/useTrainerDiscovery.ts`               | A6                  | A     |
| `src/hooks/useScrollEnhance.ts`                  | A7 (extended in B5) | A / B |
| `src/components/TrainerDiscoveryPanel.tsx`       | A7 (extended in B5) | A / B |

### Phase B (new files)

| File                                                      | Task | Phase |
| --------------------------------------------------------- | ---- | ----- |
| `crosshook-core/Cargo.toml`                               | B1   | B     |
| `crosshook-core/src/discovery/models.rs` (additions)      | B1   | B     |
| `crosshook-core/src/discovery/client.rs`                  | B2   | B     |
| `crosshook-core/src/text_utils.rs`                        | B3   | B     |
| `crosshook-core/src/lib.rs` (text_utils mod registration) | B3   | B     |
| `crosshook-core/src/install/discovery.rs` (use re-export) | B3   | B     |
| `crosshook-core/src/discovery/matching.rs`                | B3   | B     |
| `crosshook-core/src/metadata/mod.rs` (additions)          | B4   | B     |
| `src-tauri/src/commands/discovery.rs` (additions)         | B4   | B     |
| `src-tauri/src/lib.rs` (additions)                        | B4   | B     |
| `src/types/discovery.ts` (additions)                      | B5   | B     |
| `src/hooks/useExternalTrainerSearch.ts`                   | B5   | B     |
| `src/components/TrainerDiscoveryPanel.tsx` (additions)    | B5   | B     |
| `src/styles/variables.css` (additions)                    | B5   | B     |

---

## Optimization Opportunities

### Maximum Parallelism Points

1. **A3 ∥ A4**: Search logic and tap indexer extension are independent; both only need A1+A2 complete.
2. **A6 ∥ A7 ∥ A8**: All three can proceed simultaneously once A5 ships. This is the widest fan-out in Phase A.
3. **B2 ∥ B3**: HTTP client and token matching have no shared state; B2 is async/IO-heavy while B3 is pure functions. Two implementers with completely different work surfaces.
4. **B5 ∥ B6**: Frontend integration and Phase B tests share no implementation dependency once B4 ships.
5. **B3 tests alongside B3**: `matching.rs` pure function tests can be written as part of B3 (no store dependency) rather than waiting for B6. This reduces B6's scope.
6. **TypeScript type drafting alongside B1**: `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery` TypeScript interfaces mirror the Rust structs being defined in B1 — can be drafted in parallel.

### Latency Reduction Insight

The Phase B critical path is: B1 → B2 → B4 → B5. B3 is off the critical path (it gates B4 but B2 typically takes longer). Start B2 immediately after B1 ships and keep B3 on a separate implementer track. B4 can begin integrating B3's scoring functions as soon as B3's API is defined, even before all B3 tests pass.

---

## Implementation Strategy Recommendations

### Phase A

1. **Start A1 immediately** — the migration is the only hard sequential gate. The v17→v18 SQL is fully specified in the feature spec.

2. **Develop A2 in parallel with A1** — the Rust models for the new table and IPC boundary types are fully specified and have no migration dependency beyond knowing the column names (which are fixed in the spec).

3. **Assign A3 and A4 to separate implementers** — these are the first true parallelization opportunity. `search.rs` is pure query logic; `community/index.rs` + `community_index.rs` is indexer plumbing. Neither touches the other's files.

4. **Use `MetadataStore::open_in_memory()` for all Rust tests** — the in-memory SQLite instance supports full migration runs, making A8 integration-quality tests possible without a real database file.

5. **IPC contract test block is mandatory for A5** — every `commands/*.rs` file must end with a `#[cfg(test)]` block casting each handler to its explicit function-pointer type. See `commands/community.rs:311–353`. Missing this will break the compile-time IPC validation convention.

6. **Register discovery scroll container in `useScrollEnhance.ts` during A7** — any `overflow-y: auto` container in `TrainerDiscoveryPanel` must be added to the `SCROLLABLE` const in `useScrollEnhance.ts`. Failure causes dual-scroll jank (WebKitGTK constraint).

7. **Gate Phase B on community demand** — do not begin B2 (HTTP client) until Phase A is validated with real community tap data. The external API integration (FLiNG RSS) needs live verification before investing in the client implementation.

### Phase B

1. **B1 first, always** — `quick-xml` must be in `Cargo.toml` before any Phase B Rust code compiles. The Phase B types in `models.rs` are the shared contract between B2 and B3. Both will hit compile errors without B1 complete.

2. **Clone `protondb/client.rs` as the starting point for B2** — don't write `client.rs` from scratch. Copy the ProtonDB client file, rename constants, types, and the static `OnceLock`, then adapt the cache key and HTTP endpoint. This is explicitly the intended approach per `shared.md`.

3. **Create `text_utils.rs` first within B3** — it is the smallest file in Phase B (~20 lines). Lift `tokenize`/`token_hits` to `crosshook-core/src/text_utils.rs` as `pub(crate)` and update `install/discovery.rs` to re-use via `use crate::text_utils::{tokenize, token_hits};`. This eliminates duplication without a cross-domain `discovery/ → install/` dependency (a resolved decision from `research-practices.md`). `matching.rs` then imports from `crate::text_utils` — keeping `matching.rs` itself zero-I/O and fully parallelizable with B2.

4. **B4 is two changes, one task** — the MetadataStore facade method additions and the IPC command additions are tightly coupled (the IPC command calls the facade method). Keep them in one task to avoid a half-wired state where `client.rs` is callable from Rust but not yet from the frontend.

5. **Write B2 tests against the cache layer, not HTTP** — use `MetadataStore::open_in_memory()` to pre-populate `external_cache_entries` and verify the cache-hit, cache-miss, and stale-fallback paths without making live HTTP requests. The live HTTP path is verified manually during integration testing.

6. **Update IPC contract test block in B4** — extend (not replace) the `#[cfg(test)]` block in `commands/discovery.rs` to include function-pointer casts for `discovery_search_external` and `discovery_check_version_compatibility`. This block verifies that Rust function signatures match what was registered in `generate_handler![]`.

7. **Never implement Phase C (FTS5) without changing `rusqlite` features** — `bundled` feature does not include FTS5. Any attempt to use FTS5 queries against the current build will silently fail or error at runtime. Track this as a separate issue requiring `bundled-full` feature flag change.
