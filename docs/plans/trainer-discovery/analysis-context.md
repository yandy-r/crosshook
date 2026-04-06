# Context Analysis: Trainer Discovery Phase B

## Executive Summary

Phase B adds async FLiNG RSS external trainer lookup on top of the fully-implemented Phase A tap-local search. The core architectural approach clones the `protondb/client.rs` OnceLock HTTP singleton + cache-first fetch pattern verbatim into `discovery/client.rs`, reusing `external_cache_entries` (schema v4) with a `trainer:source:v1:{key}` namespace — no new DB migration needed. Progressive loading ensures Phase A local results render immediately while Phase B external results load asynchronously.

---

## Architecture Context

- **System Structure**: Business logic lives exclusively in `crosshook-core/src/discovery/`. Phase B adds `client.rs` and `matching.rs` to the existing `mod.rs` + `models.rs` + `search.rs` module. `src-tauri/src/commands/discovery.rs` is a thin IPC adapter (~30–50 lines total per command) with no business logic.
- **Data Flow**: Local search (Phase A, sync) fires immediately → external search (Phase B, async) fires on user-triggered "Search Online" action → results merge in `TrainerDiscoveryPanel` with independent loading states. External results are ephemeral — stored only in `external_cache_entries` JSON blob, never in `trainer_sources`.
- **Integration Points**:
  1. `discovery/mod.rs` — add `pub mod client; pub mod matching;`
  2. `commands/discovery.rs` — add two commands alongside existing sync `discovery_search_trainers`
  3. `src-tauri/src/lib.rs:~318` — register new commands in existing `// Trainer discovery` block
  4. `TrainerDiscoveryPanel.tsx` — add external results section, "Search Online" button, trust badges, offline banner
  5. `src/types/discovery.ts` — add `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery`
  6. `useScrollEnhance.ts` `SCROLLABLE` selector — register any new scrollable container (mandatory)

---

## Critical Files Reference

- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: **PRIMARY Phase B template** — OnceLock singleton (line 26), cache-first 3-stage flow (lines 85–130), stale fallback (line 111), `persist_lookup_result` (line 318), `load_cached_lookup_row` with `allow_expired` flag (line 346)
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry` / `put_cache_entry` / `evict_expired_cache_entries` — the only cache API to use
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `lookup_latest_version_snapshot` (line 75), `compute_correlation_status` pure fn (line 185) — for `discovery_check_version_compatibility`
- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: `tokenize` (line 292) and `token_hits` (line 272) — lift to new `text_utils.rs` as part of Phase B
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `with_conn` (returns `T::default()` on unavailable) vs `with_sqlite_conn` (returns `Err`) — choose based on whether unavailability must be surfaced
- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: Async IPC command reference — `.inner().clone()` before `await` pattern (line 55)
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`: Frontend hook reference — `requestIdRef` race guard (line 37), `{ data, loading, error, refresh }` shape
- `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs`: Existing Phase A types; Phase B adds `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery`, `DiscoveryCacheState`
- `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`: Phase A UI; Phase B adds external results section, trust badges, offline banner, progressive loading
- `src/crosshook-native/crates/crosshook-core/Cargo.toml`: Add `quick-xml = { version = "0.36", features = ["serialize"] }` — the only new dependency

---

## Patterns to Follow

