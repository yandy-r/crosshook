# Feature spec: `[ui]` settings table (YAN-869 / #537, M1-48)

Binding inputs: [`issue.md`](./issue.md), [`decisions.md`](./decisions.md) (D1–D13). Where research files disagree, `decisions.md` wins. Paths are relative to `src/crosshook-native/` unless they start with `docs/`. The batched plan is [`ui-settings.plan.md`](./ui-settings.plan.md); the frozen signatures are in its **B0 Contracts** section and are summarised here.

## Summary

Add a typed `[ui]` table to `settings.toml` (`UiSettings`, a field of `AppSettingsData`) that stores layout and display preferences for the future native UI and for Classic's Library view mode. Parsing of `[ui]` is lenient and never fails the file. Writes go through a new `UiSettingsService` in `crosshook-app` that keeps the values in memory, clamps them, and persists only the changed keys through `SettingsStore::update`, at most once per second. Classic stops using `localStorage`; the first start after the upgrade imports the old view mode once from WebKit's `localStorage` SQLite file, read-only and best-effort.

## Persistence classification and usability

| Datum                                              | Class                              | Notes                                                                           |
| -------------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------------------- |
| All `[ui]` keys (10 keys + `extra`)                | TOML settings (`settings.toml`)    | Hand-editable; additive; no schema change                                       |
| WebKit `localStorage` `crosshook.library.viewMode` | External, read once, never written | Opened `mode=ro&immutable=1`; a success is copied into `[ui].library_view_mode` |
| Pending throttled write, "persist failed" latch    | Runtime-only (memory)              | `UiSettingsService`; flushed on app exit                                        |
| Classic `sessionStorage` dismissals                | Unchanged (runtime-only)           | Native equivalent is memory-only and not built here (D13)                       |
| SQLite metadata                                    | No change                          | Schema version unchanged                                                        |

- **Migration/compat:** old files have no `[ui]` and load with defaults; builds with M1-11 (0.6.1+) keep `[ui]` verbatim in `extra`; builds before M1-11 drop it on save (documented downgrade row, not fixable here).
- **Offline:** fully local. **Degraded:** invalid values are clamped or reset with one `tracing::warn!` naming the key and a fixed reason (never the value); unwritable file keeps values for the session and publishes `AppEvent::UiSettingsPersistFailed` once.
- **Visibility/editing:** every key is documented in `docs/features/settings-reference.doc.md` and editable by hand.

## `[ui]` keys (D1–D3)

| Key                     | TOML type / values                               | Default       | Range / rule                                                                                 |
| ----------------------- | ------------------------------------------------ | ------------- | -------------------------------------------------------------------------------------------- |
| `library_view_mode`     | `"grid"` or `"list"`; `Option`, absent = not set | absent (grid) | invalid value resets to absent; absent triggers the one-time import                          |
| `window_width`          | integer, `Option<u32>`                           | absent        | clamp 800..=16384                                                                            |
| `window_height`         | integer, `Option<u32>`                           | absent        | clamp 600..=16384                                                                            |
| `ui_scale`              | float (integer accepted), `f64`                  | `1.0`         | clamp 0.75..=2.0; NaN/inf resets to 1.0                                                      |
| `density`               | `compact` / `comfortable` / `spacious`           | `comfortable` | enum                                                                                         |
| `renderer`              | `auto` / `wgpu` / `gl` / `tiny-skia`             | `auto`        | enum; `tiny-skia` is an explicit serde rename                                                |
| `controller_mode`       | `auto` / `on` / `off`                            | `auto`        | enum; a legacy boolean is reset, not mapped                                                  |
| `sidebar_collapsed`     | bool                                             | `false`       |                                                                                              |
| `inspector_width`       | integer                                          | `320`         | clamp 240..=640                                                                              |
| `console_drawer_height` | integer                                          | `280`         | clamp 120..=800 (separate from top-level `console_drawer_collapsed_default`)                 |
| `palette_recent`        | array of command ids, MRU first                  | `[]`          | ids match `^[a-z][a-z0-9-]{0,31}(:[a-z0-9][a-z0-9-]{0,31}){0,2}$`; dedupe keep first; cap 20 |
| any other key           | preserved in `UiSettings.extra`                  | -             | stripped from IPC (`clear_extra`), restored on save (`preserve_extra_from`)                  |

