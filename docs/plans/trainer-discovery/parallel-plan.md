# Trainer Discovery Implementation Plan

Trainer discovery adds a searchable index of game trainer sources from community taps, surfaced in a new `TrainerDiscoveryPanel` component. Phase A (MVP) creates a new `trainer_sources` SQLite table (v17→v18 migration) populated from `trainer-sources.json` files discovered during tap sync, queries it with LIKE-based SQL search, and exposes results via a sync IPC command + React hook. Phase B adds external HTTP lookup (FLiNG RSS) following the `protondb/client.rs` cache-first pattern and advisory version matching. Zero new crate dependencies are required for either phase. The primary ship blocker is legal (DMCA §1201 opt-in consent dialog), not technical.

## Critically Relevant Files and Documentation

- `docs/plans/trainer-discovery/feature-spec.md`: Authoritative feature spec — resolved decisions, exact data models, IPC signatures, SQL queries, file manifest. **Start here.**
- `docs/plans/trainer-discovery/research-security.md`: Security findings (5 WARNING, 5 ADVISORY). S1 (DMCA) is a hard ship blocker.
- `docs/plans/trainer-discovery/research-ux.md`: UI workflow, component patterns, accessibility requirements, 300ms debounce, 500ms search SLA.
- `AGENTS.md`: Normative architecture rules, Tauri IPC conventions, scroll container requirements, persistence classification.
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Schema migration chain (v0→v17); add v18 here.
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs`: Transactional DELETE+INSERT indexer, A6 field bounds, watermark skip — template for `index_trainer_sources()`.
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: MetadataStore facade — `with_conn`/`with_conn_mut`/`with_sqlite_conn` accessors.
- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: HTTP client singleton pattern (`OnceLock`), cache→live→stale-fallback flow — clone for Phase B.
- `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: Serde conventions for IPC boundary types.
- `src/crosshook-native/src-tauri/src/commands/community.rs`: Sync IPC command reference + mandatory `#[cfg(test)]` contract test block (lines 311–353).
- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: Async IPC reference — `.inner().clone()` for await boundary.
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`: Frontend hook reference — `requestIdRef.current` race guard, `{ data, loading, error, refresh }` return shape.
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: CRITICAL — register new scroll containers in `SCROLLABLE` selector.
- `src/crosshook-native/src/components/CommunityBrowser.tsx`: Component reference for result cards and client-side filtering.

## Implementation Plan

### Phase 1: Foundation (Schema + Models)

#### Task 1.1: Schema Migration v17→v18 — CREATE TABLE trainer_sources

Depends on [none]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `docs/plans/trainer-discovery/feature-spec.md` (lines 219–238 for exact CREATE TABLE SQL)

**Instructions**

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`

Add `migrate_17_to_18()` function following the existing migration waterfall pattern (`if version < 18 { ... }`). The migration creates the `trainer_sources` table and two indexes:

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

Also execute `UPDATE community_taps SET last_head_commit = NULL;` to force re-index on next sync so tap manifests containing `trainer-sources.json` files get indexed.

Set `user_version` to 18 via `conn.pragma_update(None, "user_version", 18_u32)`.

Add a `#[test] fn migration_17_to_18_creates_trainer_sources_table()` test using `db::open_in_memory()` + `run_migrations()` — verify table exists via `sqlite_master` query. Follow `migration_16_to_17_creates_suggestion_dismissals_table` as the direct template.

#### Task 1.2: Discovery Domain Models

Depends on [none]

**READ THESE BEFORE TASK**

- `docs/plans/trainer-discovery/feature-spec.md` (lines 193–301 for exact struct definitions)
- `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs` (Serde conventions)
- `src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs` (module layout reference)

**Instructions**

Files to Create

- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs`

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/lib.rs` — add `pub mod discovery;`

Create the `discovery/` module directory mirroring `protondb/` layout.

`discovery/mod.rs`: Re-export public types from `models.rs` and (later) `search.rs`.

`discovery/models.rs`: Define these types per feature-spec:

