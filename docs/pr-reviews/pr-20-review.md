# PR #20 Review: feat: implement the platform-native-ui native app feature set

**PR**: #20 (`feat/platform-native-ui` -> `main`)
**Date**: 2026-03-23
**Scope**: +23,756 / -22,192 across 150+ files
**Closes**: #19

## Overview

This PR implements the full native CrossHook app (Rust/Tauri/React) and removes the legacy C#/.NET codebase. Five specialized review agents analyzed the changes in parallel:

| Agent                 | Focus                                                |
| --------------------- | ---------------------------------------------------- |
| Code Reviewer         | Rust core logic, security, error handling, panics    |
| Silent Failure Hunter | Shell scripts, swallowed errors, missing exit checks |
| Type Design Analyzer  | Rust/TS type quality, IPC boundary consistency       |
| Test Analyzer         | Coverage gaps from C#->Rust migration                |
| Comment Analyzer      | Comment accuracy, missing docs, TODO tracking        |

---

## Critical Issues (7 found)

### C1. Shell scripts lose real Proton exit codes

**Status**: Open
**Agents**: Silent Failure Hunter, Comment Analyzer
**Files**: `runtime-helpers/steam-host-trainer-runner.sh:232-239`, `runtime-helpers/steam-launch-helper.sh:349-357`

Both runtime helper scripts use `if "$proton" run ...; then / exit 0 / fi / exit_code=$?` — but `set -e` is suppressed inside `if` conditionals, so `$?` after a failed `if` block is always 1, never the actual Proton exit code. If Proton exits with code 42, the script logs and exits with code 1.

**Impact**: Real Proton failure codes (which indicate specific error conditions like missing dependencies or segfaults) are lost, making debugging much harder.

**Fix**: Capture the exit code directly:

```bash
"$proton" run "$trainer_path" || true
exit_code=$?
if [ "$exit_code" -eq 0 ]; then
  log "Trainer proton run exited successfully."
else
  log "Trainer proton run exited with code $exit_code"
fi
exit "$exit_code"
```

### C2. Launch log stream silently drops lines on process exit

**Status**: Open
**Agent**: Code Reviewer
**File**: `src-tauri/src/commands/launch.rs:99-127`

When the child process exits (`try_wait` returns `Ok(Some(_))`), the loop breaks immediately without performing a final read of the log file. Lines written between the last 500ms poll and process exit are silently dropped.

**Fix**: Perform one final read after the loop breaks:

```rust
// After the loop:
if let Ok(content) = tokio::fs::read_to_string(&log_path).await {
    if content.len() > last_len {
        for line in content[last_len..].lines().filter(|l| !l.is_empty()) {
            let _ = app.emit("launch-log", line.to_string());
        }
    }
}
```

### C3. Launch log emit errors silently discarded

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src-tauri/src/commands/launch.rs:112`

`let _ = app.emit("launch-log", line.to_string());` discards any error. If the frontend disconnects, every log line silently fails to deliver. Users see no launch output with zero indication of why.

**Fix**: Log a warning on first failure and break:

```rust
if let Err(error) = app.emit("launch-log", line.to_string()) {
    tracing::warn!(?error, "failed to emit launch log line to frontend");
    break;
}
```

### C4. Log stream ignores file read errors and child process errors

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src-tauri/src/commands/launch.rs:99-127`

Two silent failures in `stream_log_lines`:

1. Line 103: `if let Ok(content) = tokio::fs::read_to_string(...)` silently skips on read failure (permissions, deleted, disk full).
2. Line 121: `Err(_) => break` silently ignores `try_wait()` OS errors.

**Fix**: Log errors from both code paths.

### C5. Community tap indexer traverses `.git/` directory

**Status**: Open
**Agent**: Code Reviewer
**File**: `crates/crosshook-core/src/community/index.rs:99-163`

`collect_manifests` recursively walks all subdirectories including `.git/`, causing unnecessary I/O over thousands of git object files and potential errors from permission-restricted git internals.

**Fix**: Skip hidden directories:

```rust
if path.is_dir() {
    if path.file_name().and_then(|v| v.to_str()).map_or(false, |n| n.starts_with('.')) {
        continue;
    }
    collect_manifests(root, &path, workspace, index)?;
    continue;
}
```

### C6. Community tap git operations have no timeout

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `crates/crosshook-core/src/community/taps.rs:184-213, 253-283`

All git operations use `Command::new("git")...output()` which blocks indefinitely. If a remote repo is unreachable (DNS failure, firewall), the `community_sync` Tauri command hangs, freezing the UI.