- **OnceLock HTTP Singleton**: Create `static TRAINER_DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client>` with `CrossHook/{version}` user-agent — separate from ProtonDB/Steam clients. See `protondb/client.rs:26,175-190`.
- **Cache-First 3-Stage Fetch**: (1) check `external_cache_entries` valid row, (2) live HTTP fetch + persist, (3) on error load stale with `allow_expired=true` → return with `is_stale=true`. See `protondb/client.rs:85-130`.
- **Async IPC Command**: `pub async fn`, `metadata_store.inner().clone()` BEFORE first `.await` (never hold `State<'_>` across await — it is not `Send`). See `commands/protondb.rs:49-57`.
- **IPC Contract Test**: `#[cfg(test)]` function-pointer cast block required for each new command. See `commands/discovery.rs:19-31`.
- **Domain Error Types**: Private `enum TrainerDiscoveryError` with manual `fmt::Display`. No `anyhow` in library code. No `thiserror`. See `protondb/client.rs:29-53`.
- **Serde IPC Boundary**: `#[serde(rename_all = "camelCase")]` on result structs; `#[serde(rename_all = "snake_case")]` on state enums; `#[serde(default, skip_serializing_if = "Option::is_none")]` on optional fields.
- **MetadataStore Facade**: Add public wrapper methods on `MetadataStore` for cache read/write operations. Use `with_conn` for operations where `T: Default` and silent-empty is acceptable. Use `with_sqlite_conn` only for stale-fallback logic that must distinguish unavailable-store from empty-result.
- **Frontend Hook**: `requestIdRef.current` increment before invoke; discard stale responses. `{ data, loading, error, refresh }` return shape. Early-return guard on empty query. `isOffline` derived from payload, not fetched separately. See `useProtonDbSuggestions.ts`.
- **Token Scoring**: Lift `tokenize`/`token_hits` to new `crosshook-core/src/text_utils.rs` (resolved decision) — do not create cross-domain dependency from `discovery/` to `install/`. Only create `text_utils.rs` if two or more modules need it; otherwise duplicate locally.
- **`discovery_check_version_compatibility` is synchronous**: SQLite-only, no await. Needs both `MetadataStore` and `ProfileStore` states to resolve `profile_id` from `profile_name`.

---

## Cross-Cutting Concerns

- **Security — URL Rendering (S5)**: External URLs from Phase B are untrusted. Always use Tauri `open()` plugin; never `<a href>` or `dangerouslySetInnerHTML`. Validate `https://` prefix before rendering. Non-HTTPS shows warning.
- **Security — Cache Poisoning (S3)**: Validate `Content-Type` header is `application/rss+xml` or `text/xml` before parsing. Apply response size abort above 1 MB at HTTP layer before reading full body.
- **Security — Legal (S1)**: Phase B is gated behind `discovery_enabled` setting and first-use consent dialog (Decision 2). No path bypasses this gate.
- **Trust Indicators (S10)**: Two-tier model — Community (tap) results get filled accent badge "Community"; External (FLiNG) results get muted chain-link icon only. Never block link opening based on trust level — informational only.
- **`useScrollEnhance` Registration**: Every new `overflow-y: auto` container in Phase B UI MUST be added to the `SCROLLABLE` selector in `useScrollEnhance.ts`. Missing this causes dual-scroll jank under WebKitGTK. Inner containers also need `overscroll-behavior: contain`.
- **FTS5 is NOT available in Phase B**: `rusqlite` uses `features = ["bundled"]` only. FTS5 SQL silently fails at runtime. Phase B uses LIKE for any SQLite queries.
- **Mutex across await**: Never hold `MetadataStore`'s internal `Arc<Mutex<Connection>>` across an `.await`. Complete cache reads before starting async HTTP fetch.
- **Trainer versions are not semver**: Display version strings as-is. Advisory comparison only in `matching.rs`.
- **Testing**: Pure functions (`tokenize`, `token_hits`, `match_trainer_version`) tested with `#[test]`. Cache operations tested via `MetadataStore::open_in_memory()`. HTTP layer tested with `wiremock`/`httpmock` — no real HTTP calls in tests. No frontend test framework configured.

---

## Parallelization Opportunities

Three independent workstreams can proceed in parallel after `discovery/models.rs` Phase B types are agreed:

