# YAN-847 (M1-35) — Business-Logic Research: invariants, tests, gaps

Scope: behaviour that must survive the move of the storage, mods, collections, launcher-export, version, steam and game-metadata Tauri commands into `crosshook-app` services.
All paths are relative to `src/crosshook-native/` unless noted. `src-tauri/` is abbreviated `tauri/`; `crates/crosshook-core/src` is abbreviated `core/`.
Read-only research; no repo files were touched other than this report.

---

## 0. Cross-cutting facts that drive the design

### 0.1 Where "DB unavailable" is handled today (three different behaviours)

`core/metadata/store.rs` defines three access paths with different disabled-store semantics:

| Access path        | Disabled/unavailable store behaviour                                                                                     | Source             |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------ | ------------------ |
| `with_conn`        | **Silently returns `Ok(T::default())`** (`Vec` → empty, `Option` → `None`, `String` → `""`, `()` → `Ok`)                 | `store.rs:172-191` |
| `with_conn_mut`    | Same silent `Ok(default)` when `!available` or no conn. Returns `Err(readonly_error)` only when the store is `read_only` | `store.rs:193-222` |
| `with_sqlite_conn` | **Errors** with `ReadOnlyDisabled { reason }` (`readonly_error`)                                                         | `store.rs:225-246` |

Consequences per domain:

- **Collections** use `with_conn` (`core/metadata/collections_ops.rs:6-115`). With a disabled store:
  - `list_collections` → `Ok([])`.
  - `create_collection` → **`Ok("")`** (an empty id returned as success).
  - `delete`, `add_profile`, `remove_profile`, `rename`, `update_description` and `set_collection_defaults` → `Ok(())` (silent no-op).
  - `get_collection_defaults` → `Ok(None)`.
  - `collections_for_profile` → `Ok([])`.
  - The issue says "collections ... report `Unavailable`, no partial writes". The service **must add an explicit `is_available()` pre-check**; relying on the store is not enough. Today's `tauri/commands/collections.rs` has no guard at all.
- **Mods** use `with_sqlite_conn` (`core/metadata/mods_ops.rs:11-58`), so they error on a disabled store. `core/metadata/mods_tests.rs:333-340` (`disabled_store_errors_loudly`) pins this. The Tauri layer still pre-guards with `is_available()`:
  - `tauri/commands/mods.rs:63-68` returns `{available:false, mods:[]}` for list.
  - `mods.rs:111-113, 138-140, 164-166, 243-245` return `Err("metadata_unavailable: the metadata database could not be opened")` for add, update, remove and analyze.
- **Version** uses `with_conn`/`with_conn_mut` (`core/metadata/version_ops.rs`). `core/metadata/version_store_tests.rs:235-246` (`test_version_store_disabled_store_noop`) pins the silent no-op for `upsert`, `lookup`, `load_all` and `acknowledge`. In `tauri/commands/version.rs`:
  - `lookup_profile_id` → `Ok(None)`.
  - `check_version_status` and `get_version_snapshot` therefore degrade to `profile_id:""` / `snapshot:None` / status `Untracked` (or manifest-derived) with no error.
  - `set_trainer_version` and `acknowledge_version_change` return `Err("profile '<n>' is not registered in the metadata store")` (`version.rs:459, 493`). This message is misleading for a disabled store.
- **Storage audit and snapshots** use `with_conn_mut` (`core/metadata/prefix_ops.rs:115-140`), so inserts silently no-op on a disabled store. Failures are only `tracing::warn!`ed (`tauri/commands/storage.rs:45-47, 103-105, 119-121, 136-138`). History uses `with_conn`, so it returns empty lists, and the response carries `available: bool` (`storage.rs:147-175`).
- **Launcher observe\_\*** use `with_conn`/`with_conn_mut` (`core/metadata/launcher_ops.rs`). The Tauri commands treat metadata sync as fail-soft (warn only) in `export.rs:201-209, 259-262, 281-284, 339-347, 421-429`.
- **Game metadata / cover art**:
  - `lookup_steam_metadata` gates cache reads on `is_available()` (`core/steam_metadata/client.rs:230-232`).
  - Cache writes use `put_cache_entry` → `with_conn` (silent no-op).
  - The image cache uses `get_game_image`/`upsert_game_image` → `with_conn`.

**Existing `AppError` precedent** (`crates/crosshook-app/src/error.rs`):

- `AppErrorKind` is `Validation{field} | NotFound | Conflict | Unavailable | Io | Cancelled | Internal` (`error.rs:7-25`).
- `From<MetadataStoreError>` maps `NewerSchema | ReadOnlyNewerSchema | SQLiteReadonly | ReadOnlyDisabled` → `Unavailable` ("metadata database is unavailable"), `Validation(_)` → `Validation{field:"metadata"}` ("metadata validation failed"), `Corrupt` → `Internal`, `Database` → `Internal`, `Io` → `Io` (`error.rs:206-239`).
- The profile services use a local helper `require_metadata(&MetadataStore, action) -> Result<(), AppError>` that returns `Unavailable` (`services/profiles/history.rs:293-302`, `presets.rs:111-120`). Decision D6 in `docs/plans/profile-writer/decisions.md:12` says metadata-required operations return explicit `Unavailable`. Collections and mods should reuse this pattern; consider lifting the helper to `services/mod.rs` rather than copying it a third time.
- **Message loss risk**: `From<MetadataStoreError>` flattens `Validation(msg)` to the fixed string "metadata validation failed" (`error.rs:214-217`). Several user-facing messages are relied on (see §0.2). The service must build `AppError::validation(...)` itself for user-correctable errors, or the error mapping needs a variant that keeps the (non-path) message. `sanitize_message` (`presenter/mod.rs:31`) strips home paths.

### 0.2 What the frontend expects from errors (IPC strings, `Result<_, String>` today)

The IPC boundary is `String` errors. Existing shells convert `AppError` via `error.message().to_string()` (`tauri/commands/settings.rs:15-17`). Frontend contracts to preserve:

- **`metadata_unavailable:` prefix** on mod commands.
  - `src/hooks/useProfileMods.ts:8` defines `METADATA_UNAVAILABLE_PREFIX`.
  - The hook handles it in `refresh` (`:115-140`), `runAdvisories` (`:70-98`) and every mutation (`:165-205`). On the prefix it sets `unavailable` and falls back to the "Mod registry is unavailable" message (`:11`).
  - `list_profile_mods` instead returns `{available:false}` rather than an error (`:119`).
  - Mock handler: `src/lib/mocks/handlers/mods.ts:4`.
- **`no_game_path:` prefix** from `detect_profile_mods` (`tauri/commands/mods.rs:22, 203-209`). Frontend: `useProfileMods.ts:9`, test `src/hooks/__tests__/useProfileMods.test.ts:257-260`.
- **Mod error texts** (the mock mirrors the real strings, `src/lib/mocks/handlers/mods.ts:98,178,196,219`):
  - `"metadata validation error: mod name is required"`.
  - `"metadata validation error: a mod named “X” is already registered for this profile"` (`core/metadata/mods_store.rs:330-332`). `ModDetectionReviewModal` renders "already registered for this profile".
  - `"mod not found"` (`mods_store.rs:189,193`, and `mods.rs:181` for remove).
  - `"profile has no metadata record yet — save the profile once, then retry"` (`mods.rs:38`).
- **Collections**: `CollectionsContext.tsx` surfaces `err.message` verbatim (`:68-74, 83-110, 235-310`).
  - Import flow: `collection_create` → `collection_update_description` → `collection_set_defaults` → `collection_add_profile`×N, with `collection_delete` rollback on failure.
  - It relies on `collection_create` returning the **new id** (a disabled-store `Ok("")` would corrupt this flow).
  - It relies on `rename` returning an error for a duplicate name (UNIQUE) and for an unknown id.
  - `useCollectionDefaults.ts:56,82` and `useCollectionMembers.ts:42` use `callCommand` directly.
- **Metadata fetch** (`src/hooks/useGameMetadata.ts:96-131`): any thrown error becomes `unavailableLookup` (`state:'unavailable'`). A successful result is normalized from `state: idle|loading|ready|stale|unavailable`, plus `from_cache`/`is_stale`. `isStale` is `state==='stale' || is_stale`.
- **Cover art** (`src/hooks/useGameCoverArt.ts:41-61`): `Option<String>` path → `convertFileSrc`; `null` or a thrown error → `coverArtUrl=null`. It logs `console.error` on a throw and has no "unavailable" state today.
- **Storage** (`src/hooks/usePrefixStorageManagement.ts`):
  - Scan error → `error` string, rethrown.
  - Cleanup returns `PrefixCleanupResult{deleted,skipped,reclaimed_bytes}` and refreshes history.
  - A `get_prefix_storage_history` throw → `persistenceAvailable=false`; `response.available` is also honoured.
  - TS type `src/types/prefix-storage.ts` (`available: boolean` at `:68`).
- **Launchers**: `useLauncherManagement.ts` (list/delete-by-slug/reexport-by-slug) and `useLauncherExport.ts` (export/preview/check/delete). The frontend never calls `rename_launcher` or `find_orphaned_launchers` outside mocks (`src/lib/mocks/handlers/launcher.ts:101,121`), but both are registered in `tauri/src/lib.rs:73,76`. They must stay on the IPC surface, because the issue lists them.
- **Version**: `useVersionCheck.ts:9` swallows errors (`.catch(()=>{})`); `useSetTrainerVersion.ts:29`, `useAcknowledgeVersionChange.ts:47`, `useOnboarding.ts:173`.

---

## 1. `apply_collection_defaults` merge semantics (issue criterion 2)

