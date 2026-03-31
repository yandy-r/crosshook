# Pattern Research: offline-trainers

## Architectural Patterns

**Repository / Store pattern** — Feature data lives in a dedicated `*Store` struct with `try_new()`, `open_in_memory()`, and `disabled()` constructors. Each store encapsulates its persistence backend (TOML or SQLite) and exposes named methods; callers never touch the underlying connection or file handle directly.

- Example store: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (`MetadataStore`)
- Example store: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` (`ProfileStore`)

**Feature module layout** — Each domain lives in a directory under `crosshook-core/src/` with a consistent internal split:

- `models.rs` — data types, error enums, constants
- `service.rs` — pure business logic, no Tauri dependency
- `mod.rs` — module re-exports and the `*Store` façade (for metadata-backed features)

Examples: `install/`, `update/`, `community/`, `metadata/`.

**Tauri command layer as a thin consumer** — Commands in `src-tauri/src/commands/` import from `crosshook_core::*`, perform minimal orchestration (prefetch, enrich, convert errors), and return `Result<T, String>`. No business logic lives in command handlers.

- Example: `src-tauri/src/commands/health.rs` — enriches core health reports with metadata
- Example: `src-tauri/src/commands/profile.rs` — thin wrapper calling `ProfileStore` methods

**Tauri state injection** — Stores are registered as Tauri managed state at startup (`lib.rs`) and injected into commands via `State<'_, T>`. The offline-trainers feature must follow this same pattern.

- Registration: `src/crosshook-native/src-tauri/src/lib.rs`

**Fail-soft MetadataStore** — SQLite availability is checked with `metadata_store.is_available()` before each query. If unavailable, commands return empty/default results without error, and the store can be constructed in `disabled()` mode. New trainer-related SQLite tables must follow the same defensive pattern.

- Pattern: `with_conn()` closure in `metadata/mod.rs:89–106`

**`with_conn()` accessor** — All MetadataStore public methods delegate to `self.with_conn(action_label, |conn| { ... })`, which handles lock acquisition and availability check in one place.

**Portable vs. machine-local boundary** (`profile/models.rs`) — CRITICAL for trainer data:

- `portable_profile()` strips all machine-specific paths (including `trainer.path`) before export
- `storage_profile()` moves those paths into `local_override.*` so they survive on the originating machine
- `effective_profile()` merges `local_override` back at runtime
- **Consequence**: any "trainer is activated/downloaded" state must live in SQLite only, never in TOML. TOML is the portable profile; SQLite is the machine-local truth.

**Pre-flight readiness pattern** (`onboarding/readiness.rs`):

- `evaluate_checks(...)` is a pure function that returns `ReadinessCheckResult { checks: Vec<HealthIssue>, all_passed: bool, critical_failures: usize, warnings: usize }`
- Offline readiness pre-flight should return the same `ReadinessCheckResult` type for consistency with onboarding checks
- Callers own I/O; the function receives already-resolved values

**Optimization catalog `OnceLock` pattern** (`launch/catalog.rs`):

- Bundled default catalog (embedded TOML) → optional user override file → merged at startup into a global `OnceLock`
- A trainer-type catalog (mapping publisher name strings to `TrainerType`) follows the same shape: `assets/default_trainer_type_catalog.toml` + optional user override

---

## Code Conventions

**Naming:**

- Rust: `snake_case` for functions, variables, fields, modules; `PascalCase` for types and enum variants
- TypeScript: `camelCase` for hooks/functions, `PascalCase` for types/components
- TypeScript interface fields mirror Rust struct fields exactly (both use `snake_case`)

**Enum serialization:**

- Always `#[serde(rename_all = "snake_case")]` on enums crossing IPC or stored in DB
- Every enum variant has a `pub fn as_str(self) -> &'static str` method for SQLite column values
- Example: `TrainerLoadingMode`, `LaunchOutcome`, `DriftState`, `CacheEntryStatus` in `metadata/models.rs`

**TOML structs:**

