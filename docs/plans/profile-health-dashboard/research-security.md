# Security Research: Profile Health Dashboard

**Date**: 2026-03-27
**Scope**: CrossHook native Linux desktop app (Tauri v2 / Rust backend)
**Feature**: Batch-validate filesystem paths stored in all saved profiles; surface per-profile health status with remediation suggestions

---

## Executive Summary

The profile health dashboard is a low-risk feature for this threat model. CrossHook is a local desktop app with no network exposure; the user who runs the app has full access to every path the health check would inspect. The main risks are:

1. **CSP is disabled** (`"csp": null` in `tauri.conf.json`) — predates this feature but becomes slightly more relevant as the IPC surface grows.
2. **Remediation messages may emit raw absolute paths** — the existing `sanitize_display_path()` helper must be reused to replace `$HOME` with `~`.
3. **No new crate risk** — all required filesystem metadata APIs are in `std`. Zero new dependencies needed.
4. **Path validation is already solid** — `validate_name()` in `toml_store.rs` prevents traversal via profile names. Profile-content paths (game_path, trainer_path, etc.) are user-controlled and are already used at launch time; health-checking them is equivalent in access scope.

No CRITICAL blockers identified. The feature is implementable securely with the advisories below applied.

---

## Findings by Severity

### CRITICAL — Hard Stops

**None.** The local desktop threat model does not surface any showstopper security issues for this feature. Path existence checks on user-configured paths are functionally equivalent to what the launch validation already does.

---

### WARNING — Must Address, Alternatives Welcome

#### W-1: CSP is disabled (`"csp": null`)

**File**: `src-tauri/tauri.conf.json:23`

The current Tauri configuration sets `"csp": null`, disabling Content Security Policy for the WebView. This means any XSS that reaches the WebView (via a maliciously crafted community profile name rendered without escaping, a future embedded web view, etc.) can invoke any registered Tauri command without restriction.

The health dashboard adds new IPC commands that perform filesystem metadata lookups. With CSP disabled, a successful XSS could batch-probe arbitrary paths by invoking these commands.

**Note**: This is an existing issue, not introduced by this feature. However, expanding the IPC surface without CSP increases the attack footprint.

**Mitigations** (team may choose one):

- **Preferred**: Enable CSP before or alongside this feature: `"csp": "default-src 'self'; script-src 'self'"` in `tauri.conf.json`. Tauri v2 supports this directly.
- **Acceptable alternative**: Add it to the tech debt backlog with a documented decision noting the local-app threat model.
- **Not acceptable**: Leaving it undocumented.

**Confidence**: High — `tauri.conf.json:23` confirms `"csp": null`. Tauri v2 CSP support confirmed by official docs.

---

#### W-3: Diagnostic bundle export must sanitize health report paths

_(Added after cross-team review)_

