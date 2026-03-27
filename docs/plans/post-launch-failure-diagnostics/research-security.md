# Security Research: post-launch-failure-diagnostics

## Executive Summary

This feature introduces structured diagnostic analysis for post-launch failures in CrossHook. The primary security surface is **reading and parsing untrusted data** from log files and crash dumps produced by third-party games, trainers, and Proton/WINE. CrossHook is a local desktop application with no network exposure, which significantly reduces the blast radius of all findings.

The codebase already demonstrates strong security habits — `symlink_metadata` checks before deletion, `validate_name` blocking path traversal, `sanitize_launcher_slug` for safe filesystem identifiers, and `env_clear()` on spawned processes. The new feature should extend these patterns consistently.

**Threat model calibration**: All data flows are local. The user owns the machine, the filesystem, and the processes. The attacker model is limited to: (1) a malicious or compromised game/trainer producing crafted log output or crash dumps, and (2) information disclosure through UI screenshots shared publicly. There is no remote attacker, no network input, and no privilege escalation surface.

**Overall risk**: LOW to MEDIUM. No hard-stop blockers were identified. Several WARNING-level items require attention to prevent resource exhaustion and information leakage, but none prevent shipping.

---

## Findings by Severity

### CRITICAL — Hard Stops

**No CRITICAL findings.** The feature reads local files on the user's own machine via Rust's standard library. There is no network input, no deserialization of untrusted binary formats into executable structures, and no privilege boundary crossing.

### WARNING — Must Address

| #   | Finding                                                                 | Area             | Mitigation                                                          |
| --- | ----------------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------- |
| W1  | Unbounded file read for log and crash dump files                        | Filesystem       | Enforce maximum file size before reading; use bounded reads         |
| W2  | Crash dump path construction from profile data without canonicalization | Filesystem       | Canonicalize and verify paths stay within expected parent directory |
| W3  | Log content accumulated without limit in frontend `ConsoleView` state   | Input Validation | Cap the number of diagnostic lines emitted to the frontend          |
| W4  | Diagnostic messages may expose full filesystem paths including username | Info Disclosure  | Sanitize paths in user-facing diagnostic output                     |

### ADVISORY — Best Practices

| #   | Finding                                                          | Area             | Mitigation                                                     |
| --- | ---------------------------------------------------------------- | ---------------- | -------------------------------------------------------------- |
| A1  | Symlink following when reading crash report directories          | Filesystem       | Use `symlink_metadata` before reads (existing pattern)         |
| A2  | Regex patterns for error detection should use Rust `regex` crate | Dependencies     | Already ReDoS-safe by design; document this guarantee          |
| A3  | Crash dumps may contain memory with sensitive data               | Data Protection  | Do not display raw crash dump bytes; extract only metadata     |
| A4  | PROTON_CRASH_REPORT_DIR environment variable may point anywhere  | Filesystem       | Validate resolved directory is within expected compatdata tree |
| A5  | Exit code values should be bounded to valid ranges               | Input Validation | Clamp to i32 range; map unknown codes to a generic message     |

---

## Filesystem Access Security

### Crash Report Directory Access

The feature will read crash reports from paths derived from `$STEAM_COMPAT_DATA_PATH`. Based on Proton's architecture, crash reports are typically found at:

- `$STEAM_COMPAT_DATA_PATH/crashreports/` (Proton-generated)
- `$PROTON_CRASH_REPORT_DIR` (user-configured override, e.g., `/tmp/umu_crashreports`)
- `$STEAM_COMPAT_DATA_PATH/pfx/drive_c/users/steamuser/` (WINE-level crash dumps)

**Path traversal risk [W2]**: The `STEAM_COMPAT_DATA_PATH` value comes from the profile's `compatdata_path` field, which is user-supplied via the UI. If the diagnostics module constructs crash report paths by joining this with `crashreports/`, a malicious profile value containing `..` segments could escape the intended directory.

**Recommended mitigation**:

```rust
fn resolve_crash_report_dir(compatdata_path: &Path) -> Option<PathBuf> {
    let candidate = compatdata_path.join("crashreports");
    // Canonicalize to resolve symlinks and .. segments
    let canonical = candidate.canonicalize().ok()?;
    let canonical_base = compatdata_path.canonicalize().ok()?;
    // Verify resolved path is within expected tree
    if canonical.starts_with(&canonical_base) {
        Some(canonical)
    } else {
        tracing::warn!(
            path = %candidate.display(),
            resolved = %canonical.display(),
            "crash report directory resolved outside compatdata tree"
        );
        None
    }
}
```

