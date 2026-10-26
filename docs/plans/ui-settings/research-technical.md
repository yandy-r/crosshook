# Research — technical: settings flow end-to-end for `[ui]` (YAN-869 / #537, M1-48)

Paths are relative to `src/crosshook-native/` unless they start with `docs/` or `scripts/`. All line numbers are from the `crosshook-yan-869` worktree.

## 1. Current flow (disk → core → app → Tauri → React)

```
settings.toml ──toml::from_str──► AppSettingsData (core)            SettingsStore::load / update
     ▲                               │  unknown keys → `extra` (flatten)
     │ write_atomic(to_string_pretty) ▼
SettingsStore::save_unlocked ◄── SettingsService::update(closure) → normalize() (app)
                                     ▲ save_request(SettingsSaveRequest) → merge_save_request
Tauri settings_save(data) ───────────┘        settings_load → AppSettingsIpcData (extra cleared, key hidden)
React PreferencesContext.persistSettings → toSettingsSaveRequest(full snapshot) → settings_save → settings_load
```

### Core: `crates/crosshook-core/src/settings/`

- `types.rs:233-324`: `AppSettingsData` has `#[derive(Clone, PartialEq, Serialize, Deserialize)]` and `#[serde(default)]` on the struct. Unknown keys go into `#[serde(default, flatten)] pub extra: toml::Table` (`:320-323`).
- Nested table precedent: `ConfigHistorySettings` (`types.rs:48-69`) has `#[serde(default)]`, a field-level `default = fn`, its own `extra` flatten, and a manual `Default`. **Copy this for `UiSettings`.**
- Clamp precedent: `pub const *_MIN/_MAX` plus `pub fn clamp_*` (`types.rs:8-37`, `:70-82`), re-exported from `mod.rs:12-18`.
- Tri-state enum precedent: `HighContrastPreference`/`ReducedMotionPreference` (`types.rs:147-231`) use `rename_all = "snake_case"`, `as_str`, `FromStr`, and `#[default] Auto`. `controller_mode` (`auto/on/off`), `density` and `renderer` should follow this pattern.
- `Default for AppSettingsData` is at `types.rs:326-364`. It is an exhaustive literal, so the compiler forces a new `ui` field to be added here.
- `clear_extra` (`types.rs:368-377`) and `preserve_extra_from` (`types.rs:381-405`) are hand-maintained per nested table. **They must be extended to `ui.extra`**, or unknown `[ui]` sub-keys get stripped or injected through IPC.
- The manual `Debug` (`types.rs:408-470`) lists fields explicitly to redact `steamgriddb_api_key`. It already omits `config_history` and `extra`. **Add `.field("ui", &self.ui)`.** `UiSettings` can derive `Debug`, because it holds no secrets and `palette_recent` holds ids only.
- `store.rs:9-12`: `SettingsStore { base_path, io_lock: Arc<Mutex<()>> }`. The lock is process-local only; M1-12 adds the cross-binary lock as a SEAM at `crates/crosshook-app/src/bootstrap/pre_runtime.rs:141`.
  - `load_unlocked` (`:80-88`) is a plain `toml::from_str` with **no normalization on load**.
  - `save_unlocked` (`:90-96`) does `write_atomic(to_string_pretty)`.
  - `load` (`:98`) and `save` (`:104`) take the lock.
  - `migrate_or_save_settings` (`:112-131`) writes only on a byte diff.
  - `update(mutator)` (`:136-149`) does load, mutate and save under one lock, and writes only on `Ok`.
- `fs_util.rs:7-33` `write_atomic`: same-directory temp file, fsync, rename, then parent fsync. It follows symlinks and leaves the original untouched on a pre-rename failure.

### How unknown-key preservation works, and how a typed `[ui]` changes it

- Today `[ui]` is **unknown**, so it lands in `AppSettingsData.extra` and round-trips verbatim. This is pinned by `crates/crosshook-core/tests/unknown_settings_keys.rs:4-33` (fixture with a `[ui]` table) and `:35-67` (`saved["ui"] == original["ui"]`).
- After the change, `[ui]` is deserialized into `ui: UiSettings` and **no longer appears in the top-level `extra`**. Unknown `[ui]` sub-keys survive only if `UiSettings` has its own `#[serde(flatten)] extra: toml::Table`.
- **Fixture collisions you must handle:**
  - `unknown_settings_keys.rs:13` has `controller_mode = true`, a bool. With a strict serde enum, **the whole `settings.toml` load fails**. Through the bootstrap that means `settings_unreadable` and defaults (`crates/crosshook-app/src/bootstrap/app_bootstrap.rs:145-160`); Tauri `settings_load` returns an error.
  - `scale` and `console_height` (`:10`, `:17`) do not match the spec names (`ui_scale`, `console_drawer_height`), so they become `ui.extra`. That is preserved, which is fine.
  - The test at `:35-67` currently compares the whole `ui` table. After the change, save also emits every default key (`ui_scale`, `density`, …), so `saved["ui"] != original["ui"]`. **Rewrite it** to assert that the unknown `[ui]` sub-keys survive and that the known keys normalize.
  - `malformed_known_fields_fail_instead_of_becoming_extras` (`:132-147`) puts `[ui] scale=1.2` next to a broken `config_history`. It still fails, so it needs no change.
