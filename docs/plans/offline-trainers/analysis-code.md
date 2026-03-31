# Offline-Trainers: Code Analysis

Comprehensive code-pattern analysis for implementing offline trainer management in CrossHook.
All patterns extracted from direct source reading — every example is real, not inferred.

---

## Verified Answers to Open Questions (from context-synthesizer)

| Question                                                         | Answer                                                                                                                                                                                                                            |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `catalog.rs`: `include_str!` + `OnceLock` or `lazy_static`?      | **`include_str!` + `OnceLock`** — confirmed, no `lazy_static` dependency                                                                                                                                                          |
| `with_conn()` disabled: returns `Ok(T::default())` or `Err`?     | **`Ok(T::default())`** — both `with_conn` and `with_conn_mut` check `!self.available` first                                                                                                                                       |
| `AppSettingsData`: `#[serde(default)]` at struct or field level? | **Struct level** (`#[serde(default)]` on the struct, `Default` derive). Adding `offline_mode: bool` just works — `bool::default()` = `false`                                                                                      |
| `community_taps.last_indexed_at` column exists?                  | **Yes** — declared in `migrate_3_to_4`, `last_indexed_at TEXT` nullable                                                                                                                                                           |
| `hash_trainer_file()`: full read or streaming?                   | **Full read** — `std::fs::read(path).ok()?` reads entire file into memory. No streaming path exists                                                                                                                               |
| SQLite DB actual path (research-security.md discrepancy)?        | **`~/.local/share/crosshook/metadata.db`** — `MetadataStore::try_new()` uses `BaseDirs::data_local_dir()`, not `config_dir()`. `paths.rs` only handles script resolution, not DB path                                             |
| `invoke_handler!` pattern for new offline commands?              | **Flat list** in `tauri::generate_handler![..., commands::offline::get_offline_readiness, ...]` — see lib.rs lines 170-249                                                                                                        |
| `offline/` module: new crosshook-core module vs extend existing? | **New `offline/` module is correct** — `lib.rs` startup already calls `commands::health::build_enriched_health_summary` directly; `offline/mod.rs` in core keeps offline scoring logic cohesive. Feature-spec.md is authoritative |

---

## Executive Summary

The offline-trainers feature integrates across four well-separated layers: (1) SQLite metadata
via `MetadataStore`, (2) TOML profile data via `GameProfile`/`ProfileStore`, (3) Tauri IPC
commands, and (4) React hooks. Every layer has a clear, repeatable pattern. The most critical
insight is that all new SQLite logic belongs in dedicated store modules (like `health_store.rs`)
called through `MetadataStore::with_conn`, migration 13 must follow the existing sequential
runner pattern, and the TOML trainer type catalog mirrors `catalog.rs` exactly — embedded
`include_str!` asset + `parse_catalog_toml` + `merge_catalogs` + `OnceLock` global.

---

## Existing Code Structure

```
metadata/
  mod.rs            — MetadataStore facade, with_conn/with_conn_mut pattern
  migrations.rs     — Sequential PRAGMA user_version runner (currently v12)
  models.rs         — All shared enums/structs/constants
  health_store.rs   — upsert_health_snapshot / load_health_snapshots (PATTERN FILE)
  version_store.rs  — hash_trainer_file(), upsert_version_snapshot(), compute_correlation_status()
  cache_store.rs    — put_cache_entry() with MAX_CACHE_PAYLOAD_BYTES size cap (PATTERN FILE)
  profile_sync.rs   — sha256_hex() utility; observe_profile_write/rename/delete

profile/
  models.rs         — GameProfile, TrainerSection (kind: String + loading_mode), LocalOverrideSection
  health.rs         — check_profile_health(), batch_check_health(), HealthIssue, HealthStatus
  toml_store.rs     — ProfileStore CRUD; save() is downstream hash event trigger

launch/
  catalog.rs        — TOML catalog: DEFAULT_CATALOG_TOML embed + parse + merge + OnceLock global

onboarding/
  mod.rs            — ReadinessCheckResult struct { checks, all_passed, critical_failures, warnings }
  readiness.rs      — check_system_readiness() / evaluate_checks() (PATTERN FILE)

settings/
  mod.rs            — AppSettingsData: add offline_mode: bool here

src-tauri/commands/
  health.rs         — batch_validate_profiles, get_profile_health, get_cached_health_snapshots (PATTERN FILE)
  launch.rs         — launch_game / launch_trainer (preflight integration point)

src/hooks/
  useProfileHealth.ts — useReducer + HookStatus + invoke() + listen() (PATTERN FILE)
```

