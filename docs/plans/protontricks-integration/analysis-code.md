# Code Analysis: Protontricks Integration

## Executive Summary

This analysis documents the exact code patterns, integration points, and conventions required to implement the `protontricks-integration` feature. All patterns below are drawn from source files currently in the repo and are directly reusable. The new `prefix_deps` module follows the established core-library submodule pattern, reuses existing binary detection and subprocess execution infrastructure, and integrates via the standard thin-IPC-command + React-hook architecture.

---

## Existing Code Structure

### Core Library Modules (`crosshook-core`)

| File                                                                       | Purpose                                              |
| -------------------------------------------------------------------------- | ---------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`                    | Module registry — add `pub mod prefix_deps;` here    |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | Binary PATH-walk, env application helpers            |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | Subprocess spawn patterns, env setup                 |
| `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`         | `TrainerSection`, `GameProfile` structs              |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | `AppSettingsData`, `SettingsStore`                   |
| `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`    | Sequential migration runner, currently at v14        |
| `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs`            | SQLite connection factory                            |
| `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`           | `MetadataStore` facade, `with_conn()` pattern        |
| `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`        | `MetadataStoreError`, row struct patterns            |
| `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`         | `ProfileHealthReport`, `HealthIssue`, `HealthStatus` |

### IPC Layer (`src-tauri`)

| File                                                        | Purpose                                                |
| ----------------------------------------------------------- | ------------------------------------------------------ |
| `src/crosshook-native/src-tauri/src/lib.rs`                 | Builder `.manage()` and `invoke_handler!` registration |
| `src/crosshook-native/src-tauri/src/commands/update.rs`     | Concurrent operation lock via `Mutex<Option<u32>>`     |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`   | IPC DTO pattern: separate load DTO / save DTO          |
| `src/crosshook-native/src-tauri/src/commands/steam.rs`      | Discovery command returning `Result<Vec<T>, String>`   |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs` | Readiness check returning a plain struct               |

### Frontend (`React/TypeScript`)

| File                                                  | Purpose                                           |
| ----------------------------------------------------- | ------------------------------------------------- |
| `src/crosshook-native/src/hooks/useProtonInstalls.ts` | Canonical hook template with cleanup flag         |
| `src/crosshook-native/src/hooks/useLaunchState.ts`    | Tauri event listener pattern via `listen()`       |
| `src/crosshook-native/src/types/profile.ts`           | `GameProfile` TypeScript mirror                   |
| `src/crosshook-native/src/types/settings.ts`          | `AppSettingsData`/`SettingsSaveRequest` split DTO |

---

## Implementation Patterns

### 1. Binary Detection via PATH Walk

**File**: `runtime_helpers.rs:301`

```rust
pub fn resolve_umu_run_path() -> Option<String> {
    let path_value =
        env::var_os("PATH").unwrap_or_else(|| std::ffi::OsString::from(DEFAULT_HOST_PATH));
    for directory in env::split_paths(&path_value) {
        let candidate = directory.join("umu-run");
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}
```

For protontricks/winetricks detection, clone this function, replacing `"umu-run"` with `"protontricks"` or `"winetricks"`. The `is_executable_file()` helper (same file, line 314) checks `is_file()` and Unix executable bit — reuse it directly.

### 2. Host Environment Application (Critical for winetricks/protontricks)

**File**: `runtime_helpers.rs:153`

```rust
pub fn apply_host_environment(command: &mut Command) {
    set_env(command, "HOME", env_value("HOME", ""));
    set_env(command, "USER", env_value("USER", ""));
    set_env(command, "LOGNAME", env_value("LOGNAME", ""));
    set_env(command, "SHELL", env_value("SHELL", DEFAULT_SHELL));
    set_env(command, "PATH", env_value("PATH", DEFAULT_HOST_PATH));
    set_env(command, "DISPLAY", env_value("DISPLAY", ""));
    set_env(command, "WAYLAND_DISPLAY", env_value("WAYLAND_DISPLAY", ""));
    set_env(command, "XDG_RUNTIME_DIR", env_value("XDG_RUNTIME_DIR", ""));
    set_env(command, "DBUS_SESSION_BUS_ADDRESS",
        env_value("DBUS_SESSION_BUS_ADDRESS", ""));
}
```

**Critical**: Proton commands use `.env_clear()` before applying env. winetricks/protontricks must NOT use `env_clear()`. Call `apply_host_environment()` instead to preserve HOME, USER, PATH, XDG_RUNTIME_DIR. Flatpak sandboxed protontricks additionally needs `WINEPREFIX` injected before calling winetricks.

### 3. Sequential SQLite Migration Pattern

**File**: `metadata/migrations.rs:4`

The runner reads `user_version`, then applies each migration in a guarded `if version < N` block, incrementing `user_version` after each:

```rust
if version < 14 {
    migrate_13_to_14(conn)?;
    conn.pragma_update(None, "user_version", 14_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "set user_version to 14",
            source,
        })?;
}
```

Add the v14→v15 migration following this exact pattern. The new `prefix_dependency_state` DDL belongs in a `migrate_14_to_15()` function, called in `run_migrations()` under `if version < 15`.

### 4. MetadataStore `with_conn()` Pattern

**File**: `metadata/mod.rs:93`

```rust
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,
{
    if !self.available {
        return Ok(T::default());
    }
    let Some(conn) = &self.conn else {
        return Ok(T::default());
    };
    let guard = conn.lock().map_err(|_| {
        MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
    })?;
    f(&guard)
}
```

All new CRUD methods on `MetadataStore` (upsert dep state, query dep state, clear deps for profile) must use `with_conn()` or `with_conn_mut()`. The graceful `T::default()` fallback when `available == false` is load-bearing — the store can be disabled at startup if SQLite fails.

### 4a. Store Sub-functions Take Bare `&Connection`

**File**: `metadata/health_store.rs:13`

Inner store files (`health_store.rs`, `offline_store.rs`, etc.) define free functions that take a bare `&Connection` — **not** `&MetadataStore`. They are dispatched through `MetadataStore::with_conn()`. The new `prefix_deps_store.rs` must follow this same pattern:

```rust
// In metadata/prefix_deps_store.rs — takes bare &Connection:
pub fn upsert_dep_state(
    conn: &Connection,
    profile_id: &str,
    package_name: &str,
    // ...
) -> Result<(), MetadataStoreError> { ... }