- **Policy conflict:** the repo rule is "malformed known fields fail the load" (that test, plus `high_contrast_invalid_string_fails_load`, `crates/crosshook-core/src/settings/tests.rs:137-157`). The spec instead says an invalid `[ui]` value is "clamped or reset to default with a logged warning". **Recommendation:** make `UiSettings` lenient through a custom `Deserialize`. Deserialize the table as `toml::Table`, parse each known key with a fallback to the default plus `tracing::warn!(key, …)`, and push the remaining keys into `extra`. This keeps one bad UI value (for example `window_width = -1`, which `u32` cannot hold, or `controller_mode = true`) from wiping every other setting. Keep the strict policy for all non-`[ui]` keys. `tracing` is already a core dependency (`crates/crosshook-core/Cargo.toml:34`).
- Clamping on load: there is no hook today, because `normalize()` runs only on the app save path (`crates/crosshook-app/src/services/settings.rs:177-187`). Clamp **inside** the `UiSettings` deserializer, or in a `UiSettings::sanitize()` that the deserializer calls. Then every reader gets clamped values: bootstrap (`app_bootstrap.rs:146`), `SettingsService::load`, Tauri `settings_load`, and M1-13 renderer selection. Also call `ui.sanitize()` from the app's `normalize()`, so setter input is clamped too.
- Value gotchas:
  - `f64::clamp(NaN)` returns NaN, and `ui_scale = nan` is valid TOML. Check `is_finite()` and fall back to the default.
  - A TOML integer such as `ui_scale = 1` must be accepted. Serde's `f64` visitor takes i64, but a hand-rolled `toml::Value` match must also handle `Integer`.
- `palette_recent`: keep the first occurrence, cap at 20, drop empty entries. To enforce "ids, never paths", reject entries containing `/`, `\` or whitespace (for example a charset of `[A-Za-z0-9._:-]` and a length cap), and warn when one is dropped. The recent-files dedupe pattern to mirror is `crates/crosshook-app/src/services/recent_files.rs:115-121` (`retain`, `insert(0)`, `truncate`).
- `library_view_mode` import trigger: the spec says "if absent". After `serde(default)` an absent key looks the same as `grid`, so presence must be kept:
  - Option 1: store `Option<LibraryViewMode>` with `skip_serializing_if = "Option::is_none"` and expose a getter that returns `unwrap_or(Grid)`.
  - Option 2: have the custom deserializer record presence in a `#[serde(skip)]` flag.
  - Option 1 is simpler and makes "imported once" visible on disk.

## 2. App layer: `crates/crosshook-app/src/services/settings.rs` (~297 lines)

- `SettingsSnapshot` `:22-33`. `SettingsSaveRequest` `:35-91` is hand-listed. Fields that may be absent use `#[serde(default)] Option<T>`, where absent means preserve. `config_history: Option<ConfigHistorySettings>` (`:89-90`) is the nested-table precedent.
- `merge_save_request` `:109-173` builds an **exhaustive** `AppSettingsData` literal (`:111-170`), so a new `ui` field is a compile error until it is added here. It then calls `preserve_extra_from(&disk)` (`:171`) and `normalize` (`:172`).
- `normalize` `:177-187` is private.
- `SettingsService { store, active_profiles_dir }` `:199-203`:
  - `new` `:208`, `load` `:216`
  - `update(FnOnce(&mut AppSettingsData))` `:244-260` runs `store.update`, then `normalize`, on `run_blocking`
  - `save_request` `:263`, `save_steamgriddb_key` `:275`
