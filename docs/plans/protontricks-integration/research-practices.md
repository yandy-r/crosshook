# Engineering Practices Research: Protontricks Integration

## Executive Summary

CrossHook already has all the primitives needed for protontricks integration: `tokio::process::Command` with env-clear + controlled env injection, a multi-step SQLite migration runner, TOML-backed profile models with `#[serde(default)]` on every field, and a Tauri IPC pattern of thin `#[tauri::command]` wrappers delegating to `crosshook-core`. The simplest viable approach — shell out to the `protontricks` CLI with controlled args, parse exit code, store installed-package state in a new SQLite table via migration 15 — fits cleanly into every existing pattern and requires no new crates. Trait-based abstraction for the runner is warranted for testability, matching how the launch module uses `test_support::ScopedCommandSearchPath`.

---

## Existing Reusable Code

| File                                                                         | Purpose                                                                                                                                                  |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`   | `tokio::process::Command` construction, env helpers, `resolve_wine_prefix_path`, `is_executable_file`, path resolution utilities — all directly reusable |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`     | Canonical pattern for building, env-seeding, and attaching log stdio to a child process                                                                  |
| `src/crosshook-native/crates/crosshook-core/src/install/service.rs`          | Blocking install via `Handle::try_current().block_on(child.wait())` — same pattern applies to synchronous protontricks invocation                        |
| `src/crosshook-native/crates/crosshook-core/src/update/service.rs`           | Async update via `spawn()` + return child — shows the async alternative                                                                                  |
| `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`      | Incremental `if version < N` migration runner (currently at v14) — add migration 15 here                                                                 |
| `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs`              | `open_at_path`, `open_in_memory` (for tests), connection configuration                                                                                   |
| `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`    | Template for a new SQLite sub-store: `upsert_*` / `load_*` / `lookup_*` all using `Connection` directly                                                  |
| `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` | `CommunityProfileManifest` / `CommunityProfileMetadata` — add `required_protontricks` here                                                               |
| `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`           | `GameProfile` / `RuntimeSection` — shows field naming conventions, `#[serde(default, skip_serializing_if = "Vec::is_empty")]` pattern                    |
| `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`          | `MetadataStoreError` — custom error enum with `Database { action, source }` and `Io { action, path, source }` variants; new module should mirror this    |
| `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`           | `git_command()` factory + `validate_branch_name` / `validate_tap_url` — security template for CLI arg validation                                         |
| `src/crosshook-native/crates/crosshook-core/src/launch/test_support.rs`      | `ScopedCommandSearchPath` — testability pattern using `OnceLock<Mutex<()>>` to swap a global binary search path                                          |
| `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`     | `is_executable_file` (via `runtime_helpers`) — reuse for protontricks binary detection                                                                   |
| `src/crosshook-native/src-tauri/src/commands/install.rs`                     | IPC wrapper template: thin `#[tauri::command]` async fn → `spawn_blocking` → core function → `.map_err(                                                  | e   | e.to_string())` |
| `src/crosshook-native/src-tauri/src/commands/shared.rs`                      | `create_log_path`, `slugify_target` — reuse for protontricks log path creation                                                                           |
| `src/crosshook-native/src-tauri/src/lib.rs`                                  | State injection pattern: `MetadataStore`, `ProfileStore` injected via `.manage()` and accessed via `State<T>`                                            |

---

## Modularity Design

- **Process construction via `runtime_helpers`**: All binary invocations start with `env_clear()`, then selectively apply `apply_host_environment` (HOME, PATH, DISPLAY, etc.) via helper functions. Protontricks needs `HOME`, `PATH`, `STEAM_COMPAT_DATA_PATH`, and `WINEPREFIX` — all available in `apply_host_environment` + `apply_runtime_proton_environment`. Do not skip `env_clear()`.

- **Prefix path resolution**: `resolve_wine_prefix_path` (in `runtime_helpers.rs:198`) and `resolve_proton_paths` normalize the `pfx/` sub-directory convention for Proton prefixes. Protontricks must receive the `STEAM_COMPAT_DATA_PATH` (parent of `pfx/`), not the `pfx/` path itself. Always use `resolve_proton_paths` to derive both.