---

## Implementation Patterns

### 1. MetadataStore `with_conn` Dispatch Pattern

Every store method is a thin `with_conn` wrapper. Returns `T::default()` when unavailable.

```rust
// From metadata/mod.rs:89-106
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,
{
    if !self.available {
        return Ok(T::default()); // fail-soft: returns Default, not Err
    }
    let guard = conn.lock().map_err(|_| MetadataStoreError::Corrupt(...))?;
    f(&guard)
}

// Usage (from mod.rs:134-136):
pub fn observe_profile_write(...) -> Result<(), MetadataStoreError> {
    self.with_conn("observe a profile write", |conn| {
        profile_sync::observe_profile_write(conn, ...)
    })
}
```

**New offline store methods follow exactly this pattern.** The `offline_store.rs` module
exposes free functions taking `&Connection`; `MetadataStore` wraps them with `with_conn`.

---

### 2. Sequential Migration Pattern

```rust
// From metadata/migrations.rs:4-121
pub fn run_migrations(conn: &Connection) -> Result<(), MetadataStoreError> {
    let version = conn.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))...;

    if version < 12 {
        migrate_11_to_12(conn)?;
        conn.pragma_update(None, "user_version", 12_u32)...?;
    }
    Ok(())
}

fn migrate_11_to_12(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS optimization_catalog (...);")
        .map_err(|source| MetadataStoreError::Database {
            action: "run metadata migration 11 to 12",
            source,
        })?;
    Ok(())
}
```

**Migration 13 adds:** `trainer_hash_cache`, `offline_readiness_snapshots`, `community_tap_offline_state`.

Important: `migrate_8_to_9` creates `version_snapshots` — bootstrap `trainer_hash_cache` from
`version_snapshots.trainer_file_hash` can be done in migration 13 with a single INSERT SELECT.

---

### 3. Enum Pattern (`as_str` + serde)

```rust
// From metadata/models.rs:103-121 (LaunchOutcome) and profile/models.rs:51-77 (TrainerLoadingMode)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerLoadingMode {
    #[default]
    SourceDirectory,
    CopyToPrefix,
}
impl TrainerLoadingMode {
    pub fn as_str(self) -> &'static str {
        match self { Self::SourceDirectory => "source_directory", ... }
    }
}
```

`OfflineCapability` and `TrainerType` follow this exactly. Both need `#[default]` variants
(`OfflineCapability::Unknown`, `TrainerType::Unknown`).

---

### 4. Health Store Pattern (direct model for `offline_store.rs`)

```rust
// From metadata/health_store.rs:13-105
pub fn upsert_health_snapshot(conn, profile_id, status, issue_count, checked_at)
    -> Result<(), MetadataStoreError>
{
    // Range-check the count first (Validation error if overflow)
    let checked = i64::try_from(issue_count).map_err(|_| MetadataStoreError::Validation(...))?;
    conn.execute("INSERT OR REPLACE INTO health_snapshots ...", params![...])
        .map_err(|source| MetadataStoreError::Database {
            action: "upsert a health snapshot row",
            source,
        })?;
    Ok(())
}

pub fn load_health_snapshots(conn) -> Result<Vec<HealthSnapshotRow>, MetadataStoreError> {
    // JOIN with profiles, filter deleted_at IS NULL
    let mut stmt = conn.prepare("SELECT ... FROM health_snapshots hs
        INNER JOIN profiles p ON hs.profile_id = p.profile_id
        WHERE p.deleted_at IS NULL")...?;
    stmt.query_map([], |row| Ok(HealthSnapshotRow {...}))...
        .collect::<Result<Vec<_>, _>>()...
}
```

`offline_readiness_snapshots` maps to `upsert_offline_readiness_snapshot` and
`load_offline_readiness_snapshots` — same `INSERT OR REPLACE` + `JOIN profiles` pattern.

---

### 5. `hash_trainer_file` and SHA-256 Utilities

```rust
// From metadata/version_store.rs:215-223
pub fn hash_trainer_file(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;      // returns None on any I/O error
    let digest = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest { let _ = write!(hex, "{byte:02x}"); }
    Some(hex)
}
```

The new `trainer_hash_cache` store uses this directly. **Stat-based invalidation** compares
`file_modified_at` and `file_size` before calling `hash_trainer_file` — only re-hash when
either changes. The `sha256_hex()` utility in `profile_sync.rs` is the same pattern, reusable
for content-addressing non-trainer data.

