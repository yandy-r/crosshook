# CLI Completion Implementation Plan

Wire all 6 placeholder CLI commands in `crosshook-cli/src/main.rs` to their corresponding `crosshook-core` functions and extend the `launch` command to support `proton_run` and `native` methods alongside the existing `steam_applaunch`. All business logic already exists in `crosshook-core` — the work is pure wiring following the established `handle_diagnostics_command` pattern at `main.rs:145-178`. Two CRITICAL security mitigations (C-1: helper script path validation, C-2: import path containment) and two WARNING mitigations (W-2: log path TOCTOU, W-4: export path validation) are inlined into the tasks they protect. The plan concludes with exit code standardization, shell completions via `clap_complete`, quickstart documentation, and dead code cleanup.

## Critically Relevant Files and Documentation

- src/crosshook-native/crates/crosshook-cli/src/main.rs: All command dispatch, 6 `emit_placeholder()` stubs, `steam_launch_request_from_profile()`, `launch_profile()`, `profile_store()` helper, reference `handle_diagnostics_command` at line 145
- src/crosshook-native/crates/crosshook-cli/src/args.rs: Clap-based argument structs for all 7 commands; needs `///` doc comments; `--dry-run` and completions subcommand added in Phase 5
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore::list()`, `load()`, `import_legacy()`, `try_new()`, `with_base_path()`
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs: `export_community_profile(profiles_dir: &Path, name, output_path)` — takes `&Path` not `&ProfileStore`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile`, section structs, `resolve_launch_method()` at line 363
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: `LaunchRequest`, `SteamLaunchConfig`, `RuntimeLaunchConfig`, `validate()`, `ValidationError`, method constants
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: `build_helper_command()` → `Command`; `build_proton_game_command()` → `io::Result<Command>`; `build_native_game_command()` → `io::Result<Command>`
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs: `analyze()`, `should_surface_report()` — post-launch log analysis
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs: `discover_steam_root_candidates(path, &mut diagnostics)`
- src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs: `discover_steam_libraries()` — NOT re-exported from `steam/mod.rs`; import as `crosshook_core::steam::libraries::discover_steam_libraries`
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: `discover_compat_tools(roots, &mut diagnostics)`
- src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs: `attempt_auto_populate(&SteamAutoPopulateRequest)`
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs: `SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `ProtonInstall`, `SteamAutoPopulateFieldState`
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `SettingsStore`, `AppSettingsData`
- src/crosshook-native/src-tauri/src/commands/launch.rs: Tauri reference for proton_run + native dispatch (lines 52-78)
- src/crosshook-native/src-tauri/src/commands/steam.rs: Tauri reference for steam discover + auto-populate
- src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri reference for profile list/import/export
- docs/plans/cli-completion/feature-spec.md: Master spec — architecture, JSON schemas, security hard stops, code templates
- docs/plans/cli-completion/research-technical.md: 12 gotchas that cause compile errors if missed
- docs/plans/cli-completion/research-security.md: 2 CRITICAL + 5 WARNING findings with mitigations
- docs/plans/cli-completion/research-integration.md: Complete core function signatures and data models
- docs/getting-started/quickstart.md: Needs CLI section added (Phase 5)

## Implementation Plan

### Phase 1: Foundation and Simple Reads

#### Task 1.1: Add doc comments to all CLI command variants Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/args.rs
- docs/plans/cli-completion/feature-spec.md (§Business Rules for per-command descriptions)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/args.rs

Add `///` doc comments to every `Command`, `ProfileCommand`, `SteamCommand`, and `DiagnosticsCommand` enum variant and their argument fields. These comments become the `--help` output via clap derive. Use the business rule descriptions from `feature-spec.md` as the source for accurate, user-facing descriptions.

Example pattern for a variant:

```rust
/// List all saved profiles
List,
```

Example pattern for a field:

```rust
/// Path to legacy .profile file to import
#[arg(long = "legacy-path", value_name = "PATH")]
pub legacy_path: PathBuf,
```

