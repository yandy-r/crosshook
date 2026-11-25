# Tauri IPC async command audit (M0-03)

This document audits all `#[tauri::command]` handlers in `src/crosshook-native/src-tauri/src/commands/` against the `v0.6.1` synchronous IPC ranking in [Baseline A](A/sync-command-ranking.md) for [YAN-809](https://linear.app/itcn/issue/YAN-809).

## Enforcement

All **152** `#[tauri::command]` handlers in `src/crosshook-native/src-tauri/src/commands/` are `async fn`.

- **Zero-sync policy:** [`scripts/check-async-commands.sh`](../../../scripts/check-async-commands.sh) (introduced in YAN-832 and run by `./scripts/lint.sh` and CI, with `--self-test` fixture coverage) rejects any non-async `#[tauri::command]` declaration. Because zero sync handlers are allowed, no Rust sync allowlist (`sync_allowlist.rs`) is needed.
- **Blocking-pool boundary:** `crosshook-app` offloads blocking filesystem, SQLite, settings, and host-subprocess work onto Tokio's blocking pool via `crosshook_app::services::run_blocking` (`tokio::task::spawn_blocking`, mapping panics and cancellations to sanitized `AppError`s) or supervised `run_job` workers.

## Blocking-on-worker gaps fixed in M0-03

Auditing `main` after the `crosshook-app` refactor found four command paths that were `async fn` at the Tauri boundary (off the GTK main thread) but still performed synchronous disk, SQLite, or host-subprocess work directly on a Tokio worker thread:

1. `metadata_store_status` (`src-tauri/src/commands/metadata.rs` -> `DiagnosticsService::metadata_status`): `MetadataStore::status()` locks the SQLite connection mutex and runs `PRAGMA user_version`. Moved into `run_blocking`.
2. `validate_launch` (`src-tauri/src/commands/launch.rs` -> `LaunchService::validate`): `launch::validate` calls `Path::is_file()` and, under Flatpak, spawns blocking `flatpak-spawn --host test`. Moved into `tokio::task::spawn_blocking`, with `JoinError` mapped to a sanitized fatal `LaunchValidationIssue`.
3. `preview_launch` (`src-tauri/src/commands/launch.rs` -> `LaunchService::preview`): `enrich_umu_gameid` loaded `SettingsStore` synchronously, `umu_database::resolve_umu_game_id` ran synchronous `should_use_umu` probes and SQLite cache reads/writes, and `build_launch_preview` ran `validate_all` synchronously. Moved settings load, UMU probe/cache operations, and `build_launch_preview` onto Tokio's blocking pool (keeping live UMU HTTP lookups async).
4. `launch_game` / `launch_trainer` (`src-tauri/src/commands/launch.rs` -> `LaunchService::prepare`, `start_game`, `start_trainer`, `run_session`): validation, environment detection (`LaunchEnv::detect`), log-file creation (`create_log_path_in`), runtime helper discovery (`resolve_helper_with`), command construction (MangoHud config reads and trainer staging copies), and hook-context preparation (`build_launch_hook_execution_context`) ran on the Tokio worker before spawning. Moved into `run_blocking`.

## Mapping of M0-02 ranked commands

Every command observed synchronously in [`A/sync-command-ranking.md`](A/sync-command-ranking.md) (sorted by `v0.6.1` max duration):

| M0-02 command                               | `v0.6.1` max (ms) | Current status on `main`                                                   | Execution boundary                                |
| ------------------------------------------- | ----------------: | -------------------------------------------------------------------------- | ------------------------------------------------- |
| `community_sync`                            |           192.163 | `async fn` -> `CommunityService::sync`                                     | Supervised job + `run_blocking`                   |
| `batch_validate_profiles`                   |           129.620 | `async fn` -> `HealthService::batch_summary`                               | `run_job` (`spawn_blocking`)                      |
| `check_generalized_readiness`               |            13.122 | `async fn` -> `OnboardingService::check_generalized_readiness`             | `run_job` (`spawn_blocking`)                      |
| `profile_list_summaries`                    |            12.837 | `async fn` -> `ProfileWriter::list_summaries`                              | `run_blocking`                                    |
| `community_list_profiles`                   |            11.651 | `async fn` -> `CommunityService::list_profiles`                            | `run_blocking`                                    |
| `list_launchers`                            |             7.296 | `async fn` -> `LauncherExportService::list`                                | `run_blocking`                                    |
| `check_game_running`                        |             6.955 | Removed in PR #628 (`f8e291ea`)                                            | Replaced by event-driven `launch-session-changed` |
| `discovery_catalog`                         |             4.157 | `async fn` -> `DiscoveryService::catalog`                                  | `run_blocking`                                    |
| `community_list_indexed_profiles`           |             3.875 | `async fn` -> `CommunityService::list_indexed_profiles`                    | `run_blocking`                                    |
| `get_cached_offline_readiness_snapshots`    |             1.580 | `async fn` -> `HealthService::cached_offline_readiness_snapshots`          | `run_blocking`                                    |
| `get_capabilities`                          |             0.779 | `async fn` -> `OnboardingService::capabilities`                            | `run_job` (`spawn_blocking`)                      |
| `get_cached_health_snapshots`               |             0.777 | `async fn` -> `HealthService::cached_health_snapshots`                     | `run_blocking`                                    |
| `profile_list`                              |             0.434 | `async fn` -> `ProfileWriter::list`                                        | `run_blocking`                                    |
| `list_launch_history_for_profile`           |             0.369 | `async fn` -> `LaunchService::launch_history`                              | `run_blocking`                                    |
| `list_running_profiles`                     |             0.180 | Replaced by `list_running_sessions`                                        | In-memory `LaunchSessionRegistry` snapshot        |
| `profile_set_favorite`                      |             0.156 | `async fn` -> `ProfileWriter::set_favorite`                                | `ProfileWriter::commit` (`run_blocking`)          |
| `profile_load`                              |             0.155 | `async fn` -> `ProfileWriter::load` + `CollectionsService::apply_defaults` | `run_blocking`                                    |
| `profile_list_favorites`                    |             0.147 | `async fn` -> `ProfileWriter::list_favorites`                              | `run_blocking`                                    |
| `detect_protontricks_binary`                |             0.127 | `async fn` -> `PrefixDepsService::detect_binary`                           | `run_blocking`                                    |
| `collection_list`                           |             0.122 | `async fn` -> `CollectionsService::list`                                   | `run_blocking`                                    |
| `get_cached_host_readiness_snapshot`        |             0.111 | `async fn` -> `OnboardingService::cached_host_readiness_snapshot`          | `run_blocking`                                    |
| `profile_list_bundled_optimization_presets` |             0.089 | `async fn` -> `ProfileWriter::list_bundled_optimization_presets`           | `run_blocking`                                    |
| `metadata_store_status`                     |             0.057 | `async fn` -> `DiagnosticsService::metadata_status`                        | `run_blocking` (fixed in M0-03)                   |
| `list_proton_installs`                      |             0.049 | `async fn` -> `SteamService::list_proton_installs`                         | `run_blocking`                                    |
| `check_launcher_exists`                     |             0.030 | `async fn` -> `LauncherExportService::check_exists`                        | `run_blocking`                                    |
| `protonup_list_providers`                   |             0.022 | `async fn` -> `ProtonUpService::list_providers`                            | Pure static provider descriptors (in-memory)      |
| `default_steam_client_install_path`         |             0.021 | `async fn` -> `SteamService::default_client_install_path`                  | `run_blocking`                                    |
| `get_trainer_type_catalog`                  |             0.017 | `async fn` -> `OfflineService::trainer_type_catalog`                       | Pure static catalog descriptors (in-memory)       |

Additional commands named in M0-03's original scope (`get_profile_health`, `community_prepare_import`, `probe_host_tool_details`, `export_launchers`, `export_diagnostics`, `profile_config_diff`, `apply_batch_migration`, `lutris_import_profiles`, `scan_prefix_storage`, `check_offline_readiness`, `batch_offline_readiness`) are all `async fn` delegating to `crosshook-app` services backed by `run_blocking` or `run_job`.

## Synchronous service calls inside `async fn` commands

Of the 152 `async fn` Tauri commands, 138 `.await` a blocking-pool or async service operation (`run_blocking`, `run_job`, `spawn_blocking`, or async I/O such as the document portal). The remaining 14 execute without `.await` because they perform no production disk, SQLite, network, or subprocess I/O:

| Command                              | Module                       | Reason no `.await` is required                                    |
| ------------------------------------ | ---------------------------- | ----------------------------------------------------------------- |
| `list_running_sessions`              | `commands/launch.rs`         | In-memory `LaunchSessionRegistry` snapshot under short mutex      |
| `launch_reset_sessions`              | `commands/launch.rs`         | In-memory `LaunchSessionRegistry` cancel broadcast                |
| `build_steam_launch_options_command` | `commands/launch.rs`         | Pure in-memory string assembly over static catalogs               |
| `check_gamescope_session`            | `commands/launch.rs`         | Reads process environment variables only                          |
| `get_trainer_type_catalog`           | `commands/offline.rs`        | Returns compiled static catalog entries (M0-02 p95 0.017 ms)      |
| `protonup_list_providers`            | `commands/protonup.rs`       | Returns compiled static provider descriptors (M0-02 p95 0.022 ms) |
| `protonup_cancel_install`            | `commands/protonup.rs`       | In-memory owner check + `CancellationToken::cancel()`             |
| `cancel_run_executable`              | `commands/run_executable.rs` | In-memory owner lookup + `CancellationToken::cancel()`            |
| `stop_run_executable`                | `commands/run_executable.rs` | In-memory owner lookup + kill `CancellationToken::cancel()`       |
| `cancel_update`                      | `commands/update.rs`         | In-memory owner lookup + `CancellationToken::cancel()`            |
| `bench_ready`                        | `commands/bench.rs`          | Benchmark-only (`CROSSHOOK_BENCH=1`); no-op in production         |
| `bench_begin`                        | `commands/bench.rs`          | Benchmark-only (`CROSSHOOK_BENCH=1`); no-op in production         |
| `bench_mark`                         | `commands/bench.rs`          | Benchmark-only (`CROSSHOOK_BENCH=1`); no-op in production         |
| `bench_clock`                        | `commands/bench.rs`          | Benchmark-only (`CROSSHOOK_BENCH=1`); no-op in production         |

## Benchmark scope note (G6 / E2-D)

No `docs/internal/bench/B/` G6 stall capture is recorded here:

- Per [Baseline A summary](A/summary.md), [`README.md`](README.md#what-e1-cannot-produce), and [`environments.md`](environments.md), G6 has no capture producer (`perf` is unavailable on E1 and kernel tracing is root-only), so G6 is waived on E1.
- Steam Deck (E2-D / E2-G) hardware is unavailable and out of scope per [`environments.md`](environments.md).