This follows the same pattern as the existing `validate_name` function in `toml_store.rs:300` which blocks path traversal characters.

**Confidence**: High — The codebase already prevents path traversal in profile names; this applies the same principle to crash report paths.

### Symlink Following [A1]

The existing `launcher_store.rs:610-611` uses `fs::symlink_metadata()` before operating on files, which is the correct pattern. The diagnostics module should follow suit:

- Use `symlink_metadata()` to check file type before reading crash report files
- Refuse to follow symlinks in the crash report directory to prevent TOCTOU attacks
- This is ADVISORY because a local attacker who can plant symlinks in the user's own compatdata directory already has equivalent access to whatever the symlink targets

**Confidence**: High — Established pattern in codebase; low real-world risk for local desktop app.

### File Size Limits [W1]

**Risk**: Log files and crash dumps can be arbitrarily large. A game that logs verbosely could produce multi-gigabyte log files. Proton crash dumps (minidumps) are typically 32KB-2MB, but full core dumps can be gigabytes.

**Current state**: The existing `stream_log_lines` function in `launch.rs:121-172` reads the entire log file into memory with `tokio::fs::read_to_string(&log_path)` on every 500ms poll cycle. This is already a latent concern for very large log files.

**Recommended mitigation**: For the diagnostics module specifically:

```rust
const MAX_LOG_SCAN_BYTES: u64 = 2 * 1024 * 1024;  // 2 MiB
const MAX_CRASH_DUMP_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB

fn safe_read_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("file exceeds size limit ({} > {})", metadata.len(), max_bytes),
        ));
    }
    fs::read(path)
}
```

For log analysis, reading just the last N kilobytes (tail read) is sufficient since failure information concentrates at the end.

**Confidence**: High — `std::fs::read` allocates the full file size. This is well-documented Rust behavior.

---

## Data Protection

### Crash Dump Sensitivity [A3]

Wine/Proton minidumps contain:

- Thread stack contents (may contain passwords, API tokens, session keys from the game process)
- Processor register states
- Memory mappings and loaded module paths
- Process memory regions

**Risk calibration**: These are crash dumps from the user's own games running on their own machine. The user could inspect them with any tool regardless of CrossHook. The risk is not that CrossHook _accesses_ them — it's that CrossHook might _display_ sensitive content in the UI where it could be captured in a screenshot and shared.

**Recommended mitigation**:

- Extract only structured metadata from crash dumps: crash reason, signal code, faulting module name, thread count
- Never display raw hex dumps or memory contents in the UI
- If parsing minidump format, extract only the `MINIDUMP_EXCEPTION_STREAM` (crash reason) and `MINIDUMP_MODULE_LIST_STREAM` (which DLL/module faulted), not memory streams

**Confidence**: High — Based on minidump format documentation and the SANS research on crash dump exploitation.

### Log File Content [W4]

Log files frequently contain:

- Full filesystem paths revealing username: `/home/yandy/.local/share/Steam/...`
- Environment variable values (potentially including auth tokens from badly-behaved games)
- Windows registry paths within the prefix
- Network addresses (if the game attempts online features)

**Recommended mitigation for diagnostic output**:

```rust
fn sanitize_display_path(path: &str) -> String {
    // Replace home directory with ~
    if let Some(home) = std::env::var_os("HOME") {
        let home_str = home.to_string_lossy();
        if path.starts_with(home_str.as_ref()) {
            return format!("~{}", &path[home_str.len()..]);
        }
    }
    path.to_string()
}
```

This is particularly important for diagnostic _suggestions_ (the human-readable output) as opposed to raw log streaming, which already displays unsanitized content in the existing `ConsoleView`.

**Confidence**: Medium — The existing ConsoleView already shows raw log content. The risk delta from diagnostics is in the structured suggestions which are more likely to be screenshot-shared.

---

## Dependency Security

### Rust `regex` Crate — ReDoS Safe [A2]

The Rust `regex` crate uses finite automata (not backtracking), providing a **hard guarantee** of `O(m * n)` worst-case time complexity where `m` is regex size and `n` is input size. This means:

- **No catastrophic backtracking is possible**, regardless of input
- Untrusted haystacks (log lines from malicious games) cannot trigger exponential behavior
- The crate is fuzz-tested as part of OSS-fuzz

**One caveat**: Iterating over all matches with `find_iter()` / `captures_iter()` has `O(m * n^2)` worst case. For the diagnostics use case (scanning individual log lines, not iterating all matches across a massive document), this is acceptable.

**Recommendation**: Use the `regex` crate for all pattern matching. No additional timeout wrapper or ReDoS mitigation is needed. Document this guarantee in code comments for future maintainers.

