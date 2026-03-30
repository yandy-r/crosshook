# Security Research: trainer-onboarding

## Executive Summary

CrossHook is a local desktop application that orchestrates trainer launches via Proton/WINE. The trainer-onboarding feature (GitHub issue #37) adds: first-run readiness checks, in-app trainer discovery guidance, and a guided workflow chaining auto-populate → profile creation → trainer selection → launch.

**Threat model context:** This is a local, single-user desktop app. There is no authentication, no network-exposed API, and no server. The user is both operator and end user. The primary threat vectors are:

1. **Malicious community tap content** injecting unexpected git arguments or file paths
2. **Trainer files with crafted names/paths** escaping the staging area
3. **Exported shell scripts** embedding unsanitized user-supplied values
4. **Information disclosure** (user paths leaked to UI or logs)

The existing codebase already addresses the most significant risks well: `shell_single_quoted()` correctly POSIX-quotes paths in exported scripts, process launches use `Command::arg()` (argv-safe, not shell-interpolated), and `sanitize_display_path()` scrubs home paths from diagnostic output. The onboarding feature does not introduce new architectural risk but it does expose several existing advisory-level gaps that become more visible at first-run.

**Overall verdict:** No CRITICAL hard stops for the base feature scope. Two WARNING items require mitigation before ship. Several ADVISORY items are best addressed in a follow-up. One CONDITIONAL WARNING (W-3) applies only if zip archive extraction is added to scope.

**Revision note:** Updated after reviewing proposed architecture from tech-designer, dependency proposals from api-researcher, and staging path question from business-analyzer.

---

## Findings by Severity

### CRITICAL — Hard Stops

_None identified._

The feature does not introduce authentication surfaces, remote execution, arbitrary code injection paths that bypass existing mitigations, or significant data-exposure risks.

---

### WARNING — Must Address

#### W-1: Git Branch Argument Injection via Community Tap Branch Name

**File:** `crates/crosshook-core/src/community/taps.rs:209,240`

**Description:** Branch names supplied by users are passed as arguments to `git fetch` and `git clone --branch` via `Command::arg()`. While this is not shell injection (no shell interpolation), git itself parses argument strings and a branch name beginning with `--` can be interpreted as a git option flag.

Example: if a malicious community tap subscription specifies `branch: "--upload-pack=/path/to/evil-script"`, git would interpret this as the `--upload-pack` option on clone, potentially executing an attacker-controlled binary during tap sync.

**Affected code:**

```rust
// taps.rs:209 — branch passed directly as git argument
command.arg("--branch").arg(workspace.branch())

// taps.rs:240 — branch passed to fetch
&["fetch", "--prune", "origin", workspace.branch()]
```

`normalize_subscription()` only checks for empty URLs and whitespace — it does not reject branch names starting with `-`.

**Severity:** WARNING — the exploit requires a user to intentionally add a malicious community tap subscription (they enter the URL themselves), but that's realistic social engineering during onboarding guidance. The guided workflow may actively encourage users to add community taps.

**Mitigations (alternatives welcome):**

- **Preferred:** Validate branch names in `normalize_subscription()`: reject any branch name starting with `-`, enforce `[a-zA-Z0-9/._-]` pattern, max 200 chars.
- **Alternative:** Insert `--` before the branch argument in git commands: `["fetch", "--prune", "origin", "--", branch]` and `clone --branch -- <branch>`. Git interprets `--` as end of options, preventing flag injection.
- **Additional defense:** URL scheme allowlist (see W-2).

---

#### W-2: Community Tap `file://` Protocol Allows Arbitrary Local Repository Cloning

**File:** `crates/crosshook-core/src/community/taps.rs:362-387`

**Description:** `normalize_subscription()` accepts any URL that is non-empty and whitespace-free. This includes `file:///home/user/.ssh/`, `file:///etc/`, and other `file://` URIs that git would happily clone from as repositories. While a `file://` clone from a non-git path would fail, a crafted local git repository could be set up to serve malicious profile content.

More practically: onboarding docs or in-app links pointing to "community tap URLs" might be copy-pasted from untrusted sources, and `file://` bypasses any HTTPS trust expectation the user may have.

**Mitigations:**

- **Preferred:** In `normalize_subscription()`, require URL scheme to be `https://` or `ssh://git@`. Reject `file://`, `git://`, and bare paths.
- **Alternative:** Warn but allow — present a clear UI warning when URL scheme is not `https://`, require explicit user confirmation.

---

#### W-3: External Link Opening — `shell:open` Capability Must Use Allowlist

**Source:** UX researcher finding #1.

**File:** `src-tauri/capabilities/default.json`

**Description:** The onboarding readiness check UI proposes action buttons such as "Install Steam ↗" and "Open Steam Settings ↗" that open URLs or launch system applications. Opening external resources in Tauri v2 requires the `opener` plugin (`opener:open-url` permission) — this permission is **not currently granted** in `capabilities/default.json` (current permissions: `core:default`, `dialog:default` only).

When this permission is added, the call site matters critically:

- **Safe:** `opener::open_url("https://store.steampowered.com/about/", None)` — hardcoded URL constant → no injection surface
- **Unsafe:** `opener::open_url(&format!("steam://open/{}", scan_result), None)` — URL constructed from scan/profile data → URI injection

If any URL is constructed by concatenating backend scan results, profile fields, or other variable data, a malicious profile or crafted scan result could redirect the open call to an arbitrary URI scheme (e.g., `file://`, `javascript:`, custom protocol handlers).

**Current state:** Capability not yet granted — safe by omission. Becomes a risk when the feature adds the first external-link button.

**Mitigations:**

- **Required:** All URLs passed to `opener::open_url` must be hardcoded string constants in the frontend TypeScript. Never construct URLs from IPC response data.
- **Required:** When adding the `opener:open-url` capability, scope it to a URL pattern allowlist in `capabilities/default.json`:

  ```json
  {
    "identifier": "opener:allow-open-url",
    "allow": [{ "url": "https://*.steampowered.com/**" }, { "url": "steam://**" }]
  }
  ```

- **Check:** Confirm no Tauri command returns a URL string intended to be passed directly to `opener::open_url` from the frontend — URLs must originate in the frontend as constants, not be retrieved from the backend.

---

#### W-4 (CONDITIONAL): Zip Archive Path Traversal — Applies Only If Extraction Is In Scope

**Status:** Conditional — base feature spec does NOT include zip extraction. If scope expands to extract FLiNG trainer archives, this becomes a pre-ship WARNING.

**Description:** The `zip` crate's `ZipArchive::extract()` API has historically been vulnerable to path traversal attacks via zip entries containing `../` path components (zip-slip). The `zip 2.x` crate strips `../` components in most paths, but the specific guarantee depends on OS and how `extract()` is called. Parsing attacker-controlled zip archives from third-party trainer sites is a meaningful attack surface — a crafted archive could write files outside the intended extraction directory.

**Mitigation (if zip extraction is added):**

1. Before extracting, iterate all entries and reject any with paths containing `..` or that produce a canonical path outside the target directory.
2. Limit total extracted bytes to a reasonable cap (e.g., 500MB).
3. Skip or reject non-`exe`, `dll`, `ini`, `json`, `pak`, `dat`, `bin`, `config` entries with a warning — alert users to unexpected file types (`.bat`, `.ps1`, `.cmd`, `.reg`, `.vbs`).
4. Extract to a temporary staging directory first, then move to the final location only after validation passes.

**Recommendation:** Do not add zip extraction in the trainer-onboarding v1 scope. The feature spec says "guidance for finding trainers" — the user downloads and extracts manually. If extraction is added later, treat it as a separate security review milestone.

---

### ADVISORY — Best Practices

#### A-1: Symlink Following During Trainer Support File Staging (`CopyToPrefix`)

**File:** `crates/crosshook-core/src/launch/script_runner.rs:268-326`

**Description:** `stage_trainer_support_files()` and `copy_dir_all()` use `fs::copy()` and `fs::read_dir()` without checking for symlinks. If the trainer directory contains symlinks (e.g., a symlink named `assets/` pointing to `/etc/`), the staging code will recursively copy the symlink target into the Proton prefix under `C:\CrossHook\StagedTrainers\`.

This does not provide privilege escalation (files are just copied into the user's own prefix), but it silently copies potentially sensitive files (SSH keys, config files) into an area accessible to the Windows trainer executable running under WINE.

**Risk context:** Low. The user must explicitly select a trainer directory containing such symlinks. FLiNG/WeMod trainer packages are zip archives and do not contain symlinks.

**Mitigation:** In `copy_dir_all()`, skip symlinks during staging (`source_path.is_symlink()` check before copy). Log skipped symlinks at debug level.

---

#### A-2: No PE Header Validation at Trainer File Selection

**Description:** The onboarding guided workflow will help users point CrossHook at a trainer `.exe` file they downloaded. The app validates that the path exists and is a file (`TrainerHostPathNotFile` error), but does not verify the file is a valid Windows PE executable (`MZ` magic bytes).

**Impact:** A user who accidentally selects the wrong file type (e.g., a `.zip` archive renamed `.exe`, or a shell script) will only discover the error after Proton fails at runtime. For first-run onboarding, this produces a confusing error and may be attributed to CrossHook misconfiguration.

**Mitigation (advisory):** Add a lightweight PE header check in the existing trainer path validation logic: read the first 2 bytes and confirm `MZ` (0x4D 0x5A). Return a new `ValidationSeverity::Warning` (not Fatal) so the user is informed but not blocked. No new dependencies required — this is 5 lines of `std::fs::File` read.

---

#### A-2b: Full PE Parser (`goblin`/`pelite`) Dependency Risk

**Context:** api-researcher proposed adding `goblin 0.9` or `pelite 0.10` for full PE header parsing. This advisory supplements A-2.

**Assessment:** Full PE parsing is not warranted for the advisory MZ check. Both crates parse untrusted binary data from user-downloaded trainer files — this is a non-trivial attack surface. While both crates have fuzzing coverage, accepting PE parsing of potentially malformed files from arbitrary third-party download sites expands the dependency risk beyond what the use case requires.

**Decision:**

- For the `MZ` magic byte check (A-2): use the 2-byte `std::fs::File` read — no new crate needed.
- If PE metadata is needed for diagnostics (e.g., architecture detection, version info): the use case should be specifically defined first. Only add `goblin` or `pelite` if there is a concrete, scoped requirement that cannot be met by the 2-byte check. Add the chosen crate as a `crosshook-core` dev-dependency initially, promote to regular dependency only when the feature ships.
- If added: always pre-validate `MZ` magic before passing to goblin/pelite, and cap file read size to avoid exhausting memory on a malformed/large file.

---

#### A-3: Trainer Source URLs in Onboarding UI Must Be Hardcoded

**Description:** The onboarding feature will display guidance on where to find trainers (FLiNG, WeMod, etc.). If these URLs are loaded from a configuration file, a database record, or a community tap index rather than being hardcoded in the binary, they become an injection surface: a malicious community tap or corrupted config could replace trainer source guidance with phishing URLs.

**Mitigation:** Trainer source recommendations in the onboarding UI must be compiled-in constants, not read from the filesystem or database at runtime. If localization or customization is needed, gate it behind a compile-time feature flag reviewed during release.

---

#### A-4: `escape_desktop_exec_argument` Missing Characters

**File:** `crates/crosshook-core/src/export/launcher.rs:593-598`

**Description:** The `.desktop` Exec= argument escaper handles `\`, space, and `"` but not single quotes, tabs, or `%` (which has special meaning in `.desktop` Exec lines — `%f`, `%u`, etc. are URI substitution tokens). If a trainer display name or script path somehow contained a `%` character, it would be interpreted by the .desktop launcher as a URI field code.

In practice, `script_path` is derived from `sanitize_launcher_slug()` which produces alphanumeric+hyphen only output, so this is not currently exploitable. However, `display_name` is written to the `Name=` and `Comment=` fields (not `Exec=`), so the risk is isolated.

**Mitigation:** Add `%` → `%%` replacement to `escape_desktop_exec_argument()`. Explicitly document the function's invariant (only called with slug-derived paths).

---

#### A-5: Readiness Check Path Resolution Should Avoid Env-Variable Expansion

**Description:** The first-run readiness checks will query whether the game has been launched at least once (checking for the existence of a compatdata directory), whether Proton is installed, and whether a trainer file is present. If any of these checks resolve paths by reading shell environment variables (e.g., `$HOME`, `$STEAM_ROOT`) in Rust code via `std::env::var`, those paths should be resolved before use — they should not be interpolated into shell commands.

The existing codebase uses `directories::BaseDirs` for config paths (safe) and `std::env::var` for specific vars. As long as readiness check code follows the same pattern (resolve path, use as `PathBuf`, never interpolate into shell), this is fine.

**Mitigation:** Ensure readiness check implementations use `PathBuf`-based path operations, not string-concatenation or shell expansion. Code review checklist item for this feature.

---

#### A-7: React Error Message Rendering — Confirm No `dangerouslySetInnerHTML`

**Source:** UX researcher finding #2.

**Description:** Inline validation error messages in the onboarding UI include user-supplied path strings (e.g., "Path not found: `/home/user/Trainers/`"). In React, `{errorMessage}` in JSX is escaped as DOM textContent — this is XSS-safe by default. The risk exists only if:

- `dangerouslySetInnerHTML` is used to render error strings
- A Markdown renderer processes the error message
- The path string is placed in an `href` or `src` attribute rather than text content

**Verdict:** Safe under normal React usage. The Tauri WebView does not add HTML interpretation to event payload strings.

**Mitigation:** Code review checklist for new onboarding UI components: confirm all error message rendering uses `{errorMessage}` as a text node, never `dangerouslySetInnerHTML`. This is already the project pattern in existing pages.

---

#### A-8: `suggestion` Field in Validation Response — Apply Path Sanitization

**Source:** UX researcher finding #3.

**Description:** The proposed trainer path validation response includes a `suggestion` field (e.g., "Did you mean `~/Trainers/`?") generated by the backend. If the backend generates suggestions by scanning nearby filesystem paths, those strings contain absolute paths that should be sanitized before transmission over IPC.

This is the same concern as A-6 — the fix is the same: `sanitize_display_path()` applied to all `reason` and `suggestion` strings in the `check_readiness` Tauri command response before serializing to IPC.

**Mitigation:** Same as A-6: apply `sanitize_display_path()` to all suggestion/reason strings in `commands/onboarding.rs` before returning them from Tauri commands.

---

#### A-9: "Game Launched Once" Check — Derive Path from Steam Roots, Not Profile Field

**Source:** UX researcher finding #4.

**Description:** The "game launched once" readiness check verifies that a Proton prefix exists for the configured game. This can be implemented two ways:

1. **Via `profile.steam.compatdata_path`** (profile field) — user/community-controlled; a malicious profile could point to any directory
2. **Via discovered Steam roots + app_id** — system-derived; bounded to known Steam library paths

Option 1 has a structural issue: if a user imports a community profile that sets `local_override.steam.compatdata_path` to an arbitrary path, the readiness check would call `path.exists()` on that path. The check only returns a boolean (pass/fail), so no data is exfiltrated. However, it produces a misleading readiness result — the check could pass (path exists) even though the game was never actually launched.

The existing `run_version_scan()` in `startup.rs:148` demonstrates the correct pattern: discover Steam library roots, then look for `steamapps/compatdata/{app_id}/` within those roots — never using `profile.steam.compatdata_path` for discovery.

**Mitigation:** Implement the "game launched once" check by discovering Steam roots via `discover_steam_libraries()` and testing for `steamapps/compatdata/{app_id}/pfx/` existence — consistent with how `startup.rs` already handles this. Do not use `profile.steam.compatdata_path` as the sole source for this check.

---

#### A-10: Skip/Dismiss Flows — Confirm `validate_launch()` Remains the Safety Gate

**Source:** UX researcher finding #5.

**Description:** The onboarding wizard has a "Skip setup" option. The concern is whether skipping leaves the app in a state where a partially-configured profile is auto-loaded and silently auto-launched.

**Verdict from codebase review (`startup.rs`):** Auto-load behavior only resolves the last-used profile NAME and loads it into the UI state — it does NOT trigger an automatic launch. `resolve_auto_load_profile_name()` only returns a name if the profile exists in the store; the user must still click Launch explicitly.

Skipping onboarding is safe because `validate_launch()` is called before every launch regardless of onboarding state. An incomplete profile that skipped onboarding will produce validation errors at launch time, not silent failures.

**One confirmed risk:** If "Skip setup" stores a permanent dismissed state that suppresses re-entry into the readiness check, a user with a broken trainer path would skip onboarding and receive no guidance until a confusing launch failure. This is a UX quality concern, not a security concern.

**Mitigation:** Readiness check results should be evaluated fresh when the user navigates to the Launch page (or when they click Launch), not only during the initial wizard. "Skip" dismisses the wizard UI — it does not disable the underlying `validate_launch()` gate or prevent readiness warnings from surfacing elsewhere in the UI.

---

#### A-6b: AV False Positive Warning in Onboarding Guidance

**Description:** Trainer executables (FLiNG, WeMod, etc.) routinely trigger antivirus false positives on Windows and even on Linux systems with AV tools (e.g., ClamAV) because they use memory scanning/writing techniques that resemble malware. Users running CrossHook may have AV tools installed that quarantine the trainer `.exe` before CrossHook can execute it.

This is not a CrossHook security risk, but a user-experience concern that if not addressed in onboarding guidance, will result in support requests blaming CrossHook.

**Mitigation (advisory):** Add a note in the onboarding guidance UI: "Some antivirus tools may flag trainer executables as threats — this is a known false positive with game trainers. If your trainer disappears after download, check your AV quarantine." This is a documentation/UX item, not a code change.

**Confidence:** High — this is well-documented behavior in the trainer community.

---

#### A-6: Sensitive Path Leakage in Onboarding Error Messages

**File:** `src-tauri/src/commands/shared.rs` (existing `sanitize_display_path`)

**Description:** The existing `sanitize_display_path()` function replaces the user's home directory prefix in paths surfaced to the UI with `~`. This prevents home paths from appearing in screenshot-shared error messages. Onboarding readiness check error messages (e.g., "trainer not found at `/home/yandy/...`") should use the same sanitization pattern.

**Mitigation:** Use `sanitize_display_path()` on all path strings that appear in onboarding readiness check messages returned to the frontend, consistent with how diagnostic reports are already sanitized (see `launch.rs:519-539`).

---

## Data Protection

| Data                                           | Storage                          | Protection                                       |
| ---------------------------------------------- | -------------------------------- | ------------------------------------------------ |
| Game/trainer file paths                        | TOML config                      | User-owned `~/.config/crosshook/` (0700 default) |
| Profile metadata (profile IDs, launch history) | SQLite                           | Same directory as TOML configs                   |
| Community tap subscriptions (URLs)             | SQLite + TOML                    | Same; no credentials stored                      |
| Trainer file hashes (version correlation)      | SQLite                           | SHA-256 digest only; original file not stored    |
| Launch log files                               | `~/.local/share/crosshook/logs/` | User-owned; not synced                           |

**Notes for onboarding:**

- No new sensitive data categories are introduced by the onboarding feature.
- The readiness check state (has user launched game before, which Proton version is installed) should be stored as a simple boolean/enum in SQLite metadata, not as full file paths.
- If trainer download source recommendations are stored in community tap index files, they are treated as untrusted user content — display them as plain text in the UI, never construct clickable `href` links from community-tap-sourced URLs without a security review.

---

## Dependency Security

**Current crosshook-core dependencies (relevant to onboarding):**

| Crate         | Version | Risk Assessment                                                         |
| ------------- | ------- | ----------------------------------------------------------------------- |
| `toml`        | 1.1.0   | Low — parse-only, well-audited; TOML injection not possible via `serde` |
| `rusqlite`    | 0.39.0  | Low — parameterized queries should be used for all SQLite writes        |
| `sha2`        | 0.11.0  | Low — hashing only; no network, no parse ambiguity                      |
| `tokio`       | 1.x     | Low — async runtime; no new tokio surface from onboarding               |
| `directories` | 6.0.0   | Low — XDG-compliant path resolution                                     |

**Confidence:** High (3 independent audit reports reference these crates; no open CVEs at time of research)

**For the base onboarding feature — no new dependencies are required:**

- PE header check: 2 bytes from `std::fs::File` — no new crate needed
- Readiness checks: filesystem existence checks — no new crate needed
- URL scheme validation: `str::starts_with()` — no new crate needed
- Branch name validation: regex-free char check — no new crate needed

**Proposed new dependencies from api-researcher — evaluation:**

| Crate     | Version | Verdict                       | Rationale                                                                                                                                                                                                                                                                          |
| --------- | ------- | ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `reqwest` | 0.12    | **Conditional**               | Only needed if Steam Store API / ProtonDB lookups are added. These calls are NOT in base onboarding scope. If added: default to `rustls-tls`, set connection + read timeouts (30s), treat API failures as non-fatal soft errors, never block onboarding on network unavailability. |
| `goblin`  | 0.9     | **Not needed for base scope** | Full PE parsing exceeds MZ check requirements. Add only with a specific concrete use case (see A-2b).                                                                                                                                                                              |
| `pelite`  | 0.10    | **Not needed for base scope** | Same as goblin — overspec for the advisory MZ check.                                                                                                                                                                                                                               |
| `zip`     | 2.x     | **Out of scope for v1**       | Archive extraction is NOT in the feature spec. Adding it creates a path traversal risk (W-3). Defer to a separate milestone with dedicated security review.                                                                                                                        |

**If `reqwest` is added:** confirm TLS is `rustls` (already default in 0.12), set explicit timeouts, handle `Err` gracefully — Steam Store and ProtonDB being unreachable must never block the readiness check from completing.

---

## Input Validation

### Current State (what already works)

- **Paths passed to OS commands**: use `Command::arg()` — argv-safe, not shell-interpolated ✓
- **Shell script path embedding**: `shell_single_quoted()` correctly POSIX-quotes all user paths ✓
- **Launch method validation**: allowlist check against `steam_applaunch | proton_run | native` ✓
- **Launcher slug generation**: `sanitize_launcher_slug()` strips to `[a-z0-9-]` ✓
- **Pinned commit validation**: `is_valid_git_sha()` enforces 7-64 hex chars ✓
- **Trainer path validation**: checks file existence and is-file before use ✓

### Gaps Relevant to Onboarding

| Input                  | Current Validation       | Gap                                               | Severity |
| ---------------------- | ------------------------ | ------------------------------------------------- | -------- |
| Community tap URL      | Non-empty, no whitespace | Allows `file://`, any git protocol                | WARNING  |
| Community tap branch   | Trim + empty check only  | Allows `--flag` injection into git args           | WARNING  |
| Trainer file content   | Exists + is-file check   | No PE magic check (A-2)                           | Advisory |
| Readiness check paths  | TBD (new code)           | Must use `PathBuf`, not string concat             | Advisory |
| Onboarding source URLs | N/A (new UI constant)    | Must be compile-time constant, not runtime-loaded | Advisory |

---

## Infrastructure Security

### File Permissions

Exported launcher scripts are written with mode `0o755` (executable). This is correct. Profile TOML files in `~/.config/crosshook/` inherit whatever permissions `fs::write()` produces (typically `0o644` on Linux). This is acceptable for a desktop app.

**Onboarding consideration:** If the readiness check writes any state file (e.g., `first-run-complete` marker), create it with `0o644` using the existing `write_host_text_file()` pattern, not world-writable.

### Proton Prefix Security

The `CopyToPrefix` staging path (`C:\CrossHook\StagedTrainers\`) is inside the user's own Proton prefix. The staged directory is fully wiped and recreated on each launch (`fs::remove_dir_all` + `fs::create_dir_all`). This prevents leftover trainer artifacts from previous runs. No security issue here.

### SQLite Integrity

`rusqlite` parameterized queries should be verified in all new onboarding metadata operations. The existing codebase uses parameterized queries throughout `metadata/`. Any new readiness check state persistence must follow the same pattern — never use string formatting to construct SQL.

### Log File Security

Launch logs at `~/.local/share/crosshook/logs/` include trainer stdout/stderr output routed through Proton. Trainer executables (Windows .exe) may print arbitrary content to stdout. This log content is read back and streamed to the frontend via `app.emit("launch-log", line)`. The frontend should treat log lines as plaintext, not HTML/Markdown, to prevent XSS-analogous rendering issues in React.

---

## Secure Coding Guidelines

For the trainer-onboarding feature implementation:

### 1. Git Argument Safety — W-1 (private helpers in `taps.rs`)

Add two private helper functions inside `taps.rs`, inline with `normalize_subscription()`. Do not extract to a shared module — single call site, rule-of-three not yet met. Follow the same pattern as `dedupe_preserving_order` in `diagnostics.rs`.

```rust
// In taps.rs — private helpers, not shared utilities

fn validate_tap_url(url: &str) -> Result<(), &'static str> {
    if !url.starts_with("https://") && !url.starts_with("ssh://git@") {
        return Err("tap URL must use https:// or ssh://git@");
    }
    Ok(())
}

fn validate_branch_name(branch: &str) -> Result<(), &'static str> {
    if branch.starts_with('-') {
        return Err("branch name must not start with '-'");
    }
    if !branch.chars().all(|c| c.is_alphanumeric() || "/._-".contains(c)) {
        return Err("branch name contains invalid characters");
    }
    Ok(())
}

