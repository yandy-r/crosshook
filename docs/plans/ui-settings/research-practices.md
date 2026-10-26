# YAN-869 / #537 (M1-48): practices & reuse research

Paths below are relative to `src/crosshook-native/` unless they start with `docs/` or `scripts/`. Worktree: `crosshook-yan-869`.

## 1. Existing reusable code (use it, don't rebuild it)

| Need                                            | Reuse                                                                                                                                                                   | Where                                                                                                                                 |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Numeric clamp + bounds constants                | `pub const X_MIN/X_MAX` + `pub fn clamp_x(n) -> n.clamp(MIN,MAX)`                                                                                                       | `crates/crosshook-core/src/settings/types.rs:6-9,34-37` (`clamp_recent_files_limit`), `:69-75` (`clamp_config_history_max_revisions`) |
| Settings enum shape                             | `#[derive(Clone, Copy, Serialize, Deserialize, Default)] #[serde(rename_all="snake_case")]` + `#[default]` + `as_str()` (+ `FromStr`)                                   | `types.rs` `UmuPreference`, `ReducedMotionPreference` (~`:84-107`, `:198-230`)                                                        |
| Custom/lenient deserialize precedent            | `HighContrastPreference` hand-written `Deserialize` with an `#[serde(untagged)] Raw` enum                                                                               | `types.rs` (~`:173-191`)                                                                                                              |
| Nested table + unknown-key preservation (M1-11) | `#[serde(default)]` struct with `#[serde(default, flatten)] extra: toml::Table`. Wire it into `clear_extra` / `preserve_extra_from`                                     | `ConfigHistorySettings` `types.rs:49-57`; `AppSettingsData::clear_extra` `:368`, `preserve_extra_from` `:381`                         |
| Atomic, locked load-mutate-save                 | `SettingsStore::update(FnOnce(&mut AppSettingsData)->Result)` under `io_lock` + `fs_util::write_atomic`                                                                 | `crates/crosshook-core/src/settings/store.rs` (`update`, `save_unlocked`); `crates/crosshook-core/src/fs_util.rs` (`write_atomic`)    |
| Async service wrapper                           | `SettingsService::update(mutate)` runs `store.update` → `normalize()` on `run_blocking`                                                                                 | `crates/crosshook-app/src/services/settings.rs:244-262`, `normalize` `:177`; `run_blocking` `services/mod.rs`                         |
| Move-to-front + dedupe + cap list               | `retain(!= x); insert(0, x); truncate(cap)`                                                                                                                             | `crates/crosshook-app/src/services/recent_files.rs:104-125` (`RecentFilesService::push`)                                              |
| Read-only SQLite open                           | `Connection::open_with_flags(p, SQLITE_OPEN_READ_ONLY)`; hardened variant adds `NOFOLLOW \| NO_MUTEX`                                                                   | `crates/crosshook-core/src/profile/lutris_import/paths.rs:89-95`; `crates/crosshook-core/src/metadata/db.rs:14-21`                    |
| SQLite fixture builder in tests                 | `Connection::open(tmp/"pga.db")` + `execute_batch("CREATE TABLE …")`                                                                                                    | `crates/crosshook-core/src/profile/lutris_import/tests/paths_tests.rs:13-20`, missing-file case `:159-163`                            |
| App-id dir (WebKit data root)                   | `CURRENT_TAURI_APP_ID_DIR = "dev.crosshook.CrossHook"`                                                                                                                  | `crates/crosshook-core/src/app_id_migration.rs:55`; `platform::base_dirs()` `platform/dirs.rs:16`                                     |
| Paused-clock timer tests                        | `#[tokio::test(start_paused = true)]` + `tokio::time::Instant`                                                                                                          | `crates/crosshook-app/src/log_tail/tests.rs:320-356`; `jobs/process.rs:14`                                                            |
| Generic sink seam for testable loops            | `drive(state, poll, cfg, cancel, sink: impl FnMut(Batch))`                                                                                                              | `crates/crosshook-app/src/log_tail.rs:530-620`                                                                                        |
| Shutdown/cancel                                 | `App` shutdown `CancellationToken`, `App::spawn`                                                                                                                        | `crates/crosshook-app/src/app.rs:91-155`                                                                                              |
| Event bus for "one error toast"                 | `AppEvent` is `#[non_exhaustive]`; services get `EventBus` like `ProfileWriter`                                                                                         | `crates/crosshook-app/src/events.rs:58-78`; `bootstrap/stores.rs:66-90`                                                               |
| Tempdir test fixtures                           | `tempdir()` + `SettingsStore::with_base_path`; app helper `service_in(dir, active)`                                                                                     | `crates/crosshook-core/src/settings/tests.rs:5-12`; `crates/crosshook-app/src/services/settings/tests.rs:344-347`                     |
| Frontend debounce (Classic only, if needed)     | `useDebounce(ms)`                                                                                                                                                       | `src/hooks/useDebounce.ts:13`                                                                                                         |
| Frontend settings write path                    | `PreferencesContext.persistSettings`                                                                                                                                    | `src/context/PreferencesContext.tsx:103-110`                                                                                          |
| Defaults to mirror                              | window 1280×800 (`src-tauri/tauri.conf.json:16-17`); inspector 320 desk (`src/components/layout/inspectorVariants.ts:3-7`); drawer 280 (`src/styles/variables.css:142`) | —                                                                                                                                     |

