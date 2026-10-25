# M1-06 / YAN-811 — Security review: moving profile mutation, revision and rollback into `crosshook-app`

Review of the planned `crates/crosshook-app/src/services/profiles/` `ProfileWriter::commit(name, mutation, reason)` pipeline (issue: [issue.md](issue.md)). All paths are relative to the repo root of this worktree (`refactor/yan-811-profile-writer`); code lives under `src/crosshook-native/`.

**Method**: read-only code audit of the 23 profile commands (`src-tauri/src/commands/profile/{lifecycle,shared,optimizations,config_history,command_arguments,favorites}.rs`), `src-tauri/src/commands/{protondb,migration,lutris,community}.rs`, `crates/crosshook-core/src/profile/` (toml_store, legacy, migration, exchange), `crates/crosshook-core/src/export/launcher_store/`, `crates/crosshook-core/src/metadata/` (config_history_*, profile_sync), `crates/crosshook-core/src/game_images/`, `crates/crosshook-core/src/fs_util.rs`, `crates/crosshook-app/src/{services,events,error,presenter}`, the five frontend `profiles-changed` listeners, and `src/lib/mocks/`. ADR-0005 was used as the HMAC reference. No builds or tests were run (read-only research).

---

## Executive Summary

No CRITICAL findings. Six WARNING and ten ADVISORY items; see Findings.

## Dependency Security

No new dependencies are introduced; the refactor reuses existing workspace crates.

## 1. Threat model and trust boundaries

| Boundary | Input crossing it | Sanitizer today |
|---|---|---|
| Webview → Tauri IPC | profile `name`, `GameProfile` payload, file paths (import/export), revision ids | `ProfileStore::validate_name` (per store op); `validate_steam_app_id` only in `profile_save`; env pairs only in `protondb_accept_suggestion` |
| Tauri IPC → Rust core | same, plus `State<ProfileStore/MetadataStore/SettingsStore>` | core store functions re-validate (`profile_path()` validates every name) |
| Disk (`.toml`, `metadata.db`, HMAC key) → core | profile TOML, revision snapshots, legacy `.profile` files | TOML parse + `verify_config_revision` (SHA-256 + HMAC) before rollback writes |
| Rust → webview (events, results, errors) | `profiles-changed` payload, command results, `Err(String)` | `sanitize_display_path` in migration only; **most profile errors stringify raw** |

In-scope attacker (per repo policy and ADR-0005): malicious/corrupted input files and a compromised webview invoking commands with arbitrary arguments — **not** an attacker with arbitrary same-user filesystem access (out of scope by ADR-0005 § Threat model).

**CRITICAL findings: none.** The name-validation and watermark-gated deletion cores are sound. The findings below are hardening gaps and refactor-sensitive invariants; several become real weaknesses if the service move drops or reorders the existing checks.

---

## 2. Findings

### CRITICAL

None found. Specifically checked and **cleared**:

- **Path traversal via profile names — cleared.** Every store mutation funnels through `validate_name` (`crates/crosshook-core/src/profile/toml_store/utils.rs:72-109`): trims, rejects empty/`.`/`..`, the `ANONYMOUS_PROFILE_KEY` sentinel, control chars (`char::is_control`), absolute paths, `/`, `\`, `:`, and Windows-reserved characters. `profile_path()` (`toml_store/store.rs:471-474`) validates on **every** load/save/delete; `rename` validates both names (`store.rs:336-338`); `duplicate` validates the generated name (`store.rs:416-433`, `generate_unique_copy_name` re-validates each candidate); `import_legacy` validates the derived stem (`store.rs:383-394`). Metadata ops re-validate via `validate_profile_name` → `Corrupt` error (`metadata/profile_sync.rs:264-267`). The metadata layer uses parameterized SQL throughout (`profile_sync.rs:31-67`, `collections.rs:167-178`) — no injection. **Refactor requirement**: `ProfileWriter::commit` must keep calling the store methods (which validate) and must never build `{name}.toml` paths itself — note that today five call sites already hand-roll `store.base_path.join(format!("{name}.toml"))` for `observe_profile_write` (`lifecycle.rs:203`, `shared.rs:80,260`, `optimizations.rs:83`, `protondb.rs:25`, `migration.rs:70,180`); these take **post-validation** names, but the service should route them through a `profile_path()`-based accessor so the invariant does not depend on call order.
- **Deletion confinement — cleared with invariants (see W-5, A-1).**
- **HMAC rollback verification — cleared (see §3, A-2).**

### WARNING

#### W-1. Legacy import bypasses `validate_steam_app_id` (BR-4)
- **Where**: `src-tauri/src/commands/profile/lifecycle.rs:350-384` (`profile_import_legacy`) → `ProfileStore::import_legacy` (`toml_store/store.rs:383-404`) → `save`. `legacy::load` (`profile/legacy.rs:9-52`) copies `SteamAppId=` verbatim into `steam.app_id` (`profile/models/legacy.rs:37-91`).
- **Risk**: `profile_save` enforces `validate_steam_app_id` (digits-only, ≤12 chars, `profile/models/resolve.rs:57-61`) before any write (BR-4, `lifecycle.rs:111-113`), but a `.profile` file whose `SteamAppId=` line carries arbitrary text is written to disk and indexed without that check. The value later feeds Steam app-id-keyed lookups (ProtonDB, art resolution via `resolve_art_app_id`).
- **Mitigation**: `ProfileWriter::import_legacy` must run the same `validate_steam_app_id(data.runtime.steam_app_id.trim())` check on the converted profile before `save`, mirroring `profile_save`. Add a test importing a fixture with `SteamAppId=../../1245620` and asserting rejection.

#### W-2. `profile_export_toml` interpolates an unvalidated name into the TOML document
- **Where**: `src-tauri/src/commands/profile/lifecycle.rs:388-401` → `profile_to_shareable_toml_with_options` (`toml_store/utils.rs:43-68`), which `format!`s `# CrossHook Profile: {name}\` into the header.
- **Risk**: `name` arrives straight from `invoke()` and is never `validate_name`d. A name containing `\n` (control chars are rejected by `validate_name` everywhere else, but not here) lets the caller prepend arbitrary lines to the returned "valid TOML" — e.g. an extra `[[pre_launch_hooks]]` table that merges with the serialized body when a victim saves/imports the shared file. The function is documented as producing output that "can be saved directly as a `.toml` profile", which makes header injection a real round-trip integrity issue, not just cosmetic.
- **Mitigation**: in the service, `validate_name(name)?` before serializing (rejects control characters including newlines). Cheap, behavior-preserving for every legitimate name. Test: `profile_export_toml` with `name = "x\n[[pre_launch_hooks]]\npath = \"/tmp/evil\""` must error.

#### W-3. Env-var validation is asymmetric between write paths
- **Where**: `protondb_accept_suggestion` re-validates at write time — `validate_env_suggestion(key, value)` (`protondb/suggestions.rs:107-110`: safe key + safe value + `RESERVED_ENV_KEYS`) is enforced for both branches (`src-tauri/src/commands/protondb.rs:126-134, 199-203`). `profile_save` (`lifecycle.rs:103-226`) accepts `data.launch.custom_env_vars` verbatim — no key/value check on the generic save path.
- **Risk**: whatever the ProtonDB path deliberately blocks (reserved env keys) can be written by the generic save path, so the reserved-key list is advisory only. Whether launch-time request validation re-checks env keys is **[INFERENCE: unverified]** — I did not audit `launch/request.rs` env handling; do not rely on it.
- **Mitigation**: centralize a `validate_launch_env(profile)` step inside `ProfileWriter::commit` so every writer (save, rollback, import, suggestion-accept) passes the same gate. If launch already validates, this is defense-in-depth; if it does not, it closes a launch-env injection hole.

