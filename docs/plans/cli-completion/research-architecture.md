# Architecture Research: cli-completion

## System Overview

CrossHook uses a two-layer Rust architecture: `crosshook-core` is the shared business logic library containing all profile management, launch orchestration, Steam discovery, and diagnostics; `crosshook-cli` is a thin binary that parses CLI arguments via `clap` and delegates to `crosshook-core` functions. Currently only `diagnostics export` and `launch` (steam_applaunch only) are wired; the other 6 command handlers call `emit_placeholder()` instead of invoking core functions. The Tauri commands in `src-tauri/src/commands/` already implement the same operations and serve as the reference pattern for wiring the CLI.

## Relevant Components

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs`: CLI entry point; all command dispatch logic; contains all 6 placeholder stubs
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/args.rs`: `clap`-based argument structs for all 7 commands
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` — `load()` returns a flat resolved `GameProfile` (local_override merged internally, no caller merging needed); also `list()`, `save()`, `delete()`, `rename()`, `duplicate()`, `import_legacy()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`: `export_community_profile(profiles_dir, name, output_path)` — takes a directory path, constructs its own `ProfileStore` internally; `import_community_profile()` for JSON community format
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/legacy.rs`: `load()`, `list()`, `save()`, `delete()` — legacy `.profile` key=value format
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `resolve_launch_method()` at line 363 — infers launch method from profile fields
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: `LaunchRequest`, `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE`, `validate()`, `resolved_method()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: `build_helper_command()` (returns `Command`), `build_proton_game_command()` (returns `std::io::Result<Command>`), `build_native_game_command()` (returns `std::io::Result<Command>`) — all use `tokio::process::Command`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs`: `attempt_auto_populate()` — sync but filesystem-scanning; safe to call directly on the CLI async runtime thread (no `spawn_blocking` needed unlike Tauri)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs`: `discover_steam_root_candidates()` — locates Steam root dirs; takes `&mut Vec<String>` diagnostics out-param
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs`: `discover_steam_libraries()` — NOT re-exported from `steam/mod.rs`; must import as `crosshook_core::steam::libraries::discover_steam_libraries`; takes `&mut Vec<String>` diagnostics out-param
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: `discover_compat_tools()` — enumerates Proton/compat-tool installs; takes `&mut Vec<String>` diagnostics out-param
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/models.rs`: `SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `ProtonInstall`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`: Tauri reference for proton_run + native launch wiring pattern (lines 52–78)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs`: Tauri reference for steam discover / auto-populate wiring
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`: Tauri reference for profile list/import/export wiring

## Data Flow

**Profile commands:**

```
CLI args → ProfileArgs/ProfileCommand
  profile list    → ProfileStore::list() → Vec<String>
  profile import  → ProfileStore::import_legacy(legacy_path) → GameProfile (legacy .profile format)
  profile export  → export_community_profile(profiles_dir, profile_name, output) → CommunityExportResult
                    (note: takes &profiles_dir path, not a ProfileStore instance)
```

**Steam commands:**

```
CLI args → SteamArgs/SteamCommand
  steam discover      → discover_steam_root_candidates("", &mut diagnostics)
                        + discover_compat_tools(roots, &mut diagnostics)
                        → emit diagnostics only under --verbose
  steam auto-populate → attempt_auto_populate(&SteamAutoPopulateRequest{game_path, steam_client_install_path})
                        → SteamAutoPopulateResult{app_id, compatdata_path, proton_path, diagnostics}
```

**Launch command (extension to proton_run and native):**

```
--profile NAME → ProfileStore::load(name) → GameProfile (local_override already merged)
               → build_launch_request_from_profile()
               → validate(&request)
               → match resolved_method():
                   steam_applaunch → build_helper_command() → Command → spawn + stream log
                   proton_run      → build_proton_game_command() → io::Result<Command> → spawn + stream log
                   native          → build_native_game_command() → io::Result<Command> → spawn + stream log

Three distinct LaunchRequest shapes:
  steam_applaunch: steam.* fields (app_id, compatdata_path, proton_path, steam_client_install_path)
  proton_run:      runtime.* fields (proton_path, prefix_path, working_directory)
  native:          game_path only
