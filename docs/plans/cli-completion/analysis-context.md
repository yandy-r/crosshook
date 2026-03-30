# Context Analysis: cli-completion

## Executive Summary

Wire 6 placeholder CLI commands in `crosshook-cli/src/main.rs` to existing `crosshook-core` functions, and extend the `launch` command from `steam_applaunch`-only to all three methods (`steam_applaunch`, `proton_run`, `native`). Work is pure orchestration — no new business logic, no new dependencies. Two CRITICAL security findings (C-1: helper script path validation, C-2: import path containment) must ship with mitigations in the same PR.

## Architecture Context

- **System Structure**: Two-crate Rust workspace. `crosshook-core` owns all business logic. `crosshook-cli` is a thin wrapper: parse args via `clap`, initialize stores, call core, branch on `--json` flag for output. Tauri commands in `src-tauri/src/commands/` already implement equivalent operations and are the reference implementation for each command.
- **Data Flow**: `run()` dispatches to `handle_profile_command`, `handle_steam_command`, `launch_profile`, `handle_diagnostics_command` (reference). Each handler: store init → core call → output branch. All core types involved derive `Serialize`; no custom serialization needed.
- **Integration Points**: Every change is in `main.rs` only. The `Command::Status` arm at `run()` line 43 calls `emit_placeholder`; `handle_profile_command` at lines 98–126 has 3 stubs; `handle_steam_command` at lines 128–143 has 2 stubs; `launch_profile` has a hard-reject guard for non-`steam_applaunch` methods at line 206. All stubs are replaced inline — no new files.

