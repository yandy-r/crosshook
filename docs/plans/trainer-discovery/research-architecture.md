# Architecture Research: trainer-discovery

## System Overview

CrossHook is a Tauri v2 native Linux desktop app (AppImage). Business logic lives in `crosshook-core` (a Rust workspace crate), Tauri IPC commands are thin wrappers in `src-tauri/src/commands/`, and the React/TypeScript frontend consumes them via `invoke()` hooks. The SQLite metadata DB (`~/.local/share/crosshook/metadata.db`) is advanced by `metadata::run_migrations()`; **`PRAGMA user_version` is 17** after the last in-tree migration guard in `migrations.rs`. Trainer-discovery adds **`user_version` 18** via **`CREATE TABLE trainer_sources`**. (`AGENTS.md` lists a separate “schema version 13” table-inventory line — **do not** use that for migration step names.)

---

## Relevant Components

### Rust — crosshook-core

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs`: crate root — re-exports all domain modules (`community`, `metadata`, `protondb`, `steam_metadata`, etc.)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/mod.rs`: re-exports `CommunityTapStore`, `CommunityProfileManifest`, `CommunityProfileMetadata`, `CommunityProfileIndex`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/taps.rs`: `CommunityTapStore` — git clone/fetch, workspace resolution, `sync_many()`, `index_workspaces()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/index.rs`: `index_taps()` — walks tap workspace directories, parses `manifest.json` files, builds `CommunityProfileIndex`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`: `CommunityProfileManifest`, `CommunityProfileMetadata`, `CompatibilityRating` — the data model that is both the JSON manifest on disk AND the source of truth for what gets inserted into `community_profiles`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` — the single SQLite facade, wraps `Arc<Mutex<Connection>>`, exposes all sub-store methods
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs`: `index_community_tap_result()`, `list_community_tap_profiles()` — the community tap indexing pipeline; inserts into `community_profiles` and `community_taps` tables
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: SQLite migration chain ending at **`user_version` 17** in-tree; trainer-discovery adds **`migrate_17_to_18`** for `trainer_sources` (Phase C may add **`migrate_18_to_19`** for FTS5 virtual table)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: `CommunityProfileRow`, `CommunityTapRow`, `MetadataStoreError` — model structs mapping to DB rows
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry()`, `put_cache_entry()` — generic TTL cache over `external_cache_entries` table, used by protondb and steam_metadata; trainer-discovery Phase 2 would use this same pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: the reference pattern for external HTTP client: `OnceLock<reqwest::Client>`, TTL-based cache via `external_cache_entries`, async `lookup_protondb()` public fn
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs`: module root re-exporting public API from `client.rs`, `models.rs`, `suggestions.rs`, `aggregation.rs`

### Tauri IPC Layer

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: Tauri app entry point — manages `MetadataStore`, `CommunityTapStore`, `ProfileStore`, `SettingsStore` as Tauri managed state; registers all IPC commands via `invoke_handler!`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs`: command module registry — new `discovery.rs` goes here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs`: reference IPC command file — shows how commands inject `State<'_, MetadataStore>`, `State<'_, CommunityTapStore>`, call core logic, return `Result<T, String>` (errors as strings)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs`: reference for async IPC commands — shows `pub async fn protondb_lookup(...)` pattern with `metadata_store.inner().clone()`

### Frontend — React/TypeScript

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useCommunityProfiles.ts`: reference hook — shows type mirroring of Rust structs, `invoke()` calls, state management pattern for community data
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: reference hook for async IPC commands with loading/error state, request-ID race-condition guard, `forceRefresh` pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CommunityBrowser.tsx`: reference component showing how `useCommunityProfiles` is consumed; client-side `matchesQuery()` for search over `CommunityProfileIndexEntry[]`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/CommunityPage.tsx`: the page that hosts `CommunityBrowser` — trainer-discovery panel would live at a sibling or nested tab here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useScrollEnhance.ts`: CRITICAL — WebKitGTK scroll management. Any new `overflow-y: auto` container must be added to `SCROLLABLE` selector here

---

## Data Flow

### Community Tap Indexing (existing, used by trainer-discovery)

```
User subscribes to tap → settings.community_taps[] (TOML)
    ↓  community_sync IPC command