If health report data flows into a future shareable diagnostic bundle (e.g., issue #49), raw absolute paths in the report expose the user's home directory, username, and installed software layout. This is a higher-risk surface than the UI because the data leaves the local machine.

**Mitigation**: Apply `sanitize_display_path()` to all path fields before they enter any export or share pipeline. If diagnostic bundle integration is out of scope for this feature, document it as a hard dependency: "health report paths must be sanitized before inclusion in any diagnostic bundle."

**Confidence**: High.

---

#### W-2: Remediation messages must not emit raw absolute paths for community-sourced profiles

Health check remediation suggestions will say things like "path `/home/user/.steam/...` not found". If a profile was imported from a community tap with crafted paths (e.g., pointing at `/etc/shadow`, `/root/...`), the health message would confirm whether those paths exist on the local system.

Within the single-user local threat model this is low impact — the user already has access to all their own paths. But:

- It leaks filesystem layout in log output or any future crash reporting.
- It confirms path existence to any script that can read the IPC response (relevant if CSP is not fixed).

**Mitigation**: Reuse the existing `sanitize_display_path()` function (defined in `src-tauri/src/commands/launch.rs:301-306`) for all path strings in health status results and remediation messages. This function already replaces `$HOME` with `~`. It should be moved to `src-tauri/src/commands/shared.rs` — this is a display-formatting utility for the Tauri command layer, not business logic, so it belongs alongside `create_log_path()` and `slugify_target()` rather than in `crosshook-core`. The health dashboard command calls it the same way `sanitize_diagnostic_report()` does in the launch flow.

**Confidence**: High — function exists and works correctly; pattern is already validated in the launch diagnostic pipeline.

---

### ADVISORY — Best Practices, Safe to Defer

#### A-1: Permission errors must be distinguished from "not found"

When calling `std::fs::metadata(path)`, the error kind can be:

- `io::ErrorKind::NotFound` — file/directory does not exist
- `io::ErrorKind::PermissionDenied` — exists but cannot be read by this process

The health status model should represent three states, not two:

- `Healthy` — path exists and has the expected type (file/dir/executable)
- `Missing` — `ENOENT`
- `Inaccessible` — `EACCES` or other I/O error

Collapsing `Inaccessible` into `Missing` gives wrong remediation advice ("re-browse to the file" when the real fix is "fix permissions").

**Confidence**: High — standard Rust I/O error semantics.

---

#### A-2: Symlink-following behavior is correct but should be documented

Rust's `Path::exists()` and `std::fs::metadata()` follow symlinks (using `stat` not `lstat`). For a health check, this is the correct behavior — we care whether the game executable at the end of the chain exists, not whether each symlink in the chain is valid.

Edge case: a symlink pointing at a broken target returns `NotFound` from `metadata()`, which is the correct health status (`Missing`). `symlink_metadata()` would return `Ok` for the dangling symlink itself.

**Recommendation**: Use `std::fs::metadata()` (follows symlinks) rather than `fs::symlink_metadata()`. Add a code comment explaining the choice.

**Confidence**: High — Rust stdlib docs confirm this behavior.

---

#### A-3: TOCTOU is inherent but non-exploitable for a status-only feature

Any "check then display" pattern has a TOCTOU gap — the filesystem state can change between the check and the display. For a health dashboard this is acceptable because:

- The result is advisory only; it drives no automated action.
- The "window" is milliseconds on local storage.
- A local adversary who can race the check to change file existence can already do so via direct filesystem access.

**Recommendation**: Display a "last checked at" timestamp in the UI and do not persist health status to disk. This makes the staleness explicit to the user.

**Confidence**: High — well-understood limitation of all health-check UIs.

---

#### A-4: Mutex poison handling if a health cache is introduced

_(Added after cross-team review)_

If a health cache is added (e.g., `Arc<Mutex<HashMap<String, ProfileHealthStatus>>>`), a panic while the lock is held poisons the Mutex. Subsequent `lock()` calls return `Err(PoisonError)`, which would make all future health checks fail silently if `.unwrap()` is used.

**Recommendation**: If a cache is used, either:

- Use `RwLock` (preferred for read-heavy access: one writer, many readers) and handle `PoisonError` explicitly with `unwrap_or_else(|p| p.into_inner())`
- Or avoid caching entirely — `metadata()` calls are fast enough to re-run on demand

**Note**: The simplest safe design has no cache at all; health checks run on demand and results are not stored.

**Confidence**: High — standard Rust concurrent data structure concern.

---

#### A-5: Batch validation should be sequential or rate-limited, not fully concurrent

Checking all profiles concurrently via `tokio::join!` or spawned tasks on a system with hundreds of profiles and slow storage (SD card, network-backed home) could cause momentary I/O pressure. This is not a security risk but can affect user experience.

**Recommendation**: Process profiles sequentially or with a bounded concurrency (e.g., 4 concurrent `metadata()` calls via `tokio::Semaphore`). Since `metadata()` is fast on local SSD/NVMe, sequential is simplest and fine for most cases.

**Confidence**: Medium — depends on how many profiles users accumulate and storage type.

---

#### A-6: IPC result type must not include raw profile file paths

The health check IPC response will include health status per profile. Profile names (e.g., `"Elden Ring"`) are safe to include — they pass `validate_name()`. However, if the response includes the raw filesystem paths from inside the profile content (game_path, trainer_path, etc.) to support frontend display, those paths must be passed through `sanitize_display_path()` first.

If the frontend only needs to know _which_ profiles are unhealthy and _what type of path_ is broken (game executable, trainer, Proton, etc.) without the raw path value, that is the safest IPC contract.

**Recommendation**: The IPC response should use an enum-tagged field indicating what kind of path is problematic (`GameExecutable`, `TrainerExecutable`, `ProtonBinary`, `CompatdataDir`, `WinePrefix`) rather than returning raw path strings to the frontend. The frontend then generates a user-friendly label without needing the raw path. If raw paths are needed for display, apply `sanitize_display_path()` server-side before serializing.

**Confidence**: High.

---

## Path Validation Security

### Profile Name Traversal

`validate_name()` in `profile/toml_store.rs:300-325` is robust:

- Rejects empty, `.`, `..`
- Rejects absolute paths and paths containing `/`, `\`, `:`
- Rejects Windows reserved path characters

The health check IPC will accept a profile name (for single-profile check) or no name at all (for batch check). In both cases, the profile name flows through `ProfileStore::load()` which calls `profile_path()` which calls `validate_name()`. **No additional validation needed.**

### Profile Content Path Traversal

Paths stored _inside_ profiles (`game.executable_path`, `trainer.path`, `steam.compatdata_path`, `steam.proton_path`, `runtime.prefix_path`, etc.) are user-entered filesystem paths. These:

- Can be absolute paths anywhere on the filesystem (correct and expected — games install anywhere)
- Can contain `../` sequences (nothing prevents this, but there is no attack surface since we only call `metadata()` on them)
- Cannot be restricted to a safe prefix without breaking legitimate use cases

The health check treats these paths identically to how the launch validation treats them: call `std::fs::metadata(path)` and check the result. The launch validator (`launch/request.rs`) already calls `Path::exists()` and `Path::is_file()` on these same paths. The health dashboard does the same — no expanded attack surface relative to existing code.

**Assessment**: No path traversal risk for a read-only metadata check. The concern only arises if the health check were to _read file contents_ from these paths (it should not).

### Symlink Security

Symlink following (via `metadata()`) is correct behavior and not exploitable in the local threat model. A symlink to `/etc/passwd` would report as "exists, is file" — the health check never reads the content.

---

## Information Disclosure

### What Health Status Reveals

Health status reveals whether user-configured paths exist on the filesystem. This is:

- **Intentional**: the entire purpose of the feature.
- **Non-sensitive in the single-user local threat model**: the user already has access to all their own paths.
- **Potentially sensitive if IPC is abused**: if CSP is disabled and XSS occurs, an attacker could probe arbitrary paths by calling the health check IPC with crafted profile names. **W-1 (CSP) covers this.**

### Remediation Suggestions

Remediation suggestions like "the game executable was not found — re-browse to the file" are safe. They should not embed the raw path in the IPC response unnecessarily. See **A-5** for the recommended IPC contract.

---

## File System Access

### Scope

The health check requires read-only access to:

1. `~/.config/crosshook/profiles/*.toml` — already accessed by all profile commands
2. Arbitrary absolute paths stored in profile content — read-only `metadata()` calls only

No writes occur during health checking. The `ProfileStore` is not modified. Profile TOML files are not written.

### Enforcing Read-Only

Rust has no `O_RDONLY`-equivalent enforcement at the type level for filesystem metadata checks. The read-only guarantee is by convention: the health check implementation must only call:

- `std::fs::metadata(path)` or `path.exists()` / `path.is_file()` / `path.is_dir()`
- `std::os::unix::fs::PermissionsExt::mode()` (for executable bit checks)

It must never call `fs::write()`, `fs::remove_file()`, `fs::rename()`, or any function that opens a write file descriptor on the checked paths.

**Recommendation**: The health check logic should follow the existing plain-function pattern used in `launch/diagnostics/` — a single function `check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthInfo` that builds and returns a result struct directly. Do not introduce a collector object (e.g., `HealthCollector` with `add_issue()` / `finalize()`) — this abstraction does not exist elsewhere in the codebase and the plain-function style is the established convention. The read-only constraint should be documented in a module-level doc comment: "all operations in this module are read-only metadata checks — no writes, no execution."

### Permission Handling

If `metadata()` returns `PermissionDenied`, the health status should be `Inaccessible` (see **A-1**), not a panic or unhandled error. The batch check must continue to the next profile and path even if one check fails.

---

## Dependency Security

### New Crates Required

**None.** All required functionality is available in the Rust standard library:

| Need                 | API                                         |
| -------------------- | ------------------------------------------- |
| Check path existence | `std::fs::metadata(path)`                   |
| Check is file        | `Metadata::is_file()`                       |
| Check is directory   | `Metadata::is_dir()`                        |
| Check executable bit | `std::os::unix::fs::PermissionsExt::mode()` |
| Async batch checking | `tokio` — already a dependency              |

Zero new crates means zero new supply chain risk, zero new audit surface, and zero maintenance overhead.

### Existing Dependency Health

The current `crosshook-core` dependencies are all well-maintained crates with no known high-severity CVEs as of 2026-03-27:

- `chrono 0.4` — stable, no outstanding CVEs
- `directories 5` — minimal, stable
- `serde 1 / serde_json 1 / toml 0.8` — foundational, heavily audited
- `tokio 1` — foundational, actively maintained
- `tracing 0.1 / tracing-subscriber 0.3` — stable

No dependency additions are required or recommended for this feature.

---

## IPC Security

### New Tauri Command Surface

The health dashboard will likely add one or two new Tauri commands:

| Command                              | Input                    | Output                     |
| ------------------------------------ | ------------------------ | -------------------------- |
| `profile_health_check_all`           | None                     | `Vec<ProfileHealthReport>` |
| `profile_health_check(name: String)` | Profile name (validated) | `ProfileHealthReport`      |

**Input validation**: Profile name passes through `ProfileStore` which calls `validate_name()`. ✓
**Output safety**: Must apply `sanitize_display_path()` to any path strings before serialization. See **W-2** and **A-5**.

### Capabilities

The existing `capabilities/default.json` grants `core:default` and `dialog:default`. The health dashboard requires neither `fs:read` nor any new capability — `std::fs::metadata()` does not require a Tauri plugin capability (Tauri filesystem plugin is only needed for JavaScript-side file I/O; Rust-side I/O in Tauri commands is unrestricted).

**No new Tauri capabilities needed.**

### Event Emission

If the health check streams progress updates (e.g., "checked 3 of 12 profiles"), it should emit Tauri events rather than polling IPC. Event payloads must similarly avoid raw paths. Use profile names (validated) and structured status enums.

---

## Secure Coding Guidelines

For the health check implementation:

1. **Use `std::fs::metadata(path)` — not `path.exists()`**: `metadata()` returns `Result<Metadata, io::Error>` and lets you distinguish `NotFound` from `PermissionDenied`. `path.exists()` swallows the error and returns `false` for both. Map error kinds as follows:

   ```rust
   match std::fs::metadata(path) {
       Ok(meta)  => { /* check is_file(), is_dir(), mode bits */ }
       Err(e) => match e.kind() {
           io::ErrorKind::NotFound         => PathStatus::Missing,
           io::ErrorKind::PermissionDenied => PathStatus::Inaccessible,
           _                               => PathStatus::Inaccessible, // unknown OS error
       }
   }
   ```

   Note: dangling symlinks return `NotFound` (not `PermissionDenied`) because `metadata()` follows symlinks. The catch-all `_` maps to `Inaccessible` rather than `Missing` because the path's existence cannot be confirmed.

2. **Never read file content from profile paths**: `metadata()` only. Do not open any file descriptor for reading on game_path, trainer_path, etc. The health check must never call `fs::read_to_string()` on user-configured paths.

3. **Return structured status, not raw error strings**: The IPC result type should be an enum (`Healthy`, `Missing`, `Inaccessible`, `NotConfigured`) rather than a raw `io::Error` string, to prevent leaking OS error messages that may contain path information.

4. **Apply `sanitize_display_path()` to all path strings in IPC responses**: Move this function to `src-tauri/src/commands/shared.rs` (not `crosshook-core` — it is a display-formatting concern for the command layer, not business logic). The health dashboard command calls it the same way `sanitize_diagnostic_report()` does in the launch flow.

5. **Profile batch check must not fail on single path error**: Use `match metadata(path) { Ok(m) => ..., Err(e) => log and continue }`. Never use `?` to propagate errors out of the per-path check loop.

6. **Check executable bit on Linux using `PermissionsExt`**: For Proton binary and game executable paths, checking `is_file()` alone is insufficient — a file can exist but not be executable. Use `metadata.permissions().mode() & 0o111 != 0` for executable bit checks. This matches what the OS actually enforces at launch time.

7. **Log at `debug` level, not `info`**: Health check results for individual paths should be logged at `debug` level only. Do not log raw paths at `info` or `warn` level as they appear in tracing output.

---

## Trade-off Recommendations

| Trade-off                                                       | Recommended Decision                                                                                                                                                    |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CSP: enforce now vs. defer                                      | Enforce alongside this feature (W-1). The implementation cost is a one-line change to `tauri.conf.json` and testing that existing commands still work.                  |
| Raw paths in IPC: include vs. omit                              | Omit raw paths from IPC; use enum-tagged field types (A-5). Frontend can display human-readable labels without knowing the raw path.                                    |
| Batch concurrency: sequential vs. async                         | Sequential for simplicity; add bounded concurrency only if profiling reveals it matters (A-4).                                                                          |
| Path scope restriction: restrict to known dirs vs. unrestricted | Unrestricted — users install games anywhere. Restricting would break legitimate configurations. Accept the "existence confirmation" information disclosure as inherent. |
| Error distinction: 2-state vs. 3-state                          | 3-state (`Healthy` / `Missing` / `Inaccessible`) — the implementation cost is negligible and the UX improvement is significant (A-1).                                   |

---

## Open Questions

1. **Should health check persist results to disk?** Persisting would enable "health last checked on boot" UX but introduces a write path and staleness risk. Recommendation: do not persist; always check live on demand.

2. **Should the IPC support partial re-check (single profile) or only batch?** Both are useful. The single-profile variant is needed for post-remediation "re-check" UX. Both should go through the same underlying `check_profile_health(profile: &GameProfile)` function.

3. **How should the UI display `Inaccessible` vs `Missing`?** This is a UX decision, but the IPC layer must distinguish them. Coordinate with ux-researcher.

4. **CSP enforcement scope**: If CSP is enabled, will the existing `devUrl: "http://localhost:5173"` dev setup require a CSP exception? Yes — `script-src 'self' 'unsafe-eval'` may be needed for Vite dev mode. Production AppImage should use strict CSP.

5. **Should `Optional` paths (dll_paths with empty strings) be health-checked?** Profile fields can be empty strings (opt-out). The health check should skip empty-string paths with a `NotConfigured` status rather than reporting them as `Missing`.