// In MetadataStore impl (metadata/mod.rs) — dispatches via with_conn_mut:
pub fn upsert_prefix_dep_state(&self, ...) -> Result<(), MetadataStoreError> {
    self.with_conn_mut("upsert prefix dep state", |conn| {
        prefix_deps_store::upsert_dep_state(conn, ...)
    })
}
```

### 5. Concurrent Operation Lock (Mutex PID Guard)

**File**: `src-tauri/src/commands/update.rs:13`

```rust
pub struct UpdateProcessState {
    pid: Mutex<Option<u32>>,
}
// ... in update_game:
if let Some(pid) = child.id() {
    *state.pid.lock().unwrap() = Some(pid);
}
// ... in cancel_update:
let pid = state.pid.lock().unwrap().take();
```

For prefix dependency installation, create a `PrefixDepsInstallState` with the same `Mutex<Option<u32>>` pattern. The key behavioral rule: one install per `(profile_id, prefix_path)` at a time. The lock must be keyed on prefix path — consider `Mutex<HashMap<String, u32>>` where key is prefix path, to allow concurrent installs against different prefixes.

### 6. Tauri Event Streaming (Log Lines)

**File**: `src-tauri/src/commands/update.rs:73`

The pattern is: spawn a `tokio` task that polls a log file for new content, emits `app.emit(event_name, line)` per line, then emits a completion event when `child.try_wait()` returns `Some(status)`. The final read after exit captures trailing lines.

For winetricks/protontricks output streaming, use the same loop. The event name `"prefix-deps-log"` and `"prefix-deps-complete"` follow existing naming conventions.

### 7. Tauri Command Structure

**File**: `src-tauri/src/commands/steam.rs:35`

```rust
#[tauri::command]
pub fn list_proton_installs(
    steam_client_install_path: Option<String>,
) -> Result<Vec<ProtonInstall>, String> {
    // ...
    Ok(installs)
}
```

All commands use `snake_case` names (Tauri requirement). Errors are serialized to `String` via `.map_err(|e| e.to_string())`. No business logic in command functions — delegate to `crosshook-core`.

**File**: `src-tauri/src/commands/settings.rs:137`

For async commands with state injection:

```rust
#[tauri::command]
pub fn settings_load(
    store: State<'_, SettingsStore>,
    profile_store: State<'_, crosshook_core::profile::ProfileStore>,
) -> Result<AppSettingsIpcData, String> {
    let data = store.load().map_err(map_settings_error)?;
    // ...
}
```

### 8. IPC DTO Split (Load vs Save)

**File**: `src-tauri/src/commands/settings.rs`

The settings layer uses separate structs: `AppSettingsIpcData` (returned by load, includes computed fields like `has_steamgriddb_api_key`) and `SettingsSaveRequest` (accepted by save, omits sensitive/computed fields). This prevents accidentally overwriting API keys from frontend round-trips.

Apply the same pattern for prefix dep state: the load DTO includes `checked_at`, `last_error`, `state` string; the write DTO may accept fewer fields.

### 9. Serde Backward Compatibility for TOML Fields

**File**: `profile/models.rs:221`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerSection {
    #[serde(default)]
    pub path: String,
    // ...
}
```