### 1.1 Locations

- Shell helper: `tauri/commands/profile/shared.rs:35-48` (`apply_collection_defaults`) and `:54-81` (`resolve_collection_defaults`). Both are `pub(super)`. The file header says "kept in the shell until M1-35".
- Callers:
  - `tauri/commands/profile/lifecycle.rs:17-40` (`profile_load`, runs `apply_collection_defaults` in `spawn_blocking`, then `profile.clear_extra()`).
  - `lifecycle.rs:42-56` (`profile_list_summaries`, passes `resolve_collection_defaults` as the resolver closure to `ProfileWriter::list_summaries`).
  - The resolver type is `FnOnce() -> Result<Option<CollectionDefaultsSection>, AppError>` (`crates/crosshook-app/src/services/profiles/reads.rs:48-60`).
  - `build_summaries` calls the resolver at most once, only after the first profile loads (`reads.rs:~97-110`), so an empty library never surfaces a collection error. **Preserve this ordering.**
- The actual merge: `core/profile/models/profile.rs:171-~212` (`GameProfile::effective_profile_with`); `effective_profile()` is the `None` shim at `:121-126`.
- Prior decisions: D7 in `docs/plans/profile-writer/decisions.md:13` and `feature-spec.md:74` ("collections stay out... move in M1-35"). The profile-writer plan explicitly says to keep the helper and its tests in the shell until now.

### 1.2 Resolution policy (`resolve_collection_defaults`, `shared.rs:54-81`)

| Input / store result                                                          | Result                                                                                                 |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `collection_id` `None` or trims to empty                                      | `Ok(None)` (identity, no lookup)                                                                       |
| `get_collection_defaults` → `Ok(None)` (NULL/blank `defaults_json`)           | `Ok(None)` → profile returned unchanged                                                                |
| → `Ok(Some(d))`                                                               | `Ok(Some(d))` → `profile.effective_profile_with(Some(&d))`                                             |
| → `Err(Corrupt(_))`                                                           | **Error bubbles** (`tracing::warn!`, then `Err(Corrupt)`); surfaces loudly in `useProfile.loadProfile` |
| → any other `Err` (`Validation` = unknown collection id, `Database`, `Io`, …) | `tracing::warn!` and **fail open**: `Ok(None)`, raw profile                                            |
| disabled store                                                                | `with_conn` returns `Ok(None)` → identity (fail open)                                                  |

The id is trimmed before lookup (`shared.rs:58`).

In the app-service version, `Corrupt` currently converts to `AppErrorKind::Internal` ("metadata database is corrupt") through `From<MetadataStoreError>`, which **loses the "collection defaults JSON is corrupt" message**. The IPC contract is "loud failure"; the text can change, but it must stay an error.

### 1.3 Field merge table (`effective_profile_with`, `profile.rs:171-~212`)

Applied on the **already-loaded** profile. `ProfileStore::load` has baked `local_override` into layer 1 and cleared it, so layer 3 is a no-op at the `profile_load` call site (documented in the doc comment, `profile.rs:~127-170`).

| `CollectionDefaultsSection` field (`core/profile/models/launch.rs:166-184`) | Rule                                                                                                      | Target                     |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------- |
| `method: Option<String>`                                                    | Replace only when `Some` **and non-blank after trim**. Whitespace-only is ignored.                        | `launch.method`            |
| `optimizations: Option<LaunchOptimizationsSection>`                         | Replace the whole section when `Some`, **keeping the profile's own `.extra`** (`mem::take` then restore). | `launch.optimizations`     |
| `custom_env_vars: BTreeMap`                                                 | **Additive union**; collection key wins on collision; empty map is a no-op. Profile-only keys retained.   | `launch.custom_env_vars`   |
| `network_isolation: Option<bool>`                                           | Replace when `Some` (including `Some(false)` over a `true` profile value).                                | `launch.network_isolation` |
| `gamescope: Option<GamescopeConfig>`                                        | Replace when `Some`, keep profile `.extra`.                                                               | `launch.gamescope`         |
| `trainer_gamescope: Option<GamescopeConfig>`                                | Replace when `Some`, keep profile `.extra`.                                                               | `launch.trainer_gamescope` |
| `mangohud: Option<MangoHudConfig>`                                          | Replace when `Some`, keep profile `.extra`.                                                               | `launch.mangohud`          |
| `extra: toml::Table` (flatten)                                              | **Never merged**: unknown collection keys stay collection-local.                                          | n/a                        |
| excluded by PRD                                                             | `presets`, `active_preset`, `pre_launch_hooks`, `post_exit_hooks` are untouched                           |                            |

Layer 3 (`local_override.*`, `profile.rs:~214-258`) runs after layer 2. It is non-empty-only (trim) for:

- `game.executable_path`, `game.custom_{cover,portrait,background}_art_path`
- `trainer.path`
- `trainer.extra_protontricks` (this one **extends** `required_protontricks`)
- `steam.compatdata_path`, `steam.proton_path`
- `runtime.prefix_path`, `runtime.proton_path`

Other invariants:

- Defaults JSON is stored in `collections.defaults_json`; `set_collection_defaults` writes `NULL` for `None` **or** an `is_empty()` struct (`core/metadata/collections.rs:376-393`).
- `get` returns `Validation("collection not found: …")` for an unknown id (`collections.rs:348-352`), `Corrupt` for bad JSON (`:368-372`), and `Ok(None)` for NULL/blank (`:361-366`).

### 1.4 IPC boundary rules for defaults (`tauri/commands/collections.rs:293-325`)

- `collection_get_defaults`: `get_collection_defaults` then **`clear_extra()`** before returning to the frontend.
- `collection_set_defaults`: incoming `defaults.clear_extra()`, then if existing defaults are stored, **`preserve_extra_from(&existing)`**, then `set_collection_defaults`. `None` clears.
  - This is the read-modify-write extras round trip. Note `existing` lookup uses `?` here, so a `Corrupt` existing row makes `set` **fail** (the user cannot overwrite corrupt defaults via `Some`; they can only clear with `None`, which skips the lookup). The `get_collection_defaults` doc at `collections.rs:328-337` says "user can clear them by saving fresh defaults", which is only true for `None`. Document this when porting, and decide whether to preserve it.
- `collection_export_to_toml` also does `defaults.clear_extra()` and drops empty defaults (`core/profile/collection_exchange/export.rs:33-40`).
- Extras helpers: `core/profile/models/extra.rs:169-180`.

### 1.5 "Used by the M1-06 new-profile flow"

The issue says `apply_collection_defaults` is "used by the M1-06 new-profile flow".

- M1-06 is YAN-811 (`docs/plans/profile-writer/issue.md:1`).
- **There is no new-profile caller today.** Only `profile_load` and `profile_list_summaries` call the helper (`lifecycle.rs:33, 52`). `core/profile/creation_defaults.rs` (`apply_profile_creation_defaults_from_settings`) is a different concept (app settings defaults) and does not touch collections.
- The service API should therefore expose the merge as a reusable service function taking `(GameProfile, Option<&str> collection_id)` and returning `Result<GameProfile, AppError>`, plus the resolver for summaries, so both the existing Tauri callers and a future native new-profile flow can share it. This needs a decision, since the repo has no new-profile caller to port.

### 1.6 Existing tests for the merge

| Test                                                                                | File:line                                                    | Covers                                                                                            |
| ----------------------------------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| `none_collection_id_returns_profile_unchanged`                                      | `tauri/commands/profile/tests/optimizations.rs:24`           | None id                                                                                           |
| `empty_collection_id_returns_profile_unchanged`                                     | `:35`                                                        | whitespace id                                                                                     |
| `valid_defaults_merge_into_profile`                                                 | `:49`                                                        | method replace; env union, collision, profile-only retained                                       |
| `unknown_collection_id_fails_open_with_unmodified_profile`                          | `:86`                                                        | `Validation` → fail open                                                                          |
| `corrupt_defaults_bubble_error_to_caller`                                           | `:101`                                                       | `Corrupt` bubbles (seeded via `with_sqlite_conn`)                                                 |
| `effective_profile_with_none_equals_shim`                                           | `core/profile/models/tests/profile_layers.rs:93`             | `None` shim equality                                                                              |
| `effective_profile_with_merges_collection_defaults_between_base_and_local_override` | `profile_layers.rs:107`                                      | layering, env collision, `network_isolation`, `method`                                            |
| `effective_profile_with_none_fields_do_not_overwrite_profile`                       | `profile_layers.rs:151`                                      | empty defaults no-op                                                                              |
| `effective_profile_with_ignores_whitespace_only_method`                             | `profile_layers.rs:177`                                      | whitespace method                                                                                 |
| `collection_defaults_replace_known_fields_without_leaking_collection_extras`        | `core/profile/models/tests/unknown_fields.rs:~183-205`       | optimizations + gamescope replace keep profile extras; collection extras don't leak               |
| `save_load_then_merge_collection_defaults_preserves_local_override_paths`           | `core/profile/toml_store/tests/load_save.rs:~47-110`         | post-load local_override baked, still wins                                                        |
| `test_collection_defaults_*` (5)                                                    | `core/metadata/collections_defaults_tests.rs:10,40,63,72,93` | roundtrip, clear→NULL (None and empty struct), unknown id on set, corrupt JSON, cascade on delete |
| `end_to_end_collections_jtbd`                                                       | `core/tests/collections_jtbd_integration.rs:51`              | end-to-end                                                                                        |

**Not covered (add as a table-driven merge test in the service):**

