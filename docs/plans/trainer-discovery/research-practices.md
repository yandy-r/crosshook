# Engineering Practices Research: Trainer Discovery

## Executive Summary

CrossHook has a well-structured, modular codebase with clear separation between business logic (`crosshook-core`), IPC layer (`src-tauri`), and React frontend. Trainer discovery should follow the ProtonDB suggestion module as its primary architectural reference: network fetch with `external_cache_entries`, typed models in `crosshook-core`, thin IPC handler, and a hook-wrapped frontend. The feature is achievable at significantly lower effort than "Very High" if scoped to index-linking (not embedding external trainer metadata).

---

## Existing Reusable Code

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — Cache-first HTTP fetch pattern; reqwest singleton via `OnceLock`, stale-fallback from `external_cache_entries`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs` — Typed result/state enums (`ProtonDbLookupState::Ready/Stale/Unavailable`) to replicate for discovery
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs` — Pure suggestion derivation without I/O; testable in isolation
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry` / `put_cache_entry` / `evict_expired_cache_entries` functions operating on `external_cache_entries`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — `index_community_tap_result` with A6 guards; trainer-discovery adds **`index_trainer_sources()`** persisting into **`trainer_sources`** (see `feature-spec.md` Option B)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/suggestion_store.rs` — TTL-based dismissal pattern; mirrors what a "dismissed source" or "hidden result" feature would need
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/taps.rs` — Git clone/fetch with security env isolation; `validate_tap_url`, `validate_branch_name`, `is_valid_git_sha`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/index.rs` — `index_taps` / `index_tap` filesystem scan; schema version gating at `COMMUNITY_PROFILE_SCHEMA_VERSION`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/offline/hash.rs` — `verify_and_cache_trainer_hash`, `normalize_sha256_hex` — reusable for cross-checking community-published SHA-256 against a discovered trainer download link
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/trainer_hash.rs` — `trainer_hash_launch_check` / `collect_trainer_hash_launch_warnings`; hash advisory mechanism to mirror for discovery results
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/discovery.rs` — `discover_game_executable_candidates`; tokenization and scored candidate ranking pattern (directly applicable to trainer filename matching)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` — `compute_correlation_status`; pure function to compare game build ID vs snapshot — adapt for game version ↔ trainer version matching
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` — `CommunityProfileMetadata` with `trainer_sha256: Option<String>` — the existing schema extension point for trainer identity
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs` — IPC command pattern: `State<'_, MetadataStore>` injection, `map_err(|e| e.to_string())`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs` — `community_sync` pattern; multi-result fan-out, watermark skip in metadata indexer
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` — Reference hook: request-id race guard, `loading`/`error` state, `invoke()` wrapping
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useCommunityProfiles.ts` — `matchesQuery` substring search; `sortProfiles` by rating then name — patterns to adapt for discovery search UI
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CommunityBrowser.tsx` — `matchesQuery` (substring search across all metadata fields); `ratingOrder` compatibility badge rendering — reusable search infrastructure

---

## Modularity Design

- **Cache-first fetch via `external_cache_entries`**: All external network data flows through `put_cache_entry` → `get_cache_entry` with RFC3339 `expires_at`. The ProtonDB client in `protondb/client.rs` shows the complete pattern: check cache, attempt live fetch, fall back to stale row on network failure. Trainer discovery metadata fetches must use this same table rather than a bespoke cache.
- **Module-per-domain in `crosshook-core/src/`**: Each domain (`protondb/`, `community/`, `install/`) has `mod.rs` + focused subfiles. A new `discovery/` module directory should follow this exact layout: `mod.rs` (public re-exports), `client.rs` (fetch + cache), `models.rs` (typed structs), and `matching.rs` (pure version/name matching logic).
- **Thin IPC handlers**: `src-tauri/src/commands/` files are ~50–100 lines each. They receive `State<'_, T>` managed values, call into `crosshook-core`, and map errors with `.map_err(|e| e.to_string())`. No business logic lives in command files.
- **Pure-function derivation (testable without I/O)**: `derive_suggestions()` in `protondb/suggestions.rs` and `compute_correlation_status()` in `version_store.rs` take typed inputs and return typed outputs without touching the database or network. Version matching and name normalization for trainer discovery must follow this pattern.
- **Scored candidate ranking**: `install/discovery.rs` uses a point-scoring system with token matching, depth penalties, and denylist terms. The same approach (score trainer source results by game-name token overlap + version proximity) avoids external search indexing crates entirely.
- **A6 field-length bounds on external data**: `community_index.rs` enforces byte limits on all string fields from community tap data before inserting into SQLite. Any external trainer metadata indexed into SQLite must apply the same guards.
- **Watermark-skip indexing**: `index_community_tap_result()` compares `last_head_commit` before re-indexing — avoid redundant work. Discovery source indexing should use a similar content-hash or `ETag`/`Last-Modified` watermark.
- **Hook wrapping with request-id race guard**: `useProtonDbSuggestions.ts` uses `requestIdRef` to discard stale responses from superseded fetches. This is standard for async hooks in this codebase.
- **`OnceLock<reqwest::Client>` HTTP singleton**: The ProtonDB client initializes one shared `reqwest::Client` lazily. Discovery should reuse this pattern rather than creating a new client per request.

