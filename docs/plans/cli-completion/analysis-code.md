# CLI Completion: Code Analysis

## Executive Summary

All business logic for the 6 placeholder commands already exists in `crosshook-core`. The work is
pure wiring in `main.rs` following the `handle_diagnostics_command` reference pattern. The primary
complexity is refactoring `steam_launch_request_from_profile()` into a generic
`launch_request_from_profile()` that branches on the resolved launch method. Two CRITICAL security
findings (C-1: helper script path, C-2: import path containment) must ship alongside the wiring.

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs`:
  CLI entry point. All 6 placeholder handlers live here. Contains `emit_placeholder()`, `launch_profile()`, `steam_launch_request_from_profile()`, `profile_store()` helper, and the reference `handle_diagnostics_command` at line 145.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/args.rs`:
  Clap derive structs for all 7 commands. All subcommands and flags already defined; only doc comments are missing on command variants.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`:
  `ProfileStore::list()`, `load()`, `save()`, `import_legacy()`, `try_new()`, `with_base_path()`. `load()` internally calls `effective_profile()` and `normalize_preset_selection()` — the CLI gets a ready-to-use profile.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`:
  `export_community_profile(profiles_dir: &Path, profile_name: &str, output_path: &Path)`. Takes `&Path`, NOT `&ProfileStore` — pass `store.base_path.as_path()`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`:
  `GameProfile` struct, all section types, and `resolve_launch_method(&profile)` at line 363. This is the canonical method inference function.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`:
  `LaunchRequest`, `SteamLaunchConfig`, `RuntimeLaunchConfig`, `LaunchOptimizationsRequest`, constants `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE`, and `ValidationError` variants.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`:
  `build_helper_command()`, `build_proton_game_command()` → `io::Result<Command>`, `build_native_game_command()` → `io::Result<Command>`. The latter two can fail (they create the log file).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs`:
  `discover_steam_root_candidates(path, &mut Vec<String>)` — pass `""` for default Linux Steam paths.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs`:
  `discover_steam_libraries(roots, &mut Vec<String>)` — NOT re-exported from `steam/mod.rs`. Import as `crosshook_core::steam::libraries::discover_steam_libraries`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`:
  `discover_compat_tools(roots, &mut Vec<String>)` — IS re-exported from `steam/mod.rs`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs`:
  `attempt_auto_populate(&SteamAutoPopulateRequest)` — synchronous. `SteamAutoPopulateResult` already derives `Serialize`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/models.rs`:
  `SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `SteamAutoPopulateFieldState`, `SteamLibrary`, `ProtonInstall`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`:
  `SettingsStore::try_new()`, `AppSettingsData` — needed for `status` command.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`:
  Tauri reference for multi-method launch dispatch at lines 52–78. Mirror this pattern in `launch_profile()`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs`:
  Tauri reference for Steam discovery and auto-populate wiring.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`:
  Tauri reference for profile list/import/export wiring.

## Architectural Patterns

### Thin CLI Shell (The Reference Pattern)

`handle_diagnostics_command` at `main.rs:145-178` is the canonical reference:

1. Initialize stores via `profile_store()` helper or `SettingsStore::try_new()`.
2. Call the `crosshook-core` function directly.
3. Branch on `global.json`:
   - JSON: `println!("{}", serde_json::to_string_pretty(&result)?)` to stdout.
   - Human: `println!()` formatted text to stdout.
4. Errors propagate as `Box<dyn Error>` via `?` and reach `eprintln!()` in `main()`.

**All 6 new handlers must follow this exact pattern.** No shared presentation layer, no new crates.

### Dual Output (JSON vs Human)

`global.json` flag at `GlobalOptions.json` gates all output:

- JSON mode: `serde_json::to_string_pretty(&result)?` → `println!()`.
- Human mode: plain-text `println!()` lines.
- Errors always go to stderr via `eprintln!()` regardless of mode.

The spec recommends a shared `output()` helper:

```rust
fn output<T: serde::Serialize>(
    global: &GlobalOptions,
    value: &T,
    human: impl FnOnce(&T),
) -> Result<(), Box<dyn Error>> {
    if global.json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        human(value);
    }
    Ok(())
}
```

### Diagnostics Out-Param

Steam discovery functions take `&mut Vec<String>` for diagnostics. Allocate before calling,
gate emission on `global.verbose` via `eprintln!()`:

```rust
let mut diagnostics: Vec<String> = Vec::new();
let roots = discover_steam_root_candidates("", &mut diagnostics);
if global.verbose {
    for msg in &diagnostics { eprintln!("{msg}"); }
}
```

### Store Initialization

The existing `profile_store()` helper at `main.rs:189-197` encapsulates `ProfileStore::with_base_path`
(when `--config` is provided) or `ProfileStore::try_new()` (otherwise). All profile/status commands
reuse this directly.

`SettingsStore::try_new()` follows the same pattern, shown in `handle_diagnostics_command`:

```rust
let settings_store = SettingsStore::try_new()
    .map_err(|error| format!("settings store: {error}"))?;