- `network_isolation: Some(true)` and `Some(false)` over both profile values.
- `trainer_gamescope` replace.
- `mangohud` replace with extra retained.
- `optimizations` replace on its own.
- A mixed all-fields-at-once row.
- Empty `custom_env_vars` is a no-op.
- `Ok(None)` defaults for an existing collection through the helper (NULL column).
- A non-Validation error (`Database`/`Io`) fails open. Only `Validation` is tested through the helper.
- `resolve_collection_defaults` directly, including the summaries resolver path (the "resolver runs once, only after the first profile loads" behaviour has no collection-specific test).
- Disabled store → identity.
- `collection_get/set_defaults` `clear_extra`/`preserve_extra_from` IPC rules (no Tauri-level test; only core `unknown_fields.rs:~215-270`).

---

## 2. Collections CRUD / membership rules (13 commands, `tauri/commands/collections.rs`)

Command list: `collection_list`, `create`, `delete`, `add_profile`, `remove_profile`, `list_profiles`, `rename`, `update_description`, `collections_for_profile`, `get_defaults`, `set_defaults`, `export_to_toml`, `import_from_toml`. All are synchronous `#[tauri::command] fn`s today, so they block the IPC caller (`map_error` = `to_string`, `collections.rs:5-7`).

Rules to preserve (`core/metadata/collections.rs`):

- **Create**: name trimmed; empty → `Validation("collection name must not be empty")` (`:47-53`). A duplicate name hits the UNIQUE constraint → `Database` error. There is no friendly duplicate message, and the frontend pre-checks duplicates itself (`CollectionsContext.tsx:~225`). Returns the new id (`db::new_id()`).
- **Delete**: unconditional `DELETE`; **unknown id is a silent `Ok`** (`:71-82`). Memberships and defaults cascade (`test_collection_delete_cascades`, `collections_crud_tests.rs:228`).
- **Add profile**: trims the name, resolves via `lookup_profile_id`. Missing → `Validation("profile not found when adding to collection: <name>")` (`:84-94`, test `:53`, which asserts the name is in the message and that no row is inserted). `INSERT OR IGNORE` → **idempotent**. An unknown `collection_id` is not validated explicitly (FK behaviour only).
- **Remove profile**: unknown profile → `Ok(())` silent (`:110-130`).
- **List profiles in collection**: joins `profiles`, excludes soft-deleted (`deleted_at IS NULL`), ordered by `current_filename` (`:132-165`).
- **List collections**: ordered `sort_order ASC, name ASC`; includes a `profile_count` subquery (`:8-45`). Note the count does **not** filter `deleted_at`, whereas membership listing does. Preserve the divergence unless a deliberate fix is approved.
- **Rename**: trimmed; empty → `Validation`; unknown id → `Validation("collection not found: …")`; duplicate → `Database` (UNIQUE) (`:216-246`, tests `collections_crud_tests.rs:82,94,101`).
- **Update description**: trim, empty → `NULL`; unknown id → `Validation` (`:248-275`, test `:115`).
- **Collections for profile**: trimmed name; unknown profile → `Ok([])` (`:277-326`; multi-membership test `:157`).
- **Profile deletion** cascades membership (`test_profile_delete_cascades_collection_membership`, `:189`).
- **Favorites** live in the same module (`set_profile_favorite`, `list_favorite_profiles`, `collections.rs:167-214`). They are already served by `ProfileWriter` (`services/profiles/reads.rs`), not part of this issue.
- **TOML export** (`core/profile/collection_exchange/export.rs`):
  - Command pre-validation: `collection_id.trim()` non-empty and `output_path` non-empty → plain string errors (`collections.rs:334-340`).
  - Unknown collection id → `InvalidManifest("collection not found: …")`.
  - Members are loaded from `ProfileStore`; a missing member file → `InvalidManifest("profile file for collection member … is missing")`.
  - Each descriptor uses `resolve_art_app_id(profile)` (effective app id: `steam.app_id`, then `runtime.steam_app_id`), `game.name` and `trainer.community_trainer_sha256`.
  - Defaults are `clear_extra`'d and `is_empty`-filtered; the description is trimmed and blank → `None`.
  - Output has a fixed 4-line comment header, then `toml::to_string_pretty` (`:93-118`). It creates parent dirs. The write is a plain `fs::write` (not atomic, no symlink guard).
  - **Security note**: `output_path` is attacker/renderer-controlled, so the service API should confirm it comes from a native save dialog or validate it.
- **TOML import preview** (`import.rs`): read-only. Requires non-empty `path`. Parse → `validate()` → classify descriptors against the local profile index into `matched`, `ambiguous` and `unmatched`. Errors: `UnsupportedSchemaVersion`, `Toml`, `InvalidManifest`, `Io`.
  - The import **commit** is entirely a frontend orchestration (`CollectionsContext.tsx:~225-300`: create, description, defaults, add members, rollback by delete). There is no atomic backend import command. The service could fold this into one `import` operation with an atomic transaction, but the issue says only "TOML import/export" and the storage boundary says unchanged. Flag as an open decision.
- Preset/matching tests: `core/profile/collection_exchange/mod.rs` `#[cfg(test)]` (future schema rejected, malformed TOML, export→preview roundtrip, steam ambiguous, tie-break by pair, pair fallback…); `matching.rs`.

**DB disabled today**: see §0.1. All writes silently succeed, and `collection_create` returns `""`. This is a latent bug; the target spec requires `Unavailable` and "no partial writes".

---

## 3. Mods (6 commands, `tauri/commands/mods.rs`)

Header doc: "Error-string contract (frontend keys off prefixes): `metadata_unavailable:` ... `no_game_path:`" (`mods.rs:1-6`).

### 3.1 Profile resolution (`resolve_profile`, `mods.rs:25-48`)

Runs in `spawn_blocking`:

1. `profile_store.load(name)` (profile must exist; error `to_string`).
2. `metadata_store.lookup_profile_id(name)`:
   - `Ok(Some(id))` → `(profile, id)`.
   - `Ok(None)` → `Err("profile has no metadata record yet — save the profile once, then retry")`.
   - `Err(e)` → `Err("metadata_unavailable: {e}")`.

The returned `GameProfile` is the **loaded** profile (local_override already baked in), and `analyze_profile_mod_coexistence` then calls `effective_profile()` again.

### 3.2 Per-command invariants

| Command                                | Rules                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_profile_mods` (`:57-101`)        | If `!is_available()` → `Ok({available:false, mods:[]})` (**not an error**). Load profile (errors if missing). `lookup_profile_id` error → `metadata_unavailable:`. `Ok(None)` → `{available:true, mods:[]}` (profile exists but has no metadata record). Else list ordered by `name COLLATE NOCASE` (`mods_store.rs:96-106`).                                                                                                                                                            |
| `add_profile_mod` (`:104-127`)         | `!is_available()` → `Err(METADATA_UNAVAILABLE)`; `resolve_profile`; `add_profile_mod` (validated, see below). Store errors → plain `to_string` ("metadata validation error: …").                                                                                                                                                                                                                                                                                                         |
| `update_profile_mod` (`:129-154`)      | Same guard and resolve. The update is scoped `WHERE profile_id=? AND mod_id=?` (a mod of another profile cannot be updated). Missing → `Validation("mod not found")` (`mods_store.rs:188-193`). `created_at` preserved, `updated_at` bumped (test `mods_tests.rs:140`).                                                                                                                                                                                                                  |
| `remove_profile_mod` (`:156-184`)      | Same guard and resolve. `removed==false` → `Err("mod not found")`. Scoped by profile. Test `remove_returns_true_then_false` (`mods_tests.rs:176`).                                                                                                                                                                                                                                                                                                                                       |
| `detect_profile_mods` (`:186-234`)     | Loads the profile; `effective_profile().game.executable_path.trim()` empty → `Err("no_game_path: …")`; no parent dir → same error. `scan_game_directory(parent)` runs in `spawn_blocking`. Then **best-effort** `mark_already_registered` enrichment only when the store is available, and any lookup error leaves flags `false` (**works with the DB down**, unlike the other 5). **Note:** this command intentionally does not require metadata or a metadata profile id. Preserve it. |
| `analyze_mod_coexistence` (`:236-258`) | Guard (`metadata_unavailable`), `resolve_profile`, then `core::launch::analyze_profile_mod_coexistence(metadata, profile_id, &profile)` (`core/launch/mod_coexistence.rs:45-61`). Propagates store errors, unlike the launch collector.                                                                                                                                                                                                                                                  |

### 3.3 Input validation (store boundary, `mods_store.rs:10-94`)

Name trimmed, required, ≤200 chars. Paths: trimmed, empties dropped, ≤32 paths, no NUL, ≤1024 chars each, serialized JSON ≤8 KiB. `source_url`: trimmed, empty → `None`, must start with `http://` or `https://` (case-insensitive), ≤2048 chars. Unique `(profile_id, name)` is case-insensitive and enforced by SQLite; the friendly message comes from `map_write_error` (`:323-336`). Validation tests: `mods_tests.rs:206-308`. Corrupt `paths_json` → typed `Corrupt` (`:310`).

### 3.4 Coexistence analysis (`core/mods/coexistence.rs`, `core/launch/mod_coexistence.rs`)

Pure rules table `RULES` (`coexistence.rs:41-105`); severities are only `Warning`/`Info` ("no rule is fatal", test `mods/tests/coexistence.rs:301`):

- `mod_coexistence_injection_vector`, Warning: shared injection vector **and** DLL-basename overlap with the profile's injection DLLs.
- `mod_coexistence_injection_vector`, Info: shared vector, no overlap, and `trainer_loading_mode == CopyToPrefix`.
- `mod_coexistence_overlay_conflict`, Warning: `OverlayInjection` mod and (MangoHud or gamescope enabled).
- `mod_coexistence_file_replacement_notice`, Info: `FileReplacement` mod.
- `mod_coexistence_script_extender_launch_order`, Info: `ScriptExtender` mod and trainer configured.

