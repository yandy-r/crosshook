# Plan: `[ui]` settings table for layout and display preferences

Tracking: Linear YAN-869 / GitHub yandy-r/crosshook#537 (M1-48). Target release `v0.7.0-alpha.1`, trunk-only, base `main`, branch `feat/yan-869-ui-settings-table`. Binding inputs: [`decisions.md`](./decisions.md) (D1–D13), [`issue.md`](./issue.md), [`feature-spec.md`](./feature-spec.md). Paths are relative to the repo root. In task bodies, `core/` = `src/crosshook-native/crates/crosshook-core/src`, `app/` = `src/crosshook-native/crates/crosshook-app/src`, `tauri/` = `src/crosshook-native/src-tauri/src`, `web/` = `src/crosshook-native/src`.

## Contents

Summary · Metadata · Batches · Worktree Setup · Mandatory Reading · Patterns to Mirror · B0 Contracts · Files to Change · NOT Building · Step-by-Step Tasks · Testing Strategy · Validation Commands · Acceptance Criteria · Risks · Notes

## Summary

Add a typed, lenient `[ui]` table (`UiSettings`) to `AppSettingsData`. A new `UiSettingsService` in `crosshook-app` holds the values in memory, clamps them, and persists only the dirty keys through `SettingsStore::update`, with a leading write plus one trailing write per second. A hardened, read-only importer copies Classic's WebKit `localStorage` view mode into `[ui].library_view_mode` once at bootstrap. Classic's `LibraryPage` moves from `localStorage` to the new narrow command `settings_set_library_view_mode`, and a user-facing settings reference is added.

## Metadata

- **Complexity**: L
- **Persistence**: `[ui]` keys are TOML settings; the WebKit file is read-once external input; the pending write and the failure latch are runtime-only. No SQLite change, no schema bump (see `feature-spec.md`).
- **Estimated files**: 9 new Rust source files, 6 new Rust test files, 1 new doc, about 30 modified files.

## Batches

Tasks in one batch run concurrently and touch **disjoint files** (audited in Files to Change). Batches run in order. Subagents skip builds mid-flight; the main agent compiles and verifies once at each batch gate. Test modules are declared (as `#[cfg(test)] mod …;`) by the batch that owns the production file, and the test files themselves land in B3, so B1 and B2 gates use `cargo check` and `cargo clippy` without `--all-targets`.

| Batch | Tasks           | Depends on | Width | What                                                                                             | Gate                                |
| ----- | --------------- | ---------- | ----- | ------------------------------------------------------------------------------------------------ | ----------------------------------- |
| B0    | none (this doc) | -          | -     | Frozen contracts (section **B0 Contracts**)                                                      | -                                   |
| B1    | 1.1 – 1.4       | B0         | 4     | Core: types and enums, lenient parse and palette, WebKit importer, wiring into `AppSettingsData` | core `check` + `clippy` + lib tests |
| B2    | 2.1 – 2.6       | B1         | 6     | App service and writer, app wiring, bootstrap import, Tauri, TS types and mocks, Classic UI      | workspace `check` + `clippy` + tsc  |
| B3    | 3.1 – 3.9       | B2         | 9     | All tests (Rust and Vitest), including the D11 fixture rewrites                                  | full Rust, Vitest, mock coverage    |
| B4    | 4.1, 4.2        | B3         | 2     | Docs: settings reference, quickstart link, app README, migration README alignment                | `format.sh --docs`                  |

- **Total tasks**: 21 · **Total batches**: 5 (B0 has no tasks) · **Max width**: 9

## Worktree Setup

- **Parent**: `/home/yandy/Projects/github.com/yandy-r/crosshook-yan-869/` (branch `feat/yan-869-ui-settings-table`).
- `./scripts/lint.sh` and `./scripts/format.sh` need `npm install -D --no-save typescript biome` at the root and `cd src/crosshook-native && npm ci` (CLAUDE.md, Worktree setup prerequisites).

## Mandatory Reading

| Priority | File                                                                         | Lines                           | Why                                                         |
| -------- | ---------------------------------------------------------------------------- | ------------------------------- | ----------------------------------------------------------- |
| P0       | `docs/plans/ui-settings/decisions.md`                                        | all                             | Binding D1–D13                                              |
| P0       | `core/settings/types.rs`                                                     | 48-82, 233-475                  | Nested-table precedent, `Default`, `clear_extra`, `Debug`   |
| P0       | `core/settings/store.rs`                                                     | 80-149                          | `load_unlocked`, `update`                                   |
| P0       | `app/services/settings.rs`                                                   | 104-187, 198-294                | `merge_save_request`, `normalize`, `SettingsService`        |
| P0       | `app/bootstrap/stores.rs`                                                    | 44-98                           | `Services` struct and `new`                                 |
| P0       | `app/bootstrap/app_bootstrap.rs`                                             | 76-175                          | Settings load, logging, `Services::new` call                |
| P0       | `tauri/commands/settings.rs`, `tauri/lib.rs`                                 | 13-160, 35-62, 125-140, 228-233 | DTO, commands, IPC-name test, handler list, `.run(context)` |
| P0       | `web/context/PreferencesContext.tsx`, `web/components/pages/LibraryPage.tsx` | 56-200; 28-36, 80-135           | Context value and the `localStorage` code to remove         |
| P1       | `core/settings/tests.rs`, `core/tests/unknown_settings_keys.rs`              | 1-60, 60-150                    | Fixture conventions and the fixtures D11 rewrites           |
| P1       | `app/log_tail/tests.rs`                                                      | 320-356                         | `start_paused` test pattern                                 |
| P1       | `core/profile/lutris_import/tests/paths_tests.rs`                            | 13-20                           | SQLite fixture builder                                      |
| P1       | `core/metadata/db.rs`                                                        | 13-33                           | Hardened read-only open flags                               |
| P1       | `scripts/check-mock-coverage.sh`                                             | 60-140                          | New command needs a mock                                    |
| P2       | `docs/plans/ui-settings/research-*.md`                                       | -                               | Evidence; decisions win on conflict                         |

## Patterns to Mirror

- **Nested table with extras**: `ConfigHistorySettings` (`core/settings/types.rs:48-69`) for `#[serde(default)]`, a manual `Default`, and `#[serde(flatten)] extra: toml::Table`.
- **Clamp consts + fns**: `clamp_recent_files_limit` and `RECENT_FILES_LIMIT_*` (`types.rs:8-37`).
- **Enum shape**: `ReducedMotionPreference` (`as_str`, `#[default]`).
- **Hand-written `Debug` that redacts**: `AppSettingsData` (`types.rs:408-470`).
- **Service over a store**: `SettingsService::update` (`app/services/settings.rs:244-260`) with `run_blocking`.
- **Event with `#[non_exhaustive]` enum**: `AppEvent::ProfilesChanged` published by `ProfileWriter` (`app/services/profiles/writer.rs:78`).
- **Read-only SQLite**: `metadata/db.rs:14-23` flags; fixture builders in `lutris_import/tests/paths_tests.rs`.
- **Paused-time tests**: `#[tokio::test(start_paused = true)]` in `app/log_tail/tests.rs:320`.
- **Narrow settings command**: `settings_save_steamgriddb_key` (`tauri/commands/settings.rs:147-160`) and its mock (`web/lib/mocks/handlers/settings.ts:28`).

## B0 Contracts

All later tasks code against these signatures. A task that needs a change stops and messages the main agent (`Main`). The app crate has `#![deny(missing_docs)]`, so every `pub` item in `app/` needs a doc comment.

### Core: `core/settings/ui.rs` + `core/settings/ui/` (`mod ui` is private; everything below is re-exported from `crosshook_core::settings`)