**Fix**: Use `tokio::process::Command` with a timeout, or set `GIT_HTTP_LOW_SPEED_LIMIT=1000` and `GIT_HTTP_LOW_SPEED_TIME=30` as environment variables on the git process.

### C7. WINE env var lists have drifted between Rust and shell scripts

**Status**: Open
**Agent**: Comment Analyzer
**Files**: `crates/crosshook-core/src/launch/env.rs:54`, `runtime-helpers/steam-launch-helper.sh:332-341`, `runtime-helpers/steam-host-trainer-runner.sh:214-223`

The `WINE_ENV_VARS_TO_CLEAR` constant in Rust includes `WINE_HEAP_DELAY_FREE` and `WINEFSYNC_SPINCOUNT`, but the shell scripts omit them. Conversely, the shell scripts unset `WINEPREFIX` which is not in the Rust list. No comment explains the intentional divergence.

**Fix**: Add a comment in `env.rs` documenting the intentional divergence. Consider having the Rust side generate or validate the shell script's unset list.

---

## Important Issues (11 found)

### I1. `import_community_profile` uses filename as profile name

**Status**: Open
**Agent**: Code Reviewer
**File**: `crates/crosshook-core/src/profile/exchange.rs:113-123`

The function extracts the profile name from the file stem. Since community manifests are always named `community-profile.json`, every import would produce the name `community-profile`.

**Fix**: Derive the name from manifest metadata (e.g., `manifest.metadata.game_name`) or the parent directory name.

### I2. `community_import_profile` accepts arbitrary filesystem paths

**Status**: Open
**Agent**: Code Reviewer
**File**: `src-tauri/src/commands/community.rs:80-86`

The Tauri command accepts any string path from the frontend and reads arbitrary files. No validation that the path belongs to a synced tap workspace.

**Fix**: Validate that the path is under a known tap workspace directory.

### I3. `.desktop` files written with 0o755 permissions

**Status**: Open
**Agent**: Code Reviewer
**File**: `crates/crosshook-core/src/export/launcher.rs:420-428`

`write_host_text_file` sets 0o755 on all files including `.desktop` entries, which should be 0o644.

**Fix**: Pass the desired permission mode as a parameter.

### I4. `safe_enumerate_directories` silently discards read_dir errors

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `crates/crosshook-core/src/steam/proton.rs:458-474`

Returns an empty list on `read_dir` failure with no diagnostics. Users with SD card Steam libraries or non-standard permissions see zero Proton installs with no explanation.

**Fix**: Add errors to the diagnostics vector.

### I5. Manifest scanning drops directory entry errors

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `crates/crosshook-core/src/steam/manifest.rs:155`

`entries.filter_map(Result::ok)` silently drops broken symlinks and permission errors. Games may silently fail to appear in auto-populate.

**Fix**: Log skipped entries to the diagnostics vector.

### I6. `list_proton_installs` command discards diagnostics

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src-tauri/src/commands/steam.rs:35-44`

Diagnostics from `discover_steam_root_candidates` and `discover_compat_tools` are collected but never returned or logged.

**Fix**: Return diagnostics alongside the install list or log them via `tracing`.

### I7. `auto-load-profile` emit failure silently discarded

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src-tauri/src/lib.rs:37`

`let _ = app_handle.emit("auto-load-profile", ...)` discards the result. If the event fails to reach the frontend, the user's profile doesn't load on startup with no indication.

**Fix**: Log the error.

### I8. Bundled script resolution silently falls back to dev path

**Status**: Open
**Agent**: Silent Failure Hunter
**Files**: `src-tauri/src/paths.rs:5-8`, `src-tauri/src/commands/launch.rs:144-166`

Two separate `resolve_script_path` functions fall back from bundled to dev paths without logging. In production, if scripts aren't bundled, users get confusing "No such file or directory" errors.

**Fix**: Log which path was resolved and fail explicitly in production if the bundled path is missing.

### I9. Constructors panic on missing home directory

**Status**: Open
**Agent**: Silent Failure Hunter
**Files**: `community/taps.rs:101-104`, `profile/toml_store.rs:61-62`, `settings/mod.rs:72-73`

`BaseDirs::new().expect(...)` panics with an unhelpful stack trace on containerized or unusual environments.

**Fix**: Return `Result` from constructors and show a user-facing dialog.

### I10. `should_stage_support_file` has dead logic branch

