# Profile write paths — business inventory (YAN-811 / GitHub #532, M1-06)

Current behavior of every profile mutation that `ProfileWriter` must absorb, plus the five React `profiles-changed` listeners. Paths are relative to `src/crosshook-native/` unless noted. Line numbers are from the worktree `refactor/yan-811-profile-writer` at research time.

The issue narrative (“every write is validate → load → mutate → save → observe → revision → health invalidate → emit”) is the **target** pipeline. It is **not** what the code does today. Only eight call sites emit `profiles-changed`, and only the Proton migration path writes a health snapshot inline. Several mutating commands cannot emit because they do not take an `AppHandle`.

`apply_collection_defaults` (`commands/profile/shared.rs:200-239`) stays out of this issue (M1-35). It is a read-time merge used by `profile_load` and `profile_list_summaries` only.

## Executive Summary

Profile TOML writes go through `ProfileStore` (`crates/crosshook-core/src/profile/toml_store/store.rs`). `save` (line 95) atomically writes `storage_profile()` pretty-TOML and, best-effort, rewrites the MangoHud companion conf. Metadata sync is `MetadataStore::observe_profile_write` / `_rename` / `_delete` (`metadata/profile_ops.rs:11-93`, SQL in `metadata/profile_sync.rs`). Revisions are `capture_config_revision` (`commands/profile/shared.rs:112-172`) → `insert_config_revision` (dedupe on the latest row’s SHA-256, `metadata/config_history_store/mod.rs:19-71`). The Tauri event is a **bare string** (`shared.rs:68-72`: `app.emit("profiles-changed", reason.to_string())`). Browser mocks emit `{ name, action }` objects instead (`src/lib/mocks/handlers/profile-*.ts`). Listeners subscribe as `subscribeEvent<string>` and ignore the payload, so both shapes happen to refresh everything.

There is no `services/profiles/` module and no `AppEvent::ProfilesChanged` yet (`crates/crosshook-app/src/events.rs` only has `Resync` and `StartupJobFinished`). `src-tauri/src/startup_bridge.rs:121-132` drops every non-startup event, including a future profile event, in the `_ => {}` arm.

## User Stories

- As a CrossHook user, I want favorite toggles to stay instant, so that the UI does not rescan health for a change that cannot affect it.
- As a CrossHook user, I want saves, renames, duplicates and rollbacks to behave exactly as before, so that my TOML profiles and history stay intact.
- As a native-UI developer, I want one profile write pipeline in crosshook-app, so that the iced UI reuses it without duplicating Tauri command logic.

## The 23 profile commands

Registered from `commands/profile/mod.rs` and `src-tauri/src/lib.rs`. “Emit” is the exact string passed to `emit_profiles_changed`, or “none”.

| # | Command | File:lines | Mutates | Observe | Revision source | Health snapshot | Emit | Target `ProfileChangeReason` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `profile_list` | `lifecycle.rs:21-23` | no | — | — | — | none | none (read) |
| 2 | `profile_load` | `lifecycle.rs:38-55` | no (read + collection merge + `clear_extra`) | — | — | — | none | none (read) |
| 3 | `profile_list_summaries` | `lifecycle.rs:58-100` | no | — | — | — | none | none (read) |
| 4 | `profile_save` | `lifecycle.rs:103-226` | TOML via `store.save` | `AppWrite` | `ManualSave` | no | `"saved"` | `Saved` |
| 5 | `profile_delete` | `lifecycle.rs:229-251` | deletes TOML + best-effort launcher files | `observe_profile_delete` | none | no | `"deleted"` | `Deleted` |
| 6 | `profile_duplicate` | `lifecycle.rs:268-290` | new TOML via `store.duplicate` | `AppDuplicate` + source profile id | **none** | no | **none** (no `AppHandle`) | `Duplicated` |
| 7 | `profile_rename` | `lifecycle.rs:293-347` | `store.rename`, then a second `store.save` of `display_name`, plus `settings.toml` `last_used_profile` | `observe_profile_rename` only (the display-name save is **not** observed) | **none** | no | **none** (no `AppHandle`) | `Renamed { from }` |
| 8 | `profile_import_legacy` | `lifecycle.rs:350-384` | `store.import_legacy` | `SyncSource::Import` | `Import` | no | `"imported-legacy"` | `Imported` |
| 9 | `profile_export_toml` | `lifecycle.rs:389-397` | no (in-memory → string) | — | — | — | none | none (pure serialize) |
| 10 | `profile_save_launch_optimizations` | `optimizations.rs:69-108` | `save_launch_optimizations` then reload | `AppWrite` (only if reload succeeds) | `LaunchOptimizationSave` | no | **none** | `Saved` (content change; see health note) |
| 11 | `profile_save_mangohud_config` | `optimizations.rs:112-128` | `save_profile_section` | `AppWrite` | `ManualSave` | no | **none** | `Saved` |
| 12 | `profile_save_gamescope_config` | `optimizations.rs:132-148` | same | `AppWrite` | `ManualSave` | no | **none** | `Saved` |
| 13 | `profile_save_trainer_gamescope_config` | `optimizations.rs:152-168` | same, writes `launch.trainer_gamescope` | `AppWrite` | `ManualSave` | no | **none** | `Saved` |
| 14 | `profile_list_bundled_optimization_presets` | `optimizations.rs:172-185` | no; empty vec if metadata unavailable | — | — | — | none | none (read) |
| 15 | `profile_apply_bundled_optimization_preset` | `optimizations.rs:189-270` | `materialize_launch_optimization_preset(..., set_as_active=true)` | `observe_profile_write_launch_change` (`AppWrite`) | `PresetApply` | no | `"bundled-optimization-preset"` | `PresetApplied` |
| 16 | `profile_save_manual_optimization_preset` | `optimizations.rs:273-343` | `save_manual_launch_optimization_preset` | same | `PresetApply` | no | `"manual-optimization-preset"` | `PresetApplied` |
| 17 | `profile_save_command_arguments` | `command_arguments.rs:25-66` | `save_command_arguments` | `AppWrite` if reload succeeds | `LaunchCommandArgumentsSave` | no | **none** | `Saved` |
| 18 | `profile_config_history` | `config_history.rs:107-145` | no | — | — | — | none | none (read; errors if metadata unavailable) |
| 19 | `profile_config_diff` | `config_history.rs:152-254` | no | — | — | — | none | none (read) |
| 20 | `profile_config_rollback` | `config_history.rs:260-338` | `store.save` of parsed snapshot | `AppWrite` | `RollbackApply` with `source_revision_id` | no | `"rollback"` | `RolledBack` |
| 21 | `profile_mark_known_good` | `config_history.rs:343-374` | SQLite flag only | — | — | — | **none** | none (not a profile-content change) |
| 22 | `profile_set_favorite` | `favorites.rs:7-18` | `profiles.is_favorite` only | — | — | — | `"favorite-updated"` | `FavoriteToggled` |
| 23 | `profile_list_favorites` | `favorites.rs:21-26` | no | — | — | — | none | none (read) |