Integers are read as `i64` before clamping, so `-1` or `i64::MAX` never fail the load. A non-table `ui` resets the whole table. Keys outside `[ui]` keep strict behaviour.

## Architecture

```
settings.toml --toml--> AppSettingsData.ui (lenient Deserialize + normalize)        [core settings/ui.rs]
bootstrap: load_settings_step -> ui_import (only StoreRoots::Xdg, key absent, file readable)
        --reads--> webkit_import::read_library_view_mode (hardened, read-only)       [core settings/webkit_import.rs]
        --store.update--> [ui].library_view_mode
Services::new(.., Handle) -> UiSettingsService { UiWriter { current, dirty, throttle } }  [app services/settings/ui*.rs]
   setters: clamp -> memory -> mark dirty -> leading write, then 1 trailing write/s -> store.update(dirty keys only)
   failure: keep memory, log, AppEvent::UiSettingsPersistFailed (first failure only)
Classic: settings_load (ui from memory snapshot) / settings_set_library_view_mode / RunEvent::Exit -> flush
```

D4: `SettingsSaveRequest` is unchanged and `merge_save_request` sets `ui: current.ui`, so a full-snapshot Classic save can never overwrite `[ui]`. The only Classic `[ui]` write is the narrow command.

## Contracts (frozen in the plan's B0 section)

- **Core:** `UiSettings`, `LibraryViewMode`, `UiDensity`, `UiRenderer`, `ControllerMode`, `UiKey`, `UiKeySet`, bound consts, `clamp_*`, `is_valid_palette_id`; `UiSettings::{normalize, library_view_mode_or_default, push_palette_recent, copy_key_from}`; `webkit_import::{webkit_local_storage_path, read_library_view_mode, import_library_view_mode}`.
- **App:** `UiSettingsService::{new, get, set_*, push_palette_recent, flush}`, `Services.ui_settings`, `Services::new(&Stores, EventBus, Handle)`, `AppEvent::UiSettingsPersistFailed { message }`, `bootstrap::ui_import`.
- **Tauri:** `settings_set_library_view_mode(mode: LibraryViewMode)`; `AppSettingsIpcData.ui: UiSettings` (extra cleared); `RunEvent::Exit` flush.
- **TS:** `UiSettings`, `UiDensity`, `UiRenderer`, `ControllerMode`, `DEFAULT_UI_SETTINGS`, `AppSettingsData.ui`, `PreferencesContext.{libraryViewMode,setLibraryViewMode}`, mock `settings_set_library_view_mode`.

## Acceptance criteria to verification

