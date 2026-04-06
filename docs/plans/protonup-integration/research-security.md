# Security Research: ProtonUp Integration

**Feature**: ProtonUp-Qt / protonup-rs integration for Proton version management
**Researcher**: Security Specialist
**Date**: 2026-04-06

---

## Executive Summary

The ProtonUp integration feature carries **three CRITICAL findings**: a chain of three CVEs in `astral-tokio-tar` that are only fully addressed by `astral-tokio-tar >= 0.6.0` (meaning the required `libprotonup` version must be verified to pull `astral-tokio-tar = "0.6"` per its own `Cargo.toml`), a user-supplied `install_dir` path traversal vector in the `install_proton_version` API surface, and the archive bomb risk (no decompressed size or file count limits). Five WARNING findings and several ADVISORY improvements round out the picture.

The feature is viable and secure to ship once the CRITICAL findings are resolved. The `libprotonup` main branch (`Cargo.lock` confirmed: `astral-tokio-tar = "0.6.0"`) is correctly patched. Use the crates.io release that locks to `astral-tokio-tar = "0.6"` in its own `Cargo.toml` (currently `libprotonup = "0.11.0"`).

**Key finding summary:**

| Severity | Count | Resolved by                                                                                                          |
| -------- | ----- | -------------------------------------------------------------------------------------------------------------------- |
| CRITICAL | 3     | Pin `libprotonup = "0.11.0"` (uses `astral-tokio-tar = "0.6"`); validate `install_dir`; enforce decompression limits |
| WARNING  | 5     | Implementation-time mitigations                                                                                      |
| ADVISORY | 5     | Deferred improvements                                                                                                |

> **2026-04-06 update**: Original C-1 (CVE-2025-62518) was understated. Two additional CVEs were found in `astral-tokio-tar`: CVE-2025-59825 (symlink TOCTOU path traversal, fixed in 0.5.4) and CVE-2026-32766 (PAX extension validation, fixed in 0.6.0). The minimum safe version is `astral-tokio-tar >= 0.6.0`. New CRITICAL findings C-2 (user-supplied `install_dir` path traversal) and C-3 (archive bomb) added based on tech-designer's API surface review.

---

## Findings by Severity

### CRITICAL Findings

| ID  | Finding                                                                                                                                                                                                                                                                | Component                                     | CVE/Advisory                                   | Mitigation                                                                                                                                                                   |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- | ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C-1 | Three-CVE chain in `astral-tokio-tar`: PAX header desync (CVE-2025-62518, fixed in 0.5.6), symlink TOCTOU path traversal (CVE-2025-59825, fixed in 0.5.4), PAX extension validation (CVE-2026-32766, fixed in 0.6.0). All allow writes outside `compatibilitytools.d/` | `tokio-tar` (all), `astral-tokio-tar` < 0.6.0 | CVE-2025-62518, CVE-2025-59825, CVE-2026-32766 | Pin `libprotonup` at version whose `Cargo.toml` declares `astral-tokio-tar = "0.6"`. Currently `libprotonup = "0.11.0"`. Verify `Cargo.lock` shows `astral-tokio-tar 0.6.x`. |
| C-2 | `ProtonInstallRequest.install_dir` is a user-supplied path with no documented validation. If passed directly to `libprotonup::files::unpack_file`, extraction can target any user-writable directory                                                                   | `install_proton_version` Tauri command        | —                                              | Validate `install_dir` is within the user's home directory or an approved Steam library path. Reject absolute paths outside `~` and all paths containing `..`.               |
| C-3 | No decompression size limit or file count limit on archive extraction. A crafted 300 MB tar.gz could expand to fill the disk (archive bomb). `libprotonup`'s `unpack_file` does not document any expansion ratio cap                                                   | Archive extraction pipeline                   | —                                              | Enforce max extracted size (e.g., 10× compressed or 4 GB hard cap) and max file count (e.g., 10,000 entries). Abort and clean up if limits are exceeded.                     |

### WARNING Findings