// In normalize_subscription() — call both before constructing the result
// In git commands — use -- end-of-options separator:
command.args(["fetch", "--prune", "origin", "--", validated_branch]);
```

Cover both helpers with `#[cfg(test)]` tests in `taps.rs`.

### 2. Readiness Check Path Construction

```rust
// BAD — string concatenation
let compatdata = format!("{}/steamapps/compatdata/{}", steam_root, app_id);

// GOOD — PathBuf composition
let compatdata = PathBuf::from(steam_root)
    .join("steamapps/compatdata")
    .join(app_id.trim());
```

### 3. Trainer File Validation — A-2 (no new dependency)

```rust
// Inline in the existing trainer path validation — no new crate needed
fn looks_like_pe_executable(path: &Path) -> bool {
    use std::io::Read;
    let mut buf = [0u8; 2];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut buf))
        .map(|_| buf == [0x4D, 0x5A]) // MZ magic
        .unwrap_or(false)
}
```

Return `ValidationSeverity::Warning` (non-blocking) so users are informed but not hard-blocked.

### 4. Symlink Skip in `copy_dir_all()` — A-1 (one-line fix in `script_runner.rs`)

```rust
// In copy_dir_all(), before the is_dir() / copy branch:
if source_path.is_symlink() {
    tracing::debug!(path = %source_path.display(), "skipping symlink during trainer staging");
    continue;
}
```

