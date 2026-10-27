# Plan: app-storage-services — six services in `crosshook-app` (YAN-847 / M1-35)

Tracking: Linear YAN-847 / GitHub #539 (M1-35). Release `crosshook v0.7.0-alpha.1`, trunk-only off `main`, one PR. Paths relative to `src/crosshook-native/` unless prefixed `docs/`; in tasks `app/` = `crates/crosshook-app`, `core/` = `crates/crosshook-core`, `tauri/` = `src-tauri`. Binding decisions: [decisions.md](./decisions.md). Frozen contracts: [b0-contracts.md](./b0-contracts.md). Spec: [feature-spec.md](./feature-spec.md).

## Summary

Move 45 Tauri commands (storage 3, mods 6, collections 13, launcher export 12, version 4, steam 3 + game metadata 4) into six `#[derive(Clone)]` services under `app/src/services/` following the profiles/launch shape. `Services::new` gains `JobRegistry` (D2); scan becomes an exclusive job with a result slot (D3); cleanup keeps the fresh-scan allow-list, now tested, on top of D5-hardened core checks; collections/mods/version gate disabled DBs with `Unavailable` (D7) while adapters keep every legacy error string byte-identical (D8, D12). Launcher goldens are frozen from the unmodified code before any move (D10). All commands become async delegates whose blocking work runs in `run_blocking` (D11).

## Batches

Tasks in a batch touch **disjoint files**; batches run in order. Subagents skip builds/formatters mid-flight; the main agent compiles once per batch gate. Every batch gate is fully compilable (B1 lands compiling skeletons — struct + `new` + module tree + re-exports + `#![expect(dead_code)]`; **no `todo!()` stubs**, which would panic at runtime and fail the batch gate).

| Batch | Tasks        | Depends on | Width | Contents                                                                                                                                                                        |
| ----- | ------------ | ---------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B0    | 0.1, 0.2     | —          | 2     | Freeze launcher goldens from unmodified code (D10) + IPC name inventory                                                                                                         |
| B1    | 1.1          | B0         | 1     | Shared wiring: `services/mod.rs`, six service `mod.rs` skeletons, `Stores`/`Services`, `app_bootstrap`, `error.rs` D8 helper, `src-tauri/lib.rs` manage lines, Cargo `[[test]]` |
| B2    | 2.0, 2.1–2.6 | B1         | 7     | Core D5 hardening + six service implementations with their functional tests (same file owners as the skeleton methods)                                                          |
| B3    | 3.1–3.4      | B2         | 4     | Tauri adapter cutover per command-file group, incl. the D9 `apply_collection_defaults` move                                                                                     |
| B4    | 4.1          | B3         | 1     | AC1 storage cleanup security tests + dead-code sweep                                                                                                                            |
| B5    | 5.1          | B4         | 1     | Docs, follow-up issues, final format                                                                                                                                            |

Total: 13 tasks, 6 batches, max width 7.

## Task 0.1: Freeze launcher-export goldens from unmodified code (B0)

