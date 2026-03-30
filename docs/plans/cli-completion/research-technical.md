# CLI Completion: Technical Specification

## Executive Summary

CrossHook CLI needs 6 placeholder commands wired to `crosshook-core` functions, plus the existing `launch` command extended to support `proton_run` and `native` methods. All commands must support `--json` structured output via the existing `GlobalOptions::json` flag. This is pure wiring -- all business logic exists in `crosshook-core`.

## Data Models

JSON output schemas and serialization patterns for each command are documented in the Command Specifications section below. All types that cross the output boundary derive `Serialize` via serde.

## Relevant Files

### CLI (to modify)

- `crates/crosshook-cli/src/main.rs`: Current CLI entry point; contains `emit_placeholder()` stubs and `launch_profile()` with steam_applaunch only
- `crates/crosshook-cli/src/args.rs`: clap argument definitions; all subcommands already parsed
- `crates/crosshook-cli/Cargo.toml`: Dependencies -- currently `clap`, `crosshook-core`, `serde_json`, `tokio`

### Core Library (to call into)

- `crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore::list()` -> `Vec<String>`, `ProfileStore::load()` -> `GameProfile`, `ProfileStore::import_legacy()` -> `GameProfile`
- `crates/crosshook-core/src/profile/models.rs`: `GameProfile` struct (Serialize + Deserialize), `resolve_launch_method()`
- `crates/crosshook-core/src/profile/exchange.rs`: `export_community_profile(profiles_dir, name, output_path)` -> `CommunityExportResult`
- `crates/crosshook-core/src/steam/discovery.rs`: `discover_steam_root_candidates(path, &mut diagnostics)` -> `Vec<PathBuf>`
- `crates/crosshook-core/src/steam/libraries.rs`: `discover_steam_libraries(roots, &mut diagnostics)` -> `Vec<SteamLibrary>` -- NOTE: not re-exported from `steam/mod.rs`, import as `crosshook_core::steam::libraries::discover_steam_libraries`
- `crates/crosshook-core/src/steam/auto_populate.rs`: `attempt_auto_populate(&request)` -> `SteamAutoPopulateResult`
- `crates/crosshook-core/src/steam/models.rs`: `SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `SteamLibrary`, `ProtonInstall`
- `crates/crosshook-core/src/steam/proton.rs`: `discover_compat_tools(roots, &mut diagnostics)` -> `Vec<ProtonInstall>`
- `crates/crosshook-core/src/launch/request.rs`: `LaunchRequest`, `validate()`, `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE`
- `crates/crosshook-core/src/launch/script_runner.rs`: `build_helper_command()`, `build_proton_game_command()`, `build_native_game_command()`
- `crates/crosshook-core/src/launch/mod.rs`: `analyze()`, `should_surface_report()`, `build_launch_preview()`
- `crates/crosshook-core/src/settings/mod.rs`: `SettingsStore::try_new()`, `AppSettingsData`
- `crates/crosshook-core/src/export/diagnostics.rs`: `export_diagnostic_bundle()`, `DiagnosticBundleResult`

### Tauri Commands (reference implementation)

- `src-tauri/src/commands/profile.rs`: Shows how core functions are called for profile operations
- `src-tauri/src/commands/steam.rs`: Shows `discover_steam_root_candidates`, `discover_compat_tools`, `attempt_auto_populate` usage patterns
- `src-tauri/src/commands/launch.rs`: Shows multi-method launch dispatch (`steam_applaunch`, `proton_run`, `native`), log streaming, diagnostic analysis
- `src-tauri/src/commands/diagnostics.rs`: Shows diagnostic bundle export
- `src-tauri/src/commands/shared.rs`: `create_log_path()`, `sanitize_display_path()` -- the CLI has its own simpler versions

## Architecture Design

### Dispatch Pattern

The CLI already uses a clean dispatch pattern in `run()`:

```
run() -> match cli.command {
    Command::Launch(cmd)       -> launch_profile(cmd, &cli.global),
    Command::Profile(cmd)      -> handle_profile_command(cmd, &cli.global),
    Command::Steam(cmd)        -> handle_steam_command(cmd, &cli.global),
    Command::Diagnostics(args) -> handle_diagnostics_command(args, &cli.global),
    Command::Status            -> emit_placeholder(&cli.global, "status"),
}
```

Each placeholder currently calls `emit_placeholder()`. The wiring replaces each `emit_placeholder()` call with a real handler function. No new dispatch layer is needed.

### Output Formatting Pattern

The `diagnostics export` command already demonstrates the pattern to follow. Every command handler should:

1. Perform the operation via `crosshook-core`
2. If `global.json` is true, serialize result with `serde_json::to_string_pretty()` and print to stdout
3. If `global.json` is false, format a human-readable table/summary to stdout
4. Errors go to stderr via `eprintln!()` or via the top-level `Box<dyn Error>` propagation

Introduce a shared `output()` helper to reduce boilerplate:

```rust
fn output<T: Serialize>(global: &GlobalOptions, value: &T, human: impl FnOnce(&T)) -> Result<(), Box<dyn Error>> {
    if global.json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        human(value);
    }
    Ok(())
}
```

### ProfileStore Initialization

The CLI already has a `profile_store()` helper that respects `--config`:

```rust
fn profile_store(profile_dir: Option<PathBuf>) -> ProfileStore {
    match profile_dir {
        Some(path) => ProfileStore::with_base_path(path),
        None => ProfileStore::try_new().unwrap_or_else(|error| { ... }),
    }
}
```

All profile/status commands reuse this. No changes needed.

### SettingsStore Initialization

Existing pattern from `handle_diagnostics_command`:

```rust
let settings_store = SettingsStore::try_new().map_err(|error| format!("settings store: {error}"))?;
```

Reuse for `status` command.

## Command Specifications

### 1. `crosshook status`

**Core calls:**

- `ProfileStore::list()` -> profile count and names
- `ProfileStore::load()` -> per-profile summary (launch method, game name)
- `SettingsStore::try_new()` + `SettingsStore::load()` -> app settings
- `discover_steam_root_candidates("", &mut diagnostics)` -> Steam roots
- `discover_compat_tools(&roots, &mut diagnostics)` -> Proton installs

**JSON output schema:**

```json
{
  "version": "0.2.4",
  "profiles": {
    "count": 3,
    "names": ["elden-ring", "cyberpunk-2077", "stardew-valley"]
  },
  "settings": {
    "auto_load_last_profile": true,
    "last_used_profile": "elden-ring",
    "onboarding_completed": true,
    "community_tap_count": 1
  },
  "steam": {
    "roots": ["/home/user/.local/share/Steam"],
    "library_count": 2,
    "proton_installs": [
      {
        "name": "GE-Proton-9-4",
        "path": "/home/user/.local/share/Steam/compatibilitytools.d/GE-Proton-9-4",
        "is_official": false
      }
    ]
  },
  "diagnostics": ["Default local Steam install: /home/user/.local/share/Steam"]
}
```

**Human output:**

```
CrossHook v0.2.4

