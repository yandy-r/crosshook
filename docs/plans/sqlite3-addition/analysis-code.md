# SQLite Metadata Layer — Code Analysis

## Executive Summary

The codebase follows three highly consistent patterns that MetadataStore must replicate exactly: a three-constructor store pattern (`try_new`/`new`/`with_path`), a structured error enum with named fields and `From` impls, and an `Arc<Mutex<T>>` wrapper for shared mutable state. Metadata sync hooks slot into existing Tauri command handlers as additional best-effort steps after the canonical TOML operations — no changes to `ProfileStore` itself. All five Phase 1 hook injection points in `commands/profile.rs` are identified below with exact line numbers.

---

## Existing Code Structure

### Module Layout

```
crates/crosshook-core/src/
  lib.rs              ← add `pub mod metadata;` here (line 9, after `pub mod logging`)
  community/          ← taps.rs, index.rs (error enum reference)
  export/             ← launcher_store.rs (Phase 2 types)
  launch/             ← request.rs, diagnostics/ (Phase 2)
  logging.rs          ← Arc<Mutex> precedent
  profile/            ← models.rs, toml_store.rs (primary pattern template)
  settings/           ← mod.rs, recent.rs (data_local_dir pattern)

src-tauri/src/
  lib.rs              ← .manage() registration, invoke_handler list
  startup.rs          ← setup hook; add sync_profiles_from_store() here
  commands/
    mod.rs            ← add `pub mod metadata;`
    profile.rs        ← 5 hook injection points (Phase 1)
    launch.rs         ← Phase 2 spawn_blocking hooks
    export.rs         ← Phase 2 launcher observation
    community.rs      ← Phase 3 tap index sync
    shared.rs         ← sanitize_display_path() destination
```

### Cargo.toml — Required Additions

**File**: `crates/crosshook-core/Cargo.toml`

Current deps: `chrono`, `directories`, `serde`, `serde_json`, `toml`, `tokio`, `tracing`, `tracing-subscriber`

Add to `[dependencies]`:

```toml
rusqlite = { version = "0.39", features = ["bundled"] }
uuid    = { version = "1",    features = ["v4", "serde"] }
```

Add to `[dev-dependencies]`:

```toml
# tempfile already present — no addition needed
```

---

## Implementation Patterns

### 1. Three-Constructor Store Pattern

**Source**: `crates/crosshook-core/src/profile/toml_store.rs:82–98`

```rust
impl ProfileStore {
    pub fn try_new() -> Result<Self, String> {
        let base_path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .config_dir()
            .join("crosshook")
            .join("profiles");
        Ok(Self { base_path })
    }

    pub fn new() -> Self {
        Self::try_new().expect("home directory is required for CrossHook profile storage")
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }
}
```

**For MetadataStore**, the test-injection constructor is `with_path(path: PathBuf)` (matching `RecentFilesStore` at `settings/recent.rs:81`), since the store resolves to a single file, not a directory.

`try_new` error type is `Result<Self, String>` (plain string), not a domain error type. This is consistent across all stores.

**MetadataStore skeleton**:

```rust
pub struct MetadataStore {
    conn: Arc<Mutex<Connection>>,
    available: bool,
}

impl MetadataStore {
    pub fn try_new() -> Result<Self, String> {
        let path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .data_local_dir()
            .join("crosshook")
            .join("metadata.db");
        Self::open(path)
    }

    pub fn new() -> Self {
        Self::try_new().expect("home directory is required for CrossHook metadata storage")
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self::open(path).unwrap_or_else(|_| Self { conn: /* dummy */, available: false })
    }
}
```

### 2. Structured Error Enum Pattern

**Source**: `crates/crosshook-core/src/community/taps.rs:48–91` and `logging.rs:20–58`

The preferred pattern uses named fields for context-carrying variants:

```rust
#[derive(Debug)]
pub enum MetadataStoreError {
    HomeDirectoryUnavailable,
    Database {
        action: &'static str,
        source: rusqlite::Error,
    },
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    MigrationFailed {
        version: u32,
        source: rusqlite::Error,
    },
}

impl fmt::Display for MetadataStoreError { /* match each arm */ }
impl Error for MetadataStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> { /* as in LoggingError:52–57 */ }
}
impl From<rusqlite::Error> for MetadataStoreError { /* Database variant */ }
impl From<io::Error> for MetadataStoreError { /* Io variant */ }
```

**Critical**: Raw `rusqlite::Error` must never reach IPC. At the Tauri command boundary convert with `.map_err(|e| e.to_string())`.

### 3. Arc<Mutex<T>> Shared State Pattern

**Source**: `crates/crosshook-core/src/logging.rs:118–121`

```rust
#[derive(Clone)]
struct RotatingLogWriter {
    state: Arc<Mutex<RotatingLogState>>,
}
```

The `Clone` derive on the outer wrapper is required — Tauri's `State<'_, T>` requires `T: Clone + Send + Sync`. The inner state is not `Clone`.

Access pattern from `RotatingLogHandle::write` (`logging.rs:193–199`):

```rust
let mut state = self.state.lock()
    .map_err(|_| io::Error::other("log writer mutex was poisoned"))?;
```

**MetadataStore** follows identically:

```rust
#[derive(Clone)]
pub struct MetadataStore {
    conn: Arc<Mutex<Connection>>,
    available: bool,
}

// In methods:
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where F: FnOnce(&Connection) -> Result<T, rusqlite::Error>
{
    if !self.available { return Ok(Default::default()); }
    let conn = self.conn.lock().map_err(|_| MetadataStoreError::Database {
        action: "acquire connection lock",
        source: rusqlite::Error::ExecuteReturnedResults, // sentinel
    })?;
    f(&conn).map_err(|source| MetadataStoreError::Database { action, source })
}
```

### 4. Best-Effort Cascade Pattern

**Source**: `src-tauri/src/commands/profile.rs:114–124` (profile_delete) and `148–194` (profile_rename)

```rust
// profile_delete — best-effort before the critical op
#[tauri::command]
pub fn profile_delete(name: String, store: State<'_, ProfileStore>) -> Result<(), String> {
    if let Ok(profile) = store.load(&name) {           // ← best-effort pre-load
        if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
            tracing::warn!("Launcher cleanup failed for profile {name}: {error}");
        }
    }
    store.delete(&name).map_err(map_error)             // ← critical op propagates
}

// profile_rename — multiple best-effort steps after critical op
store.rename(&old_name, &new_name).map_err(map_error)?;  // ← critical (line 158)
// ... then best-effort launcher cleanup, display_name update, settings update
```

Metadata sync hooks are inserted as **additional best-effort steps** in the same style:

```rust
// After the critical TOML op:
if let Err(error) = metadata_store.observe_profile_write(&name, &data) {
    tracing::warn!(%error, %name, "metadata sync after profile save failed");
}
```

### 5. IPC Error Boundary Pattern

**Source**: `src-tauri/src/commands/profile.rs:9–11`

```rust
fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}
```

All Tauri commands return `Result<T, String>`. The pattern is either `map_error` helper or inline `.map_err(|e| e.to_string())`. Choose the helper when used multiple times in the same file.

### 6. data_local_dir Base Path Pattern

**Source**: `crates/crosshook-core/src/settings/recent.rs:68–75`

```rust
pub fn try_new() -> Result<Self, String> {
    let path = BaseDirs::new()
        .ok_or("home directory not found — CrossHook requires a user home directory")?
        .data_local_dir()           // ← ~/.local/share on Linux
        .join(SETTINGS_DIR)         // "crosshook"
        .join(RECENT_FILE_NAME);    // "recent.toml"
    Ok(Self { path })
}
```

`metadata.db` goes at `~/.local/share/crosshook/metadata.db` — same base as recent.toml, CommunityTapStore (`taps.rs:104–108` uses `data_local_dir().join("crosshook/community/taps")`), and logging (`logging.rs:108–109` uses `data_local_dir()`).

### 7. Test Pattern

**Source**: `toml_store.rs:397–410`, `taps.rs:447–493`, `startup.rs:72–78`

All tests use `tempfile::tempdir()` and the `with_base_path`/`with_path` constructor:

```rust
#[test]
fn some_test() {
    let temp_dir = tempdir().unwrap();
    let store = MetadataStore::with_path(temp_dir.path().join("metadata.db"));
    // ... assertions
}
```

Helper pairs for tests involving multiple stores (`startup.rs:72–78`):

```rust
fn store_pair() -> (SettingsStore, ProfileStore) {
    let temp_dir = tempdir().unwrap();
    let settings_store = SettingsStore::with_base_path(...);
    let profile_store  = ProfileStore::with_base_path(...);
    (settings_store, profile_store)
}
```

---

## Integration Points

### Files to Create

| File                                        | Purpose                                                             |
| ------------------------------------------- | ------------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/mod.rs` | MetadataStore struct, constructors, error enum, all Phase 1 methods |
| `src-tauri/src/commands/metadata.rs`        | Tauri command handlers exposing MetadataStore queries to frontend   |