There is **no Rust debounce or throttle utility** anywhere in `crosshook-app` or `crosshook-core`. The only "debounce" in core is the background portal's once-per-process gate (`platform/portals/background.rs:42`), which is unrelated. `tokio` with the `time` feature is already a dependency of crosshook-app (`crates/crosshook-app/Cargo.toml`), and `test-util` is a dev-dependency, so nothing new is needed.

## 2. Repo rules that constrain the layout

- CLAUDE.md:50 sets a soft cap of about 500 lines per file. Current sizes:
  - `settings/types.rs` is 475 lines.
  - Core `settings/tests.rs` is about 630 lines and already over the cap.
  - App `services/settings.rs` is about 300 lines, and its `settings/tests.rs` is 417.
  - `src-tauri/src/commands/settings.rs` is 358.
  - **New code must go in new files.** Do not append to `types.rs` or to core `tests.rs`.
- CLAUDE.md:51-52 requires DRY and single responsibility.
- CLAUDE.md:47-48 requires plans to be feature-complete and to carry a persistence classification. The issue already provides the Storage boundary.
- `scripts/check-mock-coverage.sh` fails CI if any `#[tauri::command]` lacks a matching `map.set('name'` in `src/lib/mocks/handlers/`. Every new command needs a mock. This is a reason to add **as few commands as possible**.

## 3. Recommended minimal module layout

```
crates/crosshook-core/src/settings/
  mod.rs          + mod ui; mod ui_import; pub use ui::{UiSettings, LibraryViewMode, UiDensity,
                    UiRenderer, ControllerMode, <bounds consts>}; pub use ui_import::import_library_view_mode
  types.rs        + `pub ui: UiSettings` (#[serde(default)]) in AppSettingsData, Default, manual Debug (:408!),
                    clear_extra/preserve_extra_from (+ ui.extra)
  ui.rs           NEW (~200 lines): UiSettings + 4 enums + bounds consts + `lenient` helper + `normalize()`
  ui_import.rs    NEW (~80 lines): WebKit localStorage read-only reader (rusqlite is core-only; app has no rusqlite)
  ui_tests.rs     NEW: round-trip/clamp/dedupe/lenient/import tests (sibling *_tests.rs like metadata/mod.rs:104-133)
  store.rs        load_unlocked: after toml::from_str, call `settings.ui.normalize()` ("validated and clamped on load")
crates/crosshook-app/src/services/
  settings.rs     + `ui: Arc<UiWriter>` field; ui()/update_ui()/set_library_view_mode()/flush_ui(); normalize() calls ui.normalize();
                    merge_save_request sets `ui: current.ui` (compile error forces it — good)
  settings/ui_writer.rs        NEW (~120 lines): throttled writer (leading + trailing, ≥1 s apart), generic over a sink
  settings/ui_writer/tests.rs  NEW: start_paused debounce tests
src-tauri/src/commands/settings.rs  + `ui: UiSettings` in AppSettingsIpcData (after clear_extra);
                                      + ONE command `settings_set_library_view_mode`
src/types/settings.ts  + UiSettings/LibraryViewMode… types, `ui` on AppSettingsData, DEFAULT_APP_SETTINGS.ui
src/lib/mocks/handlers/settings.ts  + map.set('settings_set_library_view_mode', …)
src/components/pages/LibraryPage.tsx  drop VIEW_MODE_KEY/localStorage (:30-35,:127-130); read settings.ui.library_view_mode
```

