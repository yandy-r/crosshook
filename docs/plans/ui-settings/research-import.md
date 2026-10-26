# Research: WebKit localStorage import of `crosshook.library.viewMode` (YAN-869 / M1-48)

Worktree: `/home/yandy/Projects/github.com/yandy-r/crosshook-yan-869`. Paths below are relative to `src/crosshook-native/` unless they are absolute or point into the cargo registry or WebKit upstream.

## TL;DR

- **Production Classic file:** `$XDG_DATA_HOME/dev.crosshook.CrossHook/localstorage/tauri_localhost_0.localstorage`.
- **SQLite schema:** `ItemTable(key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB NOT NULL ON CONFLICT FAIL)`.
- **Value encoding:** raw UTF-16 code units in native endianness. On x86_64 and aarch64 that means UTF-16LE.
- **Path resolution:** resolve the path from inside the process with `crate::platform::base_dirs().data_local_dir()` plus the existing `CURRENT_TAURI_APP_ID_DIR` constant. This picks the correct location in native, Flatpak per-app and Flatpak shared-XDG modes, because Tauri resolves the WebKit data dir through the same `$XDG_DATA_HOME`.
- **Opening the file:** open it with the `file:` URI `?mode=ro&immutable=1` and `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI | SQLITE_OPEN_NOFOLLOW | SQLITE_OPEN_NO_MUTEX`. Do a size pre-check first. If anything fails, return `None`.
- **Crate placement:** `rusqlite 0.40` (bundled) is already a dependency of `crosshook-core` only. Put the reader in `crosshook-core::settings` and call it from the `crosshook-app` settings service.

## 1. Where WebKit writes it (evidence chain)

| Step                           | Evidence                                                                                                                                                                                                                                                                |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tauri identifier               | `src-tauri/tauri.conf.json:4` `"identifier": "dev.crosshook.CrossHook"`; mirrored as `CURRENT_TAURI_APP_ID_DIR` in `crates/crosshook-core/src/app_id_migration.rs:55` (legacy `com.crosshook.native` at :53, already migrated at :175-190)                              |
| Tauri data dir default (Linux) | `~/.cargo/registry/.../tauri-2.11.2/src/manager/webview.rs:534-545`: when there is no `data_directory`, it uses `path().resolve(identifier, BaseDirectory::LocalData)`                                                                                                  |
| `LocalData`                    | `tauri-2.11.2/src/path/desktop.rs:86-87`: `dirs::data_local_dir()` (= `$XDG_DATA_HOME` or `~/.local/share`)                                                                                                                                                             |
| wry → WebKit                   | `wry-0.55.1/src/webkitgtk/web_context.rs:32-40`: `WebsiteDataManager::builder().base_data_directory(data_dir)`                                                                                                                                                          |
| WebKitGTK API                  | `Cargo.lock:5755-5782`: `webkit2gtk 2.0.2` / `webkit2gtk-sys` → `webkit2gtk-4.1`. In upstream `Source/cmake/OptionsGTK.cmake:196`, 4.1 builds with `ENABLE_2022_GLIB_API OFF`                                                                                           |
| Storage level                  | Upstream `UIProcess/WebsiteData/glib/WebsiteDataStoreGLib.cpp` `defaultUnifiedOriginStorageLevel()` returns `None` when the 2022 API is off. At `None`, `OriginStorageManager.cpp:524-526` uses the _custom_ localStorage path directly, with no hashed per-origin dirs |
| Dir name                       | `UIProcess/API/glib/WebKitWebsiteDataManager.cpp:242-243`: `localStorageDirectory = <base_data>/localstorage`. Same in `WebsiteDataStore.cpp:2631-2636` (`USE(GLIB)` → `"localstorage"`)                                                                                |
| File name                      | `NetworkProcess/storage/LocalStorageManager.cpp:59-66`: `<databaseIdentifier>.localstorage`. `WebCore/page/SecurityOriginData.cpp:108-121`: `protocol + '_' + encodeForFileName(host) + '_' + port.value_or(0)`                                                         |
| Origin (release)               | `tauri-2.11.2/src/manager/mod.rs:338-345`: on Linux the app URL is `tauri://localhost` (`http://tauri.localhost` is only used on Windows and Android) → **`tauri_localhost_0.localstorage`**                                                                            |
| Origin (`tauri dev`)           | `tauri.conf.json:8` `devUrl: http://localhost:1420`. `PROXY_DEV_SERVER = cfg!(all(dev, mobile))` (`manager/webview.rs:43`) is false on desktop, so the dev origin is `http_localhost_1420.localstorage`. This is dev-only state and should **not** be imported (see §6) |

