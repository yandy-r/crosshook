# CLI Completion: Recommendations, Improvements, and Risk Assessment

## Executive Summary

The CLI completion feature is well-positioned for rapid delivery. The `crosshook-core` crate provides all necessary business logic with no Tauri dependencies, and the existing `diagnostics export` command establishes a proven wiring pattern. The 6 placeholder commands are straightforward to wire. The most complex work is extending the launch command to support `proton_run` and `native` methods, which requires refactoring the CLI's profile-to-request mapping but no new core logic. The primary risks are MetadataStore integration gaps and launch method edge cases around Proton prefix resolution.

---

## Implementation Recommendations

### Approach: Direct Core Calls (Thin Wrapper Pattern)

Every CLI command should follow the pattern already established by `diagnostics export`:

1. Instantiate the required store(s) (`ProfileStore`, `SettingsStore`)
2. Call `crosshook-core` functions directly
3. Branch on `global.json` for output formatting

**Why this approach**: The Tauri command layer (`src-tauri/src/commands/`) depends on `AppHandle`, `State<>`, and event emission. It cannot be shared with the CLI. The core crate is already cleanly separated and exposes all needed public APIs.

**Reference implementation**: `crates/crosshook-cli/src/main.rs:145-177` (`handle_diagnostics_command`)

### Technology Choices

| Dependency          | Purpose                            | Status                   |
| ------------------- | ---------------------------------- | ------------------------ |
| `clap` (v4, derive) | Arg parsing + help generation      | Already in use           |
| `serde_json`        | `--json` output                    | Already in use           |
| `crosshook-core`    | All business logic                 | Already in use           |
| `tokio`             | Async runtime for launch/steam ops | Already in use           |
| `clap_complete`     | Shell completion generation        | **Recommended addition** |

No new dependencies are required for the core wiring work. `clap_complete` is optional and can be added later for shell completion scripts.

### Phasing Strategy

**Phase 1: Simple reads (2 commands)**

- `crosshook profile list` -- Calls `ProfileStore::list()`, returns sorted names
- `crosshook status` -- Calls `batch_check_health()`, `discover_steam_root_candidates()`, and `ProfileStore::list()` for summary

**Why first**: These are read-only, have no side effects, and establish the output formatting pattern for all subsequent commands. They also provide immediate value for scripting and CI.

**Phase 2: Import/Export (2 commands)**

- `crosshook profile import` -- Calls `ProfileStore::import_legacy()` from `toml_store.rs:324`
- `crosshook profile export` -- Calls `export_community_profile()` from `exchange.rs:158`

**Why second**: These are write operations but use well-tested core functions. Legacy import is important for migration users.

**Phase 3: Steam discovery (2 commands)**

- `crosshook steam discover` -- Calls `discover_steam_root_candidates()` + `discover_steam_libraries()` + `discover_compat_tools()`
- `crosshook steam auto-populate` -- Calls `attempt_auto_populate()` with `SteamAutoPopulateRequest`

**Why third**: These depend on filesystem state and produce rich diagnostic output. The auto-populate result has multiple field states (Found/NotFound/Ambiguous) that need thoughtful human-readable formatting.

**Phase 4: Launch completion (1 command, most complex)**

- Refactor `steam_launch_request_from_profile()` into a generic `launch_request_from_profile()` that handles all three methods
- Wire `proton_run` via `build_proton_game_command()` / `build_proton_trainer_command()`
- Wire `native` via `build_native_game_command()`
- Add `--dry-run` flag using `build_launch_preview()`
- Add `--method` override flag

**Why last**: This is the most complex command and benefits from patterns established in earlier phases.

### Quick Wins

1. **Remove `emit_placeholder()`**: Replace all 6 placeholder calls with real implementations. The function at `main.rs:180-187` should be deleted when all commands are wired.