---

## KISS Assessment

The "Very High" complexity estimate assumes a real-time full-text search engine over live trainer source websites. This is avoidable.

| Simpler Alternative                                                                                                                                        | Complexity Reduction                          | Trade-off                                                                |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------ |
| **Community tap metadata only** — Discovery is a curated list of trainer sources published as community tap metadata, not a live web scraper               | High → Low                                    | Users must add taps; no auto-discovery of new sources                    |
| **SQLite `LIKE` substring search** on indexed `community_profiles.trainer_name` / `game_name` — no tantivy, no FTS5 extension                              | High → Low                                    | No fuzzy/stemming, but substring is sufficient for trainer source lookup |
| **Link-only model** — Discovery returns a `download_url` string from the index; CrossHook does not fetch, parse, or verify until user explicitly downloads | Eliminates HTTP fetch layer entirely from MVP | User clicks out to browser; hash verification deferred to a Phase 2      |
| **Game-name token matching** using the existing `tokenize()` / `token_hits()` functions from `install/discovery.rs`                                        | Zero new code for matching                    | No semantic search; works well for exact game titles                     |
| **Skip version matching in Phase 1** — Show all trainers for a game and let the community metadata carry the version context string                        | Eliminates version comparison logic           | User must visually verify version; still better than nothing             |

**Minimal viable scope**: A community tap extension where tap maintainers publish a `trainer-sources.json` alongside their profiles. CrossHook indexes it into a new `trainer_sources` SQLite table via the existing `index_community_tap_result` pathway. Discovery queries `trainer_sources` with a `game_name LIKE ?` search and returns links. Zero live web traffic from CrossHook itself; version matching is advisory text from the tap metadata.

---

## Abstraction vs. Repetition