Profiles: 3 (elden-ring, cyberpunk-2077, stardew-valley)
Last used: elden-ring
Onboarding: completed

Steam roots:
  /home/user/.local/share/Steam

Proton installs: 2
  GE-Proton-9-4 (custom)
  Proton 9.0 (official)
```

**Error handling:** Individual section failures (e.g., Steam not found) should not abort the command. Collect errors into a `diagnostics` array and still produce partial output.

### 2. `crosshook profile list`

**Core call:** `ProfileStore::list()` -> `Result<Vec<String>, ProfileStoreError>`

**JSON output schema:**

```json
{
  "profiles": ["cyberpunk-2077", "elden-ring", "stardew-valley"],
  "count": 3,
  "profiles_dir": "/home/user/.config/crosshook/profiles"
}
```

**Human output:**

```
elden-ring
cyberpunk-2077
stardew-valley

3 profiles in /home/user/.config/crosshook/profiles
```

**Implementation note:** `ProfileStore::list()` returns sorted names. The JSON form includes `profiles_dir` from `store.base_path` for tooling integration.

### 3. `crosshook profile import --legacy-path <PATH>`

**Core call:** `ProfileStore::import_legacy(&legacy_path)` -> `Result<GameProfile, ProfileStoreError>`

This already:

1. Reads legacy `.profile` format (key=value pairs)
2. Normalizes Z: Windows drive paths to Linux paths
3. Converts to `GameProfile` via `From<LegacyProfileData>`
4. Derives launch method (`steam_applaunch`/`proton_run`/`native`)
5. Saves as TOML to the profiles directory

**JSON output schema:**

```json
{
  "imported": true,
  "profile_name": "elden-ring",
  "legacy_path": "/path/to/elden-ring.profile",
  "profile": { "...GameProfile..." },
  "launch_method": "steam_applaunch"
}
```

**Human output:**

```
Imported legacy profile: elden-ring
  Source: /path/to/elden-ring.profile
  Launch method: steam_applaunch
  Game: /games/elden-ring/eldenring.exe