`shares_injection_vector` requires a trainer configured, and either a script extender or an overlay mod whose path basenames include one of `PROXY_DLL_NAMES` (`dxgi, d3d11, d3d9, dinput8, winmm, version`; `mods/detection.rs:14-21`). Basenames are compared lowercased.

`analyze_profile_mod_coexistence` adds, on top of the rules:

- A **path-missing pass**: one Info `mod_coexistence_registered_path_missing` per enabled mod with at least one registered path whose `symlink_metadata` fails (relative paths resolve against `executable_path.parent()`; absolute paths always checked) (`mod_coexistence.rs:116-137`).
- The context comes from the `effective_profile()`: trainer path non-empty, `trainer.loading_mode`, `injection.method`, injection DLL basenames, `launch.mangohud.enabled`, `launch.gamescope.enabled`.

The request-shaped launch variant `collect_mod_coexistence_launch_warnings` (`mod_coexistence.rs:18-38`) is non-blocking: returns `[]` on a disabled DB, unknown profile, no enabled mods, or any error. It never emits the path-missing issue (test `mods_launch_tests.rs:190`). It already lives in the launch path and is **not** part of the mods service move. The service must not change it.

### 3.5 Existing tests (mods)

- `core/metadata/mods_tests.rs`: 22 tests (CRUD, ordering, duplicate name incl. case, same name different profiles, update/remove semantics, list_enabled, validation limits, corrupt JSON, disabled store errors).
- `core/metadata/mods_launch_tests.rs` (8): disabled → empty, unknown profile, corrupt JSON, code mapping, panel path-missing gating, preview never emits path-missing, profile-vs-request agreement.
- `core/mods/tests/coexistence.rs` (~19 rules tests) and `detection.rs` (~23 detection/`mark_already_registered` tests).
- Frontend: `src/hooks/__tests__/useProfileMods.test.ts`, `ModDetectionReviewModal.test.tsx`.
- **No Tauri-level test for the `resolve_profile` flow or the `metadata_unavailable:` / `no_game_path:` string prefixes, and no `detect_profile_mods` command test.** Gap for the service: assert profile resolution outcomes (missing profile, no metadata row, DB disabled), the cross-profile `mod_id` scoping, and "detect works with the DB down".

---

## 4. Version tracking (4 commands, `tauri/commands/version.rs`)

All four are **synchronous** and do blocking I/O: manifest scan and trainer hash of the whole file via `fs::read` (`core/metadata/version_store.rs:212-220`).

### 4.1 `check_version_status(name)` (`version.rs:354-421`)

1. `profile_store.load(name)`; errors propagate. Uses the **loaded** profile (not `effective_profile`). Trimmed `steam.app_id`.
2. Steam client path = `steam_client_install_path_from_compatdata(profile.steam.compatdata_path)` (`core/steam/client_path.rs`; empty string when the path lacks `/steamapps/compatdata/`).
3. If `app_id` non-empty: `locate_manifest_for_app` = `discover_steam_root_candidates` → `discover_steam_libraries` → first `<steamapps>/appmanifest_<id>.acf` that `is_file()` → `parse_manifest_full` → `(build_id, state_flags)`. Any failure → `("", None)`.
4. `current_build_id = None` if empty. `current_trainer_hash = hash_trainer_file(trimmed trainer.path)` (`None` for empty path or unreadable file; SHA-256 lowercase hex).
5. `profile_id = lookup_profile_id(name)? .unwrap_or_default()` (empty string when unregistered). Latest snapshot only when `profile_id` non-empty (`lookup_latest_version_snapshot` orders by `checked_at DESC LIMIT 1`).
6. `compute_correlation_status` (`version_store.rs:175-208`), pure:
   - `state_flags` is `Some(n)` with `n != 4` → **`UpdateInProgress`** (checked first).
   - No snapshot `steam_build_id` → `Untracked`.
   - Otherwise `build_changed = current_build != snapshot_build`; `trainer_changed = current_hash != snapshot_hash` (Option comparison; `None != Some` counts as changed). → `BothChanged | GameUpdated | TrainerChanged | Matched`.
7. Returns `VersionCheckResult{profile_id, current_build_id, snapshot, status, update_in_progress}`.

**Read-only**: `check_version_status` does NOT write a snapshot. Snapshot writes happen elsewhere (launch finalizer `services/launch/snapshot.rs:141-158`, community import seed `services/profiles/writer.rs:335-345`, `status="untracked"`). The launch finalizer **skips** the upsert when status is `UpdateInProgress` (`snapshot.rs:~127-135`).

`manifest` lookup helpers are duplicated: `crates/crosshook-app/src/services/launch/snapshot.rs:86`, `services/health/steam.rs:13`, `bootstrap/version_scan.rs:71`, and `version.rs:329-347`. Reuse or extract rather than adding a 5th copy.

### 4.2 `get_version_snapshot` (`:424-443`)

`lookup_profile_id` → empty id → `Ok(None)` → latest snapshot → `VersionSnapshotInfo` (`status` parsed from string with `status_from_str`, unknown strings → `Unknown`, `version.rs:302-312`; `row.id` is omitted from the DTO).

### 4.3 `set_trainer_version(name, version)` (`:450-479`)

