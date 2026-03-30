# CLI Completion

CrossHook's CLI binary (`crosshook-cli`) has argument parsing for 7 commands but only `diagnostics export` and `launch` (steam_applaunch only) are wired to `crosshook-core` — the remaining 6 (`status`, `profile list`, `profile import`, `profile export`, `steam discover`, `steam auto-populate`) call `emit_placeholder()`. All business logic already exists in `crosshook-core`; the work is pure wiring following the established `handle_diagnostics_command` pattern, plus extending `launch` to support `proton_run` and `native` methods by refactoring `steam_launch_request_from_profile()` into a generic builder. Two CRITICAL security findings (helper script path validation, import path containment) must ship with mitigations alongside the wiring work.

## Relevant Files

- src/crosshook-native/crates/crosshook-cli/src/main.rs: CLI entry point with all command dispatch; contains 6 `emit_placeholder()` stubs, `steam_launch_request_from_profile()`, `launch_profile()`, `profile_store()` helper, and the reference `handle_diagnostics_command` pattern
- src/crosshook-native/crates/crosshook-cli/src/args.rs: Clap-based argument structs for all 7 commands; needs `///` doc comments on command variants for `--help` output
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore` — `list()`, `load()`, `save()`, `import_legacy()`, `try_new()`, `with_base_path()`
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs: `export_community_profile(profiles_dir, name, output_path)` — takes directory path, constructs its own store internally
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile`, section structs, `resolve_launch_method()` at line 363
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: `LaunchRequest`, `SteamLaunchConfig`, `RuntimeLaunchConfig`, `validate()`, `ValidationError`, method constants (`METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE`)
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: `build_helper_command()`, `build_proton_game_command()` (returns `io::Result<Command>`), `build_native_game_command()` (returns `io::Result<Command>`)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs: `analyze()`, `should_surface_report()` — post-launch log analysis
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs: `discover_steam_root_candidates(path, &mut diagnostics)` — locates Steam root dirs
- src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs: `discover_steam_libraries()` — NOT re-exported from `steam/mod.rs`; must import as `crosshook_core::steam::libraries::discover_steam_libraries`
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: `discover_compat_tools(roots, &mut diagnostics)` — enumerates Proton installs
- src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs: `attempt_auto_populate(&SteamAutoPopulateRequest)` — Steam library scan for a game path
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs: `SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `ProtonInstall`, `SteamAutoPopulateFieldState`
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `SettingsStore`, `AppSettingsData` — needed for `status` command
- src/crosshook-native/src-tauri/src/commands/launch.rs: Tauri reference for proton_run + native launch dispatch (lines 52-78)
- src/crosshook-native/src-tauri/src/commands/steam.rs: Tauri reference for steam discover / auto-populate wiring
- src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri reference for profile list/import/export wiring
- docs/getting-started/quickstart.md: Needs CLI section added (Phase 5 acceptance criterion)

## Relevant Patterns

**Thin CLI Shell (Reference: handle_diagnostics_command)**: Each handler initializes stores via `profile_store()` helper, calls the `crosshook-core` function, then branches on `global.json` for output formatting. See [main.rs:145-177](src/crosshook-native/crates/crosshook-cli/src/main.rs) for the canonical implementation.

**Dual Output (JSON vs Human)**: `global.json` flag gates all output. JSON mode emits `serde_json::to_string_pretty()` to stdout. Human mode prints plain-text lines. Errors go to stderr via `eprintln!`. See [main.rs:166-176](src/crosshook-native/crates/crosshook-cli/src/main.rs).

**Diagnostics Out-Param**: Steam discovery functions take `&mut Vec<String>` for diagnostics. Allocate before calling, gate emission on `global.verbose` via `eprintln!`. See `discover_steam_root_candidates`, `discover_compat_tools`, `discover_steam_libraries`.

**Profile-to-LaunchRequest Mapping**: Three distinct `LaunchRequest` shapes per method: `steam.*` fields for `steam_applaunch`, `runtime.*` fields from `profile.runtime` for `proton_run`, `game_path` only for `native`. Use `resolve_launch_method(&profile)` from `profile/models.rs` for method inference.

**Store Initialization**: `profile_store(profile_dir: Option<PathBuf>)` helper encapsulates `ProfileStore::with_base_path` when `--config` is provided or `ProfileStore::try_new()` otherwise. Exits with `process::exit(1)` on failure.

**Error Message Format**: `error: <what failed>\n  hint: <actionable suggestion>` to stderr. Error types implement `Display` + `std::error::Error`, propagate via `?` into `Box<dyn Error>`.

**Async Handler Convention**: All handlers should be `async fn` for consistency (existing `handle_diagnostics_command` is sync — do not follow that anomaly).

## Relevant Docs

**docs/plans/cli-completion/feature-spec.md**: You _must_ read this before implementation. Contains architecture diagram, all JSON output schemas, `launch_request_from_profile()` code template, security hard stops (C-1, C-2), and 5-phase task breakdown.

**docs/plans/cli-completion/research-technical.md**: You _must_ read this when implementing any command. Contains 12 documented gotchas that will cause compile errors if missed (especially: `discover_steam_libraries` NOT re-exported, `export_community_profile` takes `&Path` not `&ProfileStore`).

**docs/plans/cli-completion/research-security.md**: You _must_ read this when implementing Phase 4 (launch) and Phase 2 (import). Contains 2 CRITICAL findings: C-1 (helper script path validation) and C-2 (import path containment) that must ship with mitigations.

**docs/plans/cli-completion/research-external.md**: You _must_ read this when implementing Phase 4 (launch completion). Contains complete code skeletons for `launch_request_from_profile()` and all three method dispatch paths.

**docs/plans/cli-completion/research-patterns.md**: Reference for wiring patterns, error handling conventions, exit codes, and testing approach.

**docs/plans/cli-completion/research-integration.md**: Complete core function signatures, data models, filesystem paths, and edge cases. Critical reference for type-correct wiring.

**CLAUDE.md**: You _must_ read this for project conventions, commit message rules, and build commands.