```rust
// ui.rs  (owner: 1.1)  — consts, clamps, struct, Default, Debug, normalize, keys
pub const UI_WINDOW_WIDTH_MIN: u32 = 800;
pub const UI_WINDOW_WIDTH_MAX: u32 = 16384;
pub const UI_WINDOW_HEIGHT_MIN: u32 = 600;
pub const UI_WINDOW_HEIGHT_MAX: u32 = 16384;
pub const UI_SCALE_MIN: f64 = 0.75;
pub const UI_SCALE_MAX: f64 = 2.0;
pub const UI_SCALE_DEFAULT: f64 = 1.0;
pub const UI_INSPECTOR_WIDTH_MIN: u32 = 240;
pub const UI_INSPECTOR_WIDTH_MAX: u32 = 640;
pub const UI_INSPECTOR_WIDTH_DEFAULT: u32 = 320;
pub const UI_CONSOLE_DRAWER_HEIGHT_MIN: u32 = 120;
pub const UI_CONSOLE_DRAWER_HEIGHT_MAX: u32 = 800;
pub const UI_CONSOLE_DRAWER_HEIGHT_DEFAULT: u32 = 280;
pub const PALETTE_RECENT_MAX: usize = 20;
pub const PALETTE_ID_MAX_BYTES: usize = 98;

pub fn clamp_window_width(n: u32) -> u32;
pub fn clamp_window_height(n: u32) -> u32;
/// Non-finite input returns `UI_SCALE_DEFAULT`; otherwise clamps to 0.75..=2.0.
pub fn clamp_ui_scale(n: f64) -> f64;
pub fn clamp_inspector_width(n: u32) -> u32;
pub fn clamp_console_drawer_height(n: u32) -> u32;

#[derive(Clone, PartialEq, serde::Serialize)]
pub struct UiSettings {
    #[serde(skip_serializing_if = "Option::is_none")] pub library_view_mode: Option<LibraryViewMode>,
    #[serde(skip_serializing_if = "Option::is_none")] pub window_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub window_height: Option<u32>,
    pub ui_scale: f64,
    pub density: UiDensity,
    pub renderer: UiRenderer,
    pub controller_mode: ControllerMode,
    pub sidebar_collapsed: bool,
    pub inspector_width: u32,
    pub console_drawer_height: u32,
    pub palette_recent: Vec<String>,
    /// Unknown `[ui]` keys, preserved verbatim.
    #[serde(flatten)] pub extra: toml::Table,
}
// Deserialize: hand-written in ui/lenient.rs, never returns Err (D2).
// Debug: hand-written; known keys, `palette_recent` as a count, `extra` as a key count (D10).
impl Default for UiSettings { /* ui_scale 1.0, Comfortable, Auto, Auto, false, 320, 280, [], no options */ }
impl UiSettings {
    /// `library_view_mode.unwrap_or_default()` (Grid).
    pub fn library_view_mode_or_default(&self) -> LibraryViewMode;
    /// Clamps every number, resets non-finite scale, sanitizes `palette_recent`. Silent. Returns `true` if anything changed.
    pub fn normalize(&mut self) -> bool;
    /// Moves `id` to the front, dedupes, caps at 20. Returns `false` (list unchanged) if `id` is not a valid palette id.
    pub fn push_palette_recent(&mut self, id: &str) -> bool;
    /// Copies the field(s) named by `key` from `src` (`WindowSize` copies width and height).
    pub fn copy_key_from(&mut self, src: &UiSettings, key: UiKey);
}

// ui/enums.rs  (owner: 1.1). Each: Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize,
// `pub fn as_str(self) -> &'static str`, `pub fn parse(s: &str) -> Option<Self>` (exact match, no trim).
#[serde(rename_all = "lowercase")] pub enum LibraryViewMode { #[default] Grid, List }
#[serde(rename_all = "snake_case")] pub enum UiDensity { Compact, #[default] Comfortable, Spacious }
#[serde(rename_all = "snake_case")] pub enum UiRenderer { #[default] Auto, Wgpu, Gl, #[serde(rename = "tiny-skia")] TinySkia }
#[serde(rename_all = "snake_case")] pub enum ControllerMode { #[default] Auto, On, Off }

// ui/keys.rs  (owner: 1.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKey { LibraryViewMode, WindowSize, UiScale, Density, Renderer, ControllerMode,
                 SidebarCollapsed, InspectorWidth, ConsoleDrawerHeight, PaletteRecent }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiKeySet(u16);
impl UiKeySet {
    pub const fn empty() -> Self;
    pub fn insert(&mut self, key: UiKey);
    pub fn contains(self, key: UiKey) -> bool;
    pub fn is_empty(self) -> bool;
    pub fn iter(self) -> impl Iterator<Item = UiKey>;
}

// ui/palette.rs  (owner: 1.2)
/// `^[a-z][a-z0-9-]{0,31}(:[a-z0-9][a-z0-9-]{0,31}){0,2}$`, at most 98 bytes. Hand-rolled, no regex dependency.
pub fn is_valid_palette_id(id: &str) -> bool;
/// Drops invalid ids, dedupes keeping the first occurrence, truncates to 20. Returns `(dropped_invalid, truncated)` counts.
pub(super) fn sanitize_palette_recent(list: &mut Vec<String>) -> (usize, usize);
/// Move-to-front insert used by `UiSettings::push_palette_recent`.
pub(super) fn push_recent(list: &mut Vec<String>, id: &str) -> bool;

// ui/lenient.rs  (owner: 1.2): `impl<'de> serde::Deserialize<'de> for UiSettings`
// 1. `toml::Value::deserialize(d)?`. Non-table -> `warn!(key = "ui", reason = "not_a_table")`, return Default.
// 2. Each known key is `remove`d from the table and parsed: wrong type -> default (`reason = "invalid_type"`);
//    unknown enum string -> default (`"invalid_value"`); integers are read as i64, then clamped (`"out_of_range"`);
//    ui_scale accepts float or integer, non-finite -> 1.0 (`"non_finite"`); library_view_mode/window_* wrong -> None.
// 3. palette_recent: non-array -> []; non-string or invalid elements dropped; one warn with counts only; dedupe; cap 20.
// 4. The remaining table becomes `extra`. Warnings use `tracing::warn!(key, reason, "ignoring invalid [ui] setting")`,
//    never the value.
```

```rust
// core/settings/webkit_import.rs  (owner: 1.3)  — `pub mod webkit_import;`
pub const WEBKIT_LOCAL_STORAGE_FILE: &str = "tauri_localhost_0.localstorage";
pub const WEBKIT_LIBRARY_VIEW_MODE_KEY: &str = "crosshook.library.viewMode";
pub const WEBKIT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// `base_dirs()?.data_local_dir()/<CURRENT_TAURI_APP_ID_DIR>/localstorage/<WEBKIT_LOCAL_STORAGE_FILE>`.
pub fn webkit_local_storage_path() -> Option<PathBuf>;
/// Hardened, read-only, never errors: lstat regular file only, <= 16 MiB, `file:<pct-encoded>?mode=ro&immutable=1`,
/// READ_ONLY|URI|NOFOLLOW|NO_MUTEX, `trusted_schema=OFF`, `query_only=ON`, `ItemTable` must be a table in sqlite_schema,
/// `SELECT value FROM ItemTable WHERE key = ?1 AND length(value) <= 16 LIMIT 1`, UTF-16LE decode, exactly "grid" or "list".
/// Any failure -> `None` with a fixed-reason `tracing::debug!` (no path, no value).
pub fn read_library_view_mode(path: &Path) -> Option<LibraryViewMode>;
/// `webkit_local_storage_path().and_then(|p| read_library_view_mode(&p))`.
pub fn import_library_view_mode() -> Option<LibraryViewMode>;
```

`core/settings/types.rs` (owner 1.4) gains `pub ui: UiSettings` (`#[serde(default)]`, declared before `extra`), `ui: UiSettings::default()` in `Default`, `self.ui.extra.clear()` in `clear_extra`, `self.ui.extra.clone_from(&current.ui.extra)` in `preserve_extra_from`, and `.field("ui", &self.ui)` in `Debug`.

### App: `app/`

