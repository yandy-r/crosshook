# Research: business rules & invariants — `[ui]` settings table (YAN-869 / #537, M1-48)

Paths are relative to `src/crosshook-native/` unless noted. Spec: `/tmp/YAN-869.md`.

## 1. Current behaviour (ground truth)

| Fact | Evidence |
| --- | --- |
| Every existing settings enum **fails the whole file load** on an unknown string (derived `Deserialize`, or `FromStr` → `serde::de::Error::custom`). | `crates/crosshook-core/src/settings/types.rs` (`UmuPreference`, `UmuDatabaseLookupPreference`, `HighContrastPreference` custom de, `ReducedMotionPreference`); tests `settings/tests.rs:138-157` (`high_contrast = "yes"` → `Err(TomlDe)`), `:192-214` (reduced_motion string/bool → `Err(TomlDe)`), `:583-609` (`from_str` rejects unknown) |
| M1-11 policy: a malformed **known** field must fail rather than silently become an extra. | `crates/crosshook-core/tests/unknown_settings_keys.rs:133-146` |
| A failed load at startup means the **whole app uses defaults** and is marked `settings_unreadable`. | `crates/crosshook-app/src/bootstrap/app_bootstrap.rs:144-160` (`load_settings_step` → `unwrap_or_default`) |
| A failed load also blocks **every write**: `SettingsStore::update` calls `load_unlocked()?` before saving, so an unreadable file is never overwritten. | `settings/store.rs:126-143` |
| `SettingsService::update` normalizes via a clamp-only `normalize()`; there is no warning logging. | `crates/crosshook-app/src/services/settings.rs:177-188, 244-260` |
| Existing clamp pattern: `u32::clamp` with pub `*_MIN/*_MAX` consts, applied on save. | `types.rs:9-12,35-38` (`clamp_recent_files_limit`), `:69-77`; re-exported in `settings/mod.rs:12-19` |
| Unknown keys are preserved through `#[serde(default, flatten)] extra: toml::Table`, with `clear_extra()` for IPC and `preserve_extra_from()` for merges. There is no `deny_unknown_fields` anywhere in `settings/`. | `types.rs` (`AppSettingsData.extra`, `clear_extra`, `preserve_extra_from`); `ConfigHistorySettings.extra` |
| `[ui]` already exists as an **unknown table** in the M1-11 fixture, with names and types that **conflict with the spec**: `scale` (spec says `ui_scale`), `console_height` (spec says `console_drawer_height`), `controller_mode = true` (spec says the enum `auto/on/off`). | `tests/unknown_settings_keys.rs:5-19`; assertion `saved["ui"] == original["ui"]` at `:56-66` |
| Settings load at bootstrap runs **before** the tracing subscriber is installed. Only Phase A steps are re-logged afterwards. | `app_bootstrap.rs:96-99`, `relog_degraded_phase_a` `:276-291`, `steps.rs:53` |
| Classic today: `loadViewMode()` maps anything other than `'list'` to `'grid'`. | `src/components/pages/LibraryPage.tsx:30-35`, setter `:127-130` |
| Classic palette ids are `route:<route>`, `profile:launch-current:<name>` and `profile:edit-current:<name>`. The `CommandPaletteCommandId` type is just `string`. Classic has no recency. | `src/lib/commands.ts:3,119,137,147`; `src/hooks/useCommandPalette.ts` (no MRU) |
| Profile names can contain spaces and Unicode. They cannot contain `/ \ :`, control characters or `<>"|?*`. | `crates/crosshook-core/src/profile/toml_store/utils.rs:57-90` |
| Classic layout defaults: inspector widths 360/320/280 by breakpoint, console drawer 280px, window 1280×800. | `src/components/layout/inspectorVariants.ts:3-7`, `src/styles/variables.css:142`, `src-tauri/tauri.conf.json:16-17` |
| `types.rs` is 475 lines and `settings/tests.rs` is 632. The soft cap is about 500 (CLAUDE.md:50). | — |

**Conclusion:** if `[ui]` enums were plain serde derives, `renderer = "vulkan"` would make the entire settings file unreadable. The app would boot on full defaults and refuse every settings write. That contradicts the spec ("an invalid value is clamped or reset to its default with a logged warning"). `[ui]` therefore needs a **lenient, per-field deserializer**. Non-`[ui]` keys stay fail-closed, which is out of scope and keeps the `malformed_known_fields…` test valid.

## 2. Per-key rules (recommended contract)

"Reset" means the key takes its default and a warning is logged. "Clamp" means the value is pinned to the nearest bound and a warning is logged. A type mismatch is always a reset.