### Files to Modify — with Exact Locations

#### `crates/crosshook-core/Cargo.toml`

- **Line 12** (after `tracing`): add `rusqlite = { version = "0.39", features = ["bundled"] }` and `uuid = { version = "1", features = ["v4", "serde"] }`

#### `crates/crosshook-core/src/lib.rs`

- **Line 6** (after `pub mod launch;`): add `pub mod metadata;`
- Current lines: `pub mod community`, `pub mod export`, `pub mod install`, `pub mod launch`, `pub mod logging`, `pub mod profile`, `pub mod settings`, `pub mod steam`, `pub mod update`

#### `src-tauri/src/commands/mod.rs`

- **Line 5** (after `pub mod launch;`): add `pub mod metadata;`
- Current lines: `pub mod community`, `pub mod export`, `pub mod install`, `pub mod launch`, `pub mod profile`, `pub mod settings`, `mod shared`, `pub mod steam`, `pub mod update`

#### `src-tauri/src/lib.rs` — Store initialization and registration

- **After line 30** (`community_tap_store` init): add MetadataStore initialization (fail-soft via `unwrap_or_else` that logs warning and creates a disabled store instead of exiting)
- **After line 65** (`.manage(community_tap_store)`): add `.manage(metadata_store)`
- **After line 112** (`commands::update::cancel_update`): add metadata commands to invoke_handler list

Note: MetadataStore init failure must NOT call `std::process::exit(1)` — unlike other stores, it's degraded-mode capable. Use a factory that returns a disabled store on failure.

#### `src-tauri/src/startup.rs`

- **After line 39** (`resolve_auto_load_profile_name` call in setup closure): add `sync_profiles_from_store(&metadata_store, &profile_store)` as best-effort reconciliation
- Pattern: follows same structure as `resolve_auto_load_profile_name` — takes store refs, returns `Result<_, _>`, called with warn-on-error

#### `src-tauri/src/commands/profile.rs` — Phase 1 Hook Injection Points

**1. profile_save (line 96–102)** — observe after successful save:

```rust
pub fn profile_save(name: String, data: GameProfile, store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>) -> Result<(), String>
{
    store.save(&name, &data).map_err(map_error)?;
    // ADD: best-effort metadata sync
    if let Err(e) = metadata_store.observe_profile_write(&name, &data) {
        tracing::warn!(%e, %name, "metadata sync after profile_save failed");
    }
    Ok(())
}
```

**2. profile_delete (line 114–124)** — observe after successful delete:

```rust
store.delete(&name).map_err(map_error)?;
// ADD: best-effort metadata sync
if let Err(e) = metadata_store.observe_profile_delete(&name) {
    tracing::warn!(%e, %name, "metadata sync after profile_delete failed");
}
```

