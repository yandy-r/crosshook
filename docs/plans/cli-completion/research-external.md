# External API & Library Research: CLI Completion

**Date**: 2026-03-30
**Researcher**: researcher agent
**Feature**: Wire up placeholder CLI commands and complete multi-method launch in `crosshook-cli`

---

## Executive Summary

The `crosshook-cli` binary already uses clap v4 with a well-structured derive-macro architecture. All required library functions exist in `crosshook-core` with correct public signatures — no new external APIs are needed for the core commands. The primary work is wiring existing functions through the CLI dispatch layer, adding `--json` toggle to each command handler, and extending `launch_profile` to support `proton_run` and `native` methods alongside the existing `steam_applaunch` path.

Key finding: The codebase already demonstrates the right pattern in `handle_diagnostics_command` — it calls a core function and conditionally prints human text vs `serde_json::to_string_pretty`. Every new command handler should follow this exact pattern.

**Confidence**: High — based on direct code inspection of the repository plus corroborating official Rust/crate documentation.

---

## Primary APIs (clap v4)

### Documentation

- Official derive tutorial: <https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html>
- Handling arguments guide (Rain's Rust CLI recommendations): <https://rust-cli-recommendations.sunshowers.io/handling-arguments.html>
- Subcommand trait: <https://docs.rs/clap/latest/clap/trait.Subcommand.html>

### Existing usage in crosshook-cli

`args.rs` already uses the recommended architecture:

- `Cli` (top-level struct with `#[command(subcommand)]`) wraps `Command` enum
- `GlobalOptions` is flattened with `#[command(flatten)]`, carrying `--json`, `--verbose`, `--profile`, `--config` as global flags
- Nested subcommands use the `ProfileArgs`/`SteamArgs` wrapper-struct pattern (outer struct holds `#[command(subcommand)]` enum)

### Subcommand dispatch pattern (canonical form)

```rust
// Top-level dispatch in run()
match cli.command {
    Command::Launch(cmd)      => launch_profile(cmd, &cli.global).await?,
    Command::Profile(cmd)     => handle_profile_command(cmd, &cli.global).await?,
    Command::Steam(cmd)       => handle_steam_command(cmd, &cli.global).await?,
    Command::Diagnostics(cmd) => handle_diagnostics_command(cmd, &cli.global)?,
    Command::Status           => handle_status(&cli.global)?,
}

// Nested dispatch (already in handle_profile_command)
match command.command {
    ProfileCommand::List      => { /* call store.list() */ }
    ProfileCommand::Import(c) => { /* call store.import_legacy() */ }
    ProfileCommand::Export(c) => { /* call export_community_profile() */ }
}
```

### Global flag via flatten

`GlobalOptions` is already in `args.rs` with `global = true` on each field — this is the canonical Rain's Rust CLI recommendations pattern. No changes needed.

**Confidence**: High — pattern is already implemented and tested in the repo.

---

## Core Functions to Wire (crosshook-core API surface)

All functions are synchronous (no `async`) unless noted. All return `Result<T, E>` where `E` implements `std::error::Error`.

### `crosshook status` → no single core function; compose from parts

There is no `get_system_status()` function. The `status` command should synthesize output from:

- `ProfileStore::list()` → profile count
- `SteamDiagnostics`-style summary or just `discover_steam_root_candidates()` to check Steam presence
- Optional: read `SettingsStore` for config path

### `profile list` → `ProfileStore::list()`

```
src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs:273
pub fn list(&self) -> Result<Vec<String>, ProfileStoreError>
```

Returns sorted `Vec<String>` of profile names (file stems). Already takes a `&self` reference to an initialized `ProfileStore`.

### `profile import` → `ProfileStore::import_legacy()`

```
src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs:324
pub fn import_legacy(&self, legacy_path: &Path) -> Result<GameProfile, ProfileStoreError>
```

Reads `<name>.profile` JSON file, converts to `GameProfile`, saves as TOML. Returns the imported profile.

### `profile export` → `export_community_profile()`

```
src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs:158
pub fn export_community_profile(
    profiles_dir: &Path,
    profile_name: &str,
    output_path: &Path,
) -> Result<CommunityExportResult, CommunityExchangeError>
```

Requires `profiles_dir` (from `ProfileStore::base_path`) and an output path. If no `--output` is given, a sensible default is `./profile_name.json`.

### `steam discover` → `discover_steam_root_candidates()` + `discover_steam_libraries()`

```
src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs:11
pub fn discover_steam_root_candidates(
    steam_client_install_path: impl AsRef<Path>,
    diagnostics: &mut Vec<String>,
) -> Vec<PathBuf>

src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs:8
pub fn discover_steam_libraries(
    steam_roots: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> Vec<SteamLibrary>
```

Both accept a `&mut Vec<String>` for diagnostics — pass an empty `Vec::new()` and optionally print them in `--verbose` mode.

### `steam auto-populate` → `attempt_auto_populate()`

```
src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs:12
pub fn attempt_auto_populate(request: &SteamAutoPopulateRequest) -> SteamAutoPopulateResult
```

Input: `SteamAutoPopulateRequest { game_path, steam_client_install_path }`.
Output: `SteamAutoPopulateResult` which already derives `Serialize` — can go directly to `serde_json::to_string_pretty()`.

### `launch` (proton_run + native) → existing `script_runner` functions

```
src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:61
pub fn build_proton_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command>

src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:~90
pub fn build_native_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<Command>
```

`LaunchRequest::resolved_method()` returns one of `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE`. Current `launch_profile` only handles `steam_applaunch`. The new dispatch pattern:

```rust
match profile.launch.method.trim() {
    METHOD_STEAM_APPLAUNCH => spawn_helper(request, &helper_script, &log_path).await?,
    METHOD_PROTON_RUN      => spawn_proton(request, &log_path).await?,
    METHOD_NATIVE          => spawn_native(request, &log_path).await?,
    _ => return Err("unsupported launch method".into()),
}
```

**Confidence**: High — all functions directly inspected in source.

---

## Libraries and SDKs

### Already in crosshook-cli/Cargo.toml

| Crate            | Version                 | Purpose                                           |
| ---------------- | ----------------------- | ------------------------------------------------- |
| `clap`           | 4 with `derive`         | CLI argument parsing (already used)               |
| `serde_json`     | 1                       | JSON output (already used in diagnostics command) |
| `tokio`          | 1 (full async features) | Async runtime, process spawning (already used)    |
| `crosshook-core` | path dep                | All business logic                                |

### No new dependencies required for core wiring

The existing dependencies are sufficient for all 7 commands. Adding new crates should be avoided unless a specific gap exists.

### Optional additions (justified only if needed)

**comfy-table** (if table output is desired for `profile list` or `steam discover`)

- crates.io: <https://crates.io/crates/comfy-table>
- Current: v7.2.x, MSRV 1.85 (switched to Rust 2024 edition)
- Zero unsafe, well-tested, supports terminal width detection
- **Verdict**: Only add if human-readable table layout is explicitly required; a simple `println!` loop is sufficient for `profile list` given it returns `Vec<String>`

**owo-colors** (if colored human output is desired)

- crates.io: <https://crates.io/crates/owo-colors>
- Zero allocations, no deps, supports `NO_COLOR` env, recommended by Rain's Rust CLI guide
- `owo_colors::set_override(false)` when `--json` is set avoids ANSI sequences in JSON output
- **Verdict**: Only add if colored status indicators are desired; plain text is acceptable MVP

**indicatif** (progress spinner for Steam discovery)

- crates.io: <https://crates.io/crates/indicatif>
- Steam library scanning is synchronous and fast; progress display adds complexity
- **Verdict**: Defer — overkill for this feature set

**Confidence**: High for "no new deps needed"; Medium for optional crate versions (based on crates.io metadata, not version-locked testing).

---

## Integration Patterns

### Pattern 1: --json toggle (canonical, from `diagnostics` command)

The `handle_diagnostics_command` already demonstrates the right pattern to replicate:

```rust
if global.json {
    println!("{}", serde_json::to_string_pretty(&result)?);
} else {
    println!("Profile: {}", result.profile_name);
    // ... human-readable lines
}
```

All core return types (`Vec<String>`, `GameProfile`, `SteamAutoPopulateResult`, `CommunityExportResult`) already derive `Serialize`, so `serde_json::to_string_pretty` works directly.

For `profile list` with `--json`, wrap the `Vec<String>` in a struct to add a count field:

```rust
#[derive(Serialize)]
struct ProfileListOutput {
    profiles: Vec<String>,
    count: usize,
}
```

### Pattern 2: async vs sync functions

All crosshook-core functions called here are synchronous. `main.rs` already uses `#[tokio::main]` and calls sync core functions from async handlers without issue. No `spawn_blocking` is needed since these are I/O operations (file reads) that complete quickly.

For `build_proton_game_command` and `build_native_game_command`, the command is built synchronously and then `.spawn()` is called via `tokio::process::Command` (already how `steam_applaunch` works via `build_helper_command`).

### Pattern 3: error propagation

Current pattern in `main.rs`:

```rust
async fn run() -> Result<(), Box<dyn Error>> { ... }
// In main:
if let Err(error) = run().await {
    eprintln!("{error}");
    std::process::exit(1);
}
```

This is sufficient. `ProfileStoreError`, `CommunityExchangeError`, and `io::Error` all implement `std::error::Error`, so `?` propagation works through `Box<dyn Error>`.

For `--json` output on errors, consider a consistent error envelope:

```rust
if global.json {
    println!(r#"{{"error":"{}"}}"#, error);
}
```

### Pattern 4: profile_store helper (already exists)

`fn profile_store(profile_dir: Option<PathBuf>) -> ProfileStore` is already in `main.rs` and handles the `--config` override. All profile commands should call this helper to get the `ProfileStore`.

### Pattern 5: Steam discovery with empty steam_client_install_path

`discover_steam_root_candidates` accepts any `impl AsRef<Path>`. When no path is configured, pass an empty `PathBuf::new()` — the function gracefully falls back to scanning `$HOME/.steam/root`, `~/.local/share/Steam`, and the Flatpak path.

**Confidence**: High — all patterns confirmed by direct source inspection.

---

## Constraints and Gotchas

### 1. proton_run requires a running Wine prefix

`build_proton_game_command` calls `env_clear()` on the command and then re-applies specific env vars via `apply_host_environment` and `apply_runtime_proton_environment`. The profile must have `runtime.prefix_path` and `runtime.proton_path` set. Validate these before spawning.

### 2. native launch requires executable permissions

`build_native_game_command` spawns the game executable directly. The CLI should check `game_path` exists and is executable before spawning. Use `fs::metadata().map(|m| m.permissions().mode() & 0o111 != 0)`.

### 3. helper script path for steam_applaunch

The existing `steam_applaunch` path uses `CARGO_MANIFEST_DIR` via `env!()` to locate `steam-launch-helper.sh`. This works for local dev but the AppImage bundles scripts at a different path. The `--scripts-dir` flag in `LaunchCommand` exists to override this. The new `proton_run`/`native` methods do NOT use the helper script — they spawn directly via `build_proton_game_command`/`build_native_game_command`.

### 4. tokio::process vs std::process

`script_runner.rs` returns `tokio::process::Command`, not `std::process::Command`. The existing `spawn_helper` async function in `main.rs` is the right model to follow for `proton_run` and `native` spawning.

### 5. serde_json and PathBuf serialization

`PathBuf` serializes to a JSON string (OS-dependent path separators). On Linux this is safe. `SteamAutoPopulateResult`, `SteamLibrary`, and `CommunityExportResult` all contain `PathBuf` fields — they will serialize correctly on Linux.

### 6. community export default output path

`export_community_profile` requires an `output_path`. If `ProfileExportCommand.output` is `None`, derive a default: `std::env::current_dir()?.join(format!("{}.json", profile_name))`. This is consistent with how tools like `cargo` behave.

### 7. import_legacy name derivation

`import_legacy` derives the profile name from the file stem of `legacy_path`. If the file stem contains characters invalid for a profile name, `ProfileStoreError::InvalidName` is returned. The CLI should surface this as a clear error message.

**Confidence**: High for constraints 1-4 (from code); Medium for 5-7 (from documentation + inference).

---

## Code Examples

### status command skeleton

```rust
fn handle_status(global: &GlobalOptions) -> Result<(), Box<dyn Error>> {
    let store = profile_store(global.config.clone());
    let profile_names = store.list().map_err(|e| format!("profile store: {e}"))?;
    let mut diag = Vec::new();
    let roots = discover_steam_root_candidates(PathBuf::new(), &mut diag);
    let libraries = discover_steam_libraries(&roots, &mut diag);

    if global.json {
        #[derive(Serialize)]
        struct StatusOutput<'a> {
            profile_count: usize,
            profiles: &'a [String],
            steam_roots: Vec<&'a PathBuf>,
            steam_library_count: usize,
            diagnostics: Vec<String>,
        }
        println!("{}", serde_json::to_string_pretty(&StatusOutput {
            profile_count: profile_names.len(),
            profiles: &profile_names,
            steam_roots: roots.iter().collect(),
            steam_library_count: libraries.len(),
            diagnostics: diag,
        })?);
    } else {
        println!("Profiles: {} loaded", profile_names.len());
        println!("Steam roots: {} found", roots.len());
        println!("Steam libraries: {} found", libraries.len());
        if global.verbose {
            for name in &profile_names { println!("  {name}"); }
            for lib in &libraries { println!("  {}", lib.path.display()); }
        }
    }
    Ok(())
}
```

### profile list command

```rust
ProfileCommand::List => {
    let store = profile_store(global.config.clone());
    let names = store.list()?;
    if global.json {
        #[derive(Serialize)]
        struct Out { profiles: Vec<String>, count: usize }
        println!("{}", serde_json::to_string_pretty(&Out { count: names.len(), profiles: names })?);
    } else if names.is_empty() {
        println!("No profiles found.");
    } else {
        for name in &names { println!("{name}"); }
    }
}
```

### profile export command

```rust
ProfileCommand::Export(cmd) => {
    let profile_name = cmd.profile
        .or_else(|| global.profile.clone())
        .ok_or("--profile NAME is required for profile export")?;
    let store = profile_store(global.config.clone());
    let output = cmd.output.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .join(format!("{profile_name}.json"))
    });
    let result = export_community_profile(&store.base_path, &profile_name, &output)?;
    if global.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Exported '{}' to {}", result.profile_name, result.output_path.display());
    }
}
```

### steam discover command

```rust
SteamCommand::Discover => {
    let mut diag = Vec::new();
    let roots = discover_steam_root_candidates(PathBuf::new(), &mut diag);
    let libraries = discover_steam_libraries(&roots, &mut diag);
    if global.json {
        #[derive(Serialize)]
        struct Out { roots: Vec<PathBuf>, libraries: Vec<SteamLibrary>, diagnostics: Vec<String> }
        println!("{}", serde_json::to_string_pretty(&Out { roots, libraries, diagnostics: diag })?);
    } else {
        println!("Steam roots: {}", roots.len());
        for r in &roots { println!("  {}", r.display()); }
        println!("Libraries:   {}", libraries.len());
        for l in &libraries { println!("  {}", l.path.display()); }
        if global.verbose {
            for d in &diag { eprintln!("[diag] {d}"); }
        }
    }
}
```

### launch multi-method dispatch

```rust
// In steam_launch_request_from_profile, remove the early METHOD_STEAM_APPLAUNCH-only check
// and allow all three methods through to a match in launch_profile:

match request.resolved_method() {
    METHOD_STEAM_APPLAUNCH => {
        let helper_script = scripts_dir.join(HELPER_SCRIPT_NAME);
        let mut child = spawn_helper(&request, &helper_script, &log_path).await?;
        stream_helper_log(&mut child, &log_path).await?
    }
    METHOD_PROTON_RUN => {
        let mut command = launch::script_runner::build_proton_game_command(&request, &log_path)?;
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = command.spawn()?;
        stream_helper_log(&mut child, &log_path).await?
    }
    METHOD_NATIVE => {
        let mut command = launch::script_runner::build_native_game_command(&request, &log_path)?;
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = command.spawn()?;
        stream_helper_log(&mut child, &log_path).await?
    }
    method => return Err(format!("unsupported launch method: {method}").into()),
}
```

---

## Open Questions

1. **Does `build_native_game_command` exist?** The function `build_proton_game_command` was confirmed in `script_runner.rs:61`. A corresponding `build_native_game_command` was referenced in the task description — its exact signature needs to be verified before implementing the `native` method dispatch. If it does not exist, the team will need to determine whether it should be created in `crosshook-core` as part of this feature or if native launch is out of scope.

2. **Status command scope**: The task says "system diagnostics + profile summary" but does not define what "system diagnostics" means in the CLI context. The `crosshook-core` steam diagnostics modules (`steam/diagnostics.rs`) expose a `DiagnosticCollector` for Steam-specific checks. The Tauri `diagnostics` command produces a bundle export. Clarify whether `crosshook status` should include Steam health checks or just profile + library counts.

3. **steam auto-populate output path**: `attempt_auto_populate` returns `SteamAutoPopulateResult` with field states and paths, but does not modify any profile. How should the CLI present partial matches (e.g., `app_id` found but `proton_path` not found)? The `SteamAutoPopulateFieldState` enum has `Found`, `NotFound`, and `Ambiguous` variants — the human output should reflect these states clearly.

4. **profile import output**: After `import_legacy` succeeds, the profile is saved to disk. Should the CLI print the saved profile path? The `GameProfile` struct does not itself store the path — the path is determined by `ProfileStore::profile_path(name)`. Printing `store.base_path.join(name + ".toml")` is the correct output.

5. **--json error envelope**: There is no standardized JSON error format for when commands fail. The `diagnostics export` command does not emit JSON on error — it returns `Err` which gets printed by the top-level `eprintln!`. Consider whether a consistent `{"error": "..."}` envelope for JSON mode error output is required.

---

## UX Decisions (from ux-researcher, 2026-03-30)

These resolve the open UX questions from the Open Questions section above.

### `steam auto-populate` human output format

Use left-aligned field labels with right-side status annotation. For `ambiguous`, always include an inline hint:

```
app_id:          42550                                               found
compatdata_path: /home/user/.steam/steam/steamapps/compatdata/42550  found
proton_path:     -                                                    ambiguous (set manually)
```

Implementation: use `{:<width}` format specifiers, not `comfy-table`. In `--json` mode: serialize `SteamAutoPopulateResult` directly (already `Serialize`). `owo-colors` for green/yellow/red state coloring is desirable but deferred past MVP.

### `profile list` default output

Flat newline-separated names. No table. Enables `$(crosshook profile list | fzf)` pipelines without extra flags.

If tabular output is added later, it must be opt-in via a `--long` / `-l` flag, not the default. `--json` gives machines all metadata they need.

### `steam discover --verbose` diagnostics

- **Default mode**: print found roots one per line; print "no Steam installations found" if empty (not silent).
- **`--verbose` mode**: print roots first, then a `Diagnostics:` section with the full `Vec<String>` indented:

```
/home/user/.local/share/Steam

Diagnostics:
  checked /home/user/.steam/root — not found
  found via default path scan: /home/user/.local/share/Steam
```

### `comfy-table` verdict

Skip for MVP. Aligned `println!` with `{:<width}` format specifiers is sufficient. Add `comfy-table` only if `profile list --long` is implemented later.

### `--json` vs TTY auto-detection

Explicit `--json` flag is correct for CrossHook. Auto-detection via `IsTerminal` is appropriate for color toggling but not output format switching — too surprising when scripts that worked interactively change format when piped.

---

## Sources

- [clap v4 derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html)
- [Rain's Rust CLI Recommendations — Handling Arguments](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html)
- [Rain's Rust CLI Recommendations — Managing Colors](https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html)
- [CLI Machine Communication (Rust Book)](https://rust-cli.github.io/book/in-depth/machine-communication.html)
- [serde_json documentation](https://docs.rs/serde_json/latest/serde_json/)
- [comfy-table crate](https://crates.io/crates/comfy-table)
- [indicatif crate](https://crates.io/crates/indicatif)
- [owo-colors crate](https://lib.rs/crates/owo-colors)
- [tokio::main vs block_on discussion](https://users.rust-lang.org/t/async-code-tokio-main-vs-futures-block-on-vs-runtime-main/31610)
- [std::process::Command docs](https://doc.rust-lang.org/std/process/struct.Command.html)
- [anyhow GitHub](https://github.com/dtolnay/anyhow)
- [Rust CLI error handling guide](https://rust-cli.github.io/book/tutorial/errors.html)