```

### Profile-to-LaunchRequest Mapping

The existing `steam_launch_request_from_profile()` at `main.rs:199-234` must be replaced by a
generic `launch_request_from_profile()` that uses `resolve_launch_method(&profile)` and branches
on method:

```rust
fn launch_request_from_profile(profile: &GameProfile) -> Result<LaunchRequest, Box<dyn Error>> {
    let method = resolve_launch_method(profile);
    let steam_client_install_path =
        resolve_steam_client_install_path(&profile.steam.compatdata_path);
    Ok(LaunchRequest {
        method: method.to_string(),
        game_path: profile.game.executable_path.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_host_path: profile.trainer.path.clone(),
        trainer_loading_mode: profile.trainer.loading_mode,
        steam: match method {
            METHOD_STEAM_APPLAUNCH => SteamLaunchConfig {
                app_id: profile.steam.app_id.clone(),
                compatdata_path: profile.steam.compatdata_path.clone(),
                proton_path: profile.steam.proton_path.clone(),
                steam_client_install_path: steam_client_install_path
                    .to_string_lossy().into_owned(),
            },
            _ => SteamLaunchConfig {
                steam_client_install_path: steam_client_install_path
                    .to_string_lossy().into_owned(),
                ..Default::default()
            },
        },
        runtime: match method {
            METHOD_PROTON_RUN => RuntimeLaunchConfig {
                prefix_path: profile.runtime.prefix_path.clone(),
                proton_path: profile.runtime.proton_path.clone(),
                working_directory: profile.runtime.working_directory.clone(),
            },
            METHOD_NATIVE => RuntimeLaunchConfig {
                working_directory: profile.runtime.working_directory.clone(),
                ..Default::default()
            },
            _ => RuntimeLaunchConfig::default(),
        },
        optimizations: LaunchOptimizationsRequest {
            enabled_option_ids: profile.launch.optimizations.enabled_option_ids.clone(),
        },
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
    })
}
```

### Launch Method Dispatch

After building the `LaunchRequest`, dispatch mirrors Tauri `launch_game` at `src-tauri/commands/launch.rs:65-75`:

```rust
let method = request.resolved_method();
let mut child = match method {
    METHOD_STEAM_APPLAUNCH => {
        let helper = scripts_dir.join(HELPER_SCRIPT_NAME);
        spawn_helper(&request, &helper, &log_path).await?
    }
    METHOD_PROTON_RUN => {
        let mut cmd = launch::script_runner::build_proton_game_command(&request, &log_path)?;
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn()?
    }
    METHOD_NATIVE => {
        let mut cmd = launch::script_runner::build_native_game_command(&request, &log_path)?;
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn()?
    }
    other => return Err(format!("unsupported launch method: {other}").into()),
};
```

The `stream_helper_log()` function works identically for all methods since all methods write to the
same log file path.

### Async Handler Convention

The spec mandates all handlers be `async fn` for consistency — `handle_diagnostics_command` is sync
(an anomaly, do not replicate). `handle_profile_command` and `handle_steam_command` are already
async at `main.rs:98` and `main.rs:128`. The new `handle_status_command` should also be async.

## Integration Points

### Files to Modify Only

**`main.rs`** is the sole file requiring logic changes. Precise changes:

1. Replace `emit_placeholder(global, "status")` in `run()` with `handle_status_command(&cli.global).await?`
2. Replace `ProfileCommand::List` stub with real core calls.
3. Replace `ProfileCommand::Import` stub with `ProfileStore::import_legacy()`.
4. Replace `ProfileCommand::Export` stub with `export_community_profile()`.
5. Replace `SteamCommand::Discover` stub with three-step discovery.
6. Replace `SteamCommand::AutoPopulate` stub with `attempt_auto_populate()`.
7. Refactor `launch_profile()` — replace `steam_launch_request_from_profile()` with generic builder and multi-method dispatch.
8. Delete `steam_launch_request_from_profile()` and `emit_placeholder()`.

**`args.rs`** needs only doc comments (`///`) added to command variants for `--help` quality. No
structural changes — all args are already fully defined.

