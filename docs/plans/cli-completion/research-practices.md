# Engineering Practices Research: CLI Completion

## Executive Summary

The CLI binary already has a fully-wired `diagnostics export` command that is the reference pattern for all six placeholder commands. Every command follows the same three-step shape: construct a `ProfileStore` (or equivalent), call a `crosshook-core` function, format and print. No shared presentation layer is needed; the Tauri and CLI consumers diverge enough (IPC serialization vs. stdout) that the repetition is acceptable and expected.

## Existing Reusable Code

See the Relevant Files section below for the complete inventory of reusable code, including specific function names, line numbers, and how each should be consumed by CLI handlers.

## Modularity Design

The recommended module boundary is: keep all CLI handlers in `main.rs` (matching the existing pattern), with crosshook-core as the sole business logic provider. No new crates or shared presentation layers needed — the repetition between Tauri and CLI consumers is acceptable given their different output mechanisms.

## Relevant Files

- `src/crosshook-native/crates/crosshook-cli/src/main.rs` — CLI entry point; contains one fully-wired handler (`handle_diagnostics_command`) and five stubs. The `launch_profile` function is the other complete implementation and should be extended for `proton_run` / `native` methods.
- `src/crosshook-native/crates/crosshook-cli/src/args.rs` — All clap argument structs; argument definitions for every placeholder command already exist and compile.
- `src/crosshook-native/crates/crosshook-cli/Cargo.toml` — Thin dependency set: `clap`, `crosshook-core`, `serde_json`, `tokio`.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` — `ProfileStore::list()` (line 273), `ProfileStore::import_legacy()` (line 324); both return `Result<_, ProfileStoreError>`.
- `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs` — `export_community_profile(profiles_dir, profile_name, output_path)` (line 158); takes path arguments, not a store instance.
- `src/crosshook-native/crates/crosshook-core/src/steam/mod.rs` — Re-exports `discover_steam_root_candidates`, `attempt_auto_populate`, `discover_compat_tools`.
- `src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs` — `discover_steam_libraries(roots, diagnostics)` — NOT re-exported from `steam/mod.rs`; must be imported as `crosshook_core::steam::libraries::discover_steam_libraries`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` — `build_proton_game_command`, `build_native_game_command` (public, line 61 and 121).
- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` — Re-exports `validate`, `analyze`, `should_surface_report`, `METHOD_PROTON_RUN`, `METHOD_NATIVE`.
- `src/crosshook-native/src-tauri/src/commands/launch.rs` — Reference for proton_run / native launch: `match method` block at line 65.
- `src/crosshook-native/src-tauri/src/commands/diagnostics.rs` — Simplest Tauri command; shows the direct core-call pattern without Tauri state overhead.
- `src/crosshook-native/src-tauri/src/commands/shared.rs` — `sanitize_display_path`, `create_log_path`, `slugify_target`; these are Tauri-specific utilities. The CLI duplicates `launch_log_path` independently — a minor divergence to note.

## Architectural Patterns

- **Direct core call pattern**: Every handler creates its own store/config, calls a `crosshook-core` function, then formats output. No intermediate adapter layer exists or is needed. The `handle_diagnostics_command` handler is the canonical example.
- **`profile_store()` factory helper**: Already defined in `main.rs` (line 189). All profile commands must route through this function — it handles both the `--config` override path and the default XDG base path, exiting on failure.
- **`--json` output branch**: `GlobalOptions.json: bool` is propagated to every handler. The `emit_placeholder` function already models the branching pattern. Each real handler should follow the same: `if global.json { serde_json::to_string_pretty(&result)? } else { println!(...) }`.
- **`diagnostics: &mut Vec<String>` accumulator**: Steam discovery functions (`discover_steam_root_candidates`, `discover_steam_libraries`) collect diagnostic strings into a caller-owned `Vec<String>`. The CLI should print these with `eprintln!` when `global.verbose` is set, mirroring the Tauri handler pattern in `steam.rs:44`.
- **`safe_read_tail` duplication**: Both `main.rs` (line 344) and `commands/launch.rs` (line 533) define the same async file-tail function. This is low-value duplication — the CLI version is fine to keep in place since adding it to `crosshook-core` would pollute core with CLI I/O concerns.
- **Launch method dispatch**: Tauri's `launch_game` (line 57–75) uses a `match request.resolved_method()` block with three arms. The CLI `launch_profile` currently only handles `METHOD_STEAM_APPLAUNCH` and returns an error for others; extending it means adding the same match arms and calling `build_proton_game_command` / `build_native_game_command` directly.
- **Log streaming model**: The CLI polls a log file in a loop (`stream_helper_log`) rather than using Tauri events. This is the correct CLI approach — keep it. The same approach should be used when adding proton_run / native methods.

## Gotchas and Edge Cases

- **`discover_steam_libraries` is not in `steam::mod.rs`'s public re-exports**. The Tauri `steam.rs` command imports it directly as `crosshook_core::steam::libraries::discover_steam_libraries`. The CLI `steam discover` command must do the same. Calling `crosshook_core::steam::discover_steam_libraries` will fail to compile.
- **`export_community_profile` takes a `profiles_dir: &Path`, not a `ProfileStore`**. The CLI `profile export` handler must pass `store.base_path.as_path()` (via `profile_store(...).base_path`), not the store itself. The function creates an internal store using `ProfileStore::with_base_path`.
- **`steam auto-populate` requires a `SteamAutoPopulateRequest` struct** from `crosshook_core::steam::models`. The `game_path` argument from the CLI args maps to `request.game_path`, but `steam_client_install_path` must be resolved from env/filesystem — the CLI has `default_steam_roots()` and `resolve_steam_client_install_path()` already defined in `main.rs`.
- **`profile list` is synchronous** (`ProfileStore::list` is `fn`, not `async fn`). The handler is declared `async` but the core call needs no `.await` — this is consistent with how `handle_diagnostics_command` works (it is `fn`, not `async fn`). Mixing the two is fine.
- **`profile import` returns `GameProfile`**, not a name string. The CLI should print the imported profile name (derived from the `legacy_path` stem) and optionally the profile content as JSON.
- **The `STATUS` command has no existing core function for a system-level summary**. It must be assembled from: `ProfileStore::list()` for profile count, `discover_steam_root_candidates` for Steam presence, and optionally `SettingsStore::try_new()` for config path. There is no single `get_system_status()` function to call.
- **`launch_log_path` in `main.rs` (line 276) and `create_log_path` in Tauri `shared.rs` (line 5) differ**: the CLI version is deterministic (profile-name-based), Tauri uses timestamp + prefix. The CLI version is appropriate for the CLI context — no change needed.
- **`SteamAutoPopulateResult`**: `attempt_auto_populate` returns this struct synchronously. Tauri wraps it in `spawn_blocking` due to its runtime constraints. In `crosshook-cli`, it can be called directly since the tokio runtime can handle blocking calls within `spawn_blocking` or the call can be made inline if the function is not actually long-running.

## Interface Design Assessment

The `crosshook-core` public API surface is well-suited for CLI consumption:

| Command                        | Core function(s)                                              | Export path                                                                               |
| ------------------------------ | ------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `profile list`                 | `ProfileStore::list`                                          | `crosshook_core::profile::ProfileStore`                                                   |
| `profile import`               | `ProfileStore::import_legacy`                                 | same                                                                                      |
| `profile export`               | `export_community_profile`                                    | `crosshook_core::profile::export_community_profile`                                       |
| `steam discover`               | `discover_steam_root_candidates` + `discover_steam_libraries` | `crosshook_core::steam::*` + `crosshook_core::steam::libraries::discover_steam_libraries` |
| `steam auto-populate`          | `attempt_auto_populate`                                       | `crosshook_core::steam::attempt_auto_populate`                                            |
| `launch` (proton_run / native) | `build_proton_game_command`, `build_native_game_command`      | `crosshook_core::launch::script_runner::*`                                                |

All functions return `Result<T, E>` where `E: Display`. No changes to `crosshook-core` are needed to wire these commands.

## KISS Assessment

This is pure wiring work. The correct approach is:

1. Replace each `emit_placeholder` call with a direct call to the core function.
2. Format the result with `println!` (human) or `serde_json::to_string_pretty` (JSON).
3. Map errors with `.map_err(|e| e.to_string().into())` or early-return via `?`.

**Over-engineering risks to avoid:**

- Do not create a trait or abstraction for "CLI output formatting" — one handler per command, inline formatting, is sufficient.
- Do not create a shared `OutputFormatter` struct — the `global.json` branch is two lines per command.
- Do not add a new crate for the CLI — `crosshook-cli` is already the right home.
- Do not introduce `anyhow` or `thiserror` — `Box<dyn Error>` is already the error type in `run()` and is sufficient for CLI use.
- Do not pull in `tabled`, `comfy-table`, or similar crates for tabular output — `println!("{:<20} {}", name, value)` is sufficient for a list of profile names.

## Testability Patterns

The existing tests in `args.rs` (lines 131–261) use `Cli::try_parse_from([...])` for argument parsing. This pattern covers argument definition correctness without touching any I/O.

For handler logic testing, the existing pattern in `crosshook-core` is unit tests with temporary directories (`tempfile::tempdir()`). The CLI handlers should not be directly unit-tested — they are thin wiring functions. Instead:

- Test `crosshook-core` functions directly (tests already exist in `toml_store.rs`, `exchange.rs`, `script_runner.rs`).
- Add `#[cfg(test)]` argument parse tests in `args.rs` for any new args added to existing commands.
- For integration testing, the existing `cargo test -p crosshook-core` pattern is the right surface — the CLI handlers are too thin to warrant separate integration tests.