**Status**: Open
**Agent**: Comment Analyzer
**File**: `crates/crosshook-core/src/launch/script_runner.rs:377-393`

The `starts_with` check on line 388 returns `true`, but line 392 also unconditionally returns `true` — making the `starts_with` branch unreachable as a meaningful decision point.

**Fix**: Remove the dead branch and add a comment explaining the simplified staging behavior.

### I11. Spawned log stream task runs unsupervised

**Status**: Open
**Agent**: Silent Failure Hunter
**File**: `src-tauri/src/commands/launch.rs:93-97`

`tauri::async_runtime::spawn` returns a `JoinHandle` that is never joined. If the task panics, it's silently swallowed.

**Fix**: Capture the handle and log if it resolves to an error.

---

## Test Coverage Gaps (8 found)

The PR removes 20 C#/.NET test files (~1,965 lines) and adds **68 Rust tests** across 20 `#[cfg(test)]` modules. Coverage is solid for happy paths but has gaps in critical areas.

### Critical Gaps

| Gap                                    | Risk | Details                                                                                                               |
| -------------------------------------- | ---- | --------------------------------------------------------------------------------------------------------------------- |
| **Export `validate()` negative cases** | 8/10 | 8 validation branches with zero negative-case tests. Old C# suite tested these extensively.                           |
| **`resolved_method()` auto-detection** | 8/10 | Fallback heuristics (empty method -> steam/proton/native) are untested. Wrong dispatch could corrupt Proton prefixes. |
| **Legacy profile conversion**          | 8/10 | `From<LegacyProfileData>` only tested for `steam_applaunch` branch. `proton_run` and `native` branches untested.      |

### Important Gaps

| Gap                                                | Risk | Details                                                        |
| -------------------------------------------------- | ---- | -------------------------------------------------------------- |
| CLI `steam_launch_request_from_profile()`          | 7/10 | Profile-to-request conversion with conditional logic, untested |
| Support file staging (`should_stage_support_file`) | 7/10 | Only `.ini` files tested; `.dll`, directory staging untested   |
| `build_native_game_command()`                      | 6/10 | Working directory logic untested                               |
| `validate_name()` positive cases                   | 5/10 | Only rejection cases tested, no acceptance cases               |
| Community tap malformed repo handling              | 5/10 | No tests for repos with missing manifests or invalid JSON      |

### Well-Tested Areas

VDF parser (6 tests), Steam discovery/libraries (4 tests), Proton resolution (5 tests, excellent), Profile TOML store (full CRUD), Community exchange round-trip, Launch validation (all 3 methods), Settings/recent files persistence, Logging with rotation.

---

## Type Design Analysis

### IPC Boundary Consistency

All Rust serde types match their TypeScript counterparts across the Tauri IPC boundary. **Zero `any` types** found in TypeScript.

### Critical Type Issues

| Issue                                  | Impact                                                   | Recommendation                                                              |
| -------------------------------------- | -------------------------------------------------------- | --------------------------------------------------------------------------- |
| **`InjectionSection` parallel arrays** | `dll_paths` and `inject_on_launch` can desynchronize     | Replace with `Vec<InjectionEntry>`                                          |
| **`LaunchSection.method` is `String`** | Only 3 valid values + empty, but unbounded               | Introduce `LaunchMethod` enum in Rust (TS already has string literal union) |
| **`SteamLaunchRequest` type alias**    | Dead weight, creates false impression of distinction     | Remove                                                                      |
| **`LaunchResult` in wrong layer**      | Defined in Tauri commands, not `crosshook-core`          | Move to core crate                                                          |
| **`ValidationResult` dead code in TS** | Doesn't match Tauri's `Result<(), String>` serialization | Remove or audit                                                             |
| **`ProtonInstall` dual alias fields**  | `aliases` and `normalized_aliases` can desynchronize     | Compute normalized via method, don't store                                  |

### Strengths

- `ValidationError` enum: 5/5 expression, 5/5 enforcement — best-designed type in the codebase
- `CompatibilityRating` enum: textbook illegal-state elimination
- TypeScript `Exclude<LaunchMethod, ''>` prevents empty methods across IPC — stricter than Rust side
- `satisfies` keyword usage at IPC call sites catches field typos at compile time

### Summary Ratings