### New Imports Required in `main.rs`

```rust
// New profile imports
use crosshook_core::profile::exchange::export_community_profile;
use crosshook_core::profile::models::resolve_launch_method;

// New steam imports
use crosshook_core::steam::{attempt_auto_populate, discover_compat_tools,
    discover_steam_root_candidates, SteamAutoPopulateRequest};
use crosshook_core::steam::libraries::discover_steam_libraries;  // CRITICAL: not in steam prelude

// New launch imports
use crosshook_core::launch::{self, LaunchRequest, RuntimeLaunchConfig, SteamLaunchConfig,
    ValidationSeverity, METHOD_STEAM_APPLAUNCH, METHOD_PROTON_RUN, METHOD_NATIVE};
// add to existing launch import: METHOD_PROTON_RUN, METHOD_NATIVE

// Already present, confirm:
use crosshook_core::settings::SettingsStore;
```

## Code Conventions

### Naming

- All CLI handler functions: `handle_<command>_command()` (async fn, returns `Result<(), Box<dyn Error>>`)
- `launch_profile()` keeps its name but its signature and body are replaced
- Profile builder function: `launch_request_from_profile()` (replaces `steam_launch_request_from_profile()`)

### Error Format

Per spec, user-facing errors follow the format:

```
error: <what failed>
  hint: <actionable suggestion>
```

Implemented via `Box<dyn Error>` propagation to `main()` → `eprintln!("{error}")`.

### Missing Required Profile Field

Profile name resolution follows the existing chain from `launch_profile()` at `main.rs:53-56`:

```rust
let profile_name = command.profile
    .or_else(|| global.profile.clone())
    .ok_or("a profile name is required via --profile")?;
```

Replicate for `profile export` command.

### Exit Codes

- `process::exit(1)` only in `profile_store()` helper (initialization failure) — preserve this
- All command failures propagate via `Box<dyn Error>` → `main()` returns `Err` → `process::exit(1)` in `main()`
- Phase 5 adds proper exit code 2 for usage errors

## Dependencies and Services

### Already in `Cargo.toml`

All dependencies are already present in `crosshook-cli/Cargo.toml`:

- `clap` (4.x derive) — argument parsing
- `serde_json` — JSON output
- `tokio` (rt-multi-thread) — async runtime
- `crosshook-core` (path dep) — all business logic

**No new dependencies are needed.**

### Re-export Hazard

`crosshook_core::steam` module re-exports:

- `attempt_auto_populate` ✓
- `discover_steam_root_candidates` ✓
- `discover_compat_tools` ✓
- `ProtonInstall`, `SteamAutoPopulateRequest`, `SteamAutoPopulateResult` ✓
- **`discover_steam_libraries` — NOT re-exported**

Always import `discover_steam_libraries` as:

```rust
use crosshook_core::steam::libraries::discover_steam_libraries;
```

### `build_proton_game_command` and `build_native_game_command` Require Log Dir

Both functions call `attach_log_stdio()` which opens/creates the log file. The log directory
`/tmp/crosshook-logs/` must exist before calling these. The existing `spawn_helper()` relies on
`build_helper_command()` which does NOT create the log dir — this is a pre-existing gap. Add:

```rust
tokio::fs::create_dir_all("/tmp/crosshook-logs").await?;
// or use the path from launch_log_path():
if let Some(parent) = log_path.parent() {
    tokio::fs::create_dir_all(parent).await?;
}
```

This must be added **before** calling `build_proton_game_command()` or `build_native_game_command()`.

## Gotchas and Warnings

### C-1 (CRITICAL): Helper Script Path Not Validated at Runtime

`DEFAULT_SCRIPTS_DIR` at `main.rs:24` is a compile-time relative path
(`"../../runtime-helpers"`). The resolved path is never checked to be owned by the current user
before execution. Mitigation: before calling `spawn_helper()`, verify the script is a regular file
(not symlink) and owned by the current UID.