| Decision                         | Recommendation                                                                                                                                                                                                                                                              | Rationale                                                                                         |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Cache read/write                 | **Reuse** `metadata::cache_store::{get_cache_entry, put_cache_entry}`                                                                                                                                                                                                       | Single `external_cache_entries` table, already migrated, eviction built in                        |
| HTTP client                      | **Reuse** the `OnceLock<reqwest::Client>` pattern from `protondb/client.rs`; do not share the ProtonDB singleton directly — create a separate `DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client>`                                                                            | Different timeout/user-agent may be needed                                                        |
| Version string matching          | **New pure function** in `discovery/matching.rs` — do not reuse `compute_correlation_status` directly (it's build-ID centric); extract the tokenize/normalize logic from `install/discovery.rs` and place a `normalize_version_string()` utility in `discovery/matching.rs` | The semantics differ; coupling would create a maintenance burden                                  |
| SHA-256 verification             | **Reuse** `offline::hash::normalize_sha256_hex()` and `verify_and_cache_trainer_hash()` verbatim                                                                                                                                                                            | Already tested, already handles caching                                                           |
| Search UI (input + results list) | **Duplicate** the `matchesQuery` pattern from `CommunityBrowser.tsx` into the Discovery component — do not abstract into a shared component yet                                                                                                                             | Only two use-sites; premature abstraction would create a poorly-shaped shared component           |
| Compatibility badge rendering    | **Reuse** the `ratingOrder` + `ratingLabel` + CSS class pattern from `CommunityBrowser.tsx`                                                                                                                                                                                 | Trainer sources can expose a compatibility rating field using the same `CompatibilityRating` enum |
| IPC command structure            | **Follow exactly** `commands/protondb.rs` and `commands/community.rs`                                                                                                                                                                                                       | Established pattern; thin handler + `map_err`                                                     |

---

## Interface Design

### Rust public API (`crosshook-core/src/discovery/`)

```rust
// mod.rs re-exports
pub use client::lookup_trainer_sources;
pub use models::{
    TrainerSourceEntry, TrainerSourceIndex, TrainerDiscoveryResult,
    TrainerDiscoveryState, TRAINER_DISCOVERY_CACHE_NAMESPACE,
};
pub use matching::score_trainer_sources_for_game;
```

**`lookup_trainer_sources(metadata_store: &MetadataStore, game_name: &str, force_refresh: bool) -> TrainerDiscoveryResult`**

- Check `external_cache_entries` by `TRAINER_DISCOVERY_CACHE_NAMESPACE::{normalized_game_name}` key
- On cache miss or `force_refresh`, fetch from the tap-published index (or a designated discovery endpoint)
- Store result in `external_cache_entries` with a TTL (24h suggested)
- Return stale data on network failure

**`score_trainer_sources_for_game(sources: &[TrainerSourceEntry], game_name: &str, game_version: Option<&str>) -> Vec<ScoredTrainerSource>`**

- Pure function, no I/O
- Reuses `tokenize()` from `install/discovery.rs` (move to a shared `text_utils.rs` if both modules need it)
- Returns sorted by score descending

### Tauri IPC commands (`src-tauri/src/commands/trainer_discovery.rs`)

```
trainer_discovery_lookup(game_name: String, force_refresh: Option<bool>, metadata_store: State<'_, MetadataStore>) -> Result<TrainerDiscoveryResult, String>
```

### Frontend hook (`src/crosshook-native/src/hooks/useTrainerDiscovery.ts`)

```typescript
useTrainerDiscovery(gameName: string): {
  result: TrainerDiscoveryResult | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}
```

Follows `useProtonDbSuggestions.ts` exactly: `requestIdRef` race guard, `forceRefresh` parameter.

### Integration with profile creation

The `TrainerSection` component (`src/crosshook-native/src/components/profile-sections/TrainerSection.tsx`) is the natural injection point. When the user is on the trainer path field with an app ID present, the discovery hook fires and renders a collapsible result panel below the path input — same UI surface as `ProtonDbLookupCard`.

---

## Testability Patterns

| Test Layer                             | Recommended Approach                                                                                                               | Anti-pattern to Avoid                                              |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Version/name matching (`matching.rs`)  | `#[test]` unit tests with no I/O; pass `&[TrainerSourceEntry]` and strings                                                         | Do not test matching through the IPC layer                         |
| Cache store (`external_cache_entries`) | `MetadataStore::open_in_memory()` already available — use it in `#[cfg(test)]`                                                     | Do not mock the SQLite connection                                  |
| HTTP fetch (`client.rs`)               | Compile-time conditional: replace `reqwest::Client` with a trait object in tests; OR use `wiremock`/`httpmock` crate               | Do not make real HTTP calls in unit tests                          |
| A6 field-length validation             | Unit tests on the validation function directly (same pattern as `check_a6_bounds` in `community_index.rs`)                         | Do not test through the full indexing transaction                  |
| IPC command smoke tests                | Follow `commands/community.rs` pattern: `fn command_names_match_expected_ipc_contract()` — just assert function signatures compile | Do not spin up a full Tauri app for command unit tests             |
| Frontend hook                          | Not tested (no frontend test framework configured); test the pure utility functions only                                           | Do not add Jest/Vitest without a broader frontend testing decision |

**Anti-pattern**: Testing search ranking through the Tauri IPC boundary. Keep `score_trainer_sources_for_game` pure and test it directly.

---

## Build vs. Depend

| Need                                 | Build (custom)                                             | Library                                              | Recommendation                                                                                                                                                                                                                                         |
| ------------------------------------ | ---------------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| HTTP fetch                           | —                                                          | `reqwest` (already in Cargo.toml)                    | **Reuse reqwest** — already present with `rustls-tls`                                                                                                                                                                                                  |
| Full-text search                     | Custom token scorer                                        | `tantivy`, SQLite FTS5                               | **Build** custom scorer using `tokenize()` from `install/discovery.rs`; only needed for simple substring+token match over <10K entries. FTS5 would require enabling SQLite feature flag; tantivy is a heavy dependency for marginal gain at this scale |
| Version string comparison            | Custom normalizer                                          | `semver` crate                                       | **Build** a simple normalizer. Trainer versions are not semver-compliant (e.g., "v1.0 +DLC", "2024.12.05"). The `semver` crate will reject most real-world trainer version strings                                                                     |
| JSON deserialization of tap metadata | —                                                          | `serde_json` (already in Cargo.toml)                 | **Reuse**                                                                                                                                                                                                                                              |
| SHA-256 hashing                      | —                                                          | `sha2` (already in Cargo.toml)                       | **Reuse** `hash_trainer_file()` from `metadata/version_store.rs`                                                                                                                                                                                       |
| URL validation                       | Custom (4-line `starts_with` check in `community/taps.rs`) | `url` crate                                          | **Reuse existing pattern** — trainer source URLs need the same allow-list (`https://`, `ssh://git@`, `git@`); no new crate needed                                                                                                                      |
| Cache TTL management                 | —                                                          | Existing `external_cache_entries` + `cache_store.rs` | **Reuse** entirely                                                                                                                                                                                                                                     |
| Async runtime                        | —                                                          | `tokio` (already in Cargo.toml)                      | **Reuse**                                                                                                                                                                                                                                              |

**Key finding**: Zero new dependencies required if scoped to the KISS MVP (community-tap-based discovery index with SQLite substring search).

---

## Resolved design decisions (feature-complete)

Aligned with **`feature-spec.md` Decisions** and Option B (`trainer_sources` + `trainer-sources.json`):

1. **`TrainerSourceEntry` shape** — **Rich struct** as in `feature-spec.md`: `name`, `url`, optional `trainer_version`, `game_version`, `notes`, `sha256` (per downloadable target / source line). Not URL-only.

2. **Discovery index publisher** — **Primary: community taps** (git). **Phase B** adds optional **central HTTP** only for fixed endpoints (FLiNG RSS, etc.) via `reqwest` + TLS; no user PII in query keys; rate limits + cache TTL documented in `discovery/client.rs`. No authenticated “CrossHook catalog API” in v1.

3. **Downloading trainer executables** — **No.** CrossHook does not download, store, or proxy trainer binaries. **Open in system browser** only; launch-time integrity remains **`trainer_hash.rs`** + user file path. UI copy must state downloads happen outside the app.

4. **Game-version matching** — **Advisory only** (BR-3 / Decision 4). Use `version_snapshots` + `compute_correlation_status()`; never block import or link opening.

5. **Hashes** — **`community_profiles`/manifest `trainer_sha256`** remains the profile-level trust signal. **`TrainerSourceEntry.sha256`** is an optional per-source hint from the tap author; both may be shown. Discovery does not replace launch-time verification.

6. **`tokenize()` sharing** — **Lift** `tokenize` / `token_hits` (or thin wrappers) to **`crosshook-core/src/text_utils.rs`** (new module) in the same implementation effort that adds Phase B ranking, re-export from `install/discovery.rs` via `use` to avoid duplication. **Acceptance:** single implementation, unit tests in `text_utils` + one install regression test.

---

## Cross-Team Convergence Notes

These findings emerged from coordination with business-analyzer, recommendations-agent, and tech-designer after the initial report was written.

### FTS5 Is Not Available (Confirmed)

`rusqlite` in `Cargo.toml` uses `features = ["bundled"]` only — no `bundled-full`, no FTS5. The tech spec initially proposed `metadata/discovery_index.rs` with FTS5 SQL. This would silently fail at runtime (SQLite would return an error on `CREATE VIRTUAL TABLE ... USING fts5`).

**Verified:** No FTS usage anywhere in `crosshook-core/src/`. The `LIKE`-based alternative is the only correct Phase 1 search approach. FTS5 as a Phase 2 enhancement requires an explicit `rusqlite` feature flag change tracked as its own issue.

### Recommended IPC Command Split (Async/Sync Separation)

The proposed `trainer_discovery_search(query, include_external)` single-command design mixes synchronous SQLite queries with potentially-blocking async HTTP. This forces tap results (fast) to wait on external results (slow, may fail).

**Correct design — two separate commands:**

1. `trainer_discovery_search_taps(query: String)` — synchronous; delegates to `list_community_tap_profiles()` with a `game_name LIKE ?` WHERE clause; returns immediately
2. `trainer_discovery_search_external(query: String, force_refresh: Option<bool>)` — async; follows ProtonDB client pattern; Phase 2 only

The hook loads tap results immediately on mount and fires the external command independently, identical to how `ProtonDbLookupCard` loads after the profile is already visible.

**Phase 1 is complete with command #1 alone.** External search is additive.

### Superseded shortcut (do not use)

An older minimal path (single `download_url` on `CommunityProfileMetadata` + filter on `community_profiles`) is **obsolete**. The **adopted** plan is **Option B**: **`trainer-sources.json`**, **`trainer_sources`**, and **`crosshook-core::discovery/`** per `feature-spec.md` and `shared.md`.