---

### 6. Data-Driven TOML Catalog (`catalog.rs` — exact template)

```rust
// From launch/catalog.rs
pub const DEFAULT_CATALOG_TOML: &str = include_str!("../../../../assets/default_optimization_catalog.toml");

static GLOBAL_CATALOG: OnceLock<OptimizationCatalog> = OnceLock::new();

pub fn load_catalog(user_config_dir: Option<&Path>, tap_catalog_texts: &[(&str, &str)]) -> OptimizationCatalog {
    let (default_entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
    // merge taps (force community=true), then user file
    OptimizationCatalog::from_entries(merged)
}

pub fn global_catalog() -> &'static OptimizationCatalog {
    GLOBAL_CATALOG.get_or_init(|| { /* fallback to embedded default */ })
}
```

Trainer type catalog at `assets/default_trainer_type_catalog.toml` follows this exactly.
Key difference: TOML key is `[[trainer_type]]` instead of `[[optimization]]`; struct is
`TrainerTypeEntry { id, display_name, offline_capability, requires_network, detection_hints }`.
Valid categories for trainer types: `["executable", "overlay", "injection", "web_overlay"]`.

---

### 7. Version Snapshot Pruning (bounded row count)

```rust
// From metadata/version_store.rs:51-63
tx.execute(
    "DELETE FROM version_snapshots WHERE profile_id = ?1
     AND id NOT IN (SELECT id FROM version_snapshots WHERE profile_id = ?1
                   ORDER BY checked_at DESC LIMIT ?2)",
    params![profile_id, MAX_VERSION_SNAPSHOTS_PER_PROFILE as i64],
)?;
```

`trainer_hash_cache` has no explicit row cap (1:1 per profile+path), but `verified_at` staleness
check replaces the version_snapshot prune pattern. Use `ON CONFLICT(profile_id, file_path) DO UPDATE`
to upsert without unbounded row growth.

---

### 8. Health Check `HealthIssue` Accumulation Pattern

```rust
// From profile/health.rs:337-443 (check_profile_health)
pub fn check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthReport {
    let mut issues: Vec<HealthIssue> = Vec::new();
    let mut has_stale = false;
    let mut has_broken = false;

    let mut required_results = Vec::new();
    required_results.push(check_required_file("game.executable_path", ...));
    // ...method-specific checks...
    for result in required_results {
        if let Some((issue, stale)) = result { issues.push(issue); ... }
    }
    // roll up status
    let status = if has_broken { Broken } else if has_stale { Stale } else { Healthy };
    ProfileHealthReport { name, status, issues, checked_at: Utc::now().to_rfc3339() }
}
```

The offline readiness pre-flight check returns a `ReadinessCheckResult` (same `Vec<HealthIssue>`)
and injects a `HealthIssue { field: "offline_readiness", severity: Warning }` into the existing
`check_profile_health` output when the score is below threshold.

---

### 9. `ReadinessCheckResult` Pattern (for offline pre-flight)

```rust
// From onboarding/mod.rs:11-16
pub struct ReadinessCheckResult {
    pub checks: Vec<HealthIssue>,
    pub all_passed: bool,
    pub critical_failures: usize,
    pub warnings: usize,
}
```

```rust
// From onboarding/readiness.rs:38-136
fn evaluate_checks(steam_roots, proton_tools) -> ReadinessCheckResult {
    let mut checks: Vec<HealthIssue> = Vec::new();
    // Each check: push HealthIssue { field, path, message, remediation, severity }
    let critical_failures = checks.iter().filter(|c| matches!(c.severity, Error)).count();
    let warnings = checks.iter().filter(|c| matches!(c.severity, Warning)).count();
    ReadinessCheckResult { checks, all_passed: critical_failures == 0 && warnings == 0, ... }
}
```

`check_offline_preflight(profile_id, conn)` follows this: one `HealthIssue` per check
(trainer_present, hash_valid, proton_available, etc.), returns `ReadinessCheckResult`.

---

### 10. Tauri IPC Command Pattern

```rust
// From src-tauri/commands/health.rs:436-442
#[tauri::command]
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<EnrichedHealthSummary, String> {
    Ok(build_enriched_health_summary(&store, &metadata_store))
}
```

- Return type always `Result<T, String>`, converting with `.map_err(|e| e.to_string())`
- `State<'_, T>` for each Tauri-registered singleton
- CPU-intensive or async work uses `tauri::async_runtime::spawn_blocking`
- Path strings sanitized via `sanitize_display_path()` before IPC (replaces home dir with `~`)