Add a test case with a symlink in the source directory to the existing staging tests. No utility extraction needed — `copy_dir_all` has one call site.

### 5. Onboarding Error Message Sanitization — A-6 (usage convention, not new utility)

`sanitize_display_path()` already exists in `commands/shared.rs`. Call it before returning any error string that contains a file path from the new `commands/onboarding.rs`.

```rust
// In commands/onboarding.rs — call existing function, no new abstraction:
use crate::commands::shared::sanitize_display_path;

// sanitize_display_path() already exists — just apply it consistently
Err(format!("Trainer not found: {}", sanitize_display_path(trainer_path)))
```

---

## Trade-off Recommendations

| Finding                         | Recommended Approach                                                                                                              | Rationale                                                                          |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| W-1: Git branch injection       | Private `validate_branch_name()` + `validate_tap_url()` helpers in `taps.rs`; `--` separator in git commands                      | In-place fix, no new shared module, single call site. Add `#[cfg(test)]` coverage. |
| W-2: `file://` protocol         | Same `validate_tap_url()` helper as W-1 — both validations in one place                                                           | Zero duplication: one function, two rules                                          |
| W-3: `shell:open` capability    | All external URLs must be hardcoded frontend constants; scope `opener:open-url` with URL allowlist in `capabilities/default.json` | Permission not yet granted — configure correctly before first use                  |
| A-1: Symlink staging            | One-line `is_symlink()` skip in `copy_dir_all()` in `script_runner.rs` + new test case                                            | In-place targeted fix, no utility extraction                                       |
| A-2: PE header check            | Inline `looks_like_pe_executable()` in existing trainer path validation, return `ValidationSeverity::Warning`                     | 5 lines, no new dependency                                                         |
| A-3: Hardcoded source URLs      | Compile-time constants only — confirmed by business-analyzer and practices-researcher                                             | No code to write; architecture decision                                            |
| A-4: Desktop `%` escaping       | Add `%` → `%%` in `escape_desktop_exec_argument()`                                                                                | 1-line fix                                                                         |
| A-5: Env-var expansion          | Use `PathBuf` composition — already the project standard; code review checklist                                                   | No new code if implementation follows existing patterns                            |
| A-6/A-8: Path sanitization      | Apply `sanitize_display_path()` to all error, reason, and suggestion strings in `commands/onboarding.rs`                          | Convention, not new abstraction                                                    |
| A-7: React XSS check            | Confirm no `dangerouslySetInnerHTML` in error display components — code review checklist                                          | Already the project standard                                                       |
| A-9: "Game launched once" check | Derive compatdata path from `discover_steam_libraries()` + app_id, not from `profile.steam.compatdata_path`                       | Follows existing `startup.rs` pattern                                              |
| A-10: Skip/dismiss safety       | "Skip" dismisses wizard only; `validate_launch()` remains mandatory at launch time; readiness re-evaluates on Launch page nav     | Auto-load confirmed non-launching; no code change to existing validate gate        |

