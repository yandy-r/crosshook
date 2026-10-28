# Feature Spec: protonup, Proton migration and umu services in `crosshook-app`

Tracking: Linear YAN-822 (M1-32), target `crosshook v0.7.0-alpha.1`, one PR to `main`, worktree `crosshook-yan-822`. Paths relative to `src/crosshook-native/`. Binding [decisions.md](./decisions.md) D1–D13 override research recommendations. Exact signatures: [b0-contracts.md](./b0-contracts.md); batches: [app-protonup-services.plan.md](./app-protonup-services.plan.md).

## Problem

15 Tauri commands in `src-tauri/src/commands/{protonup,migration,umu_database}.rs` carry catalog-identity checks, install orchestration, an unowned cancel registry, uninstall planning and A-4/A-5 path logic. The install identity check (`protonup_install_version_async`, requested version must match the server catalog's download URL and checksum identity) has **no test**, so a refactor could drop it silently. `ProtonInstallRegistry` lets any webview cancel any `op_id`. `JobId` is a bearer capability with no owner. Uninstall errors, install failure progress and umu refresh errors serialize raw paths and URLs, and a caller-supplied `steam_client_install_path` can mint an uninstall root. Native consumers need toolkit-independent services, not Tauri `State` orchestration.

## Scope

Three `#[derive(Clone)]` services under `crates/crosshook-app/src/services/{protonup,proton_migration,umu}/` (D1): positional `new()`, async methods, `run_blocking` for filesystem/store work, `AppError` only, `tests/` subdirectories, files ≤ ~500 lines, `assert_impl_all!(…: Send, Sync)`. `Services` gains `protonup`, `proton_migration`, `umu`; three `.manage()` lines are added and `ProtonInstallRegistry` is deleted. The Tauri commands become thin delegates (D2). The startup umu refresh job calls `services.umu.refresh()` (D11).

## Non-goals (D13)

TS snake_case arg drift (`force_refresh`, `steam_client_install_path` in three callers; native drops them silently today and still will), redirect-hop allowlisting for Proton downloads, mid-extract cancellation, an offline gate for umu refresh, manual Proton-GE install/cancel from the issue test plan, `openat2` race-free deletion, any frontend or mock change. No schema/TOML change: `proton_release_catalog` (v22/v23), `umu_gameid_lookup_cache` (v24) and the schema version stay; install progress and the owner ledger are runtime-only.

## Acceptance criteria and test mapping

| #   | Criterion (issue)                                          | Evidence                                                                                                                                                                                                                             |
| --- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| AC1 | Install rejected when version missing from fetched catalog | `protonup/tests/security/identity.rs` missing version, provider mismatch, both entry points; `tests/identity.rs` pure cases                                                                                                          |
| AC2 | Rejected when download URL differs (host or path)          | `security/identity.rs` host, path, Some→None, query; each asserts zero installer calls and `jobs.job_counts() == (0,0)`                                                                                                              |
| AC3 | Rejected when checksum identity differs                    | `security/identity.rs` checksum URL host/path/None, checksum kind; authoritative catalog entry installed, caller `asset_size`/`release_url` ignored                                                                                  |
| AC4 | Cancel with a foreign `op_id` rejected                     | `security/cancel.rs` foreign owner (job uncancelled), unrelated JobRegistry id (untouched), own cancel → `Cancelled` outcome, unknown/finished → `NotFound`                                                                          |
| AC5 | Uninstall plan refuses paths outside resolved roots        | `security/uninstall.rs` real temp dirs: `..` ending outside and inside, final/intermediate symlink escape, sibling-prefix root, root itself, nested child, fake caller Steam root ignored, sentinels untouched, error text path-free |
| AC6 | Migration apply/batch async, same TOML (golden)            | `tests/profile_writer_golden.rs` scenarios `migrate`, `batch_migrate` captured from **unmodified** code (Phase 0), then driven through `ProtonMigrationService`                                                                      |
| AC7 | Commands delegate; Proton Manager smoke passes             | `command_names_match_expected_ipc_contract` in three command files; `./scripts/check-mock-coverage.sh`; `npm run typecheck`, `npm test`, `npm run test:smoke`                                                                        |

Further D12 tests: `install.rs` (op_id returned before the job finishes, synthetic `Failed`, `Conflict` on duplicate, offline `Unavailable`, owner ledger empty after completion, progress `Lagged` continue), `catalog.rs`, `uninstall.rs`, umu refresh error sanitized, migration A-4/A-5.

## Architecture

**Identity (D3).** `verify_install_identity(request, supplied, catalog)` is pure: find by `(provider, version)`; with `Some(supplied)` compare provider, version, `download_url`, `checksum_url`, `checksum_kind` by exact `Option<String>` equality. `start_install` (supplied) and legacy `install_version` (`None`) both call it; the job installs the **catalog** entry. The three existing async `Err` texts are verbatim; legacy missing version stays `Ok` with `DependencyMissing`.

**Seams (D4).** Boxed-future traits `ProtonCatalogSource`, `ProtonInstaller`, `UmuRefresher` plus an app-private `ProtonRootSource` (trusted Steam roots and install-root candidates), each with a core-backed default and a `#[cfg(test)]` constructor. No core HTTP change. Core `plan_uninstall`/`resolve_install_root_candidates` read `$HOME` and their testable cores are `pub(crate)`; tests therefore inject temp roots through `ProtonRootSource` and pass the trusted temp Steam root to core, so **no test mutates process env** (core still also derives the real `$HOME` candidates, which tests never target).

**Install job and ownership (D5, D6).** `start_install(owner, request, version)` verifies identity and the offline flag, then starts `JobRegistry` job `protonup-install` keyed `protonup-install:<provider>:<version>` (duplicate → `Conflict`). The job body registers `JobId → InstallOwner` in the service ledger before it hands back the progress receiver; a drop guard removes the entry. The `JobId` string is the `op_id`. Core `ProgressEmitter` events are relayed (path-free messages, `Lagged` ⇒ continue); a synthetic `Phase::Failed` fires when the installer ends without a terminal phase. `cancel_install` reaches `JobRegistry::cancel` only for ledger ids owned by the caller; unknown and foreign ids are the same `NotFound "install operation was not found"`. The adapter derives the owner from the injected `tauri::Webview` label (never a JS arg), pumps progress to `protonup:install:progress`, and maps `NotFound` to `Ok(false)`. Webview-label ownership isolates windows, not contexts inside one window.

**Uninstall (D7, D8).** The caller's Steam path is honoured only if it canonically equals a backend-discovered root; `tool_path` rejects raw `..`, must pass `ensure_within_workspace(candidate_root, tool_path)` against a valid install root, then core `plan_uninstall`/`execute_uninstall_for_path` (re-plans before `remove_dir_all`) remain the containment authority. Responses use fixed texts; details go to `tracing`.

**Migration and umu (D10, D11).** `check` moves the A-4/A-5 logic into `run_blocking`; `apply`/`apply_batch` delegate to `ProfileWriter`. `UmuService::{refresh, coverage, clear_gameid_cache}`; refresh errors become one fixed text.

**Offline (D9).** `offline_mode` ⇒ `start_install` is `Unavailable "offline mode is enabled"`; legacy install returns `Ok(success:false, NetworkError, same text)`; `list_available_versions` forces `force_refresh=false`. Core has no public cache-only catalog read, so an expired cache may still try one live fetch before the stale fallback (recorded, D9 fallback clause). umu refresh and GAMEID lookup are unchanged.

## Intentional behaviour changes (everything else is IPC-identical)

Path-free uninstall/install/umu error texts; owner-checked cancel; `Conflict` on a duplicate concurrent install of the same version; synthetic `Failed` progress instead of a hung "resolving"; backend-resolved uninstall roots plus `..`/intermediate-symlink rejection; offline install gate; progress pump survives `Lagged`.

## Risks

| Risk                                                | Mitigation                                                                                                      |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Parallel slices clobber shared files                | Exhaustive per-batch ownership; `services/mod.rs`, `lib.rs`, `stores.rs` only in task 1.3; no git state changes |
| Golden fixtures regenerated from refactored code    | Phase 0 captures before any service code exists; fixtures frozen afterwards                                     |
| Owner entry leaks or races instantly-finishing jobs | Guard created inside the job body before the receiver is returned; `owner_count() == 0` asserted after finish   |
| Frontend snake_case drift hides native arg loss     | Out of scope; filed as D13 follow-up; command pins prevent accidental renames                                   |
| Webview label isolates windows only                 | Documented limit; a backend-issued session token would change the IPC (non-goal)                                |

## Research references

[technical](./research-technical.md) · [security](./research-security.md) · [contract](./research-contract.md).
