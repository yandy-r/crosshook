# Pattern Research: Trainer Discovery Phase B

Concrete coding patterns and conventions in the CrossHook codebase that apply directly to implementing Phase B of trainer discovery. Each section cites the actual source file and line numbers observed during research.

---

## HTTP Client Singleton Pattern

**Source**: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`

A `static OnceLock<reqwest::Client>` holds the singleton. A private helper function initializes it lazily on first call and returns a `&'static` reference. The `OnceLock::set` result is intentionally discarded (`let _ = ...`) because a concurrent race to initialize is benign — `OnceLock::get()` always returns the winner.

```
// client.rs:26
static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

// client.rs:175–190 — the init helper
fn protondb_http_client() -> Result<&'static reqwest::Client, ProtonDbError> {
    if let Some(client) = PROTONDB_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(ProtonDbError::Network)?;

    let _ = PROTONDB_HTTP_CLIENT.set(client);
    Ok(PROTONDB_HTTP_CLIENT
        .get()
        .expect("HTTP client should be initialized before use"))
}
```

Key decisions:

- `timeout` is set at build time via a named constant (`REQUEST_TIMEOUT_SECS: u64 = 6`).
- `user_agent` includes the crate version via `env!("CARGO_PKG_VERSION")`.
- Build failure maps to the domain error type via `.map_err(ProtonDbError::Network)`.
- The `.get().expect(...)` after `set` is the only `expect` in the path — it is safe because `set` either succeeded or raced with another successful set.

For `discovery/client.rs`, clone this pattern exactly with a `DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client>` static and a `discovery_http_client()` private helper. Use a separate timeout constant (`DISCOVERY_REQUEST_TIMEOUT_SECS`).

---

## Cache-First Fetch Pattern

**Source**: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:85–130`

The `lookup_protondb` public function is the canonical template. The flow has four branches:

1. **Validate input** — `normalize_app_id` returns `None` for whitespace-only inputs; returns `Default` immediately.
2. **Check valid cache** — `load_cached_lookup_row(store, key, allow_expired=false)` returns the cache row only if `expires_at > now`.
3. **Fetch live** — `fetch_live_lookup(app_id).await` is called only on cache miss or `force_refresh=true`. On success, `attach_cache_state` sets `from_cache=false` and `persist_lookup_result` writes to `external_cache_entries`.
4. **Stale fallback** — On live fetch error, `load_cached_lookup_row(store, key, allow_expired=true)` ignores `expires_at`. If a stale row exists, it is returned with `state = Stale` and `is_offline = true`. If no row exists at all, `state = Unavailable` is returned.

Error on live fetch is logged with `tracing::warn!(app_id, %error, "message")` before falling back — never silenced.

The `external_cache_entries` table is used for all remote data cache. Cache keys follow `namespace:identifier` format (e.g. `protondb:1245620`). For discovery, use `trainer_discovery:fling:{slug}`.

Relevant files:

- `protondb/client.rs:85–130` — `lookup_protondb` function body
- `protondb/client.rs:318–344` — `persist_lookup_result` (serializes to JSON, calls `metadata_store.put_cache_entry`)
- `protondb/client.rs:346–394` — `load_cached_lookup_row` (uses `with_sqlite_conn`, handles `allow_expired` flag)
- `metadata/cache_store.rs:29–88` — `put_cache_entry` uses `ON CONFLICT(cache_key) DO UPDATE SET ...`

---

## Domain Error Types

**Source**: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs:29–53`

Private error enums are used inside client modules for internal error categorization. They are never exposed at the IPC boundary.

```rust
#[derive(Debug)]
enum ProtonDbError {
    NotFound,
    HashResolutionFailed,
    Network(reqwest::Error),
    InvalidAppId(String),
    InvalidTimestamp(i64),
}

impl fmt::Display for ProtonDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "ProtonDB summary not found for this Steam App ID"),
            Self::HashResolutionFailed => write!(f, "ProtonDB report feed hash could not be resolved"),
            Self::Network(error) => write!(f, "network error: {error}"),
            Self::InvalidAppId(id) => write!(f, "app ID {id:?} cannot be used for a report feed lookup"),
            Self::InvalidTimestamp(ts) => write!(f, "ProtonDB counts timestamp {ts} is not positive"),
        }
    }
}
```