- `TrainerSourcesManifest` — deserializes `trainer-sources.json` files from taps. Contains `schema_version: u32`, `game_name: String`, `steam_app_id: Option<u32>`, `sources: Vec<TrainerSourceEntry>`.
- `TrainerSourceEntry` — individual source: `source_name`, `source_url`, `trainer_version?`, `game_version?`, `notes?`, `sha256?`.
- `TrainerSourceRow` — maps to `trainer_sources` DB columns (id, tap_id, game_name, steam_app_id, source_name, source_url, trainer_version, game_version, notes, sha256, relative_path, created_at, tap_url from JOIN).
- `TrainerSearchQuery` — `query: String`, `compatibility_filter: Option<String>`, `platform_filter: Option<String>`, `limit: Option<u32>`, `offset: Option<u32>`. `#[serde(rename_all = "camelCase")]`.
- `TrainerSearchResult` — **Phase A scope**: shaped from `TrainerSourceRow` + JOIN data only. Fields: `id: i64`, `game_name: String`, `steam_app_id: Option<u32>`, `source_name: String`, `source_url: String`, `trainer_version: Option<String>`, `game_version: Option<String>`, `notes: Option<String>`, `sha256: Option<String>`, `relative_path: String`, `tap_url: String`, `tap_local_path: String`, `relevance_score: f64`. `#[serde(rename_all = "camelCase")]`. Note: the feature-spec `TrainerSearchResult` includes `community_profiles` fields (`community_profile_id`, `trainer_name`, `compatibility_rating`, etc.) — those are Phase B additions when results are merged with community profile data.
- `TrainerSearchResponse` — `results: Vec<TrainerSearchResult>`, `total_count: i64`. `#[serde(rename_all = "camelCase")]`.
- `VersionMatchStatus` — enum: `Exact`, `Compatible`, `NewerAvailable`, `Outdated`, `Unknown`. `#[serde(rename_all = "snake_case")]` with `#[default]` on `Unknown`. (Phase B — define in models.rs now for forward compatibility but not used in Phase A.)
- `VersionMatchResult` — `status`, `trainer_game_version`, `installed_game_version`, `detail?`. (Phase B.)

Apply Serde conventions: `#[serde(rename_all = "camelCase")]` on result structs, `#[serde(default, skip_serializing_if = "Option::is_none")]` on optional fields.

Register `pub mod discovery;` in `lib.rs` alongside existing domain modules.

### Phase 2: Core Logic (Search + Indexer)

#### Task 2.1: LIKE-Based Search Query

Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- `docs/plans/trainer-discovery/feature-spec.md` (lines 365–381 for exact SQL)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` (query patterns, `nullable_text` helper)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (MetadataStore facade)

**Instructions**

Files to Create

- `src/crosshook-native/crates/crosshook-core/src/discovery/search.rs`

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs` — add `mod search;` and re-export `search_trainer_sources`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — add `pub fn search_trainer_sources(...)` method on `MetadataStore`

Create `discovery/search.rs` with a `search_trainer_sources(conn: &Connection, query: &str, limit: i64, offset: i64) -> Result<TrainerSearchResponse, MetadataStoreError>` free function.

SQL query (extended from feature-spec to include `ct.local_path` for Import CTA path resolution):

```sql
SELECT ts.id, ts.game_name, ts.steam_app_id, ts.source_name,
       ts.source_url, ts.trainer_version, ts.game_version,
       ts.notes, ts.sha256, ts.relative_path,
       ct.tap_url, ct.local_path, 0.0 AS relevance_score
FROM trainer_sources ts
JOIN community_taps ct ON ts.tap_id = ct.tap_id
WHERE (ts.game_name LIKE '%' || ?1 || '%'
    OR ts.source_name LIKE '%' || ?1 || '%'
    OR ts.notes LIKE '%' || ?1 || '%')
ORDER BY ts.game_name, ts.source_name
LIMIT ?2 OFFSET ?3
```

Note: `ct.local_path` is the tap workspace root on disk. The Import CTA in the frontend constructs the full manifest path as `${tapLocalPath}/${relativePath}/community-profile.json` (if the associated profile exists). Without `local_path` in the search result, the Import CTA cannot resolve the filesystem path needed by `community_import_profile`.

Validate: empty query returns error `"search query cannot be empty"`. Trim query, cap at 512 bytes before binding. Maximum limit: 50 (enforce via `std::cmp::min(limit, 50)`).

Add `MetadataStore::search_trainer_sources()` public method using `self.with_conn("search trainer sources", |conn| ...)` — returns empty `Vec` when store is disabled (never panic).