New fields on TOML-persisted structs must use `#[serde(default)]` (and `#[serde(skip_serializing_if = "Vec::is_empty")]` for Vec fields). This keeps existing profile TOML files valid when the new field is absent. The `AppSettingsData` struct (settings/mod.rs:126) uses `#[serde(default)]` on the entire struct.

### 10. React Hook Pattern with Cleanup Flag

**File**: `src/hooks/useProtonInstalls.ts:40`

```typescript
useEffect(() => {
    let active = true;
    async function loadProtonInstalls() {
        try {
            const result = await invoke<T>('command_name', { ... });
            if (!active) return;
            setState(result);
        } catch (err) {
            if (!active) return;
            setError(normalizeLoadError(err));
        }
    }
    void loadProtonInstalls();
    return () => { active = false; };
}, [reloadVersion, dependency]);
```

All new `usePrefixDeps` hooks must follow this pattern. The `active` flag prevents state updates after unmount. Use `reloadVersion` counter (incremented by `setReloadVersion(v => v + 1)`) to trigger manual reloads without changing other deps.

### 11. Frontend Event Listener with Cleanup

**File**: `src/hooks/useLaunchState.ts:225`

```typescript
useEffect(() => {
  let active = true;
  const unlistenLog = listen<string>('prefix-deps-log', (event) => {
    if (!active) return;
    setLogLines((lines) => [...lines, event.payload]);
  });
  const unlistenComplete = listen<number | null>('prefix-deps-complete', (event) => {
    if (!active) return;
    setExitCode(event.payload);
  });
  return () => {
    active = false;
    void unlistenLog.then((unlisten) => unlisten());
    void unlistenComplete.then((unlisten) => unlisten());
  };
}, []);
```

`listen()` returns a `Promise<UnlistenFn>` — both the promise and the returned function must be cleaned up on unmount.

### 12. Settings DTO Mirror Pattern

**File**: `src/types/settings.ts`

`SettingsSaveRequest` is the frontend write type; `AppSettingsData` extends it with computed/read-only fields. The `toSettingsSaveRequest()` function strips computed fields before sending to IPC. Apply this same split for any new settings fields.

---

## Integration Points

### Files to Modify

| File                                        | Change                                                                                                                       |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `crosshook-core/src/lib.rs`                 | Add `pub mod prefix_deps;`                                                                                                   |
| `crosshook-core/src/profile/models.rs`      | Add `required_protontricks: Vec<String>` to `TrainerSection` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]` |
| `crosshook-core/src/settings/mod.rs`        | Add `protontricks_binary_path: String` to `AppSettingsData` with `#[serde(default)]`; add to `Default` impl and `Debug` impl |
| `crosshook-core/src/metadata/migrations.rs` | Add `migrate_14_to_15()` and the `if version < 15` guard in `run_migrations()`                                               |
| `crosshook-core/src/metadata/mod.rs`        | Add dep-state CRUD methods using `with_conn()` / `with_conn_mut()`                                                           |
| `src-tauri/src/lib.rs`                      | Add `.manage(commands::prefix_deps::PrefixDepsInstallState::new())` and register 4 new commands in `invoke_handler!`         |
| `src-tauri/src/commands/mod.rs`             | Add `pub mod prefix_deps;`                                                                                                   |
| `src/types/profile.ts`                      | Add `required_protontricks?: string[]` to `trainer` section of `GameProfile`                                                 |
| `src/types/settings.ts`                     | Add `protontricks_binary_path: string` to `SettingsSaveRequest` and `AppSettingsData`                                        |
| `src/hooks/useScrollEnhance.ts`             | Register `PrefixDepsPanel`'s scroll container selector in `SCROLLABLE` if it uses `overflow-y: auto`                         |

### Files to Create