For `discovery/client.rs`, define a private `DiscoveryError` enum with at minimum:

- `Network(reqwest::Error)` — HTTP transport errors
- `ParseError(String)` — RSS/XML parsing failures
- `RateLimited` — HTTP 429 response
- `NotFound` — resource absent at expected URL

Do **not** use `anyhow` in library (`crosshook-core`) code. Do **not** derive `thiserror` — implement `fmt::Display` manually to match the existing style.

---

## Serde Conventions

**Source**: `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs` and `src/crosshook-native/crates/crosshook-core/src/discovery/models.rs`

**IPC result structs** (sent to frontend):

- `#[derive(Debug, Clone, Serialize, Deserialize)]`
- `#[serde(rename_all = "camelCase")]` — frontend receives camelCase
- Optional fields: `#[serde(default, skip_serializing_if = "Option::is_none")]`
- Empty string fields: `#[serde(default, skip_serializing_if = "String::is_empty")]`
- Vec fields: `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
- Bool fields with `#[serde(default)]` — no `skip_serializing_if` (booleans are always serialized)

**State enums** (e.g. `ProtonDbLookupState`, `VersionMatchStatus`):

- `#[serde(rename_all = "snake_case")]`
- `#[default]` on the idle/unknown variant
- Example: `protondb/models.rs:114–123`, `discovery/models.rs:100–108`

**Custom Deserialize for string-enum types**: When remote APIs return string values not matching Rust enum variants, use a custom `Visitor` impl (see `ProtonDbTier` at `protondb/models.rs:79–111`). For discovery, FLiNG version strings are plain strings and don't need custom visitors — store them as `Option<String>`.

**Internal-only structs** (never serialized to IPC boundary):

- No `Serialize` / `Deserialize` derives — e.g. `TrainerSourceRow` at `discovery/models.rs:32–48`
- These are row-mapping types for SQLite results only

**Cache key format** (from `protondb/models.rs:9–21`):

```rust
pub const PROTONDB_CACHE_NAMESPACE: &str = "protondb";
pub fn cache_key_for_app_id(app_id: &str) -> String {
    format!("{PROTONDB_CACHE_NAMESPACE}:{}", app_id.trim())
}
```

Discovery cache keys should follow `trainer_discovery:fling:{slug}` or `trainer_discovery:rss:{feed_url_hash}`.

**TypeScript side** (`src/crosshook-native/src/types/discovery.ts`):

- Field names are camelCase, matching the Rust `rename_all = "camelCase"` output
- `VersionMatchStatus` is a string literal union: `'exact' | 'compatible' | 'newer_available' | 'outdated' | 'unknown'`
- Internal row types (`TrainerSourceRow`) have no TypeScript counterpart — they never cross IPC

---

## Async IPC Command Pattern

**Source**: `src/crosshook-native/src-tauri/src/commands/protondb.rs:49–57`

The canonical async command pattern:

```rust
#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}
```

Critical: `metadata_store.inner().clone()` extracts the `MetadataStore` from the `State` wrapper **before** the `await`. `State<'_, T>` is not `Send` and cannot cross `await` points. The clone is cheap because `MetadataStore` holds an `Arc<Mutex<Connection>>` internally.

For `discovery_search_external` (Phase B async command):

```rust
#[tauri::command]
pub async fn discovery_search_external(
    query: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ExternalTrainerSearchResponse, String> {
    let metadata_store = metadata_store.inner().clone();
    discovery::fetch_external_results(&metadata_store, &query)
        .await
        .map_err(|e| e.to_string())
}
```

For sync commands (no `await`), the `inner().clone()` is not needed — `State<'_, T>` can be used directly in sync context. See `commands/discovery.rs:5–17` for the existing sync command.

