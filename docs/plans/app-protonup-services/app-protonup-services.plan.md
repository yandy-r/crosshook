# Plan: app-protonup-services — protonup, Proton migration and umu services in `crosshook-app` (YAN-822 / M1-32)

Tracking: Linear YAN-822. Release `crosshook v0.7.0-alpha.1`, one PR to `main`. Paths relative to `src/crosshook-native/`; `app/` = `crates/crosshook-app`, `core/` = `crates/crosshook-core`, `tauri/` = `src-tauri`. Binding [decisions.md](./decisions.md) D1–D13; [feature-spec.md](./feature-spec.md); exact [b0-contracts.md](./b0-contracts.md) (read it before any task).

## Summary

Move the orchestration of 15 Tauri commands into `ProtonUpService`, `ProtonMigrationService` and `UmuService`. The install identity check, owner-checked cancel and uninstall root authority become tested service behaviour; install becomes a `JobRegistry` job keyed per provider/version with core progress relayed to the unchanged `protonup:install:progress` event. IPC names, arg keys, serde shapes and the `bool` cancel return are frozen (D2); error texts become path-free (D8); the golden migration TOML is captured from unmodified code first (D10).

## Batches and ownership contract

Tasks in a batch own **disjoint files, including `tests/mod.rs`**. Batches run in order. Every agent uses the absolute worktree root `/home/yandy/Projects/github.com/yandy-r/crosshook-yan-822`, resolves **all** paths absolutely (relative paths have written into the main checkout before) and never runs `git stash/checkout/reset/commit`. Subagents skip build/test/lint/format mid-flight; Main runs each gate once after the batch lands. A wrong contract goes to Main; never edit another slice's file.

| Batch | Tasks              | Depends on | Width | Contents                                                                                        |
| ----- | ------------------ | ---------- | ----- | ----------------------------------------------------------------------------------------------- |
| P0    | 0.1                | —          | 1     | Golden capture: `migrate` + `batch_migrate` fixtures from unmodified code.                      |
| B0    | planning artifacts | research   | 1     | This plan, spec and `b0-contracts.md` (done).                                                   |
| B1    | 1.1, 1.2, 1.3      | P0, B0     | 3     | Contracts/skeletons that compile: types, seams, pure helpers, struct shells, `Services` fields. |
| B2    | 2.1, 2.2, 2.3, 2.4 | B1         | 4     | Service behaviour + critical tests (install/catalog/cancel, uninstall, migration, umu + scans). |
| B3    | 3.1, 3.2           | B2         | 2     | Tauri adapters/cutover and docs/follow-ups.                                                     |

**Shared-file ownership is exact.** `app/src/services/mod.rs`, `app/src/lib.rs`, `app/src/bootstrap/stores.rs`, `app/src/bootstrap/stores/tests.rs` → **only 1.3**. `app/src/error.rs` is not edited (service-private mappers). `app/src/bootstrap/scans.rs` → only 2.4. `app/tests/profile_writer_golden.rs` + `tests/fixtures/profile_writer/*` → 0.1, then only 2.3 (append service scenarios; never change a fixture). `tauri/src/lib.rs`, `tauri/src/commands/{protonup,migration,umu_database,shared}.rs`, `tauri/Cargo.toml` → only 3.1. No `#[allow(dead_code)]`, no no-op stubs, no behaviour in B1.

**Intermediate-compile rule.** B1 leaves pure helpers/fields temporarily unused and `mod` lines for B2 files are appended by B2 tasks, so the **B1 gate is `cargo check`** (warnings allowed). From the B2 gate on, `clippy -D warnings` must pass. Within B2 the `mod` declarations for sibling files are written by their declared owner (2.1 declares protonup's B2 files including 2.2's; 2.3 and 2.4 declare their own) so Main's batch gate sees one consistent graph.

## Task 0.1: Golden capture from unmodified code (P0)

