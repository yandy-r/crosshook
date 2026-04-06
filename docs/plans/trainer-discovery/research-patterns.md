# Pattern Research: trainer-discovery

Concrete coding patterns and conventions in the CrossHook codebase that apply directly to implementing trainer discovery. Each section cites the actual source files.

---

## Architectural Patterns

**Domain module layout (`mod.rs` + focused subfiles)**: Every domain in `crosshook-core` uses a directory with `mod.rs` for public re-exports and private child modules for implementation.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs` — public re-exports only; `client`, `models`, `aggregation`, `suggestions` are private
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs` — declares each domain module as `pub mod`; discovery needs `pub mod discovery;` added here

**`MetadataStore` wrapper pattern**: All SQLite access goes through `MetadataStore::with_conn` or `with_conn_mut` (immutable/mutable lock) or `with_sqlite_conn` (when the return type does not implement `Default`). No module holds a raw `Connection`.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:97–159` — the three wrapper methods
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:22–165` — `index_community_tap_result` called as a free function, passed `&mut Connection` by the wrapper

**Thin IPC command handlers (~50–100 lines)**: Command files in `src-tauri/src/commands/` contain only `#[tauri::command]` functions. They receive `State<'_, T>` values, delegate to `crosshook-core`, and map errors with `.map_err(|e| e.to_string())`. No business logic.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs:49–57` — `protondb_lookup` is the canonical minimal async command
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs:263–299` — `community_sync` with fan-out loop and `tracing::warn!` fallback

**IPC command signature contract test**: Every commands file ends with a `#[cfg(test)]` block that casts each function to its expected function-pointer type. This verifies the IPC contract compiles without spinning up a Tauri app.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs:311–353` — definitive example; `discovery.rs` must include an equivalent block

**Cache-first fetch with stale fallback**: `lookup_protondb` in `client.rs` is the reference implementation. Check `external_cache_entries` → attempt live fetch → on failure, return stale row from `external_cache_entries` → on cache miss, return `Unavailable` state. Uses `OnceLock<reqwest::Client>` for the HTTP client singleton.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:85–130` — `lookup_protondb`; the full cache-first/stale-fallback flow
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:175–190` — `protondb_http_client()` — the `OnceLock` init pattern to clone for `DISCOVERY_HTTP_CLIENT`

**Watermark-skip indexing with transactional DELETE+INSERT**: `index_community_tap_result` compares `last_head_commit` before re-indexing a tap. If unchanged, returns early. Otherwise runs `DELETE FROM community_profiles WHERE tap_id = ?` then batch `INSERT` inside an `Immediate` transaction.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:22–165`

**Pure-function derivation (no I/O, directly testable)**: `derive_suggestions` in `protondb/suggestions.rs` takes typed values, returns typed values, has no database or network calls. Discovery version matching and name-scoring must follow this pattern.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/suggestions.rs:1–80` — see `SuggestionStatus`, `ProtonDbSuggestionSet`, and the struct definitions that flow into `derive_suggestions`

**Scored candidate ranking with denylist filtering**: `discover_game_executable_candidates` in `install/discovery.rs` uses a `Candidate` struct with `score: i32` and `depth: usize`, denylist term arrays (`SUSPICIOUS_FILE_TERMS`, `SUSPICIOUS_PATH_TERMS`, `SKIP_DIRECTORY_TERMS`), and a bounded scan (`MAX_SCANNED_FILES`, `MAX_RETURNED_CANDIDATES`). The tokenize/token_hits functions from this module are directly reusable for trainer name matching.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/discovery.rs:1–80` — candidate struct and denylist constants

---

## Code Conventions

**Rust naming**: `snake_case` for all functions, modules, variables, fields. `PascalCase` for types, enums, structs. `SCREAMING_SNAKE_CASE` for `const` and `static`. Module files as directories with `mod.rs`. Tauri command names match frontend `invoke()` strings exactly (e.g. `protondb_lookup` ↔ `invoke('protondb_lookup', ...)`).

**Serde conventions on IPC boundary types**:

- Top-level result structs: `#[derive(Debug, Clone, Serialize, Deserialize)]` + `#[serde(rename_all = "camelCase")]` (frontend receives camelCase)
- Optional fields: `#[serde(default, skip_serializing_if = "Option::is_none")]`
- State enums: `#[serde(rename_all = "snake_case")]` with `#[default]` on the idle/unknown variant
- Tagged enum requests (like `AcceptSuggestionRequest`): `#[serde(tag = "kind", rename_all = "snake_case")]`

Example reference: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs:114–149` — `ProtonDbLookupState`, `ProtonDbCacheState`

**Cache key namespacing**: Cache keys in `external_cache_entries` use `namespace:identifier` format. ProtonDB uses `protondb:{app_id}`. Discovery should use `trainer_discovery:game:{steam_app_id}` and `trainer_discovery:search:{query_slug}`.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs:9–21` — `PROTONDB_CACHE_NAMESPACE`, `cache_key_for_app_id`, `normalize_app_id`

**Module public API pattern**: `mod.rs` re-exports only what external callers need. Internal submodules are `mod subname;` (private). Only exported symbols are `pub use subname::Symbol;`.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs` — reference for the discovery `mod.rs` structure

**TypeScript type organization**: Types live in `src/types/` as domain-specific files (`protondb.ts`, `community_schema.ts`). All are barrel-exported from `src/types/index.ts`. New discovery types go in `src/types/discovery.ts` and must be added to `index.ts`.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/index.ts`

**TypeScript IPC invocation**: Always `invoke<ReturnType>('command_name', { paramName })` where param names are camelCase (Tauri converts from the Rust snake_case). Never call `invoke` without a typed return.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbSuggestions.ts:42–46` — canonical form

**`nullable_text` helper**: Before inserting to SQLite, empty strings are stored as `NULL` via `nullable_text(value)` which trims and returns `None` for empty.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:330–337`

**A6 field-length validation before INSERT**: Every string field from external data is byte-checked against named constants (`MAX_GAME_NAME_BYTES`, etc.) in `check_a6_bounds` before insertion. New fields added to `community_profiles` (e.g. `source_url`, `source_name`) must be added to `check_a6_bounds` with their own constants.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:9–16` — constants
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:259–328` — `check_a6_bounds`

---

## Error Handling

**`MetadataStoreError` typed error**: All database errors are wrapped in `MetadataStoreError::Database { action: &'static str, source: SqlError }`. The `action` field is a human-readable past-tense description of what failed. Display is `"failed to {action}: {source}"`.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:8–63`

**IPC error conversion**: Command handlers map all errors to `String` via `.map_err(|e| e.to_string())`. Never return structured error types across the IPC boundary.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs:12–14` — `fn map_error(error: impl ToString) -> String`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs:54–57` — inline `.map_err(|e| e.to_string())`

**`tracing::warn!` for non-fatal failures in fan-out loops**: When looping over results and a single failure should not abort the whole operation (e.g. indexing multiple taps), use `tracing::warn!(%e, field = %value, "description")` and continue.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs:273–280` — canonical `if let Err(e)` + `tracing::warn!` pattern

**Private module-level error enums**: When a module needs internal error categorization (not exposed via IPC), define a private `enum ModuleError` with `impl fmt::Display`. Do not use `anyhow` in library code.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:29–53` — `ProtonDbError`

**Frontend error pattern**: In hooks, catch errors as `unknown`, narrow with `instanceof Error`, and store as `string | null` in state. Never re-throw unless the caller explicitly needs to handle it.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbSuggestions.ts:53–59` — `catch (err)` block

---

## Testing Approach

**Unit tests in `#[cfg(test)]` at bottom of each file**: Tests live adjacent to the code they test. No separate test modules directory. Test helpers are `fn make_entry(...)` factory functions (not fixtures).

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:371–445` — full test block with factory `make_entry` and boundary tests for `check_a6_bounds`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs:243–288` — unit tests for serialization round-trips and pure logic