- `run_blocking` uses `spawn_blocking` (`crates/crosshook-app/src/services/mod.rs:35-41`). The module doc requires every public service function to be async (`mod.rs:1-5`).
- Construction: `Services::new(&stores, events)` (`crates/crosshook-app/src/bootstrap/stores.rs:63-72`) has the `EventBus`, but `SettingsService` does not take it today. `Services::new` is called from `App::bootstrap` at `app_bootstrap.rs:104`, where `app` (runtime and shutdown token) is in scope.
- Tests in `crates/crosshook-app/src/services/settings/tests.rs`: the `make_save_request()` fixture (`:15-49`) is an exhaustive literal, so a new request field must be added there. Other tests: merge and extras `:88-105` (already uses `[ui] scale`), tri-state tables `:119-191`, `#[tokio::test]` service tests `:344-417`. Put new UI and debounce tests in their own files under `services/settings/`.
- Error mapping: `SettingsStoreError` maps to fixed strings (`crates/crosshook-app/src/error.rs:149-165`). This is safe to surface as a toast.

### Recommended typed API (spec: "typed getters and setters")

Add these to `SettingsService` (or a `UiSettingsHandle` it owns):

- `ui()`: sync read of the in-memory `UiSettings`.
- `set_library_view_mode`, `set_window_size(w, h)`, `set_ui_scale`, `set_density`, `set_renderer`, `set_controller_mode`, `set_sidebar_collapsed`, `set_inspector_width`, `set_console_drawer_height`, `push_palette_recent(id)`.

Each setter clamps, updates memory immediately (the session keeps working even when the disk is not writable), and marks a debounced flush.

## 3. Runtime and debounce writer placement

- Runtime: `crosshook-app` depends on `tokio` with `rt-multi-thread, sync, time, macros, …`, plus `tokio-util` (`crates/crosshook-app/Cargo.toml:21-22`). Dev-dependencies include `tokio` `test-util` (`:31`), so `#[tokio::test(start_paused = true)]` works. Precedent: `crates/crosshook-app/src/log_tail/tests.rs:320`.
- `AppRuntime` either owns a runtime or borrows a handle (`crates/crosshook-app/src/runtime.rs:7-47`); Tauri passes its handle (`src-tauri/src/lib.rs:20`). `App::spawn` (`crates/crosshook-app/src/app.rs:~118-140`) cancels tasks through the `shutdown` token (`app.rs:15-28`); `JobRegistry::start` is another option (`crates/crosshook-app/src/jobs/registry.rs:134`).
- **There is no debounce or throttle utility today.** The nearest relatives:
  - `log_tail` per-window coalescing (`crates/crosshook-app/src/log_tail.rs:380-413` `TailConfig`, `:525-619` `drive`, which uses `tokio::select!{cancel, sleep}`), with an injectable poll for tests.
  - `tokio::time::timeout` in `services/background.rs:307`.
- **Recommended shape:** a new `crates/crosshook-app/src/services/settings/ui_writer.rs` containing:
  - **State:** `Arc<UiState { current: Mutex<UiSettings>, dirty: Notify (or watch::Sender<u64> generation), last_error_reported: AtomicBool }>`.
  - **Writer task, a throttle with a trailing edge:** loop { wait for dirty → `store.update(|s| s.ui = snapshot)` on `spawn_blocking` → `sleep(1s)` }. A burst of 100 resize events gives one leading write and one trailing write, which satisfies "≤2 writes" and "at most 1/s".
    - Write only `s.ui` through `store.update`. Never save a cached whole `AppSettingsData`, or concurrent `save_request` and `ProfileWriter` `last_used_profile` writes (`crates/crosshook-app/src/services/profiles/writer.rs:238`) would be clobbered.
    - **Test seam:** make the writer generic over a `FnMut(UiSettings) -> Result<()>` sink, or count writes through file mtime/bytes on a temp store. Drive it with paused time and assert `writes <= 2` after 100 `set_window_size` calls.
  - **Spawning:** `Services::new` would need a `Handle` or `&App`. Alternatives:
    - Pass `app` into `Services::new` (`app_bootstrap.rs:104`; also update `crates/crosshook-app/src/bootstrap/stores/tests.rs:119-127`).
    - Spawn lazily inside the first async setter with `tokio::spawn`. This needs an ambient runtime, which async callers have.
    - Prefer `app.spawn` so app shutdown cancels the task.
  - **Shutdown flush risk:** an owned runtime uses `shutdown_background`, which drops tasks without polling them again (`app.rs` doc comment at `:40-47`; `runtime.rs:39-46`). A pending trailing write would be **lost**. Expose `async fn flush_ui()` for shells to call on window close, and make the task flush on `cancel.cancelled()` when it is still polled (borrowed handle case).
  - **Write failure:** keep the values in memory and publish a one-shot event. `AppEvent` is `#[non_exhaustive]` (`crates/crosshook-app/src/events.rs:58-80`). Add something like `AppEvent::SettingsWriteFailed { message }` and gate it with `last_error_reported` so only a single toast fires; reset the gate on a successful write. The Tauri bridge already ignores unknown variants (`src-tauri/src/startup_bridge.rs:158`), so Classic is unaffected unless you wire it. This requires passing `EventBus` into `SettingsService::new` (`stores.rs:69-72`).

