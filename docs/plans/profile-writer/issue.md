# YAN-811 / GitHub #532 — M1-06

Target release: crosshook v0.7.0-alpha.1 (trunk-only, base `main`).

*Paths are relative to* `src/crosshook-native/`*.*

## Context

Every profile write repeats the same sequence:

1. validate
2. load
3. mutate
4. save
5. `observe_profile_write`
6. `capture_config_revision` (TOML serialize, SHA-256, dedupe)
7. invalidate the health snapshot
8. emit `profiles-changed`

The sequence is spread across `src-tauri/src/commands/profile/*` (23 commands: lifecycle, shared, optimizations, config_history, command_arguments, favorites), `protondb.rs::protondb_accept_suggestion` and the migration apply path.

Today a favorite toggle emits a reason string with no name. That makes five listeners rescan everything, including a main-thread `batch_validate_profiles`.

## Scope

* Add `crates/crosshook-app/src/services/profiles/`, built around one `ProfileWriter::commit(name, mutation, reason)` pipeline. All of these use it:
  * save, delete (with launcher cleanup), duplicate, rename, favorites;
  * the optimizations, MangoHud, gamescope, trainer-gamescope and command-args saves;
  * bundled and manual presets;
  * config history, diff, rollback (with lineage) and known-good marking;
  * new-profile defaults from settings;
  * legacy import and TOML export;
  * list summaries;
  * custom-art auto-import;
  * re-validation of env suggestions;
  * ProtonDB accept and Proton migration apply.
* `AppEvent::ProfilesChanged { reason: ProfileChangeReason, names }`, with reasons `Saved`, `Deleted`, `Renamed { from }`, `Duplicated`, `FavoriteToggled`, `RolledBack`, `PresetApplied`, `Imported` and `Migrated`.
  * Tauri emits `profiles-changed` with an object payload.
  * The five React listeners and the mocks are updated so health re-validation is skipped for reasons that cannot affect health, such as `FavoriteToggled`.
* Blocking store and diff work runs in `spawn_blocking` inside the service, and every public fn is async.
* Collections and `apply_collection_defaults` are **not** in this issue; they move in M1-35.

## Storage boundary

* TOML profiles stay byte-compatible.
* The SQLite config-revision rows and the health snapshot table are unchanged (schema version unchanged).
* Change events are runtime-only.

## Persistence & usability

* No migration.
* Offline behaviour is unchanged.
* With `MetadataStore::disabled()`, saves still succeed, revisions are skipped, and a warning is logged and surfaced.
* Users still edit profiles in TOML or the UI exactly as before.

## Acceptance criteria

- [ ] Golden-file tests show that save, rename, duplicate and rollback produce byte-identical TOML compared with the pre-refactor fixtures.
- [ ] Revision SHA-256 dedupe, the diff output cap and semantic diff are preserved and tested.
- [ ] A favorite toggle triggers no `batch_validate_profiles`, asserted by a call counter in tests and by a Vitest listener test.
- [ ] Rename and delete keep the launcher cleanup and re-export behaviour, with tests.
- [ ] Tauri profile commands delegate to the service, and React smoke passes.

## Test plan

* `cargo test -p crosshook-app`, with tempdir stores and the metadata DB both enabled and disabled
* `npm test`
* `npm run test:smoke`

---

**Plan key:** `M1-06` · **Release:** v0.7.0-alpha.1 · **Estimate:** 8 pts · **GitHub labels:** `type:refactor`, `area:profiles`, `priority:high` · **Blocked by:** M1-02