## Build vs. Depend Assessment

| Concern             | Recommendation                  | Rationale                                                                       |
| ------------------- | ------------------------------- | ------------------------------------------------------------------------------- |
| Table formatting    | `println!` with manual padding  | Profile list is a simple `Vec<String>`; no table crate needed                   |
| Color output        | None                            | Not requested; adds a dependency for no functional gain                         |
| JSON output         | `serde_json` (already a dep)    | Re-use existing dependency                                                      |
| Error display       | `Box<dyn Error>` (already used) | Consistent with existing `run()` signature                                      |
| Progress indicators | None                            | CLI is synchronous from user perspective; log streaming handles launch feedback |
| Async runtime       | `tokio` (already a dep)         | No change needed                                                                |

## Open Questions

1. Should `status` print Steam library paths in addition to profile count, or just a health summary? The Tauri `health.rs` command computes health scores from SQLite — the CLI has no `MetadataStore` wired. A simple version (profile count + Steam root detection) avoids needing the metadata store.
2. Should `profile export` default to stdout when `--output` is omitted, or require `--output`? The current arg definition makes `output: Option<PathBuf>`. `export_community_profile` requires an output path — the handler must either require the flag or construct a default path (e.g., `./profile-name.json`).
3. Should `steam discover` print all discovered Proton/compat tool versions in addition to Steam roots and libraries? The Tauri `list_proton_installs` command already does this via `discover_compat_tools`. Including it in `steam discover` output is consistent and requires no additional core calls.