## 4. Tauri layer: `src-tauri/src/commands/settings.rs`

- `AppSettingsIpcData` `:17-67` is a hand-written DTO. `from_snapshot` `:69-122` calls `data.clear_extra()` and copies fields one by one. **Add `ui: UiSettings`** (with `ui.extra` cleared by `clear_extra`) so Classic can read `library_view_mode`.
- `settings_load` `:125-131`, `settings_save` `:135-147` (delegates to `save_request`), `settings_save_steamgriddb_key` `:150-160`. Registered at `src-tauri/src/lib.rs:133-135`; `SettingsService` is managed at `lib.rs:41`.
- Tests in the same file: the IPC name pin `:181-191` and DTO JSON shape tests. Add a test for `ui` serialization and `ui.extra` stripping.
- **Save path choice for Classic's view mode:**
  - (a) Add `ui: Option<UiSettings>` to `SettingsSaveRequest`. Absent means preserve, matching the `config_history` precedent.
    - Risk: `toSettingsSaveRequest` sends a **full React snapshot** (`src/types/settings.ts:85-127`; `src/context/PreferencesContext.tsx:103-111`). Every unrelated settings save would then rewrite the whole `[ui]` table from possibly stale React state.
    - The same applies to the load-then-save helpers: `src/hooks/useCommunityProfiles.ts:238-243`, `src/hooks/profile/profileMetadataSync.ts:11-45`.
  - (b) **Recommended:**
    - Leave `SettingsSaveRequest` without `ui`; `merge_save_request` sets `ui: current.ui` (preserve).
    - Add one narrow command, `settings_set_library_view_mode(mode)`, that calls `SettingsService::set_library_view_mode`. Use an immediate write, since toggles are rare and Classic expects to read its own write back.
    - Register it in `lib.rs`, add the mock handler (`scripts/check-mock-coverage.sh` fails CI on drift; mock handlers live in `src/lib/mocks/handlers/settings.ts:10-47`), and pin it in the command-name test.
- TS types are **hand-written**, not generated: `src/types/settings.ts`.
  - `SettingsSaveRequest` `:24-70`, `AppSettingsData extends SettingsSaveRequest` `:72-83`, `DEFAULT_APP_SETTINGS` `:130-166`.
  - ts-rs is an optional prototype feature (`crates/crosshook-core/Cargo.toml:10-11,54`; `crates/crosshook-core/src/ts_rs_exports.rs`), used only for onboarding, health and launch_hooks into `src/types/generated/`. Do not use ts-rs for settings; follow the existing hand-mirrored pattern.
  - Add `UiSettings` and the union types. Put `ui` on `AppSettingsData` only, not on `SettingsSaveRequest`.
  - The mock store seeds from `DEFAULT_APP_SETTINGS` (`src/lib/mocks/store.ts:1-8`).
  - Tests with partial `settings_load` mocks (`src/__tests__/a11y/routes.a11y.test.tsx:56-64`, `src/context/__tests__/ProfileContext.test.tsx:29`) return no `ui`. `LibraryPage` must default safely (`settings.ui?.library_view_mode ?? 'grid'`).
- Classic `LibraryPage`: `VIEW_MODE_KEY`/`loadViewMode` `src/components/pages/LibraryPage.tsx:30-35`, state `:82`, persist `:132-135`.
  - Replace these with `usePreferencesContext()` (`src/context/PreferencesContext.tsx:191`): read `settings.ui.library_view_mode`; write through the new command, then refresh settings or update optimistically.
  - Remove every `localStorage` use (clean cutover). The one-time import happens on the Rust side.

## 5. Best-effort WebKit localStorage import

