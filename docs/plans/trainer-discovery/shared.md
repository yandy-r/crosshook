# Trainer Discovery Phase B — External Source Lookup

Phase B extends the fully-implemented Phase A trainer discovery (LIKE search over `trainer_sources`, sync IPC, `TrainerDiscoveryPanel`) with an async FLiNG RSS HTTP client (`discovery/client.rs`) following the `protondb/client.rs` OnceLock singleton + cache→live→stale-fallback pattern, token-based relevance scoring (`discovery/matching.rs` adapting `install/discovery.rs:tokenize()`), advisory version matching via `version_store.rs`, and progressive frontend loading where local tap results render immediately while external results load asynchronously. All cache writes use the existing `external_cache_entries` table with a `trainer:source:v1:{key}` namespace — no new migration needed (schema stays at v18). The only new crate dependency is `quick-xml` for RSS parsing.

> **Planning vs code:** Phase A code is in-tree and working. This document describes Phase B design. `feature-spec.md` Decisions section (line 634+) is authoritative when conflicts arise.

## Relevant Files

### Rust — crosshook-core (business logic)

- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs`: Phase A module root; Phase B adds `pub mod client; pub mod matching;` and re-exports
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs`: Phase A types + Phase B additions (`ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery`, `DiscoveryCacheState`)
- `src/crosshook-native/crates/crosshook-core/src/discovery/search.rs`: Phase A LIKE search; untouched by Phase B
- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: **PRIMARY Phase B reference** — OnceLock HTTP singleton (line 26), cache-first flow (lines 85–130), stale fallback (line 111), `persist_lookup_result` (line 318), `load_cached_lookup_row` (line 346)
- `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: Serde conventions — `#[serde(rename_all = "camelCase")]` on structs, `#[serde(rename_all = "snake_case")]` on enums, cache key namespace pattern
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry()`, `put_cache_entry()`, `evict_expired_cache_entries()` — Phase B cache read/write API
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `compute_correlation_status()` (line 185, pure fn) and `lookup_latest_version_snapshot()` (line 75) — Phase B version check
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` facade — `with_conn`/`with_sqlite_conn` accessors; add Phase B cache and version-check facade methods
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: `external_cache_entries` schema (v4, line 264); `trainer_sources` (v18, line 782) — no new migration for Phase B
- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: `tokenize()` (line 292) and `token_hits()` (line 272) — scoring primitives for `matching.rs`
- `src/crosshook-native/crates/crosshook-core/Cargo.toml`: Add `quick-xml` for RSS parsing; `reqwest` already has `json` + `rustls-tls`
- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: `pub mod discovery;` already declared (Phase A)

### Tauri IPC Layer

- `src/crosshook-native/src-tauri/src/commands/discovery.rs`: Phase A sync `discovery_search_trainers`; Phase B adds async `discovery_search_external` + sync `discovery_check_version_compatibility`
- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: **Async IPC reference** — `.inner().clone()` before `await` (line 55), `map_err(|e| e.to_string())` at boundary
- `src/crosshook-native/src-tauri/src/commands/mod.rs`: `pub mod discovery;` already declared (Phase A)
- `src/crosshook-native/src-tauri/src/lib.rs`: `tauri::generate_handler![]` — register Phase B commands in the `// Trainer discovery` block (line ~318)

### Frontend — React/TypeScript

- `src/crosshook-native/src/hooks/useTrainerDiscovery.ts`: Phase A hook (untouched by Phase B) — debounced local search
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`: **Hook reference** — `requestIdRef` race guard (line 37), cache state from payload, `{ data, loading, error, refresh }` shape
- `src/crosshook-native/src/types/discovery.ts`: Phase A types; Phase B adds `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery`
- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`: Phase A panel; Phase B adds "Search Online" button, external results section, trust badges, offline banner
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: **CRITICAL** — register any new scroll containers in `SCROLLABLE` selector
- `src/crosshook-native/src/styles/variables.css`: CSS custom properties for badges/banners

## Relevant Tables