#### W-4. Error strings leak absolute paths across IPC today; the service move is the chance to fix it
- **Where**: profile commands return `map_error = |e| e.to_string()` (`shared.rs:9-11`). `ProfileStoreError::NotFound(path)` prints the full path (`toml_store/error.rs` Display), `Io` prints the OS message (often path-bearing), and `MigrationApplyResult.error` embeds `request.new_path` raw (`profile/migration/apply.rs:30-40, 89-96`) — only the `old_path`/`new_path` *fields* are sanitized afterwards (`commands/migration.rs:139-141, 258-260`), the `error` field is not.
- **Risk**: usernames/home layouts and out-of-home mount points (`/mnt/...`) are exposed to the webview and any UI/log that captures command errors. Existing mitigation is partial: `crosshook_app::AppError` sanitizes on construction (`crates/crosshook-app/src/error.rs:35-39` via `presenter::sanitize_message`) and deliberately reduces `Io` to kind-only (`error.rs:84-118`), but the profile commands don't go through it.
- **Mitigation**: `ProfileWriter` returns `AppError`; the src-tauri bridge maps it to its sanitized `message()` for IPC. Keep `sanitize_display_path` on migration result fields *and* apply it to `MigrationApplyResult.error` (or move migration errors through `AppError` too). Note `sanitize_message` only redacts home-style paths (`presenter/mod.rs:31-60`) — out-of-home paths remain visible by design; document that.

#### W-5. Launcher slug collisions let one profile's cleanup delete another profile's launcher
- **Where**: `sanitize_launcher_slug` (`export/launcher/names.rs:43-67`) maps every non-alphanumeric char to `-`, so `"Test 1"` and `"Test-1"` (and `"Test_1"`) all slug to `test-1` and derive identical script/desktop paths (`export/launcher_store/paths.rs:13-36`). `delete_launcher_for_profile` (`export/launcher_store/mutations.rs:98-115`) deletes whatever watermarked file sits at those paths.
- **Risk**: deleting or renaming profile A removes profile B's launcher file (both are legitimately CrossHook-watermarked, so `verify_crosshook_file` correctly does not protect against this). Impact is bounded — a re-export restores it — but it is cross-profile interference reachable from ordinary rename/delete.
- **Mitigation**: do **not** loosen slug sanitization in the move (alphanumeric-only output is what keeps deletion inside the two fixed subdirectories, `.local/share/crosshook/launchers` and `.local/share/applications/crosshook-*-trainer.desktop`). Optionally detect the collision (list profiles, compare slugs) and warn; at minimum add a regression test asserting the delete path is always `<home>/.local/share/crosshook/launchers/<slug>-trainer.sh` or `<home>/.local/share/applications/crosshook-<slug>-trainer.desktop` for adversarial display names (`"../../evil"`, `"a/b"`, emoji, empty).