| Acceptance criterion                                                          | Verified by (all new tests live in new files unless noted)                                                                                                                                                                                                                                           |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Round-trip and clamp tests for every key, `palette_recent` max 20 with dedupe | `core settings/ui/tests.rs`: per-key round-trip table, clamp table (min-1/min/max/max+1/NaN/inf/negative/`i64::MAX`), palette cap/dedupe/invalid-id table, byte-stable save, `Debug` redaction                                                                                                       |
| Invalid value is clamped or reset with a warning; load never fails            | `core settings/ui/lenient_tests.rs` lenient table (`renderer="vulkan"`, `controller_mode=true`, `ui=3`, wrong types); `core tests/unknown_settings_keys.rs` new test: bad `[ui]` loads `Ok` and other settings survive                                                                               |
| Unknown `[ui]` keys survive (M1-11)                                           | rewritten `unknown_settings_keys.rs` (D11) incl. IPC `clear_extra`/`preserve_extra_from` for `ui.extra`; app `services/settings/tests.rs` extras test                                                                                                                                                |
| Import test: fixture DB, missing file, corrupt file                           | `core settings/webkit_import/tests.rs`: UTF-16LE `list`/`grid` fixture (WAL mode, no `-shm`/`-wal` left), missing, corrupt, symlink, FIFO, oversize, view-as-table, bad value, odd length; `app bootstrap/ui_import/tests.rs`: import persists once, skipped when key present or settings unreadable |
| Debounce: 100 resize events produce at most 2 writes                          | `app services/settings/ui_writer_tests.rs` (`start_paused = true`): 100 `set_window_size` calls over 500 ms give `<= 2` sink calls, last value wins, writes at least 1 s apart, `flush` writes pending keys                                                                                          |
| In-memory values apply when unwritable; one error event                       | `ui_writer_tests.rs` failing sink: memory still returns new value, `UiSettingsPersistFailed` published exactly once for two failures                                                                                                                                                                 |
| Classic view mode visible in native and vice versa (D9)                       | `app services/settings/ui_tests.rs`: write via the Classic path (`set_library_view_mode` + `flush`), read by a fresh `UiSettingsService`, and the reverse; `merge_save_request` keeps `ui`; Vitest `LibraryPage` renders list from `settings_load`                                                   |
| Classic uses `[ui]`, not `localStorage`                                       | Vitest `LibraryPage.test.tsx`: toggle calls `settings_set_library_view_mode`, no `localStorage` write; `src-tauri` DTO + IPC-name tests; `./scripts/check-mock-coverage.sh`                                                                                                                          |
| Docs settings reference updated                                               | `docs/features/settings-reference.doc.md` created, linked from `quickstart.md`; app README row and migration README keys aligned (D11, D12)                                                                                                                                                          |
| Test plan commands                                                            | `cargo test -p crosshook-core settings`, `cargo test -p crosshook-app settings`, `npm run test:smoke`                                                                                                                                                                                                |

## Interpretations of `decisions.md` made by this plan

1. `Services::new` gains a `tokio::runtime::Handle` parameter (D5 needs a spawner; `App` already owns one). All callers (bootstrap, `stores/tests.rs`) are updated; no shim.
2. `UiSettingsService::get` and the setters are synchronous and non-blocking (memory only); only `flush` is `async`. `services/mod.rs`'s "every public function is async" doc is amended to say blocking store I/O is async.
3. A present but invalid `library_view_mode` resets to its default, absent (D2 "reset to default"); the import may then run again, which is harmless.
4. The bootstrap import runs only for `StoreRoots::Xdg`, so tests with explicit roots never read a developer's real WebKit file. It adds no `BootstrapStep`, so the frozen step-order tests do not change.
5. `AppEvent::UiSettingsPersistFailed` carries `message: String` (sanitized `AppError` text) so a future toast has copy; the Tauri startup bridge already ignores unknown variants.
6. The "failed once per session" latch (D6) never resets within a session. A failed write drops its dirty keys; the next real change re-arms the writer.
7. Import value decoding follows D7: UTF-16LE only, exactly `grid` or `list`.

## Risks

- **Lost trailing write on exit:** mitigated by `flush()` on `RunEvent::Exit` (Tauri) and by writing the leading edge immediately.
- **Cross-process writes** stay unguarded until M1-12 (D13); every write re-reads under the store lock and touches only dirty `[ui]` keys.
- **Downgrade clobber:** an older 0.7 reading a newer `renderer` variant resets it to `auto` in memory and the next save persists `auto`; recorded for M6-03 (YAN-903).
- **WebKit file layout** (`tauri_localhost_0.localstorage`) is derived from upstream sources, not from a real Classic profile (none exists on the dev machine); a miss is silent by design.
- **`flatten` + hand-written `Deserialize` + TOML datetimes** inside `[ui]`: covered by a test with an unknown datetime key.
- **Not built (D13):** native toast consumer (M1-19), early `[ui].renderer` read (M1-13), runtime-only dismissals in native, cross-process lock (M1-12).