---

### 11. React Hook Pattern (`useProfileHealth.ts`)

```typescript
// useReducer with typed state
type HookStatus = "idle" | "loading" | "loaded" | "error";
type ProfileHealthState = { status: HookStatus; summary: ...; error: string | null };
type ProfileHealthAction = { type: "batch-loading" } | { type: "batch-complete"; summary } | ...;

// invoke for queries
const batchValidate = useCallback(async (signal?: AbortSignal) => {
    dispatch({ type: "batch-loading" });
    const summary = await invoke<EnrichedHealthSummary>("batch_validate_profiles");
    dispatch({ type: "batch-complete", summary });
}, []);

// listen for push events (re-trigger on profile changes, launch complete)
listen<EnrichedHealthSummary>("profile-health-batch-complete", (event) => {
    dispatch({ type: "batch-complete", summary: event.payload });
});

// cached snapshots on mount for instant badge display
const snapshots = await invoke<CachedHealthSnapshot[]>('get_cached_health_snapshots');
```

`useOfflineReadiness` follows: `useReducer` + `invoke<OfflineReadinessSnapshot[]>` +
`listen<string>("offline-readiness-updated", ...)`.

---

## Integration Points

### Files to Modify

| File                        | Change                                                                              |
| --------------------------- | ----------------------------------------------------------------------------------- |
| `metadata/migrations.rs`    | Add `if version < 13 { migrate_12_to_13(conn)?; ... }` and migration fn             |
| `metadata/mod.rs`           | Add `mod offline_store;` + public methods calling `offline_store` fns               |
| `metadata/models.rs`        | Add `OfflineCapability`, `TrainerType` enums, `OfflineReadinessRow`, size constants |
| `profile/models.rs`         | Add `trainer_type: Option<TrainerType>` to `TrainerSection`                         |
| `profile/health.rs`         | Inject offline readiness `HealthIssue` (Warning) when score < threshold             |
| `settings/mod.rs`           | Add `offline_mode: bool` to `AppSettingsData`                                       |
| `src-tauri/commands/mod.rs` | Add `pub mod offline;`                                                              |
| `src-tauri/lib.rs`          | Register offline commands in `invoke_handler!`                                      |
| `src/types/health.ts`       | Extend `HealthIssue`, add `OfflineReadinessSnapshot` TS type                        |
| `src/types/profile.ts`      | Add `TrainerType` union type                                                        |
| `src/types/index.ts`        | Re-export new types                                                                 |

### Files to Create

| File                                       | Role                                                                                                                                        |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `metadata/offline_store.rs`                | Free fns: `upsert_trainer_hash_cache`, `lookup_trainer_hash_cache`, `upsert_offline_readiness_snapshot`, `load_offline_readiness_snapshots` |
| `launch/trainer_type_catalog.rs`           | Mirror of `catalog.rs` for trainer types                                                                                                    |
| `offline/mod.rs`                           | `check_offline_preflight()` — builds `ReadinessCheckResult` from SQLite state                                                               |
| `src-tauri/commands/offline.rs`            | IPC commands: `get_offline_readiness`, `refresh_offline_readiness`, `get_trainer_hash_cache`                                                |
| `src/hooks/useOfflineReadiness.ts`         | `useReducer` hook for offline readiness state                                                                                               |
| `src/components/OfflineReadinessBadge.tsx` | Status chip, mirrors `HealthBadge.tsx`                                                                                                      |
| `assets/default_trainer_type_catalog.toml` | Default trainer type catalog                                                                                                                |

---

## Code Conventions

### Rust

- **Enum variants**: PascalCase, `#[serde(rename_all = "snake_case")]`, `#[default]` on Unknown/first variant
- **Error action strings**: imperative present tense, e.g. `"upsert a trainer hash cache row"`
- **Module visibility**: store module functions are `pub(crate)` or `pub`, struct fields `pub`
- **`Option<String>` columns**: use `rusqlite::OptionalExtension` for nullable FK lookups
- **Timestamps**: `Utc::now().to_rfc3339()` everywhere, stored as `TEXT NOT NULL`
- **UUIDs**: `db::new_id()` (already used in profile_sync, cache_store, etc.) — not `uuid::new_v4()` directly
- **Row cap constants**: declare in `models.rs` as `pub const MAX_*: usize = N;`
- **Transaction pattern**: `Transaction::new_unchecked(conn, TransactionBehavior::Immediate)` for multi-statement writes

### TypeScript / React