| ID  | Finding                                                                                                                                   | Component                             | Impact                                                                                  | Mitigation                                                                                           |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| W-1 | No rate limit handling for GitHub API at 60 req/hr for unauthenticated requests                                                           | GitHub Releases API                   | Cache miss storms could trigger API bans; degraded UX when rate-limited                 | Implement exponential backoff; respect `X-RateLimit-*` headers; enforce TTL cache                    |
| W-2 | Partial downloads leave temp files if process is killed via SIGINT/SIGTERM                                                                | `tempfile` crate, extraction pipeline | Disk space leak; potential for partial/corrupted archives to be mistakenly used         | Explicit cleanup in signal handlers; use `tempfile()` (OS-backed) rather than `NamedTempFile`        |
| W-3 | Checksum file itself downloaded from same server without secondary trust anchor                                                           | SHA-512 verification workflow         | If GitHub CDN is compromised, attacker controls both binary and checksum                | Document accepted risk; advisory GPG verification path                                               |
| W-4 | No defense-in-depth path validation on extracted entries in the CrossHook integration layer                                               | Extraction step                       | A crafted archive could still escape even with a patched tar library (defense in depth) | Validate each extracted entry path against destination root before writing                           |
| W-5 | Install target directory not checked for symlink before extraction (pattern exists in `db.rs` but not applied to `compatibilitytools.d/`) | Filesystem write path                 | Symlinked install dir could redirect extraction outside the Steam root                  | Use `fs::symlink_metadata()` + `is_symlink()` check before any write, mirroring `db.rs:open_at_path` |

### ADVISORY Findings

| ID  | Finding                                                                          | Component                   | Recommendation                                                                         |
| --- | -------------------------------------------------------------------------------- | --------------------------- | -------------------------------------------------------------------------------------- |
| A-1 | No GPG signature verification beyond SHA-512                                     | GE-Proton release assets    | GloriousEggroll does not provide GPG-signed releases; SHA-512 is the available option  |
| A-2 | GitHub API accessed without authentication token                                 | Release listing             | Provide optional token config; document 60 req/hr limit                                |
| A-3 | JSON deserialization from GitHub API not using `deny_unknown_fields`             | GitHub release JSON parsing | Add `#[serde(deny_unknown_fields)]` where feasible; validate version string format     |
| A-4 | Version strings from GitHub/cache not validated against allowlist before display | UI rendering                | Sanitize version tags to `[0-9A-Za-z.\-_]+` before showing in UI or constructing paths |
| A-5 | No TLS pinning or minimum TLS version enforcement                                | `reqwest`/`rustls`          | Document reliance on system/rustls defaults; rustls already enforces TLS 1.2+          |

---

## Section 1: Binary Download Verification

### SHA-512 Checksum (Not SHA-256)

**Confidence: High** — GloriousEggroll releases include `.sha512sum` files alongside `.tar.gz` tarballs. The official README documents verification via `sha512sum -c $checksum_name`. `libprotonup` implements this natively using the `sha2` crate.

The `libprotonup` download workflow is:

1. Download tarball to temp file
2. Download matching `.sha512sum` file
3. Call `hash_check_file(file_name, &mut file, expected_hash).await?`
4. Only proceed to `unpack_file()` if hash passes

This is the correct approach. **Do not short-circuit the hash check.**

### GPG Signatures

**Confidence: High** — GloriousEggroll does **not** provide GPG-signed release artifacts. No GPG public key is published for verification. SHA-512 from the same GitHub release is the only available integrity mechanism. This is a known ecosystem gap, not a CrossHook-specific issue.

Mitigation accepted: SHA-512 verification is mandatory; GPG is not available.

### HTTPS Enforcement

`libprotonup` uses `reqwest` with `rustls` by default. `rustls` enforces:

- TLS 1.2 minimum (TLS 1.0/1.1 are rejected)
- Certificate chain validation
- No `danger_accept_invalid_certs` in production paths

CrossHook must not override these defaults. Do not expose `accept_invalid_certs` as a config option.

### Man-in-the-Middle Risk

HTTPS with `rustls` protects against network MITM. The remaining trust chain is: GitHub CDN → tarball + checksum. If GitHub's CDN is compromised at the source, both files could be replaced consistently. This is the same trust model used by every package manager that does not do GPG signing; it is accepted practice for this class of tooling.

---

## Dependency Security

### libprotonup / protonup-rs

**Confidence: High** — Actively maintained (v0.11.0, March 2026, 25 releases, 11+ contributors). Uses Rust 2024 edition. OpenSSF Best Practices badge present. GPL-3.0 licensed.

**Key dependencies:**

- `sha2` — cryptographic hash verification
- `reqwest` with `rustls` — HTTPS downloads
- `async-compression` — gzip/xz/zstd
- `tokio` — async runtime
- `tempfile` — temporary file management