### Also in scope, outside the 23

| Command | File:lines | What it does | Observe | Revision | Health | Emit | Target reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `protondb_accept_suggestion` | `commands/protondb.rs:108-234` | Catalog: append `enabled_option_ids` if missing. EnvVar: insert `launch.custom_env_vars`. Both `ProfileStore::save`. | `AppWrite` via `observe_suggestion_apply` (`protondb.rs:18-48`) | `ProtonDbSuggestionApply` | no | **none** (no `AppHandle`) | `Saved` (revision source stays `ProtonDbSuggestionApply`) |
| `apply_proton_migration` | `commands/migration.rs:63-147` | `apply_single_migration` (`profile/migration/apply.rs:12-108`) writes TOML with `write_atomic` on `storage_profile()`, **not** `ProfileStore::save` | `SyncSource::AppMigration` only when outcome is `Applied` | **none** (`ConfigRevisionSource::Migration` exists at `metadata/models.rs:475-496` and is unused) | **yes**: recompute `check_profile_health` and `upsert_health_snapshot` (`migration.rs:108-136`) | **none** | `Migrated` |
| `apply_batch_migration` | `commands/migration.rs:157-265` | Preflight all paths + loads, then per-item `apply_single_migration`. One failure does not abort the rest. | same, per applied item | none | same upsert, per applied item | **none** | one `Migrated` for the applied names |

`check_proton_migrations` (`migration.rs:24-57`) is read-only and stays a scan.

### Adjacent writers that call the same helpers (cutover hazards)

These are not named in the issue’s command list, but they import `capture_config_revision` / `emit_profiles_changed` or call `observe_profile_write`. Deleting the shell helpers without migrating them breaks the build.

| Command | File:lines | Notes |
| --- | --- | --- |
| `lutris_import_profiles` | `commands/lutris.rs:28-80` | For each `Imported` entry: `observe_profile_write(..., LutrisImport)`, then `capture_config_revision(..., Import)`. Emits `"lutris-import"` once if `imported_count > 0`. Maps to `Imported`. |
| `community_import_profile` | `commands/community.rs:104-177` | `observe_profile_write(..., Import)` plus a version-snapshot seed. **No** revision capture, **no** event. |
| Launch success hook | `commands/launch/streaming.rs:380-418` | After a clean/indeterminate exit, `set_known_good_revision` on the latest revision. Not a profile write. Independent of `profile_mark_known_good`. |

`crosshook-cli` and `crosshook-app` bootstrap do not call `observe_profile_write` or `ProfileStore::save`.

## What each mutating path actually checks

### `profile_save` (`lifecycle.rs:103-226`)

1. `validate_steam_app_id(runtime.steam_app_id.trim())` (`profile/models/resolve.rs:58`). Empty is valid. Non-digits and more than 12 digits fail the whole save (`"Invalid Steam App ID in runtime section: …"`). No other field is validated here.
2. `data.clear_extra()`. If the file already exists, `store.load` and `preserve_extra_from` so unknown TOML keys survive (`profile/models/extra.rs:163-166`). New profiles skip that load.
3. New profile (`!store.profile_exists`, `store.rs:477`): `settings_store.load()` is **required** (load error fails the save). `apply_profile_creation_defaults_from_settings` (`profile/creation_defaults.rs:7-27`) fills only empty fields: `runtime.proton_path`, `launch.method`, and `trainer.loading_mode` when it is still the default (`SourceDirectory`). Then, if `default_bundled_optimization_preset_id` is non-empty, metadata is available, and `launch.active_preset` is empty, copy that preset’s option ids into `launch.presets[bundled/…]`, set `active_preset`, and mirror ids into `launch.optimizations`. Missing/unreadable preset is logged and skipped; it does not fail the save. An explicit wizard `active_preset` is not overwritten.
4. Custom art, three fields, each independent and fail-soft (warn, keep the original path): `game.custom_cover_art_path` via `import_custom_cover_art`; `custom_portrait_art_path` and `custom_background_art_path` via `import_custom_art` with `GameImageType::Portrait` / `Background`. Skipped when empty or `is_in_managed_media_dir` (`lifecycle.rs:168-197`).
5. `store.save` — this is the hard failure. Name validation happens inside `profile_path` → `validate_name` (`toml_store/utils.rs:72-109`): trim, reject empty / `.` / `..` / the anonymous-profile sentinel / control chars / absolute paths / `/` `\` `:` and `<>:"/\|?*`.
6. `observe_profile_write(..., AppWrite, None)` fail-soft warn.
7. `capture_config_revision(..., ManualSave, source_revision_id=None)`.
8. Emit `"saved"`.