- **Hook return type**: explicit object shape, not tuple
- **AbortSignal**: pass to async `invoke` calls to cancel on unmount
- **Error normalization**: `error instanceof Error ? error.message : String(error)`
- **Tauri event listeners**: always clean up with `unlisten()` in `useEffect` return fn
- **Cached-then-live pattern**: load cached data first (instant), fire live scan after 700ms delay

### SQL

- **FK constraints**: `REFERENCES profiles(profile_id) ON DELETE CASCADE` — all offline tables must have this
- **Soft delete filter**: `WHERE p.deleted_at IS NULL` in all JOIN queries against profiles
- **UNIQUE INDEX naming**: `idx_<table>_<column(s)>` e.g. `idx_trainer_hash_cache_profile_path`
- **Upsert pattern**: `INSERT OR REPLACE` for PK-keyed tables, `ON CONFLICT(...) DO UPDATE SET` for UNIQUE-indexed tables

---

## Dependencies and Services

### Rust Crates (already in workspace)

- `sha2` — used in `version_store.rs` and `profile_sync.rs`, already declared
- `rusqlite` — connection, params, OptionalExtension, Transaction
- `serde` + `serde_json` — for JSON columns (blocking_reasons, etc.)
- `chrono` — `Utc::now().to_rfc3339()` timestamps
- `toml` — catalog deserialization (`toml::from_str`)
- `tracing` — structured logging (`tracing::warn!`, `tracing::debug!`)

### Frontend

- `@tauri-apps/api/core` — `invoke`
- `@tauri-apps/api/event` — `listen`
- `react` — `useReducer`, `useCallback`, `useEffect`, `useMemo`, `useRef`

### No New Crates Needed for Phase 1

`hash_trainer_file` is already using `sha2`. The trainer type catalog uses `toml` (already present).
Phase 3 (Aurora integration) may need `keyring` for W-1, but Phase 1 has no new dependencies.

---

## Gotchas and Warnings

1. **`version_snapshots` bootstrap**: Migration 13 should `INSERT INTO trainer_hash_cache SELECT ...
FROM version_snapshots` to pre-populate hashes from existing data — avoids full re-hash on first
   run. The `file_modified_at` and `file_size` columns will be `NULL` for bootstrapped rows,
   which is the correct signal to trigger a stat-check on next access.

2. **`with_conn` returns `T::default()` when unavailable**: This means `Vec::new()` for
   `load_offline_readiness_snapshots` and `0` for numeric results. Frontend must handle empty
   results gracefully — do not treat empty as an error.

3. **`TrainerSection.kind` is a raw `String`, not an enum**: Existing TOML files use `type = "fling"`
   (note: serialized as `type` not `kind`, see `#[serde(rename = "type")]`). The new
   `trainer_type: Option<TrainerType>` field is separate from `kind`. Migration of existing profiles
   should follow `legacy.rs` pattern — detect `kind` string on load, map to `TrainerType`, default
   to `Unknown` if unrecognized.

4. **`local_override` section**: Machine-local path overrides live in `LocalOverrideSection`.
   `trainer_type` goes into the main `TrainerSection` (portable), NOT `LocalOverrideSection`,
   because trainer type is a classification property not a path override.

5. **`health_store.rs` uses `INSERT OR REPLACE`**: This deletes the old row and inserts new.
   For `offline_readiness_snapshots` this is fine since `profile_id` is the PK. For
   `trainer_hash_cache` use `ON CONFLICT(profile_id, file_path) DO UPDATE` to avoid deleting
   `created_at`.

6. **`OnceLock` global catalog initialization**: `initialize_catalog` is called once during
   Tauri startup (`lib.rs`). The trainer type catalog needs the same treatment — call
   `initialize_trainer_type_catalog` in `lib.rs` setup alongside the optimization catalog.
   The `OnceLock::set()` call silently ignores duplicate initialization (safe for tests).

7. **`sanitize_display_path` in IPC commands**: All `path` strings in `HealthIssue` passed
   over IPC must be sanitized. The offline readiness issues that include file paths (e.g.,
   trainer path) must call `sanitize_display_path(&issue.path)` before return.

8. **`version_snapshots` table has `trainer_file_hash` but no `file_size`/`file_modified_at`**:
   The hash cache adds stat-based invalidation that the version snapshot table does not have.
   This is intentional — they serve different purposes. Don't conflate the two.