```

`steam_client_install_path` is not stored in profiles — resolved at launch time via `resolve_steam_client_install_path()` (already in `main.rs`), which checks `$STEAM_COMPAT_CLIENT_INSTALL_PATH` env var first, then walks `compatdata_path` ancestors.

**Status command:** currently a placeholder; no core function exists for it — likely just print basic env/version info.

## Integration Points

**`handle_profile_command` in `main.rs`** (lines 98–126): Replace `emit_placeholder` calls with:

- `ProfileCommand::List` → `profile_store(…).list()` → iterate and print names
- `ProfileCommand::Import` → `profile_store(…).import_legacy(&command.legacy_path)` → print or JSON serialize result
- `ProfileCommand::Export` → `export_community_profile(&store.base_path, &profile_name, &output_path)` → print result

**`handle_steam_command` in `main.rs`** (lines 128–143): Replace `emit_placeholder` calls with:

- `SteamCommand::Discover` → `discover_steam_root_candidates("", &mut diagnostics)` + `discover_compat_tools(roots, &mut diagnostics)` → collect diagnostics vec, emit only under `global.verbose`
- `SteamCommand::AutoPopulate` → `attempt_auto_populate(&SteamAutoPopulateRequest{game_path: command.game_path, steam_client_install_path: PathBuf::default()})` → print/serialize result; call directly (no `spawn_blocking` needed in CLI)

**`steam_launch_request_from_profile` in `main.rs`** (lines 199–234): Remove the early-return guard for non-`steam_applaunch` methods. Build distinct `LaunchRequest` shapes per method: `steam.*` fields for `steam_applaunch`, `runtime.*` fields from `profile.runtime` for `proton_run`, `game_path` only for `native`.

**`launch_profile` in `main.rs`** (lines 49–96): Add `fs::create_dir_all` for `/tmp/crosshook-logs/` before calling `build_proton_game_command` or `build_native_game_command` — the log directory is not auto-created by core.

**`Command::Status` arm in `run()`** (line 43): Implement basic status — crosshook version, config path, profile count.

## Key Dependencies

- `clap` v4 with `derive` feature — argument parsing (already in `Cargo.toml`)
- `serde_json` v1 — JSON output for `--json` flag (already in `Cargo.toml`)
- `tokio` v1 with `process`, `fs`, `io-std`, `io-util`, `rt-multi-thread` — async spawn + log streaming (already in `Cargo.toml`)
- `crosshook_core::profile::{ProfileStore, export_community_profile, import_community_profile}` — profile operations
- `crosshook_core::steam::{attempt_auto_populate, discover_steam_root_candidates, discover_compat_tools, SteamAutoPopulateRequest}` — steam operations
- `crosshook_core::steam::libraries::discover_steam_libraries` — NOT re-exported from `steam/mod.rs`; requires explicit submodule path
- `crosshook_core::launch::script_runner::{build_proton_game_command, build_native_game_command}` — already imported in Tauri but not in CLI; both return `std::io::Result<Command>` (unlike `build_helper_command` which returns `Command` directly); all three use `tokio::process::Command`
- `crosshook_core::profile::models::RuntimeSection` — needed to extract `proton_path` and `prefix_path` from `GameProfile` for `proton_run` launch requests

## Gotchas

- `handle_diagnostics_command` is a sync `fn` while all other handlers are `async fn`. New command handlers should be `async fn` to be consistent with the existing pattern.
- `build_proton_game_command` and `build_native_game_command` return `std::io::Result<Command>`, not `Command` directly — requires `?` or `.map_err(...)` unwrapping before spawn, unlike `build_helper_command`.
- `discover_steam_libraries` is not re-exported from `crosshook_core::steam` — must use the full path `crosshook_core::steam::libraries::discover_steam_libraries`.
- `profile.runtime` (not `profile.steam`) holds `proton_path` and `prefix_path` for `proton_run` launches.
- `/tmp/crosshook-logs/` is not auto-created by core — the CLI must call `fs::create_dir_all` before invoking `build_proton_game_command` or `build_native_game_command`, or those calls will fail with a "No such file or directory" I/O error.
- `steam_client_install_path` is not persisted in profiles — resolved at runtime by `resolve_steam_client_install_path()` already in `main.rs`; do not add it as a profile field.
- `ProfileStore::load()` merges `local_override` internally — callers receive a flat `GameProfile` with all machine-specific fields in canonical positions; no caller-side merging is needed.
- `export_community_profile` takes a `&Path` directory (not a `&ProfileStore`) and constructs its own store — do not pass `&store` directly.
- `discover_steam_root_candidates`, `discover_steam_libraries`, and `discover_compat_tools` all use a `&mut Vec<String>` diagnostics out-param rather than returning diagnostics — callers must decide whether to surface these (emit only under `--verbose` to avoid noise).
- `attempt_auto_populate` is sync and filesystem-scanning; Tauri wraps it in `spawn_blocking` to avoid blocking the GUI event loop, but the CLI has no event loop sensitivity and can call it directly on the async runtime thread.
