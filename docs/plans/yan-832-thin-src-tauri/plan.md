# Plan: YAN-832 / GH #574 (M1-08): thin `src-tauri` to `crosshook-app` delegations

Paths are relative to `src/crosshook-native/` unless noted. Every decision is listed as **D#: decision. _Why:_ reason.**

## Baseline (main @ 4ef71a6)

- `src-tauri/src` non-test lines (everything before the first `#[cfg(test)]`, test-only files excluded): **4,182**.
- 163 `#[tauri::command]` handlers: 151 async, 12 sync (`metadata_store_status`, `get_background_protection_state`, `get_trainer_type_catalog`, `launch_reset_sessions`, `protonup_list_providers`, `protonup_cancel_install`, `validate_launch`, `build_steam_launch_options_command`, `check_game_running`, `list_running_sessions`, `cancel_run_executable`, `stop_run_executable`).
- `lib.rs` manages 5 stores, 29 services, the background holder and `BootstrappedApp` separately (35 `.manage` calls).
- `map_app_error`/`map_error` is copy-pasted in 18 command files.
- Wire DTOs and pure event mapping live in the adapter: `AppSettingsIpcData` (settings.rs), `LaunchSessionChanged`/`RunningSessionsDto` (session_bridge.rs), startup-job payload mapping (startup_bridge.rs), launch relay mapping and payloads (commands/launch/relay), `RunCommandError` and run payloads (run_executable.rs), `batch_lines` (process_relay.rs), update/prefix-dep completion payloads, bench env/plan logic (bench.rs).
- `check_game_running` calls `crosshook_core::launch::is_process_running` (spawns `ps`) on the GTK main thread.

## Decisions