CommunityTapStore::sync_many()
    → git clone/fetch each tap
    → index_workspaces() walks local checkout for manifest.json files
    → returns CommunityProfileIndex (in-memory)
    ↓  MetadataStore::index_community_tap_result()
    → transactional DELETE + INSERT into community_profiles (SQLite)
    → watermark skip if HEAD commit unchanged
    → A6 field length bounds applied
    ↓  IPC returns Vec<CommunityTapSyncResult> to frontend
```

### Trainer Discovery Search (Phase A)

```
User enters search query
    ↓  discovery_search_trainers IPC command (sync)
MetadataStore → LIKE query on trainer_sources
    (game_name, source_name, notes) + JOIN community_taps for tap_url
    → returns TrainerSearchResponse
    ↓  useTrainerDiscovery hook
TrainerDiscoveryPanel component displays results
```

### External Cache Pattern (protondb reference, for Phase 2)

```
discovery_get_trainer_sources IPC command (async, Phase B)
    ↓  DiscoveryClient (HTTP, OnceLock<reqwest::Client>)
    → check external_cache_entries for cache_key "trainer:source:v1:{normalized_game_key}"
         (normalize: lowercase, collapse whitespace; document exact fn in client.rs)
    → default TTL: fresh 24h, allow stale read up to 7d on failure (tune in implementation; test with mocked clock)
    → if miss: fetch external API (FLiNG RSS per Decision 3), put_cache_entry()
    → return JSON payload mirroring TrainerSourceEntry list (+ stale flag)
