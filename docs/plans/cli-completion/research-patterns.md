# Pattern Research: cli-completion

## Architectural Patterns

**Thin CLI Shell over crosshook-core**: `crosshook-cli` is a thin consumer of `crosshook-core`. All business logic lives in the library; the CLI is responsible only for argument parsing, store initialization, calling core functions, and formatting output. The diagnostics export command is the canonical reference for this pattern.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs` — `handle_diagnostics_command` shows the full pattern

**Store Initialization Pattern**: Both `ProfileStore` and `SettingsStore` follow the same constructor pattern:

- `try_new()` — returns `Result<Self, String>`, used in library code
- `new()` — panics on failure, convenience wrapper
- `with_base_path(path: PathBuf)` — for CLI testing and custom config dirs

The CLI helper `profile_store(profile_dir: Option<PathBuf>)` encapsulates this initialization, calling `ProfileStore::with_base_path` when `--config` is provided or `ProfileStore::try_new()` otherwise, exiting with `std::process::exit(1)` on failure.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs:189-197`

**Command Dispatch via Nested Match**: The main `run()` function dispatches to handler functions via `match cli.command`. Each subcommand group (Profile, Steam, Diagnostics) has its own handler function that performs a second `match` on the sub-command variant. Handlers are `async fn` even when they contain no async operations.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs:35-47`

**Launch Method Dispatch Pattern**: The Tauri commands demonstrate the `proton_run`/`native`/`steam_applaunch` dispatch pattern. The CLI must mirror this: call `request.resolved_method()`, then match the constant strings `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE` to select the appropriate `build_*_command` function from `script_runner`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs:57-75`

**Profile-to-LaunchRequest Mapping**: The existing `steam_launch_request_from_profile` in main.rs maps `GameProfile` to `LaunchRequest`. For `proton_run` and `native`, the fields map from `profile.runtime.*` instead of `profile.steam.*`. The `resolve_launch_method` function in `profile/models.rs` handles method inference from profile state.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs:363-379`

---

## Code Conventions

**Naming**: `snake_case` for all Rust symbols. Handler functions named `handle_<group>_command` or `<action>_<noun>`. Argument struct names: `<Command>Args` for subcommand containers, `<Command>Command` for leaf-level args.

**Error Return Type**: CLI `main.rs` uses `Box<dyn Error>` throughout for simplicity. Core library functions use typed error enums (`ProfileStoreError`, `SettingsStoreError`, `CommunityExchangeError`). Conversion from typed errors to `Box<dyn Error>` is via the `?` operator since all core error types implement `std::error::Error`.

**JSON vs. Human Output**: The `GlobalOptions.json: bool` flag gates all output. When `--json` is set, emit structured JSON. Otherwise print human-readable lines. The diagnostics export command demonstrates the dual-output pattern:

```rust
if global.json {
    println!("{}", serde_json::to_string_pretty(&result)?);
} else {
    println!("Diagnostic bundle exported: {}", result.archive_path);
    // ...
}
```

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs:166-176`

**Verbose Logging**: `global.verbose` gates `eprintln!` of intermediate state. This is used before calling `emit_placeholder` to show what arguments were parsed. For real implementations, use `eprintln!` for verbose debug info.

**Clap Arg Definition Style**: Args use `#[arg(long = "kebab-case-name", value_name = "UPPERCASE")]`. Optional args typed as `Option<T>`. Hidden internal/testing args use `#[arg(long, hide = true)]`.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/args.rs`

**Subcommand Naming**: `#[command(rename_all = "kebab-case")]` on all `Subcommand` enums. This produces `profile list`, `steam auto-populate`, `diagnostics export`, etc.

---

## Error Handling

**Fail Fast at Boundary**: Errors from store initialization that cannot be recovered are handled with `eprintln!` + `std::process::exit(1)` (not `?`), because the `profile_store` helper is called in contexts where the return type is `Result<(), Box<dyn Error>>` but store failure is fatal. This pattern is used in the `profile_store` helper at line 189.