Add a minimal `#[cfg(test)]` module with 2 compile-check tests only (Task 4.3 writes the full test suite):

- `search_returns_error_for_empty_query`
- `search_returns_empty_for_no_matches`

#### Task 2.2: Tap Indexer Extension — Walk for trainer-sources.json

Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/community/index.rs` — existing tap directory walker
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — transactional DELETE+INSERT, A6 bounds, watermark skip
- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs` — `validate_tap_url()` for HTTPS-only pattern
- `docs/plans/trainer-discovery/feature-spec.md` (lines 193–216 for manifest schema)

**Instructions**

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/community/index.rs` — extend tap directory walker to discover `trainer-sources.json` files alongside `community-profile.json`
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — add `index_trainer_sources()` free function

In `community/index.rs`: When walking tap workspace directories, look for `trainer-sources.json` files. Parse them as `TrainerSourcesManifest`. Parse failures are logged as diagnostics (`tracing::warn!`) and skipped, not errors — mirrors `community-profile.json` failure handling.

In `metadata/community_index.rs`: Add `index_trainer_sources()` following the `index_community_tap_result()` transactional pattern:

1. Watermark skip: reuse the same `last_head_commit` check (tap-level, shared with profiles)
2. Open `Transaction::new(conn, TransactionBehavior::Immediate)`
3. `DELETE FROM trainer_sources WHERE tap_id = ?1`
4. For each `TrainerSourceEntry`: validate A6 field bounds + HTTPS-only URL (`source_url`), then INSERT
5. `tx.commit()`

Add A6 bound constants: `MAX_SOURCE_URL_BYTES: usize = 2_048`, `MAX_SOURCE_NAME_BYTES: usize = 512`, `MAX_NOTES_BYTES: usize = 4_096`.

URL validation: reject any `source_url` that does not start with `https://`. Log rejected entries with `tracing::warn!`.

Add 2 minimal compile-check tests only (Task 4.3 writes the full test suite):

- `index_trainer_sources_inserts_entries`
- `index_trainer_sources_rejects_http_url`

### Phase 3: IPC Integration

#### Task 3.1: IPC Command File + Registration