### Concrete paths (process-internal `XDG_DATA_HOME` + `/dev.crosshook.CrossHook/localstorage/tauri_localhost_0.localstorage`)

| Mode                                         | `XDG_DATA_HOME` inside the process                                                  | Resulting file                                                                                                                              |
| -------------------------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Native/AppImage                              | `~/.local/share`                                                                    | `~/.local/share/dev.crosshook.CrossHook/localstorage/tauri_localhost_0.localstorage`                                                        |
| Flatpak default (ADR-0004 per-app isolation) | `~/.var/app/dev.crosshook.CrossHook/data` (Flatpak remap)                           | `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/tauri_localhost_0.localstorage` (this matches the issue text) |
| Flatpak `CROSSHOOK_FLATPAK_HOST_XDG=1`       | host data dir (`platform/xdg.rs:39-97`, applied in `pre_runtime` before any lookup) | the same as native                                                                                                                          |

**Observed on this workstation:**

- `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/` exists and holds `hsts-storage.sqlite`, `storage/salt`, `CacheStorage/`, `WebKitCache/` and `mediakeys/`. There is **no** `localstorage/` dir, because WebKit only creates the file on the first `setItem`.
- `~/.local/share/dev.crosshook.CrossHook` does not exist.
- So "missing file" is the common real-world case, and the code must treat it as silent.
- `storage/salt` is the NetworkStorageManager general path (`NetworkStorageManager.cpp:245-252`), not localStorage.

## 2. On-disk format

From upstream `NetworkProcess/storage/SQLiteStorageArea.cpp`:

- **Schema (:48-49):** `CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB NOT NULL ON CONFLICT FAIL)`. The pre-2.38 `LocalStorageDatabase` used the identical table, so one reader covers both.
- **Statements:** reads use `SELECT value FROM ItemTable WHERE key=?` (:61). Writes are `bindText(1,key)` + `bindBlob(2,value)` (:402).
- **Value encoding:**
  - `WebCore/platform/sql/SQLiteStatement.cpp:108-122`: `bindBlob(String)` upconverts to `char16_t` and stores the raw bytes. These are UTF-16 in host byte order, with no BOM and no NUL terminator.
  - `columnBlobAsString` (:264-277) reads them back as `char16_t`.
  - On both supported arches this is UTF-16LE: `"list"` is stored as `6C 00 69 00 73 00 74 00`.
- **Key:** stored as SQLite TEXT. WebKit binds it as text, so a plain `WHERE key = ?1` with a Rust `&str` matches.
- **Journal mode:**
  - `WebCore/platform/sql/SQLiteDatabase.cpp:191-193`: every non-read-only WebKit open runs `PRAGMA journal_mode=WAL`.
  - `SQLiteStorageArea.cpp:183-189`: localStorage opens with `ReadWriteCreate`.
  - `-wal`/`-shm` siblings therefore exist while a Classic webview is alive. WebKit groups writes in 500 ms transactions (`transactionDuration`, :46).
- **Corruption:** `SQLiteStorageArea.cpp:526-541`: WebKit deletes the file on `SQLITE_CORRUPT`/`NOTADB`. Our reader must never "repair" anything and only returns `None`.
- **Size limits:**
  - The quota is 5 MB per origin (`LocalStorageManager.cpp:40`, `localStorageQuotaInBytes = 5 * MB`), measured by `key.sizeInBytes() + value.sizeInBytes()` (`SQLiteStorageArea.cpp:601`).
  - SQLite page overhead plus UTF-16 values can make the file a few times larger.
  - Recommend a defensive pre-open cap of **16 MiB** on `metadata.len()`. Larger files are skipped and logged as a warning.
  - Also cap the value read with `length(value) <= 64` in SQL, because valid values are 8–10 bytes.

## 3. Repo precedents and rules

**Path lookup**

- Use `crate::platform::base_dirs()` (`crates/crosshook-core/src/platform/dirs.rs:15-18`, re-exported at `platform/mod.rs:21`). `BaseDirs::new()` is forbidden outside `dirs.rs`.
  - Enforced by `scripts/check-startup-path.sh:13-15,224-233`.
  - Also stated in CLAUDE.md/AGENTS.md (startup-path rule).
- The reader must run only after `pre_runtime`, because the XDG override must already be applied (`platform/xdg.rs:26-31`). The settings service load already runs well after bootstrap.

**Host gateway**

- No host tool runs and the file is on a path the sandbox can see. Every candidate is inside the sandbox's own `$XDG_DATA_HOME`, or under `$HOME`, which the manifest grants (`packaging/flatpak/dev.crosshook.CrossHook.yml:59` `--filesystem=home`).
- So the gateway does not apply, and neither do `host_fs` helpers (those are for `/run/host` system paths).