2. **Clarify `resolve_steam_client_install_path()` usage**: The CLI version at `main.rs:236-256` and the core version at `runtime_helpers.rs:166-191` serve **different purposes**. The CLI version derives a Steam client path by walking up from a `compatdata_path` looking for `steam.sh`. The core version resolves from an explicit configured path or env/filesystem fallback. These are not interchangeable. When building `LaunchRequest` for `proton_run`, use the core version (`crosshook_core::launch::runtime_helpers::resolve_steam_client_install_path()`). The CLI's ancestor-walking version remains useful for the `steam_applaunch` path where only compatdata is available.

3. **Deduplicate `safe_read_tail()`**: The CLI version at `main.rs:344-369` mirrors the Tauri version. Both can share the core implementation or remain as thin local helpers since they're straightforward.

4. **Add help text to all commands**: Currently only `DiagnosticsCommand::Export` has doc comments. Add `///` doc comments to all `Command` and `Subcommand` variants for `--help` output.

---

## Improvement Ideas

### Related Features Enabled by CLI Completion

1. **Shell Completion Scripts** -- `clap_complete` can generate Bash/Zsh/Fish completions from the `Cli` struct. Ship as `crosshook completions --shell zsh > _crosshook`. Minimal effort since clap does all the work.

2. **Man Page Generation** -- `clap_mangen` generates man pages from the same `Cli` struct. Ship alongside the AppImage or as a separate install step.

3. **CI Integration Templates** -- With `--json` output on all commands, provide example GitHub Actions workflows:
   - `crosshook status --json` for health checks in CI
   - `crosshook profile list --json` for automated profile validation
   - `crosshook steam discover --json` for environment verification

4. **Pipe-Friendly Output** -- When stdout is not a TTY, suppress progress indicators and emit machine-parseable output by default. This is a UX enhancement for scripting.

5. **`crosshook launch --dry-run`** -- The `build_launch_preview()` function in `preview.rs` already generates a complete launch preview without executing anything. Wire this as `--dry-run` to show the resolved command, environment, and validation without launching. Valuable for debugging and CI.

6. **Config File Support** -- The `--config` flag currently sets the profile store base path. A future enhancement could support a TOML config file for default options, eliminating repetitive flags in scripts.

### Future Enhancements

- **`crosshook profile show <name>`** -- Display a single profile's details (fields, health status, launch method)
- **`crosshook profile validate <name>`** -- Run health check on a single profile without launching
- **`crosshook launch --trainer-only`** / **`--game-only`** -- Already supported in `LaunchRequest` fields, just need CLI args
- **Exit code conventions** -- Standardize exit codes (0=success, 1=general error, 2=validation failure, 3=launch failure) for scripting
- **`crosshook version`** -- Already available via clap's `--version`, but a standalone `version` subcommand could include core library version and build info

---

## Risk Assessment

### Technical Risks

| Risk                                        | Likelihood | Impact | Mitigation                                                                                                                                                                                                                                                |
| ------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **proton_run prefix resolution edge cases** | Medium     | High   | The `resolve_wine_prefix_path()` heuristic checks for `pfx/` child directory. Standalone prefixes (no pfx/) resolve differently than Steam-managed ones. Test with both prefix styles.                                                                    |
| **Missing helper scripts at runtime**       | Medium     | Medium | `steam_applaunch` requires `steam-launch-helper.sh` from `runtime-helpers/`. The CLI resolves this via `CARGO_MANIFEST_DIR` (dev) or a `--scripts-dir` flag. AppImage distribution needs the script bundled alongside the binary.                         |
| **MetadataStore not available for status**  | High       | Medium | The `status` command wants launch history, failure trends, and health snapshots, all of which require `MetadataStore` (SQLite). Without it, `status` degrades to profile count + basic health. Decide upfront whether to initialize MetadataStore in CLI. |
| **No frontend test framework**              | Certain    | Low    | CLI changes don't affect the frontend. Rust tests cover all core logic. CLI-specific tests should focus on arg parsing and output formatting.                                                                                                             |
| **Launch log streaming on non-TTY**         | Low        | Medium | The current `stream_helper_log()` polls a log file every 500ms and writes to stdout. In CI/pipe contexts, this works but may produce partial lines. Consider buffering by newline.                                                                        |
| **Community export schema validation**      | Low        | Low    | `exchange.rs:219` validates schema version. If the schema version constant changes in core, CLI exports will follow automatically. No drift risk.                                                                                                         |