- **Owns:** `app/tests/profile_writer_golden.rs`, new `app/tests/fixtures/profile_writer/{migrate,batch_migrate}.toml`.
- **Change:** add scenarios `migrate_matches_pre_refactor_bytes` and `batch_migrate_matches_pre_refactor_bytes` to the existing `harness = false` binary, using `ProfileWriter::apply_proton_migration` / `apply_batch_migration` on the unchanged writer. Seed `Alpha` (`proton_run`, `runtime.proton_path = "/proton/GE-Proton9-20/proton"` is stale): single case applies `RuntimeProtonPath` with a replacement file created under the `HOME` tempdir (the harness already points `HOME` at a tempdir); batch case adds a `Beta` copy of the seed, one `Applied` runtime item for `Alpha`, one `AlreadyValid` item for `Beta` (its `runtime.proton_path` set to an existing file) and one `steam.proton_path` item. Because the replacement path embeds the random `HOME`, compare after replacing that exact prefix with `$HOME` on the **actual** bytes only (add `assert_golden_normalized`); fixtures contain `$HOME`. Also assert the returned `MigrationApplyResult`/`BatchMigrationResult` counts. Extend `TESTS` (`[(&str, fn); 6]`). Capture by running the new scenarios once against the unmodified tree, inspect the produced TOML, and commit it as the fixture; the fixtures are frozen afterwards (never regenerate from refactored code).
- **Acceptance:** `cargo test -p crosshook-app --test profile_writer_golden` passes with six scenarios; fixtures differ from `seed.toml` in exactly the migrated `proton_path` lines.

### P0 gate (Main)

```sh
# from src/crosshook-native
cargo test -p crosshook-app --test profile_writer_golden
```

## Task 1.1: protonup foundation (B1)

- **Owns:** `app/src/services/protonup/{mod,types,seams,identity,messages,owners,progress}.rs`, `protonup/tests/{mod,helpers,identity,messages,owners,progress}.rs`.
- **Change:** exactly b0 §3, §8, §9: `InstallOwner`, DTOs, `ProtonInstallStart`, the four seam traits with `Core*` defaults, `ProtonUpService` struct with `new`, `with_seams`, `jobs()`, `owner_count()`, `assert_impl_all!`; pure `verify_install_identity`, `OwnerLedger`/`OwnerGuard`, `next_progress`/`forward_progress`, all fixed texts and mappers; test `Harness`, `FakeCatalog`, `FakeInstaller` (modes incl. `Gated`), `FakeRoots`, `finished()`. `mod.rs` declares only B1 modules; no `catalog/roots/install/cancel/uninstall` files exist yet.
- **Tests (B1, pure):** identity table (each field mutated alone; `release_url`/`asset_size`/`published_at` accepted; returns the catalog entry), owners (authorize own/foreign/unknown share one error, guard removes on drop incl. panic path), progress (`Lagged` continues, `Closed` ends), messages (every mapper output has no `/` and no `http`).
- **Acceptance:** compiles under `cargo check`; no behaviour methods on the service.

## Task 1.2: migration and umu shells (B1)

- **Owns:** `app/src/services/proton_migration/mod.rs`, `app/src/services/umu/{mod,seams}.rs`.
- **Change:** b0 §4/§5 structs, positional `new`, `UmuRefresher` + `CoreUmuRefresher`, `with_refresher` (`#[cfg(test)]`), `assert_impl_all!`. No methods, no `tests` module (B2 adds them with their owners).
- **Acceptance:** compiles; core default calls `crosshook_core::umu_database::refresh_umu_database()`.

## Task 1.3: service wiring (B1)

- **Owns:** `app/src/services/mod.rs`, `app/src/lib.rs`, `app/src/bootstrap/stores.rs`, `app/src/bootstrap/stores/tests.rs`.
- **Change:** b0 §2 and §6: `BoxFuture` alias, three `pub mod` + `pub use` lines (alphabetical), `lib.rs` re-exports, three `Services` fields with doc comments and constructors (shared `steam`, `protonup` before `storage`, `proton_migration` before `lutris`), extend `services_new_builds_bundle_from_stores` with the shared-registry Conflict assertion for key `protonup-install:x:y`.
- **Acceptance:** `Services::new` builds; no other file touched.

### B1 gate (Main)

```sh
# from src/crosshook-native; warnings allowed in B1 (documented intermediate-compile rule)
cargo check --workspace --all-targets
cargo test -p crosshook-app protonup
cargo test -p crosshook-app services_new_builds_bundle_from_stores
```

## Task 2.1: protonup install, catalog, roots, cancel (B2)

