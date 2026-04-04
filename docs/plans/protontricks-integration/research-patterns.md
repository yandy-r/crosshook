# Pattern Research: protontricks-integration

## Overview

CrossHook follows a strict three-layer architecture: Tauri IPC commands (thin), Tauri command handlers (`src-tauri/src/commands/`), and `crosshook-core` for all business logic. New features for protontricks/winetricks integration must live in `crosshook-core` as a new submodule, expose thin `#[tauri::command]` handlers, and use a dedicated React hook wrapping `invoke()`. All SQLite state changes follow the incremental migration pattern already established in `metadata/migrations.rs`.

## Architectural Patterns

**Thin IPC Command Layer**: Tauri commands in `src-tauri/src/commands/` delegate directly to `crosshook-core`. They do input validation, state extraction from `State<'_>`, and serialize results to `String` errors — no business logic lives in them.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`

**Core Logic in `crosshook-core` Submodules**: Each feature domain is a directory under `crates/crosshook-core/src/` with a `mod.rs` re-exporting the public API. The new `prefix_deps` module should follow this pattern.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/mod.rs`
- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`

**Store Pattern**: Persistent state is encapsulated in `*Store` structs (e.g. `SettingsStore`, `ProfileStore`, `MetadataStore`). They hold a `base_path: PathBuf` or a connection handle, expose `load()` / `save()` / operation methods returning typed `Result<T, *StoreError>`, and are registered via `.manage()` in `lib.rs`. Commands receive stores as `State<'_, StoreType>`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:122`
- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs:199`

**Async Tauri Commands with `spawn_blocking` for Sync I/O**: Async commands (`pub async fn`) are used when the operation involves I/O or long-running work. Synchronous `rusqlite`/file operations that block the thread are wrapped in `tauri::async_runtime::spawn_blocking(move || { ... }).await`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs:385`

**Process Execution via `tokio::process::Command` with `env_clear()`**: All subprocess launches start with `Command::new(executable)`, call `env_clear()` to prevent environment leakage, then explicitly layer environment variables via `apply_host_environment()`, `apply_runtime_proton_environment()`, etc. The winetricks command must follow the same pattern.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs:21`

**Binary Detection via PATH Walk**: Checking whether a tool is available on `PATH` is done by iterating `env::split_paths()` and calling `is_executable_file()`. The same helper used for `umu-run` can be reused for `winetricks`/`protontricks` detection.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs:301`

**Catalog/Config File Pattern (TOML, Embedded + User Override)**: For lists of known items (optimizations catalog), the pattern is: embed a default TOML at compile time with `include_str!()`, merge with a user override file at `~/.config/crosshook/`, then store in a `OnceLock<Catalog>`. For protontricks, the list of known verbs/packages could reuse this approach if needed, but for simple validation, a static set suffices.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs:8`

**React Hook Pattern**: Each feature area has a dedicated hook in `src/hooks/use*.ts` that wraps `invoke()`, owns loading/error state, and returns a stable interface. The hook uses `useEffect` with a cleanup flag (`let active = true; ... return () => { active = false; }`) to prevent state updates on unmounted components.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonInstalls.ts`

**Tauri Event Streaming for Long-running Operations**: For operations that produce output over time (like a `winetricks` install), the pattern is to spawn a background task that reads output and emits events via `app.emit("event-name", payload)`. The frontend hooks into these with `listen()` from `@tauri-apps/api/event`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs:350`
- Frontend listener: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useLaunchState.ts:1`

## Code Conventions

**Rust naming**: `snake_case` for everything, modules as directories with `mod.rs`. Public API re-exported from `mod.rs`. Error types named `*StoreError` or `*Error` as enums with `Display` + `std::error::Error` implementations.

**TypeScript naming**: `PascalCase` for components, `camelCase` for hooks and functions. Hook files named `use*.ts`. Type definition files per domain in `src/types/*.ts`. Types re-exported from `src/types/index.ts`.

**Type mirror pattern**: Rust structs that cross the IPC boundary derive `Serialize + Deserialize` with `serde`. TypeScript interfaces mirror them exactly, with `snake_case` field names matching the Rust serialization. See the `LaunchRequest` / `LaunchRequest` mirror:

- Rust: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs:23`
- TypeScript: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/launch.ts:23`

**Command registration**: Every `#[tauri::command]` function must be listed in `tauri::generate_handler![...]` in `lib.rs`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs:208`

**Serde on IPC types**: Use `#[serde(default)]`, `#[serde(rename = "...")]`, and `#[serde(skip_serializing_if = "...")]` to control serialization. Use `#[serde(rename_all = "snake_case")]` on enums.

