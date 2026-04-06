# Task Structure Analysis: trainer-discovery

## Executive Summary

Phase A (MVP) introduces a new `trainer_sources` table (v17→v18 migration), a `discovery/` module in `crosshook-core`, a thin IPC command file, and a `TrainerDiscoveryPanel` React component. All foundation tasks (schema, models, tap indexer extension) are serialized; the UI and tests can proceed in parallel once the IPC layer is complete. Phase B adds external HTTP lookup following the ProtonDB client pattern and is gated on Phase A shipping. Zero new crate dependencies are required in either phase.

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

All Phase B tasks depend on Phase A shipping. Within Phase B, B1 is the foundation for B2, B3, B4. B5 and B6 are independent once B4 is ready.

#### B1 — External Source Models (BLOCKER for B2, B3)

**Files (1):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` — extend with `ExternalTrainerSource`, `ExternalSearchResult`, `TrainerSourceCacheState`

#### B2 — External HTTP Client (depends on B1)

**Files (1):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/client.rs` — `OnceLock<reqwest::Client>`, cache-first fetch, stale fallback; cache key: `trainer:source:v1:{normalized_game_name}`; reuse `external_cache_entries` via `cache_store.rs`

#### B3 — Token Matching Pure Functions (depends on B1; independent of B2)

**Files (1):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/matching.rs` — port `tokenize()`/`token_hits()` from `install/discovery.rs`; pure functions, no I/O; advisory version string comparison (no semver crate)

#### B4 — Aggregation + Async IPC (depends on B2, B3)

**Files (2):**

- `src/crosshook-native/src-tauri/src/commands/discovery.rs` — add `discovery_search_external` async IPC command following `commands/protondb.rs` async pattern (`.inner().clone()` for moving state across await); add `discovery_check_version_compatibility` on-demand IPC command
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — expose external search method on `MetadataStore`

#### B5 — Frontend Integration (depends on B4; can run alongside B6)

**Files (2):**

- `src/crosshook-native/src/hooks/useTrainerDiscovery.ts` — extend to merge tap + external results progressively
- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` — add trust indicators (Community vs External badges), offline degraded mode with persistent banner, "Retry" CTA

#### B6 — Phase B Tests (depends on B3 + B4; can run alongside B5)

**Files (1):**

- `src/crosshook-native/crates/crosshook-core/src/discovery/` — `#[cfg(test)]` blocks in `client.rs` and `matching.rs`

---

### Phase C: FTS5 Search Optimization (Deferred)

Gated on community tap ecosystem reaching ~1000 profiles. Requires switching `rusqlite` feature from `bundled` to `bundled-full` in `Cargo.toml`. Separate issue; do not implement in Phase A or B.

---

## Task Granularity Recommendations

- Each task touches 1–3 files maximum
- Foundation tasks (A1, A2) are the only true serial blockers before any parallel work can begin
- A3 and A4 are independent of each other and can run in parallel (both depend on A1+A2)
- A6, A7, A8 are fully independent of each other and can all run in parallel once A5 is complete
- A6 (types + hook) and A7 (component) are split to allow type/IPC work to unblock component work immediately
- Tests (A8) are separated from business logic (A3) to allow parallel implementation

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
  B1 (models)
    ├── B2 (HTTP client)  ─┐  [B2 and B3 are parallel]
    └── B3 (matching.rs)  ─┤
              └── B4 (aggregation + async IPC)
                    ├── B5 (frontend integration)  ─┐ [parallel]
                    └── B6 (Phase B tests)          ─┘
```

**Key blocking relationships:**

- A1 blocks A2 (models need table schema finalized)
- A2 blocks A3 and A4 (indexer and search need manifest/row types)
- A3 + A4 both block A5 (facade exposes search; IPC commands call it)
- A5 blocks A6, A7, A8 (frontend and tests need the IPC endpoint to exist)

**Non-blocking relationships:**

- A8 (unit tests for search.rs) can start as soon as A3 is complete — does not need A6/A7
- B3 (matching.rs pure functions) is fully independent of B2 (HTTP client) — can be developed and tested in isolation

---

## File-to-Task Mapping

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
| `src-tauri/src/lib.rs`                           | A5                  | A     |
| `src/types/discovery.ts`                         | A6                  | A     |
| `src/types/index.ts`                             | A6                  | A     |
| `src/hooks/useTrainerDiscovery.ts`               | A6 (extended in B5) | A / B |
| `src/hooks/useScrollEnhance.ts`                  | A7                  | A     |
| `src/components/TrainerDiscoveryPanel.tsx`       | A7 (extended in B5) | A / B |
| `crosshook-core/src/discovery/client.rs`         | B2                  | B     |
| `crosshook-core/src/discovery/matching.rs`       | B3                  | B     |

---

## Optimization Opportunities

### Maximum Parallelism Points

1. **A3 ∥ A4**: Search logic and tap indexer extension are independent; both only need A1+A2 complete.
2. **A6 ∥ A7 ∥ A8**: All three can proceed simultaneously once A5 ships. This is the widest fan-out in Phase A.
3. **B2 ∥ B3**: HTTP client and token matching are pure-function isolated; no shared state.
4. **B5 ∥ B6**: Frontend integration and Phase B tests share no implementation dependency.

### Quick Win (independent of task sequence)

The `CommunityBrowser.tsx` component already displays community profile metadata including `trainer_name`, `trainer_version`, and `trainer_sha256`. Surfacing these more prominently in the profile detail view is a UI-only change that can ship before Phase A is complete, requiring no schema or IPC changes.

---

## Implementation Strategy Recommendations

1. **Start A1 immediately** — the migration is the only hard sequential gate. The v17→v18 SQL is fully specified in the feature spec.

2. **Develop A2 in parallel with A1** — the Rust models for the new table and IPC boundary types are fully specified and have no migration dependency beyond knowing the column names (which are fixed in the spec).

3. **Assign A3 and A4 to separate implementers** — these are the first true parallelization opportunity. `search.rs` is pure query logic; `community/index.rs` + `community_index.rs` is indexer plumbing. Neither touches the other's files.

4. **Use `MetadataStore::open_in_memory()` for all Rust tests** — the in-memory SQLite instance supports full migration runs, making A8 integration-quality tests possible without a real database file.

5. **IPC contract test block is mandatory for A5** — every `commands/*.rs` file must end with a `#[cfg(test)]` block casting each handler to its explicit function-pointer type. See `commands/community.rs:311–353`. Missing this will break the compile-time IPC validation convention.

6. **Register discovery scroll container in `useScrollEnhance.ts` during A7** — any `overflow-y: auto` container in `TrainerDiscoveryPanel` must be added to the `SCROLLABLE` const in `useScrollEnhance.ts`. Failure causes dual-scroll jank (WebKitGTK constraint).

7. **Gate Phase B on community demand** — do not begin B2 (HTTP client) until Phase A is validated with real community tap data. The external API integration (FLiNG RSS) needs live verification before investing in the client implementation.

8. **Never implement Phase C (FTS5) without changing `rusqlite` features** — `bundled` feature does not include FTS5. Any attempt to use FTS5 queries against the current build will silently fail or error at runtime. Track this as a separate issue requiring `bundled-full` feature flag change.