**In-memory SQLite for store tests**: `MetadataStore::open_in_memory()` is available in `#[cfg(test)]` to test any store function without touching disk.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:70–73` — `open_in_memory` method

**IPC contract tests (compile-time only)**: Each commands file has a `fn command_names_match_expected_ipc_contract()` test that casts each handler to its explicit function-pointer type. This catches parameter renames and type mismatches at compile time.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs:315–353` — complete reference; discovery commands must replicate this pattern

**Test naming convention**: `snake_case` function names that describe the scenario. Format: `{verb}_{subject}_{condition}` — e.g. `rejects_oversized_game_version`, `accepts_exactly_256_byte_version_strings`.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs:411–445`

**Pure-function tests without I/O**: Functions like `check_a6_bounds`, `derive_suggestions`, and `tokenize` are tested directly — never through the IPC boundary or database.

**No frontend test framework**: The repo has no configured Jest/Vitest. Do not add one. Test pure TypeScript utilities (e.g. from `useCommunityProfiles.ts`) if they are extracted to standalone functions, but hooks themselves are not tested.

---

## Patterns to Follow

**For `crosshook-core/src/discovery/` module directory:**

1. `mod.rs` — public re-exports only; mirrors `protondb/mod.rs` structure
2. `models.rs` — all `#[derive(Debug, Clone, Serialize, Deserialize)]` structs + `#[serde(rename_all = "camelCase")]` for IPC types; state enum with `#[serde(rename_all = "snake_case")]` and `#[default]`
3. `search.rs` — SQLite LIKE query builder for Phase 1; pure, testable, no IPC dependency
4. `version_match.rs` (Phase 2) — pure function taking `Option<&str>` inputs, returning a `VersionMatchStatus` enum; tested without I/O

**For `src-tauri/src/commands/discovery.rs`:**

- Follow `commands/protondb.rs` for async pattern (`lookup_protondb` returns `Ok(result)` without `map_err` since the function never errors — returns `Unavailable` state instead)
- Follow `commands/community.rs` for sync pattern with `map_error` helper
- Add `pub mod discovery;` to `commands/mod.rs`
- End the file with the IPC contract test block

**For the MetadataStore integration:**

- Add public methods on `MetadataStore` that delegate to private free functions (e.g. `pub fn search_community_profiles_for_trainer(...)` → `community_index::search_by_query(conn, ...)`)
- New methods use `self.with_conn("verb a noun", |conn| community_index::function(conn, ...))` pattern
- Add any new public types to the `pub use` list in `metadata/mod.rs`

**For `src/hooks/useTrainerDiscovery.ts`:**

- Mirror `useProtonDbSuggestions.ts` exactly: `useState<T | null>`, `useState<boolean>`, `useState<string | null>`, `useRef(0)` for request ID guard
- Guard on early return when required inputs (`gameName`) are missing
- Expose `refresh: () => Promise<void>` that forces `forceRefresh = true`
- Typed return interface exported alongside the hook function

**For `src/types/discovery.ts`:**

- `interface TrainerSearchQuery` with all optional filter fields
- `interface TrainerSearchResult` mirroring the Rust struct field-for-field (camelCase)
- `interface TrainerSearchResponse` with `results`, `totalCount`, `query`
- Add `export * from './discovery';` to `src/types/index.ts`

**For SQL queries:**

- All queries are parameterized with `?1`, `?2`, ... positional binds — never string interpolation
- `LIKE '%' || ?1 || '%'` for substring search (the `%` wrapping stays in the SQL template, not in the user's input)
- Queries end with `LIMIT ?N OFFSET ?M` and use a `MAX_DISCOVERY_RESULTS_PER_PAGE: usize = 50` constant enforced in Rust before binding
- JOIN `community_profiles cp JOIN community_taps ct ON cp.tap_id = ct.tap_id` to get `tap_url` in results

**For scroll containers in new React components:**

- Any new `overflow-y: auto` container must be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts` or it will cause dual-scroll jank
- Inner scroll containers use `overscroll-behavior: contain`

**Validation before any INSERT of external data:**

- URL fields: `https://` prefix only (follow `community::taps::validate_tap_url` pattern)
- Query strings: trim and cap at 512 bytes before passing to SQL
- All string fields: add to `check_a6_bounds` with named byte-limit constants before any `INSERT INTO community_profiles`
