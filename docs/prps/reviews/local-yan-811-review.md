# Local pre-PR review — YAN-811 (refactor/yan-811-profile-writer vs main)

Reviewers: correctness, security, quality (parallel). Verdicts: APPROVE / APPROVE WITH NOTES / APPROVE. CRITICAL 0, HIGH 0.

## CorrectnessReviewer

I found no CRITICAL or HIGH issues. The commit pipeline holds up: the TOML is written once, effects run in order (observe, then revision, then health invalidation), the event is published inside the blocking closure, and nothing is published on a failure, a same-name rename or `AlreadyValid`/`Failed`. Rollback verifies SHA-256 and HMAC before any write. The golden tests do drive the real writer methods (`save`, `rename`, `duplicate`, `config_rollback`). There are 2 MEDIUM and 5 LOW gaps, mostly about writes that still happen outside the D3 lock.

**Findings**

- [SEVERITY MEDIUM] src/crosshook-native/src-tauri/src/commands/lutris.rs:31 — Lutris TOML writes happen outside the D3 write lock. — `apply_lutris_import` calls `store.save` for each entry on a plain `spawn_blocking`. Only the later `writer.record_imports` (imports.rs:22) takes the lock, and it only runs effects. The old command was sync, so it was serialized with every other profile command on the main thread. Now a `writer.save` creating the same profile name can interleave between the `store.list()` snapshot (`reserved_names`) and `store.save`, and the losing write is silently overwritten. Also, if the command future is dropped between the two awaits, the TOML stays written but its observe, revision and `Imported` event are lost, which breaks D4. Lastly,…

- [SEVERITY MEDIUM] src/crosshook-native/src-tauri/src/commands/community.rs:104-127 — Community import lost its serialization with other profile writes. — The command is still sync and runs `store.save` plus `observe_profile_write` on the main thread. Every other write now runs on the blocking pool under the writer lock, so the old "all profile commands serialized on the main thread" guarantee is gone for this path. This is a new race against autosave and full save on a profile with the same name. The crate README (lines 119-121) also claims "shells never call `ProfileStore` mutators, metadata observe…", which this path contradicts. — Fix: D10 keeps community import out of scope, so at minimum make the command async and run its write inside a writer-locked …

- [SEVERITY LOW] src/crosshook-native/crates/crosshook-app/src/services/profiles/presets.rs:~211 — Preset apply hard-fails after the TOML is already written. — `let updated = ctx.profiles.load(&name)?;` runs after `ctx.profiles.save`. A reload failure returns `Err`, which skips the effects and the `PresetApplied` event even though the file changed (against D4). `LaunchSelectionSave` (sections.rs) handles the same case fail-soft. — Fix: on reload failure, fall back to the in-memory `profile` used for the save (or skip the effects with a warning), and still return `names: Some(..)`.

- [SEVERITY LOW] src/crosshook-native/crates/crosshook-app/src/services/profiles/migration.rs:~195-205 vs docs/plans/profile-writer/feature-spec.md:165 — Migration records no config revision, but the spec's mapping says it should. — The spec maps migration to `Migrated`/`Migration`, but `apply_one` queues only `ObserveWrite` and `InvalidateHealth`. The plan (Task 1.5) and the research inventory describe "no revision", so the docs contradict each other. With no revision, history and diff treat the pre-migration snapshot as the latest, and rolling back to it silently restores the stale Proton path. — Fix: either push `Effect::Revision { source: ConfigRevisionSource::Migration, .. }` after `ObserveWrite`, or amend feature-spec.md:165 to say migration has no rev…

- [SEVERITY LOW] src/crosshook-native/src/hooks/useProfileHealth.ts:243-249 together with sections.rs (all section and optimization saves publish `Saved`) — Every debounced autosave now triggers a full health batch. — Each MangoHud, gamescope, optimization or command-argument autosave runs a full `batch_validate_profiles` (which rewrites every `health_snapshots` row) and sets health `status: 'loading'`, so the dashboard can flicker. Research flagged this as a new cost; these fields cannot affect health. This follows the spec mapping, so it is a consequence of the design rather than a bug. — Fix: if this cost isn't intended, have `profileChangeAffectsHealth` skip section saves. That needs a distinct reason or kind, and changing D1/D13 is your decision.