| Key | Type / TOML form | Valid | Default | Out-of-range | Unknown / wrong type |
| --- | --- | --- | --- | --- | --- |
| `library_view_mode` | `Option<LibraryViewMode>`, `"grid" \| "list"` | exact match after trim (mirrors existing `FromStr` style) | absent ⇒ getter returns `grid` | n/a | reset to `grid` (counts as **present**: no localStorage import) |
| `window_width` | `Option<u32>`, integer | 800..=16384 | absent ⇒ centred default (README §10). Consumer fallback is 1280 | clamp (negatives clamp to 800) | reset to `None` |
| `window_height` | `Option<u32>`, integer | 600..=16384 | absent ⇒ consumer fallback 800 | clamp | reset to `None` |
| `ui_scale` | `f64`, float, or integer coerced to f64 | 0.75..=2.0 | `1.0` | clamp | **NaN/±inf (valid TOML literals) ⇒ reset to 1.0**. `f64::clamp(NaN)` returns NaN, so check `is_finite()` first |
| `density` | enum `compact \| comfortable \| spacious` | exact | **not in spec**; recommend `comfortable` | n/a | reset |
| `renderer` | enum `auto \| wgpu \| gl \| tiny-skia` | exact | `auto` | n/a | reset (`"vulkan"` → `auto`, warn). Needs `#[serde(rename = "tiny-skia")]` or kebab-case: `rename_all = "snake_case"` would emit `tiny_skia` |
| `controller_mode` | enum `auto \| on \| off` | exact | `auto` | n/a | reset. A legacy bool is **not** mapped: no shipped build ever wrote a bool, so it is reset (unlike `HighContrastPreference`) |
| `sidebar_collapsed` | `bool` | true/false | `false` | n/a | reset to `false` |
| `inspector_width` | `u32`, integer | 240..=640 | **not in spec**; recommend `320` (Classic desk) | clamp | reset |
| `console_drawer_height` | `u32`, integer | 120..=800 | **not in spec**; recommend `280` (`variables.css:142`) | clamp | reset |
| `palette_recent` | `Vec<String>` | see §3 | `[]` | truncate to 20 (warn) | non-array ⇒ `[]`. Bad elements are dropped individually; the rest are kept |
| (any other key in `[ui]`) | `#[serde(flatten)] extra: toml::Table` | — | — | — | preserved verbatim (forward compatibility) |
| `[ui]` itself not a table (e.g. `ui = 3`) | — | — | all defaults | — | reset the whole table to defaults with a warning. Never fail the file |

Further rules:
- **Same rules on write.** `normalize()` in `services/settings.rs:177` must also run `settings.ui.normalize()`, so typed setters clamp the same way the load path does. This matches how `recent_files_limit` is clamped on save. Setters take typed enums, so an invalid variant can't be represented. Numeric setters clamp silently: no error, no toast.
- **Keep `console_drawer_height` separate from the top-level `console_drawer_collapsed_default`** (`types.rs` `AppSettingsData`). Do not move or rename the existing key.
- **`window_*` is saved only on resize end and only for a normal (non-maximized, non-fullscreen) window.** Otherwise a fullscreen size becomes the restored size. That is a consumer rule (M1-19); persistence only clamps. Fitting to the monitor is also the consumer's job.
- **Naming conflict to settle.** README §10 (`docs/research/native-ui-migration/README.md:544-547`) says `scale` with default `auto`, and `renderer` values `auto/wgpu/gl/software`. The issue spec (newer) says `ui_scale` = 1.0 and `tiny-skia`. **Follow the spec.** Note the drift in the PR and update §10 or the docs settings reference.

## 3. `palette_recent` invariants