**Map Errors to String for Box<dyn Error>**: Core typed errors (e.g., `ProfileStoreError`) implement `Display` + `std::error::Error`, so they propagate via `?` into `Box<dyn Error>` without explicit mapping. When a more specific message is needed, use `.map_err(|e| format!("context: {e}"))?`.

**Validation Before Launch**: Call `launch::validate(&request)?` before spawning. Validation returns `Result<(), ValidationError>`, where `ValidationError` implements `Display`. The `ValidationError::issue()` method produces a structured `LaunchValidationIssue` for Tauri IPC; the CLI only needs the display string.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs:65`

**Log Tail Analysis After Exit**: After the child process exits, read the log tail and call `launch::analyze(Some(status), &log_tail, &request.method)`. If `launch::should_surface_report(&report)` is true, emit the summary and pattern matches to `eprintln!`. This post-hoc analysis is the existing steam_applaunch pattern and must be applied to proton_run as well.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs:73-95`

---

## Testing Approach

**Clap Parser Unit Tests**: All CLI argument parsing is tested with `Cli::try_parse_from(&[...])` in `args.rs`. Tests cover: global flags, subcommand variants, required args, optional args with defaults. This is the only test pattern in use for the CLI layer.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/args.rs:131-260`

**Core Unit Tests with `tempdir`**: Core library tests use `tempfile::tempdir()` to create isolated filesystem state. Tests exercise the actual file I/O through the store's public API (`save`, `load`, `list`, `delete`). No mocking.

- Example: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/legacy.rs:186-261`

**Test Organization**: Tests are in `#[cfg(test)] mod tests { ... }` inline within each source file. The `crosshook-core` crate uses `tempfile` for filesystem isolation. The `crosshook-cli/src/args.rs` tests only cover parsing, not execution.

**No CLI Integration Tests**: There are no integration tests for the CLI execution path (no `tests/` directory in `crosshook-cli`). New commands should add parse tests to `args.rs` following the existing pattern.

---

## Patterns to Follow

### Wiring a Placeholder Command

Use `handle_diagnostics_command` as the exact template:

1. Remove `emit_placeholder(global, "command name")` from the handler
2. Initialize required stores (use existing `profile_store()` helper for `ProfileStore`)
3. Build the options/request struct from CLI args
4. Call the `crosshook_core` function, propagating errors with `?`
5. Check `global.json` to choose output format
6. Add parse tests to `args.rs` for any new args

### Building a proton_run LaunchRequest from a Profile

The existing `steam_launch_request_from_profile` only handles `steam_applaunch`. For `proton_run`:

- Use `crosshook_core::profile::resolve_launch_method(&profile)` to determine method
- Map `profile.runtime.prefix_path` → `request.runtime.prefix_path`
- Map `profile.runtime.proton_path` → `request.runtime.proton_path`
- Map `profile.runtime.working_directory` → `request.runtime.working_directory`
- Set `request.method = METHOD_PROTON_RUN`
- Call `build_proton_game_command(&request, &log_path)?` from `script_runner`

### Building a native LaunchRequest from a Profile

For `native`:

- Use `profile.game.executable_path` → `request.game_path`
- Set `request.method = METHOD_NATIVE`
- Working directory from `profile.runtime.working_directory` or inferred from game path parent
- Call `build_native_game_command(&request, &log_path)?` from `script_runner`

### Profile List Command

```rust
// In handle_profile_command, ProfileCommand::List branch:
let store = profile_store(command.profile_dir.or_else(|| global.config.clone()));
let names = store.list().map_err(|e| format!("profile list: {e}"))?;
if global.json {
    println!("{}", serde_json::to_string_pretty(&names)?);
} else {
    for name in &names {
        println!("{name}");
    }
}
```

### Profile Import Command (Legacy .profile files)

```rust
// crosshook_core::profile::legacy provides load() for .profile files
use crosshook_core::profile::legacy;
let legacy_data = legacy::load(command.legacy_path.parent(), stem)?;
let profile = GameProfile::from(legacy_data);
store.save(&profile_name, &profile)?;
```