**Profile TOML sections**: New profile fields go into the appropriate section struct in `models.rs` (e.g. `TrainerSection`, `RuntimeSection`) or a new top-level section struct. All fields require `#[serde(default)]` so existing TOML files remain valid.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs:220`

**CSS classes**: BEM-like `crosshook-*` prefix for all component CSS classes. CSS variables defined in `src/styles/variables.css`.

**Scroll containers**: Any new `overflow-y: auto` container must be added to the `SCROLLABLE` selector in `src/hooks/useScrollEnhance.ts`.

## Error Handling

**Rust errors**: The `crosshook-core` modules use typed error enums (e.g. `SettingsStoreError`, `MetadataStoreError`) that implement `Display`, `std::error::Error`, and `From<>` for wrapped error types. Tauri commands convert these to `String` at the IPC boundary via `.map_err(|e| e.to_string())`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:214`

**Fail fast, no silent fallbacks**: The `crosshook-core` functions return `Result<T, E>` and propagate errors. The Tauri command layer propagates them as `Result<T, String>`. The pattern is `validate(...).map_err(|e| e.to_string())?` — no swallowing.

**Logging with `tracing`**: The project uses `tracing` crate macros (`tracing::warn!`, `tracing::info!`, `tracing::error!`, `tracing::debug!`). Use structured fields: `tracing::warn!(%error, "message")` or `tracing::warn!(field = %value, "message")`.

**MetadataStore graceful degradation**: The `MetadataStore` can be `disabled()` (SQLite not available). Operations on it check availability with `metadata_store.is_available()` and silently skip if unavailable. New SQLite-backed state should follow the same pattern.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs:48`

**TypeScript errors**: Hook-level errors normalize to `string | null` via a `normalizeLoadError()` helper pattern. Components receive `error: string | null` from hooks and render error UI accordingly.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonInstalls.ts:16`

## Testing Approach

**Unit tests inline in the module**: Rust tests are `#[cfg(test)] mod tests { ... }` at the bottom of the file. They use `tempfile::tempdir()` for filesystem isolation. No mocking of `MetadataStore` or `ProfileStore` — tests use real in-memory or temp-dir instances.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:297`
- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs:336`

**Test naming convention**: Descriptive snake_case names expressing what is being tested and expected, e.g. `build_gamescope_args_default_returns_empty`, `load_returns_default_settings_when_file_is_missing`.

**Test run command**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

**No frontend test framework**: There is no configured Jest or Vitest setup. Frontend behavior is validated via dev/build scripts.

**For `prefix_deps` module**: Write unit tests covering:

1. Verb/package name validation (reject injection characters).
2. Binary detection returning `None` when tool absent.
3. `list-installed` output parsing (space/newline-delimited verbs).
4. Command construction (verify args, no shell expansion).

## Patterns to Follow for protontricks-integration

**New Rust module**: Create `crosshook-core/src/prefix_deps/` with `mod.rs`, `detection.rs`, `runner.rs`, `models.rs`. Export public API from `mod.rs`. Add `pub mod prefix_deps;` to `crosshook-core/src/lib.rs`.

**New Tauri commands file**: Create `src-tauri/src/commands/prefix_deps.rs`. Add `mod prefix_deps;` to `src-tauri/src/commands/mod.rs`. Register commands in `lib.rs` `generate_handler![]` macro.

**Process invocation**: Use `tokio::process::Command::new(binary_path)` with `.arg()` calls only (never shell string interpolation). Call `.env_clear()` then apply `apply_host_environment()` and add `WINEPREFIX`. Use `--` separator before verb arguments to prevent flag injection.

**Streaming install output**: Follow the `spawn_log_stream` / `app.emit("prefix-dep-log", line)` pattern from `commands/launch.rs:350`. The frontend hook should listen via `listen("prefix-dep-log", ...)` and `listen("prefix-dep-complete", ...)`.

**SQLite migration**: Add `migrate_14_to_15()` in `metadata/migrations.rs` following the `if version < N { migrate... }` guard pattern. Create a `prefix_dependency_state` table tracking dependency state per profile/package/prefix.

**Profile TOML schema extension**: Add `required_protontricks: Vec<String>` to `TrainerSection` in `models.rs` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. This preserves backward compatibility with existing TOML files.

**Settings for tool path override**: Add `protontricks_binary_path: String` to `AppSettingsData` with `#[serde(default)]` (empty = auto-detect and PATH discovery). Keep the unified binary-path model aligned with existing `default_proton_path` style settings.

**TypeScript types**: Add `PrefixDepsInstallRequest`, `PrefixDepsInstallResult`, `PrefixDepsStatus` interfaces to `src/types/prefix-deps.ts`. Mirror Rust struct field names exactly. Export from `src/types/index.ts`.

**React hook**: Create `src/hooks/usePrefixDeps.ts` following `useProtonInstalls.ts` structure. Expose `{ status, installedVerbs, install, checkDeps, error }`.