```rust
// events.rs (owner 2.2)
#[non_exhaustive] pub enum AppEvent {
    /* existing variants */
    /// The first failed `[ui]` write of this session. Later failures only log.
    UiSettingsPersistFailed {
        /// Sanitized `AppError` text, safe to show.
        message: String,
    },
}

// services/settings/ui_writer.rs (owner 2.1) — pub(super) items
pub(super) type SinkFuture = Pin<Box<dyn Future<Output = Result<(), AppError>> + Send>>;
pub(super) type UiSink = Arc<dyn Fn(UiPatch) -> SinkFuture + Send + Sync>;
pub(super) struct UiPatch { pub(super) settings: UiSettings, pub(super) keys: UiKeySet }
pub(super) const MIN_WRITE_INTERVAL: Duration = Duration::from_secs(1);
pub(super) struct UiWriter { /* Arc<Shared>, DropGuard */ }
impl UiWriter {
    /// Spawns the writer task on `runtime`. Loop: wait for `Notify` -> write dirty keys now (leading edge)
    /// -> `sleep(MIN_WRITE_INTERVAL)` -> repeat. Changes made during the sleep leave a permit, giving one trailing write.
    /// The task exits when the last `UiWriter` clone drops (DropGuard on a CancellationToken).
    pub(super) fn spawn(initial: UiSettings, sink: UiSink, events: EventBus, runtime: &Handle) -> Self;
    pub(super) fn snapshot(&self) -> UiSettings;
    /// Applies `apply` to the in-memory copy, normalizes it, and if anything changed marks `key` dirty and wakes the task.
    pub(super) fn mutate(&self, key: UiKey, apply: impl FnOnce(&mut UiSettings)) -> bool;
    /// Writes the pending dirty keys immediately (serialized with the task by an async gate). `Ok(())` if nothing is pending.
    pub(super) async fn flush(&self) -> Result<(), AppError>;
}
// Write failure: log `error!`, drop the dirty keys (the next real change re-arms), and on the FIRST failure of the
// writer's life publish `AppEvent::UiSettingsPersistFailed { message }`. `flush` also returns the `Err`.

// services/settings/ui.rs (owner 2.1)
/// Typed `[ui]` getters and setters over an in-memory copy, with a throttled writer. Cheap to clone.
#[derive(Clone)] pub struct UiSettingsService { /* Arc<UiWriter> */ }
impl UiSettingsService {
    /// Loads `[ui]` from `store` (defaults if the file is unreadable) and spawns the writer on `runtime`.
    /// The production sink runs `store.update(|s| for k in patch.keys.iter() { s.ui.copy_key_from(&patch.settings, k) }; s.ui.normalize())`
    /// through `run_blocking`.
    pub fn new(store: SettingsStore, events: EventBus, runtime: Handle) -> Self;
    pub fn get(&self) -> UiSettings;
    pub fn set_library_view_mode(&self, mode: LibraryViewMode);
    pub fn set_window_size(&self, width: u32, height: u32);
    pub fn set_ui_scale(&self, scale: f64);
    pub fn set_density(&self, density: UiDensity);
    pub fn set_renderer(&self, renderer: UiRenderer);
    pub fn set_controller_mode(&self, mode: ControllerMode);
    pub fn set_sidebar_collapsed(&self, collapsed: bool);
    pub fn set_inspector_width(&self, width: u32);
    pub fn set_console_drawer_height(&self, height: u32);
    /// `false` when `id` is not a valid palette id.
    pub fn push_palette_recent(&self, id: &str) -> bool;
    pub async fn flush(&self) -> Result<(), AppError>;
}
// services/settings.rs (owner 2.2): `mod ui; mod ui_writer; pub use ui::UiSettingsService;`
//   + `#[cfg(test)] mod ui_tests; #[cfg(test)] mod ui_writer_tests;`
//   merge_save_request: `ui: current.ui,` ; normalize(): `settings.ui.normalize();` ; SettingsSaveRequest UNCHANGED (D4).
// services/mod.rs (owner 2.2): add UiSettingsService to the `pub use settings::{…}` line.
// lib.rs (owner 2.2): `pub use services::{LaunchService, ProfileWriter, UiSettingsService};`
// app.rs (owner 2.2): `pub(crate) fn runtime_handle(&self) -> Handle` (clone of the runtime handle).

