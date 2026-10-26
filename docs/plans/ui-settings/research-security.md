# M1-48 `[ui]` settings — security research

Worktree: `crosshook-yan-869`. All paths are relative to `src/crosshook-native/` unless stated otherwise.
Threat model: CrossHook is a single-user desktop app. `settings.toml` and the WebKit data directory both live in the user's own `$HOME`. Anyone who can write them already runs code as that user. Most items below are therefore **robustness and data-integrity** findings: hangs, bricked settings, lost updates and log or bundle leaks. They are not privilege-boundary breaks. Severity is rated on that basis. `[INFERENCE]` marks claims I did not check against source code or tests in this repo.

## 0. Existing controls (reuse them; don't add a second convention)

| Control | Location | Relevance |
|---|---|---|
| Atomic write: O_EXCL temp in the same directory, fsync, rename, then directory fsync | `crates/crosshook-core/src/fs_util.rs:25-117` | The debounced writer **must** use this through `SettingsStore`. |
| Symlinked target is canonicalized and followed; a dangling symlink is rejected | `fs_util.rs:45-60`, tests `:367-407` | This is intentional for dotfile managers. Keep it. |
| Process-wide `io_lock` shared by every clone of the managed store | `settings/store.rs:11,65,76`; `src-tauri/src/lib.rs:36,41` | Serializes in-process load-mutate-save. |
| `SettingsStore::update` = load → mutate → save under the lock; no write if load fails | `store.rs:136-150` | This is the only safe path for the debouncer. |
| Single normalize choke point (clamps) used by `update` and `merge_save_request` | `crates/crosshook-app/src/services/settings.rs:177-187,244-260,171-172` | Extend it with `settings.ui.normalize()`. |
| Hand-written `Debug` that redacts `steamgriddb_api_key` and omits `extra` | `settings/types.rs:408-470` (redaction `:418-425`) | Must be extended by hand for `ui`. |
| Fixed-string `AppError` for settings errors, so TOML/IO text never reaches the UI | `crates/crosshook-app/src/error.rs:149-165` | Reuse it for import and writer errors. |
| Unknown-key preservation (`extra`, `clear_extra`, `preserve_extra_from`) | `types.rs:323,368-405`; IPC strip at `src-tauri/src/commands/settings.rs:77` | `UiSettings` must join this machinery. |
| Hardened SQLite open: `READ_ONLY \| NOFOLLOW \| NO_MUTEX`, `query_only` | `crates/crosshook-core/src/metadata/db.rs:13-23,29-33` | Precedent for the localStorage read. |
| Weaker precedent: plain `READ_ONLY`, no NOFOLLOW, no immutable | `profile/lutris_import/paths.rs:91` | **Don't copy it.** Out of scope; note only. |

## 1. Reading the untrusted WebKit localStorage SQLite file

Today the Classic value comes from `src/components/pages/LibraryPage.tsx:30-35`, which reads it and accepts only `'list'`, else falls back to `grid`. It is written at `:132-135`.
`[INFERENCE]` WebKitGTK stores this value at `<XDG_DATA_HOME>/dev.crosshook.CrossHook/localstorage/<origin>.localstorage`, probably `tauri_localhost_0.localstorage`. It is a SQLite file holding `ItemTable(key TEXT PRIMARY KEY, value BLOB)` with values in UTF-16LE. **Verify this against a real Classic install before you hard-code the filename.** Use a real file as the test fixture.

### Threats and mitigations

