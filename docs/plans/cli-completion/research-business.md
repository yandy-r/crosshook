# CLI Completion — Business Analysis

## Executive Summary

CrossHook's CLI binary (`crosshook-cli`) has full argument parsing infrastructure but 6 commands are stub placeholders emitting "not_implemented" responses. The core library (`crosshook-core`) already implements all required business logic consumed identically by the Tauri GUI — wiring the CLI commands is principally an integration exercise. The most complex business gap is the `launch` command, which currently supports only `steam_applaunch` but must add `proton_run` (direct Proton process) and `native` (direct Linux process) methods that each have distinct validation rules, environment setup, and process spawning paths.

---

## User Stories

### Steam Deck Console-Mode User

- As a Steam Deck user in desktop mode, I want to launch a configured game+trainer pair from the terminal so I can use CrossHook without the GUI.
- As a Steam Deck user, I want `--json` output from any command so I can parse results in shell scripts or Decky Loader plugins.

### Power User / Linux Desktop User

- As a Linux power user, I want `crosshook profile list` to enumerate my profiles so I can pipe the names into other scripts.
- As a Linux power user, I want `crosshook steam discover` to tell me which Steam installations are found so I can diagnose why CrossHook is not detecting my games.
- As a Linux power user, I want `crosshook steam auto-populate --game-path /path/to/game.exe` to pre-fill Steam metadata into a profile so I don't have to manually find App IDs and compat paths.

### Automation / CI User

- As a CI system, I want `crosshook launch --profile elden-ring` to return a non-zero exit code on failure so I can detect launch failures in automated test pipelines.
- As a scripter, I want `crosshook profile export --profile elden-ring --output /tmp/elden-ring.json` to produce a portable community profile JSON so I can share configurations.
- As a scripter, I want `crosshook profile import --legacy-path /path/to/old.profile` to convert a legacy `.profile` file into a TOML profile so I can automate migrations.

### System Administrator

- As a sysadmin, I want `crosshook status` to show system diagnostics and a profile summary so I can quickly assess the health of a CrossHook installation.

---

## Business Rules

### Core Rules Per Command

#### `crosshook status`

- Must report: number of profiles found, profile store base path, Steam installation candidates found via `discover_steam_root_candidates`, and basic system facts (platform, user).
- No destructive operations — read-only.
- `--json` output must be a structured object, not free-form text.
- Must not fail if zero profiles exist — empty list is a valid state.

#### `crosshook profile list`

- Calls `ProfileStore::list()` which returns sorted profile names (`.toml` files in the profiles directory).
- Human output: one name per line.
- JSON output: `{"profiles": ["name1", "name2"]}`.
- Must not fail if the profiles directory does not exist — return empty list (the store already handles this).
- Respects `--config` to override the base profiles directory.

#### `crosshook profile import`