```

**Edge case:** The profile name is derived from the legacy file stem (e.g., `elden-ring.profile` -> `elden-ring`). If a TOML profile with that name already exists, `ProfileStore::save()` silently overwrites it. This matches existing behavior.

### 4. `crosshook profile export --profile <NAME> [--output <PATH>]`

**Core call:** `export_community_profile(profiles_dir, profile_name, output_path)` -> `Result<CommunityExportResult, CommunityExchangeError>`

Note: This function takes `profiles_dir: &Path`, NOT a `&ProfileStore`. Pass `store.base_path.as_path()` as the first argument.

This:

1. Loads the profile from the TOML store
2. Strips machine-specific paths (executable paths, compatdata, proton, dll paths, icon path, working directory)
3. Builds metadata (game_name, derived trainer display name)
4. Writes a community JSON manifest to the output path

**Output path default:** If `--output` is omitted, derive from profile name: `{cwd}/{profile_name}.crosshook.json`

**JSON output schema:**

```json
{
  "exported": true,
  "profile_name": "elden-ring",
  "output_path": "/home/user/elden-ring.crosshook.json",
  "manifest": { "schema_version": 1, "metadata": { "..." }, "profile": { "..." } }
}
```

**Human output:**

```
Exported community profile: elden-ring
  Output: /home/user/elden-ring.crosshook.json
```

**Arg changes:** The `ProfileExportCommand.profile` field should be required (not Optional) -- or resolve via `global.profile` as it already does. The current code falls back to `<unset>` which would error. Preserve the existing resolution chain (`command.profile || global.profile || error`).

### 5. `crosshook steam discover`

**Core calls:**

- `discover_steam_root_candidates("", &mut diagnostics)` -> `Vec<PathBuf>`
- `crosshook_core::steam::libraries::discover_steam_libraries(&roots, &mut diagnostics)` -> `Vec<SteamLibrary>` (not re-exported from `steam` module root)
- `discover_compat_tools(&roots, &mut diagnostics)` -> `Vec<ProtonInstall>`

**JSON output schema:**

```json
{
  "roots": ["/home/user/.local/share/Steam"],
  "libraries": [
    { "path": "/home/user/.local/share/Steam", "steamapps_path": "/home/user/.local/share/Steam/steamapps" },
    { "path": "/mnt/games/SteamLibrary", "steamapps_path": "/mnt/games/SteamLibrary/steamapps" }
  ],
  "proton_installs": [
    {
      "name": "GE-Proton-9-4",
      "path": "/home/user/.local/share/Steam/compatibilitytools.d/GE-Proton-9-4/proton",
      "is_official": false,
      "aliases": []
    }
  ],
  "diagnostics": ["Default local Steam install: /home/user/.local/share/Steam"]
}
```

**Human output:**

```
Steam roots:
  /home/user/.local/share/Steam

Libraries:
  /home/user/.local/share/Steam/steamapps
  /mnt/games/SteamLibrary/steamapps

Proton installs:
  GE-Proton-9-4 (custom)  /home/user/.local/share/Steam/compatibilitytools.d/GE-Proton-9-4/proton
  Proton 9.0 (official)   /home/user/.local/share/Steam/steamapps/common/Proton 9.0/proton