| File                                          | Purpose                                                                                                        |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `crosshook-core/src/prefix_deps/mod.rs`       | Module root; exports public API                                                                                |
| `crosshook-core/src/prefix_deps/detector.rs`  | Binary detection (`resolve_winetricks_path`, `resolve_protontricks_path`) using PATH walk                      |
| `crosshook-core/src/prefix_deps/checker.rs`   | `winetricks list-installed` runner; parses output into `Vec<String>`                                           |
| `crosshook-core/src/prefix_deps/installer.rs` | `winetricks <pkg>` / `protontricks <appid> <pkg>` runner with streaming                                        |
| `crosshook-core/src/prefix_deps/models.rs`    | `DepCheckRequest`, `DepInstallRequest`, `DepState` enum, result types                                          |
| `src-tauri/src/commands/prefix_deps.rs`       | 4 IPC commands: `check_prefix_deps`, `install_prefix_dep`, `cancel_prefix_dep_install`, `get_prefix_dep_state` |
| `src/hooks/usePrefixDeps.ts`                  | React hook mirroring `useProtonInstalls.ts` structure                                                          |
| `src/components/PrefixDepsPanel.tsx`          | React component; embeds in `ProfilesPage.tsx`                                                                  |

---

## Code Conventions

### Rust Naming

- Module files: `mod.rs` with submodules as sibling files
- Functions: `snake_case`
- Error types: Enum with structured variants (`Database { action, source }`, `Io { action, path, source }`, `Validation(String)`)
- Errors use **custom typed enums only** — no `anyhow`, no `Box<dyn Error>` in `crosshook-core` public APIs. Pattern: `Database { action: &'static str, source: rusqlite::Error }`, `Io { action, path, source }`, `Validation(String)`.

### TypeScript Naming

- Hooks: `camelCase` (`usePrefixDeps`)
- Components: `PascalCase` (`PrefixDepsPanel`)
- IPC field names: `snake_case` (match Rust serde output exactly)
- CSS classes: BEM-like `crosshook-*` prefix

### IPC Command Names (Tauri)

All `#[tauri::command]` functions use `snake_case`. The JS side calls `invoke('command_name')` — names must match exactly. Based on existing patterns, the 4 new commands should be named:

- `check_prefix_deps`
- `install_prefix_dep`
- `cancel_prefix_dep_install`
- `get_prefix_dep_state`

### State Registration in `lib.rs`

Every new process-scoped state struct must be `.manage()`-ed in `lib.rs` before `invoke_handler!`. The `PrefixDepsInstallState` (holding the concurrent lock) follows `UpdateProcessState::new()`.

---

## Dependencies and Services

### Rust Crates Already in Use

- `tokio` (async runtime, `tokio::process::Command`)
- `rusqlite` (SQLite connection)
- `serde` / `serde_json` / `toml` (serialization)
- `chrono` (timestamps — use `Utc::now()`)
- `uuid` (for `db::new_id()` — reuse for row IDs)
- `directories` (platform path resolution)
- `tracing` (structured logging — use `tracing::warn!`, `tracing::info!`, etc.)
- `anyhow` is NOT used in `crosshook-core` — only in `src-tauri` layer if at all

### Frontend Packages Already in Use

- `@tauri-apps/api/core` (`invoke`)
- `@tauri-apps/api/event` (`listen`)
- `react` (hooks: `useState`, `useEffect`, `useCallback`, `useReducer`, `useRef`)

---

## Gotchas and Warnings

### 1. Never `env_clear()` for winetricks/protontricks

Proton commands use `Command::new(...).env_clear()`. winetricks/protontricks require HOME, USER, PATH, XDG_RUNTIME_DIR from the host. Use `apply_host_environment()` after constructing the command — do NOT call `.env_clear()` for these tools.

### 2. Never Shell-Interpolate Arguments

All subprocess arguments must be passed via `.arg()`, never via shell interpolation. This is a security requirement enforced across the codebase. Package names from the profile must be passed as individual `.arg()` calls, never joined into one string. **Always add `cmd.arg("--")` before any verb/package args** — this is security-critical (S-06: flag injection prevention). Never skip the separator.

### 3. MetadataStore Can Be Disabled

`MetadataStore` has a `.disabled()` state where `available == false`. The `with_conn()` method returns `T::default()` silently in that case. Dep-state CRUD methods must handle this gracefully — callers should not assume the store is always available.

### 4. Settings IPC Has a Split DTO

`AppSettingsData` (Rust internal) does not map 1:1 to what the frontend receives. The IPC layer in `settings.rs` has a separate `AppSettingsIpcData` that includes computed fields (`has_steamgriddb_api_key`, `resolved_profiles_directory`). When adding `protontricks_binary_path`, add it to `AppSettingsData`, `SettingsSaveRequest` (in `commands/settings.rs`), and `AppSettingsIpcData`. The `merge_settings_from_request()` function must be updated to copy the new field.