| # | Threat | Sev. | Mitigation |
|---|---|---|---|
| 1.1 | **WAL/SHM side files created or modified on read.** A plain `SQLITE_OPEN_READ_ONLY` open of a WAL-mode database can create or attach `-shm`, take locks and run WAL recovery in WebKit's directory. That breaks the "read once, never written" rule in the issue. | Med | Open with a URI: `file:<abs>?mode=ro&immutable=1` plus flags `SQLITE_OPEN_READ_ONLY \| SQLITE_OPEN_URI \| SQLITE_OPEN_NOFOLLOW \| SQLITE_OPEN_NO_MUTEX`. `immutable=1` turns off locking and change detection and never creates `-wal` or `-shm`. Trade-off: frames not yet checkpointed into the WAL are ignored. That is acceptable for a best-effort import, because a clean Classic exit checkpoints the WAL `[INFERENCE]`. Percent-encode the path for the URI. Rusqlite has no URI builder, so escape `%`, `?` and `#`. |
| 1.2 | **Symlink at the db path**, for example `…/x.localstorage -> /dev/zero` or another user's file. | Low | First call `fs::symlink_metadata`: reject symlinks and require `file_type().is_file()`. Also set `SQLITE_OPEN_NOFOLLOW` (precedent `db.rs:18`). Don't canonicalize. |
| 1.3 | **FIFO, socket or device at the path.** `open(2)` on a FIFO blocks until a writer appears, so startup hangs. | Med (hang) | Use the `is_file()` check from 1.2. Run the import on `spawn_blocking`, **off the bootstrap critical path**, so a hang can only stall the import. Residual TOCTOU: someone could swap the file between `lstat` and the SQLite open. Only the same user can do that. See hardening option B for a TOCTOU-free variant. |
| 1.4 | **Huge file.** | Low | Cap with `metadata.len() <= 32 MiB` (WebKit's quota is about 5 MB per origin, and UTF-16 doubles it). With `immutable` and a primary-key lookup, SQLite reads pages lazily, so a large file does not mean large memory use. The cap is a cheap sanity guard. |
| 1.5 | **Malformed or crafted DB**: NOTADB or CORRUPT pages, `ItemTable` defined as a **VIEW** with a recursive CTE (infinite loop), or triggers or functions in the schema. | Med (hang) | (a) `PRAGMA trusted_schema=OFF` and `PRAGMA query_only=ON`. Optionally add `PRAGMA cell_size_check=ON`. (b) Before querying, require `SELECT type FROM sqlite_schema WHERE name='ItemTable'` to return `table`. Reject views, because a view is the only route to unbounded execution in a primary-key lookup. (c) Query with a bounded, parameterized statement: `SELECT value FROM ItemTable WHERE key = ?1 AND length(value) <= 16 LIMIT 1`. (d) Map any `rusqlite::Error` to `None` and fall back to the default. Show no dialog (required by the issue). |
| 1.6 | **Hostile value**: very long, invalid encoding or an unexpected string. | Low | Accept the value only if its bytes equal exactly one of `b"l\0i\0s\0t\0"`, `b"g\0r\0i\0d\0"` (UTF-16LE), `b"list"` or `b"grid"`. Anything else yields `None`. Never decode into a free-form `String` that is stored or logged. |
| 1.7 | **Path built from `$HOME`/XDG**: relative or `..` components, or an empty `HOME`. | Low (env is already trusted) | Build candidates **only** from `crate::platform::base_dirs()` (`platform/dirs.rs:16`) plus compiled-in segments: `dev.crosshook.CrossHook/localstorage/<fixed filename>`. Use `app_id_migration::CURRENT_TAURI_APP_ID_DIR` (`app_id_migration.rs:52`) instead of a new literal. Require each root to be `is_absolute()` and to contain no `Component::ParentDir`, the same rule as `fs_util.rs:157-160`. **Never** call `expand_path_with_tilde` or `resolve_user_home` (`settings/paths.rs:26-54,63-95`). They spawn `getent` through the host gateway. Never list the directory and open every entry. |
| 1.8 | **Flatpak path ambiguity**: in host-XDG opt-in mode, `XDG_DATA_HOME` is redirected to the host (`platform/xdg.rs:72-87`), so `base_dirs().data_dir()` may not be the sandbox data directory. | Low (correctness) | Use a fixed, deduplicated candidate list: `data_dir()/<id>/localstorage/…` first, then `home_dir()/.var/app/dev.crosshook.CrossHook/data/<id>/localstorage/…`. The first regular file that passes 1.2-1.4 wins. |
| 1.9 | **Re-reading the untrusted file on every launch**: if the import writes nothing, `library_view_mode` stays absent. | Low | After any attempt (success or fallback), write the resolved value with `SettingsService::update`. That makes the file read exactly once, as the spec requires. You need an `Option<LibraryViewMode>` on disk, or a raw-TOML presence check, to tell "absent" from "grid". `#[serde(default)]` alone loses that distinction. **Skip the import entirely when settings fail to load** (see 3.3). |
| 1.10 | **Leaks in logs or diagnostics**: absolute paths and raw values in `warn!`. | Low | Log at `debug!` and include only a fixed reason enum, such as `missing`, `not_regular`, `too_large`, `open_failed`, `schema` or `no_value`. Include no path or value. This matches the fixed-string policy in `error.rs:152-153`. |

**Hardening option B (only if reviewers want no TOCTOU):** open with `nix::fcntl::open(path, O_RDONLY|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC)`. `nix` already has the `fs` feature (`crosshook-core/Cargo.toml:52`). Then `fstat` the descriptor for regular-file status and size, read up to the cap, copy into a `tempfile::tempdir()` (0700), and open the copy with `immutable=1`. That removes 1.2, 1.3 and 1.4 races and keeps every side effect in a private directory. It costs about 30 more lines and a copy. Option A (lstat plus NOFOLLOW plus immutable) is enough for the same-user threat model.

Put this in its own module, e.g. `crates/crosshook-core/src/settings/webkit_import.rs`, exposing `pub fn read_classic_library_view_mode(candidates: &[PathBuf]) -> Option<LibraryViewMode>`. Inject the candidates so tests can cover these fixtures: valid DB, missing file, corrupt bytes, symlink, FIFO (`nix::unistd::mkfifo`), oversize sparse file (`set_len`), `ItemTable` as a view, wrong encoding, and a WAL-mode fixture. The WAL test should assert that no `-shm` or `-wal` appears afterwards.

## 2. `palette_recent` — command ids only, never paths or secrets

Today's Classic ids (`src/lib/commands.ts:3,121,138,148`) are `route:<route>`, `profile:launch-current:<profileName>` and `profile:edit-current:<profileName>`. **The profile ids embed user data.** Profile names can't contain `/ \ :` (`profile/toml_store/utils.rs:57-95`), so they can't be paths. They are still user-chosen strings, and they would persist in `settings.toml`. That file is copied verbatim into the diagnostic bundle with only `$HOME` redaction (`export/diagnostics/mod.rs:197-198`, `profiles.rs:35-46`). They would also go stale after a rename.

**Proposed grammar** (ASCII, closed namespaces, no user data):

```
command-id = namespace 1*2( ":" segment )
namespace  = "route" / "profile" / "settings" / "app"      ; closed allow-list
segment    = %x61-7A *31( %x61-7A / %x30-39 / "-" )         ; [a-z][a-z0-9-]{0,31}
total length <= 64 bytes
```

- Regex: `^(route|profile|settings|app)(:[a-z][a-z0-9-]{0,31}){1,2}$`, with `len() <= 64`. It accepts `route:library`, `route:host-tools`, `route:proton-manager` and `profile:launch-current`.
- The alphabet excludes `/ \ ~ . %`, whitespace, `=`, uppercase and non-ASCII. Path-like and URL-like strings can't match, and base64 or uppercase tokens can't either. A lowercase hex key can't match unless it sits behind an allow-listed namespace and is ≤32 characters per segment. On top of that, writes only come from the registry, as described next.
- **Profile commands are recorded without the name** (`profile:launch-current`). Those commands always act on the *current* profile anyway (`commands.ts:128-155`).
- Use a `CommandId` newtype with `TryFrom<&str>`. The setter `record_palette_command(id: CommandId)` should accept only ids produced by the native command registry (M4-10), never free text from a search box.
- On load: drop entries that fail the grammar, using `warn!` with a **count only**. Keep entries that pass the grammar but are unknown to this build, for forward compatibility. The UI hides unknown ids at render time.
- Normalize the list with `retain(valid)`, then dedupe keeping the first, most-recent occurrence (`HashSet<&str>`), then `truncate(20)`. Run this in `UiSettings::normalize()` on load *and* in the setter (MRU insert means remove-then-insert at index 0, then truncate). Don't trim or lowercase invalid ids to make them fit; reject them. That keeps the round-trip exact.

## 3. User-editable TOML values (clamp, don't brick)

| # | Threat | Mitigation |
|---|---|---|
| 3.1 | `ui_scale = nan`, `inf` or `-inf` are valid TOML floats. **`f64::clamp(NaN)` returns NaN**, so a naive clamp passes it through. `serde_json` then sends NaN to the UI as `null`, which breaks TypeScript `number` types and scale maths. | `if !v.is_finite() { 1.0 } else { v.clamp(0.75, 2.0) }`. Integer input such as `ui_scale = 1` should deserialize, because serde's f64 visitor accepts integers. Add a test. |
| 3.2 | Negative or huge ints, e.g. `window_width = -1` or `= 9223372036854775807`. Deserializing straight into `u32` **fails the whole settings load**. | Deserialize the dimension fields as `i64`, then `clamp` and cast: width 800..=16384, height 600..=16384, inspector 240..=640, console 120..=800. Keep the stored type `u32`. Clamp with `i64::clamp` before `as u32`, never `as` first. |
| 3.3 | **One bad `[ui]` value takes down all settings.** Today any type or enum error makes `store.load()` return `TomlDe` (`store.rs:86-87`). Tests assert that for the existing enums (`settings/tests.rs:193-215`). The issue instead says "clamped or reset to default with a logged warning". Worse, `bootstrap/state.rs:99-102` falls back to `Default` in memory, so any writer that saved *that* snapshot would wipe the user's file, including `steamgriddb_api_key`. | Make `[ui]` **lenient by construction**: put `#[serde(default, deserialize_with = "lenient")]` on every `UiSettings` field, where `lenient` deserializes `toml::Value` → `T` and falls back to `T::default()` with `warn!(field = "ui.density", "invalid value reset")`. Log the field name, not the value. Also make `AppSettingsData.ui` itself lenient, so `ui = 5` or `[ui]` as an array resets the table instead of failing the load. Writers must **only** use `SettingsStore::update`, never `save(&snapshot)`, so a failed load aborts the write (`store.rs:136-150`). Leave the existing top-level enums strict; that is a separate decision. |
| 3.4 | Unknown enum strings, such as `density = "dense"` or `renderer = "vulkan"`. | Lenient fallback to the default, per 3.3. `renderer` feeds M1-13 bootstrap phase A, which runs *before* the service exists. That reader must apply the same lenient parse plus the `auto` fallback so a typo can't stop startup. Reuse `UiSettings` deserialization; don't parse it a second way. |
| 3.5 | Strict IPC vs lenient file parsing. Webview input is the less-trusted side, because an XSS can call `invoke`. | IPC setters (Classic `settings_set_library_view_mode` etc.) should take strict typed enums, so serde rejects bad input. Values still pass through `normalize()` inside `SettingsService::update`. Lenience applies only to the hand-edited file. |
| 3.6 | Forward-compatibility keys inside `[ui]` being dropped, or leaking to the frontend. | Give `UiSettings` an `extra: toml::Table` with `#[serde(flatten)]` (the `ConfigHistorySettings` pattern, `types.rs:55-56`). Add it to `clear_extra` (`types.rs:368`) and `preserve_extra_from` (`types.rs:381`) so IPC strips it (`commands/settings.rs:77`). Test that `flatten` and the lenient fields work together. |

## 4. Debounced writer, atomic save and lost updates

| # | Threat | Mitigation |
|---|---|---|
| 4.1 | **Lost update.** The debouncer keeps a full `AppSettingsData` taken at t0. The Settings form saves at t0.5. The flush at t1 writes the stale snapshot over the form's changes, including the API key, taps and sources. | The debouncer buffers only a **`UiPatch`**, one `Option<_>` per `[ui]` field. The flush runs `service.update(move \|s\| patch.apply(&mut s.ui))`. That re-reads from disk under `io_lock` and changes only the dirty fields. Concurrent edits to *different* `[ui]` fields then merge, and the last writer wins per field. |
| 4.2 | **Settings form wiping `[ui]`.** `merge_save_request` builds a new `AppSettingsData` from scratch (`services/settings.rs:109-174`), and `SettingsSaveRequest` (`:42-104`) has no `ui` field. | In `merge_save_request`, set `ui: current.ui` (or `data.ui.unwrap_or(current.ui)` if M3-06 adds it later as `Option<UiSettings>`). Because the struct literal is exhaustive, the compiler forces this decision. Add a test: form save after a view-mode change keeps `[ui]`. Classic's view-mode toggle must be a **targeted** command, not a full-form save. |
| 4.3 | **Cross-process race.** Classic and native running together, the CLI (`crosshook-cli/src/*.rs` uses `SettingsStore::try_new`), or an editor. `io_lock` is process-local. | The window is milliseconds (read, parse, write) because writers always re-read. This is a residual risk; document it. If AC4 ("visible in native and vice versa") is ever tested with both apps open at once, add an advisory `nix::fcntl::Flock` on a sidecar `settings.toml.lock` inside `SettingsStore::update`. Never lock `settings.toml` itself, because rename replaces the inode. Not needed for the stated acceptance criteria `[INFERENCE]`. |
| 4.4 | **Temp-file permissions.** | Already safe: `tempfile` creates the file 0600 with O_EXCL in the same directory, then applies the target's permissions or 0644 *before* rename (`fs_util.rs:87-106`). There is no window where the file is more open than its target. No change is needed. Pre-existing and out of scope: a new `settings.toml` is created 0644 under a default-umask config directory (`store.rs:100`) and holds `steamgriddb_api_key`. `[ui]` adds no secrets, so this feature doesn't widen the exposure. Track it as a follow-up if desired. |
| 4.5 | **Unwritable file**: error spam or a retry storm at 1 Hz. | On write error, keep values in memory, emit **one** toast per session (an `AtomicBool` latch), log once, and don't re-arm the timer until the next real change. Map errors through `AppError::from` (fixed strings, `error.rs:149-165`). |
| 4.6 | **Pending patch lost on exit.** | Flush on shutdown or window close with a bounded wait. Also flush before any full-form save, or let the form's `update` win and re-apply the patch afterwards. Either way the patch goes through `update`. |
| 4.7 | **Write amplification**: each save does two fsyncs. | Trailing debounce with a 1 s window. 100 resize events → ≤2 writes, testable with `tokio::time::pause` (app dev-dependencies already include `test-util`, `crosshook-app/Cargo.toml:35`). Skip the write when the patch is a no-op against the loaded value. |

## 5. `Debug` and log leakage

- `AppSettingsData`'s `Debug` is hand-written and lists fields explicitly (`types.rs:408-470`). A new `ui` field will **silently be left out** unless someone adds `.field("ui", &self.ui)`. Leaving it out is safe, but switching to `#[derive(Debug)]` "to fix it" would leak `steamgriddb_api_key` and `extra`.
- `UiSettings` holds no secrets. A derived `Debug` would still print `extra`, which could carry unknown keys written by a newer build. **Write `Debug` by hand**: print every known field plus `extra_keys: self.extra.len()`, matching how the parent omits `extra`.
- There is no test today that guards the redaction (no match for `redacted` in `settings/tests.rs`). Add `debug_redacts_secrets_and_extras`: set the API key, a top-level extra and a `[ui]` extra, then assert that `format!("{settings:?}")` contains none of their values and does contain `"<redacted>"`.
- `SettingsSaveRequest` and `SettingsSnapshot` derive `Debug` (`services/settings.rs:18,42`). The first contains no key; the second delegates to the redacting impl. No change is needed.
- Never `tracing::*!` raw localStorage values, palette ids under `warn!`, or import paths. Use counts and fixed reason codes (1.10, 2).

## 6. Recommended implementation checklist (security-relevant only)

1. `settings/ui.rs` (new; keep under ~500 lines, per CLAUDE.md): `UiSettings` with lenient fields, `extra`, a hand-written `Debug`, `normalize()` (finite check, `i64` clamps, palette grammar, dedupe and cap), and the `CommandId` newtype.
2. Wire it in: `AppSettingsData.ui` (lenient), `Default`, `Debug` (`types.rs:408`), `clear_extra`/`preserve_extra_from`, `normalize()` in `services/settings.rs:177`, and `ui: current.ui` in `merge_save_request`.
3. `settings/webkit_import.rs`: candidate paths from `base_dirs` plus constants, lstat regular-file and size checks, an `immutable=1` URI open with NOFOLLOW, `trusted_schema=OFF`, `query_only`, the `ItemTable`-is-a-table check, a bounded primary-key query, an exact byte allow-list, `debug!` reason codes, and a one-shot write through `update`.
4. Debouncer in `crosshook-app`: a `UiPatch` buffer applied through `SettingsService::update` only; a single toast latch; flush on shutdown.
5. Tests: NaN/±inf/integer `ui_scale`; negative and `i64::MAX` dimensions; bad enum and bad `[ui]` type → load still succeeds and other settings survive; palette grammar accept/reject table (paths, `~`, URLs, uppercase tokens, 65-byte ids, profile-name suffix); dedupe plus cap at 20; form save keeps `[ui]`; the `Debug` redaction test; the import fixtures from §1 including the no-`-shm` assertion.

## 7. Paths reviewed

`settings/{types.rs,store.rs,paths.rs,recent.rs,tests.rs}`, `fs_util.rs`, `metadata/db.rs`, `profile/lutris_import/paths.rs`, `profile/toml_store/utils.rs`, `platform/{dirs.rs,xdg.rs}`, `app_id_migration.rs`, `export/diagnostics/{mod.rs,profiles.rs}`, `crosshook-app/src/{services/settings.rs,error.rs,bootstrap/stores.rs,bootstrap/state.rs}`, `src-tauri/src/{lib.rs,commands/settings.rs}`, `src/lib/commands.ts`, `src/components/pages/LibraryPage.tsx`, Cargo manifests and `/tmp/YAN-869.md`.