#### W-6. Custom-art auto-import copies any readable image on disk into asset-scope-served storage
- **Where**: `profile_save` auto-import (`lifecycle.rs:158-193`) feeds the frontend-supplied `data.game.custom_cover_art_path` (any absolute path) into `import_custom_art` (`game_images/import.rs:33-72`): `fs::read(source)` → `validate_image_bytes` (magic-byte MIME allow-list jpeg/png/webp, 5 MB cap, `game_images/client/validation.rs:24-36`) → content-hash-named write under `~/.local/share/crosshook/media/<type>/`.
- **Risk**: a compromised webview can direct the backend to copy any user-readable ≤5 MB image anywhere on the system into `media/`, which **is** inside the Tauri asset-protocol scope (`src-tauri/tauri.conf.json:24-27`: `$LOCALDATA/crosshook/media/**`) — i.e. an exfiltration bridge for image files, and a way to plant content in a scoped directory. Content validation prevents arbitrary-file read (non-images fail), so this is bounded to images. Secondary: the destination write is a plain non-atomic `std::fs::write` (`import.rs:61`), though the hash-named idempotent destination makes torn writes low-impact.
- **Mitigation**: acceptable to keep (the file dialog is the normal source and the webview is trusted per Tauri's model), but the service should (a) keep `validate_image_bytes` as the mandatory gate, (b) prefer restricting auto-import to paths under `$HOME`, and (c) switch the destination write to `fs_util::write_atomic` for consistency. Also note `is_in_managed_media_dir` (`import.rs:18-24`) is lexical `starts_with` — a symlink placed inside `media/` skips re-import; harmless for writes (dest is hash-named) but worth a comment in the service.

### ADVISORY

#### A-1. Launcher cleanup invariants that must survive the move verbatim
`cleanup_launchers_for_profile_delete` (`shared.rs:44-62`) must keep delegating to `core::export::delete_launcher_for_profile` and must never inline `fs::remove_file`. The safety stack, all in core, is: `native` method short-circuit → `derive_steam_client_install_path` splits `steam.compatdata_path` on the `/steamapps/compatdata/` marker (`shared.rs:16-22`; profile-controlled string steers the derived home, so confinement must not depend on it) → `resolve_target_home_path` only accepts absolute, non-compatdata roots (`export/launcher/paths.rs:62-92`) → alphanumeric slug → **`verify_crosshook_file`**: `symlink_metadata` (symlinks/dirs refused) + watermark content check before any unlink (`export/launcher_store/fs_ops.rs:23-47`, `mutations.rs:35-60`). Two ordering contracts to preserve: cleanup runs **before** `store.delete` and its failure only warns (`lifecycle.rs:237-243`); on rename, the old profile is loaded **before** `store.rename` and cleanup targets the old slug (`lifecycle.rs:308-325`). Because deletion is watermark-gated, even a hostile `compatdata_path` can only remove CrossHook-authored files — keep it that way.

#### A-2. Rollback must verify HMAC before any write, exactly once, in this order
`profile_config_rollback` (`config_history.rs:260-340`) currently: availability check → `revision_id > 0` → profile-scoped `get_config_revision(&profile_id, revision_id)` → **`metadata_store.verify_config_revision(&revision)?`** (line ~290) → parse snapshot → `store.save` → `observe_profile_write` → `capture_config_revision(RollbackApply, Some(revision_id))` (lineage) → emit. `verify_config_revision` (`metadata/config_history_ops.rs:53-68`) always re-hashes SHA-256 and, with a key loaded, enforces the ADR-0005 epoch rules (`config_history_hmac.rs:210-263`): post-epoch rows missing/foreign/mismatched signatures fail closed (`IntegrityCheckFailed`, message at `metadata/models.rs:87-101`), pre-epoch and key-unavailable rows degrade to hash-only with a warn. The `RollbackApply` row is signed with trust derived from re-verifying the source (`config_history_store/mod.rs:104-125`, `derived_trust`), so forged hash-only content is never upgraded. **Refactor requirement**: this exact call sequence (verify → parse → save → capture-with-lineage) moves as a unit into the service; do not add `clear_extra()` to the restored profile (unknown TOML fields must survive the round-trip), and keep revision lookups scoped by `profile_id` so a revision id from another profile is unreachable.

#### A-3. TOML byte-compatibility = one serializer + `write_atomic`, nowhere else
All durable profile writes go through `ProfileStore::save` → `storage_profile()` → `toml::to_string_pretty` → `crate::fs_util::write_atomic` (`toml_store/store.rs:95-121`). `write_atomic` (`fs_util.rs:25-146`) is solid: same-dir temp, permission preservation (0644 for new files), file fsync, rename, parent-dir fsync, symlink-follow with dangling-symlink refusal, pre-rename failure leaves the original untouched. `apply_single_migration` correctly reuses the same writer (`profile/migration/apply.rs:76-96`). **Refactor requirements**: (1) the service must call `ProfileStore` mutators, never serialize profiles itself — any second serialization path breaks the golden-file byte-identity acceptance criterion; (2) the MangoHud companion write stays best-effort non-fatal (`store.rs:113-120`); note it uses non-atomic `std::fs::write`/`remove_file` (`profile/mangohud.rs:79-99`) — pre-existing, do not regress further, optionally unify on `write_atomic`; (3) `legacy::save` (`legacy.rs:55-77`, non-atomic `File::create`) is only used by the legacy store, not the import path — leave it out of the new service.

#### A-4. `capture_config_revision` is silently permissive when the store is disabled
`capture_config_revision` (`shared.rs:112-176`) returns `None` when `!metadata_store.is_available()` with **no warning** (only insert/serialize failures warn). The issue spec requires: "With `MetadataStore::disabled()`, saves still succeed, revisions are skipped, and a warning is logged and surfaced." The service should add an explicit `tracing::warn!` (and a surfaced flag in whatever result the bridge returns) on the disabled path — this is a behavior improvement the spec mandates, not a regression. Keep the rest of the fail-open contract: revision capture must never fail a user-facing save (dedupe via `insert_config_revision`, `MAX_SNAPSHOT_TOML_BYTES` 256 KiB rejection, retention pruning, `MAX_CONFIG_REVISIONS_PER_PROFILE` 20, `MAX_HISTORY_LIST_LIMIT` 50, diff caps `DIFF_MAX_LINES` 2000 / `MAX_DIFF_OUTPUT_BYTES` 512 KiB — all enforced in core; the service must keep calling these core functions rather than reimplementing caps).

#### A-5. Event payload: object shape, validated names, unified mocks
Today `emit_profiles_changed` sends a bare reason string (`shared.rs:68-72`) while the browser-dev mocks already send objects (`src/lib/mocks/handlers/profile-mutations.ts:305` `{name, action}`, `profile-history.ts:104`) and tests emit `''` (`src/hooks/__tests__/useProfileSummaries.test.ts:54`). The new `ProfilesChanged { reason, names }` payload is runtime-only (per spec) and safe: names cannot contain `/`, `\`, `:`, or control chars *if they came from store ops* — but `profile_set_favorite` (`favorites.rs:6-18`) calls `set_profile_favorite` (parameterized SQL, `metadata/collections.rs:167-178`, no name validation) and emits without ever validating the name. Mitigation: `ProfileWriter`'s favorite path runs `validate_name` before the DB update and before emitting; the five listeners (`src/hooks/{useProfileSummaries,useLibrarySummaries,useProfile,useCommunityProfiles,useProfileHealth}.ts`) and the mocks switch to the object payload in the same change, and `useProfileHealth`'s `batchValidate` skip-list for `FavoriteToggled` must be deny-by-default (validate on every reason except an explicit allow-list) so a future reason cannot silently skip health revalidation.

#### A-6. Panics: contain them with `run_blocking`, and know the existing panic sites
Current profile commands are synchronous `#[tauri::command]` fns — a panic inside one is not contained by any repo sanitization layer. Realistic panic sources on these paths: `SettingsStore`'s `io_lock.lock().expect("settings mutex poisoned")` (`settings/store.rs:98-141`) — reached from `profile_save`, `profile_rename`, and every revision-cap resolution after any prior settings panic poisons the lock; `ProfileStore::new().expect(...)` (bootstrap-only, not IPC-reachable). Non-issues verified: HMAC `new_from_slice(...).expect(...)` takes a fixed 32-byte key (`config_history_hmac.rs:151-153`); the metadata conn lock maps poison to `Corrupt` errors, not panics (`metadata/store.rs:187-242`); the launch-session registry expects are outside profile writes. The `crosshook-app` convention already solves this: every service fn is async and blocking work runs in `spawn_blocking` via `run_blocking` (`services/mod.rs:29-37`), whose `From<JoinError>` collapses panics to `AppErrorKind::Internal` with the fixed message `"background task failed"` — panic payloads (which may embed user paths) never cross IPC (`error.rs:67-77`). **Refactor requirement**: `ProfileWriter::commit` follows `run_blocking`, the src-tauri bridge stays thin (state clone in, `AppError` → sanitized string out), and the service adds no `unwrap`/`expect` on data derived from IPC input.

#### A-7. Legacy import silently overwrites an existing profile
`import_legacy` derives the target name from the file stem and `save` overwrites unconditionally (`toml_store/store.rs:383-404`); the command then reports success (`lifecycle.rs:350-384`). A user importing `/tmp/elden-ring.profile` when `elden-ring.toml` exists clobbers it with no prompt. This is pre-existing behavior — the refactor is behavior-preserving, so keep it, but surface the overwrite in the result (or log a warn) and record it as a known data-loss edge for the future. Also note the arbitrary-read property: `path` is frontend-supplied and `legacy::load` reads any user-readable file; only known `Key=Value` lines are captured, so exfiltration via the returned `GameProfile` is limited to those fields — acceptable under the trusted-webview model, worth a comment.

#### A-8. Name length is uncapped
`validate_name` has no length limit; `write_atomic` already truncates only the *temp* name to 64 bytes and handles `NAME_MAX` gracefully (`fs_util.rs:33-35, 56-66`, tested at `fs_util.rs:433-443`), so an over-long name fails cleanly with an OS error at persist time (which then must flow through `AppError` per W-4, since `Io` messages can embed the path). No change required; do not add a bespoke limit in the service that could reject names the current store accepts.

#### A-9. Concurrency contract: last-write-wins must be a decision, not an accident
`ProfileStore` has no cross-command locking; `save`/`save_launch_optimizations` explicitly document last-completed-write-wins (`toml_store/store.rs:125-128, 189-192`). Moving the load→mutate→save sequence into one `spawn_blocking` task keeps that semantics; adding a **global** mutex in `ProfileWriter` would serialize unrelated profiles and change observable behavior (and risk deadlocks if a mutation closure ever calls back into the service). If serialization is desired, scope it per-profile-name and document it; otherwise keep the documented race and make `commit`'s doc comment state it explicitly. The rename flow also has a crash window (rename → launcher cleanup → display_name re-save, `lifecycle.rs:303-343`) — pre-existing, non-fatal, unchanged.

#### A-10. Community/TOML export sanitization is a denylist — don't route shareable export through `profile_save`
The shareable-TOML path (`profile_to_shareable_toml_with_options`) strips hooks by default (`include_hooks: false` default; the UI passes it explicitly at `src/hooks/profile/useProfileActions.ts:237-266`) but otherwise serializes the full profile; the *community* JSON path has the hardened denylist (`profile/exchange/utils.rs:21-52`: hook paths cleared+disabled, DLL paths, icon/proton/working-dir, art paths — with an explicit "denylist invariant" note that new path-bearing fields must be enumerated). The new `ProfileWriter::export_toml` must keep using the shareable-TOML options (default no hooks) and must not gain a "save exported profile" shortcut that skips `clear_extra`/hook stripping. Frontend-supplied `data` (not the on-disk profile) is what gets serialized — keep that, but combine with W-2's name validation.

---

## 3. Verified-cleared areas (evidence)

- **HMAC key handling** (`metadata/config_history_hmac.rs`): key file `0600`, 32-byte CSPRNG + epoch, opened `O_NOFOLLOW` with owner/length checks, `Zeroizing` buffers, stale-temp cleanup, epoch lowering on rewound DBs; `key_id` is a non-secret fingerprint; the key never crosses IPC (`models.rs:571-572` marks `hmac_signature` core-only; `ConfigRevisionSummary` omits it). Flatpak import copies the key only with the DB and forces `0600` (`flatpak_migration/copier.rs`, `types.rs:54-57`). No changes needed — the service must simply keep calling `insert_config_revision`/`verify_config_revision` rather than touching rows.
- **SQL**: all profile/metadata/revision statements use `params![]` binding (verified across `profile_sync.rs`, `collections.rs`, `config_history_store/mod.rs`).
- **Revision scoping**: `get_config_revision` filters `WHERE id = ?1 AND profile_id = ?2` (`config_history_store/mod.rs:217-220`), so cross-profile revision access via `profile_config_diff`/`profile_config_rollback`/`profile_mark_known_good` is already prevented; `insert_config_revision` validates `source_revision_id` lineage in-transaction.
- **Symlinks**: `write_atomic` refuses dangling/looping symlinks and follows resolvable ones intentionally; launcher deletion uses `symlink_metadata` (never follows); `copy_symlink` in fs_util rejects absolute/`..` targets (not on profile paths, noted for completeness).

## 4. Security invariants checklist for the implementation plan

1. Every name that reaches a path, SQL bind, or event payload passes `validate_name` first (adds: export name W-2, favorite name A-5).
2. Deletion of launcher artifacts only via `delete_launcher_for_profile` (watermark + regular-file + slug-sanitized path); cleanup before delete, warn-only on failure.
3. Rollback: verify (hash + HMAC) → parse → save → observe → capture with lineage → emit; no `clear_extra` on restore.
4. One serialization path: `ProfileStore::save`/core mutators + `write_atomic`; no service-level TOML writing.
5. Revision capture fail-open on error, fail-loud (warn + surfaced) on disabled store; all caps stay in core.
6. All service fns async, blocking work in `run_blocking`, errors as sanitized `AppError`; bridge stringifies `message()` only.
7. `profiles-changed` object payload `{reason, names}` with validated names; health listener skips only an explicit reason allow-list; mocks and listener typings updated together.
8. Input gates centralized in `commit`: `validate_steam_app_id`, env-var validation, command-arg catalog validation (already in store, `toml_store/store.rs:489+`), art-import MIME/size gate.

## 5. Test hooks (map findings → tests for the plan)

- W-1: import fixture with malformed `SteamAppId` → rejected; valid import → TOML on disk matches pre-refactor fixture.
- W-2: export with newline-bearing name → `InvalidName`; normal export byte-identical to fixture.
- W-3: `profile_save` with reserved env key → rejected (new test; today it passes — land the gate with the service).
- W-4: assertion helpers that command errors contain no `/home/<user>` substring (extend `presenter/tests.rs` patterns to profile bridge tests).
- W-5/A-1: adversarial display-name table → derived delete paths confined to the two launcher dirs; slug-collision pair deletes only watermarked files.
- A-2: tampered `snapshot_toml` (recomputed hash) with intact key → rollback fails `SignatureRequired`/`SignatureMismatch`; key-unavailable session → hash-only warn + rollback proceeds (mirrors `metadata/tests/hmac.rs`).
- A-3: golden-file byte-identity for save/rename/duplicate/rollback (spec AC-1).
- A-4: disabled-store save succeeds, revision skipped, warn recorded and surfaced.
- A-5: Vitest listener test — `FavoriteToggled` triggers no `batch_validate_profiles` (call counter), unknown reason does.
- A-6: poison-the-settings-lock test → favorite/save returns `AppError` (Internal), process alive, message is the fixed string.

## 6. Uncertainties

- Launch-time env validation (W-3) — **[INFERENCE: unverified]**; `launch/request.rs` env handling was not audited. The recommendation (centralize in `commit`) holds either way.
- Whether Tauri v2 catches panics in *synchronous* commands today — **[INFERENCE: unverified]**; irrelevant post-refactor because `run_blocking` contains them, but don't cite current behavior as safe.
- `RESERVED_ENV_KEYS` exact membership (`protondb/suggestions.rs`) — existence verified, contents not enumerated; enumerate when implementing W-3.