### Profile Export Command (Community JSON)

```rust
// crosshook_core::profile::exchange provides export_community_profile()
use crosshook_core::profile::exchange::export_community_profile;
let result = export_community_profile(&store.base_path, &profile_name, &output_path)?;
```

### Steam Discover Command

```rust
use crosshook_core::steam::{discover_steam_root_candidates, discover_compat_tools};
let mut diagnostics = Vec::new();
let roots = discover_steam_root_candidates("", &mut diagnostics);
// Emit roots as JSON array or one per line
```

### Steam Auto-Populate Command

```rust
use crosshook_core::steam::{attempt_auto_populate, SteamAutoPopulateRequest};
let request = SteamAutoPopulateRequest {
    game_path: command.game_path,
    steam_client_install_path: PathBuf::new(),
};
let result = attempt_auto_populate(&request);
// result.app_id, result.compatdata_path, result.proton_path
```

### Status Command

The `status` command has no subcommands. It should emit a summary of known profiles and settings. Pattern: initialize `ProfileStore` + `SettingsStore`, call `.list()` + `.load()`, then print or serialize.

### Log Streaming for proton_run / native

Unlike `steam_applaunch`, `build_proton_game_command` and `build_native_game_command` return `std::io::Result<Command>` (not `Command` directly) because they call `attach_log_stdio` internally. The CLI must:

1. Call `build_proton_game_command(&request, &log_path)?` to get the command
2. Spawn and stream the log with the existing `stream_helper_log` / `drain_log` helpers
3. The log path must be created before calling — use the existing `launch_log_path` helper

### Async vs. Sync Handlers

The CLI's `run()` function is `async`. All handler functions should be declared `async fn` for consistency even if they contain no `await` points. The diagnostics command handler is `fn` (not `async`) — this is inconsistent with the other handlers and is a gotcha to avoid when adding new handlers. New handlers should be `async fn`.

---

## Exit Codes

From `docs/plans/cli-completion/research-ux.md` (confirmed by docs-researcher):

| Code | Meaning                |
| ---- | ---------------------- |
| 0    | Success                |
| 1    | General error          |
| 2    | Usage error (bad args) |
| 3    | Profile not found      |
| 4    | Launch failure         |
| 5    | Steam not found        |

The current CLI only uses `std::process::exit(1)` for store init failure and lets `main()` emit any `Box<dyn Error>` to stderr before exiting 1. New commands should use `std::process::exit(3)` for `ProfileStoreError::NotFound` and `std::process::exit(5)` for Steam discovery failures rather than the generic exit 1.

## Error Message Format

Errors surfaced to the user should follow the two-line format:

```
error: <what failed>
  hint: <actionable suggestion>
```

This matches the UX spec. The `hint:` line is optional when no actionable suggestion exists. Emit both lines to `eprintln!`.

---

## Addenda from Integration Research

**ValidationError structured output**: `ValidationError` exposes `.message()`, `.help()`, and `.severity()` methods alongside `Display`. For `--json` output on launch validation failures, serialize via `.issue()` which returns a `LaunchValidationIssue { message, help, severity }` — all three fields are `serde`-derived. For plain text, `eprintln!("{error}")` via `Display` is sufficient.

**Diagnostics out-param**: `discover_steam_root_candidates(path, &mut Vec<String>)` collects diagnostic strings into a caller-owned `Vec`. Pattern: allocate `let mut diagnostics = Vec::new()` before calling, then gate emission on `global.verbose`:

```rust
let mut diagnostics = Vec::new();
let roots = discover_steam_root_candidates("", &mut diagnostics);
if global.verbose {
    for entry in &diagnostics {
        eprintln!("{entry}");
    }
}
```

Same pattern applies to `discover_steam_libraries` and `resolve_proton_path`.

**Legacy import name derivation**: The profile name for a legacy import is derived from the file stem of the `--legacy-path` argument, not from any field inside the file. Validate that the stem is a valid profile name before calling `store.save()` to surface a clear error rather than a cryptic I/O failure.