```rust
// Required before spawn_helper():
let meta = std::fs::metadata(&helper_script)
    .map_err(|e| format!("helper script not found: {e}"))?;
if !meta.is_file() {
    return Err("helper script path is not a regular file".into());
}
// Check owner == current UID via std::os::unix::fs::MetadataExt::uid()
```

### C-2 (CRITICAL): Import Path Not Contained

`profile import --legacy-path` accepts arbitrary filesystem paths. A malicious path could read
sensitive files or use symlinks to overwrite existing profiles. Mitigation:

```rust
// Before calling import_legacy():
let meta = std::fs::metadata(&command.legacy_path)
    .map_err(|e| format!("cannot access import path: {e}"))?;
if !meta.is_file() {
    return Err("import path must be a regular file, not a symlink or directory".into());
}
```

### `export_community_profile` Signature

This function takes `profiles_dir: &Path`, NOT a `ProfileStore`. Passing the store itself will
cause a type error. The correct call:

```rust
export_community_profile(store.base_path.as_path(), &profile_name, &output_path)?
```

### `discover_steam_libraries` Import Path

This is the most common compile error waiting to happen. The `steam/mod.rs` does not re-export it.

```rust
// WRONG — will not compile:
use crosshook_core::steam::discover_steam_libraries;
// CORRECT:
use crosshook_core::steam::libraries::discover_steam_libraries;
```

### `ProfileStore::load()` Returns Effective Profile

`load()` already calls `effective_profile()` internally (merging `local_override`) and clears the
override section. The CLI does not need to call `effective_profile()` again after loading.

### `handle_diagnostics_command` is Sync — Do Not Replicate

This function is sync (`fn`, not `async fn`). All new handlers must be `async fn`. The spec
explicitly flags this as an anomaly not to follow.

### `resolve_steam_client_install_path` for `proton_run` Profiles

The existing `resolve_steam_client_install_path()` at `main.rs:236-269` walks ancestors of
`compatdata_path` looking for `steam.sh`. For `proton_run` profiles, `compatdata_path` may be
empty — the ancestor walk produces no result. The fallback to `default_steam_roots()` handles this
correctly but silently. This is acceptable for v1.

### Log Directory Creation for `proton_run`/`native`

The `launch_log_path()` helper at `main.rs:275-288` builds the path but does NOT create the
directory. For `steam_applaunch`, `build_helper_command()` handles this internally. For
`proton_run` and `native`, `build_proton_game_command()` / `build_native_game_command()` call
`attach_log_stdio()` which will fail if `/tmp/crosshook-logs/` does not exist. Must add
`create_dir_all` before dispatch.

### `ProfileCommand::Export` Profile Name Resolution

`ProfileExportCommand.profile` is `Option<String>`. Current stub falls back to `"<unset>"` as a
no-op. The real handler must preserve the existing resolution chain:

```rust
let profile_name = command.profile
    .or_else(|| global.profile.clone())
    .ok_or("a profile name is required; use --profile or -p")?;
```

### Legacy Import File Stem as Profile Name

`ProfileStore::import_legacy()` derives the profile name from the file stem (e.g.,
`elden-ring.profile` → `elden-ring`). The name is returned inside the `GameProfile` from core.
The CLI must extract the stem separately from `command.legacy_path.file_stem()` for display
purposes, since `import_legacy()` returns only `GameProfile` (not the derived name).

Actually, per toml_store.rs, `import_legacy()` returns `Result<GameProfile, ProfileStoreError>`.
The name is derived from `legacy_path.file_stem()` inside `import_legacy()` and saved under that
stem. The CLI should display the derived name by computing it from the path:

```rust
let profile_name = command.legacy_path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("unknown");
```

## Task-Specific Guidance

### Phase 1: Foundation (profile list + status)

**`profile list`**:

```rust
ProfileCommand::List => {
    let store = profile_store(global.config.clone());
    let profiles = store.list()
        .map_err(|e| format!("failed to list profiles: {e}"))?;
    if global.json {
        #[derive(serde::Serialize)]
        struct ListOutput<'a> {
            profiles: &'a [String],
            count: usize,
            profiles_dir: String,
        }
        println!("{}", serde_json::to_string_pretty(&ListOutput {
            profiles: &profiles,
            count: profiles.len(),
            profiles_dir: store.base_path.to_string_lossy().into_owned(),
        })?);
    } else {
        for name in &profiles { println!("{name}"); }
        if !profiles.is_empty() { println!(); }
        println!("{} profile(s) in {}", profiles.len(), store.base_path.display());
    }
}
```