- **Blocking process execution**: `install/service.rs` uses `Handle::try_current().block_on(child.wait())` inside a `spawn_blocking` closure — the correct pattern for synchronous Proton-family tool invocations from a `#[tauri::command]` context. Protontricks runs are inherently blocking (can take minutes for vcrun2019); use the same approach.

- **SQLite sub-stores**: Every store function in `metadata/health_store.rs`, `metadata/launch_history.rs`, etc. takes a bare `&Connection` and returns `Result<T, MetadataStoreError>`. The `MetadataStore` struct holds `Arc<Mutex<Connection>>` and dispatches via `with_conn`. New prefix-dependency store functions follow this exact shape.

- **Migration runner**: `metadata/migrations.rs` uses `if version < N { migrate_N_minus_1_to_N(conn)?; pragma_update(version, N); }` for each step. The next migration is 14 → 15. It is additive-only (new tables, new columns via `ALTER TABLE ... ADD COLUMN`).

- **IPC thin wrapper**: Every `#[tauri::command]` in `src-tauri/src/commands/` is a thin async fn that: (1) builds a log path via `create_log_path`, (2) calls `tauri::async_runtime::spawn_blocking(|| core_fn(...).map_err(|e| e.to_string()))`, (3) returns `Result<SomeType, String>`. Never put business logic in the IPC layer.

- **Profile schema extension**: Profile fields use `#[serde(default, skip_serializing_if = "Vec::is_empty")]` so new optional fields are backward-compatible without explicit version bumps. `CommunityProfileManifest` wraps `GameProfile`, so `required_protontricks` belongs on `GameProfile` (or a dedicated `PrefixDepsSection`), not on `CommunityProfileMetadata`.

- **Binary availability check**: `resolve_umu_run_path` in `runtime_helpers.rs:302` walks `PATH` and calls `is_executable_file`. Use the same function signature pattern to implement `resolve_protontricks_path` and `resolve_winetricks_path`.

- **Event emission for long operations**: `launch.rs` emits `"launch-log"` lines from a polling loop and `"launch-complete"` / `"launch-diagnostic"` at exit. Protontricks installation should emit `"protontricks-log"` and `"protontricks-complete"` events using the same `AppHandle::emit` pattern.

- **Error enum convention**: Error types are custom enums (not `anyhow`) with variants that carry structured context: `Database { action: &'static str, source: SqlError }`, `Io { action: &'static str, path: PathBuf, source: io::Error }`. `anyhow` is not in `crosshook-core`'s dependencies — do not add it.

---

## KISS Assessment

| Option                                                                       | Complexity                                    | Fit                                                                              | Recommendation                          |
| ---------------------------------------------------------------------------- | --------------------------------------------- | -------------------------------------------------------------------------------- | --------------------------------------- |
| Shell out to `protontricks <app_id> <packages>`                              | Low — `Command::new("protontricks")` + args   | Exact match to existing `git_command()` and `new_direct_proton_command` patterns | **Preferred**                           |
| Shell out to `winetricks --no-isolate <packages>` with explicit `WINEPREFIX` | Low — identical to above, different binary    | Fallback when protontricks unavailable                                           | **Preferred as fallback**               |
| Embed winetricks shell script                                                | Medium — ship script in AppImage, maintain    | Adds coupling to upstream shell script versioning                                | Avoid unless offline-first is mandatory |
| Implement verb logic natively in Rust                                        | Very High — reimplement 500+ winetricks verbs | No benefit, massive surface area                                                 | Reject                                  |
| Use `protontricks-launch` wrapper                                            | Low — same Command pattern                    | Only needed if running apps in the prefix (not applicable here)                  | Not applicable                          |

