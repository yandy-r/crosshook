# Security Research: CLI Completion Feature

**Date**: 2026-03-30
**Scope**: CrossHook native Linux CLI — 7 commands: `status`, `profile list`, `profile import`, `profile export`, `steam discover`, `steam auto-populate`, `launch`
**Researcher**: security-researcher agent

---

## Executive Summary

The CLI completion feature introduces moderate security surface primarily through process spawning (launch commands) and filesystem access (profile import/export, Steam discovery). Two CRITICAL findings relate to helper script path validation and unrestricted profile import paths. Five WARNING-level findings cover argument injection, TOCTOU races, and path exposure. All findings have practical mitigations.

## Findings by Severity

### Severity Summary

| Severity | Count | Commands Affected                                                              |
| -------- | ----- | ------------------------------------------------------------------------------ |
| CRITICAL | 2     | `launch`, `profile import`                                                     |
| WARNING  | 5     | `launch`, `profile import/export`, `steam auto-populate`, `diagnostics export` |
| ADVISORY | 6     | all commands                                                                   |

---

## CRITICAL Findings

| ID  | Title                                                                    | Command(s)       | Location                    |
| --- | ------------------------------------------------------------------------ | ---------------- | --------------------------- |
| C-1 | Helper script path is compile-time relative, not validated at runtime    | `launch`         | `main.rs:267`, `args.rs:52` |
| C-2 | Profile `legacy_path` is fully user-controlled with no containment check | `profile import` | `args.rs:76`, `legacy.rs:8` |

### C-1: Helper Script Path Is Compile-Time Relative, Not Runtime-Validated

**Impact**: The default helper script path is derived from `CARGO_MANIFEST_DIR` at compile time:

```rust
// main.rs:271
fn default_scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_SCRIPTS_DIR)
}
// DEFAULT_SCRIPTS_DIR = "../../runtime-helpers"
```

The resolved path is `<manifest_dir>/../../runtime-helpers/steam-launch-helper.sh`. At runtime in the AppImage, `CARGO_MANIFEST_DIR` is baked into the binary at build time and resolves to the **build host** filesystem path, not the AppImage mount path. If an attacker can place or symlink a `steam-launch-helper.sh` at that resolved location, or if the path resolution yields a writable location on the target system, they can substitute an arbitrary shell script that runs with the invoking user's privileges.

Additionally, the `--scripts-dir` flag (`args.rs:52`, marked `hide = true`) accepts a fully arbitrary `PathBuf` with no existence check, signature check, or containment restriction before it is executed via `spawn_helper`.

**Why this is CRITICAL**: The script is invoked directly as `/bin/bash <path>` with an env-cleared child process. No HMAC, checksum, or permission check is performed on the script before execution. A misconfigured or maliciously placed script will run silently.

**Required fix**: At runtime, verify the helper script path using one of:

- Embed the helper script bytes into the binary with `include_str!` / `include_bytes!` and write to a temp file with mode `0700` before each invocation (preferred for AppImage distribution).
- Use an AppImage-relative path: `std::env::current_exe()` → traverse to `<appimage_mount>/usr/share/crosshook/` where the helper is installed.
- At minimum, assert `helper_script.is_file()` and verify it is owned by the current UID before invoking.

---

### C-2: `profile import --legacy-path` Accepts Arbitrary Filesystem Paths Without Containment

**Impact**: The `legacy_path` argument (`args.rs:76`) is a raw `PathBuf` with no containment check. It is passed directly to `legacy::load()` which calls `fs::read_to_string(&path)`. The `legacy::load` function also calls `validate_name(name)` but the `name` validated here is the caller-supplied profile name, **not the `legacy_path`** itself.

This allows:

