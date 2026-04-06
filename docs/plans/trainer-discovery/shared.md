# Trainer Discovery

CrossHook's trainer-discovery feature adds search and cataloging for game trainers across community taps, building on the existing community tap sync pipeline and a new SQLite **`trainer_sources`** table populated from per-game **`trainer-sources.json`** manifests (see [`feature-spec.md`](./feature-spec.md) Decision 1, Option B). Phase A (MVP) adds **`migrate_17_to_18`** in `metadata/migrations.rs` (`CREATE TABLE trainer_sources`), extends tap indexing to parse `trainer-sources.json`, exposes LIKE-based search over **`trainer_sources`** (not new columns on `community_profiles`), and surfaces results in **`TrainerDiscoveryPanel`**. **`CommunityProfileMetadata` / `CommunityProfileRow` are not extended** with `source_url` / `source_name` for this feature. Phase B adds external source HTTP clients (FLiNG RSS, etc.) following `protondb/client.rs`, `external_cache_entries` caching, and token scoring. Phase C adds FTS5 (see `feature-spec.md`) via `rusqlite` `bundled-full` and a follow-on migration for an FTS virtual table over indexed discovery text. The feature integrates with trainer hash verification (#156), version snapshots, and the community import flow — no launch pipeline changes.

> **Planning vs code:** Files in this folder describe the intended design. SQLite **`user_version`** after `run_migrations()` is **17** in-tree today (`migrations.rs`); trainer-discovery implementation adds **18** for `trainer_sources`. `AGENTS.md` “Current schema version: 13” refers to the documented table-inventory snapshot, not `user_version`.

## Relevant Files

### Rust — crosshook-core (business logic)

- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: Crate root re-exporting all domain modules; add `pub mod discovery;` here
- `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`: `CommunityProfileMetadata` — **no discovery-specific `source_url` / `source_name` changes** under Option B
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` facade — wraps `Arc<Mutex<Connection>>`, exposes `with_conn`/`with_conn_mut`/`with_sqlite_conn`; add trainer-source search/count helpers delegating to `discovery::search`
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: After the existing `version < 17` block, add **`migrate_17_to_18`** — `CREATE TABLE trainer_sources` + indexes (SQL in `feature-spec.md`). Optional later: **`migrate_18_to_19`** for FTS5 virtual table (Phase C)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs`: Add `index_trainer_sources()` — transactional DELETE+INSERT (or equivalent) for rows keyed by `tap_id`, A6 bounds, HTTPS-only URL validation; call from the same tap sync path as `index_community_tap_result()`
- `src/crosshook-native/crates/crosshook-core/src/community/index.rs`: Walk tap workspaces for **`trainer-sources.json`** alongside `community-profile.json`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: `CommunityProfileRow`, `MetadataStoreError` — row type unchanged for Option B; discovery row types live under `discovery/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry()`/`put_cache_entry()` — reuse for Phase B external metadata (`external_cache_entries`)
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `compute_correlation_status()` — version matching for Phase B
- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: Reference HTTP client — `OnceLock<reqwest::Client>`, cache→live→stale-fallback; clone for `discovery/client.rs` in Phase B
- `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: Reference for Serde conventions — `#[serde(rename_all = "camelCase")]` on IPC types, `#[serde(rename_all = "snake_case")]` on state enums, cache key namespace pattern
- `src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs`: Reference for pure-function derivation
- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: `tokenize()` / `token_hits()` for Phase B ranking (optional lift to `text_utils` — see `research-practices.md` resolved decisions)
- `src/crosshook-native/crates/crosshook-core/src/offline/hash.rs`: Existing trainer hash verification; discovery surfaces optional `sha256` from `trainer-sources.json` only as metadata
- `src/crosshook-native/crates/crosshook-core/Cargo.toml`: Phase A keeps `rusqlite = { features = ["bundled"] }`; Phase C adds `bundled-full` (or equivalent) for FTS5

### Tauri IPC Layer

- `src/crosshook-native/src-tauri/src/lib.rs`: App entry point — registers managed state and `invoke_handler!`; add discovery commands here
- `src/crosshook-native/src-tauri/src/commands/mod.rs`: Command module registry — add `pub mod discovery;`
- `src/crosshook-native/src-tauri/src/commands/community.rs`: Reference sync IPC commands — `community_sync`, `community_import_profile`; IPC contract test block (`#[cfg(test)]` at lines 311–353)
- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: Reference async IPC — `.inner().clone()` before `await`