- **Owns:** `protonup/{catalog,roots,install,cancel}.rs`, `protonup/mod.rs` (append `mod` lines for all B2 protonup files incl. `uninstall`), `protonup/tests/{mod,security,install,catalog}.rs`, `protonup/tests/security/{identity,cancel}.rs`.
- **Change:** b0 §3 and §3.1 verbatim (algorithm order is binding): `flags`, `list_providers`, `list_available_versions`, `resolve_install_roots`, `get_suggestion`, legacy `install_version`, `start_install` + job body + reaper, `cancel_install`. `tests/security.rs` declares `identity`, `cancel` and (2.2's) `uninstall`.
- **Critical tests (D12):** `security/identity.rs` AC1–AC3 incl. both entry points, authoritative entry used (assert the fake installer saw the **catalog** `asset_size`/`release_url`), every rejection → `Harness::assert_rejected`; `security/cancel.rs` AC4 (gated installer: foreign owner → `NotFound MSG_OP_NOT_FOUND`, token uncancelled, release → `Succeeded`; unrelated `jobs.start` id → same error, job untouched; own cancel → `Outcome::Cancelled`; completed id → `NotFound`); `install.rs` op_id usable before completion, progress reaches `Done`, path-bearing failure becomes fixed text, synthetic `Failed`, duplicate key `Conflict`, offline `Unavailable` (no job) plus legacy offline result, ledger empty and `job_counts() == (0,0)` after `finished()`; `catalog.rs` default provider, offline forces `force_refresh=false`, prerelease flag read from settings, stale/offline pass-through.
- **Acceptance:** AC1–AC4 evidence; no env mutation; no timing sleeps.

## Task 2.2: protonup uninstall (B2)

- **Owns:** `protonup/uninstall.rs`, `protonup/tests/{uninstall.rs}`, `protonup/tests/security/uninstall.rs`.
- **Change:** b0 §3.2: root authority via `ProtonRootSource`, raw-`..` rejection, `ensure_within_workspace` per valid candidate, core `plan_uninstall`/`execute_uninstall_for_path`, fixed texts. `uninstall` returns the same structured shape as today (`success:false` on refusal, `Ok` always).
- **Critical tests (AC5):** all D12 AC5 cases against real temp dirs with a `FakeRoots` whose trusted Steam root is `<tmp>/Steam`: `..` ending outside/inside, final and intermediate symlink escape (unix-only), sibling-prefix root, root itself, nested child, fake caller-supplied Steam root ignored, valid direct child plan + execute (only that dir removed, outside sentinels intact), `tool/proton` file input normalizes, error text contains neither tmp path nor `/`. `uninstall.rs`: `conflicting_app_ids` surfaced from a `config/config.vdf` fixture, failure text fixed.
- **Acceptance:** AC5; core containment code unchanged.

## Task 2.3: proton_migration behaviour and golden driver (B2)

- **Owns:** `proton_migration/{check,apply}.rs`, `proton_migration/mod.rs` (append `mod` lines), `proton_migration/tests/{mod,check,apply}.rs`, `app/tests/profile_writer_golden.rs` (append only).
- **Change:** b0 §4: move A-4/A-5 logic verbatim into `check`; `apply`/`apply_batch` delegate. Append `migrate_service_…` and `batch_migrate_service_…` scenarios that drive the **service** over the same inputs and assert bytes equal the P0 fixtures.
- **Tests:** `check.rs` — supplied path without `steamapps` falls back (use `<tmp>/steamapps` positive case; negative asserts fallback path was used via `installed_proton_versions` contents), A-4 sanitization with `HOME` tempdir read only via `display_path_tilde` expectations built from `std::env::var("HOME")`, stale-profile suggestion; `apply.rs` — invalid name → `Validation`, vanished replacement → error, single/batch outcomes and one `Migrated` event.
- **Acceptance:** AC6 byte-identical; no fixture edited.

## Task 2.4: umu behaviour and startup scheduling (B2)

- **Owns:** `umu/{refresh,coverage}.rs`, `umu/mod.rs` (append `mod` lines), `umu/tests/{mod,refresh,coverage}.rs`, `app/src/bootstrap/scans.rs`.
- **Change:** b0 §5. `schedule_umu` takes `&Services` (call site in `schedule` already has it), clones `services.umu` into the job, keeps `UmuRefresh::Skip`/bench behaviour and the existing `UmuRefreshSummary` payload; failure message = `error.message()`.
- **Tests:** fake refresher ok (`refreshed:true`), 304-style `refreshed:false`, `Network`/`Io` errors → `UMU_REFRESH_FAILED` with no URL or path in message and correct `AppErrorKind`; coverage blank/None → `Unknown`; `clear_gameid_cache` returns deleted row count (seed with `UmuGameIdCacheRow::found`, in-memory metadata). Existing bootstrap tests (`UmuRefresh::Skip`) remain unchanged.
- **Acceptance:** bootstrap job no longer references `crosshook_core::umu_database` directly.

### B2 gate (Main)

```sh
# from src/crosshook-native; one filter per cargo invocation
cargo test -p crosshook-app protonup
cargo test -p crosshook-app proton_migration
cargo test -p crosshook-app umu
cargo test -p crosshook-app --test profile_writer_golden
cargo test -p crosshook-app --test bootstrap_ordering
cargo clippy --workspace --all-targets -- -D warnings
```

## Task 3.1: Tauri adapters and cutover (B3)

- **Owns:** `tauri/src/commands/{protonup,migration,umu_database,shared}.rs`, `tauri/src/lib.rs`, `tauri/Cargo.toml` (drop `uuid`, `tokio-util`; verify with grep first).
- **Change:** b0 §7 table. 15 commands keep names and arg spellings; `protonup_install_version_async` and `protonup_cancel_install` gain an injected `webview: tauri::Webview`; the pump uses `next_progress`; add three `.manage()` lines and delete the `ProtonInstallRegistry` manage (lib.rs:71-73) and type; delete `sanitize_display_path` if unused; add three `command_names_match_expected_ipc_contract` tests naming all 15 commands, plus `map_app_error` and owner-derivation tests.
- **Acceptance:** AC7 delegation; `git grep ProtonInstallRegistry` empty; no frontend or mock edits.

## Task 3.2: Documentation and follow-ups (B3)

- **Owns:** repo-root `CLAUDE.md` and `AGENTS.md` (architecture service list, new protonup/migration/umu paragraph mirroring the community one), `app/README.md`.
- **Change:** document the three services, the 15 delegating commands, `protonup-install` job kind/key, owner-ledger and webview-label limits, path-free errors, offline gates, `ProtonRootSource`. State the D13 follow-ups (Main files them via GitHub/Linear; do not change their scoped-out behaviour). No changelog edit (generated from the Conventional Commit).
- **Acceptance:** docs match shipped behaviour; no claim of race-free deletion or per-context cancel isolation.

### B3 gate and AC mapping (Main)

```sh
# from src/crosshook-native
cargo test -p crosshook-app protonup
cargo test -p crosshook-app proton_migration
cargo test -p crosshook-app umu
cargo test -p crosshook-app services_new_builds_bundle_from_stores
cargo test -p crosshook-native command_names_match_expected_ipc_contract
cargo clippy --workspace --all-targets -- -D warnings
```

| AC  | Evidence                                                                                 | Batch    |
| --- | ---------------------------------------------------------------------------------------- | -------- |
| AC1 | `protonup/tests/security/identity.rs` (missing version, provider mismatch, both entries) | B2 (2.1) |
| AC2 | same file, URL host/path/None/query rejections                                           | B2 (2.1) |
| AC3 | same file, checksum URL/kind rejections; authoritative entry used                        | B2 (2.1) |
| AC4 | `protonup/tests/security/cancel.rs`                                                      | B2 (2.1) |
| AC5 | `protonup/tests/security/uninstall.rs`                                                   | B2 (2.2) |
| AC6 | `profile_writer_golden` fixtures (P0) + service scenarios (2.3)                          | P0, B2   |
| AC7 | command pin tests, `check-mock-coverage.sh`, smoke                                       | B3/final |

## Final gate (Main, once integrated)

```sh
# from repository root
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native
./scripts/lint.sh
./scripts/check-mock-coverage.sh
./scripts/check-host-gateway.sh
./scripts/check-app-deps.sh
./scripts/check-schema-version.sh

# from src/crosshook-native (needs `npm ci`; first smoke run needs `npm run test:smoke:install`)
npm run typecheck && npm test
npm run test:smoke
```

Format changed files with the repo formatters (`biome format --write` for TS, `cargo fmt` for Rust) before the final check. Prerequisites: the worktree has no `node_modules` until `npm ci`.

## Risks and delivery

- Frontend snake_case drift (D13) persists natively; pins stop accidental Rust renames only. Webview-label ownership separates windows, not contexts in one window.
- Core cannot serve a cache-only catalog read; offline `list_available_versions` may still attempt one live fetch on an expired cache (documented, D9 clause).
- Root authority relies on `HOME`-derived core candidates; tests avoid env mutation through `ProtonRootSource` and an explicit Steam root.
- Squash title: `refactor(app): move protonup, Proton migration and umu into services`; body `Closes` YAN-822/GitHub issue per orchestrator; report only checks actually run, the D13 follow-ups, and the intentional behaviour deltas listed in the spec.