Depends on [2.1]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src-tauri/src/commands/community.rs` (sync command pattern + contract test block at lines 311–353)
- `src/crosshook-native/src-tauri/src/lib.rs` (`tauri::generate_handler![]` macro at line ~209)
- `src/crosshook-native/src-tauri/src/commands/mod.rs` (module registry)

**Instructions**

Files to Create

- `src/crosshook-native/src-tauri/src/commands/discovery.rs`

Files to Modify

- `src/crosshook-native/src-tauri/src/commands/mod.rs` — add `pub mod discovery;`
- `src/crosshook-native/src-tauri/src/lib.rs` — register `discovery_search_trainers` in `tauri::generate_handler![]` macro (line ~209)

Create `commands/discovery.rs` with:

```rust
#[tauri::command]
pub fn discovery_search_trainers(
    query: TrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<TrainerSearchResponse, String> {
    metadata_store
        .search_trainer_sources(&query.query, query.limit.unwrap_or(20) as i64, query.offset.unwrap_or(0) as i64)
        .map_err(|e| e.to_string())
}
```

This is a **sync** `fn` (not `async fn`) — LIKE-based SQLite queries have no network I/O.

MANDATORY: End file with `#[cfg(test)]` contract test block casting each handler to its explicit function-pointer type. Follow `commands/community.rs:311–353` exactly.

### Phase 4: Frontend

#### Task 4.1: TypeScript Types + Discovery Hook

Depends on [3.1]

**READ THESE BEFORE TASK**

- `docs/plans/trainer-discovery/feature-spec.md` (lines 303–340 for TypeScript interfaces)
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` (hook pattern reference)
- `src/crosshook-native/src/types/index.ts` (barrel export)

**Instructions**

Files to Create

- `src/crosshook-native/src/types/discovery.ts`
- `src/crosshook-native/src/hooks/useTrainerDiscovery.ts`

Files to Modify

- `src/crosshook-native/src/types/index.ts` — add `export * from './discovery';`

Create `types/discovery.ts` with TypeScript interfaces mirroring the Rust structs field-for-field (camelCase per `#[serde(rename_all = "camelCase")]`):

- `VersionMatchStatus` type union: `'exact' | 'compatible' | 'newer_available' | 'outdated' | 'unknown'`
- `TrainerSearchQuery` interface
- `TrainerSearchResult` interface
- `TrainerSearchResponse` interface
- `VersionMatchResult` interface

Create `useTrainerDiscovery.ts` hook:

- Signature: `useTrainerDiscovery(query: string, options?: { limit?: number; offset?: number })`
- Return: `{ data: TrainerSearchResponse | null; loading: boolean; error: string | null; refresh: () => Promise<void> }`
- 300ms debounce on query changes (UX spec requirement)
- `requestIdRef.current` increment for stale request cancellation (copy from `useProtonDbSuggestions.ts`)
- Guard on empty/whitespace query (return null data, no IPC call)
- `invoke<TrainerSearchResponse>('discovery_search_trainers', { query: { query, limit, offset } })`

#### Task 4.2: TrainerDiscoveryPanel Component

Depends on [4.1]

**READ THESE BEFORE TASK**

- `docs/plans/trainer-discovery/research-ux.md` (UI workflow, component patterns, accessibility)
- `src/crosshook-native/src/components/CommunityBrowser.tsx` (result card pattern, client-side filtering)
- `src/crosshook-native/src/components/pages/CommunityPage.tsx` (host page)
- `src/crosshook-native/src/hooks/useScrollEnhance.ts` (SCROLLABLE selector)
- `src/crosshook-native/src/styles/variables.css` (CSS custom properties)

**Instructions**

Files to Create

- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`

Files to Modify

- `src/crosshook-native/src/hooks/useScrollEnhance.ts` — add TrainerDiscoveryPanel scroll container to `SCROLLABLE` selector
- `src/crosshook-native/src/components/pages/CommunityPage.tsx` — integrate TrainerDiscoveryPanel as sibling/nested tab

Component structure:

1. **Search input** — debounced (300ms), pre-fills from active profile game name if available
2. **Result cards** — game name, trainer name, compatibility badge (reuse `crosshook-protondb-tier-badge` CSS tokens), source name, trust indicator (Community badge)
3. **Expandable detail** — trainer_version, game_version, notes, SHA-256 (if present), source link CTA
4. **Source link CTA** — opens via Tauri `open()` shell plugin. NEVER `<a href>` navigation. NEVER `dangerouslySetInnerHTML`.
5. **Import CTA** — constructs manifest path from `tapLocalPath + '/' + relativePath + '/community-profile.json'` from the search result, then calls existing `community_import_profile` IPC. Only show if the associated community-profile.json exists at that path (check via filesystem or hide in Phase A; resolve in Phase B).
6. **Empty state** — "No trainers found" when query has no matches
7. **`discovery_enabled` settings integration** — this task OWNS the opt-in flow:
   - Read `discovery_enabled` from `SettingsStore` (defaults to `false` per resolved legal decision)
   - If `false` on panel open: show consent dialog before rendering any results
   - On consent: write `discovery_enabled = true` to settings via `update_settings` IPC
   - `discovery_search_trainers` backend already returns empty when store is disabled — the frontend gates on the setting value
   - The consent dialog explains: CrossHook links to external sources only, does not host trainers, user responsible for legal compliance, trainers for online games may violate ToS
8. **Legal disclaimer dialog** — part of the `discovery_enabled` opt-in flow above. Shown once on first panel open when `discovery_enabled = false`.

CRITICAL: Any `overflow-y: auto` container MUST be added to `SCROLLABLE` in `useScrollEnhance.ts`. Use `overscroll-behavior: contain` on inner scroll containers.

CSS classes: `crosshook-discovery-*` prefix (BEM-like). Use CSS custom properties from `variables.css`.

ARIA: `aria-live="polite"` on results count region. Keyboard-accessible expand/collapse. Focus management on search input.

Keep under 400 lines; extract `TrainerResultCard.tsx` as a separate component if needed.

#### Task 4.3: Rust Unit Tests for Discovery (Full Suite)

Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` (test patterns at lines 371–445)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (open_in_memory)

**Instructions**

This task writes the **complete test suite**, expanding the minimal compile-check tests added in Tasks 2.1 and 2.2. Do not add a second `#[cfg(test)]` module — extend the existing one.

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/discovery/search.rs` — expand existing `#[cfg(test)]` module with full test suite
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — expand existing tests for `index_trainer_sources()`

Test cases for `search.rs` (expand existing module):

- `search_returns_error_for_empty_query` (already exists from 2.1 — keep)
- `search_returns_empty_for_no_matches` (already exists from 2.1 — keep)
- `search_matches_game_name_substring` (NEW)
- `search_matches_source_name_substring` (NEW)
- `search_matches_notes_substring` (NEW)
- `search_respects_limit_cap_at_50` (NEW)
- `search_respects_offset_pagination` (NEW)
- `search_returns_tap_url_from_join` (NEW)

Test cases for `index_trainer_sources()` (expand existing module):

- `index_trainer_sources_inserts_entries` (already exists from 2.2 — keep)
- `index_trainer_sources_rejects_http_url` (already exists from 2.2 — keep)
- `index_trainer_sources_rejects_javascript_url` (NEW)
- `index_trainer_sources_enforces_a6_bounds_on_source_url` (NEW)
- `index_trainer_sources_enforces_a6_bounds_on_source_name` (NEW)
- `index_trainer_sources_enforces_a6_bounds_on_notes` (NEW)
- `index_trainer_sources_deletes_and_reinserts_on_reindex` (NEW)

All tests use `db::open_in_memory()` + `run_migrations()`. Factory function `make_trainer_source_entry(...)` for test data.

Run verification: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

## Advice

- **`shared.md`**: Aligned to **Option B** (`trainer_sources` + `trainer-sources.json`). If anything drifts, **`feature-spec.md` Decisions** win.
- **The `feature-spec.md` executive summary is misleading**: It says "zero new database tables" which was the original framing. The resolved decisions section (line 626) supersedes this — Option B creates a new table.
- **`TrainerSearchResult` in `feature-spec.md` references `community_profiles` fields** (`community_profile_id`, `trainer_name`, `compatibility_rating`, etc.) that don't exist in the `trainer_sources` table. The search query for Phase A should return `TrainerSourceRow`-shaped data from `trainer_sources` JOIN `community_taps`. The full `TrainerSearchResult` with profile-level fields is a Phase B concern when results are merged with `community_profiles` data.
- **Watermark skip is shared between profiles and trainer sources**: Both `index_community_tap_result()` and `index_trainer_sources()` key off the same `last_head_commit` on `community_taps`. When the v18 migration clears the watermark, both will re-index on next sync. This is the correct behavior.
- **`discovery_enabled` setting**: The feature-spec says `default: true` at line 424 but the resolved legal decision (line 628) says opt-in with `discovery_enabled = false` default. Follow the resolved decision — **default is false**.
- **Sync vs async is non-negotiable for Phase A**: `discovery_search_trainers` must be `pub fn`, not `pub async fn`. LIKE queries are synchronous SQLite operations. Mixing sync/async for the same command is an anti-pattern in this codebase.
- **IPC contract test is a hard convention**: Omitting the `#[cfg(test)]` function-pointer cast block from `commands/discovery.rs` will break the established convention across all command files. This is not optional.
- **FTS5 is blocked by a Cargo feature flag**: `rusqlite` uses `features = ["bundled"]`. FTS5 requires `bundled-full`. Do not use FTS5 queries or MATCH syntax anywhere in Phase A or B. Track as a separate issue for Phase C.
- **Component scroll registration is a WebKitGTK requirement**: Missing the `useScrollEnhance.ts` SCROLLABLE registration causes dual-scroll jank that is only visible in the AppImage (not in dev browser). Register on first commit, not as follow-up.
- **Import CTA reuses existing flow**: The "Import Profile" button constructs `tapLocalPath/relativePath/community-profile.json` from the search result and calls `community_import_profile`. The search query includes `ct.local_path` for this purpose. Do not build a parallel import mechanism.
- **`validate_tap_url()` is reference-only**: It is a private function in `community/taps.rs` that also accepts SSH scheme URLs (for git repos). The HTTPS-only check for trainer source URLs should be a custom inline check in `index_trainer_sources()`, not a call to `validate_tap_url()`.
- **`discovery_enabled` setting is a Phase A ship blocker**: The opt-in consent flow (default `false`, consent dialog, persist `true` on acceptance) is owned by Task 4.2. No separate backend task is needed — the frontend reads/writes the setting via existing `SettingsStore` IPC.