The simplest viable approach: `Command::new("protontricks")`, pass `steam_app_id` (or `0` for non-Steam prefixes using `--no-steam`) and package names as positional args. Capture stdout/stderr to a log file via `attach_log_stdio`. Parse exit code (0 = success). Store result in SQLite. This is three new files: `prefix_deps/mod.rs`, `prefix_deps/runner.rs`, `prefix_deps/store.rs`.

---

## Abstraction vs. Repetition

**Extract (shared, reuse existing):**

- `resolve_wine_prefix_path` / `resolve_proton_paths` — already public in `runtime_helpers`; call directly
- `apply_host_environment` — already public; call directly
- `attach_log_stdio` — already public; call directly
- `create_log_path` / `slugify_target` — already in `src-tauri/src/commands/shared.rs`; reuse
- `MetadataStoreError` variants — use the existing error type for DB operations; define a new `PrefixDepsError` for the module boundary

**Repeat (do not over-abstract):**

- The `Command` construction for protontricks is specific enough that a `protontricks_command()` factory function inside the new module (mirroring `git_command()`) is sufficient — do not add a generic "external tool runner" abstraction
- IPC command wrappers are intentionally repetitive; follow the `install.rs` template directly

**New abstraction warranted:**

- A `ProtontricksRunner` trait with a single `run(prefix_path, steam_app_id, packages) -> Result<RunOutput, PrefixDepsError>` method, with a `RealRunner` (shells out) and a `FakeRunner` (returns canned results) implementation. This is the minimum needed for unit testing without requiring protontricks installed. Follow the `ScopedCommandSearchPath` precedent for test isolation.

---

## Interface Design

The prefix-dependencies module's public API (in `crosshook-core::prefix_deps`) should expose:

```rust
// Detection
pub fn resolve_protontricks_path() -> Option<String>;  // mirrors resolve_umu_run_path
pub fn resolve_winetricks_path() -> Option<String>;

// Core operation
pub fn install_prefix_dependencies(
    request: &PrefixDepsRequest,
    log_path: &Path,
) -> Result<PrefixDepsResult, PrefixDepsError>;

// State query (used by health check and launch pre-flight)
pub fn check_dependencies_installed(
    prefix_path: &str,
    packages: &[String],
) -> PrefixDepsCheckResult;

// Types
pub struct PrefixDepsRequest {
    pub prefix_path: String,         // raw path (run through resolve_proton_paths internally)
    pub steam_app_id: String,        // "0" for non-Steam; passed as first arg to protontricks
    pub packages: Vec<String>,       // e.g. ["vcrun2019", "dotnet48"]
    pub proton_path: String,         // needed for WINEPREFIX env population
    pub steam_client_install_path: String,
    pub force_reinstall: bool,
}

pub struct PrefixDepsResult {
    pub succeeded: bool,
    pub installed_packages: Vec<String>,
    pub skipped_packages: Vec<String>,   // already installed per SQLite state
    pub log_path: String,
}

pub enum PrefixDepsError {
    ProtontricksNotFound,
    PrefixPathInvalid { path: PathBuf },
    PackageNameInvalid { name: String },
    SpawnFailed { message: String },
    ProcessFailed { exit_code: Option<i32> },
    Database { action: &'static str, source: rusqlite::Error },
}
```

The Tauri IPC layer exposes:

- `check_protontricks_available() -> bool` (synchronous)
- `install_prefix_dependencies(request: PrefixDepsRequest) -> Result<PrefixDepsResult, String>` (async, `spawn_blocking`)
- Optionally: `get_prefix_dependency_state(profile_name: String) -> Result<Vec<InstalledPackageRow>, String>`

---

## Testability Patterns

**Recommended:**

- Define a `ProtontricksRunner` trait in `prefix_deps/runner.rs` with a `run` method. `RealRunner` shells out; `FakeRunner` takes a `HashMap<Vec<String>, Result<(), String>>` lookup.
- Use `open_in_memory()` from `metadata/db.rs` for all SQLite tests — this is the established pattern throughout the metadata module.
- Test `check_dependencies_installed` against a seed-populated in-memory DB.
- For path resolution tests, follow `ScopedCommandSearchPath` (a `OnceLock<Mutex<Option<PathBuf>>>`) to swap the binary search path in tests.
- Test package name validation exhaustively (flag injection, shell metacharacters, empty strings) as pure unit tests — no process spawning needed.