- **D1: The one managed state is the `BootstrappedApp`.** Every command takes `app: State<'_, BootstrappedApp>` and calls `app.services().<service>.<method>(..)`. All other `.manage(..)` calls are deleted. _Why:_ `BootstrappedApp` is already managed, is `Clone`, derefs to `App` and exposes `services()`/`stores()`/`startup()`; adding a second wrapper type adds nothing.
- **D2: Every command is `async`.** Commands that borrow `State` and returned a bare value now return `Result<T, String>`. _Why:_ Tauri v2 requires a `Result` return for async commands with borrowed arguments, and `Ok(T)` serializes to exactly the same JSON as `T`, so the wire shape is unchanged.
- **D3: `check_game_running` moves behind `LaunchService::is_game_running(exe_name) -> bool` (async, `spawn_blocking`, trimmed-empty → `false`).** _Why:_ it is polled every 3 s and spawns a host `ps`; it must not run on the main thread, and core calls belong in the service.
- **D4: `metadata_store_status` delegates to a service that already owns a `MetadataStore` clone** (no new store field, no new service). _Why:_ commands must not reach into stores; the status read is in-memory.
- **D5: `normalize_host_path` stays a direct `crosshook_core::platform` call** (made explicit in the command map as owned by M1-36). _Why:_ CLAUDE.md allows non-listed commands to call core; the resolver is pure platform code with its own core tests, and wrapping it in a service adds a pass-through.
- **D6: New `crosshook_app::ipc` module holds the Classic wire contract**: settings IPC DTO, session-changed/running-sessions DTOs, startup-event mapping (`AppEvent` → named payloads), launch relay mapping and payloads, `RunCommandError` and run/update/prefix-dep payloads, `batch_lines`. It is serde-only (no Tauri, no toolkit) and moves to M7 removal together with `src-tauri`. _Why:_ the issue moves src-tauri's tests into `crosshook-app`, which only works if the tested logic moves too; `crosshook_app::services::prefix_deps::{DepLogPayload, completion_payload}` is the existing precedent; pure mapping is then testable without a Tauri runtime. `check-app-deps.sh` bans only toolkit crates.
- **D7: Bench env/plan/READY logic moves to `crosshook_app::bench`** (env decision, rate parsing, root/log validation, replay plan, monotonic READY marker, paced line reader). `src-tauri/src/commands/bench.rs` keeps only `bench_ready` and the emit loop. _Why:_ the native UI benchmark harness (M1-28) needs the same READY contract; the logic is toolkit-independent.
- **D8: `src-tauri/src/error_map.rs`** owns the two wire formats: `pub(crate) fn message(AppError) -> String` (plain `Result<T, String>`) and the `RunCommandError` envelope conversion re-exported from `crosshook_app::ipc`. All 18 local copies are deleted. _Why:_ DRY; one place defines the string wire format.
- **D9: `src-tauri/src/event_bridge.rs` (plus `event_bridge/` submodules if needed for the ~500-line cap)** replaces `startup_bridge.rs`, `session_bridge.rs`, `commands/launch/relay/mod.rs` emission and the per-command relay spawns (run-exe, update, prefix-dep, protonup progress). It only emits; payload construction comes from `crosshook_app::ipc`. Event names and payloads are unchanged. _Why:_ the issue mandates it, and it puts every Tauri event name in one file.
- **D10: `scripts/check-async-commands.sh`** fails when a `#[tauri::command]` attribute (skipping further attributes, doc comments and blank lines) is followed by a `fn` that is not `async fn`. It has `--self-test` (temp fixture with a sync command must fail, async must pass), runs from `scripts/lint.sh` and from `.github/workflows/lint.yml` next to `check-startup-path.sh`. _Why:_ the AC requires a grep check in `lint.sh`; the self-test proves it catches regressions.
- **D11: `docs/internal/native-parity/command-map.md`** has one row per command: command, `crosshook-app` service method (or core function for D5), owning issue (M1-05/06/07/31–36/42/08), and the `crosshook-app` (or core, for D5) test that covers it. Gaps found while building the map get a test in `crosshook-app` in this PR. _Why:_ the AC requires every command to be mapped and tested.
- **D12: The `command_names_match_expected_ipc_contract` tests stay in src-tauri**, updated to the new signatures. Other src-tauri tests move with the code they test (D6, D7) or collapse into `error_map` tests. _Why:_ the issue keeps the name-contract tests until M7.
- **D13: The window-close/exit lifecycle in `lib.rs` (`RunEvent::Exit`: release bus name, flush `[ui]`, `App::shutdown`) is untouched**, apart from reading the services through the one `BootstrappedApp`. _Why:_ the AC requires an unchanged lifecycle; no leaked jobs or watchdogs.

## Tasks

1. **App-side moves (crosshook-app)**: add `ipc` and `bench` modules (D6, D7), `LaunchService::is_game_running` (D3), the metadata-status delegation (D4); move the matching src-tauri tests with them. Update `crates/crosshook-app/README.md`.
2. **Adapter rewrite (src-tauri)**, after 1: `error_map.rs` (D8), `event_bridge.rs` (D9), single managed state (D1), every command async one-line delegation (D2), delete moved code, update contract tests (D12), keep the lifecycle (D13).
3. **Lint gate** (independent): `scripts/check-async-commands.sh` with self-test, wired into `lint.sh` and `lint.yml` (D10).
4. **Command map and test gaps**, after 2: `docs/internal/native-parity/command-map.md` (D11) and any missing `crosshook-app` tests.
5. **Docs**: CLAUDE.md/AGENTS.md lines that describe `src-tauri` relays or bridges (`startup_bridge`, per-session relay) point at `event_bridge`.

## Verification

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace`
- `./scripts/check-async-commands.sh` and `--self-test`, `./scripts/check-host-gateway.sh`, `./scripts/check-app-deps.sh`, `./scripts/check-startup-path.sh`, `./scripts/check-mock-coverage.sh`
- `npm test`, `npm run typecheck`, `npm run test:smoke`
- Non-test LOC of `src-tauri/src` < 2,500 (same counting rule as the baseline).
- Manual Classic dogfooding (launch, update, prefix deps, community sync) is a follow-up issue; it cannot be run headless.

## Storage boundary

No data changes. Runtime behaviour only (one command moves off the main thread). Command names, argument keys, result shapes and event names/payloads are unchanged.