### Three-CVE Chain in `astral-tokio-tar` — CRITICAL (C-1)

Three separate CVEs affect the `astral-tokio-tar` extraction library used by `libprotonup`. All are resolved only by `astral-tokio-tar >= 0.6.0`.

**CVE-2025-62518 (TARmageddon) — fixed in 0.5.6, CVSS 8.1**
PAX/ustar header size desynchronization. The parser uses the ustar header size (often zero) instead of the PAX-specified size to advance the stream, allowing smuggled archive entries to be written anywhere. `protonup-rs` v0.9.1 migrated from abandoned `tokio-tar` to `astral-tokio-tar` to address this, but the subsequent CVEs show the migration target itself was not yet fully safe.

**CVE-2025-59825 (Symlink TOCTOU Path Traversal) — fixed in 0.5.4**
The `Entry::unpack_in_raw` API memoizes validated directory paths but does not invalidate the cache when a symlink entry modifies the filesystem hierarchy (TOCTOU race). Two chained symlinks that individually pass validation can combine to point outside the extraction directory. The `allow_external_symlinks` default of `true` is part of the attack surface. This is the tech-designer's concern about `libprotonup::files::unpack_file` and `entry.unpack()` following symlinks — confirmed vulnerability.

**CVE-2026-32766 (PAX Extension Validation) — fixed in 0.6.0**
Malformed PAX extensions silently skipped during archive parsing. Allows crafted archives to bypass extension-level validation.

**Status**: `libprotonup` main branch `Cargo.toml` declares `astral-tokio-tar = "0.6"` and `Cargo.lock` confirms `astral-tokio-tar 0.6.0`. This covers all three CVEs.

**Required Cargo.toml entry:**
```toml
libprotonup = "0.11.0"  # Cargo.toml requires astral-tokio-tar = "0.6"
```

**Verification after adding the dependency:**
```bash
cargo audit --manifest-path src/crosshook-native/Cargo.toml
# Expected: no vulnerabilities
grep "astral-tokio-tar" src/crosshook-native/Cargo.lock
# Expected: version = "0.6.x" — NOT 0.5.x
# Must NOT appear: name = "tokio-tar" (the abandoned original crate)
```

### reqwest TLS Configuration

`reqwest` with `rustls` is memory-safe and does not link to OpenSSL or system TLS libraries on Linux. This is preferable for an AppImage distribution where system TLS library availability varies. Default configuration provides correct certificate validation; do not override.

### Supply Chain Risk

`libprotonup` has approximately 554K SLoC in upstream dependencies. This is typical for an async Rust project with HTTP and compression capabilities. Mitigate via periodic `cargo audit` in CI.

---

## Section 3: Filesystem Security

### `install_dir` Path Traversal — CRITICAL (C-2)

`ProtonInstallRequest.install_dir` is a user-supplied optional string. If non-empty, it overrides the default install location. If passed without validation to `libprotonup::files::unpack_file`, an attacker (or a bug) can direct extraction to:
- An arbitrary absolute path (e.g., `/etc/cron.d/`, `~/.config/autostart/`)
- A relative path with `..` traversal (e.g., `../../.config/autostart/`)

**Required validation before calling `unpack_file`:**
```rust
fn validate_install_dir(install_dir: &Path, steam_roots: &[PathBuf]) -> Result<(), InstallError> {
    // Resolve without following final symlink
    let canonical = install_dir.canonicalize()
        .map_err(|_| InstallError::InvalidInstallDir)?;

    // Must be within the user's home directory
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(InstallError::InvalidInstallDir)?;

    if !canonical.starts_with(&home) {
        return Err(InstallError::InvalidInstallDir);
    }

    // Additionally: must be within a known Steam compatibilitytools.d path
    let approved = steam_roots.iter().any(|root| {
        canonical.starts_with(root.join("compatibilitytools.d"))
    });
    if !approved {
        return Err(InstallError::InvalidInstallDir);
    }

    Ok(())
}
```

If `install_dir` is empty/not provided, derive it exclusively from the discovered Steam root — never from user free-form input.

### Archive Bomb Protection — CRITICAL (C-3)

GE-Proton tarballs are legitimately 300–600 MB compressed, expanding to 1–3 GB. However, a crafted archive could include an extreme expansion ratio or an unbounded number of files. `libprotonup`'s `unpack_file` does not document size or file count limits.