- No existing code: a grep for `localstorage|LocalStorage|webkitgtk` in `crates/` and `src-tauri/src` finds nothing.
- `rusqlite` is a core dependency (`crates/crosshook-core/Cargo.toml:40`). Use `platform::is_flatpak()` (`crates/crosshook-core/src/platform/detect.rs:16`) and `base_dirs()` (`crates/crosshook-core/src/platform/dirs.rs:16`). App identifier: `dev.crosshook.CrossHook` (`src-tauri/tauri.conf.json:4`).
- Paths:
  - Flatpak (from the spec): `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/`.
  - Non-Flatpak: `$XDG_DATA_HOME/dev.crosshook.CrossHook/localstorage/` [INFERENCE: wry/WebKitGTK data dir; verify on a real install].
  - Origin file name, for example `tauri_localhost_0.localstorage` [INFERENCE]. Glob `*.localstorage` and look up the key.
- Schema: WebKit `ItemTable(key TEXT UNIQUE, value BLOB)`; values are typically UTF-16LE [INFERENCE: fixture must confirm]. Decode both UTF-16LE and UTF-8 defensively, and accept only `"grid"` or `"list"`.
- Open with `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI` and the `?immutable=1` URI, so nothing (`-shm`/`-wal`) is created. Trade-off: unflushed WAL content is ignored. Any error returns `None`, with only a debug or warn log.
- Placement: a core module (for example `settings/webkit_import.rs`) exposing a pure `read_library_view_mode(dir: &Path) -> Option<LibraryViewMode>`. This is testable against fixture, missing and corrupt DBs to meet the acceptance criterion.
- Invoke it once in `App::bootstrap` after `load_settings_step` (`app_bootstrap.rs:97-98`), so it is shared by both shells:
  - only when settings were readable;
  - only when `library_view_mode` is absent;
  - persist through `store.update` so the import never repeats.
  - Note that this adds a write during bootstrap. An alternative is to do it lazily in `SettingsService::load`.

## 6. Size, modularity and documentation

- `crates/crosshook-core/src/settings/types.rs` is about 470 lines, so adding `UiSettings` there would break the ~500-line soft cap. Create `settings/ui.rs` (types, consts, clamp, lenient deserialize) and re-export it from `mod.rs`. `settings/tests.rs` is already about 632 lines, so put the new tests in a separate `settings/ui_tests.rs`, or wire it as `#[cfg(test)] mod ui_tests;`.
- `crates/crosshook-app/src/services/settings.rs` is about 297 lines. Put the writer in `services/settings/ui_writer.rs`, with tests in a new file.
- Docs: **no user-facing settings reference exists** under `docs/`. The only settings mentions are the `docs/getting-started/quickstart.md:108,133` path notes. The acceptance criterion requires one, so create one (for example `docs/getting-started/settings-reference.md`) covering every `[ui]` key, its range and default, the localStorage import, and the runtime-only dismissals.
  - Also update the `SettingsService` row in `crates/crosshook-app/README.md:97` and the persistence notes at `:279-281`.
  - The native-migration plan already classifies `[ui]` keys (`docs/research/native-ui-migration/README.md:544-547,586`). Note that its key names (`scale`, `software`) differ from the spec (`ui_scale`, `tiny-skia`). The spec wins; flag the plan doc for a fix.

## 7. Risks (ranked)

1. **A strict serde `[ui]` hard-fails the whole settings load** on one bad UI value; the existing fixture's `controller_mode = true` already does this. Use the lenient deserializer and update `unknown_settings_keys.rs`.
2. **Lost trailing write on shutdown** with an owned runtime. Add an explicit `flush_ui()` and have the shells call it.
3. **Full-snapshot React saves clobbering `[ui]`** if `ui` is put on `SettingsSaveRequest`. Use preserve-only plus a narrow command.
4. `clear_extra`/`preserve_extra_from` not extended to `ui.extra` lets IPC inject or strip unknown `[ui]` sub-keys. Add the test next to `unknown_settings_keys.rs:69-117`.
5. The manual `Debug` impl will silently miss `ui` unless it is updated.
6. NaN `ui_scale`, negative or oversized integers, and int-for-float values all need explicit handling.
7. Renderer for M1-13 "bootstrap phase A": settings are loaded only in phase B (`app_bootstrap.rs:97-98`; `pre_runtime.rs` has no settings reads). M1-13 must read `[ui].renderer` with its own early `SettingsStore` load. The lenient, cheap `UiSettings` deserializer helps.
8. Cross-process writes, with Classic and native running concurrently, stay unguarded until M1-12 (`pre_runtime.rs:141` SEAM).

## Checks for the main agent

`cargo test -p crosshook-core settings`, `cargo test -p crosshook-core --test unknown_settings_keys`, `cargo test -p crosshook-app settings`, `cargo test -p crosshook-native` (src-tauri DTO tests), `./scripts/check-mock-coverage.sh`, `npm run test` / `npm run test:smoke`.