```

---

## Integration Points

### Phase A (MVP) — `trainer_sources` + LIKE search

1. **`migrate_17_to_18`** (`metadata/migrations.rs`): `CREATE TABLE trainer_sources` + indexes (exact SQL in `feature-spec.md`). **Do not** `ALTER TABLE community_profiles` for `source_url` / `source_name` (Option B).

2. **Tap workspace walk** (`community/index.rs`): discover **`trainer-sources.json`** next to each `community-profile.json`.

3. **`index_trainer_sources()`** (`metadata/community_index.rs`): parse manifest → transactional DELETE/INSERT (or equivalent) per `tap_id`, A6 bounds, HTTPS-only URL validation.

4. **`crosshook-core::discovery` module** (created in Phase A, not deferred):
   - `discovery/mod.rs`: re-exports
   - `discovery/models.rs`: `TrainerSearchQuery`, `TrainerSearchResponse`, row structs aligned with `feature-spec.md`
   - `discovery/search.rs`: parameterized LIKE SQL against **`trainer_sources`**

5. **IPC** (`commands/discovery.rs`): `discovery_search_trainers(query: TrainerSearchQuery, metadata_store: State<'_, MetadataStore>) -> Result<TrainerSearchResponse, String>` — **sync**.

6. **Register** in `lib.rs` + `commands/mod.rs`; **contract tests** cast function pointers.

7. **Frontend**: `useTrainerDiscovery.ts`, `TrainerDiscoveryPanel.tsx`; open URLs via shell plugin only.

### Phase B — External cache + version match + extra IPC

1. **`discovery/client.rs`**: `static` `OnceLock<reqwest::Client>` (same builder knobs as ProtonDB); **`trainer:source:v1:{normalized_game_key}`** cache keys; TTL policy as in External Cache Pattern above.

2. **`discovery_get_trainer_sources`**: **Request** `{ "gameName": string, "steamAppId"?: number, "forceRefresh"?: boolean }` → **Response** `{ "sources": TrainerSourceEntry[], "isStale": boolean, "cacheKey": string }` (Serde camelCase on boundary). Registers as **`pub async fn`** with `metadata_store.inner().clone()` before await.

3. **`discovery_check_version_compatibility`**: **Request** `{ "communityProfileId": number, "profileName": string }` → **Response** `VersionMatchResult` (reuse shapes from `feature-spec.md`). Implementation loads latest **`version_snapshots`** row for that profile via `version_store.rs` helpers, compares **`steam_build_id`** (manifest) to **`human_game_ver` / stored trainer strings** using **`compute_correlation_status()`** — **advisory only** (no blocking).

4. **`discovery_search_external`** (optional name per `feature-spec.md`): async aggregation entry point for FLiNG RSS + merge with tap rows in UI.

### Phase C — FTS5 + rebuild (feature-complete)

1. **`rusqlite`**: add **`features = ["bundled-full"]`** (or approved FTS-capable set).

2. **`migrate_18_to_19`**: create **FTS5 virtual table** with `content='trainer_sources'` + triggers, **or** document rebuild-only strategy; **`discovery_rebuild_index`** must repopulate FTS from `trainer_sources`.

3. **`discovery/search.rs`**: FTS **`MATCH`** + BM25; sanitize user tokens; **fallback** to LIKE on FTS init failure (log + metric).

4. **Acceptance**: see `feature-spec.md` Phase C (p95 latency, rebuild idempotence).

**IPC — `discovery_rebuild_index`**: **Request** `{}` or `{ "tapId"?: string }` (optional scope) → **Response** `{ "ok": true, "rowsIndexed": number }`. **Sync** preferred (may be slow — show UI spinner); register in `commands/discovery.rs`.

---

## Key Dependencies

### Rust (all already in Cargo.toml for Phase A — Phase C may widen `rusqlite` features)

| Crate                  | Role                                                                                             |
| ---------------------- | ------------------------------------------------------------------------------------------------ |
| `rusqlite` 0.39        | SQLite queries, LIKE search. FTS5 requires adding `bundled-full` feature — not currently enabled |
| `reqwest` 0.12+        | HTTP client (Phase 2 external lookups)                                                           |
| `serde` / `serde_json` | IPC serialization                                                                                |
| `tokio`                | Async runtime for async IPC commands                                                             |
| `chrono`               | TTL timestamps in cache store                                                                    |
| `sha2`                 | Already in use for trainer hash verification                                                     |

### TypeScript (all already in package.json)

| Package                    | Role                                                |
| -------------------------- | --------------------------------------------------- |
| `@tauri-apps/api/core`     | `invoke()` for IPC calls                            |
| `@tauri-apps/plugin-shell` | `open()` for external URLs (trainer download pages) |

### Internal Modules Affected

| Module                      | Impact                                                                                                      |
| --------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `crosshook-core::metadata`  | `migrate_17_to_18`; `index_trainer_sources()`; `MetadataStore` search helpers; optional Phase C FTS helpers |
| `crosshook-core::community` | Index walk includes **`trainer-sources.json`** paths; no `CommunityProfileMetadata` URL fields for Option B |
| `src-tauri::commands`       | New `discovery.rs` command file + `lib.rs` registration                                                     |
| Frontend hooks              | New `useTrainerDiscovery.ts`                                                                                |
| Frontend components         | New `TrainerDiscoveryPanel.tsx`; `CommunityPage.tsx` likely gets a new tab                                  |

---

## Architectural Patterns

- **Flat IPC commands as module files**: each domain area has its own `commands/*.rs` file (`protondb.rs`, `community.rs`); new discovery commands follow the same pattern in `commands/discovery.rs`
- **MetadataStore as monolithic SQLite facade**: all DB operations go through `MetadataStore` methods. Discovery SQL lives in **`crosshook-core::discovery::search`** (Phase C FTS helpers may live in `discovery/fts.rs` or `metadata/` — pick one module in implementation; document in PR)
- **`OnceLock` HTTP client singleton**: `protondb/client.rs:26` defines `static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client>`. The discovery client replicates this exactly
- **Cache-key-based external cache**: `external_cache_entries` table with TTL. Cache key format follows `protondb`'s `cache_key_for_app_id()` convention
- **Field bounds enforcement (A6 security)**: apply the same style of byte limits to **`trainer_sources`** text columns at index time (`index_trainer_sources`)
- **Watermark skip for indexing**: `index_community_tap_result()` no-ops if `HEAD` commit is unchanged — **`index_trainer_sources()` must run under the same tap sync decision** so trainer rows do not drift from profile rows
- **Type mirroring**: Frontend TypeScript types manually mirror Rust structs (see `useCommunityProfiles.ts:7–63`). The `TrainerSearchResult` type must be mirrored in `useTrainerDiscovery.ts`
- **IPC errors as strings**: all commands return `Result<T, String>` with `map_err(|e| e.to_string())` — do not change this convention
- **IPC contract tests**: every `commands/*.rs` file ends with a `#[cfg(test)]` block casting each handler to its explicit function-pointer type — compile-time IPC signature validation. Reference: `commands/community.rs:311–353`. `commands/discovery.rs` must follow this pattern for all exposed commands
- **Pure-function isolation**: business logic with no I/O (version matching, name scoring) goes in dedicated files (`discovery/matching.rs`) and is tested directly — see `protondb/suggestions.rs` and `metadata/version_store.rs::compute_correlation_status` as references
- **MetadataStore threading model**: wraps `Arc<Mutex<Connection>>` — single-writer SQLite via mutex. All DB access uses an internal `with_sqlite_conn(action_label, |conn| {...})` accessor. **Never hold the lock across an await point.** Async commands (**Phase B** `discovery_get_trainer_sources`, `discovery_search_external`, etc.) must call `metadata_store.inner().clone()` before the first `.await` — same pattern as `commands/protondb.rs:55`. Phase A `discovery_search_trainers` stays synchronous (`pub fn`).
- **Tauri managed state injection**: `MetadataStore`, `CommunityTapStore` etc. are registered in `lib.rs` via `.manage()` and injected into commands as `State<'_, T>` parameters

---

## Edge Cases

- `MetadataStore::disabled()` path: when SQLite is unavailable at startup, `MetadataStore` is created with `conn: None`. All sub-store functions check `is_available()` before accessing `conn`. Discovery search must degrade gracefully when the store is disabled — return an empty result or an explicit "unavailable" error, not a panic
- Watermark skip and new manifests: if a tap adds **`trainer-sources.json`** but HEAD is unchanged, existing watermark skip may skip re-index — implementation should either bundle trainer indexing into the same “dirty” detection as profiles or document that maintainers must bump tap content / force sync after adding manifests
- `useScrollEnhance.ts` constraint: any new scrollable panel container needs its CSS selector added to the `SCROLLABLE` constant in this hook, or WebKitGTK will apply enhanced scroll to the wrong container causing dual-scroll jank
- External URL opening: trainer download links must use `tauri-plugin-shell`'s `open()` — never `fetch()` or `reqwest` — to avoid binary retrieval (BR-1 legal constraint)
- A6 bounds on `trainer_sources.source_url` / `source_name` / `notes`: enforce before INSERT (see `feature-spec.md` / `shared.md` constants)

---

## Other Docs

- `docs/plans/trainer-discovery/research-technical.md` — implementation phases, component table, migration plan
- `docs/plans/trainer-discovery/feature-spec.md` — business rules, user stories, external API details
- `docs/plans/trainer-discovery/research-security.md` — A6 bounds, DMCA considerations, URL safety
- Tauri v2 managed state docs: <https://v2.tauri.app/develop/state-management/>
- `AGENTS.md` (project root) — normative architecture rules, directory map, 18-table SQLite inventory, persistence classification
- rusqlite `bundled-full` feature (enables FTS5): <https://docs.rs/rusqlite/latest/rusqlite/#optional-features>