- [SEVERITY LOW] src/crosshook-native/src-tauri/src/commands/profile/lifecycle.rs:~23-31 — `profile_load` does a blocking SQLite read on the async runtime. — `apply_collection_defaults` (a `get_collection_defaults` SQLite call) runs directly in the async command after `writer.load(...).await`. `profile_list_summaries` correctly moves the same lookup into `run_blocking` through its resolver closure. — Fix: wrap the collection merge in `tauri::async_runtime::spawn_blocking`, or pass a resolver into a `ProfileWriter::load_with` the same way summaries do.

- [SEVERITY LOW] src/crosshook-native/crates/crosshook-app/src/services/profiles/lifecycle.rs:169-196, 209-230 (and sections.rs `SectionSave`) — Event names and observe paths use the raw, untrimmed name. — `SaveProfile`, `DeleteProfile` and `SectionSave` never validate or trim `name`, but `ProfileStore` trims it on write. The event's `names`, `ObserveWrite`'s `<name>.toml` path and the health lookup therefore use the untrimmed string, while the file on disk is trimmed. Observe and revision behave as before, but the event payload is new. The frontend trims today, so this can't currently be reached from the UI. — Fix: validate up front with `ProfileStore::validate_profile_name`, as `rename` and `set_favorite` already do, and use the result everywhere.

**Checked and found correct**
- **Writer lock (D3) and deadlocks:** the lock is held only inside `run_blocking` and never across an `.await`. No commit calls another facade. The settings and metadata locks are only taken while the writer lock is held, never the other way round. Favorite, rollback, migration and `record_imports` all take the lock (the Lutris write itself does not; see the first finding).
- **Rename:**
  - Launcher cleanup runs on the pre-rename profile and returns "files deleted".
  - The display-name rewrite uses `save`, which keeps the companion and the golden bytes.
  - `last_used_profile` is written only when it pointed at the old name.
  - Effects run in the order `ObserveRename`, then the revision for the new name, then health.
  - A same-name rename does nothing, which the frontend's own guard already assumes.
- **Delete:** health invalidation runs before the soft delete, and cleanup is best-effort.
- **Duplicate:** the source profile id is looked up for lineage before the copy, `SyncSource::AppDuplicate` is used, and the TOML is written once.
- **Migration:** the replacement path is re-checked under the lock, the batch preflight loads every profile before any write, `save_without_companion` is used, and the event carries only applied names.
- **Section saves:** extras come only from disk (clear, then preserve from the loaded copy).
- **Events (D1):** the Rust enum, the bridge payload and its Resync form, `profileEvents.ts`, all five listeners and every mock emit site match D1, including the mock same-name rename (no event, returns `false`).

## Review Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 0     | pass   |
| HIGH     | 0     | pass   |
| MEDIUM   | 2     | info   |
| LOW      | 5     | note   |

Verdict: APPROVE WITH NOTES. Nothing blocks the merge. The Lutris write outside the lock should be fixed first, because it is the one real gap in D3/D4 coverage.