- **Order:** index 0 is the most recent (MRU). To record an id: trim it, validate it, remove any existing equal entry, insert at 0, then truncate to `PALETTE_RECENT_MAX = 20`.
- **Load normalization:** drop invalid entries, then dedupe keeping the **first** occurrence (that preserves MRU), then truncate to 20. Warn once per dropped or truncated batch, not per entry.
- **Dedupe key:** exact string after trimming (no case folding). Profile-based ids embed case-sensitive profile names.
- **"Ids, never paths": validate structure, not registry membership.** Core cannot know the native command registry (M4-10 will define it). Unknown but well-formed ids must survive, both for forward compatibility and for ids from commands that are temporarily unavailable. A proposed `is_valid_command_id(s)`:
  - non-empty after trim, at most 128 bytes, no `char::is_control`;
  - contains neither `/` nor `\` (rejects POSIX paths, `~/…` and `C:\…`), and does not start with `~` or `.`.
  - This accepts every Classic id form, including `profile:launch-current:My Game`: profile names cannot contain `/ \ :` (`utils.rs:57-90`).
  - A strict `[a-z0-9:_.-]` charset would wrongly reject ids that carry profile names with spaces, so don't use one.
- **Stale ids** (renamed or deleted profile, removed command) are **not pruned in core**. The palette skips ids it can't resolve. Pruning against the profile list would couple settings to `ProfileStore`.
- **Privacy:** profile names in ids are already persisted as `last_used_profile`, so this is not a new data class. When logging warnings, never print the full rejected value. Log the index and reason only, because a rejected entry may be a path.

## 4. Implementation shape that satisfies the invariants

1. **New module `crates/crosshook-core/src/settings/ui.rs`** (keeps `types.rs` under the cap), referenced from `types.rs` as `pub ui: UiSettings` with `#[serde(default)]`. Put tests in `settings/ui_tests.rs` or `ui/tests.rs`, because `tests.rs` is already 632 lines. Re-export the types, consts and helper fns from `settings/mod.rs:12-19`.
2. **Lenient deserialization.** Hand-write `impl Deserialize for UiSettings`:
   - Deserialize into a `toml::Value`. If it is not a table, use defaults and a warning.
   - Pull each known key out with `remove(key)` and parse it through `parse_enum / parse_u32_clamped / parse_f64_clamped / parse_bool / parse_ids`. Each returns `(value, Option<UiWarning>)`.
   - Whatever remains in the table becomes `extra`.
   - This never errors, so a bad `[ui]` can never make `settings.toml` unreadable. Do **not** add `#[serde(deny_unknown_fields)]`.
3. **Pure core.** A pure `UiSettings::normalize(&mut self) -> Vec<UiWarning>` is shared by the load path and the setters, which makes the clamp tests table-driven. The deserializer emits `tracing::warn!(field, reason, action)` for each warning (core already depends on `tracing`, `crosshook-core/Cargo.toml:34`).
4. **Extras plumbing.** Extend `AppSettingsData::clear_extra` with `self.ui.extra.clear()` and `preserve_extra_from` with `self.ui.extra.clone_from(&current.ui.extra)`. Otherwise IPC would leak or drop future `[ui]` keys.
5. **Struct literal.** `merge_save_request` (`services/settings.rs:109-174`) builds `AppSettingsData` field by field, so the new field is a compile error there until handled. Set **`ui: current.ui`** and do **not** add `ui` to `SettingsSaveRequest`. Classic's Settings form posts a full request (`src/lib/mocks/handlers/settings.ts:13-26` shows the shape), and must never clobber `[ui]` keys written by native or by the Library toggle. `[ui]` writes go only through typed setters / dedicated commands. Each setter mutates **one field** inside `store.update(...)`, never a replacement of the whole `ui`, so concurrent per-key edits don't lose each other.
6. **IPC read.** `AppSettingsIpcData` (`src-tauri/src/commands/settings.rs:23-67`) needs `ui` (at least `library_view_mode`) for Classic. Add a write command for the view mode. Browser mocks and `scripts/check-mock-coverage.sh` must cover the new command, and `src/types/settings.ts` defaults (`:150-162`) must gain `ui`.

## 5. Degraded behaviour & write semantics

- **Invalid `[ui]` value:** load succeeds; the value is reset or clamped in memory and a warning is logged. The file is **not** rewritten just to normalize; the next real save persists the normalized value. Don't call `migrate_or_save_settings` for this.
- **Invalid non-`[ui]` key:** unchanged. The whole file is unreadable, bootstrap falls back to defaults, and writes are refused (`store.rs:126-143`). `[ui]` setters therefore also fail in that state. Per the spec, in-memory `[ui]` values still apply for the session, with **one** error toast. The service needs an in-memory `UiSettings` mirror plus a "write-failure already reported" latch that resets after a successful write.
- **Debounce:** at most one write per second; 100 resize events must produce at most 2 writes. That implies leading-edge + trailing-edge or trailing-only coalescing. Last value wins.
  - **Risk:** the final value is lost if the app exits inside the window. Recommend an explicit `flush()` on shutdown or window close.
  - Test with `#[tokio::test(start_paused = true)]` (tokio `test-util` is already a dev-dependency, `crosshook-app/Cargo.toml:35`; pattern in `src/log_tail/tests.rs:320`).
- **Warning timing risk:** warnings raised by the bootstrap load (`app_bootstrap.rs:97`) fire before logging exists and are lost. They are re-emitted on the next post-logging load (`SettingsService::load`; Classic calls `settings_load` on mount). If the spec's "logged warning" must hold at boot, return the warnings from a `load_with_report` and log them after `init_logging_step`, mirroring `relog_degraded_phase_a`. Recommend deciding this explicitly. Tests should assert the warning on a post-init load.