`ProfileStore::save` also rewrites the MangoHud companion file and only warns on that failure (`store.rs:112-122`). Every `save` does this, including rollback and the rename display-name rewrite. Migration does **not**.

### `profile_delete` (`lifecycle.rs:229-251`)

Load is best-effort. If it succeeds, `cleanup_launchers_for_profile_delete` (`shared.rs:44-65`):

- `launch.method == "native"` → skip, `Ok(None)`, debug log. No files touched.
- Otherwise derive Steam root as the prefix of `steam.compatdata_path` before `/steamapps/compatdata/` (`shared.rs:16-21`). Home is that root with `/.local/share/Steam` or `/.steam/root` stripped, else `$HOME`, else `""` plus a warning (`shared.rs:24-41`).
- `export::delete_launcher_for_profile`. A cleanup error is **logged and ignored**. Profile deletion still proceeds.

Then `store.delete` (hard fail if the TOML is missing: `NotFound`). Deletes the MangoHud companion best-effort (`store.rs:313-333`). `observe_profile_delete` soft-sets `profiles.deleted_at` (`profile_sync.rs:168-176`), fail-soft. Emit `"deleted"`. No revision row is written or removed. Favorites disappear because `list_favorite_profiles` filters `deleted_at IS NULL` (`metadata/collections.rs:189-194`).

### `profile_duplicate` (`lifecycle.rs:268-290`)

`lookup_profile_id` of the source is fail-open (`ok().flatten()` → `None` if metadata is down or the row is missing). `ProfileStore::duplicate` (`store.rs:416-432`):

- Validates the source name, loads it, lists names, `generate_unique_copy_name` (`store.rs:448-468`).
- Strips a trailing `(Copy)` or `(Copy N)` via `strip_copy_suffix` (`utils.rs:119-137`). `"MyGame (Copy)"` becomes `"MyGame (Copy 2)"`, not `"MyGame (Copy) (Copy)"`. A name that is only `"(Copy)"` uses the full source string as the base.
- First candidate `"{base} (Copy)"`, then `"{base} (Copy 2)"` … `"{base} (Copy 1000)"`. All 1000 taken → `InvalidName`.
- Saves a clone. Source file is not modified. The clone keeps the source display name, game name, and launcher slug. Nothing here renames `steam.launcher.display_name`.

Observe uses `SyncSource::AppDuplicate` and the source profile id (so metadata can record lineage). Response `profile` is `clear_extra`’d; the file on disk still has the extras `save` wrote. **No revision. No event.** The UI refreshes itself: `useProfileCrud.ts:359-364` calls `refreshProfiles` + `loadProfile` after the invoke. Adding a `Duplicated` event will make the five listeners refresh **in addition** to that direct refresh.

### `profile_rename` (`lifecycle.rs:293-347`)

Order:

1. `store.load(old)` best-effort (used for cleanup and the display-name rewrite). Load failure does not block the rename; `had_launcher` stays false and `display_name` is not updated.
2. `store.rename` (`store.rs:336-380`). Hard fail: missing source, `AlreadyExists` if the new TOML exists, invalid name. Same name is a no-op `Ok(())` that still falls through the rest of the command. MangoHud companion is renamed, with copy+delete fallback.
3. `observe_profile_rename` (`profile_sync.rs:88-164`): requires an existing `profiles` row for `old_name` or returns `Corrupt` (warn, command still succeeds). Updates `current_filename` / `current_path` and inserts `profile_name_history` with source `"app_rename"`. Stable `profile_id` is preserved, so revision rows and the favorite flag stay attached. **No new revision row**, so history is not snapshotted at the rename.
4. Launcher cleanup on the **pre-rename** profile under the **old** name. `had_launcher` is true only when `script_deleted || desktop_entry_deleted`. Native → false. Cleanup error → false + warn. The command returns this `bool`; it does not re-export.
5. If the pre-rename load worked, load the new file and set `steam.launcher.display_name = new_name.trim()`, then `store.save`. That second save rewrites MangoHud conf and is **not** observed and **not** revisioned. Save failure is a warning only.
6. If `settings.last_used_profile.trim() == old_name.trim()`, rewrite it to `new_name.trim()` and `settings_store.save`. Failure is a warning. This is the only profile command that writes `settings.toml`.

Frontend re-export (not in the command): `useProfileCrud.ts:385-392` returns `{ ok, hadLauncher }` → `useProfileNotifications.ts:145-172` sets `pendingLauncherReExport` → `HeroDetailProfilesTab.tsx:122-180` → `useLauncherExport.ts:96-114` waits 150 ms, then `validate_launcher_export` + `export_launchers` with the **already renamed** profile. Export failure is silent. Undo rename (`useProfileNotifications.ts:145`) uses the same `hadLauncher` path.

### `profile_import_legacy` (`lifecycle.rs:350-384`)

`ProfileStore::import_legacy` (`store.rs:383-395`) takes the file stem as the profile name (`validate_name`), `legacy::load` from the parent directory, converts to `GameProfile`, `save` (overwrites an existing `{stem}.toml`). Observe + `Import` revision use that stem, not a caller-supplied name. Emit `"imported-legacy"`. Response is `clear_extra`.

### Launch optimizations (`optimizations.rs:54-108`, store `store.rs:129-188`)

Command rejects a blank trimmed name (`"profile name is required"`). Store:

- `switch_active_preset: Some` → trim, empty or unknown key → `LaunchPresetNotFound`; every id in that preset must be `is_known_launch_optimization_id` or `InvalidLaunchOptimizationId`. Copies the preset onto `launch.optimizations` and sets `active_preset`. Preset `extra` is replaced by the previous optimizations `extra`.
- Otherwise filter blank ids, reject unknown ids, write `enabled_option_ids`, and if `active_preset` is set, mirror the ids into that preset slot.