### Integration Challenges

1. **Profile-to-LaunchRequest mapping gap**: The current `steam_launch_request_from_profile()` at `main.rs:199-234` only handles `steam_applaunch`. For `proton_run`, the CLI must populate:
   - `request.runtime.prefix_path` from `profile.runtime.prefix_path` (or fall back to `profile.steam.compatdata_path`)
   - `request.runtime.proton_path` from `profile.runtime.proton_path`
   - `request.runtime.working_directory` from `profile.runtime.working_directory`
   - Optimization support via `profile.launch.optimization_ids` (if present in profile TOML)

   The Tauri frontend builds this mapping in JavaScript. The CLI needs to replicate it in Rust.

2. **Optimization directive resolution**: `resolve_launch_directives()` validates that wrapper binaries (mangohud, gamemoderun) exist on PATH. If they don't exist, the validation fails with an error. The CLI should handle this gracefully -- either skip optimizations or report which wrappers are missing.

3. **Steam discovery blocking I/O**: `discover_steam_root_candidates()` and `discover_steam_libraries()` do synchronous filesystem I/O. In the Tauri app, these are wrapped in `spawn_blocking`. The CLI already uses tokio's multi-threaded runtime, so blocking is acceptable for short-lived commands, but the `discover` command should note this.

### Backward Compatibility

- **Arg structure is stable**: The `args.rs` definitions are already published (clap parses them). Adding implementations to placeholders is purely additive.
- **`--json` output contracts**: Once JSON schemas ship, they become API contracts. Version the JSON output format or document that it's unstable during initial release.
- **Exit codes**: Currently the CLI uses `std::process::exit(1)` for all errors. Standardizing exit codes is a breaking change if anyone scripts against the current behavior (unlikely since commands are placeholders).

---

## Alternative Approaches

### Option A: Minimal Wiring (Recommended)

Each command handler directly instantiates stores and calls core functions inline. No shared state, no abstractions.

**Pros**: Simplest, fastest to deliver, mirrors existing diagnostics pattern, easy to review, each command is self-contained

**Cons**: Minor store initialization duplication across commands

**Effort**: Low (each command is 15-40 lines of Rust)

**Example** (profile list):

```rust
ProfileCommand::List => {
    let store = profile_store(global.config.clone());
    let names = store.list().map_err(|e| e.to_string())?;
    if global.json {
        println!("{}", serde_json::to_string_pretty(&names)?);
    } else {
        for name in &names {
            println!("{name}");
        }
    }
}
```

### Option B: Shared CLI Context

Create a `CliContext` struct initialized in `run()` that holds `ProfileStore` and optionally `MetadataStore`. Pass to all command handlers.

**Pros**: Single store initialization, cleaner function signatures, prepares for MetadataStore integration

**Cons**: Over-engineering for current scope, forces eager initialization of stores that some commands don't need, adds an abstraction layer

**Effort**: Medium

### Option C: Command Trait Abstraction

Define a `CliCommand` trait with `execute()` and `output()` methods. Each command implements the trait.

**Pros**: Clean separation of concerns, testable output formatting, extensible

**Cons**: Significant over-engineering for 7 commands, adds indirection, doesn't match existing codebase patterns, slower to deliver

**Effort**: High

### Recommendation

**Option A** for initial delivery. The diagnostics export pattern is proven and each command fits in a single function. If MetadataStore integration becomes needed for multiple commands later, refactor to Option B at that point.

---

## Task Breakdown Preview