**Required mitigations in the CrossHook integration wrapper:**
- Track cumulative extracted bytes. If the total exceeds a hard cap (recommend 8 GB for GE-Proton's expected range with margin), abort extraction and clean up the temp directory.
- Track file count. If entries exceed a cap (recommend 50,000 for GE-Proton's actual ~2,000–5,000 files with margin), abort.
- On abort, log the violation and surface a clear user-facing error.

These limits must be enforced in CrossHook's wrapper around `unpack_file`, since they cannot be relied upon inside `libprotonup`.

### Writing to `compatibilitytools.d/`

The target directory `~/.local/share/Steam/compatibilitytools.d/` (or equivalents) is user-owned and under the user's `$HOME`. No privilege escalation is required or should be attempted.

**Risks and mitigations:**

1. **Incorrect target directory**: Construct the path from a known Steam root, not from user-controlled input. Canonicalize and validate the path before extraction (see C-2 above).

2. **Symlink following in the target directory**: CVE-2025-59825 confirms this is a real attack vector, not just a theoretical concern. With `astral-tokio-tar >= 0.6.0` this is mitigated at the library level, but CrossHook should also validate each entry path before extraction (W-4) and check the install directory itself for symlinks (W-5).

3. **Root ownership edge case**: The directory can become root-owned in some configurations. CrossHook should check write permissions before starting a download and surface a clear error (not a generic OS error) if the directory is not writable.

### Path Validation for Extracted Entries (WARNING W-4)

Even with a patched tar library, defense in depth requires verifying extracted paths. Before writing each entry, check that the fully resolved path starts with the intended install directory:

```rust
fn is_safe_extraction_path(entry_path: &Path, install_root: &Path) -> bool {
    let canonical_root = install_root.canonicalize().unwrap_or_else(|_| install_root.to_path_buf());
    let full_path = canonical_root.join(entry_path);
    // Resolve ".." components without following symlinks
    let resolved = normalize_path(&full_path);
    resolved.starts_with(&canonical_root)
}
```

Reject any archive entry that would extract outside `install_root`. Log the violation and abort the extraction.

### Temp File Handling

`libprotonup` uses the `tempfile` crate. The `tempfile()` function (unnamed temp file) relies on the OS to clean up on process exit, including abnormal exits. This is the preferred approach.

**Risk**: If `NamedTempFile` is used anywhere in the pipeline, a SIGKILL or crash could leave partial downloads. Verify `libprotonup` uses `tempfile()` not `NamedTempFile` for download intermediates.

**Warning W-2 mitigation**: Register a signal handler (or use Tokio's `signal` module) to initiate cleanup on SIGINT/SIGTERM:

```rust
// In the download task cancellation path
tokio::select! {
    result = download_task => result,
    _ = tokio::signal::ctrl_c() => {
        // Temp files backed by tempfile() are OS-cleaned
        // but log the interruption for UX clarity
        Err(InstallError::Cancelled)
    }
}
```

### Atomic Installation

Extract to a temporary directory within `compatibilitytools.d/` (e.g., `GE-Proton9-1.tmp/`), then rename atomically to the final name. This prevents Steam from seeing a partially extracted tool. Rename is atomic on the same filesystem.

---

## Section 4: Input Validation

### Version String Sanitization

Version strings come from two sources:

1. GitHub API JSON responses (release `tag_name`)
2. SQLite `external_cache_entries` (TTL cache of fetched data)

Both must be treated as untrusted. Before using a version string to:

- Construct filesystem paths
- Display in the UI
- Pass to `libprotonup` APIs

Apply a strict allowlist validation:

```rust
fn is_valid_version_tag(tag: &str) -> bool {
    // GE-Proton versions: "GE-Proton9-26", "GE-Proton10-1"
    // Wine-GE versions: "GE-Wine8-26-LoL"
    let re = regex::Regex::new(r"^[A-Za-z0-9][A-Za-z0-9\.\-_]{0,99}$").unwrap();
    re.is_match(tag) && !tag.contains("..") && !tag.contains('/')
}
```

Version strings must never be passed to shell commands without validation. The feature uses `libprotonup` APIs directly, avoiding shell invocation entirely.

### JSON Deserialization Safety

GitHub API responses deserialized via `serde_json` should use `#[serde(deny_unknown_fields)]` on structs where feasible. Known risk: deeply nested JSON can cause stack overflows. Mitigate by enforcing a depth limit or using `serde_json` with a stream deserializer for large responses.

For the GitHub Releases API, responses are not deeply nested and are bounded in size (a few KB for the releases list). Risk is LOW for this specific endpoint.

### Cache Data Integrity

The SQLite `external_cache_entries` table stores the fetched version list. On read, re-validate cached version strings before use. Do not assume cache data is clean even if it was clean when written — the database file could be externally modified.

### URL Validation

Download URLs must originate from `api.github.com` or GitHub CDN (`objects.githubusercontent.com`, `github.com/releases/download`). Validate URL hostname before making download requests:

```rust
fn is_approved_download_host(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("api.github.com")
            | Some("github.com")
            | Some("objects.githubusercontent.com")
            | Some("releases.githubusercontent.com")
    )
}
```

This prevents a compromised cache entry from redirecting downloads to an attacker-controlled server.

---

## Section 5: Infrastructure and Configuration Security

### GitHub API Rate Limits (WARNING W-1)

Unauthenticated requests: 60/hour per IP. As of May 2025, GitHub has tightened unauthenticated rate limits and may continue doing so.

**Impact for CrossHook**: The release list fetch (to populate the version browser) is the primary API call. With a TTL cache in `external_cache_entries`, steady-state load should be 1 request per cache expiry window (suggested: 6–24 hours).

**Required handling:**

- Parse `X-RateLimit-Remaining` and `X-RateLimit-Reset` headers
- If `X-RateLimit-Remaining == 0`, surface a rate limit message in the UI with reset time
- Implement exponential backoff with jitter on 429 responses
- Never retry indefinitely — cap at 3 retries then fail gracefully to cached data

**Optional enhancement**: Allow users to configure a GitHub personal access token (PAT) in CrossHook settings for 5,000 req/hr. Store the token in the OS keyring (via `keyring` crate), never in TOML settings files.

### Secrets Storage

No API tokens, credentials, or secrets are required for baseline operation. If an optional GitHub PAT is added, it must:

- Be stored in the OS keyring, not TOML
- Be treated as sensitive — never logged, never shown in full in the UI

### HTTPS Enforcement

All download and API URLs must use `https://`. Enforce at the URL validation layer before making any request:

```rust
if url.scheme() != "https" {
    return Err(SecurityError::InsecureScheme);
}
```

### Temp Directory Usage

Use the system temp directory (`std::env::temp_dir()` or `tempfile::TempDir`) for intermediate downloads. Do not use `/tmp` directly with predictable filenames. `libprotonup`'s use of the `tempfile` crate satisfies this requirement.

### Cleanup of Partial Downloads

On installation failure (checksum mismatch, extraction error, cancellation), ensure the temp extraction directory is removed. The `TempDir` type from the `tempfile` crate handles this via `Drop`, but verify that panics or forced exits do not leave orphaned directories.

---

## Section 6: Secure Coding Guidelines

These are implementation requirements for the CrossHook integration code wrapping `libprotonup`:

### Required Checks (ship-blocking)

1. **Pin `libprotonup >= 0.9.1`** in `Cargo.toml` and verify no `tokio-tar` (abandoned) appears in `Cargo.lock`.

2. **Validate every extracted path** against the install root before writing (W-4). This is defense-in-depth on top of the patched tar library.

3. **Validate download URL hostname** before requesting. Reject non-`https://` URLs and non-approved hosts.

4. **Validate version strings** from all sources (API, cache) against an allowlist regex before use in path construction or API calls.

5. **Handle rate limit responses** (429, `X-RateLimit-Remaining == 0`) gracefully. Fall back to cached data with age indicator in the UI.

### Required Checks (should address before ship)

6. **Register SIGINT/SIGTERM handlers** in the installation task to ensure temp files and partial extractions are cleaned up on interruption.

7. **Check write permission on `compatibilitytools.d/`** before starting a download. Surface a clear, actionable error if the directory is not writable.

8. **Validate checksum file format** before parsing. A malformed or truncated checksum file should cause an explicit error, not a silent hash mismatch.

### Advisory (can defer)

9. Optionally support GitHub PAT for higher rate limits, stored in OS keyring.
10. Add `#[serde(deny_unknown_fields)]` to GitHub API response structs.
11. Log security-relevant events (download started, checksum verified, extraction completed) at `debug` level without including full file paths in release builds.

---

## Section 7: Trade-Off Recommendations

### Trust Model: SHA-512 vs. GPG

**Trade-off**: SHA-512 from the same GitHub release is vulnerable to a GitHub CDN compromise. GPG verification would provide independent trust but GloriousEggroll does not sign releases.

**Recommendation**: Accept SHA-512 as the trust anchor. Document this limitation in the feature spec. If GPG signing is added upstream in the future, add verification. This is the same trade-off made by ProtonUp-Qt, Lutris, Heroic, and every other tool that installs GE-Proton.

### Authenticated vs. Unauthenticated GitHub API

**Trade-off**: Optional PAT improves rate limits and reliability but adds secrets management complexity.

**Recommendation**: Ship without PAT support initially. The TTL cache makes 60 req/hr sufficient for normal use. Add optional PAT in a follow-up if users report rate limit issues.

### Atomic Extraction vs. Direct Extraction

**Trade-off**: Extracting to a `.tmp` directory and renaming adds complexity but prevents Steam from seeing partial installations.

**Recommendation**: Implement atomic extraction. The rename operation is a single `std::fs::rename()` call; the complexity cost is minimal and the user experience benefit (no corrupted tool entries in Steam) is high.

---

## Section 8: Open Questions

1. Does `libprotonup` use `tempfile()` (OS-backed unnamed) or `NamedTempFile` for download intermediates? This determines the cleanup guarantee on abnormal exit.

2. What is the `external_cache_entries` TTL strategy? Too short (< 1 hour) risks hitting the 60 req/hr GitHub API limit under normal usage.

3. Does CrossHook need to support Wine-GE (Lutris) installations in the first iteration, or only GE-Proton (Steam)? The attack surface is the same, but the install path differs.

4. Will CrossHook invoke the `protonup-rs` binary as a subprocess, or link `libprotonup` as a Rust library dependency? The security posture differs: library integration allows checksum verification in-process; subprocess invocation requires validating the binary path and trusting the subprocess.

5. If the `protonup` binary path is not found, CrossHook shows install guidance. What is the fallback behavior if `libprotonup` is used as a library dependency instead — does the fallback become unnecessary?

---

---

## Section 9: Codebase-Verified Findings

These findings are derived from reading the existing CrossHook source code and correct/extend earlier claims.

### db.rs Symlink Detection (confirmed, extends to `compatibilitytools.d/`)

`metadata/db.rs:open_at_path` already implements symlink detection at line 15:

```rust
if metadata.file_type().is_symlink() {
    return Err(MetadataStoreError::SymlinkDetected(path.to_path_buf()));
}
```

The `MetadataStoreError::SymlinkDetected` variant is defined in `models.rs:40`. This pattern must be mirrored for the `compatibilitytools.d/` install target before writing any files. The install path should use `fs::symlink_metadata()` to detect symlinks **without following them**, and refuse to extract into a symlinked directory.

Additionally, `db.rs` sets `0o700` on the parent directory and `0o600` on the database file. The install function for ProtonUp should similarly verify that the target directory is owned by the current user and not world-writable before extraction.

### MAX_CACHE_PAYLOAD_BYTES = 524_288 (512 KiB) — Validated for GE-Proton release list

`models.rs:152` confirms:

```rust
pub const MAX_CACHE_PAYLOAD_BYTES: usize = 524_288;
```

The GitHub releases API response for GE-Proton (`/repos/GloriousEggroll/proton-ge-custom/releases`) returns per-page JSON with asset metadata. A single page of 30 releases with full asset metadata is typically 50–150 KB. The 512 KiB cap is sufficient for a single paginated page, but if the integration fetches all releases (100+ GE-Proton versions), the total response could exceed 512 KiB.

**Recommendation**: Fetch only the first page (`?per_page=30`) or use `/releases/latest` for the most recent version. If storing multiple pages, store each page under a separate `cache_key`. The `put_cache_entry` function already handles oversized payloads gracefully by storing `NULL payload_json` and logging a warning — this is correct behavior.

Also note: the `cache_store.rs:put_cache_entry` function does not validate `source_url` or `cache_key` before writing to SQLite. These are internal values generated by CrossHook code, not from user input, so the risk is low — but the implementation should ensure version list cache keys are hardcoded constants, not derived from API responses.

### No Command Execution Path (confirmed)

`steam/proton.rs` shows all installed tool discovery uses `fs::read_dir`, `fs::read_to_string`, and VDF parsing — no `std::process::Command` or shell invocations. The install integration must maintain this: call `libprotonup` Rust APIs directly. Never shell out to `protonup-rs` binary with version strings from the API or cache.

### Install Path Derivation (confirmed pattern, apply to ProtonUp)

`steam/proton.rs:discover_compat_tools_with_roots` and `steam/libraries.rs:discover_steam_libraries` both derive filesystem paths from Steam VDF config, not from user-supplied free-form text. The ProtonUp install target (`compatibilitytools.d/`) must follow the same pattern: derive the path from the discovered Steam root (already stored via `SteamLibrary.path`), not from any user input. The install path is always:

```
{steam_root}/compatibilitytools.d/{version_name}/
```

where `{version_name}` must pass `is_valid_version_tag()` before path construction.

### `safe_enumerate_directories` Follows Symlinks (observation)

`steam/proton.rs:safe_enumerate_directories` uses `path.is_dir()` at line 53 and `path.is_dir()` per entry at line 482 — both of which **follow symlinks**. This means a symlinked `compatibilitytools.d` subdirectory would be enumerated as a normal directory during discovery. This is acceptable for read-only discovery but reinforces the requirement to use `symlink_metadata()` (not `metadata()` or `is_dir()`) when **writing** to the install directory.

## Sources

- [TARmageddon CVE-2025-62518 — Edera](https://edera.dev/stories/tarmageddon)
- [CVE-2025-62518 Detail — NVD](https://nvd.nist.gov/vuln/detail/CVE-2025-62518)
- [astral-tokio-tar path traversal advisory — GitHub](https://github.com/astral-sh/tokio-tar/security/advisories/GHSA-3wgq-wrwc-vqmv)
- [protonup-rs releases — GitHub](https://github.com/auyer/Protonup-rs/releases)
- [libprotonup — lib.rs](https://lib.rs/crates/libprotonup)
- [GloriousEggroll proton-ge-custom — GitHub](https://github.com/GloriousEggroll/proton-ge-custom)
- [GitHub REST API Rate Limits — GitHub Docs](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
- [Updated rate limits for unauthenticated requests — GitHub Changelog](https://github.blog/changelog/2025-05-08-updated-rate-limits-for-unauthenticated-requests/)
- [GitHub REST API Best Practices — GitHub Docs](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)
- [reqwest TLS documentation — docs.rs](https://docs.rs/reqwest/latest/reqwest/tls/index.html)
- [reqwest ClientBuilder — docs.rs](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html)
- [tempfile crate — docs.rs](https://docs.rs/tempfile/)
- [Rust zip crate CVE-2025-29787 — Snyk](https://security.snyk.io/vuln/SNYK-RUST-ZIP-9460813)
- [Zip Slip vulnerability — Snyk GitHub](https://github.com/snyk/zip-slip-vulnerability)
- [Symlink attacks — Medium](https://medium.com/@instatunnel/symlink-attacks-when-file-operations-betray-your-trust-986d5c761388)
- [Serde security with untrusted input — GitHub Issue](https://github.com/serde-rs/serde/issues/1087)
- [RustSec Advisory Database](https://rustsec.org/advisories/)
- [CVE-2025-59825: astral-tokio-tar path traversal — GitHub Advisory](https://github.com/astral-sh/tokio-tar/security/advisories/GHSA-3wgq-wrwc-vqmv)
- [CVE-2025-59825 — NVD](https://nvd.nist.gov/vuln/detail/CVE-2025-59825)
- [CVE-2025-59825: astral-tokio-tar Tar Unpack RCE — Miggo](https://www.miggo.io/vulnerability-database/cve/CVE-2025-59825)
- [Google Security Research GHSA-9p78-p5g6-gcj8 — uv/astral-tokio-tar arbitrary write](https://github.com/google/security-research/security/advisories/GHSA-9p78-p5g6-gcj8)
- [CVE-2026-32766: astral-tokio-tar PAX extension validation — GitLab Advisory](https://advisories.gitlab.com/pkg/cargo/astral-tokio-tar/CVE-2026-32766/)
- [RUSTSEC-2025-0110: astral-tokio-tar PAX Header Desynchronization — RustSec](https://rustsec.org/advisories/RUSTSEC-2025-0110.html)
- [astral-sh/tokio-tar repository](https://github.com/astral-sh/tokio-tar)
- [astral-tokio-tar — crates.io](https://crates.io/crates/astral-tokio-tar)