- **Owns**: `app/tests/fixtures/launcher_export/*` (new), `app/tests/launcher_export_golden.rs` (new), `app/Cargo.toml` (one `[[test]]` block). Nothing else — the source code stays unmodified (D10).
- **Steps**:
  1. Write a **throwaway** capture binary (e.g. `app/tests/capture_launcher_goldens.rs`, not committed; run via `cargo test -p crosshook-app --test capture_launcher_goldens -- --nocapture`) that pins `$HOME` to a tempdir **before any runtime or thread starts** (`harness = false` `main`, mirroring `profile_writer_golden.rs:8-13`), materializes a stub trainer file, and drives the **current** `crosshook_core::export::{export_launchers, preview_trainer_script_content, preview_desktop_entry_content}` for 6 export cases: `steam_applaunch` plain, `steam_applaunch` + gamescope, `proton_run` plain, `proton_run` + network isolation + 2 custom env vars (ordered), `proton_run` + umu (`umu_preference: Proton`), `copy_to_prefix` loading mode — plus the 2 preview pairs for case 1 and case 4. Requests use fixed fake absolute paths (`/fake/home`, `/fake/steam`, no icon — `validate` requires an existing icon file).
  2. Write the produced script + desktop bytes (and preview strings) to `app/tests/fixtures/launcher_export/<case>/{script.sh,desktop.desktop}` / `preview-{script,desktop}.txt`. Verify determinism: run twice, assert identical bytes; two runs in different processes produce identical SHA-256s. Delete the throwaway capture.
  3. Land `app/tests/launcher_export_golden.rs` (normal `#[test]`, `harness = false` only if HOME must precede the runtime — prefer `harness = false` to match the capture env): each case re-runs the same core calls and compares `fs::read` == `include_bytes!` byte-for-byte. Substitute the tempdir home with a fixed `@HOME@` placeholder **before comparison on both sides** so the fixture is machine-independent; assert modes 0o700/0o644 via `std::os::unix::fs::PermissionsExt`.
  4. Add the `[[test]] name = "launcher_export_golden"` block to `app/Cargo.toml`.
- **Acceptance**: fixtures exist with recorded SHA-256s noted in the test header; `cargo test -p crosshook-app --test launcher_export_golden` passes **before any source move**; `git status` shows no source-file change.
- **Tests**: the golden test itself (AC3 evidence base).

## Task 0.2: IPC command-name inventory (B0)

- **Owns**: `docs/plans/app-storage-services/ipc-inventory.md` (new; a docs file, allowed).
- **Steps**: run `grep -n "tauri::command" tauri/src/commands/{storage,mods,collections,export,version,steam,game_metadata}.rs` and record all 45 names with their current sync/async kind, argument names, and the frontend payload keys from research-ux §1. Note the four mock-only names (`rename_launcher`, `find_orphaned_launchers`, `get_version_snapshot`, `import_custom_cover_art`). Cross-check against `src/lib/mocks/handlers/` (`check-mock-coverage.sh` list).
- **Acceptance**: 45 rows; B3 uses it as the cutover checklist and every B3 task cites its rows.
- **Tests**: none (inventory).

## Task 1.1: Shared wiring + compiling skeletons (B1)