```

**Implementation note:** All three discovery functions accept `&mut Vec<String>` diagnostics. Surface these via `--verbose` or include in JSON output.

### 6. `crosshook steam auto-populate --game-path <PATH>`

**Core call:** `attempt_auto_populate(&SteamAutoPopulateRequest { game_path, steam_client_install_path: PathBuf::new() })` -> `SteamAutoPopulateResult`

The `steam_client_install_path` is empty by default, causing the function to fall back to standard Linux Steam root discovery.

**JSON output schema:** Direct serialization of `SteamAutoPopulateResult`:

```json
{
  "app_id_state": "Found",
  "app_id": "1245620",
  "compatdata_state": "Found",
  "compatdata_path": "/home/user/.local/share/Steam/steamapps/compatdata/1245620",
  "proton_state": "Found",
  "proton_path": "/home/user/.local/share/Steam/compatibilitytools.d/GE-Proton-9-4/proton",
  "diagnostics": ["..."],
  "manual_hints": ["..."]
}
```

**Human output:**

```
Auto-populate results for /games/elden-ring/eldenring.exe:
  App ID:      1245620 (found)
  Compatdata:  /home/user/.local/share/Steam/steamapps/compatdata/1245620 (found)
  Proton:      /home/user/.local/share/Steam/compatibilitytools.d/GE-Proton-9-4/proton (found)