### Core design details

**Lenient field deserialization.** One generic helper covers about 11 fields, so the rule of three clearly applies:

```rust
fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(d: D) -> Result<T, D::Error> {
    let raw = toml::Value::deserialize(d)?;           // any TOML value succeeds
    Ok(raw.try_into().unwrap_or_else(|error| { tracing::warn!(%error, "invalid [ui] value; using default"); T::default() }))
}
```

Apply it with `#[serde(default, deserialize_with = "lenient")]`. Strict serde is a problem here. With strict serde, an unknown variant such as `library_view_mode = "tiles"` fails the **whole** `settings.toml` load. Bootstrap would then fall back to all defaults and set `settings_unreadable` (`bootstrap/app_bootstrap.rs:145-149`). That breaks the issue's "reset that key, warn" rule.

**`normalize(&mut self)`** handles ranges and lists:

- `u32` fields go through `clamp`.
- `ui_scale` is an **`f64`**, not an `f32`. TOML floats are f64, and an f32 round-trip writes `1.100000023841858`, which breaks the byte-stable test in `tests/unknown_settings_keys.rs:68-79`.
- Use `if !v.is_finite() { default }` **before** clamping. `f64::clamp(NaN)` returns NaN, and TOML accepts `nan`/`inf`.
- `palette_recent` is processed in order: drop invalid ids, dedupe keeping the first occurrence, then `truncate(20)`.
- Ids are restricted to `[A-Za-z0-9._:-]`, with no `/` or whitespace. This enforces "never paths".
- Log `tracing::warn!` whenever a value changes.

**Enums.**

- Copy the `UmuPreference` derive set and `as_str()`.
- `UiRenderer::TinySkia` needs `#[serde(rename = "tiny-skia")]`, because `snake_case` would produce `tiny_skia`.
- Skip `FromStr` unless a caller needs it. A repo-wide grep finds no non-test `Preference::from_str` callers. M1-13 might parse `ICED_BACKEND` for `UiRenderer`; only then add `FromStr` to that one enum.

**`library_view_mode: Option<LibraryViewMode>`** (serialized with `skip_serializing_if = "Option::is_none"`):

- With `#[serde(default)]`, "absent" and "grid" look the same, so the "import only if absent" rule needs `Option`.
- Expose a `library_view_mode()` getter that applies `unwrap_or_default()`.
- After the import attempt, write `Some(found_or_default)` even if the import failed. This keeps the import truly once and stops the file being re-probed on every start.

**`ui_import.rs`.**

