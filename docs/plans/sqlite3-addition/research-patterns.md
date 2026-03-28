# Patterns Research: sqlite3-addition

## Overview

CrossHook-core follows a strict, highly consistent set of patterns across all store modules. Every store uses the same three-constructor convention (`try_new`, `new`, `with_base_path`), every error enum follows the same `From`-impl + `Display` contract, and all Tauri commands return `Result<T, String>` at the IPC boundary. The `MetadataStore` must conform to these patterns to integrate naturally. The one structural divergence: unlike TOML stores which hold only a `PathBuf`, `MetadataStore` must hold `Arc<Mutex<Connection>>` — the existing `RotatingLogWriter` in `logging.rs` demonstrates this pattern in the codebase already.

---

## Relevant Files

### Store Implementations

- `crates/crosshook-core/src/profile/toml_store.rs` — most complete store: `try_new`, `with_base_path`, `validate_name`, test suite, IPC result structs
- `crates/crosshook-core/src/settings/mod.rs` — `SettingsStore` with `#[serde(default)]` on data struct
- `crates/crosshook-core/src/settings/recent.rs` — `RecentFilesStore` with `with_path()` (single-file variant of `with_base_path`)
- `crates/crosshook-core/src/community/taps.rs` — `CommunityTapStore` with structured `Io { action, path, source }` error variant
- `crates/crosshook-core/src/export/launcher_store.rs` — `LauncherStoreError`, result structs (`LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`)

### Tauri Integration

- `src-tauri/src/lib.rs` — store construction, `.manage()` registration, `tauri::generate_handler![...]`
- `src-tauri/src/startup.rs` — `store_pair()` test helper, cross-store coordination, `StartupError` aggregating multiple store errors
- `src-tauri/src/commands/profile.rs` — `profile_rename` cascade, `map_error` helper, best-effort pattern with `tracing::warn!`
- `src-tauri/src/commands/export.rs` — thin Tauri command wrappers delegating to core
- `src-tauri/src/commands/launch.rs` — `async` commands, `AppHandle`, `tauri::async_runtime::spawn`
- `src-tauri/src/commands/shared.rs` — `create_log_path`, `slugify_target` — shared utility location

### Supporting Modules

- `crates/crosshook-core/src/logging.rs` — `Arc<Mutex<RotatingLogState>>` pattern, `data_local_dir()` for path resolution
- `crates/crosshook-core/src/profile/models.rs` — `#[serde(default)]`, `#[serde(rename_all = "snake_case")]`, `#[serde(skip_serializing_if)]`
- `crates/crosshook-core/Cargo.toml` — existing dependencies: `chrono`, `serde`, `serde_json`, `tokio`, `directories`; `tempfile` in `[dev-dependencies]`

---

## Architectural Patterns

### 1. Three-Constructor Store Pattern

All four stores use the identical constructor signature set. The `MetadataStore` must follow exactly:

```rust
// Production: resolves XDG path, returns Result<Self, String>
pub fn try_new() -> Result<Self, String> {
    let path = BaseDirs::new()
        .ok_or("home directory not found — CrossHook requires a user home directory")?
        .data_local_dir()          // for metadata.db — same base as recent.toml and logs
        .join("crosshook")
        .join("metadata.db");
    Self::with_path(&path)
        .map_err(|e| e.to_string())
}

// Panic wrapper — used in main.rs when absence is fatal
pub fn new() -> Self {
    Self::try_new().expect("home directory is required for CrossHook metadata storage")
}

// Test injection — accepts arbitrary path, returns domain error
pub fn with_path(path: &Path) -> Result<Self, MetadataError> { ... }
```

Reference implementations:

- `ProfileStore::try_new()` at `toml_store.rs:83–98` — `config_dir()`
- `RecentFilesStore::try_new()` at `recent.rs:67–75` — `data_local_dir()` (correct base for `metadata.db`)
- `CommunityTapStore::try_new()` at `taps.rs:103–110` — `data_local_dir()`

The exact string `"home directory not found — CrossHook requires a user home directory"` must be preserved verbatim — it is the established user-facing error message across all stores.

### 2. Tauri Store Registration

`lib.rs:run()` constructs all stores before the builder, fails fast on errors, then passes them to `.manage()`:

```rust
// lib.rs:15–31 — pattern for MetadataStore registration
let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
    eprintln!("CrossHook: failed to initialize metadata store: {error}");
    std::process::exit(1);
});
// OR for fail-soft (see feature-spec §7):
// let metadata_store = MetadataStore::try_new()
//     .map_err(|e| tracing::error!(%e, "metadata store unavailable"))
//     .ok();   // -> Option<MetadataStore>

tauri::Builder::default()
    .manage(metadata_store)  // adds to lib.rs:62–66 block
    ...
```

All `.manage()` calls are at `lib.rs:62–66`. Commands access state via `State<'_, MetadataStore>` parameter.

### 3. IPC Error Boundary — Always `Result<T, String>`

Every Tauri command returns `Result<T, String>`. The conversion is always `.map_err(|error| error.to_string())`. Commands that use one error type often define a private `map_error` helper:

```rust
// commands/profile.rs:9–11 — canonical pattern
fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}

// Usage:
store.list().map_err(map_error)
store.save(&name, &data).map_err(map_error)
```

For commands that mix error types (e.g., multi-step cascades), inline `.map_err(|e| e.to_string())` is used per-call instead of a shared helper.

### 4. Best-Effort Cascade Pattern (profile_rename)

The definitive multi-step orchestration pattern lives in `commands/profile.rs:149–194`. The critical operation propagates errors with `?`; all subsequent steps are best-effort with `tracing::warn!`:

```rust
// Step 1: Load before mutating (need old state for cleanup)
let old_profile = store.load(&old_name).ok();   // .ok() — silently ignore if missing

// Step 2: Critical op — propagate error
store.rename(&old_name, &new_name).map_err(map_error)?;

// Step 3+: Best-effort — warn and continue
if let Some(ref profile) = old_profile {
    match cleanup_launchers_for_profile_delete(&old_name, profile) {
        Ok(Some(result)) => result.script_deleted || result.desktop_entry_deleted,
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(%error, %old_name, %new_name, "launcher cleanup during profile rename failed");
            false
        }
    };
}

// Subsequent best-effort steps use the same if-let-Err-warn pattern:
if let Err(err) = store.save(&new_name, &profile) {
    tracing::warn!(%err, %new_name, "display_name update after profile rename failed");
}
```

The `MetadataStore` sync hooks integrate as additional best-effort steps after existing steps at position 4–5 in this cascade.

### 5. `Arc<Mutex<...>>` for Shared Mutable State

`RotatingLogWriter` at `logging.rs:118–120` is the existing precedent for `Arc<Mutex<...>>` in the codebase:

```rust
#[derive(Clone)]
struct RotatingLogWriter {
    state: Arc<Mutex<RotatingLogState>>,  // exactly the MetadataStore conn pattern
}
```

`MetadataStore` follows the same structure:

```rust
#[derive(Clone)]
pub struct MetadataStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
}
```

`Clone` is required for `.manage()` in Tauri state.

---

## Code Conventions

### Error Enum Definition

Every module defines its own error enum. Never reuse another module's error enum. The pattern from `community/taps.rs:48–91`:

```rust
#[derive(Debug)]
pub enum MetadataError {
    // Simple variants
    DatabaseNotAvailable,
    // Structured Io variant — carries context (mirrors CommunityTapError, LoggingError)
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    // Wrapped external error
    Sqlite(rusqlite::Error),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatabaseNotAvailable => write!(f, "metadata database is not available"),
            Self::Io { action, path, source } =>
                write!(f, "failed to {action} '{}': {source}", path.display()),
            Self::Sqlite(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for MetadataError {}

impl From<rusqlite::Error> for MetadataError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}
```

### IPC Result Structs

Any struct that crosses the Tauri IPC boundary gets `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` plus `#[serde(default)]` on all fields. Compare `LauncherInfo` at `launcher_store.rs:27–43`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SyncReport {
    #[serde(default)]
    pub profiles_upserted: usize,
    #[serde(default)]
    pub profiles_deleted: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}
```

### Structured Logging

`tracing::warn!` is the canonical level for best-effort failures. Field names use `%` sigil for `Display` types:

```rust
// commands/profile.rs:166–168 — exact format to replicate
tracing::warn!(%error, %old_name, %new_name, "launcher cleanup during profile rename failed");
tracing::warn!(%err, %new_name, "display_name update after profile rename failed");

// lib.rs:49–53 — warn with named field
tracing::warn!(%error, profile_name, "failed to emit auto-load-profile event");
```

For metadata sync hooks, use the same format:

```rust
tracing::warn!(%error, profile_name = name, "metadata sync failed after profile save");
```

### Module Directory Structure

Each concern lives in a directory with a `mod.rs` as the public routing surface. See: `profile/`, `settings/`, `community/`, `export/`, `launch/`. The `metadata/` module follows exactly:

```
crates/crosshook-core/src/metadata/
  mod.rs            — MetadataStore struct, SyncReport, public API
  db.rs             — open_at_path(), setup_pragmas(), new_id()
  migrations.rs     — hand-rolled migration runner, PRAGMA user_version
  models.rs         — SQLite-facing structs, SyncSource, LaunchOutcome enums
  profile_sync.rs   — observe_profile_write/rename/delete
  launcher_sync.rs  — observe_launcher_exported/scan
  launch_history.rs — record_launch_started/finished
```

---

## Error Handling

### Error Propagation Chain

```
rusqlite::Error
  → MetadataError::Sqlite (via From impl)
    → MetadataStore methods return Result<T, MetadataError>
      → Tauri commands: .map_err(|e| e.to_string())
        → Frontend receives String in rejected Promise
```

### Best-Effort vs. Hard-Fail

- **Hard-fail** (propagate with `?`): the critical TOML operation that the command is named for
- **Best-effort** (warn and continue): metadata sync, launcher cleanup, display_name update, settings update

No `unwrap()` outside of tests or the `new()` fallback methods.

---

## Testing Approach

### Unit Tests: In-Memory SQLite

For all `MetadataStore` logic tests — no filesystem needed:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_profile_write_creates_stable_id() {
        let store = MetadataStore::open_in_memory().unwrap();
        store.observe_profile_write("my-game", ...).unwrap();
        // assert on row count, field values
    }
}
```