**3. profile_rename (line 148–194)** — observe after critical rename (line 158):

```rust
store.rename(&old_name, &new_name).map_err(map_error)?;  // line 158
// ADD: best-effort metadata sync (before existing best-effort steps)
if let Err(e) = metadata_store.observe_profile_rename(&old_name, &new_name) {
    tracing::warn!(%e, %old_name, %new_name, "metadata sync after profile_rename failed");
}
// ... existing launcher cleanup, display_name update, settings update
```

**4. profile_duplicate (line 141–146)** — observe new copy after success:

```rust
let result = store.duplicate(&name).map_err(map_error)?;
// ADD: best-effort metadata sync for the new copy
if let Err(e) = metadata_store.observe_profile_write(&result.name, &result.profile) {
    tracing::warn!(%e, name = %result.name, "metadata sync after profile_duplicate failed");
}
Ok(result)
```

**5. profile_import_legacy (line 197–204)** — observe after successful import:

```rust
let profile = store.import_legacy(Path::new(&path)).map_err(map_error)?;
// ADD: best-effort metadata sync (need profile name from path stem)
let profile_name = Path::new(&path).file_stem()
    .and_then(|s| s.to_str()).unwrap_or("unknown");
if let Err(e) = metadata_store.observe_profile_write(profile_name, &profile) {
    tracing::warn!(%e, %profile_name, "metadata sync after import_legacy failed");
}
Ok(profile)
```

All five modified commands require `metadata_store: State<'_, MetadataStore>` added to their parameter list, with `use crosshook_core::metadata::MetadataStore;` at the top of profile.rs.

---

## Code Conventions

### Rust Naming

- Store methods: `snake_case` — `observe_profile_write`, `observe_profile_rename`, `observe_profile_delete`, `sync_profiles_from_store`
- Error variants: `PascalCase` — `Database`, `MigrationFailed`, `HomeDirectoryUnavailable`
- Constants: `SCREAMING_SNAKE_CASE` — `DEFAULT_DB_FILE_NAME`, `SCHEMA_VERSION`

### Serde Derive

All types crossing the IPC boundary need: `#[derive(Debug, Clone, Serialize, Deserialize)]`
Profile-resident types (`GameProfile`, `GameSection`, etc.) already derive Serialize — no changes needed.

### Tracing

Log macros: `tracing::warn!(%error, %name, "message")` — use `%` for Display types, `?` for Debug. Don't use `{:?}` in format strings.

### Error Conversion at IPC Boundary

```rust
fn map_metadata_error(error: MetadataStoreError) -> String {
    error.to_string()
}
```

Private helper per commands file, matching `profile.rs:9–11`.

---

## Dependencies and Services

### Tauri State Types — Full Registration Order (lib.rs:62–66)

```rust
.manage(profile_store)          // ProfileStore
.manage(settings_store)         // SettingsStore
.manage(recent_files_store)     // RecentFilesStore
.manage(community_tap_store)    // CommunityTapStore
.manage(metadata_store)         // MetadataStore ← add here
.manage(commands::update::UpdateProcessState::new())
```

### MetadataStore SQLite Configuration (Phase 1)

These PRAGMAs must run at connection open time:

```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
PRAGMA user_version=1;   -- for migration tracking
```

### GameProfile Fields Extracted for Metadata

From `models.rs`:

- `profile.game.name` → `game_name` column in `profiles` table
- `profile.launch.method` → `launch_method` column in `profiles` table
- Profile name (filename stem) → `current_name` column

---

## Gotchas and Warnings

1. **`rusqlite::Connection` is `!Send`** — cannot cross async await points. Phase 2 async commands (`launch_game`, `launch_trainer`) must wrap all metadata writes in `tokio::task::spawn_blocking`. No existing example in the codebase — this is a new pattern to introduce.