## 6. Cross-version compatibility

| Scenario | Outcome |
| --- | --- |
| Old file without `[ui]` | Every field defaults (`#[serde(default)]`). `library_view_mode = None` triggers the one-time import. Add `settings_backward_compat_without_ui` next to the `settings_backward_compat_*` tests (`tests.rs:350-560`). |
| 0.6.1+ build (M1-11, no `UiSettings`) reads a 0.7 file | `[ui]` lands in `AppSettingsData.extra` and round-trips verbatim (`unknown_settings_keys.rs:36-67`). |
| ≤0.6.0 (no M1-11) | Ignores `[ui]` on load (no `deny_unknown_fields`) but **drops it on save**. This is the documented downgrade-matrix row (README §10 :565) and is not fixable here. |
| A newer build adds a `[ui]` key | Preserved through `ui.extra`. |
| A newer build adds a new **variant** (e.g. `renderer = "vello"`) and an older 0.7 opens the file | Reset to `auto` in memory. The **next save writes `auto`**, which clobbers the newer choice. Preserving it would mean keeping the raw value and skipping serialization of the defaulted field (a duplicate key in field plus `extra` is invalid TOML). Recommend accepting this as a known limitation and recording it for M6-03 (YAN-903). |
| Byte stability | `settings_normalized_save_is_byte_stable_across_reload_and_save` (`unknown_settings_keys.rs:69-79`) must still pass. Field order is stable (declaration order, then `extra`). `Option` window sizes need `skip_serializing_if = "Option::is_none"`. `f64` prints its shortest round-trip form. |

**Required fixture update.** The M1-11 fixture `[ui]` (`unknown_settings_keys.rs:7-19`) becomes a *known* table. `controller_mode = true` would now reset to `"auto"` on save, and the `saved["ui"] == original["ui"]` assertion would fail.
- Change it to valid typed values (`controller_mode = "auto"`, `ui_scale = 1.1`, `console_drawer_height = 220`).
- Deliberately **keep one unknown key inside `[ui]`** (for example `scale = 1.1` or `future_ui_key`), so the test also proves `ui.extra` preservation.
- The `[ui]` case in `malformed_known_fields_fail_instead_of_becoming_extras` (`:140`) still fails because of `config_history`, so it stays correct. Add a new test asserting that `[ui] renderer = "vulkan"` plus `ui_scale = "big"` **loads Ok** with defaults.

## 7. One-time localStorage import (business-rule view)

- **Trigger:** only when `[ui].library_view_mode` is **absent**. That requires the `Option`; a plain `LibraryViewMode` default can't tell absent from `grid`. A present-but-invalid value is reset and does **not** import.
- **Accepted values:** `grid` or `list` only. Anything else maps to `grid`, matching Classic's `LibraryPage.tsx:32-35`.
- **Access:** open the SQLite file strictly read-only (rusqlite is already in core, `Cargo.toml:40`), ideally `immutable=1` / `SQLITE_OPEN_READ_ONLY` so no `-wal`/`-shm` files are created in WebKit's directory. Never write it.
- **Persisting the result:** after an attempt (hit, miss or corrupt), write the resolved value so the import is truly **once**. Classic stops writing localStorage in the same change, so the source is frozen. Never write when settings are unreadable.
- **Paths:**
  - Flatpak: `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/` (spec).
  - Non-Flatpak: likely `$XDG_DATA_HOME/dev.crosshook.CrossHook/localstorage/`, using the identifier from `tauri.conf.json:4`. `[INFERENCE]`, verify.
  - WebKit row format (`ItemTable(key, value)`, value UTF-16LE): `[INFERENCE]`, verify against a real fixture.

## 8. Risks / open decisions

1. Defaults for `density`, `inspector_width` and `console_drawer_height` are missing from the spec. Proposed: `comfortable` / 320 / 280.
2. Spec vs README naming (`ui_scale` vs `scale`, `tiny-skia` vs `software`). Recommend following the spec and fixing README §10 or the docs reference.
3. Bootstrap warning loss (§5). Decide between re-emitting on the next load and a `load_with_report`.
4. Downgrade clobbers unknown future variants (§6). Accept and document.
5. Debounce exit window: needs a flush on shutdown.
6. A Classic form save must not touch `[ui]` (`ui: current.ui` in the merge). A regression test is needed in `services/settings/tests.rs`.
7. No user-facing settings reference exists under `docs/` today (grep for `settings.toml` and `recent_files_limit` in `docs/getting-started`, `docs/features` finds none). The "docs settings reference" acceptance item means **creating** one, e.g. `docs/features/settings-reference.doc.md` (filename `[INFERENCE]`).