1. **Rust core** (`discovery/client.rs` + `discovery/matching.rs` + `text_utils.rs`): Pure Rust — no frontend dependency. Includes `quick-xml` addition to `Cargo.toml`, OnceLock HTTP client, 3-stage cache fetch, RSS parsing, token scoring, advisory version comparison. Unblocks IPC layer.
2. **Tauri IPC layer** (`commands/discovery.rs` additions + `lib.rs` registration): Depends on Phase B Rust types existing in `discovery/models.rs` (can stub with placeholder types). Contract test block must include all three commands.
3. **Frontend** (`useExternalTrainerSearch.ts` + `TrainerDiscoveryPanel.tsx` updates + `types/discovery.ts`): Can start from TypeScript type definitions and mock IPC responses. Does not require Rust compilation. Trust badge CSS variables go in `src/styles/variables.css`.

`text_utils.rs` (extracting `tokenize`/`token_hits`) is a prerequisite for `discovery/matching.rs` but is small (~20 lines) — complete first.

`discovery/models.rs` Phase B type additions are a prerequisite for both the IPC layer and frontend types — complete these first.

---

## Implementation Constraints

- **No new DB migration**: Schema stays at v18 (Phase A). Reuse `external_cache_entries` (v4) with `trainer:source:v1:{key}` namespace.
- **`quick-xml` is the only new crate dependency**: Must be added to `crosshook-core/Cargo.toml` before Phase B compiles. `reqwest` already present with `rustls-tls`; use `.text().await?` not `.json()` for RSS response.
- **FLiNG is the sole external source** (Decision 3): PCGamingWiki is cross-reference only (game name normalization, optional) — not a trainer source, not a hard dependency.
- **`compute_correlation_status()` is NOT directly reusable for trainer version strings**: It compares Steam build IDs and file hashes. Phase B needs new advisory string comparison logic in `discovery/matching.rs`.
- **FLiNG RSS endpoint not live-verified**: `https://flingtrainer.com/category/trainer/feed/` is inferred from WordPress behavior. Verify it returns valid XML before committing. Have `scraper` HTML fallback plan ready if RSS unavailable.
- **FLiNG 403 without User-Agent**: HTTP client must set `CrossHook/{version}` user-agent — same as ProtonDB client.
- **External results are NOT written to `trainer_sources`**: They live only in `external_cache_entries` JSON blob.
- **FLiNG download links are unstable**: Store trainer page URL from RSS `<link>` field only. Never store OneDrive/Google Drive URLs from `<description>` or `<content:encoded>`.
- **`discovery_enabled` gate**: Both new commands must respect the `discovery_enabled` setting — checked by frontend before invoking.
- **Oversized RSS payload → NULL cache**: If RSS response exceeds 512 KiB, `put_cache_entry` stores NULL. Mitigation: truncate to top N items before serializing to JSON.
- **Cache key design choice**: Two options — (a) single `trainer:source:v1:fling_index` key for full RSS parse (1h TTL, simple), or (b) per-game keys `trainer:source:v1:{normalized-game}` (6h TTL). Strategy (a) is simpler and aligns with spec; implement first.

---

## Key Recommendations

- **Start with `discovery/models.rs` Phase B types** — these unblock all three parallel workstreams. Agree on Rust struct shapes and TypeScript equivalents before splitting work.
- **Create `text_utils.rs` as the first file** — small, self-contained, no dependencies. Gets `tokenize`/`token_hits` into their own module and unblocks `matching.rs`.
- **Copy `protondb/client.rs` as `discovery/client.rs` starting point** — the patterns are identical. Rename constants, change the static name, swap the URL and XML parsing. Reduces risk and ensures consistency.
- **`discovery_check_version_compatibility` can be a separate PR**: Lowest-risk Phase B task, no HTTP dependency. Implement after `discovery/matching.rs` has the advisory version comparison function.
- **Test the IPC contract cast block before frontend wiring**: The compile-time cast block catches signature mismatches early.
- **Verify FLiNG RSS endpoint early**: Manual `curl` with User-Agent before writing the XML parser avoids building against a 403 or HTML captcha response.
- **FLiNG RSS title strip**: Titles follow `"Game Name (+N Trainer)"` or `"Game Name vX.Y (+N Trainer)"` — strip from last `v` or `Trainer` occurrence. Handle both formats.