**`status`**: Aggregate `ProfileStore::list()` + `SettingsStore::try_new()` +
`discover_steam_root_candidates("", &mut diag)` + `discover_compat_tools(&roots, &mut diag)`.
Failures in individual sections should populate a `diagnostics` vec rather than aborting.

### Phase 2: Import/Export

**`profile import`**: Call `store.import_legacy(&command.legacy_path)`. Apply C-2 mitigation first
(verify path is a regular file, not symlink).

**`profile export`**: Resolve `output_path` from `--output` or default to
`std::env::current_dir()?.join(format!("{profile_name}.crosshook.json"))`. Call
`export_community_profile(store.base_path.as_path(), &profile_name, &output_path)`.

### Phase 3: Steam Discovery

**`steam discover`**: Call the three discovery functions in sequence sharing a single `diagnostics`
vec. Surface diagnostics via `--verbose` (stderr) or include in JSON output.

**`steam auto-populate`**: Single call to `attempt_auto_populate(&SteamAutoPopulateRequest { game_path: command.game_path, steam_client_install_path: PathBuf::new() })`. Return `SteamAutoPopulateResult` directly for JSON (already `Serialize`).

### Phase 4: Launch Completion

1. Add `use crosshook_core::launch::{METHOD_PROTON_RUN, METHOD_NATIVE}` to imports.
2. Add `use crosshook_core::profile::models::resolve_launch_method` to imports.
3. Replace `steam_launch_request_from_profile()` with `launch_request_from_profile()` using the template from the spec.
4. Add `create_dir_all` for log directory before `proton_run`/`native` dispatch.
5. Extend `launch_profile()` dispatch block to match on `METHOD_PROTON_RUN` and `METHOD_NATIVE`.
6. Apply C-1 mitigation (verify helper script before spawning steam_applaunch).

### Phase 5: Polish

- Delete `emit_placeholder()` — after all handlers are wired, this becomes dead code.
- Add `///` doc comments to all `Command`, `ProfileCommand`, `SteamCommand` variants in `args.rs`.
- Run `cargo test -p crosshook-cli` to verify all parsing tests pass.
- Update `docs/getting-started/quickstart.md` with CLI usage section.

## Verified Corrections (Post-Analysis)

### `resolve_steam_client_install_path` — Core Version Exists But Has Different Signature

The feature-spec suggests deleting the CLI's `resolve_steam_client_install_path()` and using the
core version. This is **partially correct but requires care**:

- Core version: `crosshook_core::launch::runtime_helpers::resolve_steam_client_install_path(configured_path: &str) -> Option<String>`
- CLI version: walks ancestors of `compatdata_path` looking for `steam.sh` + falls back to default roots

The core version takes a pre-configured path string (reads from `STEAM_COMPAT_CLIENT_INSTALL_PATH`
env var or returns None). The CLI version does additional ancestor-walking. They are not
drop-in replacements. `runtime_helpers` is `pub mod` in `launch/mod.rs` but the function is NOT
re-exported from `crosshook_core::launch` — import as:

```rust
use crosshook_core::launch::runtime_helpers::resolve_steam_client_install_path;
```

For v1, keeping the CLI's local implementation is simpler and avoids the signature mismatch.

### `resolved_method()` on `LaunchRequest` — Confirmed Exists

`LaunchRequest::resolved_method(&self) -> &str` is confirmed at `request.rs:76`. This is the
method used in the Tauri `launch_game` command and should be used in the CLI's dispatch block
after building the `LaunchRequest` (not before).

### `SteamLaunchConfig` and `RuntimeLaunchConfig` Both Derive `Default` — Confirmed

Both structs at `request.rs:43-63` derive `Default`. The `..Default::default()` spread syntax in
the `launch_request_from_profile()` template is valid.

### `steam/mod.rs` Re-exports — Confirmed Exact Set

From `steam/mod.rs`, the following are re-exported (and `discover_steam_libraries` is confirmed absent):

- `attempt_auto_populate` ✓
- `discover_steam_root_candidates` ✓
- `discover_compat_tools` ✓
- `DiagnosticCollector` ✓
- `ProtonInstall`, `SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `SteamAutoPopulateFieldState` (via models) ✓
- `discover_steam_libraries` — **NOT present, import directly from `steam::libraries`**