No `AppHandle`, so no event. Frontend autosave calls this directly (`useProfileLaunchAutosave.ts`, `useProfileLaunchAutosaveEffects.ts`) and does not wait for `profiles-changed`. Concurrent `save` / `save_launch_optimizations` are last-completed-write-wins (`store.rs:132-136`).

### Section saves (`shared.rs:242-277`)

`profile_save_mangohud_config`, `profile_save_gamescope_config`, `profile_save_trainer_gamescope_config`. Load (hard fail if missing) → clone → mutate one section → `clear_extra` + `preserve_extra_from` → `save` → observe `AppWrite` (warn) → revision `ManualSave`. Name is **not** trimmed (unlike optimizations and command args). No event. Revision source is `ManualSave`, not a distinct gamescope/mangohud source. `ConfigRevisionSource` has no overlay-specific variant (`metadata/models.rs:475-484`).

### Command arguments (`command_arguments.rs:25-66`, store `store.rs:192-233`)

Blank name → error. Store trims ids and drops empty custom tokens, then `validate_profile_command_arguments` with `resolved_launch_method` (the UI passes the effective method because the on-disk `launch.method` may be empty or `native` while the session is Proton/Steam — tests at `toml_store/tests/command_arguments.rs:206-256`). Unknown ids and invalid tokens fail the save; the command logs the rejection before mapping the error. Observe + `LaunchCommandArgumentsSave` only if the post-save `load` succeeds. No event.

### Presets

Bundled (`optimizations.rs:189-270`): metadata **required** (hard error if unavailable). Blank name or `preset_id` → error. Unknown preset → error. Corrupt `option_ids_json` → error. Materialize under `bundled_optimization_preset_toml_key`, set active. Observe, `PresetApply` revision, then `upsert_profile_launch_preset_metadata(..., Bundled, Some(preset_id))` fail-soft (skipped with debug if `profile_id` is missing). Emit `"bundled-optimization-preset"`. Response `clear_extra`.

Manual (`optimizations.rs:273-343`): blank name → error. `validate_manual_launch_preset_name` rejects empty and the bundled prefix (`store.rs:282-287`, test `toml_store/tests/manual_presets.rs`). Metadata upsert is `User` origin with `vendor_id=None`, and is skipped entirely when metadata is unavailable (the TOML write still happens). Emit `"manual-optimization-preset"`.

### Config history

History (`config_history.rs:107-145`): metadata required. Unknown profile id → empty list, not an error. `limit` defaults to and is capped at `MAX_HISTORY_LIST_LIMIT` (50, `metadata/models.rs:421`). Rows omit `snapshot_toml`.

Diff (`config_history.rs:152-254`): metadata required. `revision_id` and optional `right_revision_id` must be `> 0`. Both rows must belong to the profile id (`get_config_revision` scopes by `profile_id`). Right side defaults to `toml::to_string_pretty` of `store.load` (the flattened effective profile, not the raw file bytes). `mode=semantic` else unified (`ConfigDiffMode::parse`, lines 37-44).

- Unified: `compute_unified_diff` (`profile/config_diff.rs:24-76`). Identical text → empty diff. Each side truncated to `DIFF_MAX_LINES` (2000) with `truncated=true`. Context radius 3.
- Byte cap: `cap_diff_output_bytes` (`config_diff.rs:79-88`), `MAX_DIFF_OUTPUT_BYTES` = 512 KiB, cut on a char boundary, sets `truncated`.
- Semantic: `compute_semantic_diff` (`profile/config_semantic_diff.rs:41-80`). Parse failure → `semantic_parse_failed`, `semantic_changes=None`, response `mode` forced back to `Unified`, and the unified text is still returned. More than `MAX_SEMANTIC_CHANGES` (500) sets `truncated` and drops the tail. `truncated` on the response is the OR of the byte cap and the semantic cap (line 244).

Rollback (`config_history.rs:260-338`): metadata required. Positive id. Profile must have a metadata row. Revision must belong to that profile. `verify_config_revision` (`metadata/config_history_ops.rs:47-64`) recomputes SHA-256 and checks HMAC (ADR-0005); failure aborts **before** `store.save`. Snapshot TOML parses as `GameProfile` or the command errors (disk unchanged). Then save, observe `AppWrite`, revision `RollbackApply` with `source_revision_id=Some(revision_id)`. Retention cap is read inline (`config_history_max_revisions_from_settings`) rather than via `resolve_config_history_max_revisions`; same fallback (`MAX_CONFIG_REVISIONS_PER_PROFILE` = 20) if settings fail to load. Emit `"rollback"`. `new_revision_id` is `None` when deduped, skipped, or capture failed. Frontend `useProfileHistory.ts:82-107` reloads the profile itself after success; the event is what makes the other four listeners refresh.

Known-good (`config_history.rs:343-374`): metadata required. Positive id. `set_known_good_revision` clears the flag on every revision of that profile, then sets it on one row (`config_history_store/mod.rs:248-291`). A missing row comes back as `MetadataStoreError::Corrupt` and is remapped to `"revision {id} not found or does not belong to profile '{name}'"`. No TOML write, no event. Automatic known-good on a clean launch is a different call site (`launch/streaming.rs:416`) and must keep working.

### `protondb_accept_suggestion` (`protondb.rs:108-234`)

`AcceptSuggestionRequest::Catalog`: trim `profile_name` and `catalog_entry_id` (both required). Entry must exist in `global_catalog`. **Every** env pair is re-checked with `validate_env_suggestion` (`protondb/suggestions.rs:107`) before load; any unsafe pair rejects the whole accept (`"catalog entry '{id}' contains unsafe env pair: …"`). If the id is already in `enabled_option_ids`, the profile is still saved and a revision is still captured, but `toggled_option_ids` is empty. New id is appended, not deduped against other copies beyond that contains-check.