**Confidence**: High — From official `regex` crate documentation at docs.rs. The crate has been audited and fuzz-tested. This is a fundamental design property of the engine, not a configuration option.

### No New Binary Parsing Dependencies Required

For crash dump analysis, the feature description mentions checking `$STEAM_COMPAT_DATA_PATH/crashreports/` for crash dumps. The recommended approach is:

- **Check presence and basic metadata** (file count, sizes, modification times) — requires only `std::fs`
- **Do not parse minidump binary format** unless the feature specifically requires extracting crash reasons — this would require a crate like `minidump` or `minidump-processor`

If minidump parsing is added later, evaluate the `minidump` crate (by Mozilla/Breakpad team, actively maintained, used in Firefox crash reporting). It has a strong security track record but would add a dependency chain.

**Confidence**: High — Current feature scope (detecting presence of crash reports and surfacing the information) does not require binary parsing.

---

## Input Validation

### Log File Content as Untrusted Input

Log lines are produced by third-party games, trainers, WINE, and Proton. They should be treated as **untrusted input** for pattern matching purposes.

**Specific concerns**:

1. **Encoding**: Log files may contain non-UTF-8 bytes (binary data, game-specific encodings). Use `String::from_utf8_lossy()` or `read()` + lossy conversion rather than `read_to_string()` which will error on invalid UTF-8.

2. **Line length**: A malicious or buggy game could write extremely long lines (megabytes without a newline). Pattern matching on such lines is safe with the Rust `regex` crate (linear time), but accumulating them in frontend state is not.

3. **Control characters**: Log output may contain ANSI escape sequences, null bytes, or other control characters. The existing `ConsoleView` renders into `<pre>` tags which handles most of this safely, but diagnostic messages should strip control characters.

**Recommended validation for diagnostic input**:

```rust
const MAX_LINE_LENGTH: usize = 4096; // Truncate individual lines

fn sanitize_log_line(line: &str) -> &str {
    let truncated = if line.len() > MAX_LINE_LENGTH {
        &line[..MAX_LINE_LENGTH]
    } else {
        line
    };
    truncated
}
```

**Confidence**: High — Standard input validation practice for log processing.

### Exit Code Validation [A5]

Unix exit codes are `i32` values. Signal-based termination produces codes like 128+signal_number. The diagnostics module should:

- Accept any `i32` value without panic
- Map recognized codes (SIGSEGV=139, SIGKILL=137, SIGABRT=134, etc.) to human-readable messages
- Map unrecognized codes to a generic "Process exited with code N" message
- Never use exit codes as indices into arrays without bounds checking

**Confidence**: High — Standard Unix process semantics.

### Frontend State Accumulation [W3]

The current `ConsoleView` accumulates log lines indefinitely in React state (`useState<ConsoleLine[]>`). The diagnostics feature should not exacerbate this:

- Cap diagnostic output to a fixed number of entries (e.g., 50 diagnostic findings per launch)
- If the diagnostics module emits findings as events, use a separate bounded buffer
- The existing log stream concern is pre-existing and outside scope, but worth noting

**Confidence**: High — Observable from `ConsoleView.tsx:21-23` which uses unbounded `useState`.

---

## Information Disclosure

### Path Exposure in Diagnostic Messages [W4]

Diagnostic suggestions will contain filesystem paths. For example:

- "Missing .NET runtime in /home/yandy/.local/share/Steam/steamapps/compatdata/12345/pfx/"
- "WINEPREFIX /home/yandy/.local/share/Steam/steamapps/compatdata/12345/pfx/ has wrong permissions"

These reveal the username and directory structure. When users share screenshots on forums (ProtonDB, Reddit, Steam forums), this becomes an information disclosure.

**Recommended mitigation**:

1. Replace `$HOME` prefix with `~` in all diagnostic display strings
2. Replace Steam library paths with `<steam-library>/` prefix
3. Replace compatdata app IDs only if they are not already public (they are — Steam app IDs are public)
4. Apply sanitization only to the _diagnostic suggestions_, not to the raw log stream (which already shows unsanitized content)

**Confidence**: Medium — This is a UX-layer concern. The existing ConsoleView already exposes paths. The delta risk is that diagnostic messages are more structured, more shareable, and more likely to be screenshot-captured.

### Environment Variable Exposure

Diagnostic messages should never include raw environment variable values in suggestions. For example:

- GOOD: "STEAM_COMPAT_DATA_PATH is not set"
- BAD: "STEAM_COMPAT_DATA_PATH is set to /home/yandy/..."

Report _whether_ a variable is set and _whether_ it points to a valid directory, but do not echo the value.