9. **Migration 4→5 shows the rename-table workaround**: SQLite does not support `ALTER TABLE DROP COLUMN`
   for columns with constraints, and `ALTER TABLE RENAME COLUMN` was added in SQLite 3.25. The
   existing codebase uses rename+rebuild for structural changes. Use `ALTER TABLE ADD COLUMN` only
   for additive changes.

10. **`MetadataStore::disabled()`**: In CLI mode and some test contexts, the store is disabled.
    All offline store method calls must tolerate this — `with_conn` handles it, but don't bypass
    `with_conn` with direct connection access.

11. **SQLite DB path is `~/.local/share/crosshook/metadata.db`**, NOT `~/.config/...`.
    `MetadataStore::try_new()` uses `BaseDirs::data_local_dir()`. `paths.rs` is unrelated — it
    only handles bundled script paths. The security requirement W-2 (`chmod 600`) applies to
    `~/.local/share/crosshook/metadata.db`.

12. **`lib.rs` startup spawns health scan at 500ms delay**: `commands::health::build_enriched_health_summary`
    is called directly (not via IPC) at startup and emits `"profile-health-batch-complete"`.
    Offline readiness scan should follow the same pattern — spawn at 500-700ms with a distinct
    event name like `"offline-readiness-scan-complete"`.

13. **`tauri::generate_handler![]` is a flat list**: All commands from all modules are listed
    flat. Adding offline commands requires: (a) `pub mod offline;` in `commands/mod.rs`,
    (b) new entries in the `generate_handler!` list in `lib.rs`. There is no per-module
    registration shortcut.

---

## Task-Specific Guidance

### Phase 1: Core Infrastructure (migration + hash cache + catalog)

**Start here:**

```
metadata/offline_store.rs     — 4 functions (upsert_hash, lookup_hash, upsert_readiness, load_readiness)
metadata/migrations.rs        — add migrate_12_to_13 with 3 new tables
metadata/models.rs            — OfflineCapability enum, TrainerType enum, OfflineReadinessRow
metadata/mod.rs               — expose new methods
launch/trainer_type_catalog.rs — copy structure from catalog.rs, change types
assets/default_trainer_type_catalog.toml — minimum: fling, aurora, wemod, plitch, cheat_engine
```

**Migration 13 bootstrap SQL:**

```sql
INSERT OR IGNORE INTO trainer_hash_cache (
    cache_id, profile_id, file_path, sha256_hash, verified_at, created_at, updated_at
)
SELECT
    lower(hex(randomblob(16))),
    profile_id,
    '' AS file_path,     -- unknown at migration time
    trainer_file_hash,
    checked_at,
    datetime('now'),
    datetime('now')
FROM version_snapshots
WHERE trainer_file_hash IS NOT NULL
  AND id IN (SELECT MAX(id) FROM version_snapshots GROUP BY profile_id);
```

Note: `file_path = ''` bootstrapped rows — the offline store should handle empty path as "unconfigured".

### Phase 2: Offline Readiness Scoring

```
offline/mod.rs         — check_offline_preflight(); scoring weights from research-business.md
profile/health.rs      — inject offline Warning issue if score < 60
src-tauri/commands/offline.rs — get_offline_readiness, refresh_offline_readiness
```

The scoring function signature:

```rust
pub fn check_offline_preflight(
    profile: &GameProfile,
    conn: &Connection,
) -> ReadinessCheckResult
```

Returns the same `ReadinessCheckResult` used by `check_system_readiness`. The Tauri command
wraps it via `with_conn`.

### Phase 3: UI Integration

Follow `useProfileHealth.ts` exactly for `useOfflineReadiness.ts`. Key divergence: offline
readiness is profile-scoped (by `profile_id`), not global, so the hook should accept an
optional profile name and call per-profile invoke.

`OfflineReadinessBadge` follows `HealthBadge.tsx` component structure. Use existing
`crosshook-status-chip` CSS class; add `--offline-ready`, `--offline-partial`, `--offline-blocked`
color variables to `variables.css`.

### Testing Strategy

All `metadata/offline_store.rs` functions should be tested with `MetadataStore::open_in_memory()`.
Follow the existing test in `migrations.rs`:

```rust
#[test]
fn migration_12_to_13_creates_offline_tables() {
    let conn = db::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM trainer_hash_cache", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0);
}
```

For `check_offline_preflight`, use `tempfile::tempdir()` to create real trainer files
(follow `healthy_steam_profile` in `health.rs` tests).

For the trainer type catalog, follow the 8 existing tests in `catalog.rs` — cover: parse valid,
skip empty id, skip duplicate, parse invalid TOML, merge override replaces in position,
merge novel id appends.