`AcceptSuggestionRequest::EnvVar`: trim name, re-validate key and value at write time (the comment at line 196 says cached suggestion data is not trusted). Insert into `custom_env_vars` (last write wins for an existing key). `toggled_option_ids` is empty; `applied_keys` is the one key.

Neither branch emits. `protondb_dismiss_suggestion` (`protondb.rs:237-267`) only writes the dismissal table (30-day retention). It loads the profile to prove it exists, does not modify TOML, and does not emit. Leave it out of the writer unless a helper move forces it.

### Proton migration (`migration.rs` + `profile/migration/apply.rs`)

Single (`migration.rs:63-147`): `Path::try_exists` on `new_path` immediately before write (TOCTOU). `false` or IO error fails the command with the **unsanitized** path in the error string. `apply_single_migration` then:

- Load failure → `Failed` result, not a command error.
- `new_path` must be a regular file (`is_file`), else `Failed`.
- If `old_path` still `try_exists` → `AlreadyValid`, no write.
- Sets `steam.proton_path` or `runtime.proton_path` only.
- Serializes `storage_profile()` and `write_atomic`. No MangoHud companion update. No revision.

Only `Applied` runs observe + health upsert. Health: `lookup_profile_id` fail-open, then `check_profile_health` and `upsert_health_snapshot(profile_id, status, issue_count, checked_at)` (`health_store.rs:13-33`, `INSERT OR REPLACE`). Upsert failure is a warning. This **replaces** the cached row with a fresh check; it does not delete the row. Returned `old_path` / `new_path` go through `sanitize_display_path` (`commands/shared.rs:6-8`).

Batch (`migration.rs:157-265`): every `new_path` must exist and every profile must `load` **before any write**. Any preflight miss returns `Err` and writes nothing. After that, items are independent (`Applied` / `AlreadyValid` / `Failed` counts). No event. The UI compensates: `useProtonMigration.ts:48-53` calls `revalidateSingle` after an applied single migration; lines 69-78 call `batchValidate` when `applied_count > 0`. A new `Migrated` event will make `useProfileHealth` batch-validate **as well as** those direct calls.

## Revision, dedupe, and `MetadataStore::disabled()`

`capture_config_revision` (`shared.rs:112-172`):

1. `!is_available()` → `None` immediately. **No log.** This does not meet the issue’s “warning is logged and surfaced”; that behavior has to be added.
2. `toml::to_string_pretty(profile)` — the in-memory value passed in, which for `profile_save` is the post-import struct **before** `storage_profile()` splits machine paths into `local_override`. Disk bytes and the revision snapshot are not the same string. Golden tests have to freeze each, not assume they match.
3. `sha256_hex` of that snapshot.
4. `lookup_profile_id`; `Ok(None)` or error → warn and skip. Observe must have run first or the id is missing and the revision is skipped (the usual race if observe failed).
5. `insert_config_revision`. Dedupe (`config_history_store/mod.rs:46-64`): skip when the **latest** row for that `profile_id` has the same `content_hash`. Returns `Ok(None)`. A hash that matches an older, non-latest row **is inserted**. `source_revision_id`, if set, must be a row of the same profile or the insert returns `Validation` and capture turns that into `None` + warn.
6. Prune keeps the newest `max_revisions` plus any row still referenced as `source_revision_id` (lineage parents survive the cap). `max_revisions` comes from settings `config_history.max_revisions`, clamped to 5..=100 (`settings/types.rs:40-78`). Settings load failure → 20 (`MAX_CONFIG_REVISIONS_PER_PROFILE`).
7. Snapshot longer than `MAX_SNAPSHOT_TOML_BYTES` (256 KiB, `models.rs:417`) is `Validation` → warn + `None`. The TOML save has already succeeded.

`MetadataStore::disabled()` (`metadata/store.rs:68-78`) has `available=false`. `with_conn` / `with_conn_mut` return `Ok(T::default())` (`store.rs:216-247`):

| Call | Disabled result | User-visible effect today |
| --- | --- | --- |
| `observe_profile_*` | `Ok(())` | save still succeeds; no profiles row |
| `lookup_profile_id` | `Ok(None)` | revision capture warns “profile_id not found” (only if capture didn’t return at step 1 first — it does, so this warn is skipped) |
| `insert_config_revision` | `Ok(None)` | no row |
| `set_profile_favorite` / `list_favorite_profiles` | `Ok(())` / `Ok(vec![])` | favorite command **succeeds and emits**, but the flag is not stored; list is empty |
| `set_known_good_revision` | `Ok(())` | command succeeds without marking anything — **except** the command checks `is_available()` first and returns an error (`config_history.rs:348-351`) |
| `upsert_health_snapshot` | `Ok(())` | migration’s lookup returns `None`, so the upsert is not even called |
| history / diff / rollback | command-level `is_available()` check | hard error, no write |
| bundled preset apply | hard error | no TOML write |
| manual preset | TOML write proceeds; metadata upsert skipped | preset exists only in the profile file |

Saves must keep succeeding when the store is disabled. History, diff, rollback, mark-known-good, and bundled-preset apply must keep failing closed.

## `profiles-changed` today

Producer (`shared.rs:68-72`): payload is a JSON string. Emit failure is a warning; the command still returns success.

| Literal | Producer |
| --- | --- |
| `"saved"` | `profile_save` only |
| `"deleted"` | `profile_delete` |
| `"imported-legacy"` | `profile_import_legacy` |
| `"rollback"` | `profile_config_rollback` |
| `"bundled-optimization-preset"` | `profile_apply_bundled_optimization_preset` |
| `"manual-optimization-preset"` | `profile_save_manual_optimization_preset` |
| `"favorite-updated"` | `profile_set_favorite` |
| `"lutris-import"` | `lutris_import_profiles` when `imported_count > 0` |