Do not change any structural aspects of the argument definitions — only add `///` doc comments. Run `cargo check -p crosshook-cli` to verify no breakage.

#### Task 1.2: Wire `profile list` command Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (lines 98-126 for `handle_profile_command`, lines 145-178 for reference pattern)
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs (`ProfileStore::list()` signature)
- src/crosshook-native/src-tauri/src/commands/profile.rs (Tauri reference for list)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

Replace the `emit_placeholder(global, "profile list")` call in the `ProfileCommand::List` arm of `handle_profile_command` with a real implementation:

1. Initialize `ProfileStore` using the existing `profile_store(global.config.clone())` helper.
2. Call `store.list().map_err(|e| format!("failed to list profiles: {e}"))?` to get `Vec<String>`.
3. Branch on `global.json`:
   - JSON: Define an inline `#[derive(serde::Serialize)] struct ListOutput` with fields `profiles: &[String]`, `count: usize`, `profiles_dir: String`. Serialize and `println!`.
   - Human: Print one name per line. After the list, print `"{count} profile(s) in {dir}"` summary line.
4. Empty list is not an error — print 0 profiles with the directory path.

This handler must be `async fn` for consistency (even though it contains no `await`).

#### Task 1.3: Wire `status` command and deduplicate `resolve_steam_client_install_path` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (line 43 for `Command::Status` arm, lines 236-269 for `resolve_steam_client_install_path`)
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs (`SettingsStore::try_new()`, `AppSettingsData`)
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs (`discover_steam_root_candidates`)
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs (`discover_compat_tools`)
- docs/plans/cli-completion/feature-spec.md (§`crosshook status` business rules, JSON schema)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

**Part 1 — Wire `status` command:**

1. Create a new `async fn handle_status_command(global: &GlobalOptions) -> Result<(), Box<dyn Error>>`.
2. Replace `emit_placeholder(global, "status")` in the `Command::Status` arm of `run()` with `handle_status_command(&cli.global).await?`.
3. Implementation aggregates:
   - `ProfileStore::try_new()` → `store.list()` for profile count and names (wrap in `match` — partial failure should not abort)
   - `SettingsStore::try_new()` → settings data (wrap in `match`)
   - `discover_steam_root_candidates("", &mut diagnostics)` → Steam roots
   - `discover_compat_tools(&roots, &mut diagnostics)` → Proton installs
4. Emit `global.verbose` diagnostics via `eprintln!`.
5. Branch on `global.json`:
   - JSON: Define an inline `#[derive(serde::Serialize)] struct StatusOutput` matching the schema in feature-spec.md (version, profiles, settings, steam, diagnostics).
   - Human: Print labeled sections (version, profile count, Steam roots, Proton installs, settings summary).
6. Use `env!("CARGO_PKG_VERSION")` for the version string.
7. Partial failures in any section populate a diagnostics vec rather than returning an error.

**Part 2 — Deduplicate `resolve_steam_client_install_path`:**

Check if `crosshook_core::launch::runtime_helpers::resolve_steam_client_install_path` exists and can replace the local version at `main.rs:236-269`. If the core version exists with a compatible signature, delete the local version and update all call sites to use the core import. If the core version does not exist or has a different signature, leave the local version in place and add a `// TODO: dedup with core version when signatures align` comment.

### Phase 2: Import/Export with Security Mitigations

#### Task 2.1: Wire `profile import` with C-2 path containment mitigation Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (lines 98-126 for `handle_profile_command`)
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs (`ProfileStore::import_legacy()` signature)
- docs/plans/cli-completion/research-security.md (C-2 finding and mitigation)
- docs/plans/cli-completion/research-integration.md (§Import/Export, §Edgecases)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

Replace the `emit_placeholder(global, "profile import")` call in the `ProfileCommand::Import` arm:

1. **C-2 Security Mitigation (CRITICAL — do not skip):**
   - Before calling `import_legacy`, validate the legacy path:

     ```rust
     let meta = std::fs::symlink_metadata(&command.legacy_path)
         .map_err(|e| format!("cannot access import path: {e}"))?;
     if !meta.file_type().is_file() {
         return Err("import path must be a regular file, not a symlink or directory".into());
     }
     ```

   - Use `symlink_metadata` (not `metadata`) so symlinks are detected rather than followed.

2. **Wire import:**
   - Initialize store: `let store = profile_store(global.config.clone());`
   - Call `store.import_legacy(&command.legacy_path)?`
   - Derive profile name from file stem for display: `command.legacy_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown")`
   - The `import_legacy` function internally derives the name from the file stem, validates it against `validate_name()`, and saves the TOML. It returns `Result<GameProfile, ProfileStoreError>`.

3. **Output:**
   - JSON: `{"imported": true, "profile_name": "<stem>", "legacy_path": "<path>", "launch_method": "<resolved>"}`
   - Human: `Imported profile "<name>" from <path> (launch method: <method>)`

#### Task 2.2: Wire `profile export` with W-4 path validation Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (lines 98-126 for `handle_profile_command`)
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs (`export_community_profile` signature — takes `&Path`, NOT `&ProfileStore`)
- docs/plans/cli-completion/research-security.md (W-4 finding)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

Replace the `emit_placeholder(global, "profile export")` call in the `ProfileCommand::Export` arm:

1. **Resolve profile name** using the existing chain:

   ```rust
   let profile_name = command.profile
       .or_else(|| global.profile.clone())
       .ok_or("a profile name is required; use --profile or -p")?;
   ```

2. **Resolve output path** — default to `<cwd>/<name>.crosshook.json` when `--output` is omitted:

   ```rust
   let output_path = command.output.unwrap_or_else(|| {
       std::env::current_dir()
           .unwrap_or_else(|_| PathBuf::from("."))
           .join(format!("{profile_name}.crosshook.json"))
   });
   ```

3. **W-4 Security Mitigation (WARNING — must address):**
   - Validate the output path is not a symlink: `if output_path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) { return Err("output path is a symlink; refusing to write".into()); }`
   - Validate parent directory is writable (check it exists).

4. **Wire export:**
   - Initialize store: `let store = profile_store(global.config.clone());`
   - Call `export_community_profile(store.base_path.as_path(), &profile_name, &output_path)?`
   - **CRITICAL**: Pass `store.base_path.as_path()` (a `&Path`), NOT `&store` — the function takes a directory path and constructs its own store internally.

5. **Output:**
   - JSON: `{"exported": true, "profile_name": "<name>", "output_path": "<path>"}`
   - Human: `Exported profile "<name>" to <path>`

### Phase 3: Steam Discovery

#### Task 3.1: Wire `steam discover` command Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (lines 128-143 for `handle_steam_command`)
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs (`discover_steam_root_candidates`)
- src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs (`discover_steam_libraries` — NOT re-exported)
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs (`discover_compat_tools`)
- src/crosshook-native/src-tauri/src/commands/steam.rs (Tauri reference)
- docs/plans/cli-completion/feature-spec.md (§`crosshook steam discover` business rules, JSON schema)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

Replace the `emit_placeholder(global, "steam discover")` call in the `SteamCommand::Discover` arm:

1. **Add import** — `use crosshook_core::steam::libraries::discover_steam_libraries;`
   **CRITICAL**: `discover_steam_libraries` is NOT re-exported from `steam/mod.rs`. Must import directly from `crosshook_core::steam::libraries`.

2. **Three-step discovery:**

   ```rust
   let mut diagnostics: Vec<String> = Vec::new();
   let roots = discover_steam_root_candidates("", &mut diagnostics);
   let libraries = discover_steam_libraries(&roots, &mut diagnostics);
   let proton_installs = discover_compat_tools(&roots, &mut diagnostics);
   ```

3. **Emit diagnostics under `--verbose`:**

   ```rust
   if global.verbose {
       for msg in &diagnostics { eprintln!("{msg}"); }
   }
   ```