**Confidence**: High — Standard information disclosure prevention.

---

## Secure Coding Guidelines

### For the Diagnostics Module

1. **File reads**: Always check file size before reading. Use `safe_read_file()` helper with configurable max size.
2. **Path construction**: Canonicalize and verify any path derived from profile/environment data before accessing.
3. **Symlinks**: Use `symlink_metadata()` for initial checks (follows existing `launcher_store.rs` pattern).
4. **Regex**: Use the Rust `regex` crate exclusively. No PCRE, no `fancy-regex` (which does use backtracking).
5. **Display paths**: Always sanitize filesystem paths in diagnostic output shown in the UI. Replace `$HOME` with `~`.
6. **Crash dump content**: Extract metadata only (file existence, size, timestamp). Do not display memory contents.
7. **Log line processing**: Truncate individual lines to `MAX_LINE_LENGTH`. Use lossy UTF-8 conversion.
8. **Bounded output**: Cap the number of diagnostic findings per analysis run.

### Reusable Helpers to Standardize

The following helpers would benefit from being shared across the diagnostics module and potentially the broader codebase:

| Helper                                 | Purpose                             | Existing Pattern                                                                     |
| -------------------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------ |
| `safe_read_file(path, max_bytes)`      | Size-bounded file read              | New — needed for crash dumps and log tail reads                                      |
| `resolve_and_verify_path(base, child)` | Canonicalize + starts_with check    | Partial — `validate_name` blocks traversal chars; canonicalize adds defense-in-depth |
| `sanitize_display_path(path)`          | Replace $HOME with ~ for UI display | New — needed for diagnostic messages                                                 |
| `is_regular_file(path)`                | Symlink-safe file type check        | Existing — `verify_crosshook_file` in `launcher_store.rs`                            |

---

## Trade-off Recommendations

| Decision                                  | Recommended       | Rationale                                                                                                                                                                      |
| ----------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Parse minidump binary format?             | No (defer)        | File presence/metadata is sufficient for v1. Adds dependency risk and complexity.                                                                                              |
| Use `fancy-regex` for lookahead patterns? | No                | `fancy-regex` uses backtracking and is ReDoS-vulnerable. The standard `regex` crate is sufficient for error pattern detection.                                                 |
| Sanitize raw ConsoleView log stream?      | No (out of scope) | Pre-existing behavior. Changing it would break user expectations. Apply sanitization only to new diagnostic output.                                                            |
| Read crash dumps to extract crash reason? | Advisory          | If desired, read only the first 4KB (minidump header + exception stream). Do not load full dump.                                                                               |
| Enforce Tauri capability restrictions?    | No change needed  | The current `core:default` + `dialog:default` capabilities are sufficient. The diagnostics module accesses files through Rust backend commands, not Tauri's filesystem plugin. |

---

## Open Questions

1. **Crash report directory variability**: Does Proton always write to `$STEAM_COMPAT_DATA_PATH/crashreports/`, or can games configure their own crash dump location within the prefix? If the latter, the diagnostics module may need to scan multiple known locations.

2. **Log file encoding**: Are there known cases where Proton/WINE log files contain non-UTF-8 content? If so, the diagnostics module needs byte-level reading with lossy conversion.

3. **Existing log stream backpressure**: The current `stream_log_lines` reads the entire log file every 500ms. Should the diagnostics module share this stream, or perform a separate one-time tail read after process exit? A separate post-exit read is simpler and avoids adding overhead to the streaming path.

4. **Feature scope for crash dumps**: Is the intent to (a) report that crash dumps exist and provide their paths, or (b) parse the minidump format to extract the crash reason? Option (a) has no dependency or security implications. Option (b) requires evaluating the `minidump` crate.

---

## Sources

- [Rust regex crate documentation — ReDoS safety guarantees](https://docs.rs/regex/latest/regex/)
- [StackHawk — Rust Path Traversal Guide](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/)
- [Segmented stacks in Wine/Proton minidumps — minidump format details](https://werat.dev/blog/segmented-stacks-in-wine-proton-minidumps/)
- [SANS — From Crash to Compromise: Windows Crash Dumps in Offensive Security](https://www.sans.org/white-papers/from-crash-compromise-unlocking-potential-windows-crash-dumps-offensive-security)
- [Linux Crash Dump Vulnerabilities (CVE-2025-5054, CVE-2025-4598)](https://linuxsecurity.com/news/security-vulnerabilities/linux-crash-dump-vulns)
- [Tauri v2 Security Documentation](https://v2.tauri.app/security/)
- [Proton FAQ — Valve](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