### Phase 1: Foundation and Simple Reads

**Estimated complexity**: Low

| Task                                                                            | Dependencies | Parallelizable |
| ------------------------------------------------------------------------------- | ------------ | -------------- |
| Wire `profile list` command                                                     | None         | Yes            |
| Wire `status` command (basic: profile count + health summary + steam detection) | None         | Yes            |
| Add doc comments to all command variants for `--help`                           | None         | Yes            |

### Phase 2: Import/Export

**Estimated complexity**: Low-Medium

| Task                                                                    | Dependencies     | Parallelizable |
| ----------------------------------------------------------------------- | ---------------- | -------------- |
| Wire `profile import` (legacy) + add collision pre-check                | None             | Yes            |
| Wire `profile export` (community JSON) + add `--output` path validation | None             | Yes            |
| Add integration tests for import/export round-trip                      | Phase 2 commands | No             |

### Phase 3: Steam Discovery

**Estimated complexity**: Medium

| Task                                                                           | Dependencies     | Parallelizable |
| ------------------------------------------------------------------------------ | ---------------- | -------------- |
| Wire `steam discover` (roots + libraries + Proton installs)                    | None             | Yes            |
| Wire `steam auto-populate` + add optional `--steam-path` arg                   | None             | Yes            |
| Design human-readable output for discovery results (multi-field state display) | Phase 3 commands | No             |

### Phase 4: Launch Completion

**Estimated complexity**: High

| Task                                                                                        | Dependencies               | Parallelizable |
| ------------------------------------------------------------------------------------------- | -------------------------- | -------------- |
| Refactor `steam_launch_request_from_profile()` into generic `launch_request_from_profile()` | None                       | No             |
| Add `proton_run` launch path (reuses `build_proton_game_command()`)                         | Refactored request builder | No             |
| Add `native` launch path (reuses `build_native_game_command()`)                             | Refactored request builder | No             |
| Add `--dry-run` flag using `build_launch_preview()`                                         | Refactored request builder | Yes            |
| Add `--method` override flag                                                                | Arg changes                | Yes            |
| Add launch-related args (`--game-only`, `--trainer-only`, optimization IDs)                 | Arg changes                | Yes            |
| Test all three launch paths with validation                                                 | All launch tasks           | No             |

### Phase 5: Polish and Documentation

**Estimated complexity**: Low

| Task                                               | Dependencies | Parallelizable |
| -------------------------------------------------- | ------------ | -------------- |
| Standardize exit codes across all commands         | All phases   | No             |
| Update quickstart guide with CLI documentation     | All phases   | No             |
| Add shell completion generation (`clap_complete`)  | All phases   | Yes            |
| Remove `emit_placeholder()` function and dead code | All phases   | No             |
| Final `cargo test -p crosshook-cli` pass           | All phases   | No             |

---

## Key Decisions Needed

1. **MetadataStore in CLI**: Should the `status` command show launch history and failure trends? If yes, the CLI needs to initialize `MetadataStore` (SQLite), which adds complexity. If no, `status` shows only profile health and Steam detection -- still useful.

2. **Profile export format**: Should `profile export` generate community JSON (shareable, path-sanitized) or raw TOML (backup, machine-specific)? The community JSON format via `exchange.rs` is the more valuable option since TOML files can just be `cp`-ed.

3. **Launch optimization support**: Should the CLI `launch` command support `--optimization` flags for enabling Proton optimizations? The Tauri frontend manages these per-profile. The CLI could either read them from the profile's saved optimization preset or accept explicit flags.

4. **JSON output schema stability**: Are the `--json` output shapes considered stable API? If scripts will depend on them, version the schema or document field stability guarantees.

5. **Trainer launch from CLI**: The Tauri app has separate "Launch Game" and "Launch Trainer" buttons. Should the CLI support `--trainer-only` launch, or is game-only sufficient for v1?

---

## Open Questions