4. **Output:**
   - JSON: `{"roots": [...], "libraries": [...], "proton_installs": [...], "diagnostics": [...]}`. All types already derive `Serialize`.
   - Human: Print labeled sections for roots, libraries, and Proton installs. Empty results are informational, not errors.

5. Always exit 0 — empty result when Steam is not installed is valid.

#### Task 3.2: Wire `steam auto-populate` command Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (lines 128-143 for `handle_steam_command`)
- src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs (`attempt_auto_populate`)
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs (`SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `SteamAutoPopulateFieldState`)
- docs/plans/cli-completion/feature-spec.md (§`crosshook steam auto-populate` business rules)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

Replace the `emit_placeholder(global, "steam auto-populate")` call in the `SteamCommand::AutoPopulate` arm:

1. **Construct request:**

   ```rust
   let request = SteamAutoPopulateRequest {
       game_path: command.game_path.clone(),
       steam_client_install_path: PathBuf::new(), // empty = auto-detect
   };
   ```

2. **Call core function:** `let result = attempt_auto_populate(&request);`
   This is synchronous — no `spawn_blocking` needed in CLI context.

3. **Output:**
   - JSON: `SteamAutoPopulateResult` already derives `Serialize`; serialize directly.
   - Human: Format per-field state with labels:

     ```
     App ID:         <value> (Found)
     Compat Data:    <path> (Found)
     Proton:         <path> (Ambiguous — set manually)
     ```

     For `Ambiguous` fields, append "set manually" hint. For `NotFound`, show "not detected".

4. Does NOT create or modify any profile — discovery only. Always exit 0.

### Phase 4: Launch Completion

#### Task 4.1: Refactor `steam_launch_request_from_profile` into generic `launch_request_from_profile` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (lines 199-234 for `steam_launch_request_from_profile`, lines 49-96 for `launch_profile`)
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs (`resolve_launch_method` at line 363)
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (`LaunchRequest`, `SteamLaunchConfig`, `RuntimeLaunchConfig`, method constants)
- docs/plans/cli-completion/feature-spec.md (§Key Implementation: `launch_request_from_profile()` code template)
- docs/plans/cli-completion/research-integration.md (§Launch System — all three LaunchRequest shapes)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

1. **Add imports:**

   ```rust
   use crosshook_core::profile::models::resolve_launch_method;
   use crosshook_core::launch::{METHOD_PROTON_RUN, METHOD_NATIVE};
   ```

2. **Replace `steam_launch_request_from_profile()`** with `launch_request_from_profile()`:
   - Use `resolve_launch_method(&profile)` to determine the method.
   - Build three distinct `LaunchRequest` shapes using match on the method constant:
     - `METHOD_STEAM_APPLAUNCH`: Populate `steam.*` fields from `profile.steam.*`. Resolve `steam_client_install_path` via the existing `resolve_steam_client_install_path()`.
     - `METHOD_PROTON_RUN`: Populate `runtime.*` fields from `profile.runtime.*` (`prefix_path`, `proton_path`, `working_directory`). Populate `optimizations` from `profile.launch.optimizations.enabled_option_ids`.
     - `METHOD_NATIVE`: Only `game_path` from `profile.game.executable_path`. Working directory from `profile.runtime.working_directory`. No trainer fields for native.
   - Use the code template from `feature-spec.md` §API Design as the starting point.
   - Set `launch_game_only: true`, `launch_trainer_only: false` for all methods (v1 constraint).
   - Set `profile_name: Some(profile_name.to_string())`.

3. **Delete the old `steam_launch_request_from_profile()`** function entirely.

4. **Update `launch_profile()`** to call the new function:
   - Remove the early-return guard that rejects non-`steam_applaunch` methods (around line 206).
   - Replace the call to `steam_launch_request_from_profile()` with `launch_request_from_profile()`.

5. **Gotchas:**
   - `ProfileStore::load()` returns the effective profile with `local_override` merged — no caller-side merging needed.
   - `LaunchRequest.method` can be set to the method string directly; `resolved_method()` on the request is for fallback when method is empty.
   - Confirm `SteamLaunchConfig` and `RuntimeLaunchConfig` derive `Default` before using `..Default::default()` spread syntax.

#### Task 4.2: Wire `proton_run` launch dispatch with W-2 log path mitigation Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (`launch_profile()` function, `launch_log_path()` helper)
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs (`build_proton_game_command` signature — returns `io::Result<Command>`)
- src/crosshook-native/src-tauri/src/commands/launch.rs (lines 65-75 for Tauri dispatch reference)
- docs/plans/cli-completion/research-security.md (W-2 finding)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

1. **W-2 Security Mitigation (WARNING — must address):**
   Before spawning any `proton_run` or `native` command, ensure the log directory exists:

   ```rust
   if let Some(parent) = log_path.parent() {
       tokio::fs::create_dir_all(parent).await?;
   }
   ```

   Ideally, use `$XDG_RUNTIME_DIR` (with mode 0700) instead of `/tmp/crosshook-logs/` when the env var is set:

   ```rust
   fn launch_log_dir() -> PathBuf {
       std::env::var_os("XDG_RUNTIME_DIR")
           .map(PathBuf::from)
           .unwrap_or_else(|| PathBuf::from("/tmp"))
           .join("crosshook-logs")
   }
   ```

   Update `launch_log_path()` to use this helper. Add the `create_dir_all` call before the dispatch match block in `launch_profile()`.

2. **Add `proton_run` arm** to the dispatch match in `launch_profile()`:

   ```rust
   METHOD_PROTON_RUN => {
       let mut cmd = launch::script_runner::build_proton_game_command(&request, &log_path)?;
       cmd.stdout(Stdio::null()).stderr(Stdio::null());
       cmd.spawn()?
   }
   ```

   **CRITICAL**: `build_proton_game_command` returns `io::Result<Command>` (not `Command` directly) — must propagate with `?` before calling `.spawn()`.

3. **Log streaming** works identically for `proton_run` — the existing `stream_helper_log` / `drain_log` pattern reads from the same log file path. No changes needed to the post-spawn streaming loop.

4. **Post-launch analysis** via `analyze()` + `should_surface_report()` already applies to all methods — no changes needed.

#### Task 4.3: Wire `native` launch dispatch with C-1 helper script validation Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (`launch_profile()` function, `spawn_helper()`)
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs (`build_native_game_command` signature — returns `io::Result<Command>`)
- docs/plans/cli-completion/research-security.md (C-1 finding)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

1. **Add `native` arm** to the dispatch match in `launch_profile()`:

   ```rust
   METHOD_NATIVE => {
       let mut cmd = launch::script_runner::build_native_game_command(&request, &log_path)?;
       cmd.stdout(Stdio::null()).stderr(Stdio::null());
       cmd.spawn()?
   }
   ```

   Same pattern as `proton_run` — `build_native_game_command` returns `io::Result<Command>`.

2. **C-1 Security Mitigation (CRITICAL — do not skip):**
   In the `METHOD_STEAM_APPLAUNCH` arm (existing code), before calling `spawn_helper()`, validate the helper script at runtime:

   ```rust
   METHOD_STEAM_APPLAUNCH => {
       let helper = scripts_dir.join(HELPER_SCRIPT_NAME);
       // C-1 mitigation: validate helper script before execution
       let meta = std::fs::metadata(&helper)
           .map_err(|e| format!("helper script not found at {}: {e}", helper.display()))?;
       if !meta.is_file() {
           return Err(format!("helper script is not a regular file: {}", helper.display()).into());
       }
       #[cfg(unix)]
       {
           use std::os::unix::fs::MetadataExt;
           let uid = nix::unistd::getuid();
           if meta.uid() != uid.as_raw() {
               return Err(format!(
                   "helper script {} is not owned by current user (uid {})",
                   helper.display(), uid
               ).into());
           }
       }
       spawn_helper(&request, &helper, &log_path).await?
   }
   ```

   If `nix` is not already in `Cargo.toml`, use `libc::getuid()` via `unsafe` or skip the UID check with a TODO comment. The file-is-regular-file check is the minimum required mitigation.

3. **Add unsupported method fallback:**

   ```rust
   other => return Err(format!("unsupported launch method: {other}").into()),
   ```

### Phase 5: Polish and Production Readiness

#### Task 5.1: Standardize exit codes across all commands Depends on [4.2, 4.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (`main()` function, `run()`, `profile_store()` helper)
- docs/plans/cli-completion/research-ux.md (§Exit Codes)
- docs/plans/cli-completion/feature-spec.md (§Exit Codes)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

1. **Define exit code constants** at the top of `main.rs`:

   ```rust
   const EXIT_SUCCESS: i32 = 0;
   const EXIT_GENERAL_ERROR: i32 = 1;
   const EXIT_USAGE_ERROR: i32 = 2;
   const EXIT_PROFILE_NOT_FOUND: i32 = 3;
   const EXIT_LAUNCH_FAILURE: i32 = 4;
   const EXIT_STEAM_NOT_FOUND: i32 = 5;
   // Note: exit code 6 (--strict flag treating warnings as failure) is deferred to a follow-up
   ```

2. **Wrap `run()` errors** in a custom error enum or use pattern matching in `main()` to map specific error types to specific exit codes:
   - `ProfileStoreError::NotFound` → exit 3
   - Launch validation or spawn failure → exit 4
   - Clap parse errors → exit 2 (clap handles this by default)
   - All other errors → exit 1

3. **Update `profile_store()` helper** to return `Result` instead of calling `process::exit(1)` directly, so the exit code logic is centralized in `main()`.

4. This is a single sweep across `main.rs` — touch all error paths to ensure consistent exit codes.

#### Task 5.2: Add `--dry-run` flag to launch command Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/args.rs (`LaunchCommand` struct)
- src/crosshook-native/crates/crosshook-cli/src/main.rs (`launch_profile()` function)
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs (look for `build_launch_preview` or similar preview function)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/args.rs
- src/crosshook-native/crates/crosshook-cli/src/main.rs

1. **Add `--dry-run` flag** to `LaunchCommand` in `args.rs`:

   ```rust
   /// Show what would be launched without executing
   #[arg(long)]
   pub dry_run: bool,
   ```

2. **Wire in `launch_profile()`**: After building the `LaunchRequest` and calling `validate()`, check `command.dry_run`:
   - If `true`: Call `build_launch_preview(&request)` from `crosshook_core::launch::preview` (confirmed to exist in `launch/preview.rs`, re-exported from `launch/mod.rs`) to generate the preview output.
   - JSON mode: Serialize the `LaunchRequest` directly.
   - Human mode: Print labeled fields (method, game path, trainer path, proton path, etc.).
   - Return early — do not spawn.

3. **Add parse test** in `args.rs` following the existing pattern:

   ```rust
   #[test]
   fn parses_launch_dry_run_flag() {
       let cli = Cli::try_parse_from(&["crosshook", "launch", "--profile", "test", "--dry-run"]).unwrap();
       // assert dry_run is true
   }
   ```

#### Task 5.3: Add shell completion generation via `clap_complete` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/args.rs (existing `Cli` and `Command` enum)
- src/crosshook-native/crates/crosshook-cli/Cargo.toml (dependencies)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/Cargo.toml
- src/crosshook-native/crates/crosshook-cli/src/args.rs
- src/crosshook-native/crates/crosshook-cli/src/main.rs

1. **Add dependency** to `Cargo.toml`:

   ```toml
   clap_complete = "4"
   ```

2. **Add `Completions` subcommand** to the `Command` enum in `args.rs`:

   ```rust
   /// Generate shell completions
   Completions {
       /// Shell to generate completions for (bash, zsh, fish, powershell)
       #[arg(value_enum)]
       shell: clap_complete::Shell,
   },
   ```

3. **Wire in `run()`** in `main.rs`:

   ```rust
   Command::Completions { shell } => {
       let mut cmd = Cli::command();
       clap_complete::generate(shell, &mut cmd, "crosshook", &mut std::io::stdout());
   }
   ```

4. **Add parse test** for the new subcommand.

#### Task 5.4: Documentation, dead code cleanup, and final test pass Depends on [4.2, 4.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (locate `emit_placeholder()` function)
- docs/getting-started/quickstart.md (existing content)
- docs/plans/cli-completion/feature-spec.md (§User Workflows for CLI examples)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs
- docs/getting-started/quickstart.md

1. **Delete `emit_placeholder()` function** from `main.rs`. After all Phase 1-4 tasks, this function has zero call sites and is dead code. Also delete any remaining `use` imports that were only needed by `emit_placeholder`.

2. **Delete `steam_launch_request_from_profile()`** if not already deleted in Task 4.1.

3. **Add CLI section to quickstart guide** — add a `## Using the CLI` section to `docs/getting-started/quickstart.md` with examples for all 7 commands:
   - `crosshook status`
   - `crosshook profile list`
   - `crosshook profile import --legacy-path <path>`
   - `crosshook profile export --profile <name>`
   - `crosshook steam discover`
   - `crosshook steam auto-populate --game-path <path>`
   - `crosshook launch --profile <name>`
     Include `--json` and `--verbose` flag documentation.

4. **Run test suites:**

   ```bash
   cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
   cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-cli
   ```

   Fix any failures.

5. **Run `cargo check`** to ensure no unused imports, dead code warnings, or compile errors.

## Advice

- **All changes live in two files** (`main.rs` + `args.rs`). Parallelism is bounded by function independence within `main.rs`, not by file count. Tasks that add different handler functions can run concurrently even though they modify the same file — they touch non-overlapping regions.
- **The `handle_diagnostics_command` at `main.rs:145-178` is the only reference implementation** — every new handler should be a structural copy of this pattern. Do not invent new patterns, shared helpers, or abstractions.
- **`discover_steam_libraries` import will cause the most compile errors** — it is NOT re-exported from `steam/mod.rs`. Always use `crosshook_core::steam::libraries::discover_steam_libraries`. This affects Tasks 1.3 and 3.1.
- **`export_community_profile` takes `&Path`, not `&ProfileStore`** — pass `store.base_path.as_path()`. The function constructs its own store internally. Getting this wrong produces a type error. Affects Task 2.2.
- **`build_proton_game_command` and `build_native_game_command` return `io::Result<Command>`** — unlike `build_helper_command` which returns `Command` directly. Must apply `?` before `.spawn()`. Affects Tasks 4.2 and 4.3.
- **Security mitigations are inlined into their parent tasks, not standalone** — C-1 is in Task 4.3, C-2 is in Task 2.1, W-2 is in Task 4.2, W-4 is in Task 2.2. This ensures they ship with the code they protect and cannot be accidentally skipped.
- **Phase 4 is the only phase with sequential dependencies** — Task 4.1 (builder refactor) must complete before 4.2 and 4.3. All other phases have fully independent tasks. Optimize total wall-clock time by starting Phases 1-3 simultaneously.
- **The `status` command must tolerate partial failures** — if `ProfileStore::try_new()` fails but Steam discovery succeeds, still emit the Steam section. Wrap each section in `match` and collect errors into a diagnostics vec.
- **All new handlers must be `async fn`** — the existing `handle_diagnostics_command` is sync, which is an anomaly. Do not replicate it. Consistent `async fn` handlers are the project convention.
- **JSON output types can be inline `#[derive(Serialize)]` structs** — no need to add types to `crosshook-core`. The CLI owns its output format independently.
- **`ProfileStore::load()` returns the effective profile** with `local_override` already merged — no additional caller-side merging is needed. This simplifies the `launch_request_from_profile()` builder.
- **Exit code standardization (Task 5.1) is deliberately in Phase 5** — doing it per-command during Phases 1-4 would create churn as the error handling pattern solidifies. A single sweep after all commands are wired is more efficient.