Not emitted, despite a TOML or metadata change: duplicate, rename, all four section/optimization/command-arg saves, ProtonDB accept, both migration commands, community import, mark-known-good. Mocks **do** emit for the silent profile commands (`profile-mutations.ts`, `profile-presets.ts`, `profile-history.ts`) with `{ name, action }`. Real Tauri and the mock bus disagree; listeners ignore the body so browser dev still refreshes.

`src/lib/events.ts:7-17` wraps browser payloads as `{ event, id: 0, payload }`. `emitMockEvent` (`events.ts:21-27`) is a no-op inside Tauri.

Target object (issue, not implemented): `AppEvent::ProfilesChanged { reason, names }` bridged to a `profiles-changed` **object**. Reasons: `Saved`, `Deleted`, `Renamed { from }`, `Duplicated`, `FavoriteToggled`, `RolledBack`, `PresetApplied`, `Imported`, `Migrated`. Wire shape is proposed in `docs/plans/profile-writer/research-recommendations.md` (internally tagged `reason.kind`); this inventory does not pick a serde representation.

Suggested `names` from current identity rules:

| Reason | Names |
| --- | --- |
| `Saved` | the saved profile name (trimmed where the command already trims) |
| `Deleted` | the deleted name |
| `Renamed { from }` | `from` = old name; `names` = new name (`profile_rename` returns no event today, so this is new) |
| `Duplicated` | the generated copy name, not the source |
| `FavoriteToggled` | the toggled name |
| `RolledBack` | the restored profile name |
| `PresetApplied` | the profile name (bundled and manual share one reason; the old strings differed) |
| `Imported` | legacy stem, each Lutris imported name, and [INFERENCE] a community import if that path starts emitting |
| `Migrated` | applied profile names only; `AlreadyValid` / `Failed` / preflight `Err` emit nothing |

## Five React listeners

All ignore the payload. All treat any emission, including a favorite toggle, as “rescan”.

| # | Hook | Subscribe | On event | Calls `batch_validate_profiles`? | Mounted from |
| --- | --- | --- | --- | --- | --- |
| 1 | `src/hooks/useProfileHealth.ts:238-259` | line 241, `subscribeEvent<string>` | line 243 `void batchValidate()` → `callCommand('batch_validate_profiles')` (`useProfileHealth.ts:117-131`) | **yes, every reason** | `ProfileHealthProvider` (`context/ProfileHealthContext.tsx:16`), so it is global. Also revalidates on `launch-complete` (line 245) and `version-scan-complete` (line 249); those stay. |
| 2 | `src/hooks/useProfile.ts:232-252` | line 234 | `refreshProfiles()` (`useProfileCrud.ts:196`, invokes `profile_list`, and clears the open profile if the list is empty unless a selection is in flight) and `loadFavorites()` (`useProfileCrud.ts:96`, invokes `profile_list_favorites`) | no | `ProfileProvider` (`context/ProfileContext.tsx:33`) |
| 3 | `src/hooks/useProfileSummaries.ts:38-53` | line 40 | line 44 `fetchSummaries()` → `profile_list_summaries` with the current `collectionId` | no | `GameDetail.tsx:52` |
| 4 | `src/hooks/useLibrarySummaries.ts:63-77` | line 65 | line 69 `fetchSummaries()` → `profile_list_summaries`, then stamps `isFavorite` from a ref | no | `LibraryPage.tsx:70`, `CollectionViewModal.tsx:51`. A separate effect at lines 82-86 updates favorite flags from the `favoriteProfiles` prop with **no IPC**. |
| 5 | `src/hooks/useCommunityProfiles.ts:432-455` | line 434 | line 439 `refreshImportedProfileNames()` (`useCommunityProfiles.ts:248-251`) → `profile_list`, stored as a `Set` of sanitized names. It does **not** refetch the community index. | no | `CommunityPage.tsx:8`, `CompatibilityPage.tsx:34`, `CommunityBrowser.tsx:64` |

`batch_validate_profiles` (`commands/health/batch.rs:6-10`) delegates to `HealthService::batch_summary` (`services/health/mod.rs:40-44`), which `spawn_blocking`s `batch::build_enriched_health_summary`. That walk **writes** `health_snapshots` for every profile it can resolve (`services/health/batch.rs:96-114`). So a favorite toggle today causes a full health recompute and snapshot rewrite, on the main-thread command path from the listener’s point of view (the command itself is async and the work is on the blocking pool).

Health checks that a content change can affect (`profile/health/profile.rs:31-121`): `game.executable_path`, non-empty `trainer.path`, enabled `injection.loaded_hooks`, `injection.dll_paths`, and for non-native methods `steam.compatdata_path` + `steam.proton_path` or `runtime.prefix_path` + `runtime.proton_path`, plus optional `steam.launcher.icon_path` and `runtime.working_directory`. Favorite, MangoHud, gamescope, trainer-gamescope, optimization ids, and command-arg tokens are not in that list. The issue only **requires** skipping `batch_validate_profiles` for reasons that cannot affect health, with `FavoriteToggled` as the example. Skipping more than `FavoriteToggled` is a product choice; emitting `Saved` for the currently silent section saves would **add** health batches that do not happen today.

Tests that pin the current (empty-string) payload and will break when the payload becomes an object:

- `src/hooks/__tests__/useProfileSummaries.test.ts:51-55` emits `payload: ''` and expects a refetch (`:81`).
- `src/components/pages/__tests__/LibraryPage.test.tsx:449-479` emits `''`.
- `src/hooks/__tests__/useProfileHealth.startup.test.ts:60-62` already counts `batch_validate_profiles` calls, but only around startup. Nothing asserts that `profiles-changed` triggers or skips a batch. That counter is the pattern the acceptance test should copy.

Mock emitters to update with the real object (they currently invent `action` strings the backend never sends):