- **Owns**: `app/src/services/mod.rs`, `app/src/services/{storage,mods,collections,launcher_export,version,steam}/mod.rs` (new), each service's `tests/{mod.rs,helpers.rs}` (new, empty declarations), `app/src/bootstrap/stores.rs`, `app/src/bootstrap/stores/tests.rs`, `app/src/bootstrap/app_bootstrap.rs`, `app/src/error.rs`, `app/src/lib.rs`, `tauri/src/lib.rs` (`.manage()` lines only), `app/README.md` (services table row placeholder).
- **Steps**: implement the Wiring section of b0-contracts.md exactly. Each `services/<x>/mod.rs` declares its leaf `mod` lines (empty doc-only files: `scan.rs`, `cleanup.rs`, `audit.rs` / `crud.rs`, `detect.rs`, `coexistence.rs` / `crud.rs`, `defaults.rs`, `transfer.rs` / `export.rs`, `list.rs`, `preview.rs` / `status.rs`, `ops.rs` / `client.rs`, `proton.rs`, `metadata.rs`), the struct + `new` + `pub use` re-exports + `static_assertions::assert_impl_all!(<Svc>: Send, Sync)` + `#![expect(dead_code, reason = "B2 fills the methods; B4/B5 removes this once everything is wired")]` at the top. `Services::new` per b0-contracts; extend `services_new_builds_bundle_from_stores` to build the bundle and assert `Arc::ptr_eq`-style sharing where applicable plus the six new fields. Add `AppError::from_metadata_validation` (D8) with unit tests pinning the exact Display texts. Six `.manage()` lines in `tauri/src/lib.rs`. Re-exports in `app/src/lib.rs`.
- **Acceptance**: `cargo check --workspace` passes; `cargo test -p crosshook-app error` passes (D8 texts); `./scripts/check-app-deps.sh` passes (no toolkit deps).
- **Gate (main agent)**: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && ./scripts/check-app-deps.sh`.

## Task 2.0: Core cleanup hardening (B2)

- **Owns**: `core/src/storage/prefix_health/cleanup.rs` (+ its `#[cfg(test)] mod tests` at the file bottom; `core/src/storage/prefix_health/mod.rs` only if a test hook is needed — none expected).
- **Steps**: implement D5 exactly per b0-contracts ("Core cleanup hardening"): `ensure_no_follow_directories`, `ensure_staged_target_layout`, `CleanupRejection`; wire them into both per-kind paths before canonicalization; every rejection becomes a `skipped` entry with the fixed reason strings; canonicalization failure fails closed. Orphan containment derives the trusted root from the reference inventory (parent of the referenced prefix; compatdata root when the prefix ends in `pfx`), never from the request.
- **Acceptance**: `cargo test -p crosshook-core storage::prefix_health` green: symlink target (in-root and out-of-root referent), dangling link, non-directory, symlinked `drive_c`/`CrossHook`/`StagedTrainers` intermediate, target equal to a root, non-immediate staged child, canonicalize failure → skip, all with fixed reason strings and the referent untouched.
- **Ordering note**: this task may land any time in B2 (storage service tests in 2.1 and 4.1 depend on it, so it must merge within B2's gate, before B3).

## Task 2.1: Storage service (B2)

- **Owns**: `app/src/services/storage/{scan,cleanup,audit}.rs`, `app/src/services/storage/tests/{scan,cleanup,audit}.rs`.
- **Steps**: implement the storage contract in b0-contracts. `scan.rs`: job wrapper + progress + result slot + `scan()` awaitable with the 300 s timeout; snapshot inserts fail-soft exactly as `commands/storage.rs:32-48`. `cleanup.rs`: one closure — collect references → fresh scan → `BTreeSet` signature allow-list + dedupe (`commands/storage.rs:72-86` moved verbatim) → **server-produced targets** → D5-hardened core cleanup → D6 rejected-request `skipped` entries with reason `not present in a fresh storage scan` → audit rows (`deleted` per target, `skipped` per skip, `summary` when `reclaimed_bytes > 0 || !deleted.is_empty()`), all fail-soft. `audit.rs`: `history()` returning the moved `PrefixStorageHistoryResponse` (`available` + 50 snapshots + 100 audit rows). Build join-failure messages as `prefix storage {scan|cleanup|history} task failed: {error}` via `AppError` texts.
- **Acceptance/tests** (`tests/`): scan produces snapshot rows in an in-memory store (fail-soft on disabled); concurrent `start_scan` → second returns `Conflict`; `scan_result` take-once; cleanup happy path deletes a fixture orphan prefix and staged trainer, dedupes repeats; a target missing from the fresh scan becomes a skipped entry + audit row (functional half of AC1; the adversarial table is 4.1); audit summary gate both ways; history limits; scan/cleanup work with `MetadataStore::disabled()`.
- **VALIDATE** (main agent, batch gate): `cargo test -p crosshook-app services::storage`.

## Task 2.2: Mods service (B2)

- **Owns**: `app/src/services/mods/{crud,detect,coexistence}.rs`, `app/src/services/mods/tests/{crud,detect,coexistence}.rs`.
- **Steps**: implement the mods contract. Move `resolve_profile` logic into `crud.rs` with the frozen texts; `list` availability DTO; `add/update/remove/analyze` Unavailable-first guards; `detect` two-closure shape (load, then scan; optional best-effort enrichment when available, failures leave flags false); `analyze` via `analyze_profile_mod_coexistence`. All store errors through `AppError::from_metadata_validation("mod", …)`.
- **Acceptance/tests**: every case in research-ux §0.2/research-business §3 — disabled DB (`list` → `{available:false,mods:[]}`, mutations → `Unavailable`), missing identity text, duplicate-name text, `mod not found` on stale remove, cross-profile mod_id isolation (update/remove scoped by profile), detect with DB down still scans, `no_game_path` validation, enrichment flags true only for registered names, coexistence issues pass through.
- **VALIDATE**: `cargo test -p crosshook-app services::mods`.

## Task 2.3: Collections service incl. D9 (B2)

- **Owns**: `app/src/services/collections/{crud,defaults,transfer}.rs`, `app/src/services/collections/tests/{crud,defaults_merge,transfer}.rs`.
- **Steps**: implement the collections contract. `crud.rs`: 9 methods with D7 guards (writes Unavailable-first; `list`/`collections_for_profile` keep `Ok(vec![])` on disabled). `defaults.rs`: get (clear extras) / set (clear + preserve round-trip, `?` on the existing-row read), the moved `apply_defaults` + `resolve_defaults_sync` + pure `resolve` (D9; policy verbatim from `commands/profile/shared.rs:35-81`). `transfer.rs`: export/import with the frozen validation strings; import stays preview-only (D16).
- **Acceptance/tests**: the 5 moved merge tests (None id, whitespace id, valid merge, unknown-id fail-open, Corrupt bubbles) **ported** into `tests/defaults_merge.rs` (they currently call the `pub(super)` shell helper with `with_sqlite_conn`; re-home against `CollectionsService`), plus the new rows: `network_isolation` Some(true)/Some(false), `trainer_gamescope` replace, `mangohud` replace with profile `.extra` retained, `optimizations` replace keeping extras, mixed all-fields, empty `custom_env_vars` no-op, NULL defaults through the helper, non-Validation (`Database`) fail-open, disabled store identity, `resolve_defaults_sync` each branch (AC2). CRUD: create/duplicate/rename/unknown-id/add-profile-missing-name texts; delete cascade; idempotent add; DB-disabled no-write assertions (inspect the in-memory store before/after).
- **VALIDATE**: `cargo test -p crosshook-app services::collections`.

## Task 2.4: Launcher-export service (B2)

- **Owns**: `app/src/services/launcher_export/{export,list,preview}.rs`, `app/src/services/launcher_export/tests/{export,list_orphans,preview}.rs`, and the flip of `app/tests/launcher_export_golden.rs` to drive `LauncherExportService` instead of raw core calls (same fixtures).
- **Steps**: implement the launcher contract. `export.rs`: validate/export (metadata observe fail-soft after the write, resolve after files+observe per D11)/check_exists/check_for_profile (settings load error is a hard error — preserved)/delete/delete_by_slug (observe with the raw slug)/`rename` taking `LauncherRenameRequest` (mode parse `invalid trainer loading mode`; D14 old-slug sanitize + parent containment before any file op; umu/runtime reset preserved, D15). `list.rs`: `list` with stale flags (settings failure → Auto fallback; per-profile load failures skipped; multi-profile slug OR), `find_orphaned`, `reexport_by_slug` (`No profile found for launcher slug '{slug}'`; final settings reload hard error — preserved). `preview.rs`: both previews with headers. All file I/O inside `run_blocking`.
- **Acceptance/tests**: mode/permission/bytes via the golden test flip (AC3); stale-flag OR-combination and unknown-slug-defaults-false; orphan filtering; rename refuses a traversal old slug (`../`) and a slug whose old paths resolve outside the launcher dirs (D14); watermark/symlink skips inherited from core (assert through the service); observe fail-soft when metadata is disabled; reexport happy path against a fixture profile.
- **VALIDATE**: `cargo test -p crosshook-app services::launcher_export && cargo test -p crosshook-app --test launcher_export_golden` (fixtures untouched: `git diff --stat app/tests/fixtures` empty).

## Task 2.5: Version service (B2)

- **Owns**: `app/src/services/version/{status,ops}.rs`, `app/src/services/version/tests/status.rs`.
- **Steps**: implement the version contract; move `status_from_str`, `row_to_snapshot_info`, `locate_manifest_for_app` (as private fns); both write ops Unavailable-first (D7).
- **Acceptance/tests**: `check_status` end-to-end against a fixture Steam library tree (StateFlags=4 → correlation, ≠4 → `UpdateInProgress`, no manifest → untracked, unreadable trainer hash → None); unregistered profile → empty id + no snapshot; `snapshot` unknown id → `Ok(None)`, `status_from_str` unknown → `Unknown`; `set_trainer_version` preserves `steam_app_id` and upserts `untracked` (assert the row); `acknowledge` sets `matched` and is `Ok` with zero snapshots; disabled DB → `set`/`acknowledge` return `Unavailable` **before** the not-registered check.
- **VALIDATE**: `cargo test -p crosshook-app services::version`.

## Task 2.6: Steam + game-metadata service (B2)

- **Owns**: `app/src/services/steam/{client,proton,metadata}.rs`, `app/src/services/steam/tests/{proton,metadata}.rs`.
- **Steps**: implement the steam contract. `client.rs`: `default_client_install_path` (resolver order verbatim) + `auto_populate` (one blocking closure). `proton.rs`: `list_proton_installs` fresh scan every call in `run_blocking` (D11 — no caching), `None` defaults to the same resolver, `Some("")` passes through, diagnostics logged at debug. `metadata.rs`: `fetch_game_metadata` (async core call, inline sync SQLite stays inside `run_blocking` phases where core requires it), `fetch_cover_art` (image-type parse with frozen `Unknown image type` text, fail-soft key read, service-level `Unavailable` on network failure while cached art returns first), both imports with frozen `Unknown art type` text.
- **Acceptance/tests**: resolver order (env var override, `$HOME` unset, candidate `steamapps` probe, empty fallback) via injected env map seam (`with_env` builder, house pattern — no `set_var` in tests); proton scan returns installs from a fixture root and does **not** block the caller thread (current-thread runtime: spawn the call, assert the runtime stays responsive while a gated closure holds the blocking pool — AC4 evidence); art-type parse matrix; `Unavailable` → adapter contract asserted at the Tauri layer in 3.4; imports content-address and are idempotent (fixture bytes); API key never appears in captured tracing output.
- **VALIDATE**: `cargo test -p crosshook-app services::steam`.

## Task 3.1: Adapter cutover — storage + mods (B3)

- **Owns**: `tauri/src/commands/storage.rs`, `tauri/src/commands/mods.rs`.
- **Steps**: both commands become `async`, take `State<'_, StorageService>` / `State<'_, ModsService>`, delegate, and map errors per the adapter table (byte-identical strings; `scan_prefix_storage` awaits `scan()` so the IPC result is unchanged). Delete the moved bodies/helpers (`target_signature`, `cleanup_target_kind_str`, `resolve_profile`, `ProfileModsResponse` re-homed, `PrefixStorageHistoryResponse` re-homed). Add/refresh the module's `command_names_match_expected_ipc_contract` pin (name-only `let _ = <cmd>;` pins for async commands, per `commands/settings.rs:182`).
- **Acceptance**: `cargo check -p crosshook-native --all-targets`; module pin test green; no logic left in the file (review: only State extraction, delegation, error mapping).
- **VALIDATE**: `cargo test -p crosshook-native commands::storage commands::mods`.

## Task 3.2: Adapter cutover — collections + D9 profile composition (B3)

- **Owns**: `tauri/src/commands/collections.rs`, `tauri/src/commands/profile/shared.rs`, `tauri/src/commands/profile/lifecycle.rs`, `tauri/src/commands/profile/tests/optimizations.rs`, `tauri/src/commands/profile/tests/mod.rs` (module list only).
- **Steps**: all 13 collections commands become async delegates. `profile_load` calls `writer.load(name)` then `collections.apply_defaults(profile, collection_id.as_deref())` (still on the blocking pool via the service's own `run_blocking`), then `profile.clear_extra()`. `profile_list_summaries` passes `move || collections.resolve_defaults_sync(collection_id.as_deref()).map_err(Into::into)` to `writer.list_summaries` — **preserve** the at-most-once, only-after-first-profile-loads ordering (empty library must not surface a collection error). Delete `apply_collection_defaults`/`resolve_collection_defaults` from `shared.rs` and the five moved tests from `optimizations.rs` (they live in `services/collections/tests/defaults_merge.rs` since 2.3). IPC pin refreshed.
- **Acceptance**: pin test green; `profile_load`/`profile_list_summaries` behaviour unchanged (existing lifecycle tests still pass); `shared.rs` no longer mentions collections.
- **VALIDATE**: `cargo test -p crosshook-native commands::collections commands::profile`.

## Task 3.3: Adapter cutover — export (B3)

- **Owns**: `tauri/src/commands/export.rs`.
- **Steps**: all 12 commands become async delegates on `State<'_, LauncherExportService>`; `rename_launcher` keeps its 15 flat IPC args and packs them into `LauncherRenameRequest` (Tauri derives camelCase keys automatically — unchanged). Delete `build_export_request_for_profile`, `apply_stale_flags`, `stale_flags_by_profile_slug`, `find_profile_name_for_launcher_slug` and the old sync fn-pointer pins; replace the pin test with name-only pins plus payload-serialization assertions (the current `:459-530` table pins sync pointers that no longer exist).
- **Acceptance**: pin + payload tests green; `export_launchers` delegates through the async service (AC4); file holds no logic.
- **VALIDATE**: `cargo test -p crosshook-native commands::export`.

## Task 3.4: Adapter cutover — version + steam + game_metadata (B3)

- **Owns**: `tauri/src/commands/version.rs`, `tauri/src/commands/steam.rs`, `tauri/src/commands/game_metadata.rs`, `tauri/src/commands/protonup.rs` (line 168 only), `tauri/src/commands/migration.rs` (lines 14, 35 only).
- **Steps**: all 11 commands become async delegates. `fetch_game_cover_art` maps service `Unavailable` → `Ok(None)` (D12) while `Some(path)` passes through; mods-style adapters per the error table; `default_steam_client_install_path` and `list_proton_installs` delegate to `SteamService` (protonup/migration now call the service through `State<'_, SteamService>` — their behaviour is unchanged). Delete moved DTOs/helpers from `version.rs` (`VersionSnapshotInfo`, `VersionCheckResult`, `status_from_str`, `row_to_snapshot_info`, `locate_manifest_for_app`) and the env-walking body of `steam.rs`. Pins refreshed per module.
- **Acceptance**: pins green in all three modules + the two rewritten callers; `check-mock-coverage.sh` green (45 names unchanged).
- **VALIDATE**: `cargo test -p crosshook-native commands::version commands::steam commands::game_metadata && ./scripts/check-mock-coverage.sh`.

## Task 4.1: AC1 security tests + dead-code sweep (B4)

- **Owns**: `app/src/services/storage/tests/cleanup_security.rs`, and the removal of the six `#![expect(dead_code)]` headers in `services/<x>/mod.rs`.
- **Steps**: write the adversarial cleanup table (kept separate from 2.1's functional tests so B2 stays small): (a) requested tuple absent from the fresh scan — wrong kind, wrong prefix, wrong target, plus a duplicate — each yields a `skipped` entry with `not present in a fresh storage scan`, a fail-soft audit row, and **nothing deleted**; (b) target replaced by a symlink after the scan, both kinds, referent inside the staged root, outside it, and dangling — referent survives, target skipped with the D5 reason; (c) forged paths outside the prefix roots: `..` escape, target equal to a root, non-immediate staged child, sibling-prefix string confusion, compatdata nested-`pfx` link; (d) audit rows for accepted targets: per-target `deleted` rows, `skipped` rows with reason, `summary` row with the reclaimed total and its gate (also asserted when nothing was deleted and nothing reclaimed). Fixtures: two tempdir roots, prefixes with `drive_c/CrossHook`, staged children, out-of-root sentinels; `ProfileStore::with_base_path` + `MetadataStore::open_in_memory`; canonicalize fixture roots in expectations; no sleeps, no `set_var`. Then remove the `#![expect(dead_code)]` headers (the build fails if any code is still dead — forcing a real sweep).
- **Acceptance**: all four AC1 sub-criteria demonstrably green in one file; workspace builds with the `expect` attributes removed.
- **VALIDATE**: `cargo test -p crosshook-app services::storage::tests::cleanup_security && cargo clippy --workspace --all-targets -- -D warnings`.

## Task 5.1: Docs + follow-ups (B5)

- **Owns**: `app/README.md`, `docs/plans/app-storage-services/decisions.md` (status notes only, no decision changes), follow-up issue drafts (D15 list).
- **Steps**: add the six services to the README services table (one row each: owns, constructor, notes). Record D15 follow-ups as issues: rename umu/runtime reset; settings-failure inconsistency; appmanifest lookup dedup; SteamGridDB initial-URL validation in core; handle-pinned (openat/beneath) deletion for the ancestor-swap race; `offline_mode` consultation for art fetch; manual cleanup test remains manual. Note in decisions.md that D1–D16 all landed.
- **Acceptance**: README lists all 15 services; follow-ups filed; final `./scripts/format.sh --modified --docs` leaves no diff.
- **VALIDATE**: `./scripts/format.sh --modified --docs`.

## Final gate (from research-practices §6; main agent, end state)

```bash
# static analysis (repo root)
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
./scripts/lint.sh
./scripts/check-host-gateway.sh && ./scripts/check-host-gateway.sh --self-test
./scripts/check-app-deps.sh
./scripts/check-startup-path.sh
./scripts/check-schema-version.sh
./scripts/check-mock-coverage.sh

# rust tests
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::storage
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::collections
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launcher_export
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app --test launcher_export_golden   # AC3
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native                            # adapter + IPC pins

# frontend (AC5)
cd src/crosshook-native
npm run typecheck && npm test && npm run lint && npm run dev:browser:check && npm run test:smoke

# formatting
./scripts/format.sh --modified --docs
```

Worktree prerequisites: `npm install -D --no-save typescript@<project-version> biome` at the worktree root, then `cd src/crosshook-native && npm ci`. EXPECT: zero warnings; no host-gateway violations; `crosshook-app` free of UI-toolkit deps; schema unchanged; all 45 commands still mock-covered; goldens pass with `git diff --stat` empty under `app/tests/fixtures`; smoke suites green. Manual (issue test plan, out of band): prefix storage cleanup on a real test prefix.

## Risks

| Risk                                                                              | Mitigation                                                                                                                 |
| --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Fixture capture drift (tempdir paths inside bytes)                                | `@HOME@` substitution applied to both sides; determinism double-run asserted at capture time                               |
| `scan()` timeout changes behaviour on pathological filesystems                    | 300 s ceiling only converts a hang into an error; a normal scan is unaffected; Conflict on concurrent scans is D3-intended |
| D6 turns silent drops into visible skips                                          | additive `skipped` entries; UI renders them; PR body documents it                                                          |
| B1/B2 file ownership violated under parallelism                                   | ownership table above is exhaustive; `mod.rs` files are B1-frozen until 4.1 removes the `expect`                           |
| Cross-batch compile break from the D9 move (3.2 touches profile files others own) | 3.2 is the only task touching `commands/profile/*`; runs after 2.3 lands the service methods                               |
| async conversion changes Tauri's camelCase arg derivation                         | args are unchanged; only the fn kind changes; pins + mock coverage catch drift                                             |

## Notes

- Squash-merge title: `refactor(app): move storage, mods, collections, launcher export, version and Steam services into crosshook-app` (`Closes #539`).
- The PR body lists the three observable deltas: concurrent scan conflict (D3), skipped entries for rejected cleanup requests (D6), and the disabled-DB `Unavailable` strings for collections/version writes (D7) — all additive or error-path only.