- Every field annotated with `#[serde(default)]`
- Optional / empty collections use `#[serde(skip_serializing_if = "Vec::is_empty")]` or `BTreeMap::is_empty`
- Optional single-value fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`
- Example: `profile/models.rs` — `GameProfile`, `TrainerSection`, `LaunchSection`

**IDs and timestamps:**

- UUIDs generated with `uuid::Uuid::new_v4().to_string()` via `db::new_id()`
- Timestamps always RFC3339 via `chrono::Utc::now().to_rfc3339()`

**Defensive storage caps:**

- Named constants (`MAX_<RESOURCE>_PER_PROFILE`, `MAX_CACHE_PAYLOAD_BYTES`) in `metadata/models.rs`
- New features must declare similar caps for any bounded-size column content

**Module exports:**

- `pub use` re-exports in `mod.rs` expose only the public surface; internal store functions stay `pub(super)` or private
- Example: `metadata/mod.rs` — re-exports rows, enums, and constants but not SQL helpers

---

## Error Handling

**Domain-specific error enums, not `anyhow`:**

- Each domain has its own error type: `MetadataStoreError`, `ProfileStoreError`, `InstallGameError`
- Variants carry structured context: `Database { action: &'static str, source: SqlError }`, `Io { action, path, source }`
- Implement `std::error::Error` and `Display` manually; `source()` chains to inner errors
- Example: `metadata/models.rs:8–73`, `profile/toml_store.rs:17–57`, `install/models.rs:48–80`

**SQLite error mapping:**

```rust
conn.execute(...).map_err(|source| MetadataStoreError::Database {
    action: "upsert a trainer hash row",
    source,
})?;
```

The `action` string is always imperative present tense ("upsert a trainer hash row") for readable error messages.

**IPC boundary — all errors become `String`:**

- Every `#[tauri::command]` returns `Result<T, String>`
- Errors converted with `.map_err(|e| e.to_string())`
- A local `fn map_error(e: ProfileStoreError) -> String { e.to_string() }` helper is the convention
- Example: `commands/profile.rs:17–19`

**Fail-fast at startup for critical paths:**

```rust
MetadataStore::try_new().unwrap_or_else(|error| {
    tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
    MetadataStore::disabled()
});
```

**Fail-soft for non-critical metadata enrichment:**

```rust
metadata_store.query_failure_trends(30).unwrap_or_default()
```

All optional metadata lookups use `unwrap_or_default()` or `unwrap_or(None)` — never `?` — so a metadata failure never blocks the core feature.

**Structured logging with `tracing`:**

- `tracing::warn!(%error, field = "value", "human message")` — structured key=value, then message
- `tracing::info!(count = n, "description")` for informational events
- Example throughout `commands/health.rs` and `metadata/mod.rs`

---

## Testing Approach

**Collocated unit tests in same file:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // ...
}
```

Every `models.rs` and `service.rs` has a `tests` module. Tests live in the file they test.

**In-memory SQLite for store tests:**

```rust
let conn = db::open_in_memory().unwrap();
run_migrations(&conn).unwrap();
```

`db::open_in_memory()` is the standard test fixture for anything touching the metadata DB.

- Example: `metadata/migrations.rs:578–592`

**Filesystem tests use `tempfile`:**

```rust
let temp_dir = tempdir().expect("temp dir");
```

`tempfile::tempdir()` is used for any test requiring real disk I/O. Available in `[dev-dependencies]`.

- Example: `install/service.rs:319–482`

**Test fixture helpers as private functions:**

```rust
fn sample_profile() -> GameProfile { ... }
fn valid_request(temp_dir: &Path) -> InstallGameRequest { ... }
```

Fixture constructors are unexported private functions in the test module. Never `pub`.

**Pure functions tested independently:**

- Functions with no I/O (e.g., `compute_correlation_status`, `resolve_launch_method`, `slugify_profile_name`) are tested without any setup
- Example: `metadata/version_store.rs` — `compute_correlation_status` has full unit tests

**Assertion style:**

- `assert_eq!(actual, expected)` for value comparisons
- `assert!(matches!(result, Err(SomeError::Variant { .. })))` for error variant checking
- No third-party assertion libraries

---

## Patterns to Follow for offline-trainers

**TrainerType enum — follow DriftState / LaunchOutcome pattern:**

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainerType {
    Standalone,
    CheatEngine,
    AppBasedSessionCache,
    AppBasedOfflineKey,
    OnlineOnly,
}

impl TrainerType {
    pub fn as_str(self) -> &'static str { ... }
}
```

Declare in `metadata/models.rs` alongside other enums. Serialize as `snake_case` text in SQLite.

**SHA-256 hash — reuse existing infrastructure:**

- `hash_trainer_file(path: &Path) -> Option<String>` already exists at `metadata/version_store.rs:215`
- `sha256_hex(content: &[u8]) -> String` already exported from `metadata/profile_sync`
- No new hashing code needed. Use `hash_trainer_file` for binary files, `sha256_hex` for content.
- **`version_snapshots` already has `trainer_file_hash TEXT`** (migration 8→9). The stat-based hash cache may be able to reuse this column rather than requiring a new table — evaluate before adding `trainer_hash_cache`.