- `lookup_profile_id` must be `Some`, else `Err("profile '<n>' is not registered in the metadata store")`.
- `steam_app_id` is **preserved** from the latest snapshot (lookup errors are swallowed via `.ok().flatten()`), else `""`.
- Inserts a **new** snapshot via `upsert_version_snapshot(profile_id, steam_app_id, None, Some(version), None, None, "untracked")`. So a manual trainer-version hint wipes `steam_build_id`/`trainer_file_hash` on the newest row. Because the new row has a NULL `steam_build_id`, the next status check will be `Untracked`. **Preserve** (maybe a latent product bug, but it is today's behaviour).
- The `version` string is **not trimmed or validated** server-side (the hook trims: `useSetTrainerVersion.ts:29`). An empty `version` is stored as `Some("")`.
- The upsert (`version_store.rs:10-70`) is a savepoint: INSERT, then prune to the newest `MAX_VERSION_SNAPSHOTS_PER_PROFILE = 20` rows by `checked_at DESC` (`models.rs:409`; test `version_store_tests.rs:91`).

### 4.4 `acknowledge_version_change(name)` (`:485-498`)

`lookup_profile_id` must be `Some`, else the same "not registered" error. `UPDATE version_snapshots SET status='matched' WHERE id = (latest by checked_at)` (`version_store.rs:152-173`). It does not touch hashes or build ids. **No-op (still `Ok`) when there are no snapshots.** The frontend refreshes health afterwards (`useAcknowledgeVersionChange.ts:26-47`).

### 4.5 Tests

- `core/metadata/version_store_tests.rs` (6): lifecycle, latest, pruning at max, acknowledge → matched, bulk latest-per-profile, disabled store no-op.
- `core/metadata/correlation_status_tests.rs` (6): in-progress, untracked, matched, game_updated, trainer_changed, both_changed.
- `core/metadata/trainer_hash_tests.rs`.
- `tauri/commands/version.rs:500-518`: only a compile-time signature pin (`command_names_match_expected_ipc_contract`).
- **Gaps**: no test for `check_version_status` end-to-end (manifest lookup with `StateFlags!=4`, missing manifest, unreadable trainer), `set_trainer_version` preserving `steam_app_id`, `status_from_str` mapping, `get_version_snapshot` for an unregistered profile, or disabled-store behaviour of the command layer.

---

## 5. Launcher export (12 commands, `tauri/commands/export.rs`)

All 12 are **synchronous** `#[tauri::command] fn`s. `export_launchers` (the `.sh` and `.desktop` writes), `list_launchers`, `reexport_launcher_by_slug` and `delete_launcher*` run blocking fs I/O on the caller thread. That is acceptance criterion 4 ("`export_launchers` no longer blocks the caller thread").

### 5.1 Slug and naming (`core/export/launcher/names.rs`)

- `resolve_display_name(preferred, steam_app_id, trainer_path)` (`:8-32`):
  1. non-blank `preferred` → `strip_trainer_suffix` (removes one trailing `" - Trainer"` after trim);
  2. else trainer file stem (suffix stripped);
  3. else `"steam-<app_id>-trainer"`;
  4. else `"crosshook-trainer"`;
  - then `\n`, `\r`, `\0` → space.
- `sanitize_launcher_slug(value)` (`:43-72`):
  - blank → `"crosshook-trainer"`;
  - lowercase (Unicode-aware `flat_map(char::to_lowercase)`), keep `is_alphanumeric()` (**Unicode alphanumerics are kept**), runs of other chars collapse into a single `-`, trim `-`;
  - empty result → fallback.
- Paths (`launcher/service.rs:138-147`, `launcher_store/paths.rs`):
  - Script `<home>/.local/share/crosshook/launchers/<slug>-trainer.sh`.
  - Desktop `<home>/.local/share/applications/crosshook-<slug>-trainer.desktop`.
  - `resolve_target_home_path` (`paths.rs:62-89`):
    - Normalizes the preferred home (trim, `\`→`/`, flatpak host-mount rewrite).
    - Uses it only if it starts with `/` and **does not contain `/compatdata/`**.
    - Else derives home from the steam client path suffix (`/.local/share/Steam`, `/.steam/root`, `/.var/app/com.valvesoftware.Steam/data/Steam`).
    - Else `$HOME`.
    - Else the preferred value, else `""` (→ `CouldNotResolveHomePath`).
  - The `export_writes_expected_paths_and_content` test pins this: `target_home_path: "/tmp/not-a-home/compatdata/steam"` is rejected in favour of the steam client path.

### 5.2 Validation (`launcher/types.rs:138-205`)

Method must be `steam_applaunch` or `proton_run` (trimmed). Errors:

- `TrainerPathRequired`, `PrefixPathRequired`, `ProtonPathRequired`.
- `TargetHomePathRequired` (only when target home is blank **and** `$HOME` unset/blank).
- `SteamAppIdRequired` (only for `steam_applaunch`).
- `InvalidCustomEnvironmentVariable` (key must match `[A-Za-z_][A-Za-z0-9_]*`; value no NUL).
- `LauncherIconPathNotFound` plus an icon extension/format check when `launcher_icon_path` is non-blank.

`validate` reads `$HOME` and checks file existence, so it is **environment-dependent**; any golden/service test must pin `HOME`, or use temp dirs as the existing tests do.

### 5.3 Build-request logic (`export.rs:16-61`, `build_export_request_for_profile`)

- Method via `resolve_launch_method(profile)`; **`native` → `Err("Profile launch method does not support launcher export")`**.
- `steam_applaunch` uses `steam.compatdata_path` / `steam.proton_path`; everything else uses `runtime.prefix_path` / `runtime.proton_path`.
- `launcher_name` = `steam.launcher.display_name`; icon = `steam.launcher.icon_path`; `trainer_loading_mode` = `trainer.loading_mode`.
- `umu_preference` = `runtime.umu_preference.unwrap_or(global)`.
- `gamescope` = `launch.resolved_trainer_gamescope()`.
- Also passes `network_isolation`, `custom_env_vars`, `runtime_steam_app_id`, `umu_game_id`.
- `core::export::launcher_store::check_launcher_for_profile` (`queries.rs:100-145`) builds the **same** request inline (duplicated mapping). The service can reuse a shared core builder; the duplication is DRY debt but must produce an identical request.

### 5.4 Export, delete, rename

- **Export** (`launcher/service.rs:82-109`): validate → write script with mode `0o700` and desktop with mode `0o644` (`write_host_text_file`). Mode 0o700 is pinned by a test because scripts carry custom env values. The `LauncherInfo`/result carries `display_name`, `launcher_slug`, `script_path`, `desktop_entry_path`. After a successful export the command does **fail-soft** `observe_launcher_exported(request.profile_name, slug, display_name, script_path, desktop_path)`. The launchers row is upserted `ON CONFLICT(launcher_slug)` with `profile_id = COALESCE(new, old)` and `drift_state='aligned'` (`core/metadata/launcher_sync.rs:5-50`).
- **Delete**:
  - `delete_launcher(display_name, steam_app_id, trainer_path, home, steam_client)` → `delete_launcher_files` → slug via `derive_launcher_paths` (`launcher_store/paths.rs`), then `observe_launcher_deleted(slug)` fail-soft. The slug sent to metadata is `sanitize_launcher_slug(&display_name)`, which can differ from the resolved-name slug when `display_name` is blank (`export.rs:258`).
  - `delete_launcher_by_slug(slug, home, steam_client)` → `derive_launcher_paths_from_slug` (the slug is **re-sanitized**, `paths.rs:~35-53`), then `observe_launcher_deleted(&launcher_slug)` with the **raw** slug.
  - Both go through `delete_launcher_at_paths` (`launcher_store/mutations.rs:32-63`): the desktop entry is removed first, then the script, each only if it is a regular file (no symlink) **and contains the CrossHook watermark** (`SCRIPT_WATERMARK="# Generated by CrossHook"`, `DESKTOP_ENTRY_WATERMARK="Generated by CrossHook"`, `launcher_store/mod.rs:19-22`). Otherwise `*_skipped_reason` is set and nothing is deleted. Tests: `launcher_store/tests/delete.rs` (symlink skipped, non-watermarked skipped, watermarked deleted).
  - `observe_launcher_deleted` only **tombstones** (`drift_state='missing'`), never deletes the row.
- **Rename** (`export.rs:290-350`, `core/export/launcher_store/mutations.rs:104-266`):
  - The command builds a partial request with `profile_name: None`, `umu_preference: Auto`, `runtime_steam_app_id: ""`, `umu_game_id: ""`. **Therefore renamed scripts lose profile-level umu/runtime settings.** Preserve or consciously fix.
  - `trainer_loading_mode` is parsed from a string (`"invalid trainer loading mode"` on failure).
  - `rename_launcher_files` validates the request; new slug = `sanitize_launcher_slug(new_display_name)`.
  - Returns early with `renamed:false` (both flags false) when neither old file is a regular file.
  - Destination safety applies **only when the slug changed**: an existing destination must be a regular file with the watermark, else `old_*_cleanup_warning = "Refusing to overwrite new …"` and **nothing is written**.
  - Otherwise it writes the new files with the same modes (0o700, 0o644), only for the old files that existed, then deletes the old files via `remove_old_launcher_file` (watermark-verified; failures become `old_*_cleanup_warning`s, not errors). When the slug is unchanged, the files are rewritten in place and nothing is deleted.
  - Then fail-soft `observe_launcher_renamed(old_slug, new_slug, new_display_name, new_script_path, new_desktop_path)`, which is `with_conn_mut` in a savepoint (`launcher_sync.rs:~60+`, test `launcher_tests.rs:314`).
  - Rename tests: `launcher_store/tests/rename.rs` (10 tests incl. symlink and non-CrossHook destination refusal).
- The Tauri `rename_launcher` takes **16 positional args** (`export.rs:290-307`). A service API should take a request struct (this changes only the Rust side; the IPC arg names must stay identical).

### 5.5 Check, list, orphans, stale flags, re-export by slug

- `check_launcher_exists(request)` → `check_launcher_exists_for_request(request.launcher_name, request)` (`queries.rs:56-98`):
  - Existence uses `is_regular_file_safe` (`lstat`; symlinks and directories count as **nonexistent**; tests `check.rs:303,328`).
  - `is_stale` = (script exists and its content `!=` the freshly generated `build_trainer_script_content`, or the read fails) **OR** (desktop exists and its `Name=` line `!=` resolved display name, or no `Name=` line).
  - An unreadable desktop file → `Err` (`LauncherStoreError`; test `check.rs:134`).
- `check_launcher_for_profile(name)` (`export.rs:224-237`): loads the profile and the **settings** (a failing settings load is a hard `Err`); calls core with empty `target_home_path`/`steam_client_install_path` (`""`, `""`), so the home is resolved from `$HOME`. `native` → `LauncherInfo::default()`. Tests `check.rs:154,207` (legacy empty method resolves as steam).
- `list_launchers(home, steam_client)` (`queries.rs:154-253`, **core** part): read the launchers dir; keep only regular files ending in `-trainer.sh` with a non-empty slug; display name from the desktop `Name=` (Name includes the " - Trainer" suffix) or fallback to the slug; `is_stale: false` always; sorted by slug. A missing dir → empty; other read errors are warned and yield empty.
- **Stale flags** are applied in the **Tauri command**, not in core (`export.rs:63-135, 354-370`):
  - `stale_flags_by_profile_slug` iterates every profile (`ProfileStore::list`/`load`). A profile that fails to load is warned and skipped.
  - `native` profiles are skipped. Others call `check_launcher_for_profile_core(profile, home, steam, global_umu)`; an error is warned and skipped; an empty slug is skipped.
  - Multiple profiles that map to the same slug are OR-combined (`is_stale = a || b`).
  - Settings load failure → warn and `UmuPreference::Auto` (note: the single-profile `check_launcher_for_profile` command instead hard-fails; keep both behaviours or document).
  - `apply_stale_flags` sets `launcher.is_stale = map.get(slug).copied().unwrap_or(false)`.
  - Tests exist only for `apply_stale_flags` (`export.rs:540-575`).
- `find_orphaned_launchers(known_profile_slugs, home, steam)` = `list_launchers` filtered by `!known.contains(slug)` (`queries.rs:256-265`). **No profile access**: the caller supplies the known slugs. Test `list.rs:67`. The service may want to compute the known slugs itself, but the current IPC contract takes them as input, so keep the signature.
- **Re-export by slug** (`export.rs:137-185, 387-432`):
  1. `find_profile_name_for_launcher_slug`: iterate all profiles (`list` error → hard `Err`; load errors warn and skip). For each, `build_export_request_for_profile` (skip on error, i.e. `native`), then `check_launcher_exists_for_request(request.launcher_name, &request)` (skip on error); first profile whose computed `launcher_slug == slug` wins. Settings failure here falls back to `UmuPreference::Auto` (silently, no log).
  2. No match → `Err("No profile found for launcher slug '<slug>'")`.
  3. Reload the profile, rebuild the request with `profile_name: Some(name)` and settings (**hard `Err` if settings fail to load here**; inconsistent with step 1), `validate`, `export_launchers`, then fail-soft `observe_launcher_exported`.
  - It matches on the _profile-derived_ slug, not on raw slug equality with an exported file. **No test covers this flow** (only the signature pin, `export.rs:491-499`).
  - Note `request.target_home_path`/`steam_client_install_path` come from the IPC args, not the profile.
- **Previews** (`launcher/preview.rs`, `export.rs:437-452`): `validated_launcher_paths` (validates, no writes), then a placement-comment header + body. Script header: `# Save this file to: <path>\n# Make executable: chmod +x <path>\n\n` + script. Desktop header: `# Save this file to: <path>\n# Permissions: 644 (not executable)\n\n` + desktop. Trainer path empty → error (tests `preview.rs:56,76`; header order `preview.rs:8,33`).
- `validate_launcher_export` (`export.rs:189-191`): `validate(&request).map_err(to_string)`.

### 5.6 Golden / byte-identical tests (acceptance criterion 3) — **none exist today**

There are **no golden fixture files for launcher script or `.desktop` output** anywhere in the repo. Verified:

- `crates/crosshook-core/tests/fixtures/` has only `launch_request/`.
- `crates/crosshook-app/tests/fixtures/` has only `profile_writer/`.
- No `*.desktop` fixtures exist.
- No snapshot crate is used for launcher output.

Current launcher output coverage is **substring assertions only**:

- `core/export/launcher/tests/content.rs` (~15 tests: gamescope enabled/disabled/nested, custom env ordering, network isolation, umu probe/dual exec, proton preference).
- `export_write.rs` (4 tests: paths, content substrings, modes 0o700 and 0o644, suffix de-dup, proton_run prefix bootstrap, steam trainer gamescope).
- `preview.rs`, `naming.rs`, and `launcher_store/tests/check.rs:59` (`is_stale` against a regenerated script).

Therefore criterion 3 requires **creating** the goldens from the **pre-move** output _before_ touching the code (never regenerate from the new service), mirroring the established pattern:

- `crates/crosshook-core/tests/launch_request_golden.rs` + `tests/fixtures/launch_request/*.json`.
- `crates/crosshook-app/tests/profile_writer_golden.rs` + `tests/fixtures/profile_writer/` (current-thread runtime, `:223`).
- `docs/plans/profile-writer/profile-writer.plan.md:431`: "Never regenerate fixture bytes from the new writer".

Determinism checks for goldens:

- `content.rs` contains no timestamps or random ids (`grep Utc|now|SystemTime` found nothing in `launcher/content.rs`).
- The script embeds `request.steam_client_install_path`, trainer/proton/prefix paths and the home path, so goldens must use fixed temp-free absolute fake paths.
- The icon path must exist for `validate` (`LauncherIconPathNotFound`), so goldens must either use an empty icon path, or `build_*_content` can be called directly (it is `pub(crate)` in core, so an app-level golden must go through `export_launchers` / `preview_*` into a tempdir, or pin the preview strings).
- Preview output contains the resolved `script_path`, which depends on `HOME`, so use an explicit `target_home_path` that passes `looks_like_usable_host_unix_path` (starts with `/`, no `/compatdata/`).

---

## 6. Steam: default client path, auto-populate, proton installs (3 commands, `tauri/commands/steam.rs`)

### 6.1 `default_steam_client_install_path()` (`steam.rs:9-33`) — sync, no args

1. `$STEAM_COMPAT_CLIENT_INSTALL_PATH` if set and non-blank (trimmed value returned verbatim, **not** checked for existence).
2. `$HOME` unset → `""`.
3. First of `~/.local/share/Steam`, `~/.steam/root`, `~/.var/app/com.valvesoftware.Steam/data/Steam` where `<candidate>/steamapps` **is a directory** → its string.
4. Else `""`.

Note this order differs from `crosshook-cli/src/launch.rs:~197-225` (`resolve_steam_client_install_path`, which uses `~/.steam/root` first and checks `steam.sh`). The CLI is out of scope; do not unify silently.

Consumers:

- `PreferencesContext.tsx:58`, `CommunityImportWizardModal.tsx:80`.
- `tauri/commands/protonup.rs:168` and `commands/migration.rs:14,35` import it through `super::steam::default_steam_client_install_path`. **Moving it to a service requires updating those two Tauri callers**; they must call the service (or a shared core fn).
- Test mock `DashboardRoutes.test.tsx:127`.
- There are **no tests** for this function today. It is environment-dependent (`std::env::var`), so the service should accept an injectable env/home for tests.

### 6.2 `list_proton_installs(steam_client_install_path: Option<String>)` (`steam.rs:35-48`) — **sync, blocking** (criterion 4)

- Config path = arg, else `default_steam_client_install_path()`.
- `discover_steam_root_candidates(configured, &mut diagnostics)` then `discover_compat_tools(roots, &mut diagnostics)`, and each diagnostic logged via `tracing::debug!`. Returns `Vec<ProtonInstall>` (never errors; `Result<_, String>` is cosmetic).
- Spec calls for "async scan"; the registry in `crosshook-app` is `app.jobs().start(JobSpec::new(kind), |ctx| async ...)` (`jobs/registry.rs:134`). The frontend uses a plain request/response `callCommand` (`useProtonInstalls.ts:50`, `useProfilesPageProton.ts:40,53`). A service must keep `Vec<ProtonInstall>` returned as a plain awaitable (via `run_blocking`) for the existing hooks. A job can only be added for the native UI.
- Core tests: `core/steam/proton/tests/discovery.rs` (3: official/custom/system tools; alias normalization; compat-tool mappings), plus `flatpak.rs`, `resolution.rs`.

### 6.3 `auto_populate_steam(request)` (`steam.rs:50-58`) — already `spawn_blocking`

- Pure wrapper around `attempt_auto_populate` (`core/steam/auto_populate.rs:12`). Empty `game_path` → NotFound states + manual hints. Discover roots (the request's `steam_client_install_path` is **always passed through**, no default substitution; a blank value yields default root discovery) → libraries → `find_game_match` → compatdata dir existence → proton resolution. Result `SteamAutoPopulateResult{app_id_state, app_id, compatdata_state, compatdata_path, proton_state, proton_path, diagnostics, manual_hints}`.
- States: `NotFound | Found | Ambiguous`. `has_any_match` is true if any state is `Found`.
- A `JoinError` → `error.to_string()` (the sanitized `AppError` will replace this).
- Also used by core `profile/exchange/utils.rs:92` (community import), independent of the command.
- Tests: `core/steam/auto_populate.rs:221` (`auto_populate_finds_app_id_compatdata_and_proton`) plus `steam/manifest` and `libraries` tests. Frontend callers `AutoPopulate.tsx:136`, `CommunityImportWizardModal.tsx:108`.

---

## 7. Game metadata and cover art (4 commands, `tauri/commands/game_metadata.rs`)

### 7.1 `fetch_game_metadata(app_id, force_refresh)` → `lookup_steam_metadata` (`core/steam_metadata/client.rs:80-122`)

- `normalize_app_id`: trim; must be all ASCII digits and non-empty, else **`Ok(SteamMetadataLookupResult::default())`**, i.e. state `Idle`, **not an error** (`models.rs:1-12`, tests `steam_metadata/tests.rs:56,65`).
- Cache key `steam:appdetails:v1:<id>` in `external_cache_entries` (`models.rs:1,15-19`); TTL **24 h** (`CACHE_TTL_HOURS`, `client.rs:18`).
- Cache hit (unless `force_refresh`) → `state: Ready, from_cache: true, is_stale: false`.
- Otherwise live fetch (`https://store.steampowered.com/api/appdetails?appids=<id>`, 6 s timeout, `CrossHook/<ver>` UA):
  - Success → persist (`put_cache_entry`, errors only warned) → `Ready, from_cache:false`.
  - Failure (network, 404, `success:false`, invalid payload) → warn → **stale cache fallback** (expired rows allowed) → `state: Stale, from_cache: true, is_stale: true`.
  - No cache → **`state: Unavailable`**, `app_details: None`.
- **This is the existing "offline reports Unavailable" behaviour**, and it is already data, not an error. No `offline_mode` setting is consulted in these commands (grep: `offline_mode` appears only in settings code and `crosshook-app/services/settings`). Offline is detected purely by request failure/timeouts. If "Offline: art fetch reports `Unavailable`" should also honour `settings.offline_mode`, that is **new** behaviour (needs a decision).
- Tests: `core/steam_metadata/tests.rs` (6: empty/non-numeric id, valid cache hit, expired cache → stale when network fails, missing cache without network → unavailable, force_refresh bypasses valid cache; uses `seed_cache_entry`). Note these tests require the network to fail to be deterministic. `models.rs` unit tests cover normalization, key and snake_case state.

### 7.2 `fetch_game_cover_art(app_id, image_type, ...)` (`game_metadata.rs:14-43`) → `download_and_cache_image` (`core/game_images/client/api.rs:20-275`)

- `image_type` strings: `cover` (default), `hero`, `capsule`, `portrait`, `background`; anything else → `Err("Unknown image type: <x>")` (warned).
- SteamGridDB API key read from settings with `settings_store.load().ok().and_then(steamgriddb_api_key).filter(non-blank)` (load failure is non-fatal and the key is silently dropped).
- `download_and_cache_image` contract:
  - `app_id` must be non-empty decimal else `Err("invalid app_id …")`.
  - Valid (non-expired) DB cache row **and file exists** → return the path with no network.
  - Cache row whose file is missing → delete the row and fall through to download.
  - Download: SteamGridDB first when a key is present; on **auth failure** fall back to Steam CDN (Portrait tries three candidate URLs); any other SteamGridDB error also falls back to CDN. CDN failure → **stale fallback path** (an expired row whose file exists) else `Ok(None)`.
  - Validation: size ≤ 5 MiB, MIME allow-list (jpeg, png, webp via magic bytes; SVG/HTML rejected) (`validation.rs:10-30`); app_id and filename path-safety (`safe_image_cache_path` rejects `..` and `/`).
  - Redirects allow-listed to 4 hosts over https only (`http.rs`).
  - Cache dir: `$LOCALDATA/crosshook/cache/images/<app_id>/…` (`client/cache.rs:42-50`). DB row `game_image_cache` with SHA-256, 24 h expiry. A DB-persist failure is **non-fatal** (file still returned).
- Return shape is `Result<Option<String>, String>`: `Ok(None)` = "no art" for download failure/offline with no stale cache. **There is no `Unavailable` state for art today**; the issue's "Offline: art fetch reports `Unavailable`, cached art still shows" is a **new surface** unless the service maps `Ok(None)` + network error to an `Unavailable` result. The current frontend (`useGameCoverArt.ts`) expects `string | null`, so changing the IPC return type would break it. The service can return a richer type internally, while the Tauri command maps back to `Option<String>`.
- Tests: `core/game_images/client/tests.rs` (validation, filename/URL builders, expiry parsing, safe-path, DB cache get/upsert, redirect hosts, app_id guard). There are **no network-failure fallback tests** (stale fallback, missing file → row deletion).
- Disabled store: `get_game_image`/`upsert_game_image` are `with_conn`, so they no-op; the image still downloads and is returned (no caching).

### 7.3 `import_custom_cover_art(source_path)` and `import_custom_art(source_path, art_type)` (`game_metadata.rs:45-65`) → `core/game_images/import.rs`

- `art_type` ∈ `cover` (default), `portrait`, `background`; anything else → `Err("Unknown art type: <x>")`. Core additionally rejects other `GameImageType`s (`Unsupported art type…`).
- `import_custom_cover_art` is a legacy wrapper for `Cover`. It is **not called by any frontend code** (only `import_custom_art` is used, `MediaSection.tsx:59`), but it is registered in `tauri/src/lib.rs:131`; the issue says 4 commands, so keep it.
- Behaviour: source must exist; `fs::read`; `validate_image_bytes` (5 MiB, MIME allow-list); destination `$LOCALDATA/crosshook/media/{covers|portraits|backgrounds}/<sha256[..16]>.<ext>`; **content-addressed and idempotent** (an existing file is not rewritten). Returns the absolute path.
- Issue constraint "Image cache stays in `$LOCALDATA/crosshook/cache/images` and `media/`": the paths above are unchanged.
- Blocking `fs::read` + `fs::write` are done **inside a sync command** (blocks the caller), so these should use `run_blocking` in the service.
- `is_in_managed_media_dir(path)` exists to decide whether imported art lives under `media/` (test `import.rs` `is_in_managed_media_dir_detects_managed_paths`).
- **Security**: `source_path` is renderer-supplied and is read verbatim (symlinks followed). Validation is by magic bytes, so only images can be copied, but any readable image file path is accepted. No allow-list. A pre-existing characteristic; the service API should keep the validation, not weaken it.

---

## 8. Storage (3 commands, `tauri/commands/storage.rs`) — acceptance criterion 1

### 8.1 Current behaviour (already rescans; already allow-lists)

- `scan_prefix_storage` (`:17-54`, `spawn_blocking`):
  1. `collect_profile_prefix_references(&profile_store)` (`core/storage/prefix_health/references.rs:7-46`): lists profiles; each load failure is warned and sets `profiles_load_failed=true`; uses `profile.effective_profile().runtime.prefix_path` (trim, skip empty), `resolve_wine_prefix_path`, then canonical-or-raw `normalized_path_string`.
  2. `scan_prefix_storage_core(refs, DEFAULT_STALE_STAGED_TRAINER_DAYS = 14, profiles_load_failed)` (`scan.rs`).
  3. For **each** prefix entry persists a `PrefixStorageSnapshotRow` (`id = "<resolved_path>-<scanned_at>"`) **fail-soft** (warn only). `INSERT OR REPLACE` (`core/metadata/prefix_storage_store.rs:10`).
  - It is a plain request/response, **not a job** (the issue wants "scan as a job").
- Scan result rules (`scan.rs:11-92`):
  - Candidate prefixes = referenced prefixes ∪ discovered siblings (`discover_candidate_prefixes`: for each _existing directory_ referenced prefix, scan its parent dir, and when the prefix ends in `pfx`, the compatdata root's `*/pfx`; skipping symlinks and non-dirs, `discovery.rs`).
  - Only dirs that contain `drive_c` are kept.
  - `is_orphan = !inventory_incomplete && no referencing profiles && has "drive_c/CrossHook" dir` (`has_crosshook_managed_marker`). **If any profile failed to load (`inventory_incomplete`), nothing is ever reported as an orphan**, a conservative invariant that must be preserved.
  - Stale staged trainers: entries under `<prefix>/drive_c/CrossHook/StagedTrainers` with `mtime <= now - 14d`; a symlink staged root is skipped; symlink entries skipped (`staged_trainers.rs`).
  - Output sorted by path. `orphan_targets` and `stale_staged_targets` are the **allow-list**: targets `{kind, resolved_prefix_path, target_path}`.
- `cleanup_prefix_storage(targets)` (`:56-145`, `spawn_blocking`):
  1. **Rescan** (`collect_profile_prefix_references` + `scan_prefix_storage_core`), so the allow-list is fresh (`:65-77`).
  2. Build the allowed signature set `(kind, resolved_prefix_path, target_path)` from `orphan_targets ∪ stale_staged_targets`. Filter the incoming targets to those whose signature is in the set, and **de-dup** repeated signatures (`:79-86`). Unknown targets are **silently dropped** (no `skipped` entry, no audit row, no error). The issue says "rejects" and the acceptance test says "cleanup rejects target missing from fresh scan". Decide whether a rejected target becomes a `skipped` entry with reason (recommended, and an audit row) or stays silently dropped. A changed IPC result shape (more `skipped` entries) is additive and backward compatible with `PrefixCleanupResult`.
  3. `cleanup_prefix_storage_core(&references, &filtered)` (`core/storage/prefix_health/cleanup.rs`), which applies **per-target defence in depth** (see §8.2) and re-checks the referenced set (`cleanup.rs:20`).
  4. Audit rows, fail-soft (warn): one row per `deleted` (`result:"deleted"`, `reclaimed_bytes:0`), one per `skipped` (`result:"skipped"`, `reason`), and one **summary** row (`target_kind:"summary"`, `result:"deleted"`, `reclaimed_bytes=total`) only if `reclaimed_bytes>0 || !deleted.is_empty()` (`:92-139`). Row kind strings `orphan_prefix` / `stale_staged_trainer` (`cleanup_target_kind_str`, `:185-190`). IDs are UUID v4, `created_at` is the single `now` RFC 3339.
- `get_prefix_storage_history` (`:154-175`): `available = is_available()`; `list_latest_prefix_storage_snapshots(50)`; `list_prefix_storage_cleanup_audit(100)`; **errors propagate** as `e.to_string()` while the disabled store yields empty lists (`with_conn`). The frontend treats any thrown error as `persistenceAvailable=false` (`usePrefixStorageManagement.ts:57-62`).
- `DEFAULT_LOW_DISK_WARNING_MB`/`check_low_disk_warning` exist (`disk.rs`), unrelated to these commands.

### 8.2 Core cleanup checks per kind (`core/storage/prefix_health/cleanup.rs`)

**OrphanPrefix** (`cleanup_orphan_prefix`, `:36-104`), checked in order, each failure → `skipped` with a reason:

1. Referenced check: `normalized(target.resolved_prefix_path)` is in the referenced map ("prefix is still referenced by at least one profile").
2. `target_path.is_dir()` (follows symlinks) else "target no longer exists".
3. `<target>/drive_c` is a dir ("does not look like a wine prefix").
4. `drive_c/CrossHook` marker exists ("not marked as crosshook-managed").
5. `fs::canonicalize(target_path)` succeeds.
6. `normalized(canonical_target) == normalized(resolved_prefix_path)` ("target path does not match resolved prefix path"). **This is what rejects a target that became a symlink pointing elsewhere**, and a target whose resolved path differs from the target path.
7. `remove_dir_all(canonical_target)`: deletes the _canonical_ path, never a symlink itself. `reclaimed_bytes += dir_size_bytes` (computed before delete; symlinks count as 0).

**StaleStagedTrainer** (`cleanup_stale_staged_trainer`, `:106-194`):

1. `target_path.exists()` else "no longer exists".
2. `staged_root = <resolved_prefix>/drive_c/CrossHook/StagedTrainers` must exist and **not be a symlink** (`symlink_metadata`; an error is treated as a symlink) ("refusing cleanup").
3. canonicalize(staged_root) and canonicalize(resolved_prefix); staged_root must start with the prefix ("escapes resolved prefix path").
4. canonicalize(target); it must `starts_with(canonical_staged_root)` ("outside staged trainers root").
5. Delete the canonical target with `remove_dir_all` or `remove_file` according to `is_dir()`.

**Gap in the symlink invariant (stale-trainer kind)**: step 4 compares the **canonicalized** target, so an entry that was swapped for a symlink to _another file inside the staged root_ would pass the root-containment check and delete the **pointee** (a different entry). The target's own `symlink_metadata` is never checked. The scan skips symlink entries (`staged_trainers.rs:41-43`), so a symlink swapped in **after** the scan is exactly the TOCTOU case. The acceptance criterion "rejects target that became a symlink after scan" is satisfied for orphan prefixes by step 6, but **not guaranteed for stale staged trainers** when the symlink points inside the staged root. Recommend: `symlink_metadata(target).is_symlink()` → skip with reason, in both kinds. For a symlink to outside the staged root, step 4 already rejects it.

### 8.3 Tests for storage today: **none**

- `core/storage/` has no `#[cfg(test)]` module and no tests directory; `grep` for `cleanup_prefix_storage`/`scan_prefix_storage` finds only source files plus the frontend hook and mock (`src/lib/mocks/handlers/system.ts:602-620`, which suppresses cleanup in browser mode). The metadata tables `prefix_storage_snapshots`/`prefix_storage_cleanup_audit` are exercised only by migration tests (`core/metadata/migrations/tests/v11_v20.rs`).
- Neither the allow-list filter, the dedupe, the audit writes, nor the history response has any test.

All four acceptance tests in criterion 1 are therefore **new**:

1. cleanup rejects target missing from the fresh scan → needs the service-level allow-list filter test (use a tempdir prefix under a fake compatdata layout with `drive_c/CrossHook`, profile references via `ProfileStore::with_base_path`).
2. cleanup rejects a target that became a symlink after scan → scan, then replace the target by a symlink, then cleanup. Add one case per kind (orphan prefix and staged trainer), including a symlink that points **inside** the staged root (see the §8.2 gap).
3. cleanup rejects paths outside prefix roots → a forged target with `resolved_prefix_path` and `target_path` outside any scanned prefix; also a forged `target_path` that differs from `resolved_prefix_path` for an orphan prefix, and a `..` escape.
4. cleanup writes audit rows for accepted targets → `MetadataStore::open_in_memory()` plus `list_prefix_storage_cleanup_audit`. Cover deleted, skipped, and summary (including the `reclaimed_bytes` summary condition) rows. Remember a disabled store silently drops audit rows (`with_conn_mut`); decide whether audit failure must block cleanup (today: never blocks).

Test seams: `ProfileStore::with_base_path`, `MetadataStore::open_in_memory`, `tempfile`. `has_crosshook_managed_marker` requires the `drive_c/CrossHook` directory to be present in the fixtures. `normalized_path_string` canonicalizes, so tempdir paths on symlinked `/tmp` roots must be canonicalized in expectations (`/tmp` → `/private/tmp` on macOS is irrelevant on Linux CI, but use `fs::canonicalize` for stability).

`scanned_at` is wall-clock; the snapshot id embeds it (`"<path>-<scanned_at>"`), so two scans in the same instant can collide. `INSERT OR REPLACE` hides that. Keep or note.

---

## 9. Where each command's work happens today (blocking inventory, for criterion 4)

| Command group       | Async?                                                                                                                  | Blocking work on the caller                                                                                                                                    |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `storage` (3)       | `async` + `spawn_blocking`                                                                                              | none (already off-thread)                                                                                                                                      |
| `mods` (6)          | `async` + `spawn_blocking`                                                                                              | none, but metadata `is_available()` and several store calls are inline in the async fn (cheap)                                                                 |
| `collections` (13)  | **all sync**                                                                                                            | SQLite calls; `export_to_toml` does file I/O and a profile load per member; `import_from_toml` reads and classifies all profiles                               |
| `export` (12)       | **all sync**                                                                                                            | `export_launchers` writes 2 files; `list_launchers` scans the dir **and loads every profile** for stale flags; `reexport_launcher_by_slug` loads every profile |
| `version` (4)       | **all sync**                                                                                                            | manifest/library discovery, trainer hash (`fs::read` of the whole file), SQLite                                                                                |
| `steam` (3)         | `list_proton_installs` and `default_steam_client_install_path` **sync**; `auto_populate_steam` async + `spawn_blocking` | `list_proton_installs` scans the Steam library dirs on the caller thread                                                                                       |
| `game_metadata` (4) | `fetch_*` async (network only; the SQLite cache read/writes are inline)                                                 | `import_custom_*` are **sync** with `fs::read`/`fs::write`                                                                                                     |

`services/mod.rs` already provides `run_blocking(work)` (private, `services/mod.rs:46-52`) for services, converting join errors into sanitized `AppError`. A worker-thread assertion for criterion 4 has precedent: look at `crates/crosshook-app/src/app/tests.rs:27` (thread-waker) and the current-thread runtime in `tests/profile_writer_golden.rs:223`. A direct test would run the service on a current-thread runtime and assert the caller thread is not blocked, e.g. an async test that yields while a gated blocking closure runs.

---

## 10. Gap table vs. the five acceptance criteria

| #   | Criterion                                                                                                                                                     | State today                                                                                                                                               | What's missing / risk                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Storage tests: reject target missing from fresh scan; reject target that became a symlink; reject paths outside prefix roots; audit rows for accepted targets | **Behaviour already exists** in `tauri/commands/storage.rs:65-139` and core `cleanup.rs`, but there are **zero tests**                                    | All four tests are new. (a) Unknown targets are silently dropped (no skipped/audit entry); decide on "reject" semantics. (b) Stale-trainer symlink swap to an in-root pointee is not rejected (§8.2); add a `symlink_metadata` check. (c) Audit insert errors are warn-only; a disabled store drops them silently. (d) Scan is not yet a job (`JobRegistry::start`), and cleanup must keep rescanning before deleting. (e) `inventory_incomplete` ⇒ no orphan targets must stay.                                                                |
| 2   | `apply_collection_defaults` merge table tests carried over or added                                                                                           | 5 shell tests (`tauri/.../tests/optimizations.rs`) + 4 `profile_layers.rs` + 1 `unknown_fields.rs` + 1 `load_save.rs` + 5 `collections_defaults_tests.rs` | Carry the 5 shell tests over with the helper (they use the `pub(super)` helper and `with_sqlite_conn`). Add the missing merge rows listed in §1.6 (network_isolation true/false, trainer_gamescope, mangohud, optimizations, mixed, empty env, NULL defaults through helper, non-Validation fail-open, disabled store). Decide on the new-profile-flow entry point (no caller exists today).                                                                                                                                                    |
| 3   | Launcher script and `.desktop` golden tests, byte-identical                                                                                                   | **No golden files exist**; only substring assertions (§5.6)                                                                                               | Capture goldens from the pre-move code (script for `steam_applaunch` and `proton_run`, with/without gamescope, network isolation, umu, custom env, `copy_to_prefix`/`source_directory`; desktop with and without icon; both previews with headers). Pin `HOME`, use fixed fake paths; avoid icon existence. Follow `launch_request_golden.rs`/`profile_writer_golden.rs`. Also pin file modes 0o700/0o644 (exists in `export_write.rs`).                                                                                                        |
| 4   | `list_proton_installs` and `export_launchers` no longer block the caller thread                                                                               | Both are plain sync `fn`s (`steam.rs:35-48`, `export.rs:194-212`)                                                                                         | Service methods must be `async` with `run_blocking`; Tauri commands become `async` and `.await` them. Add a non-blocking test (precedent in §9). Also move `list_launchers`/`reexport_launcher_by_slug` (profile-iterating) off the caller thread.                                                                                                                                                                                                                                                                                              |
| 5   | Tauri commands delegate; Library, collections and Settings storage smoke tests pass                                                                           | Commands contain the logic                                                                                                                                | Thin delegating commands; IPC arg/return names unchanged (§0.2); add `command_names_match_expected_ipc_contract`-style signature pins like `version.rs:500-518`/`export.rs:454-538`. Smoke tests are frontend (Vitest) plus the Rust suites; there are no Tauri-level behaviour tests for these commands, so service tests carry the load. Callers that must be rewired: `protonup.rs:168`, `migration.rs:14,35` (`default_steam_client_install_path`), and `lifecycle.rs:9,33,52` (`apply_collection_defaults`/`resolve_collection_defaults`). |

---

## 11. Decisions the plan needs (business-relevant, cannot be derived from code)

1. **Unavailable semantics per domain.** Collections: add an explicit guard and return `AppErrorKind::Unavailable` (today they silently no-op, and create returns `""`). Mods: keep `list` returning `available:false`, and the others `Unavailable` (map to the legacy `metadata_unavailable:` prefix in the Tauri shell to keep `useProfileMods` working). Version `set`/`ack`: return `Unavailable` instead of the misleading "not registered". Storage history: keep `available` + empty lists. Mods `detect` stays working without the DB.
2. **Error text compatibility.** `From<MetadataStoreError>` drops `Validation` text. Decide the mapping so "a mod named … is already registered", "mod not found", "profile not found when adding to collection: <name>" and "collection not found" stay user-visible (the mock handlers and `ModDetectionReviewModal` test mirror them).
3. **Storage cleanup rejection reporting** (silent drop vs `skipped` + audit row) and **the stale-trainer symlink check** (§8.2). Both are additive to the IPC `PrefixCleanupResult`.
4. **Scan as a job vs plain awaitable.** The existing hook awaits the scan result directly. The job registry (`JobRegistry::start`, `JobKey` uniqueness → `Conflict`) is available. A job key like `"prefix-storage-scan"` would prevent concurrent scans, but a job needs event plumbing; the classic frontend would keep calling a command that returns the result. Same question for `list_proton_installs`.
5. **Offline art semantics**: a new `Unavailable` state for art versus the current `Ok(None)`; whether to consult `settings.offline_mode`; keep `fetch_game_cover_art` returning `Option<String>` at the IPC layer.
6. **New-profile flow entry point** for `apply_collection_defaults` (no caller exists in the repo).
7. **Collections TOML import commit**: stay frontend-orchestrated (current), or provide a transactional service operation (non-goal-sensitive: "collections TOML unchanged").
8. **`rename_launcher` request loss** (umu/runtime fields reset to defaults) and the **settings-failure inconsistency** across `check_launcher_for_profile` (hard error) vs stale flags/slug matching (soft fallback): preserve or fix.
9. **`output_path`/`source_path` trust**: both come from the renderer. The service should keep existing validation; consider documenting the native-dialog requirement.
10. **File-size bar** (CLAUDE.md "~500 lines"): `export.rs` is 576 lines and holds 12 commands and the stale/slug logic. Split the launcher service into `launcher_export/{mod,request,stale,reexport}.rs` plus tests, and keep each service module under the bar. Version, mods, collections and steam are all under it.