---

## Architecture Security Review

### Proposed Tauri IPC Commands (tech-designer)

Five new commands proposed: `get_onboarding_status`, `check_readiness`, `complete_onboarding_step`, `dismiss_onboarding`, `get_trainer_guidance`.

| Command                    | Security Assessment                                                                                                                                           |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_onboarding_status`    | ✅ Read-only SQLite query; no FS writes; safe                                                                                                                 |
| `check_readiness`          | ✅ Filesystem reads only; uses existing discovery functions; no new scan vectors. Apply `sanitize_display_path()` to any path in returned error strings (A-6) |
| `complete_onboarding_step` | ✅ SQLite write via parameterized query. Bound `readiness_json` to `MAX_DIAGNOSTIC_JSON_BYTES` (4KB) — tech-designer already planned this                     |
| `dismiss_onboarding`       | ✅ Simple boolean flag write; no security concerns                                                                                                            |
| `get_trainer_guidance`     | ✅ Returns static compiled-in content; no FS access; safe                                                                                                     |

**IPC registration:** All five commands must be registered in `capabilities/default.json` with only the necessary Tauri permissions. No FS shell permissions are needed for guidance/status commands — confirm none are over-permissioned.

### `onboarding_progress` SQLite Table Design

Tech-designer proposed: `id TEXT PK`, `stage TEXT NOT NULL`, `readiness_json TEXT`, `completed_at TEXT`, `created_at/updated_at TEXT NOT NULL`.

**Assessment:**

- ✅ `readiness_json` bounded to 4KB — correctly mirrors `MAX_DIAGNOSTIC_JSON_BYTES` pattern
- ✅ Inherits existing `db.rs` protections: `secure_delete=ON`, WAL mode, `0o600` file perms, `0o700` dir perms, symlink rejection
- ✅ No sensitive path data in `readiness_json` — store check results as boolean/enum flags only (confirmed by business-analyzer)
- ✅ `stage TEXT NOT NULL` — validate on read with an allowlist of known stage names before acting on the value, not just on write

**Minor note:** Ensure the `stage` column values are validated against an enum or allowlist when read back for display — never interpolate a raw DB string value into a UI command or file path.

### Staging Path Traversal Analysis (Response to business-analyzer)

**Question:** Does `stage_trainer_into_prefix` verify the staging destination is within the expected prefix path before writing?

**Analysis:** The current implementation is safe by construction, not by explicit boundary check:

1. `trainer_base_name` = `trainer_host_path.file_stem()` — this strips all directory components. `file_stem()` returns only the final path component stem with no separators. A path like `/trainers/../../../etc/passwd.exe` produces `file_stem()` = `"passwd"`, not `"../../../etc/passwd"`.

2. The staged path is: `wine_prefix_path/drive_c/CrossHook/StagedTrainers/{trainer_base_name}/{trainer_file_name}` — `trainer_base_name` is a pure filename OsStr with no `/` or `\` characters. `PathBuf::join()` with a pure filename cannot escape the prefix.

3. On Linux filesystems, files cannot be named `..` or contain null bytes — these would be rejected at the `fs::metadata()` call long before reaching `stage_trainer_into_prefix`.

**Verdict:** Staging destination is safe. The safety is structural (use of `file_stem()` stripping directories) rather than an explicit assertion. An explicit `assert!(staged_directory.starts_with(&staged_root))` guard would make this more legible to future readers, but is not strictly required.

---

## Open Questions

1. **RESOLVED — Trainer source URLs:** Business-analyzer and practices-researcher confirm trainer guidance (FLiNG, WeMod, etc.) is static text compiled into the binary. Not loaded from community taps or SQLite at runtime. ✅

2. **RESOLVED — Readiness state storage:** Business-analyzer confirmed: boolean/enum flags only in SQLite, no file paths. Tech-designer confirmed: `onboarding_progress` table with bounded `readiness_json` (4KB). ✅

3. **OPEN — Does the guided workflow expose a "skip" path that bypasses launch validation?** Ensure any "skip for now" or "launch anyway" button in the wizard still invokes the existing `validate_launch` Tauri command. A wizard skip must only bypass guidance steps, not the underlying safety gate.

4. **OPEN — Pre-recommended community tap URLs in onboarding?** If the onboarding wizard displays suggested tap URLs for users to add, those must be hardcoded binary constants and reviewed for trustworthiness before each release. Do not pull suggested tap URLs from a remote source.

5. **OPEN — Is `reqwest` (Steam Store API / ProtonDB) in scope for this feature?** Api-researcher proposes network API calls for enhanced readiness checks. Base feature spec does not require network calls. If added, network calls must be async non-blocking, fail gracefully, have timeouts, and must not gate the wizard on network availability.

6. **OPEN — Is zip archive extraction in scope?** Api-researcher proposes extracting FLiNG zip archives. Base feature spec does not include this. If added: mandatory dedicated security review for W-3 (zip path traversal) before implementation.

7. **OPEN — `stage` column allowlist validation:** When reading the `stage` column from `onboarding_progress` table to determine workflow position, validate against a known enum of stages. Do not branch on raw DB string values without validation.