[Some lines truncated to 768 chars. Use agent://CorrectnessReviewer:raw:5-5 to read line 5 whole; cut lines run to line 11, each read the same way]

## SecurityReviewer

I found no CRITICAL or HIGH issues. I found 1 MEDIUM and 4 LOW gaps, all in the areas you asked about. Rollback checks SHA-256 and HMAC before it writes, and the rest of your checklist came out clean (list at the end).

**Findings**

- [SEVERITY MEDIUM] `crates/crosshook-app/src/services/profiles/lifecycle.rs:163-198` (`SaveProfile::persist`), `:211-235` (`DeleteProfile`), `:238-269` (`DuplicateProfile`, untrimmed source name in `lookup_profile_id`), `sections.rs:285-299` (`SectionSave`), `migration.rs:196-211` (`apply_one`) — **problem:** these facades never validate or trim the profile name before the steps that use it. Only `rename`, `set_favorite` and `export_toml` do.
  - **Evidence, effects:** the store trims names internally (`profile_path` → `validate_name`), so `" Alpha "` writes `Alpha.toml`. The untrimmed string then goes into `Effect::ObserveWrite` (`writer.rs:94-96` builds the path `" Alpha .toml"`), `Effect::Revision`, `Effect::InvalidateHealth` and the event `names`. In core, `observe_profile_write` (`metadata/profile_sync.rs:19,52-53`) checks the trimmed name but stores the raw one as `current_filename`. That creates a second metadata row for the same file, so revisions and health end up on the wrong `profile_id`.
  - **Evidence, delete:** `observe_profile_delete(" Alpha ")` matches no row, so the soft delete does nothing and the removed profile stays in favorites.
  - **Evidence, save:** a name that would fail validation makes `profile_exists` return false. The save then runs the new-profile branch and `auto_import_custom_art`, which copies files into the managed media directory, before `ctx.profiles.save` rejects the name. Those copies are left orphaned.
  - **Severity:** the same behaviour existed in the old Tauri commands, and the shipped React callers trim. But this is now the toolkit-independent API the native UI will call, and D2 requires "validate → load".
  - **Fix:** at the top of each facade (`save`, `delete`, `duplicate`, the three section saves, `import_legacy`, migration items, ProtonDB accept), do `let name = ProfileStore::validate_profile_name(&name)?;` and pass the trimmed name everywhere. `required_profile_name` in `sections.rs` and `suggestions.rs` should call `validate_profile_name` instead of only checking for blank. Add tests for a name with surrounding spaces and for `../x`, checking that no art is copied and no event fires.

- [SEVERITY LOW] `src-tauri/src/commands/lutris.rs:48` — **problem:** `writer.record_imports(imports).await.map_err(map_error)?` turns a pipeline failure into a command `Err`, even though `apply_lutris_import` has already written every TOML.
  - **Evidence:** `record_imports` can only fail with a `JoinError` (cancellation at shutdown, or a panic in effects). Before, these effects were fail-soft and never failed the command. Now the UI reports the import as failed while the files exist, and a retry creates duplicate `(2)`-suffixed profiles.
  - **Fix:** log the error with `tracing::warn!` and still return `Ok(result)`.

- [SEVERITY LOW] `crates/crosshook-app/src/services/profiles/writer.rs:69-76` together with `writer.rs:148-149` → `revisions.rs:26-31` → `crosshook-core/src/settings/store.rs:99` (`.expect("settings mutex poisoned")`) — **problem:** a panic inside `run_effects` happens after the TOML is saved but before `publish`.
  - **Evidence:** the caller gets "background task failed" for a write that persisted, and no `profiles-changed` event is sent. That breaks D4's "publish after primary persistence even if advisory effects fail". The write lock itself is recovered correctly.
  - **Fix:** run the advisory effects inside `std::panic::catch_unwind(AssertUnwindSafe(|| run_effects(..)))`. On a panic, record a warning and still publish.

- [SEVERITY LOW] `crates/crosshook-app/src/error.rs:104-107` (`ProfileStoreError::TomlDe/TomlSer => error.to_string()`) and `services/profiles/history.rs:200-204` (rollback parse error interpolates `{error}`) — **problem:** TOML parse errors quote the offending source line, which can contain user paths outside `$HOME`. `sanitize_message` only hides `$HOME`, so those paths reach the UI over IPC.
  - **Evidence:** this goes against the policy written in the same file for `SettingsStoreError` ("Fixed strings only: TOML Display echoes source lines, which hold user paths that may live outside `$HOME`").
  - **Fix:** use fixed messages such as `"profile file is not valid TOML"` and `"failed to parse snapshot for revision {id}"`, and log the full error with `tracing::warn!`.

- [SEVERITY LOW] `src-tauri/src/commands/lutris.rs:31` and `src-tauri/src/commands/community.rs:104-114` — **problem:** these two write paths save profile TOML outside the writer lock (D3).
  - **Evidence:** writer commands now run on the blocking pool instead of the main thread, while `community_import_profile` is still a sync command calling `store.save` (it overwrites an existing name). An autosave on the pool that loads before an import of the same profile and saves after it silently discards the imported content. Meanwhile metadata records the imported content's hash. The old main-thread ordering prevented this.
  - **Fix:** run the Lutris TOML writes under the writer lock, for example as a `ProfileMutation` that calls `apply_lutris_import` inside `persist`. For community import, either route it through the writer or document it as an accepted race and track it as an M1 follow-up.

**Checked, no findings:**
- Rename validates and trims both names before calling commit, and checks that `accepts` matches `from == old_name`.
- Favorite validates the name before taking the lock, and returns `Unavailable` when metadata is disabled, with no event.
- The D12 export header uses `validate_profile_name`, which rejects control characters, so newline injection is blocked; a test covers it.
- Launcher cleanup still goes through core's `delete_launcher_for_profile`, which only deletes regular files carrying the CrossHook watermark, skips native profiles, and is best-effort on both delete and rename.
- Rollback looks up the revision scoped to the profile, then runs `verify_config_revision` (SHA-256 plus HMAC), and only then parses and saves.
- ProtonDB accept re-runs `validate_env_suggestion` on every catalog pair and on the raw env pair before loading the profile.
- The write mutex recovers from poisoning.
- `JoinError`s are mapped to sanitized errors.
- The event payload carries only `reason` and `names`; the bridge resync emits `saved` with `names: []`, and a malformed payload in TS is treated as a resync.
- Mocks load only through the `__WEB_DEV_MODE__` dynamic import, and production `profileEvents.ts` does not import them.
- Migration uses `save_without_companion`, which serializes the same way as the old `write_atomic` path, so the bytes are identical.
- With metadata disabled, saves still succeed, and observe, revision and health are skipped with a warn log plus a `MetadataUnavailable` warning. History, diff, rollback, known-good and bundled presets fail closed, and a test covers the banner.

## Review Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 0     | pass   |
| HIGH     | 0     | pass   |
| MEDIUM   | 1     | info   |
| LOW      | 4     | note   |

Verdict: APPROVE. No CRITICAL or HIGH issues. The MEDIUM name-validation gap is worth fixing before the native UI starts calling the service.

## QualityReviewer

I found no CRITICAL or HIGH issues. The architecture is sound: `src-tauri` profile commands now only delegate to the writer, and collection composition stays in the shell as D7 allows. Every service file is under about 420 lines and has module docs. The golden tests seed from the frozen fixtures and compare raw bytes. The diff tests check the byte cap, the 500-change semantic cap and the fallback to unified. Rename and delete clean up launchers end to end, and the Vitest favorite test drains timers and has a `saved` positive control.

What remains is a performance and UX regression from the newly emitted `saved` events, rule text that overstates the cutover, and a few test and dead-code gaps.

- **[MEDIUM]** `src/crosshook-native/crates/crosshook-app/src/services/profiles/sections.rs:30-122` and `src/crosshook-native/src/hooks/useProfileHealth.ts:117-136,243-249` — Autosaves now each trigger a full health batch. Problem and evidence:
  - The optimization, gamescope, trainer-gamescope, MangoHud and command-argument saves were silent before (research-business table, rows 10–13 and 17). They now publish `Saved`.
  - The frontend autosaves these sections on a 350 ms debounce (`hooks/profile/constants.ts:2`). Every autosave therefore runs a full `batch_validate_profiles` (filesystem checks, Steam discovery and a rewrite of every health snapshot). It also runs `profile_list`, two summary refetches and the community name rescan, and sets the health `loading` status, which shows the Dashboard's "Checking…" indicator.
  - research-business.md:262 flagged this ("would add health batches that do not happen today").
  - The backend itself treats these saves as health-neutral: `sections::write_effects` adds no `InvalidateHealth`.
  - `batchValidate` is not single-flight. Overlapping calls can finish out of order and leave an older summary on screen. Existing direct calls add more overlap: `useProfileActions.ts:185,191` (`revalidateSingle` after save and rollback) and `useProtonMigration.ts:52,75`.
  - **Fix:** without changing D13, make the `profiles-changed` health path single-flight with a trailing coalesce: one in-flight batch, one pending re-run, and a request counter that drops stale `batch-complete` results. Also remove the now-redundant direct `revalidateSingle`/`batchValidate` calls after save, rollback and migration.

- **[MEDIUM]** `CLAUDE.md:32`, `AGENTS.md:45`, `.cursor/rules/project.mdc:21` and `crates/crosshook-app/README.md:117-123` — The rule text overstates what the code does. Problem and evidence:
  - The bullet says every mutation, including "imports", goes through `ProfileWriter`. It also says not to call `ProfileStore` mutators or metadata observe from `src-tauri`.
  - Two shell commands still do exactly that, as D10 allows:
    - `src-tauri/src/commands/community.rs:113-128` writes via core `import_community_profile` and calls `metadata_store.observe_profile_write`. It takes no writer lock and emits no event.
    - `src-tauri/src/commands/lutris.rs:31` calls `apply_lutris_import(&store, …)` (`ProfileStore::save`) outside the D3 write lock.
  - The README also says every commit invalidates the cached health snapshot. Section saves, presets and ProtonDB accept do not (`sections.rs:141-159`, `presets.rs:215`, `suggestions.rs:155-176`).
  - **Fix:** name the sanctioned exceptions (community import unchanged; Lutris writes in core, then `record_imports`) and the health-invalidation scope in all four documents. Alternatively, route both imports through the writer.

- **[MEDIUM]** `crates/crosshook-app/src/services/profiles/tests/{lifecycle,events}.rs` and `src/components/pages/health-dashboard/useHealthDashboardState.ts:46-57` — Several required behaviours have no tests. Evidence:
  - **Duplicate:** D9 makes its revision capture and `Duplicated` event new, but no service test checks the event, the revision row, or `AppDuplicate` lineage. Only the golden bytes are covered.
  - **Rename revision:** the test checks the `Renamed` event but not that a revision is captured.
  - **Legacy import and `record_imports`:** these facades have no tests at all.
  - **Dashboard overlay (D13):** no test for `overlayFavorites` or for the dashboard star and favorite-first ordering updating without a health call.
  - **Other listeners:** the favorite-skip paths in `useProfile`, `useLibrarySummaries` and `useCommunityProfiles` are untested. Only `useProfileSummaries` and `useProfileHealth` have tests.
  - **Rust favorite test:** the feature-spec requires a Rust call counter. The Rust test only checks the event and that the cached health row survives.
  - **Fix:** add these service tests:
    - duplicate: copy-name event, revision row, source lineage;
    - legacy import and `record_imports`: event names, `Import` revision, health invalidation;
    - rename: a revision exists under the new name.
    
    Add a Vitest test of `overlayFavorites` plus a dashboard render test where a favorite change makes zero `batch_validate_profiles` calls, and one favorite-event case for each remaining listener.

- **[LOW]** `crates/crosshook-core/src/profile/toml_store/store.rs:138-206,350` and `crates/crosshook-core/src/profile/migration/apply.rs:57-80` — Dead write paths remain in core. Problem and evidence:
  - `save_launch_optimizations`, `save_command_arguments`, `materialize_launch_optimization_preset`, `save_manual_launch_optimization_preset`, the public `unique_copy_name` and `apply_single_migration` are now called only by tests. The service never calls them.
  - They remain public profile-write paths that skip the writer lock, revisions and events.
  - `services/profiles/migration.rs:185-232` (`apply_one`) re-implements the load → prepare → `save_without_companion` steps of `apply_single_migration` instead of reusing them.
  - **Fix:** remove the unused wrappers and move their tests onto the `GameProfile::apply_*` rules. Alternatively, have `apply_single_migration` return the mutated profile and call it from `apply_one`.

- **[LOW]** `crates/crosshook-app/src/services/profiles/launcher_cleanup.rs:10-20` — The Steam-root rule is now copied three times. Evidence:
  - The same compatdata derivation exists in `services/health/steam.rs:7-16` and `src-tauri/src/commands/version.rs:92-99`.
  - The doc comment in `health/steam.rs:7` points at `commands/profile.rs::derive_steam_client_install_path`, which no longer exists.
  - **Fix:** expose one `pub(crate)` helper in `crosshook-app` (or core) and use it in all three places.

- **[LOW]** `crates/crosshook-app/src/services/profiles/types.rs:89-90` — `ImportedProfileWrite.path` is dead data. Lutris fills it in (`lutris.rs:40-44`), but nothing reads it; `imports.rs:44-46` derives the path again from the name.
  - **Fix:** remove the field.

- **[LOW]** `crates/crosshook-app/src/services/profiles/lifecycle.rs:361-375` and `revisions.rs:26-31` — A poisoned settings mutex can break D4. Problem and evidence:
  - `SettingsStore::update` and `load` call `.expect("settings mutex poisoned")` (`crosshook-core/src/settings/store.rs:99,140`).
  - Both calls run after the TOML is persisted but before `publish`: the rename's last-used move and the revision-cap lookup.
  - If they panic, the write persists, the caller gets `Internal`, and no `profiles-changed` is sent.
  - **Fix:** run the fail-soft settings steps with poison recovery, or publish before any post-persistence step that can panic.

- **[LOW]** `crates/crosshook-app/tests/profile_writer_golden.rs:29-33` — `std::env::set_var("HOME", …)` runs inside `LazyLock` while other parallel test threads may read the environment through `tempdir` or `derive_target_home_path`. That is a data race and could cause rare flaky crashes.
  - **Fix:** give each test its own launcher home by putting a Steam compatdata path in the seeded profile, or force the tests to run serially.

## Review Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 0     | pass   |
| HIGH     | 0     | pass   |
| MEDIUM   | 3     | info   |
| LOW      | 5     | note   |

**Verdict: APPROVE.** The autosave health-batch regression is the one I'd fix before release.