- `fn import_library_view_mode(data_dir: &Path) -> Option<LibraryViewMode>`. It never returns an error.
- It reads `<data_dir>/dev.crosshook.CrossHook/localstorage/*.localstorage` using `CURRENT_TAURI_APP_ID_DIR`. Inside Flatpak, `base_dirs().data_dir()` already resolves to `~/.var/app/<id>/data`, so the issue's Flatpak path is covered.
- Open with `READ_ONLY | NOFOLLOW` and run `SELECT value FROM ItemTable WHERE key = ?1`.
- Values are BLOBs. Decode as UTF‑16LE and fall back to UTF‑8. [INFERENCE: this is from WebKit's `LocalStorageDatabase` (trac changesets 149458/276653); verify against a real CrossHook profile before writing the fixture.]
- Take `data_dir` as a parameter so tests stay hermetic and `StoreRoots::Explicit` works (`bootstrap/options.rs:16-25`).

### App design details

**Where the import runs.** Run it once from bootstrap, right after `load_settings_step` (`bootstrap/app_bootstrap.rs:97`), and only when `ui.library_view_mode.is_none()`. Bootstrap runs before any webview opens, so WebKit is not holding the database yet. Both UIs share this bootstrap: Tauri calls `App::bootstrap` in `src-tauri/src/lib.rs:28`.

**`UiWriter` (throttle).**

- State: `Mutex<{ current: UiSettings, pending: bool, last_write: Option<tokio::time::Instant>, flush_scheduled: bool, error_reported: bool }>`.
- A change updates `current`, so the new value applies to the session immediately.
- If at least 1 s has passed since the last write, it writes now (leading edge).
- Otherwise it sets `pending` and, if nothing is scheduled yet, `tokio::spawn`s a single `sleep_until(last+1s)` followed by a write of the latest snapshot (trailing edge).
- The write is `SettingsService::update(|s| { let extra = take(&mut s.ui.extra); s.ui = snap; s.ui.extra = extra; })`, which keeps disk-side unknown `[ui]` keys.
- On the first failure, publish once (for example `AppEvent::SettingsWriteFailed { message }`) and latch it. This needs `EventBus` in `SettingsService::new`, following the `ProfileWriter` precedent (`bootstrap/stores.rs:79-84`).
- `flush_ui()` is for shutdown, so the trailing write is not lost. Tauri has no exit hook today (no `RunEvent` in `src-tauri/src`). Classic never uses this path anyway (see below).

**Debounce test.**

- Make the writer generic over `sink: Arc<dyn Fn(UiSettings) -> Result<(), AppError> + Send + Sync>`, or an async equivalent.
- Under `start_paused = true`, send 100 events 5 ms apart (500 ms total) and assert `count <= 2`.
- A second test checks that the last value wins and that the gap between writes is at least 1 s.
- This is the same seam style as `log_tail::drive`. Counting writes through file inodes or mtimes would be flaky.

**Classic path.**

- `settings_set_library_view_mode(mode)` calls `SettingsService::update` **directly**, with no throttle. It is a rare click, and Classic has no exit flush.
- Do **not** add `ui` to `SettingsSaveRequest` or `toSettingsSaveRequest`. Those send full snapshots (`PreferencesContext.tsx:105-106`, `useCommunityProfiles.ts:240`, `profileMetadataSync.ts:13,42`), so a stale `ui` would clobber native writes.
- `merge_save_request` preserving `current.ui` is the safe default.

## 4. What NOT to build (KISS)

- **No generic debounce crate, trait, or `utils` module.** There is one consumer. Keep it private in `services/settings/ui_writer.rs`.
- **No file watcher or live cross-process sync.**
  - The flock instance lock (`bootstrap/pre_runtime.rs:141`, `SEAM(M1-12)`) means Classic and native never run concurrently.
  - The AC "visible in native and vice versa" is met by both UIs reading `settings.toml` at startup.
- **No per-key Tauri commands.**
  - Only `library_view_mode` has a Classic consumer.
  - Every other key is native-only. No `crosshook-ui` crate exists yet; the crates are core, cli and app.
  - Each extra command costs a mock (`check-mock-coverage.sh`).
- **No 11 hand-written typed setters.** Expose:
  - `ui() -> UiSettings` (getters live on the struct);
  - `update_ui(FnOnce(&mut UiSettings))`;
  - logic-bearing helpers only: `push_palette_recent(id)` and `set_library_view_mode`.
    `normalize()` clamps after any mutation, so per-field setters add nothing.
- **No `macro_rules!` for the enum boilerplate,** and no retrofitting of the 4 existing enums. That would be scope creep. Plain derives are fine.
- **No separate `ui.toml`, no SQLite rows, and no schema bump.** The issue's storage table says TOML only.
- **No ts-rs export.** The feature is an off-by-default prototype (`crosshook-core/Cargo.toml:10-11`). Settings TS types are hand-written in `src/types/settings.ts`.
- **No persistence for session dismissals.** "Runtime-only in native" means plain in-memory state in the future native UI. There is nothing to add to Rust now, and Classic's `sessionStorage` (`useProfileNotifications.ts:35-135`, `useFlatpakMigrationToast.ts:4-41`, `useMetadataStoreStatus.ts`) stays as is.
- **No write-back to localStorage, and no deletion of the old key.** It is read once and never written.

## 5. Testability checklist (maps to the acceptance criteria)

- **Core `settings/ui_tests.rs`:**
  - a table-driven round-trip for every key;
  - a clamp table with min-1, min, max, max+1, NaN and inf;
  - `palette_recent` with 25 entries plus duplicates, which must end as 20 unique entries in order;
  - path-like ids dropped;
  - a lenient test where `library_view_mode="tiles"` or `controller_mode=true` resets that one key while other settings load;
  - `[ui]` unknown keys survive save;
  - `tiny-skia` serializes literally.
- **Import:**
  - a fixture DB built with rusqlite: `CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE PRIMARY KEY, value BLOB NOT NULL ON CONFLICT FAIL)` plus a UTF‑16LE value;
  - a missing directory or file returns `None`;
  - a corrupt file (`fs::write(junk)`) returns `None`, because `prepare` fails with "file is not a database".
- **App:**
  - `ui_writer` paused-clock tests;
  - `merge_save_request` preserves `current.ui`;
  - `update` normalizes `ui`;
  - bootstrap import runs only when the key is absent and persists `Some`.
- **Frontend:**
  - `LibraryPage` test asserts the toggle calls `settings_set_library_view_mode` and makes no `localStorage` write;
  - update `src/types/__tests__/settings.test.ts` defaults.
- **Commands:** `cargo test -p crosshook-core settings`, `cargo test -p crosshook-app settings`, `npm run test:smoke`, `./scripts/check-mock-coverage.sh`.

## 6. Existing tests that WILL break (update them in the same PR)

- **`crates/crosshook-core/tests/unknown_settings_keys.rs:4-33,57-66`.**
  - The fixture uses `[ui]` as an _unknown_ table with keys `scale`, `controller_mode = true` and `console_height`. The test then asserts `saved["ui"] == original["ui"]`.
  - With a typed `UiSettings`, defaults get added and `controller_mode` resets, so that assertion fails.
  - Fix: move the unknown-table probe to a neutral name such as `[future_table]`, and add a `[ui]` future-key preservation assertion.
- **`crates/crosshook-app/src/services/settings/tests.rs:89-104`** uses `[ui] scale = 1.25` as "extras". It still passes, but the meaning is now wrong. Switch it to a non-`ui` table.
- **`unknown_settings_keys.rs:133-146`** (`malformed_known_fields_fail…`): the `[ui]\nscale=1.2` case still fails, but only because of `config_history`. Leave it, or make the intent explicit.

## 7. Risks and open points

1. **Lenient vs. strict policy.** M1-11's rule is "malformed known fields fail" (`unknown_settings_keys.rs:133`). `[ui]` intentionally deviates and drops invalid values. Document this in the `ui.rs` module docs and in the docs reference. A hand-edited bad value is lost on the next save, which matches the spec.
2. **Unspecified defaults.**
   - `density`: suggest `comfortable`.
   - Window size: 1280×800, from `tauri.conf.json`.
   - `inspector_width`: 320.
   - `console_drawer_height`: 280.
   - Naming conflict: the migration README (`docs/research/native-ui-migration/README.md:545-546`) uses `scale` and `software`, while the issue uses `ui_scale` and `tiny-skia`. Follow the issue and update the README row.
3. **WebKit storage layout.** Newer WebKitGTK may use a per-origin `storage/<hash>/LocalStorage/` tree instead of `localstorage/*.localstorage` [INFERENCE]. Probe a real Flatpak profile. If both layouts exist, check both; failures stay silent per the spec.
4. **Classic first-paint flicker.** `PreferencesContext` starts from `DEFAULT_APP_SETTINGS`, so the page renders `grid` and then switches to `list` after `settings_load`. Gate the view on the preferences having loaded, or accept the flicker.
5. **Manual `Debug` impl** (`types.rs:408+`). A new field is not caught by the compiler. Add `.field("ui", &self.ui)`.
6. **No settings reference exists in `docs/`.** Grep finds no user-facing `settings.toml` reference. To meet the AC, create one (for example `docs/features/settings-toml.doc.md`, matching the `*.doc.md` convention) covering `[ui]`. Also update the services table in `crates/crosshook-app/README.md:97`. The CHANGELOG is generated by git-cliff, so don't hand-edit it.
7. **The "visible in native" AC cannot be exercised end-to-end** because no native UI crate exists yet. Verify it at the `SettingsService` layer by writing through the Tauri command or service, then reading through a fresh `SettingsStore`.