- Reading any file on the filesystem the user can access (e.g., `~/.ssh/id_rsa`, `/etc/shadow` if readable, other users' config files on shared systems).
- A malicious TOML/legacy profile sourced from an untrusted location silently replacing an existing profile if the parsed name collides.

The key path here: `legacy::load(profiles_dir, name)` uses `profile_path(profiles_dir, name)` which appends `.profile` to the name and constructs a path inside `profiles_dir` — so within legacy load the path is safe. However, the new CLI command is intended to read from an **external** file (`--legacy-path`) which is a separate concern not yet implemented. When implementation lands, reading from an arbitrary external path with no canonicalization or containment check is the risk.

**Required fix**: Before reading the legacy file, canonicalize the path and confirm it does not reside in a sensitive system directory. At minimum, emit a clear warning when the path is outside `~/.config/crosshook/` and require an explicit `--force` flag.

---

## WARNING Findings

| ID  | Title                                                                          | Command(s)            | Location                      |
| --- | ------------------------------------------------------------------------------ | --------------------- | ----------------------------- |
| W-1 | Profile field values passed as raw args to shell script without shell escaping | `launch`              | `script_runner.rs:169–205`    |
| W-2 | Log path in `/tmp` is TOCTOU-susceptible for log injection                     | `launch`              | `main.rs:275–288`             |
| W-3 | `--json` output for `diagnostics export` emits full absolute archive path      | `diagnostics export`  | `main.rs:166–170`             |
| W-4 | `profile export --output` path not validated before write                      | `profile export`      | `args.rs:84` (unimplemented)  |
| W-5 | `steam auto-populate --game-path` follows symlinks during filesystem scan      | `steam auto-populate` | `args.rs:103` (unimplemented) |

### W-1: Profile Field Values Passed as Raw Arguments to Bash Script

**Impact**: `build_helper_command` and `trainer_arguments` pass values from the profile (`app_id`, `compatdata_path`, `proton_path`, `steam_client_install_path`, `trainer_path`, `trainer_host_path`) as individual `OsString` arguments to `/bin/bash <script>` via `command.args(...)`. Rust's `Command::args` does not invoke a shell — it passes each element as a separate `argv` entry — so shell metacharacters in these values (`; | && $()` etc.) do **not** constitute shell injection at the process-spawning layer.

However, the shell script itself (`steam-launch-helper.sh`) receives these as positional parameters (`$1`, `$2`, ...) and may use them unquoted in shell constructs. If the shell script does `eval` or unquoted variable expansion (`rm -rf $trainer_path`), it becomes vulnerable to shell injection from malicious profile data.

**Why this is WARNING not CRITICAL**: The Rust layer is safe; the risk depends entirely on the shell script implementation which is not in scope for this evaluation but must be audited in parallel.

**Required fix**:

1. Audit `steam-launch-helper.sh` and all runtime helper scripts for unquoted variable usage.
2. Ensure all path arguments from the profile are quoted in the shell script: `"$trainer_path"` not `$trainer_path`.
3. Consider defining a validation allowlist for `app_id` (should be numeric digits only).

---

### W-2: Log Path in `/tmp` Is TOCTOU-Susceptible

**Impact**: Log path is constructed as `/tmp/crosshook-logs/<sanitized_name>.log` in `launch_log_path()`. While the sanitization (replacing non-alphanumeric chars with `-`) is correct, the path resides in a world-writable directory. A local attacker on the same system can:

- Pre-create `/tmp/crosshook-logs/<name>.log` as a symlink to an arbitrary file before CrossHook opens it for writing (symlink attack).
- Inject content into the log before `stream_helper_log` reads it, causing misleading diagnostic output.

This is a standard TOCTOU issue with `/tmp` usage. On single-user systems (Steam Deck personal use) the risk is low. On shared Linux systems the risk is moderate.

**Required fix**: Use `tempfile::Builder` or create the log directory with mode `0700` (owner-only) before writing. Alternatively, use `XDG_RUNTIME_DIR` (which is user-private) for transient logs:

```rust
// preferred
let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|_| std::env::temp_dir());
let log_dir = runtime_dir.join("crosshook-logs");
```

---

### W-3: `--json` Output Emits Full Absolute Archive Path

**Impact**: The `diagnostics export --json` output emits the full archive path including the home directory expansion:

```json
{"archive_path": "/home/alice/.config/crosshook/diagnostics-20261030.tar.gz", ...}
```

If this output is captured in logs, CI systems, or shared terminals, the username and directory structure are leaked. When `--redact-paths` is not passed to the Rust layer (it is applied inside the archive, not to the CLI JSON output), this path is always unredacted.

**Required fix**: Apply the same `redact_home_paths()` function to `archive_path` in the JSON output when `--redact-paths` is set, or document clearly that `--redact-paths` does not apply to the CLI output path.

---

### W-4: `profile export --output` Path Not Validated Before Write (Future Implementation)

**Impact**: The `--output` flag (`args.rs:84`) is parsed as an arbitrary `PathBuf`. When implemented, writing the exported profile to this path without containment checks would allow:

- Overwriting system files if running with elevated privileges.
- Path traversal if the path is sourced from another profile field rather than the CLI directly.

**Required fix**: When implementing `profile export`, validate that the output path:

1. Does not exist as a symlink pointing outside the intended directory.
2. Has a parent directory the user has write permission to.
3. Canonicalizes to a path outside protected directories (`/etc`, `/usr`, `/bin`).

---

### W-5: `steam auto-populate --game-path` Follows Symlinks During Filesystem Scan (Future Implementation)

**Impact**: Steam library scanning traverses the filesystem looking for `.acf` manifest files and VDF files. If an attacker can place a symlink in a Steam library directory that points to a sensitive system path, the scanner may read unintended files.

**Required fix**: When implementing `steam auto-populate` and `steam discover`, use `std::fs::symlink_metadata()` instead of `metadata()` to detect symlinks before following them, and skip symlinked entries during scan.

---

## ADVISORY Findings

| ID  | Title                                                                                   | Command(s)               |
| --- | --------------------------------------------------------------------------------------- | ------------------------ |
| A-1 | `profile_store()` in CLI calls `process::exit(1)` on init failure                       | all                      |
| A-2 | `--config` flag allows arbitrary profile directory, enabling profile confusion          | `launch`, `profile list` |
| A-3 | `diagnostics export` bundle includes raw log files that may contain sensitive paths     | `diagnostics export`     |
| A-4 | No rate limiting or lockfile for concurrent CLI invocations                             | `launch`                 |
| A-5 | `serde_json::to_string_pretty` for `--json` output does not sanitize ANSI/control chars | all `--json` commands    |
| A-6 | `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var override accepted without validation         | `launch`                 |

### A-1: `profile_store()` Calls `process::exit(1)` on Init Failure

`profile_store()` at `main.rs:190` calls `std::process::exit(1)` if the home directory cannot be found. This bypasses Rust's normal error propagation, skips Drop implementations (including tempfile cleanup), and cannot be caught in tests. Use `?` propagation instead.

### A-2: `--config` Flag Allows Arbitrary Profile Directory

The `--config` global flag sets an arbitrary profile base directory. While intentional for testing, in production use it opens a profile-confusion attack: an attacker who can influence CLI invocations can redirect the profile store to a controlled directory, loading a malicious profile with arbitrary paths. This is acceptable for a personal tool but should be noted.

### A-3: Diagnostic Bundle May Include Sensitive Absolute Paths

Even with `--redact-paths`, paths in Steam VDF files and in PROTON_LOG output captured in launch logs may contain home directory paths. The `collect_app_logs()` and `collect_launch_logs()` functions read raw file bytes without applying path redaction.

### A-4: No Lockfile for Concurrent CLI Invocations

Multiple simultaneous `crosshook launch` calls for the same profile share the same log path (`/tmp/crosshook-logs/<name>.log`). Concurrent launches interleave log output and may cause `drain_log` to read partially written bytes. A per-profile lockfile under `XDG_RUNTIME_DIR` would prevent this.

### A-5: `--json` Output Does Not Sanitize Control Characters

Profile names, game paths, and error messages included in JSON output are serialized via `serde_json::to_string_pretty`. Serde-json correctly escapes JSON string control characters. This is safe. However, if consumers pipe the output into a terminal emulator, profile names containing ANSI escape sequences could cause visual confusion. This is a cosmetic issue only.

### A-6: `STEAM_COMPAT_CLIENT_INSTALL_PATH` Env Var Override Without Validation

`resolve_steam_client_install_path()` reads `STEAM_COMPAT_CLIENT_INSTALL_PATH` from the environment and uses it as the Steam install path without checking that it points to a real Steam installation. A malicious value (e.g., `/tmp/fake-steam`) would be passed to the helper script as `--steam-client`. Validate the candidate by checking for `steam.sh` existence as is done for the fallback paths.

---

## Detailed Analysis

### Process Spawning Security

**Current architecture**: CrossHook uses `tokio::process::Command` with `env_clear()` and explicit env var re-injection. This is a correct approach that prevents host environment bleed-through.

**What is safe**:

- `env_clear()` is applied before all process spawns (`script_runner.rs:140`, `runtime_helpers.rs:26`).
- Arguments are passed as discrete `OsString` argv entries, not via shell interpolation — Rust's `Command` does not invoke a shell by default.
- `WINE_ENV_VARS_TO_CLEAR` is a well-maintained list that prevents Proton session contamination.
- Profile name sanitization for log paths (`launch_log_path()`) replaces non-alphanumeric chars with `-`.

**What is not safe**:

- The helper script path is not integrity-checked at runtime (C-1).
- Shell scripts receiving arguments may use them unquoted (W-1).
- The `launch_method` field from a profile is compared against an allowlist (`validate()` in `request.rs:464`) but the comparison is on the trimmed string, not the raw bytes — a profile with `method = "steam_applaunch\x00"` would pass trimming. Rust strings are UTF-8 and NUL bytes are rejected by the TOML parser, so this is theoretical.

### File System Security

**Profile name validation** (`validate_name()` in `toml_store.rs:443`) is thorough:

- Rejects `.` and `..`
- Rejects absolute paths and any `/`, `\`, `:` characters
- Rejects all Windows reserved path characters

This is the correct containment mechanism. The profile store cannot escape `base_path` via name manipulation.

**Profile directory creation**: `fs::create_dir_all(&self.base_path)` creates the config directory without setting restrictive permissions. On Linux, `~/.config/crosshook/profiles/` will inherit the umask. The default umask (`022`) results in world-readable profile files. Profile TOML files contain absolute paths to game executables and trainers; consider writing with explicit mode `0600` via `OpenOptions`.

**Legacy import path**: Not yet implemented but the `legacy_path` argument is uncontained (C-2).

### Data Exposure

**Diagnostic bundle (`--redact-paths` flag)**:

- `--redact-paths` applies `redact_home_paths()` to profile TOML content and settings — this is a text-level `$HOME` → `~` replacement.
- Launch log files are included raw, bypassing redaction.
- The archive path printed to stdout/JSON is never redacted (W-3).

**`--json` output**:

- `diagnostics export --json` serializes `DiagnosticBundleResult` which includes the full `archive_path` and a `summary` with counts — no raw paths beyond `archive_path`.
- `profile list --json` (unimplemented) should return only profile names, not paths.
- Error messages propagated via `eprintln!("{error}")` may contain file system paths. Ensure error types don't expose internal path structure unnecessarily — the existing `ProfileStoreError::NotFound(path)` will print the full path, which is acceptable in a personal-use CLI context.

**stderr error messages**:

- `ValidationSeverity` labels are printed to stderr with `[fatal]`, `[warning]`, `[info]` prefixes — no sensitive data.
- `"helper exited with status {status}"` reveals process exit codes but no paths.
- `"could not determine Steam client install path"` — no path leakage.

### Dependency Security

| Crate                  | Version    | Notes                                                                                                                                                                                  |
| ---------------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `clap`                 | 4.x        | No known CVEs in v4. V4 does not process shell arguments — safe.                                                                                                                       |
| `serde` + `serde_json` | 1.x        | No known deserialization RCE (data-only model). Memory safe in Rust.                                                                                                                   |
| `toml`                 | 1.1.0      | No known CVEs. Serde-based — same guarantees as serde.                                                                                                                                 |
| `tokio`                | 1.x        | No relevant CVEs for the features in use.                                                                                                                                              |
| `anyhow`               | (via core) | Not a direct CLI dep; no CVEs.                                                                                                                                                         |
| `rusqlite`             | 0.39.0     | Bundles SQLite. Known CVEs only in pre-0.27 rusqlite versions. SQLite bundling means the SQLite version is pinned at build time — verify `SQLite >= 3.45` (no CVE-2025-6965 exposure). |
| `sha2`                 | 0.11.0     | Crypto crate, no known CVEs.                                                                                                                                                           |
| `uuid`                 | 1.x        | No known CVEs.                                                                                                                                                                         |
| `flate2`               | 1.x        | Wraps miniz/zlib. No known CVEs affecting this use case.                                                                                                                               |

**CVE-2024-24576 (Rust std, CVSS 10.0)**: This critical Rust vulnerability affects Windows only — CrossHook is a Linux-only application. Not applicable.

**Supply chain risk**: The dependency set is minimal and well-maintained. No unmaintained crates. No `unsafe` blocks in `crosshook-cli` itself; `crosshook-core` uses rusqlite which has unsafe internal FFI to SQLite but this is the bundled crate's responsibility.

**Recommendation**: Add `cargo audit` to CI pipeline to catch new advisories.

### Launch Method Security

#### `steam_applaunch` (Current CLI implementation)

The CLI currently only supports `steam_applaunch`. The Steam client mediates the launch — CrossHook's helper script invokes `steam steam://run/<appid>` and waits. The Steam client itself is responsible for Proton sandbox setup. This is the most constrained launch method from a privilege escalation standpoint.

**Risk surface**:

- `app_id` is passed as `--appid <value>` to the shell script. If the script constructs `steam://run/$app_id` without quoting, a malicious app_id (`12345; rm -rf /`) would be exploitable.
- `steam_client_install_path` is passed without existence or ownership validation.

#### `proton_run` (Future CLI expansion)

Direct Proton invocation. CrossHook constructs:

```
<proton_path> run <game_path>
```

with env vars set explicitly. This runs the Proton binary directly without Steam client mediation. Risks:

- `proton_path` is passed as the program name to `Command::new(proton_path.trim())`. If `proton_path` is not an absolute path and `PATH` is not cleared, a relative path could resolve to a malicious binary. However, `env_clear()` is applied first so the `PATH` in the child will not assist resolution — the OS `execve` call will use an absolute path if given.
- If `proton_path` is relative, the OS will resolve it against the process working directory. **Always validate that `proton_path` is absolute before spawning.**

#### `native` (Future CLI expansion)

Direct execution of a Linux binary:

```rust
Command::new(request.game_path.trim())
```

with `env_clear()`. Same risk as `proton_run` — verify `game_path` is absolute and exists before invoking. The `build_native_game_command` function at `script_runner.rs:121` already calls `env_clear()`.

---

## Secure Coding Guidelines for Implementation

### Path Handling

```rust
// CORRECT: Validate paths are absolute before spawning
fn validate_executable_path(path: &str) -> Result<(), Error> {
    let p = Path::new(path.trim());
    if !p.is_absolute() {
        return Err(anyhow!("path must be absolute: {}", path));
    }
    if !p.exists() {
        return Err(anyhow!("path does not exist: {}", path));
    }
    Ok(())
}

// WRONG: Calling Command::new with a relative path
Command::new("proton")  // resolves via PATH, env_clear already clears it but still unsafe
```

### Profile Import (Legacy Path Containment)

```rust
// Profile import: do NOT constrain to ~/.config/crosshook —
// legacy paths are intentionally external files the user selects.
// DO validate it's a regular file and not a device/symlink to sensitive paths.
fn validate_import_path(path: &Path) -> Result<(), Error> {
    let metadata = path.symlink_metadata()
        .with_context(|| format!("cannot access: {}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(anyhow!("not a regular file: {}", path.display()));
    }
    // Optionally: check extension is .toml or .profile
    Ok(())
}
```

### App ID Validation

```rust
// Steam App ID should be numeric only
fn validate_app_id(app_id: &str) -> Result<(), Error> {
    let trimmed = app_id.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!("app_id must be a numeric Steam App ID, got: {}", app_id));
    }
    Ok(())
}
```

### Log Directory Creation

```rust
// Use XDG_RUNTIME_DIR for logs, with restrictive permissions
fn launch_log_path(profile_name: &str) -> PathBuf {
    let safe_name: String = profile_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    let log_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("crosshook-logs");

    // Create with 0700 to prevent other users from reading/writing
    std::fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(&log_dir)
        .ok();

    log_dir.join(format!("{safe_name}.log"))
}
```

---

## Trade-off Recommendations

| Decision                  | Option A                                   | Option B                          | Recommendation                                                     |
| ------------------------- | ------------------------------------------ | --------------------------------- | ------------------------------------------------------------------ |
| Helper script path        | Embed in binary (`include_bytes!`)         | AppImage-relative runtime path    | **Embed** — eliminates external file trust entirely                |
| Legacy import containment | Block reads outside `~/.config/crosshook/` | Warn + require `--force`          | **Warn + `--force`** — user may legitimately import from Downloads |
| Log directory             | `/tmp/crosshook-logs` (current)            | `$XDG_RUNTIME_DIR/crosshook-logs` | **XDG_RUNTIME_DIR** — user-private, no TOCTOU                      |
| Diagnostic path redaction | Apply only inside archive                  | Apply to CLI output too           | **Apply to CLI output** — simple one-line fix                      |
| Profile file permissions  | Inherit umask                              | Explicit 0600                     | **Explicit 0600** — minimal cost, better hygiene                   |
| `cargo audit` in CI       | Optional                                   | Required                          | **Required** — zero cost to add, high value                        |

---

## Sources

- [CVE-2024-24576: Rust std command injection on Windows (CVSS 10)](https://thehackernews.com/2024/04/critical-batbadbut-rust-vulnerability-exposes-windows-systems-to-attacks.html) — Windows-only, not applicable to CrossHook
- [rusqlite CVE history (pre-0.27 only)](https://www.cvedetails.com/product/90260/Rusqlite-Project-Rusqlite.html?vendor_id=23945)
- [SQLite CVE-2025-6965](https://www.wiz.io/vulnerability-database/cve/cve-2025-6965) — affects SQLite; verify bundled version in rusqlite 0.39
- [Rust process::Command security — tokio docs](https://docs.rs/tokio/latest/tokio/process/struct.Command.html)
- [serde deserialization safety discussion](https://github.com/serde-rs/serde/issues/1087)
- [Securing Rust Apps: Command Injection Prevention](https://www.stackhawk.com/blog/rust-command-injection-examples-and-prevention/)