---

## Contract Test Pattern

**Source**: `src/crosshook-native/src-tauri/src/commands/community.rs:310–353` and `src/crosshook-native/src-tauri/src/commands/discovery.rs:19–31`

Every commands file ends with a `#[cfg(test)]` block that casts each `#[tauri::command]` function to its exact function-pointer type. This is a **compile-time IPC contract test** — it verifies the Tauri command signature matches what the frontend invokes, without running a Tauri instance.

Minimal example from `discovery.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_match_expected_ipc_contract() {
        let _ = discovery_search_trainers
            as fn(
                TrainerSearchQuery,
                State<'_, MetadataStore>,
            ) -> Result<TrainerSearchResponse, String>;
    }
}
```

For Phase B, two new commands must be added to this block:

```rust
let _ = discovery_search_external
    as fn(
        String,
        State<'_, MetadataStore>,
    ) -> Result<ExternalTrainerSearchResponse, String>;

let _ = discovery_check_version_compatibility
    as fn(
        VersionCompatibilityQuery,
        State<'_, MetadataStore>,
    ) -> Result<VersionMatchResult, String>;
```

Async commands use the same cast syntax — Rust's type system treats `async fn` as returning `impl Future`, which satisfies the function pointer type when the signature matches.

---

## Token Scoring Pattern

**Source**: `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs:272–303`

Two small pure functions perform all text matching:

```rust
fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.len() >= 2 {
                Some(token)
            } else {
                None
            }
        })
        .collect()
}

fn token_hits(value: &str, target_tokens: &[String]) -> usize {
    target_tokens
        .iter()
        .filter(|token| value.contains(token.as_str()))
        .count()
}
```

`tokenize` splits on any non-ASCII-alphanumeric character, lowercases, and drops tokens shorter than 2 chars. `token_hits` counts how many target tokens appear as substrings of the candidate string.

These functions are currently private to `install/discovery.rs`. For Phase B matching (`discovery/matching.rs`), they should be **lifted to a shared location** or duplicated with attribution rather than made `pub(crate)` from the install module (which would create a cross-domain dependency). Options:

1. Duplicate to `discovery/matching.rs` (preferred — no cross-domain coupling)
2. Extract to `crosshook-core/src/text_utils.rs` as `pub(crate)` (only if three or more modules need them)

The scoring heuristic in `score_candidate` (`install/discovery.rs:214–270`) shows the weight structure:

- Stem token hit: `+40 + (hits * 12)`
- Path segment token hits: `+(hits * 4)`
- Suspicious file terms: `-120`
- Installer hint match: `-150`

For trainer name matching in Phase B, adapt the same approach: assign positive weight to query token hits against `game_name`/`source_name`, and filter results scoring below a threshold.

---

## Version Comparison Pattern

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs:178–211`

`compute_correlation_status` is a pure function — no I/O, no database calls, fully testable in isolation:

```rust
pub fn compute_correlation_status(
    current_build_id: &str,
    snapshot_build_id: Option<&str>,
    current_trainer_hash: Option<&str>,
    snapshot_trainer_hash: Option<&str>,
    state_flags: Option<u32>,
) -> VersionCorrelationStatus {
    if let Some(flags) = state_flags {
        if flags != 4 {
            return VersionCorrelationStatus::UpdateInProgress;
        }
    }

    let Some(snapshot_build) = snapshot_build_id else {
        return VersionCorrelationStatus::Untracked;
    };

    let build_changed = current_build_id != snapshot_build;
    let trainer_changed = current_trainer_hash != snapshot_trainer_hash;

    match (build_changed, trainer_changed) {
        (true, true) => VersionCorrelationStatus::BothChanged,
        (true, false) => VersionCorrelationStatus::GameUpdated,
        (false, true) => VersionCorrelationStatus::TrainerChanged,
        (false, false) => VersionCorrelationStatus::Matched,
    }
}
```

For Phase B `discovery/matching.rs`, the advisory version matching function follows this exact template:

- Pure function, takes `Option<&str>` for all optional inputs
- Returns a `VersionMatchStatus` enum variant (already defined at `discovery/models.rs:99–108`)
- No network or database calls — those happen in the caller
- The function compares the trainer's `game_version` field (from FLiNG RSS/manifest) against the installed game's Steam build ID or known version string

Example signature to implement:

```rust
pub fn match_trainer_version(
    trainer_game_version: Option<&str>,
    installed_build_id: Option<&str>,
    installed_human_version: Option<&str>,
) -> VersionMatchResult
```

---

## MetadataStore Facade Pattern

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:98–160`