### Frontend — React/TypeScript

- `src/crosshook-native/src/hooks/useCommunityProfiles.ts`: Reference for community profile types (import path from discovery)
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`: Reference hook — `requestIdRef` race guard, loading/error state
- `src/crosshook-native/src/components/CommunityBrowser.tsx`: Reference — `matchesQuery()` pattern
- `src/crosshook-native/src/components/pages/CommunityPage.tsx`: Host page — discovery panel as sibling/nested tab
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: CRITICAL — register new scroll containers in `SCROLLABLE`
- `src/crosshook-native/src/types/index.ts`: Barrel — add `export * from './discovery'`
- `src/crosshook-native/src/styles/variables.css`: CSS custom properties

## Relevant Tables

- **`trainer_sources`**: Phase A search target — one row per source entry from `trainer-sources.json`; LIKE on `game_name`, `source_name`, `notes`; **`migrate_17_to_18`**
- **`community_profiles`**: Unchanged for Option B — still indexed from `community-profile.json`; used for **Import Profile** correlation, not primary discovery search
- **`community_taps`**: JOIN for `tap_url`; watermark skip unchanged
- **`external_cache_entries`**: Phase B — keys such as `trainer:source:v1:{normalized_game_key}` (see `feature-spec.md` / `research-architecture.md`)
- **`version_snapshots`**: Phase B version matching via `version_store.rs`
- **`trainer_hash_cache`**: Launch-time verification; discovery does not substitute this

## Relevant Patterns

**MetadataStore Facade**: All SQLite access through `MetadataStore` wrapper methods. See `metadata/mod.rs` and `metadata/community_index.rs`.

**Thin IPC Command Handlers**: `commands/protondb.rs` (async) and `commands/community.rs` (sync).

**IPC Contract Tests**: Mandatory `#[cfg(test)]` function-pointer assertions per command file.

**Cache-First Fetch**: `protondb/client.rs:85–130`.

**Watermark-Skip Indexing**: `community_index.rs` — extend so trainer source index respects the same tap HEAD semantics as profiles.

**Domain Module Layout**: Mirror `protondb/`: `discovery/mod.rs`, `models.rs`, `search.rs`, `client.rs` (Phase B), optional `version_match.rs`.

**Serde on IPC Boundary**: `camelCase` / `snake_case` conventions as in existing commands.

**Frontend Hook Pattern**: `useProtonDbSuggestions.ts`-style `requestIdRef` + `{ data, loading, error, refresh }`.

## Relevant Docs

**docs/plans/trainer-discovery/feature-spec.md**: Authoritative decisions, SQL, IPC names, phases.

**AGENTS.md**: Architecture rules, Tauri IPC, scroll containers, persistence classification.

**docs/plans/trainer-discovery/research-practices.md**: Resolved design decisions, FTS/LIKE notes, testing guidance.

**docs/plans/trainer-discovery/research-technical.md**: Technical depth, migration and module layout (aligned to Option B + FTS virtual table in Phase C).

**docs/plans/trainer-discovery/research-security.md**: URL validation, DMCA, WebKitGTK.

**docs/plans/trainer-discovery/research-ux.md**: UX flows and a11y.

**docs/features/steam-proton-trainer-launch.doc.md**: Profile import and launch integration.

## Critical Constraints

- **Phase A/B search**: LIKE on `trainer_sources` until Phase C enables FTS5.
- **DMCA §1201**: Opt-in — `discovery_enabled = false` default; consent on first enable (`feature-spec.md` Decision 2).
- **`useScrollEnhance`**: Register every new `overflow-y: auto` discovery container.
- **No frontend test framework**: Prefer `cargo test -p crosshook-core` with `MetadataStore::open_in_memory()`.
- **Sync vs async IPC**: `discovery_search_trainers` stays sync; external calls are separate async commands.
- **Import**: Use existing `community_import_profile` only.
- **Trainer versions**: Not semver — display strings as provided.
- **MetadataStore::disabled()**: Empty results, no panic.