2. **`available` flag is not optional** — MetadataStore must have `available: bool`. When SQLite fails to open (disk full, permissions), all public methods must silently no-op (return `Ok(Default::default())`). The app must not crash on metadata failure.

3. **lib.rs initialization is different from other stores** — Other stores call `std::process::exit(1)` on failure. MetadataStore must NOT exit — use a warn-and-degrade pattern:

   ```rust
   let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
       tracing::warn!(%error, "metadata store unavailable; running in degraded mode");
       MetadataStore::disabled()
   });
   ```

4. **File permissions** — Create `metadata.db` with `0o600` (user-only). Use `std::fs::OpenOptions` with explicit permissions before passing to `rusqlite::Connection::open()`. Security requirement from research-security.md.

5. **Parameterized queries only** — Never interpolate values into SQL strings. Always use `?` placeholders via `conn.execute("...", params![val1, val2])`.

6. **UPSERT required for all writes** — Use `INSERT INTO ... ON CONFLICT DO UPDATE SET ...` to make `sync_profiles_from_store()` and `observe_profile_write()` idempotent.

7. **UUID generation** — Phase 1 `profiles` table uses UUIDs as stable identity. Generate via `uuid::Uuid::new_v4().to_string()` at first INSERT. Store UUID in the `profiles` table; recover it from the table on subsequent writes.

8. **Schema migration at `PRAGMA user_version`** — Check version at open, run migrations sequentially up to current version. Use a single `run_migrations(conn: &Connection)` function.

9. **`with_path` for tests must not panic** — Unlike `new()`, the test constructor should handle failure gracefully. If path is invalid, return a disabled store rather than panicking, so test failures are clear.

10. **`clone()` semantics on MetadataStore** — The `Arc<Mutex<Connection>>` clone is a reference clone, not a deep copy. All clones share the same connection and therefore the same mutex. This is intentional (Tauri State clones are shared).

---

## Task-Specific Guidance

### Phase 1 — Core Tasks (can be parallelized after Cargo.toml is done)

| Task                       | Files                                      | Blocker                  |
| -------------------------- | ------------------------------------------ | ------------------------ |
| Add Cargo.toml deps        | `crosshook-core/Cargo.toml`                | None — do first          |
| Create MetadataStore       | `metadata/mod.rs`                          | Cargo.toml deps          |
| Add lib.rs module          | `crosshook-core/src/lib.rs`                | MetadataStore exists     |
| Register in app            | `src-tauri/src/lib.rs`                     | MetadataStore exists     |
| Add commands module        | `commands/mod.rs` + `commands/metadata.rs` | MetadataStore exists     |
| Hook profile_save          | `commands/profile.rs:96–102`               | MetadataStore registered |
| Hook profile_delete        | `commands/profile.rs:114–124`              | MetadataStore registered |
| Hook profile_rename        | `commands/profile.rs:148–194`              | MetadataStore registered |
| Hook profile_duplicate     | `commands/profile.rs:141–146`              | MetadataStore registered |
| Hook profile_import_legacy | `commands/profile.rs:197–204`              | MetadataStore registered |
| Startup reconciliation     | `startup.rs`                               | MetadataStore registered |

### Phase 2 Blockers (do NOT implement in Phase 1)

- `launch/request.rs` needs `profile_name: String` field added to `LaunchRequest` before any launch hooks
- `commands/launch.rs:301` has `sanitize_display_path()` that must be promoted to `commands/shared.rs` before Phase 2

### Testing Strategy

- Unit test each MetadataStore method with `with_path(tempdir)` constructor
- Test degraded mode: pass a path in a non-existent directory (no `create_dir_all`) — `observe_*` should return `Ok(())`
- Test `sync_profiles_from_store()` with a ProfileStore containing 3 profiles — verify 3 rows in DB
- Test rename creates history row in `profile_name_history`
- For Tauri command hooks: test helper functions directly (not via Tauri test harness), matching the pattern in `profile.rs` tests (lines 278–366)