| Type                           | Encapsulation | Expression | Usefulness | Enforcement |
| ------------------------------ | ------------- | ---------- | ---------- | ----------- |
| Rust: GameProfile              | 2/5           | 2/5        | 4/5        | 2/5         |
| Rust: LaunchRequest            | 2/5           | 3/5        | 4/5        | 4/5         |
| Rust: ValidationError          | 4/5           | 5/5        | 5/5        | 5/5         |
| Rust: CommunityProfileManifest | 3/5           | 3/5        | 4/5        | 4/5         |
| Rust: SteamAutoPopulateResult  | 2/5           | 4/5        | 4/5        | 2/5         |
| Rust: AppSettingsData          | 3/5           | 3/5        | 5/5        | 4/5         |
| TS: GameProfile                | 3/5           | 4/5        | 4/5        | 3/5         |
| TS: LaunchRequest              | 3/5           | 4/5        | 5/5        | 3/5         |
| TS: LaunchPhase/LaunchAction   | 4/5           | 5/5        | 5/5        | 4/5         |

---

## Comment Analysis

### Critical Comment Issues

1. **Stale C# reference in test name**: `desktop_exec_escaping_matches_csharp_rules` in `export/launcher.rs:529` references deleted C# codebase.
2. **WINE env var list divergence undocumented** (see C7 above).
3. **Dead logic branch in `should_stage_support_file`** lacks explanatory comment (see I10 above).

### Missing Documentation (Recommended)

| Location                           | Missing                                                                           |
| ---------------------------------- | --------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs` | Crate-level `//!` doc                                                             |
| `steam/vdf.rs` (module)            | `//!` doc explaining VDF format and supported subset                              |
| `vdf.rs:90-92` `normalize_key`     | Comment explaining case-insensitivity mirrors Steam behavior                      |
| `script_runner.rs:159-184`         | Comment explaining `pfx` parent-directory derivation for `STEAM_COMPAT_DATA_PATH` |
| `script_runner.rs:187-212`         | Doc comment explaining three-tier fallback for Steam client path                  |
| `script_runner.rs:307-345`         | Doc comment explaining why trainer staging into prefix is necessary               |
| `request.rs:238-239`               | Comment explaining why Steam applaunch doesn't require game path to exist         |
| `launcher.rs:491-493`              | Comment explaining `/compatdata/` path rejection                                  |
| All 3 runtime helper scripts       | File-level header comments explaining purpose and orchestration                   |
| `logging.rs`                       | Module-level doc explaining rotation strategy                                     |
| `steam/proton.rs:273-310`          | Doc comment on three-tier resolution strategy                                     |

### Positive Observations

- Shell script comments at `steam-launch-helper.sh:321-331` explaining FD closure and WINE env cleanup are excellent "why" comments.
- Self-documenting naming is strong throughout (`require_game_path_if_needed`, `looks_like_windows_executable`, `stage_trainer_into_prefix`).
- Test names serve as living documentation (`allows_game_only_steam_launch_without_trainer_paths`).

---

## Overall Strengths

1. **Error handling via `Result<T, E>`** is consistent with no swallowed errors in core business logic
2. **Input validation** is thorough — `validate_name` blocks path traversal, launch validation covers all methods
3. **POSIX single-quote escaping** (`shell_single_quoted`) is correct
4. **VDF parser** handles edge cases well (escapes, comments, unquoted tokens)
5. **Serde derive macros** used consistently on all IPC boundary types
6. **Test fixtures** use `tempfile` consistently for safe parallel execution
7. **Build scripts** all use `set -euo pipefail` with proper `die` functions
8. **68 meaningful Rust tests** — substantially more test investment than typical for a rewrite

---

## Recommended Action Plan

### Before Merge (Critical)

1. Fix exit code loss in both shell scripts (C1)
2. Add final log file read after process exit (C2)
3. Log emit failures in launch log stream (C3, C4)
4. Skip `.git/` in community indexer (C5)
5. Add git operation timeouts (C6)

### Fast Follow (Important)

6. Fix community profile import naming (I1)
7. Add missing test coverage for `validate()`, `resolved_method()`, legacy conversion
8. Surface diagnostics from Steam/Proton discovery to UI (I4, I5, I6)
9. Replace `InjectionSection` parallel arrays with `Vec<InjectionEntry>`
10. Introduce `LaunchMethod` enum in Rust
11. Add file-level docs to runtime helper scripts and crate root

### Low Priority (Suggestions)

12. Remove `SteamLaunchRequest` type alias and `ValidationResult` TS dead code
13. Separate file permissions for scripts vs `.desktop` files
14. Validate community import paths against known workspaces
15. Return `Result` from store constructors instead of panicking
16. Add remaining module-level doc comments