**CPU-heavy operations — `spawn_blocking`:**
Hash computation on large trainer binaries should not block the async runtime:

```rust
tauri::async_runtime::spawn_blocking(move || {
    hash_trainer_file(&trainer_path)
}).await.ok().flatten()
```

The pattern is used in launch commands; apply it to any trainer hash preflight check.

**Offline readiness scoring — extend health check pattern:**

- New `OfflineReadinessScore` struct should mirror `ProfileHealthReport` structure
- `HealthIssue { field, path, message, remediation, severity }` is already the IPC shape for issues
- Integrate as a new check in `check_profile_health()` in `profile/health.rs` by adding trainer-type-aware path checks
- Persist scores to SQLite via a new `trainer_offline_status` table following `health_snapshots` schema

**New SQLite table — follow migration pattern:**

```rust
fn migrate_12_to_13(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS trainer_hash_cache (
            profile_id      TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            trainer_path    TEXT NOT NULL,
            file_hash       TEXT NOT NULL,
            file_size       INTEGER NOT NULL,
            mtime_secs      INTEGER NOT NULL,
            checked_at      TEXT NOT NULL,
            PRIMARY KEY (profile_id)
        );
    ").map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 12 to 13",
        source,
    })?;
    Ok(())
}
```

Add to `migrations.rs`, bump version to 13. Follow the exact `migrate_N_to_N+1` naming.

**Stat-based hash invalidation (mtime + size):**

- Store `mtime_secs` and `file_size` alongside `file_hash` in the hash cache table
- On next check: `stat()` the file; if mtime or size changed, recompute hash; else return cached value
- Returns `Option<String>` — `None` if file unreadable, matches `hash_trainer_file` return type

**IPC command structure:**

```rust
#[tauri::command]
pub fn get_trainer_offline_status(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<TrainerOfflineStatusReport, String> { ... }
```

New command goes in `src-tauri/src/commands/` — either a new `trainer.rs` or added to `health.rs` if status is health-adjacent. Register in `lib.rs` via the same `invoke_handler` call.

**TrainerType on profile model — upgrade existing `kind` field (follow `TrainerLoadingMode` template):**

The existing `kind: String` already serializes as `[trainer]\ntype = "..."` via `#[serde(rename = "type")]`. The correct upgrade path (matching `TrainerLoadingMode`) is to replace the `String` with the enum directly — **not** add a second field:

```rust
// profile/models.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerType {
    #[default]
    Standalone,
    CheatEngine,
    AppBasedSessionCache,
    AppBasedOfflineKey,
    OnlineOnly,
    #[serde(other)]   // forward-compat: unknown values deserialize to Unknown
    Unknown,
}

pub struct TrainerSection {
    pub path: String,
    #[serde(rename = "type", default)]
    pub kind: TrainerType,         // was String — same TOML key, now typed
    #[serde(rename = "loading_mode", default)]
    pub loading_mode: TrainerLoadingMode,
}
```

Use `#[serde(other)]` on an `Unknown` catch-all variant so existing profiles with unrecognized `type` strings (e.g. `type = "fling"`) deserialize without error. Implement `TrainerType::as_str()` for SQLite columns and `FromStr` for CLI/config parsing. `#[serde(default)]` handles missing `type` key.

**`offline_activated` flag is SQLite-only — NEVER in TOML:**

- Download status, activation state, and offline readiness score are machine-local facts
- They must live exclusively in the SQLite metadata DB, never added to `GameProfile` or `TrainerSection`
- TOML encodes the portable profile; SQLite encodes the machine state

**Frontend type mirroring:**
Add to `src/types/health.ts` (or a new `src/types/trainer.ts`):

```typescript
export type TrainerType =
  | 'standalone'
  | 'cheat_engine'
  | 'app_based_session_cache'
  | 'app_based_offline_key'
  | 'online_only';

export interface TrainerOfflineStatusReport {
  profile_name: string;
  trainer_type: TrainerType | null;
  offline_capable: boolean;
  readiness_score: number;
  issues: HealthIssue[];
  checked_at: string;
}
```

All field names must match Rust struct fields exactly. Re-export from `src/types/index.ts`.

**React hook pattern:**
Follow `useProfileHealth.ts` — `useReducer` with typed `HookStatus` (`idle | loading | loaded | error`), `invoke()` for the Tauri command, `listen()` for optional push events.