```

**Potential arg enhancement:** Consider `--steam-path <PATH>` arg to pass an explicit `steam_client_install_path`. Low priority since auto-discovery works for most setups.

### 7. `crosshook launch` (extend for proton_run and native)

**Current state:** `launch_profile()` in main.rs hard-rejects non-`steam_applaunch` profiles at line 206:

```rust
if method != METHOD_STEAM_APPLAUNCH {
    return Err("crosshook-cli launch currently supports only steam_applaunch profiles".into());
}
```

**Required changes:**

Replace `steam_launch_request_from_profile()` with a universal `launch_request_from_profile()` that builds a `LaunchRequest` for any method, then dispatch to the appropriate command builder.

**Launch dispatch (mirror Tauri `launch_game`):**

```rust
let method = request.resolved_method();
let mut command = match method {
    METHOD_STEAM_APPLAUNCH => {
        let script_path = resolve_helper_script(command_args)?;
        build_helper_command(&request, &script_path, &log_path)
    }
    METHOD_PROTON_RUN => build_proton_game_command(&request, &log_path)?,
    METHOD_NATIVE => build_native_game_command(&request, &log_path)?,
    other => return Err(format!("unsupported launch method: {other}").into()),
};
```

**LaunchRequest construction per method:**

| Field                             | steam_applaunch                | proton_run                     | native                         |
| --------------------------------- | ------------------------------ | ------------------------------ | ------------------------------ |
| `method`                          | `"steam_applaunch"`            | `"proton_run"`                 | `"native"`                     |
| `game_path`                       | `profile.game.executable_path` | `profile.game.executable_path` | `profile.game.executable_path` |
| `trainer_path`                    | `profile.trainer.path`         | `profile.trainer.path`         | `""` (unused)                  |
| `trainer_host_path`               | `profile.trainer.path`         | `profile.trainer.path`         | `""` (unused)                  |
| `steam.app_id`                    | required                       | `""`                           | `""`                           |
| `steam.compatdata_path`           | required                       | `""`                           | `""`                           |
| `steam.proton_path`               | required                       | `""`                           | `""`                           |
| `steam.steam_client_install_path` | resolved                       | resolved                       | `""`                           |
| `runtime.prefix_path`             | `""`                           | required                       | `""`                           |
| `runtime.proton_path`             | `""`                           | required                       | `""`                           |
| `runtime.working_directory`       | `""`                           | from profile                   | from profile                   |
| `optimizations`                   | from profile                   | from profile                   | `default()`                    |
| `launch_game_only`                | `true`                         | `true`                         | `true`                         |

**Key implementation details:**

1. **Method resolution**: Use `resolve_launch_method(&profile)` from `profile/models.rs` to determine the method. This mirrors what the Tauri frontend does.

2. **proton_run requires runtime section**: `profile.runtime.prefix_path` and `profile.runtime.proton_path` must be populated. Validation (`launch::validate()`) will catch missing fields.

3. **native is simplest**: Just needs `game_path` and an optional `working_directory`. Validation rejects `.exe` files for native.

4. **Helper script resolution for steam_applaunch**: The existing `default_scripts_dir()` uses `CARGO_MANIFEST_DIR` which works in dev mode. For production CLI, add `--scripts-dir` override (already exists as hidden arg).

5. **Log path**: Currently uses `/tmp/crosshook-logs/{safe-name}.log`. Consider using `create_log_path()` style with timestamp to avoid overwriting old logs, but this is a polish item.

6. **Launch optimizations for proton_run**: The profile's `launch.optimizations.enabled_option_ids` should be forwarded to `LaunchOptimizationsRequest`.

**proton_run LaunchRequest builder:**

```rust
fn proton_launch_request_from_profile(profile: &GameProfile) -> Result<LaunchRequest, Box<dyn Error>> {
    let steam_client_install_path = resolve_steam_client_install_path(&profile.steam.compatdata_path);

    Ok(LaunchRequest {
        method: METHOD_PROTON_RUN.to_string(),
        game_path: profile.game.executable_path.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_host_path: profile.trainer.path.clone(),
        trainer_loading_mode: profile.trainer.loading_mode,
        steam: SteamLaunchConfig {
            steam_client_install_path: steam_client_install_path.to_string_lossy().into_owned(),
            ..Default::default()
        },
        runtime: RuntimeLaunchConfig {
            prefix_path: profile.runtime.prefix_path.clone(),
            proton_path: profile.runtime.proton_path.clone(),
            working_directory: profile.runtime.working_directory.clone(),
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

**native LaunchRequest builder:**

```rust
fn native_launch_request_from_profile(profile: &GameProfile) -> Result<LaunchRequest, Box<dyn Error>> {
    Ok(LaunchRequest {
        method: METHOD_NATIVE.to_string(),
        game_path: profile.game.executable_path.clone(),
        runtime: RuntimeLaunchConfig {
            working_directory: profile.runtime.working_directory.clone(),
            ..Default::default()
        },
        launch_trainer_only: false,
        launch_game_only: true,
        ..Default::default()
    })
}
```

## System Constraints

### Async Runtime

- CLI uses `tokio` with `rt-multi-thread` (already configured)
- `launch_profile()` is async for process spawning and log streaming
- `handle_profile_command()` and `handle_steam_command()` are async but currently only use `emit_placeholder()`
- Most crosshook-core functions are synchronous (blocking filesystem I/O), called from async context without `spawn_blocking`
- The Tauri commands use `spawn_blocking` for heavy operations; the CLI can call these synchronously since there's no event loop to block
- Exception: `build_proton_game_command()` and `build_native_game_command()` return `tokio::process::Command`, which requires tokio runtime

### Process Spawning

- `steam_applaunch` spawns `/bin/bash` running a helper shell script
- `proton_run` spawns the Proton binary directly (or via host wrappers like `gamemoderun`)
- `native` spawns the game executable directly
- All three redirect stderr/stdout to a log file via `attach_log_stdio()`
- The CLI's `stream_helper_log()` polls the log file and streams to stdout -- this pattern works for all three methods

### File System Access

- Profile store reads/writes to `~/.config/crosshook/profiles/*.toml`
- Settings at `~/.config/crosshook/settings.toml`
- Steam discovery scans `~/.steam/root`, `~/.local/share/Steam`, `~/.var/app/com.valvesoftware.Steam/data/Steam`
- Launch logs at `/tmp/crosshook-logs/`
- Legacy profiles use `.profile` extension in the same or specified directory

### No New Dependencies Required

- All serialization uses `serde` + `serde_json` (already in `crosshook-cli/Cargo.toml`)
- All core operations come from `crosshook-core`
- No new crates needed

## Codebase Changes Summary

### Files to Modify

1. **`crates/crosshook-cli/src/main.rs`** -- Replace 6 placeholder handlers and extend `launch_profile()`
2. **`crates/crosshook-cli/src/args.rs`** -- No changes required (all args already defined)
3. **`crates/crosshook-cli/Cargo.toml`** -- No changes required (all deps present)

### Implementation Order (dependency-based)

1. Add shared `output()` helper function
2. `profile list` (simplest, validates pattern)
3. `profile import` (straightforward, one core call)
4. `profile export` (needs default output path logic)
5. `steam discover` (multi-step discovery)
6. `steam auto-populate` (single core call)
7. `status` (aggregates multiple sources)
8. `launch` extension (most complex, needs new request builders and dispatch)

## Technical Decisions

### Decision 1: Unified vs. Per-Method LaunchRequest Builder

**Option A (Recommended): Single `launch_request_from_profile()` with method dispatch inside**

- Centralizes request construction
- Method resolved once via `resolve_launch_method()`
- Maps profile fields to LaunchRequest based on resolved method

**Option B: Separate builder per method (current steam-only approach)**

- More explicit but duplicates common field mapping
- Current `steam_launch_request_from_profile()` would remain as-is

**Recommendation:** Option A. A single function with an internal match on the resolved method avoids duplication and ensures the method resolution logic from `profile/models.rs` is used consistently.

### Decision 2: Error Propagation Strategy

**Option A (Recommended): `Box<dyn Error>` passthrough (current pattern)**

- Already used throughout main.rs
- Simple, sufficient for a CLI binary

**Option B: Custom CLI error enum**

- More structured but over-engineering for a thin CLI wrapper

**Recommendation:** Option A. The CLI is a thin orchestration layer; custom error types add no value here.

### Decision 3: JSON Schema for Status Command

**Option A (Recommended): Flat struct with optional sections**

- Steam section is `Option<SteamStatus>` when Steam is not found
- Profile section always present (possibly empty list)

**Option B: Result-per-section with error messages**

- Each section has `{ "data": ..., "error": null }` shape

**Recommendation:** Option A. Simpler to consume. Errors surface via the `diagnostics` array.

### Decision 4: Launch Log Streaming for proton_run and native

**Option A (Recommended): Reuse existing `stream_helper_log()` pattern**

- The spawned process writes to a log file; CLI polls and streams to stdout
- Works identically for all three methods since `attach_log_stdio()` redirects process output to the log file

**Option B: Direct stdout/stderr capture**

- Simpler but loses log file persistence for post-mortem analysis

**Recommendation:** Option A. Consistent with existing pattern and preserves log files for diagnostic analysis.

## Gotchas and Edge Cases

1. **`ProfileStore::load()` returns effective profile**: The `load()` method calls `effective_profile()` internally, merging `local_override` into base fields and clearing the override section. The CLI gets the ready-to-use profile -- no need to call `effective_profile()` again.

2. **`storage_profile()` vs `effective_profile()` for export**: `export_community_profile()` internally calls `portable_profile()` which strips machine paths. The CLI just passes through -- no need to pre-process the profile.

3. **Steam client install path resolution**: Both the CLI and Tauri have independent `resolve_steam_client_install_path()` / `default_steam_client_install_path()` implementations. The CLI version walks ancestors of `compatdata_path` looking for `steam.sh`. This works for `steam_applaunch` but `proton_run` profiles may not have `compatdata_path` set -- the CLI should fall back to `discover_steam_root_candidates("")` in that case.

4. **`build_proton_game_command()` can fail**: Returns `std::io::Result<Command>` because `attach_log_stdio()` creates the log file. The log directory `/tmp/crosshook-logs` must be created first. The CLI's existing `launch_log_path()` doesn't create the directory. Add `fs::create_dir_all()` before spawning.

5. **native launch rejects `.exe` files**: `validate()` for native method returns `NativeWindowsExecutableNotSupported` if the game path ends with `.exe`. This is intentional -- native means Linux-native executables only.

6. **Trainer launch is CLI game-only**: The CLI currently only does `launch_game_only = true`. Trainer-only and combined launch are Tauri-only features. This is by design for v1.

7. **`LaunchSection::normalize_preset_selection()`**: Called by `ProfileStore::load()`. If a profile has an `active_preset` referencing a bundled or user preset, the active preset's optimization IDs are copied into `launch.optimizations`. The CLI gets the resolved optimization set automatically.

8. **`serde_json` for `GameProfile`**: All profile types derive both `Serialize` and `Deserialize`, so `serde_json::to_string_pretty()` works out of the box. No custom serialization needed.

9. **`discover_compat_tools()` may be slow**: Scans filesystem for all Proton installs. For the `status` command, consider whether this should be gated behind `--verbose` or always included. Recommendation: always include since it's useful system info.

10. **Legacy import preserves Windows path normalization**: The `legacy::load()` function converts `Z:\path\to\file` to `/path/to/file`. The `From<LegacyProfileData>` conversion also derives the launch method. No extra handling needed in the CLI.

11. **`discover_steam_libraries` is not re-exported**: The `crosshook_core::steam` module root does NOT re-export `discover_steam_libraries`. Import it directly as `crosshook_core::steam::libraries::discover_steam_libraries`. By contrast, `discover_compat_tools` IS re-exported. Always check the re-export list in `steam/mod.rs` before importing.

12. **`export_community_profile` takes `profiles_dir: &Path`, not `&ProfileStore`**: The function constructs its own `ProfileStore::with_base_path()` internally. Pass `store.base_path.as_path()` as the first argument, not the store itself.