**Anti-patterns to avoid:**

- Do not spawn real protontricks in unit tests — it may not be installed and would be slow.
- Do not use `std::env::set_var` without a mutex guard in tests (data race; `ScopedCommandSearchPath` shows the correct approach).
- Do not mock `std::process::Command` at the type level — use a trait for the runner boundary instead.
- Do not write tests that depend on the presence of a real Wine prefix on disk; use `tempfile::tempdir()` for any path-existence checks.

---

## Build vs. Depend

| Need                          | Build in core                                                     | External library | Recommendation    | Rationale                                                                                               |
| ----------------------------- | ----------------------------------------------------------------- | ---------------- | ----------------- | ------------------------------------------------------------------------------------------------------- |
| Process spawning              | `tokio::process::Command` (already in deps)                       | —                | **Use existing**  | Already imported everywhere                                                                             |
| Path detection (which binary) | `is_executable_file` + PATH walk (already in `runtime_helpers`)   | —                | **Use existing**  | `resolve_umu_run_path` is the exact template                                                            |
| Package name validation       | 10-line regex-free validation                                     | —                | **Build in core** | Simple allowlist: `[a-z0-9_-]+`, reject starting with `-` (mirrors `validate_branch_name`)              |
| SQLite state tracking         | `rusqlite` (already in deps, bundled feature)                     | —                | **Use existing**  | No new crate needed                                                                                     |
| Log streaming                 | `attach_log_stdio` + `stream_log_lines` pattern (already in core) | —                | **Use existing**  | Identical to launch log streaming                                                                       |
| TOML schema extension         | `serde` + `toml` (already in deps)                                | —                | **Use existing**  | `#[serde(default, skip_serializing_if)]` on new fields                                                  |
| Async task management         | `tokio::async_runtime::spawn_blocking` (already in deps)          | —                | **Use existing**  | All existing IPC commands use this pattern                                                              |
| Structured error types        | Custom enum (already the project pattern)                         | `thiserror`      | **Build in core** | `anyhow` and `thiserror` are not in `crosshook-core` deps; custom enums match every existing error type |

No new crates are needed. `std::process::Command` is sufficient for synchronous protontricks invocations (blocking, single-stage); `tokio::process::Command` (already in tokio features) is used if async streaming output is desired.

---

## Open Questions

1. **Non-Steam prefix app ID**: Protontricks requires a Steam App ID as the first argument. For CrossHook-managed prefixes (not tied to a Steam game), the convention is unclear — `0` is commonly used but protontricks behavior varies. The `steam_app_id` field already exists on `RuntimeSection` and `LaunchRequest`; it should be plumbed through, defaulting to `"0"` (same as `resolved_umu_game_id_for_env`).

2. **Already-installed detection**: Protontricks has `protontricks --list` to show installed verbs, but parsing its output is fragile. The simpler approach is to track installed packages in SQLite and only skip re-installation if the user explicitly requests it — do not depend on protontricks output parsing for state.

3. **Blocking duration**: Some packages (dotnet48, vcrun2019) take several minutes. The `spawn_blocking` thread pool may need a longer timeout consideration. Tauri's default `spawn_blocking` pool is unbounded; this is acceptable but should be documented.

4. **Winetricks fallback**: Should protontricks unavailability automatically fall back to winetricks with `WINEPREFIX` set? This is a UX decision, not a technical one — the runner trait makes it easy to add, but the policy needs to be specified.

5. **Prefix dependency state in community schema**: `required_protontricks: Vec<String>` should go on `GameProfile` (likely a new `PrefixDepsSection` with `#[serde(default, skip_serializing_if = "PrefixDepsSection::is_empty")]`), not on `CommunityProfileMetadata`. The section can also hold the tool preference (`protontricks` vs `winetricks`).