- **`external_cache_entries`** (existing, v4): Phase B cache target — keys `trainer:source:v1:{key}`; 512 KiB payload cap; TTL via `expires_at`; `ON CONFLICT(cache_key) DO UPDATE SET` upsert
- **`version_snapshots`** (existing, v9): Phase B version check — `lookup_latest_version_snapshot(profile_id)` returns latest `steam_build_id`, `human_game_ver`, `trainer_version`
- **`trainer_sources`** (existing, v18): Phase A search target — Phase B reads but does not modify; external results are NOT written here
- **`community_taps`** (existing): JOIN for tap URL/local_path in search; unchanged

## Relevant Patterns

**OnceLock HTTP Client Singleton**: Each HTTP client domain gets its own `static OnceLock<reqwest::Client>`. Builder sets timeout + `CrossHook/{version}` user-agent. Lazy init, race-safe. See `protondb/client.rs:26,175-190`.

**Cache-First Fetch (3-stage)**: (1) Check `external_cache_entries` for valid row → return on hit, (2) HTTP fetch live → parse → persist → return, (3) On error: load stale row (allow_expired=true) → return with `is_stale=true`, (4) No row at all → return `Unavailable`. See `protondb/client.rs:85-130`.

**Domain Error Types**: Private `enum DiscoveryError` with `Network(reqwest::Error)`, `ParseError(String)`, etc. Manual `fmt::Display` impl. Never exposed at IPC boundary. See `protondb/client.rs:29-53`.

**Async IPC Command**: `pub async fn`, `metadata_store.inner().clone()` before first `.await`, `map_err(|e| e.to_string())`. See `commands/protondb.rs:49-57`.

**IPC Contract Test**: `#[cfg(test)]` function-pointer cast block in every commands file. Compile-time signature verification. See `commands/discovery.rs:19-31`.

**Token Scoring**: `tokenize()` splits on non-alphanumeric, lowercases, filters ≥2 chars; `token_hits()` counts substring matches. See `install/discovery.rs:272-303`.

**MetadataStore Facade**: `with_conn` (returns `T::default()` when unavailable) for optional reads; `with_sqlite_conn` (returns `Err`) when unavailability must be surfaced. See `metadata/mod.rs:98-160`.

**Serde IPC Boundary**: `#[serde(rename_all = "camelCase")]` on result structs; `#[serde(rename_all = "snake_case")]` on state enums; `#[serde(default, skip_serializing_if = "Option::is_none")]` on optional fields. See `discovery/models.rs`.

**Frontend Hook Race Guard**: `requestIdRef.current` increment before invoke; discard responses where ref has advanced. See `useTrainerDiscovery.ts:38-51`.

## Relevant Docs

**docs/plans/trainer-discovery/feature-spec.md**: You _must_ read this for Phase B task list (lines 602–616), Decision 3 (FLiNG RSS only, line 640), IPC command signatures (lines 389–401), external dependency details (lines 23–53), and security findings (lines 555–577).

**docs/plans/trainer-discovery/research-external.md**: You _must_ read this for FLiNG RSS endpoint details, HTTP client code patterns, cache key conventions, and three-stage fetch implementation. All Phase B network code derives from this.

**docs/plans/trainer-discovery/research-security.md**: You _must_ read this for S3 (cache poisoning mitigation), S5 (URL rendering in WebKitGTK), S9 (SHA-256 integration), and S10 (trust indicators for external results).

**docs/plans/trainer-discovery/research-practices.md**: You _must_ read this for resolved design decisions: FTS5 unavailable, IPC command split, `tokenize()` lifting plan, testability patterns.

**docs/plans/trainer-discovery/research-ux.md**: You _must_ read this for trust badge design, version badge two-stage render, offline banner patterns, progressive loading, and existing component reuse.

**AGENTS.md**: You _must_ read this for architecture rules (business logic in crosshook-core), Tauri IPC conventions (snake_case, Serde), scroll container rules, and persistence classification.

**docs/plans/trainer-discovery/research-architecture.md**: Reference for Phase B data flow diagrams, cache infrastructure details, and version store integration.

**docs/plans/trainer-discovery/research-patterns.md**: Reference for detailed code pattern examples with line numbers.

**docs/plans/trainer-discovery/research-integration.md**: Reference for FLiNG RSS XML structure, PCGamingWiki API format, external_cache_entries schema, and frontend integration plan.