1. Is there an existing CLI quickstart guide that needs updating, or should a new one be created? (Acceptance criterion mentions "CLI documentation in quickstart guide")

2. Should `crosshook status` output match the Health Dashboard page exactly, or is a simplified summary acceptable for CLI?

3. For `steam auto-populate`, should the CLI save the auto-populated fields to the profile automatically, or just print what was discovered (like a dry-run)?

4. The `LaunchCommand` has hidden `--scripts-dir` for helper script resolution. In AppImage distribution, how will the helper scripts be located? This matters for the proton_run path which doesn't use helper scripts (direct `Command` execution) vs steam_applaunch which requires them.

5. Should the CLI record launch events to the MetadataStore (launch history, version snapshots, known-good tagging)? The Tauri app does this extensively. Skipping it means CLI launches are "invisible" to the health dashboard.

---

## Relevant Files

| File                                                  | Purpose                                                                                |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `crates/crosshook-cli/src/main.rs`                    | CLI entry point, all command handlers, launch implementation                           |
| `crates/crosshook-cli/src/args.rs`                    | Clap argument definitions and parser tests                                             |
| `crates/crosshook-cli/Cargo.toml`                     | CLI dependencies (clap, crosshook-core, serde_json, tokio)                             |
| `crates/crosshook-core/src/lib.rs`                    | Core module root (all public modules)                                                  |
| `crates/crosshook-core/src/profile/toml_store.rs`     | `ProfileStore` -- list, load, save, delete, import_legacy, rename, duplicate           |
| `crates/crosshook-core/src/profile/exchange.rs`       | Community profile import/export with schema validation                                 |
| `crates/crosshook-core/src/profile/legacy.rs`         | Legacy `.profile` format parser with path normalization                                |
| `crates/crosshook-core/src/profile/health.rs`         | `check_profile_health()`, `batch_check_health()`                                       |
| `crates/crosshook-core/src/launch/mod.rs`             | Launch module public API re-exports                                                    |
| `crates/crosshook-core/src/launch/request.rs`         | `LaunchRequest`, validation, `METHOD_*` constants                                      |
| `crates/crosshook-core/src/launch/script_runner.rs`   | `build_proton_game_command()`, `build_native_game_command()`, `build_helper_command()` |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | Environment setup, Proton path resolution, `resolve_steam_client_install_path()`       |
| `crates/crosshook-core/src/launch/preview.rs`         | `build_launch_preview()` for dry-run support                                           |
| `crates/crosshook-core/src/launch/optimizations.rs`   | `resolve_launch_directives()`, wrapper binary resolution                               |
| `crates/crosshook-core/src/steam/discovery.rs`        | `discover_steam_root_candidates()`                                                     |
| `crates/crosshook-core/src/steam/auto_populate.rs`    | `attempt_auto_populate()`                                                              |
| `crates/crosshook-core/src/steam/models.rs`           | `SteamAutoPopulateRequest/Result`, `ProtonInstall`, `SteamLibrary`                     |
| `crates/crosshook-core/src/steam/libraries.rs`        | `discover_steam_libraries()`                                                           |
| `crates/crosshook-core/src/steam/proton.rs`           | `discover_compat_tools()`, `resolve_proton_path()`                                     |
| `crates/crosshook-core/src/settings/mod.rs`           | `SettingsStore`, `AppSettingsData`                                                     |
| `crates/crosshook-core/src/export/diagnostics.rs`     | `export_diagnostic_bundle()` -- reference for complete command pattern                 |
| `src-tauri/src/commands/launch.rs`                    | Tauri launch command -- reference for all three launch methods                         |
| `src-tauri/src/commands/steam.rs`                     | Tauri steam commands -- reference for auto-populate wiring                             |
| `src-tauri/src/commands/health.rs`                    | Tauri health commands -- reference for enriched health data                            |
| `src-tauri/src/commands/profile.rs`                   | Tauri profile commands -- reference for profile CRUD patterns                          |