Three accessor methods exist on `MetadataStore`, each wrapping `Arc<Mutex<Connection>>`:

| Method             | Visibility     | Return constraint     | Use when                                                                      |
| ------------------ | -------------- | --------------------- | ----------------------------------------------------------------------------- |
| `with_conn`        | `fn` (private) | `T: Default`          | Standard read/write — returns `T::default()` when DB unavailable              |
| `with_conn_mut`    | `fn` (private) | `T: Default`          | Write operations needing `&mut Connection` for transactions                   |
| `with_sqlite_conn` | `pub fn`       | No `Default` required | External callers (tests, other modules) that cannot afford a default fallback |

The difference between `with_conn` and `with_sqlite_conn`:

- `with_conn` silently returns `T::default()` when the store is `!available` — appropriate for optional metadata reads
- `with_sqlite_conn` returns `Err(MetadataStoreError::Corrupt("unavailable"))` when `!available` — appropriate when unavailability must be surfaced to the caller

All public `MetadataStore` methods are thin wrappers:

```rust
pub fn search_trainer_sources(
    &self,
    query: &str,
    limit: i64,
    offset: i64,
) -> Result<TrainerSearchResponse, MetadataStoreError> {
    self.with_conn("search trainer sources", |conn| {
        crate::discovery::search_trainer_sources(conn, query, limit, offset)
    })
}
```

The `action` string passed to `with_conn` is a human-readable description used in the error message: `"failed to {action}: {source}"`.

For Phase B, add these public methods to `MetadataStore`:

- `pub fn get_discovery_cache_entry(key: &str)` → delegates to `cache_store::get_cache_entry`
- `pub fn put_discovery_cache_entry(...)` → delegates to `cache_store::put_cache_entry`
- `pub fn search_external_trainer_results(query: &str, limit: i64, offset: i64)` → delegates to `discovery::search_external_results`

Use `with_conn` (not `with_sqlite_conn`) for these — they return types implementing `Default` (`Option<String>`, `TrainerSearchResponse`).

---

## Frontend Hook Patterns

**Source**: `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` and `src/crosshook-native/src/hooks/useTrainerDiscovery.ts`

**State shape** (standard across all async hooks):

```typescript
const [data, setData] = useState<T | null>(null);
const [loading, setLoading] = useState(false);
const [error, setError] = useState<string | null>(null);
const requestIdRef = useRef(0);
```

**Request ID guard** (prevents stale responses from overwriting newer ones):

```typescript
const id = ++requestIdRef.current;
// ... await invoke(...)
if (requestIdRef.current !== id) {
  return; // stale — a newer request superseded this one
}
```

**Return shape** (exported interface alongside the hook):

```typescript
export interface UseExternalTrainerSearchReturn {
  data: ExternalTrainerSearchResponse | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}
```

**Debounce** (`useTrainerDiscovery.ts:26` and `:72–95`):

```typescript
const debounceTimerRef = useRef<ReturnType<typeof setTimeout>>();
// In useEffect:
debounceTimerRef.current = setTimeout(() => {
  void fetchResults(query);
}, 300);
return () => clearTimeout(debounceTimerRef.current);
```

**Cache state tracking**: `useProtonDbSuggestions.ts` does not track cache state in the hook — that data comes back in the result payload. The hook returns the full typed response and lets components inspect `result.cache.isStale`. Phase B external search hook should follow the same convention.