**Read-only rusqlite precedents**

- `crates/crosshook-core/src/metadata/db.rs:14-23` `raw_open`: `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NOFOLLOW | SQLITE_OPEN_NO_MUTEX`, plus `busy_timeout`.
  - :86 says "Ordinary READ_ONLY, not immutable: live WAL must remain visible". That choice is right for CrossHook's own live DB. See §4 for why the import differs.
- `crates/crosshook-core/src/profile/lutris_import/paths.rs:89-96` `read_pga_games`: a third-party SQLite read with `SQLITE_OPEN_READ_ONLY`, a bounded `LIMIT`, and errors mapped to a domain error. It is the closest analogue (importing another app's DB).

**Dependencies**

- `crates/crosshook-core/Cargo.toml:40` `rusqlite = { version = "0.40.0", features = ["bundled"] }`. Bundled SQLite supports `immutable=` and `NOFOLLOW` (`rusqlite-0.40.0/src/lib.rs:1204,1215,1250`).
- `crosshook-app`, `crosshook-cli` and `src-tauri` do **not** depend on rusqlite, so the reader belongs in core.
- `tempfile` is already a core dependency (`Cargo.toml:53,73`).

**Settings types**

- `AppSettingsData` has `#[serde(default)]` (`settings/types.rs:234`) and `extra: toml::Table` flatten (:321-323).
- `SettingsStore::update` (`settings/store.rs:136`) is the atomic load-mutate-save to use when persisting the imported value once.

**Source of the value**

- `src/components/pages/LibraryPage.tsx:30-35`: the key is `crosshook.library.viewMode`, and `loadViewMode` maps `'list'` → list and anything else → grid.
- :134: `setItem(VIEW_MODE_KEY, mode)` only ever writes `grid` or `list`.

## 4. Opening read-only and safely

Recommended open, which writes nothing, takes no locks and creates no `-shm`/`-wal` files:

```rust
let uri = format!("file:{}?mode=ro&immutable=1", percent_encode_path(path)); // escape '%', '?', '#'
let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
    | OpenFlags::SQLITE_OPEN_URI
    | OpenFlags::SQLITE_OPEN_NOFOLLOW
    | OpenFlags::SQLITE_OPEN_NO_MUTEX;
let conn = Connection::open_with_flags(&uri, flags).ok()?;
conn.query_row(
    "SELECT value FROM ItemTable WHERE key = ?1 AND length(value) <= 64",
    ["crosshook.library.viewMode"], |r| r.get::<_, Vec<u8>>(0)).optional().ok()??
```

**Why `immutable=1` and not plain READ_ONLY:**

- The spec says the import is "read once, never written".
- [INFERENCE from SQLite WAL docs] A plain `SQLITE_OPEN_READONLY` connection on a WAL-mode DB may still create or need the `-shm` file, and it takes shared locks. If Classic is running, those locks can contend with WebKit's 500 ms write transactions.
- `immutable=1` skips locking and WAL entirely.

**Tradeoff (document this in code):**

- If Classic is running _right now_ with an un-checkpointed WAL, the import can see a stale view mode. When WebKit closes cleanly it checkpoints and removes the `-wal`.
- This is acceptable because:
  1. the import runs only while `[ui].library_view_mode` is absent;
  2. after this issue, Classic itself writes `[ui]`, so the window closes permanently;
  3. the fallback is the default (`grid`) anyway.
- Do **not** copy the repo's db.rs "not immutable" comment here. That rule is about CrossHook's own live DB.

**Percent-encoding:** SQLite URI filenames need `%`, `?` and `#` escaped (`%25`/`%3F`/`%23`). XDG paths with those characters are rare but possible, so add a small private helper. No new crate is needed.

**Decoding the blob:**

- Require `len % 2 == 0`.
- Decode with `u16::from_le_bytes` per chunk, then `String::from_utf16`.
- Match exactly `"grid"` → `Grid` and `"list"` → `List`.
- Everything else, including odd length or invalid UTF-16, gives `None`, with a `tracing::debug!`/`warn!` and no error surfaced.
- Gate the LE assumption with `#[cfg(target_endian = "little")]`, or use `from_ne_bytes`. WebKit writes native-endian, so `from_ne_bytes` is the most faithful choice and needs no cfg.

**Failure mapping (all → `None`, no dialog):**

| Case                          | Handling                                                                     |
| ----------------------------- | ---------------------------------------------------------------------------- |
| Path missing                  | `fs::symlink_metadata` returns `NotFound`                                    |
| Symlink or not a regular file | reject before open; `NOFOLLOW` covers this too                               |
| Oversized                     | `len() > 16 MiB`                                                             |
| Not a DB / corrupt            | the open succeeds lazily, then `query_row` returns `SQLITE_NOTADB`/`CORRUPT` |
| Wrong schema                  | `no such table: ItemTable`                                                   |
| Key absent                    | `QueryReturnedNoRows`, mapped through `.optional()`                          |

## 5. Recommended shape

**New core module:** `crates/crosshook-core/src/settings/webkit_import.rs` (well under 500 lines), declared in `settings/mod.rs`.

- `pub(crate) const WEBKIT_LOCAL_STORAGE_FILE: &str = "tauri_localhost_0.localstorage";`
- `pub fn webkit_local_storage_path() -> Option<PathBuf>` returns `base_dirs()?.data_local_dir().join(CURRENT_TAURI_APP_ID_DIR).join("localstorage").join(FILE)`. Reuse `app_id_migration::CURRENT_TAURI_APP_ID_DIR` and do not hard-code a second copy.
- `pub fn read_library_view_mode(db: &Path) -> Option<LibraryViewMode>` is a pure function that takes a path, so tests can pass tempdir fixtures without env mutation.
- `pub fn import_library_view_mode() -> Option<LibraryViewMode>` composes the two functions above.

**Optional second candidate:** when running natively (`!platform::is_flatpak()`), it is reasonable to also try `home/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/...`. This covers a user who used the Flatpak Classic and now runs a native build. However:

- the per-app isolation in ADR-0004 implies the data domains are meant to stay separate;
- the issue only asks for the active environment's file.

Recommend: active-environment path only; mention the cross-install case in decisions.md.

**Caller (crosshook-app settings service, `crates/crosshook-app/src/services/settings.rs:216`):**

- When the loaded `[ui]` has no `library_view_mode`, call the import inside the existing `run_blocking`.
- If it yields `Some`, persist it once through `SettingsStore::update` so the import never re-runs. A failed write leaves the in-memory value in place, per the spec's degraded behaviour.

**Prerequisite for the types researcher:** "absent" detection requires the field to be `Option<LibraryViewMode>`, or the raw TOML must be inspected before defaults apply. A plain `#[serde(default)]` enum cannot tell absent from `grid`.

**Fixture tests (`settings/tests.rs`, or `settings/webkit_import/tests.rs` if the module becomes a dir):**

1. **Valid DB.** Create it with the exact WebKit DDL, `PRAGMA journal_mode=WAL`, and insert `("crosshook.library.viewMode", utf16le("list"))`. Expect `Some(List)`. Repeat with `grid` → `Some(Grid)`.
2. **Missing file** → `None`.
3. **Corrupt file** (`b"not a sqlite database"` written to the path) → `None`.
4. **Edge cases**, each → `None`:
   - odd-length blob;
   - unknown value `"tiles"`;
   - key absent;
   - wrong table;
   - symlink to a valid DB (NOFOLLOW);
   - file over the cap (use a sparse `set_len`).
5. **Writes nothing.** After the read, the directory listing is unchanged: no `-shm`/`-wal` created and the mtime is unchanged.

## 6. Risks

1. **WebKit API bump.**
   - If Tauri or wry moves to `webkitgtk-6.0` (`ENABLE_2022_GLIB_API ON`), the level becomes `Basic`. localStorage then migrates to `<base>/storage/<salted-hash>/<salted-hash>/LocalStorage/localstorage.sqlite3` (`OriginStorageManager.cpp:527-536`), and the path is not derivable without WebKit's salt-hash scheme.
   - Consequence: the import silently yields `None`, which is acceptable.
   - Pin the assumption in a doc comment that cites `webkit2gtk-4.1`.
2. **Dev-origin state.** `tauri dev` writes `http_localhost_1420.localstorage`. Not importing it is deliberate, because dev state must not leak into user settings.
3. **Stale read under a concurrent Classic** (see §4). This is bounded to the first run.
4. **Endianness.** Use `from_ne_bytes` to match WebKit's native-endian writes.
5. **Flatpak shared-XDG mode.** The path follows `XDG_DATA_HOME` automatically. Do not special-case `is_flatpak()` for the primary path.
6. **Lint guard.** Any `BaseDirs::new()` in the new module fails `scripts/check-startup-path.sh`, so use `platform::base_dirs()`.

## 7. Persistence classification (for the plan doc)

| Datum                    | Class                                                   | Notes                                                                                                                             |
| ------------------------ | ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| WebKit localStorage DB   | external, read-once, never written (immutable URI open) | —                                                                                                                                 |
| `[ui].library_view_mode` | TOML settings                                           | written once after a successful import                                                                                            |
| Import attempted/failed  | runtime-only                                            | no marker file. "Absent key" is the trigger, and a failed import just leaves the default in memory; the next save persists `grid` |