- `src/lib/mocks/handlers/profile-mutations.ts:29, 61, 91, 111, 131, 151, 173, 206, 251, 267, 304, 321, 345`
- `src/lib/mocks/handlers/profile-presets.ts:72, 107`
- `src/lib/mocks/handlers/profile-history.ts:104` (`action: 'rollback'`)
- `src/lib/mocks/handlers/proton.ts` migration handlers do not emit (matches Tauri today).
- `protondb.ts` accept handler does not emit (matches Tauri today).

## Business Rules

Core rules to preserve follow.

### Business rules to preserve

**Rename launcher cleanup and re-export.** Cleanup runs on the old name before the display-name rewrite, using the pre-rename profile. Return value is “files were actually deleted”, not “a launcher was configured”. Native profiles never delete launcher files. Cleanup failure must not fail the rename. The bool is the only signal the UI has; dropping it drops auto re-export (`useLauncherExport.ts:96-114`). Re-export uses `export_launchers`, which observes launcher export metadata (`commands/export.rs` `reexport_launcher_by_slug` is a different command and is not what rename uses).

**Delete launcher cleanup.** Same helper, but the result is discarded. Delete still succeeds when cleanup fails. Native skip applies. Metadata delete is a soft delete.

**Duplicate naming.** See the `(Copy)` / `(Copy N)` / 1000-cap rules above. The copy is a byte-level `GameProfile` clone saved through `storage_profile()`. No launcher files are created for the copy. No revision. Metadata row uses `AppDuplicate` and may point at `source_profile_id`.

**Rollback lineage.** Restore only after `verify_config_revision`. New row source `rollback_apply` with `source_revision_id` pointing at the restored id. Dedupe can suppress that row when the live snapshot hash already equals the latest hash (rolling back to the current contents). Parents referenced by `source_revision_id` are exempt from pruning.

**Known-good.** One active flag per profile. Manual mark does not emit and does not write TOML. Launch success stamps the latest revision, fail-soft, and is outside this command set — the writer must not break `set_known_good_revision`.

**Revision SHA-256 dedupe.** Latest-row only, per `profile_id`, on the pretty-TOML snapshot hash. Not a global content-addressed store.

**Diff caps.** 2000 lines per side, 512 KiB UTF-8-safe output, 500 semantic changes, semantic parse failure falls back to unified and reports `semantic_parse_failed`.

**Disabled metadata.** TOML save/delete/rename/duplicate/section saves succeed. Revisions are skipped. Today that skip is silent inside `capture_config_revision`; the issue requires a warning that is logged **and surfaced**. History, diff, rollback, mark-known-good, and bundled apply stay hard errors. Favorite currently reports success while writing nothing.

**New-profile defaults.** Only when the TOML does not exist. Settings load failure fails the create. Defaults never clobber a non-empty proton path, launch method, non-default trainer mode, or an already chosen `active_preset`.

**Custom-art auto-import.** Only `profile_save`. Three independent paths. Failure keeps the user path. Already-managed paths are not copied again.

**Env suggestion revalidation.** ProtonDB accept re-runs `validate_env_suggestion` on catalog pairs and on the raw env pair at write time. Dismiss does not.

**Health.** No general “invalidate” function exists. Migration recomputes one snapshot. Everyone else relies on the next `batch_validate_profiles`, which upserts every profile it checks. Rename/delete do not remove `health_snapshots` rows; delete hides them because the loader joins `profiles` where `deleted_at IS NULL` (`health_store.rs:43-45`). The stable `profile_id` survives rename, so the old snapshot remains visible under the new filename until the next batch.

**Collections.** Not this issue. `profile_load` / summaries still merge collection defaults after load, with corrupt JSON failing the read and other metadata errors failing open (`shared.rs:200-239`).

**Concurrency.** Section helpers document last-completed-write-wins. No profile write lock exists. The writer should not invent one.

**Byte compatibility.** `save` writes `storage_profile()` pretty-TOML. Rollback parses the revision snapshot and saves it back through the same path, so machine paths re-split into `local_override`. Golden files have to be captured from the pre-refactor `save` / rename display-name rewrite / `duplicate` / rollback, not from `toml::to_string_pretty` of the effective profile.

## Edge cases

- Same-name `profile_rename` is `Ok` inside the store and still runs observe (which will not find a filename change), cleanup, display-name save, and the settings compare. It returns `had_launcher` and emits nothing.
- Rename when the metadata row is missing: TOML is renamed, observe warns `Corrupt`, command returns `Ok(had_launcher)`. Later revision capture cannot find a profile id.
- Rename display-name `save` can fail after the file was already renamed. The command still returns `Ok`. The settings `last_used_profile` update is independent and also best-effort.
- Duplicate of a missing source is `NotFound`. Duplicate does not apply creation defaults or art import.
- `profile_save` of an existing name is an update: no creation defaults, extras preserved. `profile_save` of a new name requires settings.
- `profile_save` art import that fails leaves the external path in the TOML that then gets revisioned.
- Section save of a missing profile fails before any write. Optimizations and command-args do the same inside the store.
- Launch-optimization and command-arg post-write `load` failure skips observe and revision after the TOML write has succeeded. The command still returns `Ok`.
- Bundled apply with metadata up but `lookup_profile_id == None` still writes TOML, observes, captures a revision only if observe created the row in time, and skips the preset-metadata upsert.
- Rollback of a snapshot whose hash equals the latest revision restores the file and emits `"rollback"` but `new_revision_id` is `None`.
- Rollback HMAC / hash failure does not write the file.
- Diff against a foreign revision id returns not-found even if that id exists for another profile.
- `profile_config_history` on a profile that exists on disk but has no metadata row returns `[]`.
- Favorite of an unknown name updates zero rows and still emits `"favorite-updated"` (`collections.rs:167-185` ignores the affected-row count).
- Disabled store: favorite emits and returns `Ok(())` without a row. Listeners still batch-validate.
- Migration `AlreadyValid` and `Failed` results do not observe and do not upsert health. Batch preflight failure writes nothing and returns `Err` (no per-item results).
- Migration error strings include the raw replacement path; success payloads are sanitized. The writer should not “fix” that split unless a test freezes it.
- `apply_single_migration` bypasses `ProfileStore::save`, so a writer that routes migration through `save` would start rewriting MangoHud companion files. That is a behavior change.
- Community import observes but does not emit, so library/health do not refresh until something else emits or the user navigates. [INFERENCE] leaving it unwired keeps today’s staleness; pointing it at `Imported` changes the UI.
- Lutris emits one event for the whole batch, with no names. Listeners rescan everything anyway.
- `useLibrarySummaries` and `useProfile` both react to a favorite event **and** the favorite command’s own `loadFavorites()` follow-up (`useProfileCrud.ts:108-109`), so one toggle already double-fetches favorites. `useLibrarySummaries` additionally refetches all summaries even though lines 82-86 would have updated the star from props.
- Mock `profile_rename` returns `false` always (`profile-mutations.ts:252-253`), so browser dev never exercises the re-export banner.
- `startup_bridge.rs:131` will swallow `AppEvent::ProfilesChanged` until a bridge arm is added. Publishing on the bus without that arm emits nothing to the webview.