## Critical Files Reference

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs`: All dispatch logic, 6 `emit_placeholder()` stubs, `steam_launch_request_from_profile()`, `launch_profile()`, reference `handle_diagnostics_command` pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/args.rs`: All clap arg structs; needs `///` doc comments on variants; no structural changes required
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore::list()`, `load()`, `import_legacy()`, `with_base_path()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`: `export_community_profile(profiles_dir: &Path, name: &str, output_path: &Path)` — takes `&Path`, NOT `&ProfileStore`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `resolve_launch_method()` at line 363
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: `LaunchRequest`, `SteamLaunchConfig`, `RuntimeLaunchConfig`, `validate()`, method constants — verify `SteamLaunchConfig` and `RuntimeLaunchConfig` derive `Default` before using `..Default::default()` spread in builder
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: `build_helper_command()` → `Command`; `build_proton_game_command()` → `io::Result<Command>`; `build_native_game_command()` → `io::Result<Command>`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`: `build_launch_preview()` — already exists for `--dry-run`; wire in Phase 4, not Phase 5
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs`: `discover_steam_libraries()` — **NOT re-exported** from `steam/mod.rs`; must import as `crosshook_core::steam::libraries::discover_steam_libraries`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs`: `attempt_auto_populate()` — sync; call directly without `spawn_blocking` (no event loop in CLI)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `SettingsStore` for `status` command
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`: Reference dispatch for `proton_run` + `native` (lines 52–78)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs`: Reference for steam discover + auto-populate wiring
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`: Reference for profile list/import/export wiring

## Patterns to Follow

- **Thin CLI Shell (canonical reference)**: `handle_diagnostics_command` at `main.rs:145-177`. Steps: init store(s) → call core → `if global.json { serde_json::to_string_pretty } else { human output }`. Errors via `?` into `Box<dyn Error>` → stderr.
- **Dual Output**: `global.json` gates all output. JSON to stdout via `println!("{}", serde_json::to_string_pretty(&val)?)`. Human text to stdout. Errors via `eprintln!` to stderr. No shared output helper needed — each command is 15-40 lines.
- **Store Init**: Reuse existing `profile_store(Option<PathBuf>)` helper. `SettingsStore::try_new().map_err(|e| format!("settings store: {e}"))?` for settings.
- **Diagnostics Out-Param**: All three discovery functions take `&mut Vec<String>`. Allocate before call; emit only under `global.verbose` via `eprintln!`.
- **Async Handlers**: All new handlers must be `async fn` — `handle_diagnostics_command` is sync, which is the anomaly to avoid.
- **Profile-to-LaunchRequest mapping**: Replace `steam_launch_request_from_profile()` with a single `launch_request_from_profile()` using an internal match on `resolve_launch_method(&profile)`. Three distinct `LaunchRequest` shapes — `steam.*` fields for `steam_applaunch`, `runtime.*` fields for `proton_run`, `game_path` only for `native`.
- **Log directory**: Must call `fs::create_dir_all` on the log dir before `build_proton_game_command` or `build_native_game_command` — `attach_log_stdio` opens the file but does not create the parent. Use `XDG_RUNTIME_DIR` with mode 0700 (W-2 mitigation), not `/tmp`.
- **`status` partial-failure pattern**: Individual section failures (Steam not found, settings unavailable) must NOT abort the command. Collect errors into a `diagnostics: Vec<String>` and always emit partial output. Always exits 0.
- **Error message format**: `error: <what failed>\n  hint: <actionable suggestion>` to stderr. Exit 0 on success, exit 1 on general error, exit 3 on profile not found, exit 4 on launch failure, exit 5 on Steam not found.

## Cross-Cutting Concerns

- **Security (CRITICAL — must ship in same PR as wiring)**:
  - C-1 (`launch`, Phase 4): Helper script path is compile-time `CARGO_MANIFEST_DIR`-relative, not runtime-validated. At minimum, assert `helper_script.is_file()` and owned by current UID before invoking. Preferred: embed via `include_bytes!` or use `std::env::current_exe()` for AppImage-relative path.
  - C-2 (`profile import`, Phase 2): `--legacy-path` accepts arbitrary filesystem paths. Validate via `path.symlink_metadata()` that it is a regular file (not symlink/device). Warn when path is outside `~/.config/crosshook/`; require `--force` to proceed.
- **Security (WARNING — inline with wiring phase)**:
  - W-2 (`launch`, Phase 4): Log path in `/tmp` is TOCTOU-susceptible. Use `XDG_RUNTIME_DIR` with mode 0700 for log directory. Bundle with Phase 4-B (proton_run + native wiring).
  - W-4 (`profile export`, Phase 2): `--output` path needs validation: not a symlink, parent is writable, not in protected dirs. Bundle with Phase 2-B.
- **`discover_steam_libraries` not re-exported**: Compile error if imported via `crosshook_core::steam::discover_steam_libraries`. Must use `crosshook_core::steam::libraries::discover_steam_libraries`.
- **`export_community_profile` takes `&Path` not `&ProfileStore`**: Pass `store.base_path.as_path()`, not `&store`.
- **`build_proton_game_command` returns `io::Result<Command>`**: Unlike `build_helper_command` which returns `Command` directly. Apply `?` before spawn.
- **`SteamLaunchConfig` / `RuntimeLaunchConfig` `Default` derive**: The builder template in `feature-spec.md` uses `..Default::default()` spread — verify both types derive `Default` in `launch/request.rs` before using this pattern.
- **`steam_client_install_path` not stored in profiles**: Must be resolved at launch time via existing `resolve_steam_client_install_path()` in `main.rs` for `steam_applaunch`. For `proton_run`, may be absent — fall back to `discover_steam_root_candidates("")`.
- **No new dependencies**: All needed crates (`clap`, `serde_json`, `tokio`, `crosshook-core`) already in `crosshook-cli/Cargo.toml`. `clap_complete` for shell completions is optional Phase 5 addition.

## Parallelization Opportunities

- **Phases 1, 2, and 3 are fully independent of each other** — all add independent handler functions to `main.rs` with no shared state. They can run concurrently across phases, not just within each phase.
- **Phase 1 tasks (all independent)**: `profile list`, `status`, and doc comments on args can be implemented concurrently.
- **Phase 2 tasks (both independent)**: `profile import` (+ C-2 mitigation) and `profile export` (+ W-4 mitigation) can be implemented concurrently.
- **Phase 3 tasks (both independent)**: `steam discover` and `steam auto-populate` can be implemented concurrently.
- **Phase 4 (one sequential blocker)**: `launch_request_from_profile()` refactor (4-A) is the single blocking task. After it lands, `proton_run` wiring (4-B, includes W-2 log dir fix) and `native` wiring (4-C, includes C-1 mitigation) can proceed concurrently. `--dry-run` via `build_launch_preview()` bundles into Phase 4, not Phase 5.
- **Phase 5**: Exit code standardization and `emit_placeholder()` deletion (5-D) are hard-blocked on all Phase 1–4 tasks completing. Quickstart docs (5-B) and shell completions (5-C) can start in parallel with Phase 4 once Phases 1–3 are done.

## Implementation Constraints

- **Only `main.rs` and `args.rs` are modified** — no new files, no new crates for core wiring.
- **`emit_placeholder()` must be deleted** in Phase 5 — blocked until ALL Phase 1–4 tasks complete.
- **MetadataStore is out of scope for v1**: `status` uses profile count + Steam detection + settings only — no SQLite.
- **Game-only launch for v1**: `launch_game_only = true`, `launch_trainer_only = false` hardcoded. `--trainer-only` deferred.
- **Optimizations read from profile**: No `--optimization` CLI override flags for v1.
- **JSON schemas are unstable for v1**: Document as such; do not lock schemas.
- **Profile export format is community JSON** (`exchange.rs`), not raw TOML copy.
- **`attempt_auto_populate` does NOT modify profiles**: Discovery only; no auto-save.
- **`handle_diagnostics_command` is sync** — this is the existing anomaly; new handlers must be `async fn`.
- **`ProfileStore::load()` returns effective profile**: `local_override` is already merged; no additional caller-side merging.
- **Log streaming pattern identical for all three launch methods**: `stream_helper_log` / `drain_log` reused; log dir must be created by caller.

## Key Recommendations

- **Start with `profile list`** as the first implementation — it validates the output pattern (single store init, 2-branch output) with minimal surface area.
- **Phases 1–3 can be parallelized across implementers**: Each adds an independent handler. Phases 1, 2, and 3 have no cross-phase dependencies — 6 implementers could each take one command.
- **Phase 4-A is the only true blocker**: The `launch_request_from_profile()` refactor is the single sequential gate in the entire plan. Everything else is parallelizable before or after it.
- **Use Tauri commands as the reference, not documentation**: `src-tauri/src/commands/launch.rs:52-78`, `src-tauri/src/commands/steam.rs`, and `src-tauri/src/commands/profile.rs` contain working code for every operation. The CLI versions are simpler (no `AppHandle`, no `spawn_blocking`).
- **`--dry-run` belongs in Phase 4**: `build_launch_preview()` already exists in core. Wire it as part of Phase 4-B alongside `proton_run` — do not defer to Phase 5.
- **Security mitigations are not optional**: C-1, C-2, W-2, and W-4 must ship inline with the tasks that introduce their risk surface. They cannot be deferred to Phase 5.
- **Verify `Default` derives before writing builder**: Check `SteamLaunchConfig` and `RuntimeLaunchConfig` in `launch/request.rs` derive `Default` before using `..Default::default()` spread from the `feature-spec.md` template.
- **Exit codes in a single Phase 5 pass**: Standardize exit codes after all commands are wired to prevent churn during development.
