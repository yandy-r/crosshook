# External API Research: protontricks-integration

**Research date**: 2026-04-03
**Author**: researcher agent

---

## Executive Summary

Protontricks is a Python-based CLI wrapper around winetricks that auto-detects Steam Proton game prefixes by Steam App ID. Winetricks is the underlying shell script that downloads and installs Windows runtime dependencies (vcrun2019, dotnet48, d3dx9, corefonts, xact, etc.) into Wine prefixes. CrossHook can invoke protontricks via `tokio::process::Command`, passing the Steam App ID and verb list. Detecting already-installed packages requires setting `WINEPREFIX` and running `winetricks list-installed`, since protontricks itself does not expose a package-state query command. The main integration constraints are: (1) protontricks must be pre-installed and on `$PATH` or at a user-configured path; (2) Flatpak protontricks has sandbox restrictions that break multi-library setups; (3) winetricks downloads packages at runtime from Microsoft CDNs (network required); (4) bwrap containerization occasionally requires the `--no-bwrap` flag as a workaround.

**Confidence**: High — based on official GitHub documentation, man pages, and Arch Linux wiki.

---

## Primary APIs

### Protontricks CLI

**Source**: [Matoking/protontricks README](https://github.com/Matoking/protontricks/blob/master/README.md), [man page](https://linuxcommandlibrary.com/man/protontricks)

#### Command Syntax

```
protontricks [OPTIONS] <APPID> <WINETRICKS_VERBS...>
```

All winetricks verbs are passed through directly to winetricks.

#### Key Commands

| Command                                     | Purpose                                                      |
| ------------------------------------------- | ------------------------------------------------------------ |
| `protontricks <APPID> <VERB> [<VERB>...]`   | Run winetricks verb(s) against a specific Steam game prefix  |
| `protontricks -l`                           | List all installed Steam games with their App IDs            |
| `protontricks -s <NAME>`                    | Search for a game's App ID by name                           |
| `protontricks -c <CMD> <APPID>`             | Run an arbitrary shell command inside the game's Wine prefix |
| `protontricks --gui`                        | Launch the graphical interface                               |
| `protontricks --help`                       | Display help                                                 |
| `protontricks-launch <EXE>`                 | Launch a Windows executable using Proton                     |
| `protontricks-launch --appid <APPID> <EXE>` | Launch a Windows executable for a specific Steam app         |

#### Key Flags

| Flag               | Purpose                                                               |
| ------------------ | --------------------------------------------------------------------- |
| `--no-bwrap`       | Disable bubblewrap (bwrap) sandboxing — use when bwrap causes crashes |
| `-q, --unattended` | (via winetricks passthrough) Non-interactive install                  |
| `-h, --help`       | Display help                                                          |

#### Environment Variables

| Variable                 | Purpose                                                                 |
| ------------------------ | ----------------------------------------------------------------------- |
| `STEAM_DIR`              | Override Steam installation directory (default: `~/.local/share/Steam`) |
| `WINETRICKS`             | Path to a local winetricks script override                              |
| `PROTON_VERSION`         | Override Proton version (format: `"Proton X.Y"`)                        |
| `STEAM_COMPAT_DATA_PATH` | Custom Wine prefix path — sets `WINEPREFIX=$STEAM_COMPAT_DATA_PATH/pfx` |

#### Exit Codes

Protontricks does not document explicit exit codes beyond standard POSIX conventions:

- `0` — Success
- Non-zero — Failure (winetricks/Wine subprocess error, missing Steam app, permission errors)
- `141` — SIGPIPE (signal 13; `128+13`) — occurs when a downstream pipe closes unexpectedly during winetricks execution

**Confidence**: Medium — exit code 141 documented, others are inferred from POSIX convention and source behavior.

#### Prefix Path Convention

Steam game prefixes follow this pattern:

```
~/.local/share/Steam/steamapps/compatdata/<APPID>/pfx/
```

When `STEAM_COMPAT_DATA_PATH` is set:

```
$STEAM_COMPAT_DATA_PATH/pfx/
```

#### Python Internal API (not for public use, but informative)

Protontricks exposes Python functions internally that could be relevant for understanding its discovery logic:

- `protontricks.steam.find_appid_proton_prefix(appid)` — resolves a Steam App ID to its prefix path
- `protontricks.steam.find_steam_proton_app(appid)` — finds the Proton runner for an app
- `protontricks.steam.find_steam_path()` — locates the Steam installation directory

The `SteamApp` class has attributes: `appid`, `name`, `prefix_path`, `install_path`. CrossHook should not depend on this internal Python API since it's not part of any stable public ABI — use the CLI instead.

**Source**: [Snyk/protontricks Python functions](https://snyk.io/advisor/python/protontricks)

---

### Winetricks CLI

**Source**: [Winetricks man page (ManKier)](https://www.mankier.com/1/winetricks), [GitHub](https://github.com/Winetricks/winetricks), [Arch man page](https://man.archlinux.org/man/winetricks.1.en)

Winetricks is the underlying engine that protontricks wraps. CrossHook may also need to call it directly (with `WINEPREFIX` set) for prefix-state queries.

#### Package Detection — Detecting Installed Verbs

The critical command for state detection:

```bash
WINEPREFIX=/path/to/prefix winetricks list-installed
```

Output format: one verb per line, space-separated list of installed verbs. Example:

```
corefonts d3dx9 vcrun2019
```

Known limitation: `list-installed` does **not** list verbs from the `settings` category (GitHub Issue [#936](https://github.com/Winetricks/winetricks/issues/936)). For game trainer dependencies (vcrun2019, dotnet48, d3dx9, corefonts, xact) this is not a concern since they are all in `dlls`, `fonts`, or `apps` categories.

**Do not use `$WINEPREFIX/winetricks.log` as CrossHook's source of truth.** This file is maintained by winetricks internally with no stability guarantee on its format. Parsing tool-internal log files is fragile and breaks if winetricks changes its format. CrossHook should track installed-verb state in its SQLite metadata DB, updated via upsert on successful installation exit, and treat `list-installed` stdout output as a one-time sync/bootstrap mechanism — not the ongoing source of truth.

#### Key Verbs for Game Trainer Dependencies

| Verb        | Description                                         |
| ----------- | --------------------------------------------------- |
| `vcrun2019` | Visual C++ 2015-2019 redistributable runtime        |
| `dotnet48`  | .NET Framework 4.8 (installs 4.0 through 4.8)       |
| `d3dx9`     | DirectX 9 redistributable DLLs                      |
| `corefonts` | Core Microsoft fonts (Arial, Times New Roman, etc.) |
| `xact`      | Microsoft XACT (audio) runtime                      |
| `vcrun2022` | Visual C++ 2022 redistributable                     |
| `dotnet6`   | .NET 6 runtime                                      |
| `dxvk`      | DXVK Vulkan-based D3D9/10/11 implementation         |

Full verb list: [github.com/Winetricks/winetricks/blob/master/files/verbs/all.txt](https://github.com/Winetricks/winetricks/blob/master/files/verbs/all.txt)

#### Important Flags

| Flag               | Purpose                                                          |
| ------------------ | ---------------------------------------------------------------- |
| `-q, --unattended` | Non-interactive — suppress all prompts (required for automation) |
| `-f, --force`      | Force reinstall even if already installed                        |
| `-v, --verbose`    | Verbose output (echo all commands)                               |
| `--country=CC`     | Set country code (avoid IP-based detection on retries)           |

#### WINEPREFIX Targeting

```bash
# Target a specific prefix directly (bypassing protontricks):
WINEPREFIX=/path/to/steamapps/compatdata/<APPID>/pfx winetricks -q vcrun2019

# List installed verbs for a prefix:
WINEPREFIX=/path/to/prefix winetricks list-installed
```

#### Progress Output Format

Winetricks writes download progress to stderr via wget progress lines:

```
0K .......... .......... 0% 823K 40s
```

A `winetricks_parse_wget_progress()` function in the script transforms this to formatted output. When running unattended (`-q`), there is no GUI — raw wget progress goes to stderr, installation messages go to stdout.

**Confidence**: High — sourced from official man pages and source code review.

---

## Libraries and SDKs

### Rust Process Management

#### `tokio::process::Command` (primary choice)

**Source**: [docs.rs/tokio/latest/tokio/process](https://docs.rs/tokio/latest/tokio/process/struct.Command.html)

The canonical async subprocess API in Tokio. CrossHook already uses Tokio (Tauri v2 uses Tokio as its async runtime).

Key methods:

- `.output().await` — capture stdout + stderr as `Vec<u8>`, returns `Output { stdout, stderr, status }`
- `.status().await` — await exit status only
- `.spawn()` — non-blocking spawn; returns `Child` for streaming I/O
- `.env(key, val)` — set environment variable for the subprocess
- `.env_clear()` + `.envs(pairs)` — full environment control
- `.stdout(Stdio::piped())` / `.stderr(Stdio::piped())` — capture streams
- `.kill_on_drop(true)` — ensure child terminates if the `Child` is dropped

Example for running protontricks:

```rust
use tokio::process::Command;
use std::process::Stdio;

async fn run_protontricks(
    protontricks_path: &str,
    app_id: u32,
    verbs: &[&str],
    steam_dir: Option<&str>,
) -> anyhow::Result<std::process::Output> {
    let mut cmd = Command::new(protontricks_path);
    cmd.arg(app_id.to_string());
    cmd.arg("-q"); // unattended
    for verb in verbs {
        cmd.arg(verb);
    }
    if let Some(dir) = steam_dir {
        cmd.env("STEAM_DIR", dir);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);
    Ok(cmd.output().await?)
}
```

Example for querying installed packages:

```rust
async fn list_installed_verbs(
    winetricks_path: &str,
    prefix_path: &str,
) -> anyhow::Result<Vec<String>> {
    let output = Command::new(winetricks_path)
        .arg("list-installed")
        .env("WINEPREFIX", prefix_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.split_whitespace().map(String::from).collect())
}
```

**Confidence**: High — official Tokio documentation.

#### Binary detection — in-tree PATH walk (no external crate)

**Do not add the `which` crate.** `resolve_umu_run_path()` in `launch/runtime_helpers.rs:301-312` already implements the exact same behavior using only `std`: walk `$PATH` entries, check `is_executable_file()` (file exists + execute permission bit set). `resolve_protontricks_path()` and `resolve_winetricks_path()` should follow this exact pattern, and must use the same `DEFAULT_HOST_PATH` fallback to match the controlled-environment behavior of all other subprocess invocations in the codebase.

Adding `which` (even with zero transitive dependencies) contradicts the project convention and violates the "check existing dependencies before adding new ones" rule. The in-tree implementation is already more correct for this project.

```rust
// Pattern from launch/runtime_helpers.rs:302 — replicate, don't add which crate:
pub fn resolve_protontricks_path() -> Option<String> {
    let path_value = env::var_os("PATH")
        .unwrap_or_else(|| OsString::from(DEFAULT_HOST_PATH));
    for directory in env::split_paths(&path_value) {
        let candidate = directory.join("protontricks");
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

pub fn resolve_winetricks_path() -> Option<String> {
    let path_value = env::var_os("PATH")
        .unwrap_or_else(|| OsString::from(DEFAULT_HOST_PATH));
    for directory in env::split_paths(&path_value) {
        let candidate = directory.join("winetricks");
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}
```

**Confidence**: High — verified against actual source at `runtime_helpers.rs:301-312`.

#### `std::process::Command` (sync alternative)

For non-async blocking contexts, `std::process::Command` provides the same interface. Since CrossHook is Tauri/async, `tokio::process::Command` is preferred.

---

## Addendum: In-Tree Infrastructure (from practices-researcher)

_Added 2026-04-03 — verified against actual source files._

### No new crates needed

The following are **already available in crosshook-core** with no new dependencies:

| Tool                      | Status                | Source location                                          |
| ------------------------- | --------------------- | -------------------------------------------------------- |
| `tokio::process::Command` | Already imported      | `launch/runtime_helpers.rs:7`, `launch/script_runner.rs` |
| `std::process::Command`   | Already in tree       | `community/taps.rs:5`, `settings/mod.rs`                 |
| `rusqlite` (bundled)      | Already in Cargo.toml | Used throughout `metadata/`                              |

The `which` crate mentioned in the initial research is **not needed** — the project already has an in-tree binary discovery pattern (see below).

### Established factory patterns to follow

**`git_command()` in `community/taps.rs:503`** — exact template for a `winetricks_command()` factory:

```rust
// Existing pattern (community/taps.rs:503-509):
fn git_command() -> Command {
    let mut command = Command::new("git");
    for (key, value) in git_security_env_pairs() {
        command.env(key, value);
    }
    command
}
```

A `winetricks_command()` factory follows this pattern directly — swap `"git"` for the configured winetricks path and apply `WINEPREFIX` instead of git security env pairs.

**`build_install_command` in `install/service.rs:103-121`** — the authoritative three-step pattern for subprocess commands in crosshook-core:

```rust
// install/service.rs:108-112 — confirmed pattern:
let mut command = new_direct_proton_command(request.proton_path.trim()); // env_clear() inside
command.arg(request.installer_path.trim());
apply_host_environment(&mut command);          // HOME, PATH, DISPLAY, WAYLAND_DISPLAY, ...
apply_runtime_proton_environment(&mut command, &prefix_path_string, ""); // WINEPREFIX, STEAM_COMPAT_DATA_PATH
```

The winetricks/protontricks factory must follow this same three-step sequence:

1. `Command::new(binary_path)` + `env_clear()` — clean start, prevents host env leakage (LD_PRELOAD, secret vars)
2. `apply_host_environment(&mut cmd)` — restores exactly the vars winetricks needs: `HOME`, `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS` (confirmed at `runtime_helpers.rs:153-167`)
3. `apply_runtime_proton_environment(&mut cmd, prefix_path, "")` — sets `WINEPREFIX` + `STEAM_COMPAT_DATA_PATH`

**Do NOT use `new_direct_proton_command` directly** — it appends `"run"` as a positional argument (Proton-specific, not valid for winetricks/protontricks). Call `Command::new(path)` + `env_clear()` manually, then apply the same two environment functions.

**Correction from earlier note**: "do NOT call env_clear" was wrong. The correct rule is: **always call `env_clear()`, immediately followed by `apply_host_environment()`**. Skipping `env_clear()` risks leaking sensitive host environment variables into the subprocess.

**`apply_runtime_proton_environment` in `runtime_helpers.rs:169`** — already sets `WINEPREFIX` and `STEAM_COMPAT_DATA_PATH`, and already resolves `pfx/` subdirectory via `resolve_wine_prefix_path`:

```rust
// launch/runtime_helpers.rs:169-196:
pub fn apply_runtime_proton_environment(
    command: &mut Command,
    prefix_path: &str,
    steam_client_install_path: &str,
) {
    let resolved_paths = resolve_proton_paths(Path::new(prefix_path.trim()));
    set_env(command, "WINEPREFIX", resolved_paths.wine_prefix_path...);
    set_env(command, "STEAM_COMPAT_DATA_PATH", resolved_paths.compat_data_path...);
    // ...
}
```

`resolve_wine_prefix_path` (`runtime_helpers.rs:198`) handles the `pfx/` suffix: if the stored path ends in `pfx` it uses it directly; otherwise checks for a `pfx` subdirectory. The winetricks integration can call this same function to compute `WINEPREFIX` correctly.

**`resolve_umu_run_path` in `runtime_helpers.rs:302`** — in-tree binary discovery pattern, replaces the `which` crate:

```rust
// launch/runtime_helpers.rs:302-312:
pub fn resolve_umu_run_path() -> Option<String> {
    let path_value = env::var_os("PATH")
        .unwrap_or_else(|| std::ffi::OsString::from(DEFAULT_HOST_PATH));
    for directory in env::split_paths(&path_value) {
        let candidate = directory.join("umu-run");
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}
```

A `resolve_winetricks_path()` and `resolve_protontricks_path()` follow this exact pattern. No external crate needed.

### Security patterns from `taps.rs` to replicate for verb validation

`community/taps.rs` already demonstrates pre-invocation input validation for subprocess arguments:

- `is_valid_git_sha()` (line 453) — validates positional args against a charset allowlist before passing to `Command`
- `validate_branch_name()` (line 461) — rejects leading `-` (flag injection), restricts charset to safe chars
- `validate_tap_url()` (line 485) — allowlists schemes before use

The **verb allowlist** for `required_protontricks` entries should follow the same pattern: charset regex `^[a-z0-9_]+$`, reject anything starting with `-`, enforce maximum length. These tests in `taps.rs` (lines 708-714) confirm the injection rejection pattern is already tested and working.

**Confidence**: High — verified by reading actual source files at the line numbers cited.

---

## Integration Patterns

### How Other Launchers Integrate

#### Heroic Games Launcher (Electron/Node.js)

**Source**: [Heroic Wiki: Wine and Proton](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To:-Wine-and-Proton), [PR #819](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/819)

Heroic does **not** use protontricks. Instead, it:

1. Locates the Proton-bundled `wine` binary at `<proton_root>/files/bin/wine`
2. Sets `WINEPREFIX` to the game's prefix path
3. Invokes `winetricks` directly with the Wine binary context

This avoids the need for protontricks entirely when the launcher already knows the Proton path. This pattern is viable for CrossHook but requires CrossHook to track the Proton version/path per game — which may already be in the metadata DB.

**Confidence**: Medium — from Heroic's documentation, not source code review.

#### Lutris

**Source**: [Lutris Wine Dependencies Wiki](https://github.com/lutris/docs/blob/master/WineDependencies.md), [GitHub](https://github.com/lutris/lutris)

Lutris bundles its own Wine runner copies and manages dependencies through system package manager instructions (`apt`, `pacman`, etc.) for native Linux libraries. For Wine-prefix dependencies (winetricks verbs), Lutris supports running winetricks directly through its Wine runner context. Lutris does not depend on protontricks.

Pattern: Lutris sets `WINEPREFIX`, `WINE` (binary path), and `WINESERVER` env vars, then invokes `winetricks <verb>` directly.

#### Bottles

**Source**: [Bottles FAQ on Winetricks](https://docs.usebottles.com/faq/where-is-winetricks), [GitHub Issue #180](https://github.com/bottlesdevs/Bottles/issues/180)

Bottles (GTK4/Python) moved away from winetricks entirely in v2. It uses its own dependency system with a repository-based manifest approach (JSON manifests describing installation steps, DLL overrides, registry entries). Bottles tracks installed dependencies in the bottle's configuration YAML. This is a more robust but complex architecture — not suitable to adopt for CrossHook's scope.

**Key lesson from Bottles**: tracking dependency state in a database (rather than querying tool-internal log files at runtime) is more reliable and supports offline inspection. CrossHook's SQLite metadata DB is the correct place — upsert on successful install, query from SQLite to determine what needs installing.

### Recommended Pattern for CrossHook

CrossHook sits closer to the Heroic model (direct winetricks invocation) with protontricks as the primary path:

```
User adds required_protontricks verbs to community profile
    |
CrossHook resolves Steam App ID for the game prefix
    |
Check SQLite metadata for known-installed verbs (cache)
    |
Run: winetricks list-installed (with WINEPREFIX set) to refresh cache
    |
Diff: required - installed = missing_verbs
    |
If missing_verbs not empty:
    Run: protontricks <APPID> -q <missing_verbs...>
    OR: WINEPREFIX=<path> winetricks -q <missing_verbs...>
    |
Update SQLite metadata with new installed verbs
    |
Emit progress events to frontend (Tauri events)
```

---

## Constraints and Gotchas

### 1. Flatpak Protontricks Sandbox Issues

**Confidence**: High — multiple GitHub issues confirm

Flatpak protontricks is sandboxed and only has access to the default Steam directory. Users with custom Steam library folders (e.g., `/media/games/`) must manually grant access:

```bash
flatpak override --user --filesystem=<PATH> com.github.Matoking.protontricks
```

**Impact for CrossHook**: If the user has Flatpak protontricks installed, CrossHook may fail to operate against games in secondary Steam libraries. Mitigation:

- Detect whether protontricks is Flatpak (resolved path under `/var/lib/flatpak/` or `~/.local/share/flatpak/` — use in-tree PATH walk, not `which` crate)
- Show a warning in the UI that Flatpak protontricks may require manual permission grants
- Prefer native protontricks when both are available

### 2. Bwrap Containerization Failures

**Confidence**: High — extensive GitHub issue history in Matoking/protontricks

Protontricks uses bubblewrap (bwrap) for Steam Runtime containerization. This fails in certain environments (nested containers, missing kernel capabilities, some distros). Multiple issues documented in 2023-2024.

**Mitigation**: Expose a `use_no_bwrap` setting in CrossHook's TOML settings. When enabled, append `--no-bwrap` to all protontricks invocations.

### 3. Network Requirements

**Confidence**: High

Winetricks downloads from Microsoft CDNs at runtime:

- `download.microsoft.com`
- `download.visualstudio.microsoft.com`
- Archive.org mirrors (fallback)

Downloads are cached at `~/.cache/winetricks/`. If cache is warm, no network needed. If network unavailable, winetricks fails with an error.

**Impact**: CrossHook should report download failures distinctly from installation failures. The `stderr` output will contain wget/curl error messages.

### 4. Steam Must Be Running

**Confidence**: Medium — from user reports and protontricks behavior

Protontricks requires Steam to be running to resolve game prefix paths when invoked by App ID. Without Steam, it cannot enumerate games via `protontricks -l` or resolve the prefix for `protontricks <APPID>`.

**Mitigation**: CrossHook should ensure Steam is running before invoking protontricks with an App ID, OR use the direct `WINEPREFIX` + `STEAM_COMPAT_DATA_PATH` + `winetricks` approach which does not require Steam.

### 5. Protontricks vs Winetricks: Choose Your Invocation Path

**Two strategies**:

| Strategy              | Command                                     | Requires Steam Running | Requires Steam App ID |
| --------------------- | ------------------------------------------- | ---------------------- | --------------------- |
| Via protontricks      | `protontricks <APPID> -q vcrun2019`         | Yes                    | Yes                   |
| Via winetricks direct | `WINEPREFIX=<path> winetricks -q vcrun2019` | No                     | No                    |

Strategy B (direct winetricks) is more robust when CrossHook already knows the prefix path (it does — from SQLite metadata or the profile). Strategy A is simpler for discovery.

**Recommendation**: Use Strategy B (direct winetricks with WINEPREFIX) for installations, and use protontricks only for initial App ID → prefix path resolution if the prefix path is unknown. Since CrossHook maintains the prefix path in its metadata DB, Strategy B is preferred.

### 6. Interactive Prompts

Winetricks may display interactive dialogs via `zenity` or `kdialog` even with `-q`. Some verbs trigger Wine's own UI (e.g., .NET installer). The `-q` flag suppresses most but not all prompts.

**Confidence**: Medium — from user reports

**Mitigation**: Run with both `-q` and `WINEDLLOVERRIDES="mscoree=d"` for .NET packages when automation is required.

### 7. 32-bit vs 64-bit Prefix Architecture

Some packages (e.g., vcrun2019) install differently into 32-bit vs 64-bit prefixes. Protontricks handles this automatically; direct winetricks invocations require the prefix to already exist with the correct architecture.

---

## Code Examples

### Rust: Detect Binary Paths

Use the in-tree PATH walk pattern from `runtime_helpers.rs:302` — no external crate:

```rust
// Follow resolve_umu_run_path() pattern from launch/runtime_helpers.rs:302
pub fn resolve_protontricks_path() -> Option<String> {
    let path_value = env::var_os("PATH")
        .unwrap_or_else(|| OsString::from(DEFAULT_HOST_PATH));
    for directory in env::split_paths(&path_value) {
        let candidate = directory.join("protontricks");
        if is_executable_file(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}
```

### Rust: Install Dependencies via Winetricks (Direct)

Follow the three-step pattern from `install/service.rs:108-112`: `env_clear()` → `apply_host_environment` → `apply_runtime_proton_environment`.

```rust
use tokio::process::Command;
use std::process::Stdio;

// Mirrors install/service.rs:103-121 pattern — env_clear then restore host env
pub async fn run_winetricks(
    winetricks_path: &str,
    prefix_path: &str,
    verbs: &[String],
) -> anyhow::Result<()> {
    if verbs.is_empty() {
        return Ok(());
    }

    let mut command = Command::new(winetricks_path.trim());
    command.env_clear();
    apply_host_environment(&mut command);           // HOME, PATH, DISPLAY, WAYLAND_DISPLAY, ...
    apply_runtime_proton_environment(&mut command, prefix_path, ""); // WINEPREFIX, STEAM_COMPAT_DATA_PATH
    command.arg("-q").args(verbs);
    command.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);

    let status = command.status().await?;
    if !status.success() {
        anyhow::bail!("winetricks failed with exit code: {:?}", status.code());
    }
    Ok(())
}

// Bootstrap/reconciliation only — SQLite is the source of truth for installed state
pub async fn bootstrap_installed_verbs(
    winetricks_path: &str,
    prefix_path: &str,
) -> anyhow::Result<Vec<String>> {
    let mut command = Command::new(winetricks_path.trim());
    command.env_clear();
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(&mut command, prefix_path, "");
    command.arg("list-installed");
    command.stdout(Stdio::piped()).stderr(Stdio::null());

    let output = command.output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
}
```

### Rust: Install Dependencies via Protontricks (App ID based)

Same three-step env pattern — `env_clear()` + `apply_host_environment` + `apply_runtime_proton_environment`. Note: protontricks discovers the prefix from the App ID internally, so `apply_runtime_proton_environment` is optional here if Steam is running; include it when `STEAM_COMPAT_DATA_PATH` override is needed for non-default library paths.

```rust
// env_clear() + apply_host_environment() — same pattern as install/service.rs:108-112
pub async fn run_protontricks(
    protontricks_path: &str,
    app_id: u32,
    verbs: &[String],
    no_bwrap: bool,
) -> anyhow::Result<()> {
    let mut cmd = Command::new(protontricks_path.trim());
    cmd.env_clear();
    apply_host_environment(&mut cmd);  // HOME, PATH, DISPLAY, WAYLAND_DISPLAY, ...
    if no_bwrap {
        cmd.arg("--no-bwrap");
    }
    cmd.arg(app_id.to_string());
    cmd.arg("-q");
    cmd.args(verbs);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);

    let output = cmd.output().await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("protontricks failed: {}", stderr);
    }
    Ok(())
}
```

### Rust: Stream Progress via Tauri Events

```rust
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use std::process::Stdio;

pub async fn install_with_progress<F>(
    winetricks_path: &str,
    prefix_path: &str,
    verbs: &[String],
    on_progress: F,
) -> anyhow::Result<()>
where
    F: Fn(String) + Send + 'static,
{
    // env_clear() + apply_host_environment() + apply_runtime_proton_environment()
    // mirrors install/service.rs:108-112
    let mut command = Command::new(winetricks_path.trim());
    command.env_clear();
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(&mut command, prefix_path, "");
    command.arg("-q").args(verbs);
    command.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);

    let mut child = command.spawn()?;

    let stderr = child.stderr.take().expect("stderr must be piped");
    let mut lines = BufReader::new(stderr).lines();

    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            on_progress(line);
        }
    });

    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!("winetricks failed with code: {:?}", status.code());
    }
    Ok(())
}
```

---

## Open Questions

1. **Does CrossHook store prefix_path in its SQLite metadata DB per game?** If yes, Strategy B (direct winetricks) is the clear winner and protontricks becomes optional (only needed as a fallback when prefix_path is unknown).

2. **How does CrossHook resolve Steam App ID from a game entry?** This matters because protontricks requires a Steam App ID. If games don't always have App IDs (e.g., non-Steam games), the direct winetricks path is the only option.

3. **Should protontricks be a hard or soft dependency?** If winetricks can always be invoked directly (with WINEPREFIX set), protontricks can be optional — the user only needs winetricks installed. This reduces the installation burden.

4. **What is the correct prefix path for non-Steam Proton games?** These don't follow the `compatdata/<APPID>/pfx` convention. CrossHook may need to store the prefix path manually in its profile/metadata.

5. **How long do typical installations take?** vcrun2019 can take 2-5 minutes on first download (downloads ~50MB); dotnet48 can take 10-20 minutes (complex Wine .NET installation). The UI must support long-running background operations with cancellation.

6. **Should CrossHook validate that winetricks verbs exist before running?** Unknown verbs cause winetricks to fail silently or with an error. Community profiles should validate verb names against a known-good list.

---

## Addendum: Tech-Designer CLI Verification

_Added 2026-04-03 in response to tech-designer queries for checker.rs / BinaryInvocation construction._

### Q1: `protontricks <app_id> list` output format — machine-parseable?

**Answer**: `list` is a **winetricks verb**, not a protontricks command. When you run `protontricks <APPID> list`, protontricks sets up the Proton environment then passes `list` directly to winetricks, which outputs a table of available verb **categories** — not installed packages. This is human-readable, not machine-parseable.

**The correct command for installed package detection is `list-installed`**, not `list`.

`list` output (human-readable, categories only, not useful for automation):

```
apps        - applications
benchmarks  - benchmarks
dlls        - DLL packages
fonts       - font packages
settings    - settings verbs
```

`list-installed` output (machine-parseable — one verb per line on stdout):

```
vcrun2019
d3dx9
corefonts
```

The format is confirmed by the winetricks test harness which uses `grep -w <verbname>` against the `list-installed` output — one verb per line, no description, no headers. The implementation reads directly from `$WINEPREFIX/winetricks.log` (one verb per line in install order). This format is stable across winetricks versions.

**Confidence**: High — confirmed by winetricks test harness source (`grep -w` usage), issue #936 (implementation reads `winetricks.log` directly).

---

### Q2: `--no-bwrap` flag — version availability

**Answer**: `--no-bwrap` was introduced in **protontricks v1.5.0 (2021-04-10)**:

> "Use bwrap containerization with newer Steam Runtime installations. The old behavior can be enabled with `--no-bwrap`"

Versions before 1.5.0 do not accept `--no-bwrap`. Since the recommended minimum version floor is 1.7.0 (see prior addendum), `--no-bwrap` can always be assumed present.

**Confidence**: High — directly from CHANGELOG.

---

### Q3: Does `protontricks <app_id> list-installed` work?

**Answer**: **Yes**, but `list-installed` is a **winetricks verb** that protontricks passes through unchanged. So `protontricks <APPID> list-installed` works and is equivalent to `WINEPREFIX=<prefix>/pfx winetricks list-installed`.

However, for CrossHook's checker logic, **call winetricks directly** with `WINEPREFIX` set rather than routing through protontricks — it is faster (no Steam discovery overhead), works without Steam running, and works for non-Steam prefixes.

`list` vs `list-installed` summary:

- `list` — lists verb categories (human-readable, not useful for automation)
- `list-installed` — lists installed verbs, one per line on stdout (use this)

**Confidence**: High.

---

### Q4: Flatpak invocation correctness

**Answer**: `flatpak run --filesystem=host com.github.Matoking.protontricks --no-bwrap <app_id> <package>` is **functional but not the recommended form**.

**Correct form per Flathub docs**:

```bash
flatpak run com.github.Matoking.protontricks <APPID> -q <verbs>
```

Passing `--filesystem=host` to `flatpak run` at invocation time grants full host filesystem access for that run — valid but overly permissive. The Flathub docs recommend `flatpak override --user --filesystem=<PATH>` for persistent per-library grants instead.

**CrossHook invocation logic**:

```rust
if is_flatpak_binary(&protontricks_path) {
    // Flatpak wrapper: flatpak run <app> <protontricks_args>
    Command::new("flatpak")
        .args(["run", "com.github.Matoking.protontricks"])
        .arg(app_id.to_string())
        .args(protontricks_flags)  // e.g. ["--no-bwrap", "-q"]
        .args(verbs)
} else {
    Command::new(&protontricks_path)
        .arg(app_id.to_string())
        .args(protontricks_flags)
        .args(verbs)
}
```

**Confidence**: Medium — Flathub README is the authority; `--filesystem=host` at runtime is valid but not documented as the intended usage.

---

### Q5: Verb name parity — protontricks vs winetricks

**Answer**: **Verb names are identical**. Protontricks passes ACTIONS directly to winetricks without transformation (stated explicitly in the README: "Any parameters in `<ACTIONS>` are passed directly to Winetricks").

| Verb        | protontricks | winetricks direct | Notes |
| ----------- | ------------ | ----------------- | ----- |
| `vcrun2019` | identical    | identical         |       |
| `dotnet48`  | identical    | identical         |       |
| `d3dx9`     | identical    | identical         |       |
| `corefonts` | identical    | identical         |       |
| `xact`      | identical    | identical         |       |

The same allowlist (`^[a-z0-9_]+$`) applies to both invocation paths.

**Confidence**: High — README explicitly states direct passthrough.

---

### Q6: Non-Steam prefix support — `protontricks 0 <package>`

**Answer**: **App ID 0 does NOT work**. Protontricks has no special handling for ID 0.

**The correct mechanism** for a custom prefix path is the `STEAM_COMPAT_DATA_PATH` environment variable, per the maintainer (Issue #449):

```bash
STEAM_COMPAT_DATA_PATH=/path/to/custom/prefix protontricks <valid_steam_appid> -q vcrun2019
```

This overrides the prefix path but still requires a valid Steam App ID for Proton binary discovery. A real game's App ID must be used even when targeting a non-Steam prefix path.

**For non-Steam games with no Steam App ID at all**: protontricks cannot help. **Winetricks-direct is the only path**:

```bash
WINEPREFIX=/path/to/pfx WINE=/path/to/proton/wine winetricks -q vcrun2019
```

Non-Steam shortcuts in Steam get CRC32-derived IDs (not sequential from 0) discoverable via `shortcuts.vdf`, but this is fragile and CrossHook should not rely on it.

**Confidence**: High — Issue #449 contains explicit maintainer statement; confirmed by steam.py code showing CRC32-based non-Steam ID generation.

**Sources**: [Issue #449](https://github.com/Matoking/protontricks/issues/449), [Issue #22](https://github.com/Matoking/protontricks/issues/22)

---

### Q7: Exit codes

**Winetricks exit codes** (confirmed from test harness source and issue tracker):

| Condition                          | Exit code                                               | Notes                                     |
| ---------------------------------- | ------------------------------------------------------- | ----------------------------------------- |
| Success                            | `0`                                                     | Confirmed by test harness                 |
| General failure / subprocess error | `1`                                                     | POSIX default                             |
| Verb unsupported on 32-bit prefix  | `32`                                                    | `w_package_unsupported_win32()` in source |
| Verb unsupported on 64-bit prefix  | `64`                                                    | `w_package_unsupported_win64()` in source |
| Verb known broken on current Wine  | `99`                                                    | Source code                               |
| Unknown verb                       | `1` + stderr `"Unknown arg <verb>"`                     | Confirmed from user reports               |
| Already installed (no `--force`)   | **`0`** + stdout `"<verb> already installed, skipping"` | Confirmed from user reports               |
| SHA256 mismatch (no `--force`)     | `1`                                                     | Confirmed from security research          |

**Protontricks exit codes**: Protontricks propagates winetricks exit codes. Its own Python-level errors (Steam not found, Proton not found, invalid App ID) exit with `1`.

**Critical implication for checker.rs**: A verb that is already installed exits **`0`** — indistinguishable from a successful fresh install by exit code alone.

**Correct state management approach** (per practices-researcher): **SQLite is the source of truth, not `list-installed` or `winetricks.log`.** CrossHook upserts verb installation records into the metadata DB on successful exit (`0`). To determine what to install, CrossHook queries SQLite for known-installed verbs, computes `required − installed = missing`, and only invokes winetricks for `missing`. Neither `list-installed` stdout nor `winetricks.log` should be the ongoing state record — both are tool-internal and have no stability guarantee. `list-installed` may serve as a one-time bootstrap/reconciliation when SQLite state is absent, but not as the primary check.

**Unknown verb**: winetricks exits `1` with stderr `"Unknown arg <verb>"`. The verb allowlist at profile validation prevents this from reaching winetricks in normal operation.

**`list-installed` on empty prefix**: exits `0` with empty stdout. Safe to parse as an empty set for bootstrap purposes.

**Confidence**: Medium-High — `0`/`1` exit codes confirmed; codes `32`, `64`, `99` from source; "already installed → exit 0" from multiple user reports.

**Sources**: [winetricks test harness](https://github.com/Winetricks/winetricks/blob/master/tests/winetricks-test), [Arch Linux: already installed behavior](https://bbs.archlinux.org/viewtopic.php?id=181539), [winetricks issue #1245](https://github.com/Winetricks/winetricks/issues/1245)

---

## Addendum: Security Follow-up Research

_Added 2026-04-03 in response to security-researcher queries._

### Q1: Flatpak Protontricks — Portal/Permission Requirements for Non-Flatpak Systems

**Confidence**: High — confirmed via Flathub manifest and multiple user reports.

Flatpak protontricks (`com.github.Matoking.protontricks`) does **not** use xdg-desktop-portal for dynamic filesystem access. It uses static Flatpak sandbox permissions declared in the manifest. Default access is limited to the Steam installation directory only.

**How it works with non-Flatpak Steam**:

- Flatpak protontricks v1.7.0+ explicitly added support for non-Flatpak Steam (`[1.7.0] - 2022-01-08: "Enable usage of Flatpak Protontricks with non-Flatpak Steam"`).
- The app can access the default Steam path (`~/.local/share/Steam`) even when Steam is a native install.
- **Secondary Steam library paths require explicit `flatpak override` grants** — they are NOT accessible by default.
- There is no portal-based dynamic permission request (no "allow this app to access /mnt/games" dialog at runtime). Access is strictly declarative.

**Detection heuristic for CrossHook**:

```rust
// Flatpak binary paths
// /var/lib/flatpak/exports/bin/com.github.Matoking.protontricks
// ~/.local/share/flatpak/exports/bin/com.github.Matoking.protontricks
fn is_flatpak_protontricks(path: &std::path::Path) -> bool {
    path.to_str()
        .map(|s| s.contains("flatpak"))
        .unwrap_or(false)
}
```

**Recommendation**: CrossHook should prefer the **native (non-Flatpak) system installation** of protontricks when both exist. If only Flatpak is present and the game prefix is in a non-default library, warn the user with a specific message explaining the permission grant required.

**Sources**: [Flatpak protontricks (Flathub)](https://github.com/flathub/com.github.Matoking.protontricks), [Steam discussions: protontricks folder access](https://steamcommunity.com/app/221410/discussions/0/4615641262428372654/)

---

### Q2: `--no-runtime` Flag — Headless/Automated Mode Suppression

**Confidence**: High — confirmed from CHANGELOG.

`--no-runtime` was introduced in **protontricks 1.2 (2019-02-27)**:

> "Steam Runtime is now supported and used by default unless disabled with `--no-runtime` flag or `STEAM_RUNTIME` environment variable."

**What it does**: Disables the Steam Runtime container (the pressure-vessel/bwrap layer that wraps Wine execution). Without it, some Wine setups may fail on newer Proton versions that expect the runtime.

**Is it useful for automation?** Partially. `--no-runtime` suppresses the runtime selection UI (if any), but the primary flag for suppressing interactive dialogs during verb installation is **`-q` passed through to winetricks** (not a protontricks-level flag). There is no dedicated protontricks "headless mode" flag — the tool is headless by default when invoked without `--gui`.

**Interaction with bwrap**: `--no-bwrap` (added v1.5.0) and `--no-runtime` are independent but related:

- `--no-bwrap` disables only the bubblewrap sandboxing
- `--no-runtime` disables the entire Steam Runtime environment

For fully automated CrossHook invocations with minimal environment assumptions, the safe combination is:

```
protontricks --no-bwrap <APPID> -q <verbs>
```

`--no-runtime` should only be used as a fallback since it can break Proton wine execution for newer Proton versions.

---

### Q3: Minimum Protontricks Version for Reliable Non-Interactive Verb Installation

**Confidence**: High — from CHANGELOG and Repology data.

| Version | Date       | Relevant Change                                              |
| ------- | ---------- | ------------------------------------------------------------ |
| 1.2     | 2019-02-27 | `--no-runtime` flag; Steam Runtime support                   |
| 1.5.0   | 2021-04-10 | `--no-bwrap` flag introduced; bwrap containerization default |
| 1.6.0   | 2021-08-08 | `protontricks-launch` added                                  |
| 1.7.0   | 2022-01-08 | Flatpak protontricks works with non-Flatpak Steam            |
| 1.9.0   | 2022-07-02 | `-l/--list` flag formalized                                  |
| 1.11.1  | 2024-02-20 | Fixed crashes on custom Proton compatibility manifests       |
| 1.12.0  | 2024-09-16 | `--cwd-app` flag; working directory control                  |
| 1.14.1  | 2026-02    | Latest upstream (as of research date)                        |

**Recommended minimum version floor**: **1.7.0** — this is the first version that:

- Supports Flatpak+native Steam combinations
- Has the `--no-bwrap` flag for environments without bwrap
- Has reliable non-interactive verb execution

**Practical floor**: **1.10.x or higher** — LTS distros (Ubuntu 22.04) ship 1.7.0 but Ubuntu 24.04 ships 1.10.5. Recommending 1.10.x covers most users running a supported Ubuntu LTS. Users on Ubuntu 22.04 would be on 1.7.0, which lacks some reliability fixes from 1.9-1.10.

**Distribution lag** (from Repology, as of 2026-04):

- Ubuntu 22.04: **1.7.0** (significantly behind)
- Debian 12: **1.10.2**
- Ubuntu 24.04: **1.10.5**
- Arch / Fedora Rawhide: **1.14.1** (current)
- Flatpak (Flathub): tracks upstream closely

**Implication**: CrossHook should detect the installed version (`protontricks --version` or check `--help` output) and warn users on versions older than 1.10.0. For Ubuntu 22.04 users, recommend installing via `pipx install protontricks` or the Flatpak.

---

### Q4: JSON Output / IPC / Machine-Readable Mode

**Confidence**: High — confirmed absent.

Protontricks has **no JSON output mode, no IPC socket, and no structured output flag**. This was confirmed by:

1. Full CHANGELOG review — no such feature has ever been added
2. Source code and README review — no `--json`, `--machine-readable`, or similar flag documented

**Implication for CrossHook**: Observable outputs from the subprocess are limited to:

- Exit code (0 = success, non-zero = failure)
- Stdout/stderr text (human-readable, no stability guarantee)

There is no programmatic API contract. **CrossHook must not treat any tool-internal file (`winetricks.log`) or command output (`list-installed`) as its source of truth for installed-verb state.** The SQLite metadata DB is the authoritative record; tool outputs serve only as bootstrap or diagnostic data.

---

### Q5: Winetricks SHA256 Mismatch Behavior (Supply Chain Risk)

**Confidence**: High — documented in multiple GitHub issues and confirmed in source code.

**What happens during a SHA256 mismatch**:
When winetricks downloads a package and the checksum does not match the hardcoded expected hash (e.g., because Microsoft updated the file on their CDN), the user sees this dialog:

> "SHA256 mismatch! This is often the result of an updated package such as vcrun2019. If you are willing to accept the risk, you can bypass this check. Alternatively, you may use the --force option."

In non-interactive (`-q`) mode without `--force`, winetricks **aborts** with a non-zero exit code. The install fails.

**How `--force` bypasses it** (from commit `fb82472`):
The `w_verify_sha256sum` function checks `WINETRICKS_FORCE=1`. If set, it logs the warning "Checksum did not match, but --force was used, so ignoring" and proceeds. The download continues with the mismatched file.

**Packages most affected** (recurring issues tracked on GitHub):

- `vcrun2019` — multiple mismatch reports (Issues [#1762](https://github.com/Winetricks/winetricks/issues/1762), [#1836](https://github.com/Winetricks/winetricks/issues/1836), [#1727](https://github.com/Winetricks/winetricks/issues/1727))
- `vcrun2022` — hash changed June 2024 (Issue [#2235](https://github.com/Winetricks/winetricks/issues/2235), fixed via [PR #2401](https://github.com/Winetricks/winetricks/pull/2401))
- `vstools2019` — mismatch (Issue [#2237](https://github.com/Winetricks/winetricks/issues/2237))
- General pattern: any `vcrun*` or `dotnet*` package can be affected when Microsoft silently updates their redistributable files

**Distribution-packaged winetricks lag**: Distro packages (apt/pacman) often ship winetricks versions with stale checksums. The upstream winetricks script updates more frequently.

**CrossHook MUST NOT pass `--force`**: This is confirmed by security-researcher. Passing `--force` disables the only integrity check winetricks performs. The correct behavior on SHA256 mismatch:

1. Let winetricks fail with its non-zero exit code
2. Surface the error to the user: "Dependency installation failed — the package checksum did not match. This usually means your winetricks is outdated. Update winetricks and try again."
3. Recommend updating winetricks: `winetricks --self-update` or upgrading the system package

**Sources**: [SHA256 mismatch Issue #2407](https://github.com/Winetricks/winetricks/issues/2407), [vcrun2019 mismatch #1836](https://github.com/Winetricks/winetricks/issues/1836), [--force bypass commit fb82472](https://github.com/Winetricks/winetricks/commit/fb824722d731cd8dfad6610d6449746e763d81ad)

---

## Sources

- [Matoking/protontricks GitHub](https://github.com/Matoking/protontricks)
- [protontricks README](https://github.com/Matoking/protontricks/blob/master/README.md)
- [protontricks man page](https://linuxcommandlibrary.com/man/protontricks)
- [Winetricks GitHub](https://github.com/Winetricks/winetricks)
- [Winetricks man page (ManKier)](https://www.mankier.com/1/winetricks)
- [Winetricks Arch man page](https://man.archlinux.org/man/winetricks.1.en)
- [Winetricks Ubuntu man page](https://manpages.ubuntu.com/manpages/questing/man1/winetricks.1.html)
- [Winetricks all verbs list](https://github.com/Winetricks/winetricks/blob/master/files/verbs/all.txt)
- [tokio::process::Command docs](https://docs.rs/tokio/latest/tokio/process/struct.Command.html)
- [which crate on crates.io](https://crates.io/crates/which)
- [Heroic Launcher Wiki: Wine and Proton](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To:-Wine-and-Proton)
- [Heroic PR #819: Fix Winetricks with Proton](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/819)
- [Bottles FAQ on Winetricks](https://docs.usebottles.com/faq/where-is-winetricks)
- [Flatpak protontricks (Flathub)](https://github.com/flathub/com.github.Matoking.protontricks)
- [Winetricks list-installed limitation (Issue #936)](https://github.com/Winetricks/winetricks/issues/936)
- [Protontricks bwrap Issue #299](https://github.com/Matoking/protontricks/issues/299)
- [Protontricks --no-bwrap flag documentation](https://github.com/Matoking/protontricks/issues/393)
- [Protontricks version history — Repology](https://repology.org/project/protontricks/versions)
- [Winetricks SHA256 mismatch Issue #2407](https://github.com/Winetricks/winetricks/issues/2407)
- [Winetricks vcrun2019 SHA256 mismatch #1836](https://github.com/Winetricks/winetricks/issues/1836)
- [Winetricks vcrun2022 hash change Issue #2235](https://github.com/Winetricks/winetricks/issues/2235)
- [Winetricks vcrun2022 hash fix PR #2401](https://github.com/Winetricks/winetricks/pull/2401)
- [Winetricks --force SHA256 bypass commit fb82472](https://github.com/Winetricks/winetricks/commit/fb824722d731cd8dfad6610d6449746e763d81ad)
- [Steam discussions: Flatpak protontricks folder access](https://steamcommunity.com/app/221410/discussions/0/4615641262428372654/)

---

## Queries Executed

1. `protontricks CLI interface commands flags exit codes documentation 2024`
2. `winetricks CLI commands flags package detection prefix 2024`
3. `protontricks detect installed packages wine prefix query list`
4. `Heroic Launcher winetricks protontricks integration source code how`
5. `Lutris winetricks integration wine dependency management source implementation`
6. `protontricks flatpak vs native differences subprocess STEAM_COMPAT_DATA_PATH limitations`
7. `rust tokio process Command spawn async subprocess output parsing crate 2024`
8. `winetricks "list-installed" output format parsing script detection installed verbs`
9. `Bottles launcher winetricks dependency prefix integration implementation 2023 2024`
10. `winetricks vcrun2019 dotnet48 d3dx9 corefonts xact packages available verbs list`
11. `protontricks python API programmatic usage steam app id prefix path detection source`
12. `rust crate "which" binary detection path resolution crates.io 2024`
13. `winetricks network requirements download Microsoft redistributable HTTPS CDN offline mode cache`
14. `protontricks flatpak "flatpak-spawn" host permission steam library path issue`
15. `protontricks "steam runtime" bwrap compatibility issues workaround 2024`
16. `winetricks installation progress output format stderr stdout parsing script automation`
17. `protontricks winetricks security risks downloads execution sandbox wine prefix attack surface`