// bootstrap/stores.rs (owner 2.3)
pub struct Services { /* existing */ /// Typed `[ui]` settings. pub ui_settings: UiSettingsService }
impl Services { pub fn new(stores: &Stores, events: EventBus, runtime: Handle) -> Self; }

// bootstrap/ui_import.rs (owner 2.3), declared `mod ui_import;` in bootstrap/mod.rs
/// Runs after `init_logging_step`. Skips unless roots are `StoreRoots::Xdg`, settings were readable,
/// and `settings.ui.library_view_mode.is_none()`. Adds no BootstrapStep.
pub(super) fn import_step(store: &SettingsStore, settings: &AppSettingsData, unreadable: bool, roots: &StoreRoots);
/// Persists `Some(mode)` via `store.update`, re-checking inside the closure that the key is still `None`.
/// Writes nothing for `None`. Returns whether it wrote.
pub(super) fn apply_import(store: &SettingsStore, imported: Option<LibraryViewMode>) -> bool;
// app_bootstrap.rs: call `ui_import::import_step(..)` right after `init_logging_step`/`relog_degraded_phase_a`;
// `Services::new(&stores, app.events().clone(), app.runtime_handle())`.
```

### Tauri: `tauri/`

```rust
// commands/settings.rs (owner 2.4)
pub struct AppSettingsIpcData { /* existing */ pub ui: UiSettings }          // from_snapshot: `ui: data.ui` after clear_extra()
pub async fn settings_load(service: State<'_, SettingsService>, ui: State<'_, UiSettingsService>)
    -> Result<AppSettingsIpcData, String>;   // snapshot.data.ui = ui.get() before from_snapshot (D4)
#[tauri::command]
pub async fn settings_set_library_view_mode(mode: LibraryViewMode, ui: State<'_, UiSettingsService>) -> Result<(), String>;
// lib.rs (owner 2.4): `.manage(services.ui_settings.clone())`; register `commands::settings::settings_set_library_view_mode`;
//   replace `.run(context).expect(..)` with `.build(context).expect(..)` + `.run(move |_, event| if let tauri::RunEvent::Exit = event {
//   if let Err(e) = tauri::async_runtime::block_on(ui_settings.flush()) { tracing::warn!(..) } })`.
```

### TypeScript: `web/`

```ts
// types/settings.ts (owner 2.5)
import type { LibraryViewMode } from './library';
export type UiDensity = 'compact' | 'comfortable' | 'spacious';
export type UiRenderer = 'auto' | 'wgpu' | 'gl' | 'tiny-skia';
export type ControllerMode = 'auto' | 'on' | 'off';
export interface UiSettings {
  library_view_mode?: LibraryViewMode;
  window_width?: number;
  window_height?: number;
  ui_scale: number;
  density: UiDensity;
  renderer: UiRenderer;
  controller_mode: ControllerMode;
  sidebar_collapsed: boolean;
  inspector_width: number;
  console_drawer_height: number;
  palette_recent: string[];
}
export const DEFAULT_UI_SETTINGS: UiSettings = {
  ui_scale: 1,
  density: 'comfortable',
  renderer: 'auto',
  controller_mode: 'auto',
  sidebar_collapsed: false,
  inspector_width: 320,
  console_drawer_height: 280,
  palette_recent: [],
};
// AppSettingsData gains `ui: UiSettings` (NOT SettingsSaveRequest); DEFAULT_APP_SETTINGS.ui = { ...DEFAULT_UI_SETTINGS };
// toSettingsSaveRequest is unchanged and must never emit `ui`.
// mocks/handlers/settings.ts: map.set('settings_set_library_view_mode', async (args) => { validate 'grid'|'list' else throw;
//   replace getStore().settings immutably with ui: { ...s.ui, library_view_mode: mode }; return null; })
// PreferencesContextValue (owner 2.6) gains: libraryViewMode: LibraryViewMode;  // settings.ui.library_view_mode ?? 'grid'
//                                           setLibraryViewMode: (mode: LibraryViewMode) => void;  // optimistic functional setSettings,
//   then void callCommand('settings_set_library_view_mode', { mode }).catch(console.error); never reverts, never reloads.
```

## Files to Change

| File                                                                                                                                                                                                                                                                                         | Action        | Task | Batch |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------- | ---- | ----- |
| `core/settings/ui.rs`, `ui/enums.rs`, `ui/keys.rs`                                                                                                                                                                                                                                           | create        | 1.1  | B1    |
| `core/settings/ui/lenient.rs`, `ui/palette.rs`                                                                                                                                                                                                                                               | create        | 1.2  | B1    |
| `core/settings/webkit_import.rs`                                                                                                                                                                                                                                                             | create        | 1.3  | B1    |
| `core/settings/mod.rs`, `core/settings/types.rs`                                                                                                                                                                                                                                             | modify        | 1.4  | B1    |
| `app/services/settings/ui.rs`, `app/services/settings/ui_writer.rs`                                                                                                                                                                                                                          | create        | 2.1  | B2    |
| `app/services/settings.rs`, `app/services/mod.rs`, `app/events.rs`, `app/lib.rs`, `app/app.rs`                                                                                                                                                                                               | modify        | 2.2  | B2    |
| `app/bootstrap/ui_import.rs`; `app/bootstrap/{stores.rs,app_bootstrap.rs,mod.rs}`, `bootstrap/stores/tests.rs`                                                                                                                                                                               | create/modify | 2.3  | B2    |
| `tauri/commands/settings.rs`, `tauri/lib.rs`                                                                                                                                                                                                                                                 | modify        | 2.4  | B2    |
| `web/types/settings.ts`, `web/lib/mocks/handlers/settings.ts`, `web/lib/mocks/store.ts`                                                                                                                                                                                                      | modify        | 2.5  | B2    |
| `web/context/PreferencesContext.tsx`, `web/components/pages/LibraryPage.tsx`                                                                                                                                                                                                                 | modify        | 2.6  | B2    |
| `core/settings/ui/tests.rs`                                                                                                                                                                                                                                                                  | create        | 3.1  | B3    |
| `core/settings/ui/lenient_tests.rs`                                                                                                                                                                                                                                                          | create        | 3.2  | B3    |
| `core/settings/webkit_import/tests.rs`                                                                                                                                                                                                                                                       | create        | 3.3  | B3    |
| `core/tests/unknown_settings_keys.rs`                                                                                                                                                                                                                                                        | modify        | 3.4  | B3    |
| `app/services/settings/ui_writer_tests.rs`                                                                                                                                                                                                                                                   | create        | 3.5  | B3    |
| `app/services/settings/ui_tests.rs`, `app/services/settings/tests.rs`                                                                                                                                                                                                                        | create/modify | 3.6  | B3    |
| `app/bootstrap/ui_import/tests.rs`                                                                                                                                                                                                                                                           | create        | 3.7  | B3    |
| `web/components/pages/__tests__/LibraryPage.test.tsx`, `web/types/__tests__/settings.test.ts`                                                                                                                                                                                                | modify        | 3.8  | B3    |
| `web/__tests__/a11y/{routes,skip-link}.a11y.test.tsx`, `web/components/__tests__/LaunchSubTabs.test.tsx`, `web/components/layout/__tests__/{AppShell,AppShell.startup}.test.tsx`, `web/components/pages/__tests__/DashboardRoutes.test.tsx`, `web/context/__tests__/ProfileContext.test.tsx` | modify        | 3.9  | B3    |
| `docs/features/settings-reference.doc.md` (create), `docs/getting-started/quickstart.md`                                                                                                                                                                                                     | create/modify | 4.1  | B4    |
| `src/crosshook-native/crates/crosshook-app/README.md`, `docs/research/native-ui-migration/README.md`                                                                                                                                                                                         | modify        | 4.2  | B4    |

No file appears twice within a batch. `core/settings/types.rs` (475 lines) and `core/settings/tests.rs` (632) are not grown beyond about 490: new tests go in new files. `LibraryPage.tsx` (485) shrinks.

## NOT Building

- No generic debounce utility or crate; the throttle is private to `ui_writer.rs` (D5).
- No `ui` in `SettingsSaveRequest` or `toSettingsSaveRequest`; no per-key Tauri commands except `settings_set_library_view_mode`.
- No ts-rs export for settings; TS types are hand-written.
- No `CHANGELOG.md` edit (git-cliff generates it).
- No toast in Classic for `UiSettingsPersistFailed`, no native toast consumer (M1-19), no early `[ui].renderer` read (M1-13), no runtime-only dismissals port, no cross-process lock (M1-12) (D6, D13).
- No localStorage write or deletion; no `BootstrapStep` for the import; no new Cargo dependency.
- No change to strict parsing of keys outside `[ui]`.

## Step-by-Step Tasks

### Task 1.1: Core UI model — consts, clamps, `UiSettings`, enums, keys — Depends on [B0]

- **BATCH**: B1
- **ACTION**: Create `core/settings/ui.rs` (D1's path) and the submodule files `ui/enums.rs`, `ui/keys.rs` exactly per B0 Contracts.
- **IMPLEMENT**: `ui.rs` declares `mod enums; mod keys; mod lenient; mod palette;` plus `#[cfg(test)] mod tests; #[cfg(test)] mod lenient_tests;` and re-exports (`pub use enums::*; pub use keys::{UiKey, UiKeySet}; pub use palette::is_valid_palette_id;`). Implement consts and clamp fns, `UiSettings` (derive `Clone, PartialEq, Serialize`), `Default`, hand-written `Debug` (known keys; `palette_recent` as `len()`; `extra` as key count), `library_view_mode_or_default`, `normalize` (clamp via the clamp fns, `window_*` clamp only when `Some`, `ui_scale` through `clamp_ui_scale`, then `palette::sanitize_palette_recent`; return whether anything changed), `push_palette_recent` (delegates to `palette::push_recent`), `copy_key_from` (one `match` over `UiKey`). `enums.rs`: four enums with the derives and serde renames in B0, `as_str`, `parse`. `keys.rs`: `UiKey`, `UiKeySet(u16)` bit set.
- **MIRROR**: `ConfigHistorySettings` + clamp fns (`types.rs:48-82`), `ReducedMotionPreference`.
- **GOTCHA**: Workspace clippy has `lossy_float_literal = deny` and `uninlined_format_args = warn`. Do not derive `Debug` or `Deserialize` on `UiSettings`. Add doc comments to every public item. Keep each file well under 500 lines.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` (after 1.2 and 1.4 land).

### Task 1.2: Lenient `Deserialize` and palette helpers — Depends on [B0]

- **BATCH**: B1
- **ACTION**: Create `core/settings/ui/lenient.rs` and `ui/palette.rs` per B0 Contracts.
- **IMPLEMENT**: `palette.rs`: `is_valid_palette_id` (hand-rolled scan of up to 3 `:`-separated segments with the first-char rules and the 98-byte cap), `sanitize_palette_recent`, `push_recent`. `lenient.rs`: `impl<'de> Deserialize<'de> for UiSettings` reading `toml::Value`, with small private helpers `take_enum<T>(table, key, parse) -> Option<T>`, `take_clamped_u32(table, key, min, max) -> Option<u32>` (read `Value::Integer(i64)`, `i64::clamp`, then `u32::try_from`), `take_bool`, `take_scale`, `take_palette`. Every fix-up emits one `tracing::warn!` with `key` and a fixed `reason`; the palette warns once with counts only. Remaining keys become `extra` (datetimes and nested tables included).
- **MIRROR**: `HighContrastPreference` custom `Deserialize` (`types.rs:173-199`).
- **GOTCHA**: `f64::clamp(NaN)` returns NaN, so check `is_finite()` first. Read `Value::Float` and `Value::Integer` for `ui_scale`. Never put a value in a log field. Never return `Err` (convert `toml::Value::deserialize` errors with `?` only; any TOML value deserializes). A present-but-invalid `library_view_mode` or `window_*` becomes `None`.
- **VALIDATE**: `cargo check -p crosshook-core`.

### Task 1.3: WebKit localStorage importer — Depends on [B0]

- **BATCH**: B1
- **ACTION**: Create `core/settings/webkit_import.rs` per B0 Contracts.
- **IMPLEMENT**: `webkit_local_storage_path` from `crate::platform::base_dirs()` and `crate::app_id_migration::CURRENT_TAURI_APP_ID_DIR` (no second literal, no `BaseDirs::new()`). `read_library_view_mode`: `fs::symlink_metadata` -> `is_file()` and `len() <= WEBKIT_MAX_FILE_BYTES`; build the URI with a private percent-encoder for `%`, `?`, `#`; open with `READ_ONLY | URI | NOFOLLOW | NO_MUTEX`; `pragma_update` `trusted_schema=OFF` and `query_only=ON`; check `SELECT type FROM sqlite_schema WHERE name = 'ItemTable'` is `table`; query as in B0; require even length and decode with `u16::from_le_bytes`; accept only `grid`/`list`. Declare `#[cfg(test)] mod tests;` at the bottom (file `webkit_import/tests.rs`). Doc comment cites `webkit2gtk-4.1` and the immutable-open trade-off (an un-checkpointed WAL is ignored).
- **MIRROR**: `core/metadata/db.rs:14-23`, `core/profile/lutris_import/paths.rs:89-95`.
- **GOTCHA**: Never log the path or a value; `debug!` with a reason code only. `scripts/check-startup-path.sh` rejects `BaseDirs::new()` outside `platform/dirs.rs`. `rusqlite` is core-only, so this must live in core.
- **VALIDATE**: `cargo check -p crosshook-core` and `./scripts/check-startup-path.sh`.

### Task 1.4: Wire `ui` into `AppSettingsData` — Depends on [B0]

- **BATCH**: B1
- **ACTION**: Edit `core/settings/mod.rs` and `core/settings/types.rs`.
- **IMPLEMENT**: `mod.rs`: `mod ui;`, `pub mod webkit_import;`, and re-export from `ui` every name in B0 (consts, clamp fns, `UiSettings`, the four enums, `UiKey`, `UiKeySet`, `is_valid_palette_id`). `types.rs`: the four edits listed under B0 (field before `extra`, `Default`, `clear_extra`, `preserve_extra_from`, `Debug`).
- **GOTCHA**: `Default for AppSettingsData` is an exhaustive literal, so the compiler flags a missing `ui`. The `Debug` impl is not compiler-checked: add the field by hand. `types.rs` must stay under about 495 lines.
- **VALIDATE (B1 gate, main agent)**: `cargo check -p crosshook-core`, `cargo clippy -p crosshook-core -- -D warnings`, no `cargo test` yet: the `#[cfg(test)]` modules declared in `ui.rs` and `webkit_import.rs` point at files that land in B3, and the `unknown_settings_keys` integration test is rewritten in 3.4.

### Task 2.1: `UiSettingsService` and the throttled writer — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Create `app/services/settings/ui_writer.rs` and `app/services/settings/ui.rs` per B0 Contracts.
- **IMPLEMENT**: `Shared { state: std::sync::Mutex<State { current: UiSettings, dirty: UiKeySet, failure_reported: bool }>, notify: tokio::sync::Notify, gate: tokio::sync::Mutex<()>, sink: UiSink, events: EventBus }`. `write_pending`: take the gate; under the state lock take `(current.clone(), dirty)` and clear `dirty`; return if empty; `await` the sink. On `Err`: `tracing::error!` (no raw error text beyond the `AppError` message), and if `!failure_reported` set it and `events.publish(AppEvent::UiSettingsPersistFailed { message })`. Task loop: `select! { cancelled => break, notified => {} }`, `write_pending`, `sleep(MIN_WRITE_INTERVAL)`. `mutate` clones the settings, applies, normalizes, compares, and only then marks dirty and calls `notify_one()`. `UiSettingsService::new` loads `store.load()` (`unwrap_or_default().ui`) and builds the production sink with `run_blocking` as in B0. Setters are thin wrappers (`set_window_size` clamps both and uses `UiKey::WindowSize`).
- **MIRROR**: `SettingsService::update` (`services/settings.rs:244-260`), `recent_files.rs:104-125` for move-to-front.
- **GOTCHA**: `#![deny(missing_docs)]` applies to the `pub` service and its methods. Do not hold the state mutex across an `.await`. Never save a cached `AppSettingsData`: the sink only copies dirty keys onto the on-disk `ui` inside `store.update`. `DropGuard` comes from `tokio_util::sync::CancellationToken::drop_guard`. The task must not capture an `App` clone.
- **VALIDATE**: `cargo check -p crosshook-app` (after 2.2 and 2.3).

### Task 2.2: App wiring — merge, normalize, exports, event, runtime handle — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Edit `app/services/settings.rs`, `app/services/mod.rs`, `app/events.rs`, `app/lib.rs`, `app/app.rs`.
- **IMPLEMENT**: Per B0: `mod ui; mod ui_writer; pub use ui::UiSettingsService;` and the two `#[cfg(test)]` module declarations in `settings.rs`; `ui: current.ui` in `merge_save_request`; `settings.ui.normalize();` in `normalize`; the new `AppEvent` variant (documented, appended last); the `pub use` additions; `App::runtime_handle`. Update the `services/mod.rs` header doc: sync typed getters and setters over memory are allowed; blocking store I/O stays async.
- **GOTCHA**: `AppEvent` derives `PartialEq, Eq, Clone, Debug`; `String` satisfies all. The Tauri `startup_bridge` has a `_ => {}` arm, so no change there. Do not add `ui` to `SettingsSaveRequest` (D4).
- **VALIDATE**: `cargo check -p crosshook-app`.

### Task 2.3: Bootstrap — import step and `Services` wiring — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Create `app/bootstrap/ui_import.rs`; edit `bootstrap/stores.rs`, `bootstrap/app_bootstrap.rs`, `bootstrap/mod.rs`, `bootstrap/stores/tests.rs`.
- **IMPLEMENT**: `ui_import.rs` per B0 (`import_step` composes `settings::webkit_import::import_library_view_mode` and `apply_import`; `apply_import` uses `store.update` and re-checks `is_none()` inside the closure; ends with `#[cfg(test)] mod tests;`). `stores.rs`: `ui_settings` field and `Services::new(stores, events, runtime)` building `UiSettingsService::new(stores.settings.clone(), events.clone(), runtime)`. `app_bootstrap.rs`: call `ui_import::import_step(&settings_store, &settings, settings_unreadable, &opts.store_roots)` after `relog_degraded_phase_a`; pass `app.runtime_handle()`. `stores/tests.rs`: make `services_new_builds_bundle_from_stores` a `#[tokio::test]` passing `tokio::runtime::Handle::current()`.
- **GOTCHA**: The import must precede `Services::new` so the new service loads the imported value. Do not add a `BootstrapStep` (the exact step-order assertion in `tests/bootstrap_report.rs:162-190` must keep passing). `StoreRoots::Explicit` runs never read the real WebKit file. The import runs synchronously during bootstrap; the FIFO/hang defence lives in the importer.
- **VALIDATE**: `cargo check -p crosshook-app`.

### Task 2.4: Tauri DTO, command, registration, exit flush — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Edit `tauri/commands/settings.rs` and `tauri/lib.rs`.
- **IMPLEMENT**: Per B0. In `settings_load`, set `snapshot.data.ui = ui.get()` before `from_snapshot`. In the in-file `mod tests`: add `let _ = settings_set_library_view_mode;` to `command_names_match_expected_ipc_contract`; add tests that (a) `[ui]\nlibrary_view_mode = "list"\nfuture = 1` serializes to JSON containing `"library_view_mode":"list"` and not `future`, (b) a default snapshot serializes the full default `ui` object without `library_view_mode`. In `lib.rs`, `let ui_settings = services.ui_settings.clone();` before the builder, `.manage(services.ui_settings.clone())`, the handler entry after `settings_save_steamgriddb_key`, and the `build(...).run(callback)` exit hook.
- **GOTCHA**: `Builder::build(context)` returns `tauri::Result<App>`; `App::run` takes `FnMut(&AppHandle, RunEvent)`. `block_on` runs on the main thread inside `RunEvent::Exit`, outside any async context. The command has a sync body but stays `async` (`unused_async` is allowed).
- **VALIDATE**: `cargo check -p crosshook-native`.

### Task 2.5: TS types, mock handler, mock store — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Edit `web/types/settings.ts`, `web/lib/mocks/handlers/settings.ts`, `web/lib/mocks/store.ts`.
- **IMPLEMENT**: Per B0. `store.ts`: seed with `structuredClone(DEFAULT_APP_SETTINGS)` so nested `ui` is never aliased. The `settings_set_library_view_mode` mock throws `'[dev-mock] invalid library view mode: …'` on a bad value and replaces `settings` immutably. Leave the `settings_save` mock unchanged: it already preserves `ui` because `toSettingsSaveRequest` omits it.
- **GOTCHA**: Mock naming must satisfy `scripts/check-mock-coverage.sh` (`map.set('settings_set_library_view_mode'`). Keep `ui` off `SettingsSaveRequest`.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck` (after 2.6).

### Task 2.6: Classic UI — `PreferencesContext` and `LibraryPage` — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Edit `web/context/PreferencesContext.tsx` and `web/components/pages/LibraryPage.tsx`.
- **IMPLEMENT**: Context: add `libraryViewMode` and `setLibraryViewMode` to the value and the `useMemo` deps, per B0 (functional `setSettings(prev => ({ ...prev, ui: { ...prev.ui, library_view_mode: mode } }))`; fire-and-forget command with a `.catch` that logs via `console.error` and keeps the value). `LibraryPage`: delete `VIEW_MODE_KEY`, `loadViewMode`, the `useState` and the `localStorage.setItem`; read `const { libraryViewMode: viewMode, setLibraryViewMode } = usePreferencesContext();` and pass `setLibraryViewMode` as `onViewModeChange` (drop `handleViewModeChange`). Remove now-unused imports.
- **GOTCHA**: No optional chaining on `settings.ui` in production code except the single `?? 'grid'` on `library_view_mode` (D8). While settings load, Library renders `grid` (accepted flash). Do not call `settings_load` after the write (it could race the throttled backend).
- **VALIDATE (B2 gate, main agent)**: `cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, `./scripts/check-app-deps.sh`, `./scripts/check-mock-coverage.sh`, `cd src/crosshook-native && npm run typecheck`.

### Task 3.1: Core tests — round-trip, clamp, palette, `Debug` — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Create `core/settings/ui/tests.rs`.
- **IMPLEMENT**: Table-driven: (1) round-trip of every key through `AppSettingsData` -> `toml::to_string_pretty` -> `toml::from_str`, including `tiny-skia` written literally, `None` options omitted, and byte-stable second save; (2) clamp table per numeric key at min-1, min, max, max+1 using the pub consts and clamp fns, plus `ui_scale` NaN, inf, -inf; (3) `palette_recent`: 25 entries with duplicates -> 20 unique in order, `push_palette_recent` move-to-front, invalid ids (`/etc/passwd`, `~/x`, `C:\x`, `Route:Library`, `has space`, 99-byte id, `a:b:c:d`) rejected, valid ids (`route:library`, `profile:launch-current`, `settings:ui:density`) accepted; (4) `Debug` of `AppSettingsData` with an API key, a top-level extra, a `[ui]` extra and palette ids contains none of those values, contains `<redacted>`, and shows `ui` (D10); (5) `copy_key_from` copies only the named key (`WindowSize` copies both fields).
- **MIRROR**: `core/settings/tests.rs:1-60` helpers (`tempdir`, `SettingsStore::with_base_path`).
- **VALIDATE**: `cargo test -p crosshook-core settings::ui`.

### Task 3.2: Core tests — lenient load and compatibility — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Create `core/settings/ui/lenient_tests.rs`.
- **IMPLEMENT**: Parse through `toml::from_str::<AppSettingsData>`: (1) never fails: `ui = 3`, `ui = "x"`, `renderer = "vulkan"`, `density = 4`, `controller_mode = true`, `ui_scale = "big"`, `window_width = -1`, `window_width = 9223372036854775807`, `inspector_width = 1.5`, `palette_recent = "x"`, `palette_recent = [1, "route:library"]` — each yields the documented default or clamp for that key only, and a sibling top-level key (`offline_mode = true`) still loads; (2) `ui_scale = 1` (integer) gives `1.0`; (3) unknown `[ui]` keys incl. a datetime and a nested table land in `ui.extra` and survive save; (4) `settings_backward_compat_without_ui`: a file with no `[ui]` loads defaults and `library_view_mode` is `None`; (5) a present-but-invalid `library_view_mode` becomes `None`; (6) keys outside `[ui]` stay strict (`high_contrast = "yes"` still `Err`); (7) warnings never contain values: install a scoped `tracing_subscriber::fmt` subscriber writing to a shared `Vec<u8>` (core already depends on `tracing-subscriber`), parse a file with `renderer = "SECRET-VALUE"` and `palette_recent = ["/home/secret/x"]`, and assert the captured output names the keys but contains neither value.
- **VALIDATE**: `cargo test -p crosshook-core settings::ui::lenient_tests`.

### Task 3.3: Core tests — WebKit import — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Create `core/settings/webkit_import/tests.rs`.
- **IMPLEMENT**: Fixture builder with `rusqlite::Connection::open` in a `tempdir`: `PRAGMA journal_mode=WAL`, `CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB NOT NULL ON CONFLICT FAIL)`, insert `("crosshook.library.viewMode", utf16le("list"))`, then `checkpoint` and close. Cases: `list` -> `Some(List)`; `grid` -> `Some(Grid)`; missing file -> `None`; corrupt bytes -> `None`; a symlink to a valid DB -> `None`; a FIFO (`nix::unistd::mkfifo`) -> `None` without hanging; a sparse file over 16 MiB (`set_len`) -> `None`; `ItemTable` as a VIEW -> `None`; wrong table name; key absent; odd-length blob; value `tiles`; UTF-8 `list` blob -> `None` (UTF-16LE only); a path containing `%`, `?`, `#`. After a successful read assert the directory listing and the file mtime are unchanged and no `-wal`/`-shm` exists. `webkit_local_storage_path()` ends with `dev.crosshook.CrossHook/localstorage/tauri_localhost_0.localstorage`.
- **GOTCHA**: Do not set `XDG_*` env vars in tests; test `read_library_view_mode(path)` only.
- **VALIDATE**: `cargo test -p crosshook-core settings::webkit_import`.

### Task 3.4: Core integration — rewrite `unknown_settings_keys.rs` (D11) — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Edit `core/tests/unknown_settings_keys.rs`.
- **IMPLEMENT**: Change the `SETTINGS` fixture `[ui]` to valid known values (`controller_mode = "auto"`, `ui_scale = 1.1`, `console_drawer_height = 220`, `palette_recent = ["launch"]`) and keep deliberately unknown keys `scale = 1.1`, `console_height = 220`, `future_ui_key = { nested = [1, 2] }`. Replace `saved["ui"] == original["ui"]` with: every unknown `[ui]` key is equal in `saved`, and every known key parses to the same value. Keep the byte-stability test. Extend the IPC-merge test: `incoming.ui.extra.insert("injected", …)` is dropped by `preserve_extra_from` and `current.ui.extra` is restored; `clear_extra` empties `ui.extra`. Add `bad_ui_values_do_not_make_settings_unreadable` (`renderer = "vulkan"`, `ui_scale = "big"`, plus `offline_mode = true` loads `Ok`, sibling kept, defaults for the bad keys). Leave the `[ui] scale=1.2` malformed-known-fields case (it still fails because of `config_history`).
- **VALIDATE**: `cargo test -p crosshook-core --test unknown_settings_keys`.

### Task 3.5: App tests — throttle, flush, failure latch — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Create `app/services/settings/ui_writer_tests.rs`.
- **IMPLEMENT**: All `#[tokio::test(start_paused = true)]`, using `UiWriter::spawn` with a counting sink (`Arc<AtomicUsize>` plus a `Mutex<Vec<(tokio::time::Instant, UiPatch)>>`). (1) **100 resize events**: 100 `mutate(UiKey::WindowSize, …)` calls with `tokio::time::advance(5ms)` between them, then `advance(2s)` and a few `yield_now()`s; assert `count <= 2`, the last patch holds the last size, and consecutive write instants differ by `>= MIN_WRITE_INTERVAL`. (2) **Leading write**: one change on an idle writer writes without waiting a second. (3) **No-op skip**: setting the current value writes nothing. (4) **flush** writes pending keys at once, returns `Ok`, and a second flush with nothing pending does not call the sink. (5) **Dirty-key isolation**: changing `Density` then `InspectorWidth` in separate windows produces patches whose `keys` contain only the changed key. (6) **Failure**: a sink that always errors; two failing writes publish exactly one `UiSettingsPersistFailed` on a subscribed `EventBus`, `snapshot()` still returns the new values, and `flush` returns `Err`. (7) Dropping the last `UiWriter` ends the task (no further sink calls after advancing time).
- **GOTCHA**: The sink is async and does not use `spawn_blocking`, so paused time stays deterministic. Count only sink calls; do not count file writes.
- **VALIDATE**: `cargo test -p crosshook-app settings::ui_writer`.

### Task 3.6: App tests — service, cross-UI contract, merge keeps `ui` — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Create `app/services/settings/ui_tests.rs`; edit `app/services/settings/tests.rs`.
- **IMPLEMENT**: `ui_tests.rs` (`#[tokio::test]`, tempdir store, `Handle::current()`): (1) each typed setter clamps (`set_ui_scale(9.0)` -> 2.0, `set_inspector_width(1)` -> 240, `set_window_size(1, 1)` -> 800x600) and `get()` shows it immediately; (2) **Classic -> native**: `set_library_view_mode(List)` + `flush()`, then a fresh `UiSettingsService` on the same store reports `Some(List)`; **native -> Classic**: `store.update(|s| s.ui.library_view_mode = Some(Grid))` then a fresh service and `SettingsService::load()` both report `Grid` (D9); (3) unreadable settings file (directory at `settings.toml`): `get()` returns defaults, setters still apply in memory, `flush()` returns `Err`; (4) a write only touches dirty keys: after `set_density` + flush, `ui.extra` unknown keys and top-level unknown keys on disk survive, and a concurrent `SettingsService::update` that changed `last_used_profile` is not reverted; (5) `SettingsService::update` normalizes an out-of-range `ui` set by the closure. `tests.rs`: change the `[ui] scale = 1.25` fixture in `merge_preserves_disk_extras_and_ignores_request_extras` to a known-plus-unknown `[ui]` (`ui_scale = 1.25`, `future_key = 1`) and assert `merged.ui.extra == current.ui.extra`; add `merge_keeps_ui_from_current` (a full form save with `make_save_request()` leaves `ui` untouched, including `library_view_mode`, `palette_recent` and `extra`) and `save_request_cannot_inject_ui_extra`.
- **GOTCHA**: `make_save_request()` needs no change because `SettingsSaveRequest` has no `ui`.
- **VALIDATE**: `cargo test -p crosshook-app settings`.

### Task 3.7: App tests — bootstrap import — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Create `app/bootstrap/ui_import/tests.rs`.
- **IMPLEMENT**: Against `apply_import` with a tempdir `SettingsStore`: `Some(List)` writes `library_view_mode = "list"` and returns `true`; `None` writes nothing (file absent or byte-identical) and returns `false`; if the key was set between the check and the update (call `apply_import` after setting `Some(Grid)`) the existing value wins; running twice writes once; an unreadable settings file leaves it untouched (`import_step` with `unreadable = true` skips) and `StoreRoots::Explicit` skips without touching `base_dirs`. Add one bootstrap-level check in the same file that `App::bootstrap` with explicit roots keeps `ui.library_view_mode` as `None` and the step list is unchanged.
- **VALIDATE**: `cargo test -p crosshook-app bootstrap::ui_import`.

### Task 3.8: Vitest — Library view mode from settings, toggle calls the command — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Edit `web/components/pages/__tests__/LibraryPage.test.tsx` and `web/types/__tests__/settings.test.ts`.
- **IMPLEMENT**: In `LibraryPage.test.tsx`: delete the `localStorage` stub in `beforeEach` and the `vi.unstubAllGlobals()` if nothing else needs it. Add (1) **list from settings**: `handlerOverrides.settings_load` returns `{ ...DEFAULT_APP_SETTINGS, ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'list' } }`; `await screen.findByRole('button', { name: 'List view' })` has `aria-pressed="true"` and list rows render (the simulated native-written value); (2) **toggle persists**: wrap the real handler through `seed`, click `List view`, assert `settings_set_library_view_mode` was called with `{ mode: 'list' }`, the mock store now holds `ui.library_view_mode === 'list'`, and `localStorage.getItem('crosshook.library.viewMode')` is `null`; (3) a rejected command keeps the list view (no revert). `settings.test.ts`: `'ui' in toSettingsSaveRequest({ ...DEFAULT_APP_SETTINGS, ui: { ...DEFAULT_UI_SETTINGS, library_view_mode: 'list' } })` is `false`, and `DEFAULT_APP_SETTINGS.ui` equals `DEFAULT_UI_SETTINGS`.
- **VALIDATE**: `cd src/crosshook-native && npx vitest run src/components/pages/__tests__/LibraryPage.test.tsx src/types/__tests__/settings.test.ts`.

### Task 3.9: Vitest — remove dead `localStorage` stubs, add `ui` to partial fixtures — Depends on [B2]

- **BATCH**: B3
- **ACTION**: Edit the seven files listed in Files to Change.
- **IMPLEMENT**: Remove each `vi.stubGlobal('localStorage', …)` block and its now-unused `memory` map (and the matching `unstubAllGlobals` only if it becomes unused) in `routes.a11y.test.tsx` (three blocks), `skip-link.a11y.test.tsx`, `LaunchSubTabs.test.tsx`, `AppShell.test.tsx`, `AppShell.startup.test.tsx`, `DashboardRoutes.test.tsx`. Add `ui: { ...DEFAULT_UI_SETTINGS }` (import from `@/types/settings`) to the partial `settings_load` fixtures in `routes.a11y.test.tsx` (`EMPTY_PROFILE_OVERRIDES`) and `ProfileContext.test.tsx` (`baseSettings`): `PreferencesProvider` now reads `settings.ui`, so a partial fixture without it throws.
- **GOTCHA**: Check each file for other `localStorage` users before deleting (grep shows none). If a test relied on the stub only for `LibraryPage`, the stub is dead code.
- **VALIDATE (B3 gate, main agent)**: `cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cd src/crosshook-native && npm run typecheck && npm test && npm run lint && npm run dev:browser:check && npm run test:smoke`.

### Task 4.1: User docs — settings reference and quickstart link — Depends on [B3]

- **BATCH**: B4
- **ACTION**: Create `docs/features/settings-reference.doc.md`; edit `docs/getting-started/quickstart.md`.
- **IMPLEMENT**: Follow the `docs/features/*.doc.md` layout. Sections: file location (use the `quickstart.md:106-110` wording: Flatpak live config `~/.var/app/dev.crosshook.CrossHook/config/crosshook/settings.toml`, legacy/AppImage `~/.config/crosshook/settings.toml`); editing rules (edit while CrossHook is closed; unknown keys are preserved; invalid `[ui]` values are clamped or reset with a warning in the log); a `## [ui]` table with columns Key | Type or values | Default | Range | Used by, covering all ten keys and `palette_recent` (ids only, at most 20, deduped); a full TOML example block; the one-time import from the old WebKit `localStorage` (Flatpak path `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/`; read-only; failures silently fall back to `grid`); the degraded behaviour (unwritable file keeps values for the session); the runtime-only list (toast and banner dismissals are not saved); known limitations (older builds drop `[ui]` on save; a newer renderer variant reads as `auto` on an older build; `window_*` is saved on resize end by the native shell). Add one line to `quickstart.md` under "Related Guides".
- **GOTCHA**: Do not edit `CHANGELOG.md`. Tables must be prettier-aligned (`proseWrap: preserve`, width 120).
- **VALIDATE**: `./scripts/format.sh --modified --docs` leaves no diff.

### Task 4.2: Crate README and migration README alignment (D11, D12) — Depends on [B3]

- **BATCH**: B4
- **ACTION**: Edit `src/crosshook-native/crates/crosshook-app/README.md` and `docs/research/native-ui-migration/README.md`.
- **IMPLEMENT**: App README: add a `UiSettingsService` row to the Services table (API: `get`, typed setters, `push_palette_recent`, `flush`; delegating Tauri command `settings_set_library_view_mode`), add `AppEvent::UiSettingsPersistFailed` where events are documented, and extend the Storage boundary bullet with the `[ui]` table and the throttled write. Migration README §10 (lines 544-548) and the "User visibility" paragraph (line 586): rename `scale` to `ui_scale` (default `1.0`, range 0.75 to 2.0), `density` default `comfortable`, `renderer` values `auto`/`wgpu`/`gl`/`tiny-skia`, and replace "Classic reads `localStorage` once and writes the key to TOML" with "Rust reads the WebKit `localStorage` file once, read-only, at bootstrap; Classic then reads and writes `[ui].library_view_mode`".
- **VALIDATE**: `./scripts/format.sh --modified --docs`.

## Testing Strategy

### Acceptance-criterion to test map

| AC                                                           | Test (task)                                                                                 |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| Round-trip and clamp for every key; palette ≤ 20 with dedupe | `ui/tests.rs` (3.1)                                                                         |
| Invalid values reset or clamp, load never fails              | `ui/lenient_tests.rs` (3.2), `unknown_settings_keys.rs` (3.4)                               |
| Import: fixture, missing, corrupt                            | `webkit_import/tests.rs` (3.3), `bootstrap/ui_import/tests.rs` (3.7)                        |
| 100 resize events produce ≤ 2 writes                         | `ui_writer_tests.rs` case 1 (3.5)                                                           |
| Classic and native see each other's view mode                | `settings/ui_tests.rs` case 2 (3.6), Vitest list-from-settings (3.8), Tauri DTO tests (2.4) |
| Docs settings reference updated                              | `docs/features/settings-reference.doc.md` (4.1)                                             |

### Other critical tests (data-integrity invariants)

- Lost-update guard: `merge_keeps_ui_from_current` and `save_request_cannot_inject_ui_extra` (3.6); dirty-key isolation and the concurrent `last_used_profile` write (3.5, 3.6).
- Unknown `[ui]` keys survive save and IPC merge (3.4); datetime inside `[ui]` (3.2).
- `Debug` redaction (3.1). No value, path or profile name appears in warnings (3.2 case 7, code review).
- Importer hardening: symlink, FIFO, oversize, view-as-table, no side files (3.3).
- Frozen bootstrap step order unchanged (`tests/bootstrap_report.rs` still passes).

### Edge cases checklist

- [x] NaN/inf/negative/`i64::MAX` numbers
- [x] Non-table `ui`, wrong types, unknown enum strings
- [x] Absent versus invalid `library_view_mode`
- [x] Unwritable settings file (memory still applies, one event)
- [x] Import with missing, corrupt, hostile files
- [x] Partial Vitest fixtures without `ui`
- [x] Network: not applicable (fully local)

## Validation Commands

Run from the repo root. The main agent runs these at each gate and in full at the end; subagents do not run them.

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
./scripts/lint.sh
./scripts/check-app-deps.sh
./scripts/check-startup-path.sh
./scripts/check-host-gateway.sh
./scripts/check-schema-version.sh
./scripts/check-mock-coverage.sh
```

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core settings
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test unknown_settings_keys
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app settings
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app --test bootstrap_report
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native commands::settings
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
```

```bash
cd src/crosshook-native && npm run typecheck && npm test && npm run lint && npm run dev:browser:check && npm run test:smoke
```

```bash
./scripts/format.sh --modified --docs   # before pushing, to avoid a lint-autofix bot commit on markdown
```

EXPECT: zero warnings; `check-app-deps.sh` clean (no new dependency); schema unchanged; every `#[tauri::command]` has a mock; `bootstrap_report` step-order test unchanged.

## Acceptance Criteria

- [ ] All 21 tasks complete across B1 to B4 with disjoint file ownership per batch.
- [ ] Round-trip and clamp tests pass for every key; `palette_recent` holds at most 20 and dedupes.
- [ ] The import tests cover a fixture DB, a missing file and a corrupt file.
- [ ] 100 resize events produce at most 2 writes under paused time.
- [ ] A write through the Classic command path is read by a fresh `UiSettingsService` and the reverse; Vitest renders the list view from `settings_load`.
- [ ] `docs/features/settings-reference.doc.md` exists and is linked from the quickstart.
- [ ] Classic has no `localStorage` use; no stray stubs remain; `settings_set_library_view_mode` is registered, mocked and name-pinned.
- [ ] No file added or grown past about 500 lines (`types.rs` and core `tests.rs` not over their current size plus the small wiring).
- [ ] All validation commands pass.

## Risks

| Risk                                                                                        | Likelihood | Impact | Mitigation                                                                                                         |
| ------------------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------ |
| Hand-written `Deserialize` plus top-level `flatten` mishandles TOML datetimes inside `[ui]` | Low        | Medium | Same buffering path as the existing top-level `extra` test; explicit datetime test in 3.2                          |
| Lost trailing write on exit with an owned runtime                                           | Medium     | Medium | Leading write plus `flush()` on `RunEvent::Exit` (2.4)                                                             |
| Paused-time tests flake if the sink uses `spawn_blocking`                                   | Medium     | Low    | Test sink is a pure async closure; only the production sink uses `run_blocking`                                    |
| Partial Vitest fixtures without `ui` crash `PreferencesProvider`                            | High       | Low    | Fixtures updated in 3.9; full `npm test` at the B3 gate                                                            |
| WebKit file name or layout differs on a real install                                        | Medium     | Low    | Derived from upstream sources; a miss is silent by design; recorded in the docs limitations                        |
| `Services::new` signature change breaks a caller outside bootstrap                          | Low        | Low    | Only `app_bootstrap.rs` and `stores/tests.rs` call it (verified by grep); `cargo check --workspace` at the B2 gate |
| Lint workflow does not run while the PR has merge conflicts; markdown autofix bot commits   | Medium     | Medium | Rebase onto `origin/main` before pushing; run `./scripts/format.sh --modified --docs`                              |
| Cross-process writes (Classic and native) unguarded until M1-12                             | Low        | Medium | Every write re-reads under the store lock and touches only dirty `[ui]` keys; documented follow-up (D13)           |

## Notes

- **Decisions D1–D13 binding.** Interpretations the plan makes (listed in `feature-spec.md`): `Services::new` takes a `Handle`; the import runs only for `StoreRoots::Xdg`; `UiSettingsService` getters and setters are sync; `ui.rs` (D1's file) keeps its submodules in `ui/` so parallel tasks own disjoint files and every file stays under the soft cap; the module path stays `settings::ui`.
- **PR**: one trunk-only PR, title `feat(app): add [ui] settings table for layout and display preferences`, body links `Closes #537`, includes the persistence classification, and lists the D13 follow-ups to file (native toast consumer M1-19, early renderer read M1-13, runtime-only dismissals, cross-process lock M1-12) plus the downgrade note for M6-03 (YAN-903).
- **Batch model**: B1 core, B2 app and Classic wiring in parallel, B3 all tests in parallel against real code, B4 docs. The main agent compiles once per gate.