- Calls `ProfileStore::import_legacy(legacy_path)`.
- The profile name is derived from the `.profile` filename stem — no separate `--name` arg exists in the current arg schema.
- On success: reports the saved profile name and path.
- On failure: propagates `ProfileStoreError` — most commonly `InvalidName` (bad filename), `NotFound` (file missing), or `Io`.
- Legacy `.profile` format uses Windows-style `Z:\` paths; the store normalizes these automatically via `legacy::load`.
- Idempotent: calling import on the same path twice will overwrite the existing TOML (save is not guarded by AlreadyExists for import).

#### `crosshook profile export`

- Calls `export_community_profile(profiles_dir, profile_name, output_path)`.
- Requires `--profile` (name) to be set (from `--profile` arg or global `--profile` flag).
- `--output` is the destination path for the community JSON. If omitted, a sensible default is needed (e.g. current directory + `<profile-name>.json`).
- The exported JSON strips all machine-specific paths (game path, trainer path, compatdata, Proton path, dll paths).
- Preserved fields: game name, Steam app ID, launch method, trainer kind.
- Error cases: profile not found, output directory not writable, schema version mismatch.

#### `crosshook steam discover`

- Calls `discover_steam_root_candidates(steam_client_install_path, &mut diagnostics)`.
- No required args — uses `$HOME` to locate default Steam roots.
- Human output: list of found root paths with their discovery source label (e.g. "Default Steam root: /home/user/.local/share/Steam").
- JSON output: `{"roots": [{"path": "...", "source": "..."}], "diagnostics": [...]}`.
- Must be non-destructive and silent on no-Steam-found scenarios (return empty list, not an error).

#### `crosshook steam auto-populate`

- Requires `--game-path` (path to the game executable).
- Calls `attempt_auto_populate(&SteamAutoPopulateRequest { game_path, steam_client_install_path: PathBuf::new() })`.
- Returns `SteamAutoPopulateResult` with `app_id`, `compatdata_path`, `proton_path` and their states (`Found`, `NotFound`, `Ambiguous`).
- Human output: per-field found/not-found status with values.
- JSON output: serialize `SteamAutoPopulateResult` directly (it already derives `Serialize`).
- Does not create or modify any profile — discovery only.
- The game executable does not need to currently exist on the filesystem — the auto-populate still attempts a manifest match.
- Optional `--steam-path` arg (not yet in `SteamAutoPopulateCommand` args): the `SteamAutoPopulateRequest.steam_client_install_path` field accepts an explicit Steam root path to override `$HOME`-based discovery. The Tauri command exposes this; the CLI args struct should add an optional `--steam-path` flag to match.

#### `crosshook launch`

- Currently only handles `steam_applaunch`. Must add `proton_run` and `native`.
- Method selection logic: `LaunchRequest::resolved_method()` already handles the resolution — the CLI must stop hard-coding `steam_applaunch` as the only valid method.

**proton_run method requirements:**

- `profile.runtime.proton_path` (or `profile.steam.proton_path`) must be set and executable.
- `profile.runtime.prefix_path` (or `profile.steam.compatdata_path`) must be a directory.
- Uses `build_proton_game_command(&request, &log_path)` from `script_runner`.
- No helper shell script required — Proton process is spawned directly.

**native method requirements:**

- `profile.game.executable_path` must be a file.
- `profile.trainer.path` must not be set (native launch does not support trainer launch per Tauri's implementation).
- Uses `build_native_game_command(&request, &log_path)` from `script_runner`.

**Shared launch rules:**

- Profile must be loaded via `ProfileStore`.
- `launch::validate(&request)` must pass before spawn.
- Log streaming behavior is the same across methods (tail `/tmp/crosshook-logs/<slug>.log`).
- On non-zero exit: propagate error with status code message.
- `--json` mode should not stream raw log lines; instead return a JSON result on completion.
- **MetadataStore (launch history)**: The CLI does NOT need to initialize `MetadataStore` for v1. `MetadataStore::disabled()` is the valid headless path — all store operations silently no-op when disabled. CLI launches will not record history or update health snapshots; this is an accepted limitation for the headless use case. If future requirements demand history tracking, `MetadataStore::try_new()` can be called with a soft fallback to `disabled()`, matching the pattern in `src-tauri/lib.rs:32-35`.

### Edge Cases and Validation

- **Profile name with path chars**: `validate_name` rejects `/`, `\`, `:` and Windows reserved chars — error message must clearly explain this.
- **Import overwrites existing profile**: `import_legacy` calls `save` which does not guard for conflicts — warn user if a profile with that name already exists (requires a pre-check via `list()`).
- **Export with no `--output`**: Must default to `<profile-name>.json` in the current working directory, not panic.
- **Steam auto-populate on a non-existent game path**: Allowed — auto-populate still scans manifests; the diagnostic message explains the file does not exist.
- **`discover_steam_root_candidates` with no Steam installed**: Returns empty `Vec` with no candidates; human output should say "no Steam installations found" rather than silently succeeding.
- **`proton_run` with empty `proton_path`**: Validation via `launch::validate` emits `SteamProtonPathRequired` or `RuntimeProtonPathRequired` as Fatal severity — must surface as a CLI error.
- **`native` with a Windows `.exe` path**: `validate` emits `NativeWindowsExecutableNotSupported` — must surface as a CLI error.

---

## Workflows

### `profile list`

1. Initialize `ProfileStore` (from `--config` or default `~/.config/crosshook/profiles`).
2. Call `store.list()`.
3. If `--json`: serialize as `{"profiles": [...]}`.
4. Else: print each name on its own line (or "no profiles found" if empty).
5. Exit 0.

### `profile import`

1. Validate `--legacy-path` file exists (early user-friendly check before calling the store).
2. Optionally: check if a profile with the same stem name already exists via `store.list()` — warn but proceed.
3. Call `store.import_legacy(&legacy_path)`.
4. On success: print `Imported profile '<name>' to <path>` (or JSON equivalent).
5. On error: print error, exit 1.

### `profile export`

1. Resolve profile name (from `--profile` or `--global.profile`) — error if unset.
2. Resolve output path (from `--output` or default `<cwd>/<profile-name>.json`).
3. Initialize `ProfileStore`.
4. Call `export_community_profile(&store.base_path, &profile_name, &output_path)`.
5. On success: print `Exported '<name>' to <output_path>` (or JSON `CommunityExportResult`).
6. On error: print error, exit 1.

### `steam discover`

1. Collect any `--config` hint as `steam_client_install_path` (or empty string).
2. Call `discover_steam_root_candidates(steam_client_install_path, &mut diagnostics)`.
3. If `--json`: serialize `{"roots": [...], "diagnostics": [...]}`.
4. Else: print each root path with label, then diagnostics if `--verbose`.
5. Always exit 0 (empty result is not an error).

### `steam auto-populate`

1. Build `SteamAutoPopulateRequest { game_path, steam_client_install_path: steam_path.unwrap_or_default() }` where `steam_path` comes from an optional `--steam-path` flag.
2. Call `attempt_auto_populate(&request)`.
3. If `--json`: serialize `SteamAutoPopulateResult` directly.
4. Else: print per-field status (app_id: found / not found, path: ...).
5. If `--verbose`: also print `result.diagnostics` and `result.manual_hints`.
6. Always exit 0 (no-match is informational, not an error).

### `launch` (full method support)

1. Resolve profile name and load via `ProfileStore`.
2. Build `LaunchRequest` from profile using `resolved_method()` to select the method.
3. For `steam_applaunch`: use existing `steam_launch_request_from_profile` logic.
4. For `proton_run`: build `LaunchRequest` with `runtime` config populated from `profile.runtime` (proton_path, prefix_path, working_directory).
5. For `native`: build `LaunchRequest` with method = `native`.
6. Call `launch::validate(&request)` — on validation failure, print each issue with severity label, exit 1.
7. Resolve log path via `launch_log_path`.
8. Spawn process via the appropriate `script_runner` function.
9. Stream log to stdout (existing implementation).
10. On exit: run `launch::analyze` and surface any diagnostic report.
11. Return exit code reflecting child process success/failure.

### Error Recovery

- All commands: unrecoverable errors (store init failure, I/O errors) print to stderr and exit 1.
- `launch`: if the helper process is spawned but exits non-zero, stream available log before reporting failure.
- `profile import`: if legacy file is malformed (unknown keys silently ignored by the parser), import may succeed with an incomplete profile — this is existing behavior, not a new concern.

---

## Domain Model

### Entities

| Entity                     | Storage                                                | Key Fields                                                     |
| -------------------------- | ------------------------------------------------------ | -------------------------------------------------------------- |
| `GameProfile`              | TOML file (`~/.config/crosshook/profiles/<name>.toml`) | game, trainer, steam, runtime, launch, injection sections      |
| `LegacyProfileData`        | `.profile` key=value file                              | GamePath, TrainerPath, SteamAppId, LaunchMethod                |
| `CommunityProfileManifest` | JSON file                                              | schema_version, metadata, profile (sanitized GameProfile)      |
| `SteamAutoPopulateResult`  | In-memory only                                         | app_id, compatdata_path, proton_path, field states             |
| `LaunchRequest`            | In-memory (IPC)                                        | method, game_path, trainer_path, steam, runtime, optimizations |

### State Transitions

**Profile lifecycle (CLI-relevant):**

```
[legacy .profile] --import--> [TOML GameProfile] --export--> [community JSON]
                                      |
                                   launch (steam_applaunch | proton_run | native)