### 5. Scroll Containers Must Register in `useScrollEnhance`

Any React component that introduces a new `overflow-y: auto` container must add its selector to the `SCROLLABLE` constant in `useScrollEnhance.ts`. Omitting this causes dual-scroll jank on WebKitGTK. The `PrefixDepsPanel` will need this if it has a scrollable log area.

### 6. Migration Must Use `execute_batch`, Not Individual Statements

All migrations use `conn.execute_batch(sql)` with a multi-statement SQL string. Do not call `conn.execute()` per statement — that pattern is not used in migrations and may not handle transactions correctly.

### 7. `db::open_in_memory()` for Tests

Migration tests use `db::open_in_memory()` (see migrations.rs:683). New migration tests should follow the same pattern — create an in-memory store, run `run_migrations()`, then assert the new table/index exists.

### 8. Flatpak protontricks Needs Special Handling

If the detected `protontricks` binary is under a Flatpak path (e.g., `flatpak run com.github.Matoking.protontricks`), the invocation changes. The `WINEPREFIX` env var must be set because Flatpak-sandboxed protontricks cannot read the host prefix path directly in all configurations. This edge case is documented in `research-external.md` — implement a Flatpak detection branch in the installer.

### 9. The `with_conn_mut` Pattern for Write Operations

`MetadataStore` has both `with_conn` (read) and `with_conn_mut` (read-write). Use `with_conn_mut` for upsert/insert/delete operations. The difference is the closure receives `&mut Connection` allowing transactions.

### 10. Prefix Path Key in SQLite is the Parent of `pfx/`

The `STEAM_COMPAT_DATA_PATH` (parent directory, e.g. `steamapps/compatdata/<APPID>`) is what identifies a prefix — not the `pfx/` subdirectory inside it. The `prefix_dependency_state` table's `prefix_path` column must store this parent path. Use `resolve_proton_paths()` from `runtime_helpers.rs` to derive it from whatever path the user has configured — it returns both `compat_data_path` and `wine_prefix_path`.

---

## Task-Specific Guidance

### Phase 1: Core Module (`prefix_deps`) Setup

1. Create `crosshook-core/src/prefix_deps/mod.rs` — empty module initially, add `pub mod` entries as submodules are created.
2. Add `pub mod prefix_deps;` to `crosshook-core/src/lib.rs` (between existing entries, alphabetical order matches current file).
3. In `detector.rs`, implement `resolve_winetricks_path()` and `resolve_protontricks_path()` using the `resolve_umu_run_path` pattern verbatim (lines 301-312 of `runtime_helpers.rs`).

### Phase 2: SQLite Migration

1. Add `migrate_14_to_15()` to `metadata/migrations.rs` with the `prefix_dependency_state` DDL.
2. Add the `if version < 15` guard in `run_migrations()` immediately after the `version < 14` block (line 138).
3. Add a test `migration_14_to_15_creates_prefix_dependency_state_table()` following the pattern at line 696.

### Phase 3: Profile and Settings Model Extension

1. In `profile/models.rs`, add to `TrainerSection`:

   ```rust
   #[serde(default, skip_serializing_if = "Vec::is_empty")]
   pub required_protontricks: Vec<String>,
   ```

   Update `TrainerSection::default()` to include `required_protontricks: Vec::new()`.

2. In `settings/mod.rs`, add to `AppSettingsData`:

   ```rust
   #[serde(default)]
   pub protontricks_binary_path: String,
   ```

   Update `Default` impl and `Debug` impl.

### Phase 4: IPC Commands

1. Create `src-tauri/src/commands/prefix_deps.rs` with `PrefixDepsInstallState` and 4 command functions.
2. Register state with `.manage(commands::prefix_deps::PrefixDepsInstallState::new())` in `lib.rs` before the plugin setup.
3. Add all 4 commands to `invoke_handler!` macro.
4. In `commands/mod.rs`, add `pub mod prefix_deps;`.

### Phase 5: Frontend

1. Create `usePrefixDeps.ts` following `useProtonInstalls.ts` structure.
2. Create `PrefixDepsPanel.tsx` — embed with `listen()` for `"prefix-deps-log"` and `"prefix-deps-complete"` events.
3. Update `profile.ts` and `settings.ts` types.
4. Register scroll container in `useScrollEnhance.ts` if panel has internal scroll.