**IPC invocation** (from `useProtonDbSuggestions.ts:42–46`):

```typescript
const result = await invoke<ExternalTrainerSearchResponse>('discovery_search_external', {
  query,
  limit: options?.limit,
  offset: options?.offset,
});
```

**Early return guard**:

```typescript
if (!query.trim()) {
  requestIdRef.current += 1; // invalidate any in-flight request
  setData(null);
  setLoading(false);
  setError(null);
  return;
}
```

**Error narrowing** (TypeScript):

```typescript
} catch (err) {
  setError(err instanceof Error ? err.message : String(err));
  setData(null);
}
```

**Offline banner / progressive loading**: For Phase B the hook should expose an additional field `isOffline: boolean` derived from `data?.cacheState?.isOffline ?? false`. This lets the UI show the offline banner without parsing the payload itself.

---

## Architectural Patterns

**Domain module layout**: Every domain in `crosshook-core` uses `mod.rs` for public re-exports only; implementation is in private child files. `discovery/mod.rs` already exists at `crates/crosshook-core/src/discovery/mod.rs` with `pub mod models; pub mod search;` and re-exports.

**Thin IPC command handlers**: Command files in `src-tauri/src/commands/` are `~30–100 lines` total. No business logic — delegate to `crosshook-core`, map errors. New Phase B commands go in the existing `commands/discovery.rs`.

**Tauri `invoke` name → Rust function name**: They must match exactly. `protondb_lookup` in Rust ↔ `invoke('protondb_lookup', ...)` in TypeScript. For Phase B: `discovery_search_external` ↔ `invoke('discovery_search_external', ...)`.

**`tracing::warn!` for non-fatal failures**: When an operation can degrade gracefully, log and continue:

```rust
if let Err(e) = some_optional_step() {
    tracing::warn!(%e, field = %value, "description of what failed and why it is non-fatal");
}
```

Do not use `eprintln!` or `println!` anywhere in library or command code.

---

## Patterns to Follow (Phase B Task Mapping)

| Phase B Task                                 | Pattern to Use                                                           |
| -------------------------------------------- | ------------------------------------------------------------------------ |
| `discovery/client.rs` — HTTP client          | HTTP Client Singleton Pattern (OnceLock)                                 |
| `discovery/client.rs` — FLiNG RSS fetch      | Cache-First Fetch Pattern                                                |
| `discovery/client.rs` — error handling       | Domain Error Types (private enum + Display)                              |
| `discovery/models.rs` — Phase B types        | Serde Conventions (camelCase structs, snake_case enums)                  |
| `commands/discovery.rs` — new async commands | Async IPC Command Pattern (.inner().clone() before await)                |
| `commands/discovery.rs` — contract test      | Contract Test Pattern (function-pointer cast block)                      |
| `discovery/matching.rs` — name scoring       | Token Scoring Pattern (tokenize + token_hits)                            |
| `discovery/matching.rs` — version advisory   | Version Comparison Pattern (pure function, Option<&str> inputs)          |
| `MetadataStore` — cache read/write           | MetadataStore Facade Pattern (with_conn wrapper)                         |
| `useExternalTrainerSearch.ts` hook           | Frontend Hook Patterns (requestIdRef, { data, loading, error, refresh }) |

**Relevant Files (all absolute paths)**:

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — HTTP singleton + cache-first template
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs` — Serde conventions reference
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` — Phase A/B model types (already defined)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs` — existing discovery module
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/discovery/search.rs` — Phase A search impl
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/discovery.rs` — tokenize + token_hits source
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` — compute_correlation_status pure function
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — with_conn / with_sqlite_conn accessors
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — put_cache_entry / get_cache_entry
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs` — async IPC command reference
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs` — sync IPC command + contract test reference
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/discovery.rs` — existing Phase A command + contract test
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` — hook state pattern reference
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useTrainerDiscovery.ts` — Phase A hook (debounce + requestIdRef)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/discovery.ts` — existing frontend types (VersionMatchResult already defined)