```

**Launch method resolution (in `LaunchRequest::resolved_method()`):**

```
explicit method set? yes --> use it
no: steam.app_id present? yes --> steam_applaunch
no: game_path looks like .exe? yes --> proton_run
no: --> native
```

**Steam auto-populate field states:**

```
SteamAutoPopulateFieldState: NotFound | Found | Ambiguous
```

- `Found`: value was detected unambiguously.
- `Ambiguous`: multiple matches found (multiple installs of same game across libraries).
- `NotFound`: no match — user must set manually.

---

## Existing Codebase Integration

### How Tauri Commands Consume crosshook-core (Reference Patterns)

The pattern for all CLI commands mirrors how Tauri IPC handlers use the core library:

| CLI Command           | Core Function                                               | Tauri Equivalent                                         |
| --------------------- | ----------------------------------------------------------- | -------------------------------------------------------- |
| `profile list`        | `ProfileStore::list()`                                      | `list_profiles` in `commands/profile.rs`                 |
| `profile import`      | `ProfileStore::import_legacy()`                             | `import_legacy_profile` in `commands/profile.rs`         |
| `profile export`      | `export_community_profile()`                                | `export_community_profile` in `commands/community.rs`    |
| `steam discover`      | `discover_steam_root_candidates()`                          | `list_proton_installs` in `commands/steam.rs`            |
| `steam auto-populate` | `attempt_auto_populate()`                                   | `auto_populate_steam` in `commands/steam.rs`             |
| `status`              | `ProfileStore::list()` + `discover_steam_root_candidates()` | No direct equivalent                                     |
| `launch` (proton_run) | `build_proton_game_command()`                               | `launch_game` method dispatch in `commands/launch.rs:70` |
| `launch` (native)     | `build_native_game_command()`                               | `launch_game` method dispatch in `commands/launch.rs:72` |

### Key Function Signatures

```rust
// profile/toml_store.rs
ProfileStore::list(&self) -> Result<Vec<String>, ProfileStoreError>
ProfileStore::import_legacy(&self, legacy_path: &Path) -> Result<GameProfile, ProfileStoreError>