## Success criteria (issue) and where the current behavior lives

| Criterion | Preserve | Evidence to pin tests to |
| --- | --- | --- |
| Golden TOML for save, rename, duplicate, rollback | `ProfileStore::save` / `storage_profile` bytes, including the rename display-name second save and rollback’s re-save. Not the revision snapshot string. | `toml_store/store.rs:95-122`, `lifecycle.rs:293-334`, `config_history.rs:297-299`. No goldens exist yet. |
| SHA-256 dedupe, diff cap, semantic diff | Latest-hash skip; 2000 / 512 KiB / 500; semantic fallback. | `config_history_store/mod.rs:46-64`, `profile/config_diff.rs`, `profile/config_semantic_diff.rs`, command mapping `config_history.rs:195-249`. Core unit tests already cover the algorithms; the service tests have to prove the command still calls them. |
| Favorite toggle does not call `batch_validate_profiles` | No TOML, no revision, no health upsert on the write. Listener must branch on `FavoriteToggled`. | `favorites.rs:7-18`. No such test today. Pattern: `useProfileHealth.startup.test.ts:60-62`. |
| Rename and delete keep launcher cleanup and re-export | Native skip, best-effort cleanup, `bool` return, UI timer + `export_launchers`. | `shared.rs:44-65`, `lifecycle.rs:229-251` and `313-325`, `useProfileNotifications.ts:145-172`, `useLauncherExport.ts:96-114`. Shell tests only cover the helper: `commands/profile/tests/lifecycle.rs:35-68`. |
| Tauri commands delegate; React smoke passes | Command names and serde shapes stay (`oldName`/`newName`, `revisionId`, `presetId`, `collectionId`, camelCase `ProfileSummary`). | `profile/mod.rs` re-exports, `tests/smoke.spec.ts`. |

## Relevant files

- `src-tauri/src/commands/profile/lifecycle.rs`: save, delete, duplicate, rename, legacy import, export, list/load/summaries.
- `src-tauri/src/commands/profile/shared.rs`: emit, capture, section-save helper, launcher cleanup, collection merge.
- `src-tauri/src/commands/profile/optimizations.rs`: optimization, overlay, and preset writes.
- `src-tauri/src/commands/profile/command_arguments.rs`: command-arg write.
- `src-tauri/src/commands/profile/config_history.rs`: history, diff, rollback, known-good.
- `src-tauri/src/commands/profile/favorites.rs`: favorite flag and list.
- `src-tauri/src/commands/protondb.rs`: accept (write) and dismiss (not a profile write).
- `src-tauri/src/commands/migration.rs` and `crates/crosshook-core/src/profile/migration/apply.rs`: Proton path apply, the only inline health upsert.
- `src-tauri/src/commands/lutris.rs`, `src-tauri/src/commands/community.rs`: extra observe/emit callers.
- `src-tauri/src/commands/launch/streaming.rs:380-418`: automatic known-good.
- `src-tauri/src/startup_bridge.rs:121-132`: event bridge that must grow an arm.
- `crates/crosshook-app/src/events.rs`, `services/mod.rs`, `services/health/`, `bootstrap/stores.rs`: service and bus patterns (`HealthService` is the template: async API, `run_blocking`, clone of stores).
- `crates/crosshook-core/src/profile/toml_store/store.rs`: durable writes and copy-name rules.
- `crates/crosshook-core/src/profile/creation_defaults.rs`: new-profile defaults.
- `crates/crosshook-core/src/metadata/profile_sync.rs`, `config_history_store/mod.rs`, `config_history_ops.rs`, `store.rs:68-83` and `216-247`: observe, dedupe, disabled store.
- `crates/crosshook-core/src/profile/config_diff.rs`, `config_semantic_diff.rs`: diff caps.
- `crates/crosshook-core/src/profile/health/profile.rs`: which fields a health batch actually reads.
- `src/hooks/useProfileHealth.ts`, `useProfile.ts`, `useProfileSummaries.ts`, `useLibrarySummaries.ts`, `useCommunityProfiles.ts`: the five listeners.
- `src/hooks/profile/useProfileCrud.ts`, `useProfileNotifications.ts`, `useProfileHistory.ts`, `useLauncherExport.ts`, `useProtonMigration.ts`: direct follow-up calls that do not go through the event.
- `src/lib/events.ts` and `src/lib/mocks/handlers/profile-mutations.ts`, `profile-presets.ts`, `profile-history.ts`: mock/event mismatch.