### Integration Tests: tempdir + store_pair Pattern

For tests requiring both `ProfileStore` and `MetadataStore`, mirror `startup.rs:72–78`:

```rust
fn store_pair() -> (ProfileStore, MetadataStore) {
    let temp_dir = tempdir().unwrap();
    let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let metadata = MetadataStore::with_path(&temp_dir.path().join("metadata.db")).unwrap();
    (profile_store, metadata)
    // NOTE: temp_dir must be kept alive in caller — do not drop early
}
```

### Test Conventions Observed

- `let temp_dir = tempdir().unwrap();` — one per test, never shared across test functions
- `ProfileStore::with_base_path(temp_dir.path().join("profiles"))` — always subdirectory, not root of tempdir
- `assert!(matches!(result, Err(XError::VariantName(ref x)) if x == "..."))` — variant matching
- Filesystem verification alongside return values: `assert!(store.profile_path("x").unwrap().exists())`
- Sample data constructors (`sample_profile()`, `steam_profile()`) for avoiding repetition within a single test module — do not create a shared crate-wide fixture

### Anti-Patterns to Avoid

- No `TestMetadataStore` type — use `open_in_memory()`
- No testing through Tauri command layer — test `MetadataStore` methods directly
- No assertions on exact UUID string values — assert on behavior (row counts, field names)

---

## Patterns to Follow

| Pattern                                                                                                                                  | Where                                                                                | Apply to MetadataStore                                                                                     |
| ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| `try_new() -> Result<Self, String>` with `BaseDirs::new().ok_or("home directory not found — CrossHook requires a user home directory")?` | `toml_store.rs:83–86`, `settings/mod.rs:71–73`, `recent.rs:68–70`, `taps.rs:104–106` | Exact same string + pattern                                                                                |
| `with_base_path(path: PathBuf)` / `with_path(path: &Path)` test injection                                                                | All stores                                                                           | `with_path(path: &Path) -> Result<Self, MetadataError>`                                                    |
| `open_in_memory()` constructor                                                                                                           | Not yet present — to be introduced                                                   | New pattern for unit testing only                                                                          |
| `#[derive(Serialize, Deserialize, Default)]` + `#[serde(default)]` on IPC structs                                                        | `launcher_store.rs:27–79`, `settings/mod.rs:19–25`                                   | `SyncReport` and any IPC row structs                                                                       |
| `fn map_error(error: XError) -> String` private helper per command file                                                                  | `commands/profile.rs:9–11`                                                           | Add to `commands/metadata.rs` when created                                                                 |
| Best-effort steps with `if let Err(e) = ... { tracing::warn!(...) }`                                                                     | `commands/profile.rs:164–192`                                                        | All metadata sync hooks in Tauri commands                                                                  |
| `Arc<Mutex<State>>` + `#[derive(Clone)]` on struct                                                                                       | `logging.rs:118–120`                                                                 | `MetadataStore { conn: Arc<Mutex<Connection>> }`                                                           |
| `tempfile` dev-dependency already present                                                                                                | `crosshook-core/Cargo.toml:17`                                                       | No new dependency needed for tests                                                                         |
| `chrono::Utc::now().to_rfc3339()` for timestamps                                                                                         | Used in codebase via `chrono` dep                                                    | Inline — no helper needed                                                                                  |
| `data_local_dir().join("crosshook").join(...)` for data files                                                                            | `recent.rs:70–73`, `logging.rs:108–110`                                              | `metadata.db` lives at `data_local_dir()/crosshook/metadata.db`                                            |
| `validate_name()` before using profile name as DB parameter                                                                              | `toml_store.rs:300–325`                                                              | Call at `MetadataStore` API boundary; use parametrized `?` placeholders in SQL, never string interpolation |
| Error variants that carry `action: &'static str, path: PathBuf, source`                                                                  | `taps.rs:52–56`, `logging.rs:22–28`                                                  | `MetadataError::Io { action, path, source: rusqlite::Error }`                                              |

---

## Open Questions (Affecting Pattern Implementation)

1. **`Option<MetadataStore>` vs. internal `available` flag in Tauri state** — the feature-spec §7 (Fail-Soft Rule) says "always present with internal `available` flag"; the practices-research says `Option<MetadataStore>`. Choose before writing `lib.rs` integration. The `Option` approach is more correct Rust; the internal flag is less ergonomically disruptive to command signatures.

2. **`sanitize_display_path()` promotion** — currently private in `commands/launch.rs:301`, used in 8 places. Any new metadata IPC commands returning stored path strings must apply the same `$HOME` → `~` replacement. Either promote to `commands/shared.rs` (which already hosts `create_log_path` and `slugify_target`) or document the requirement at each new command site.

3. **Migrate-in-`try_new()` vs. explicit `migrate()` call** — codebase precedent (`ProfileStore::save()` auto-creates directories) favors running migrations in `try_new()`. No separate bootstrap call.