// profile/exchange.rs
export_community_profile(profiles_dir: &Path, profile_name: &str, output_path: &Path)
    -> Result<CommunityExportResult, CommunityExchangeError>

// steam/discovery.rs
discover_steam_root_candidates(steam_client_install_path: impl AsRef<Path>, diagnostics: &mut Vec<String>)
    -> Vec<PathBuf>

// steam/auto_populate.rs (via mod.rs pub use)
attempt_auto_populate(request: &SteamAutoPopulateRequest) -> SteamAutoPopulateResult

// launch/script_runner.rs
build_proton_game_command(request: &LaunchRequest, log_path: &Path) -> io::Result<Command>
build_native_game_command(request: &LaunchRequest, log_path: &Path) -> io::Result<Command>
```

### Diagnostics / JSON Output Pattern

The already-implemented `diagnostics export` command (see `main.rs:146-178`) demonstrates the canonical `--json` pattern:

```rust
if global.json {
    println!("{}", serde_json::to_string_pretty(&result)?);
} else {
    // human-readable lines
}
```

All new commands should follow this same pattern. All result types (`CommunityExportResult`, `SteamAutoPopulateResult`, etc.) already derive `Serialize`.

### ProfileStore Construction Pattern

Already established in `main.rs:189-197`:

```rust
fn profile_store(profile_dir: Option<PathBuf>) -> ProfileStore {
    match profile_dir {
        Some(path) => ProfileStore::with_base_path(path),
        None => ProfileStore::try_new().unwrap_or_else(|error| { ... exit(1) }),
    }
}
```

All new commands should reuse this helper or a refactored equivalent.

### MetadataStore and Launch History

`MetadataStore` (`metadata/mod.rs`) manages launch history, version snapshots, health data, and config revisions. It is a SQLite-backed store initialized via `MetadataStore::try_new()` and has an explicit disabled/no-op mode via `MetadataStore::disabled()`.

The Tauri `launch_game` command initializes `MetadataStore` as managed Tauri state and records launch start/finish/version snapshots on every launch. The CLI does not need to replicate this for v1 — `MetadataStore::disabled()` is the correct no-op path.

Key facts:

- `MetadataStore::disabled()` makes `is_available()` return false and all operations return `Ok(Default::default())` — no panics, no errors.
- DB path: `~/.local/share/crosshook/metadata.db` (via `BaseDirs::data_local_dir()`).
- DB file and directory permissions are set to 0600/0700 on open (`db.rs:26-46`).
- If the CLI does gain MetadataStore support in a future iteration, it should use `MetadataStore::try_new()` with a soft fallback, matching `src-tauri/lib.rs:32-35`, never hard-fail on SQLite unavailability.

`profile export` format: the CLI `profile export` command wraps `export_community_profile()` which produces a community-shareable JSON (paths stripped). This is the correct format — a raw TOML export/backup use case is not part of this feature.

---

## Success Criteria

1. `crosshook profile list` returns profile names, exits 0, supports `--json`.
2. `crosshook profile import --legacy-path <path>` converts a `.profile` file and saves a TOML, exits 0 on success and 1 on error.
3. `crosshook profile export --profile <name>` writes a community JSON, exits 0 on success; exits 1 if profile is not found.
4. `crosshook steam discover` lists found Steam roots, exits 0 regardless of whether any are found.
5. `crosshook steam auto-populate --game-path <path>` reports Steam metadata fields, exits 0.
6. `crosshook status` reports profile count and Steam root count, exits 0.
7. `crosshook launch --profile <name>` succeeds for profiles with `proton_run` and `native` methods, not only `steam_applaunch`.
8. All commands produce valid JSON when `--json` is passed, parseable by `jq`.
9. All commands exit 1 on error and print a diagnostic to stderr.
10. No regressions: existing `diagnostics export` and `steam_applaunch` launch continue to work.

---

## Resolved Questions

1. **`profile export` default output path**: Default to `<cwd>/<profile-name>.json` when `--output` is omitted. If the working directory is not writable the error from `export_community_profile` will surface clearly. No change to args struct needed.
2. **`profile import` name collision**: Warn to stderr if the stem name already exists (pre-check via `store.list()`), then proceed with the overwrite. Do not fail — import is a migration operation and idempotency is desirable.
3. **`status` scope**: Simple count + Steam root list is sufficient for v1. No `ProfileHealthReport` — that requires MetadataStore and is scope creep for a headless status check.
4. **`launch` proton_run/native scripts dir**: Confirmed correct — `build_proton_game_command` and `build_native_game_command` spawn the process directly without a helper shell script. Only `steam_applaunch` requires the bundled `steam-launch-helper.sh`.
5. **`--json` for `launch`**: In `--json` mode, suppress log streaming to stdout and emit a single JSON result object after the child process exits. Log lines should still be written to the log file at `/tmp/crosshook-logs/`. This avoids NDJSON complexity and keeps JSON output parseable.

## Open Questions

1. **`--steam-path` arg in `SteamAutoPopulateCommand`**: The `SteamAutoPopulateCommand` struct in `args.rs` currently only has `game_path`. An optional `--steam-path` field should be added to pass `steam_client_install_path` to `attempt_auto_populate`. This is a minor args.rs change — confirm with the feature implementer.
2. **`MetadataStore` for CLI launch (future)**: If launch history from CLI becomes a requirement, the pattern is `MetadataStore::try_new()` with soft fallback to `disabled()`. Out of scope for v1.
