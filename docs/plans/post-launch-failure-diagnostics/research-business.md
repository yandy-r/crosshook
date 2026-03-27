# Post-Launch Failure Diagnostics: Business Logic & Requirements

## Executive Summary

CrossHook's current launch pipeline captures child process exit status and streams raw log lines but provides zero structured interpretation of failures. When a game or trainer launch fails, users see hundreds of lines of WINE/Proton debug output with no indication of what went wrong or how to fix it. This feature adds a diagnostic analysis layer between process exit and user-facing display: translating exit codes into human-readable messages, detecting common Proton/WINE error patterns in log output, and surfacing actionable suggestions.

The existing `ValidationError` / `LaunchValidationIssue` pattern (from #39) provides a proven severity + message + help model that this feature extends into the **post-launch** domain. The `DiagnosticCollector` in `steam/diagnostics.rs` offers a reusable diagnostic-collection pattern already in the codebase.

**Competitive differentiation**: No competing Linux launcher (Lutris, Bottles, Heroic) performs structured WINE log parsing. They all pass raw logs to the user. Lutris does pre-launch validation (which CrossHook already has via `ValidationError`), and Bottles has "Eagle" for pre-launch executable analysis, but nobody analyzes post-launch failure output. This makes CrossHook's diagnostics feature genuinely novel in the Linux gaming space.

## User Stories

### US-1: Steam Deck Gamer (Primary Persona)

> As a Steam Deck user who launches trainers through CrossHook, I want to understand **why** my game or trainer failed to launch so I can fix it without Googling raw WINE error codes.

- **Context**: Using gamepad-only navigation (Steam Deck Gaming Mode), limited ability to inspect raw log files. Needs concise, actionable feedback.
- **Acceptance**: After a failed launch, a structured diagnostic appears in the console/launch panel with severity, description, and at least one actionable suggestion.

### US-2: Linux Desktop Gamer

> As a Linux desktop gamer, I want CrossHook to tell me when a failure is caused by a known issue (wrong Proton version, missing .NET, bad prefix) so I can fix the configuration without trial and error.

- **Context**: Comfortable with terminal but should not need to manually grep through log files.
- **Acceptance**: The top 10 common failure modes are detected and produce specific remediation steps.

### US-3: Power User / Profile Author

> As a community profile author, I want detailed failure diagnostics I can include in bug reports or share with users who report compatibility issues.

- **Context**: Needs copy-to-clipboard diagnostic report, crash report collection, complete diagnostic details.
- **Acceptance**: Can copy a structured diagnostic summary (extending existing `LaunchPreview.to_display_toml()` clipboard pattern). Crash reports from `$STEAM_COMPAT_DATA_PATH/crashreports/` are surfaced when available.

## Business Rules

### BR-1: Exit Code Analysis

| Exit Code | Signal  | Human-Readable Message                                | Severity |
| --------- | ------- | ----------------------------------------------------- | -------- |
| 0         | -       | Launch completed successfully                         | Info     |
| 1         | -       | Generic failure (check log for details)               | Error    |
| 134       | SIGABRT | Process crashed (assertion failure or abort)          | Error    |
| 137       | SIGKILL | Process was killed (out of memory or force-killed)    | Error    |
| 139       | SIGSEGV | Segmentation fault (memory access violation)          | Error    |
| 143       | SIGTERM | Process was terminated externally                     | Warning  |
| 127       | -       | Command not found (executable missing or not in PATH) | Error    |
| 126       | -       | Permission denied (executable not runnable)           | Error    |

- **Rule**: Any non-zero exit from `child.try_wait()` triggers diagnostic analysis.
- **Rule**: Signal-based exits (128 + signal_number) are always translated before displaying the raw code.
- **Rule**: On Unix, `ExitStatus::code()` returns `None` when killed by signal. Use `ExitStatusExt::signal()` to extract the signal number separately. Exit code is `Option<i32>`, signal is `Option<i32>`.
- **Rule**: **Phase 1**: Diagnostics run only on `!status.success()` (non-zero exit). **Phase 1.1 (deferred)**: Exit code 0 triggers pattern scanning for Proton methods with `info` severity ceiling. WINE/Proton processes can exit 0 even when the game crashed internally, but scanning on every successful launch risks false-positive noise. The Phase 1.1 change is a one-line guard removal in `stream_log_lines()` for Proton methods. For `native` method, exit 0 always skips pattern scanning.
- **Edge case**: The shell scripts use `set -euo pipefail`, so intermediate failures within the helper script may produce exit codes from inner commands (e.g., `cp` failing returns 1, not a signal).
- **Edge case**: Steam client launch (`steam -applaunch`) returns immediately with exit 0 even if the game later fails; the actual game process exit is not directly captured in the `steam_applaunch` path. Diagnostics for this path are limited to log pattern detection only — no reliable exit code signal for the game process itself.
- **Edge case**: `LaunchResult.succeeded` currently reflects whether the process _spawned_, not whether it exited cleanly. Diagnostics must not conflate "spawn success" with "execution success".

### BR-2: Proton Error Pattern Detection

Patterns are matched against log output (the same lines streamed via `launch-log` event). Priority order, with detectability assessment from API research:

| Priority | Pattern                                          | Category                      | Detectability | Actionable Suggestion                                                                                                                   |
| -------- | ------------------------------------------------ | ----------------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| 1        | `could not load.*ntdll\.dll\|import_dll.*failed` | Proton Corruption / DLL Load  | HIGH          | "The WINE prefix appears corrupt. Delete the prefix and let Proton recreate it on next launch."                                         |
| 2        | `Bad EXE format`                                 | Architecture Mismatch         | HIGH          | "The executable may be the wrong architecture for this Proton version. Verify the game is 64-bit compatible with the selected Proton."  |
| 3        | `mscoree\.dll.*not found\|status c0000135`       | Missing .NET/vcredist         | HIGH          | "A .NET or Visual C++ runtime is missing. Install vcredist or .NET in the prefix using protontricks."                                   |
| 4        | `vkCreateInstance failed\|vulkan.*not available` | Vulkan/GPU Init Failure       | HIGH          | "Vulkan initialization failed. Check GPU drivers are installed and up to date."                                                         |
| 5        | `WINEPREFIX.*not.*exist\|wineprefix.*invalid`    | Wrong Prefix                  | MEDIUM        | "The configured WINEPREFIX path does not exist or is invalid. Check the prefix path in your profile."                                   |
| 6        | `err:module:.*\.dll.*not found`                  | Missing DLL                   | HIGH          | "A required DLL is missing. This may be a vcredist, .NET, or DirectX dependency."                                                       |
| 7        | `Permission denied\|EACCES`                      | File Permission               | MEDIUM        | "File permission error. Check that the game and trainer executables are readable."                                                      |
| 8        | `bwrap:.*bind.*denied\|flatpak.*sandbox`         | Flatpak Sandbox               | MEDIUM        | "Flatpak sandbox restriction detected. CrossHook may need additional permissions."                                                      |
| 9        | `anti-cheat\|EasyAntiCheat\|BattlEye`            | Anti-Cheat                    | MEDIUM        | "Anti-cheat software detected. Trainers are incompatible with anti-cheat protected games."                                              |
| 10       | `Connection refused\|timeout.*Steam`             | Steam Connectivity            | MEDIUM        | "Cannot connect to Steam client. Ensure Steam is running and logged in."                                                                |
| 11       | `wineboot.*failed\|prefix.*initialization`       | Prefix Init                   | MEDIUM        | "WINE prefix initialization failed. The prefix may need to be recreated."                                                               |
| 12       | (heuristic: runtime < 5s + non-zero exit)        | Launch Timing / Short Runtime | LOW           | "The process exited very quickly, which may indicate a startup failure. Check that the executable path and Proton version are correct." |

**Detectability tiers**: HIGH = reliable log signature, few false positives. MEDIUM = heuristic pattern, may need exit-code correlation. LOW = no reliable log signature, heuristic only.

- **Rule**: Patterns are matched case-insensitively against the accumulated log buffer.
- **Rule**: Multiple patterns can match; diagnostics are listed in priority order.
- **Rule**: Pattern matching runs after process exit (the full log is available) NOT during streaming. This avoids false positives from partial output.
- **Rule**: Log files can reach 10-50MB for WINE debug output. Pattern analysis is capped to the **last 2MB** of the log file to stay within Steam Deck memory constraints.
- **Rule**: Pattern matching is method-aware. `steam_applaunch` delegates to shell scripts with different failure signatures than `proton_run` which launches directly via `tokio::process::Command`. Native launches skip all WINE/Proton patterns.
- **Rule**: Patterns are designed to be **version-agnostic** across Proton 5.x through 9.x and Proton GE (GloriousEggroll) custom builds. Different Proton versions have different error signatures, but the patterns target the common WINE-layer error messages that appear across versions.
- **Rule**: Trainer version mismatch detection is pattern-based only (detecting generic error messages). The system cannot programmatically determine if trainer version X is compatible with game version Y. Detectability: LOW.
- **Edge case**: `steam_applaunch` logs come from the helper script, not directly from the game process. Pattern matching must work across both helper-script-prefixed lines (e.g., `[steam-helper] ...`) and raw WINE/Proton output.
- **Edge case**: Some patterns like `fixme:ntdll:.*not implemented` appear in _successful_ WINE runs. These must be combined with non-zero exit code or other failure indicators to avoid false positives. Exit code 0 with such patterns produces `info` severity, not `fatal`.

### BR-3: Diagnostic Severity Classification

**Diagnostics reuse the existing `ValidationSeverity` enum (`Fatal`, `Warning`, `Info`)** — no separate `DiagnosticSeverity` type. The frontend already renders `ValidationSeverity` badges in `LaunchPanel`, and Fatal vs Critical is a cosmetic distinction without behavioral difference. Zero mapping layer needed.

| Severity  | Meaning                                                                                 | Display Treatment      |
| --------- | --------------------------------------------------------------------------------------- | ---------------------- |
| `fatal`   | Launch failed with unrecoverable error                                                  | Red badge, shown first |
| `warning` | Launch completed but issues detected, or pattern match that indicates potential problem | Orange badge           |
| `info`    | Informational diagnostic (e.g., exit 0 with notes, crash report reference)              | Blue/grey badge        |

- **Rule**: Exit code 0 never produces a `fatal` diagnostic (but may produce `warning` if error patterns detected in log — see BR-1 note on WINE exit 0 with internal crash).
- **Rule**: Signal-based exits (SIGSEGV, SIGABRT, SIGKILL) are always `fatal`.
- **Rule**: Pattern-detected issues inherit severity from the pattern definition, but severity is **downgraded** when exit code is 0 (e.g., a "missing DLL" pattern with exit 0 becomes `warning` instead of `fatal`). This downgrade is a post-processing step in `analyze()` before returning the report.
- **Rule**: Anti-cheat detection and similar non-fatal pattern matches are `warning` level, not `fatal`.

### BR-4: Crash Report Collection

- **Rule**: After non-zero exit from Proton methods (`proton_run`, `steam_applaunch`), check for crash reports in `$STEAM_COMPAT_DATA_PATH/crashreports/` (or `$compatdata_path/crashreports/`).
- **Rule**: **Crash report path resolution is method-dependent**: For `steam_applaunch`, use `steam.compatdata_path`. For `proton_run`, use `runtime.prefix_path` (which may differ from `steam.compatdata_path`). The `resolve_proton_paths()` helper in `runtime_helpers.rs` handles the `pfx/` subdirectory heuristic — crash reports live at the compat_data level, not inside `pfx/`.
- **Rule**: Only collect crash reports modified within the last 5 minutes (avoids surfacing stale reports from previous sessions).
- **Rule**: Report crash dump **metadata only**: file count, total size, and timestamps. Do NOT read file contents into the frontend — crash dumps are binary and contain sensitive memory contents (see BR-10).
- **Rule**: Native launch method (`native`) skips crash report collection entirely.
- **Edge case**: `crashreports/` directory may not exist; this is normal and not an error.
- **Edge case**: Long game loading times could produce stale crash reports from a _previous_ session that fall within the 5-minute freshness window. Tightening to 2 minutes or recording the launch-start timestamp and filtering to "modified after launch started" would improve accuracy.
- **Open**: Should the feature also check `$PROTON_CRASH_REPORT_DIR` (user-configured override)? This variable may point outside the compatdata tree, which introduces path traversal risk and requires validation that the path is within expected boundaries.

### BR-5: Trainer vs Game Failure Differentiation

- **Rule**: The two-step launch flow (game first, trainer second) means failures can happen at different phases. Diagnostics must know **which process** failed: game or trainer.
- **Rule**: `launch_game` and `launch_trainer` are separate Tauri commands (lines 44 and 76 of `launch.rs`), each spawning their own child process and log file. `stream_log_lines()` is already called separately for each. Diagnostics are scoped to the specific child that exited.
- **Rule**: The `analyze()` function receives a `target_kind` parameter (`"game"` or `"trainer"`) alongside `launch_method`. This tailors diagnostic messages (e.g., "Game crashed" vs "Trainer crashed") and enables target-specific pattern matching (e.g., trainer staging failures only apply to trainer launches).
- **Rule**: Helper scripts log exit codes for both game and trainer Proton runs separately (e.g., `"Trainer proton run exited with code $exit_code"` in `steam-launch-helper.sh:388`). Pattern matching can detect these to provide phase-specific diagnostics even for `steam_applaunch`.

### BR-6: Diagnostic Scope and Boundaries

- **Rule**: Diagnostics focus on **trainer orchestration failures** — the domain CrossHook owns. They must NOT expand into a general-purpose WINE debugging tool.
- **Rule**: Diagnostic suggestions should reference CrossHook-specific remediation (change profile settings, re-run auto-populate, adjust prefix path) before suggesting general WINE/Proton troubleshooting.
- **Rule**: The system acknowledges when a failure is outside CrossHook's control (e.g., "This appears to be a game-level crash, not a CrossHook configuration issue").
- **Rule**: No new Tauri IPC commands needed initially — diagnostics flow through Tauri events (`launch-diagnostic`, `launch-complete`), following the existing `update-complete` event pattern in `update.rs`.

### BR-7: Diagnostic Timing

- **Rule**: Diagnostics run **after** `child.try_wait()` returns `Some(status)` (process exited), never during active streaming. This is post-hoc analysis only — no real-time diagnostic feedback during the gaming session.
- **Rule**: The `stream_log_lines` function currently breaks out of its loop when the process exits and does a final log read. Diagnostic analysis should run after this final read, using the complete log content.
- **Rule**: For `steam_applaunch` method, the game process exit may not be directly observed (Steam manages the process). Diagnostics in this path focus on helper script exit code and log patterns.

### BR-8: Diagnostic Lifecycle

- **Rule**: Each launch produces its own diagnostic context. Diagnostics from a previous launch must not bleed into the current session.
- **Rule**: The `useLaunchState` reducer resets state on `profileId` or `method` change. Diagnostics follow the same lifecycle.
- **Rule**: Diagnostics are associated with a specific `helperLogPath` and the exit status of that specific child process.

### BR-9: Diagnostic Output Structure

A diagnostic entry consists of:

```
DiagnosticReport {
  target_kind: "game" | "trainer"         // Which process failed (per BR-5)
  launch_method: string                   // "steam_applaunch" | "proton_run" | "native"
  exit_code: i32?                         // None when killed by signal
  signal: i32?                            // None when exited normally
  proton_version: string?                 // From $STEAM_COMPAT_DATA_PATH/version file (per BR-11)
  entries: Vec<DiagnosticEntry>           // Sorted by severity then priority
  crash_reports: Vec<CrashReportRef>      // Metadata only, per BR-4/BR-10
}

DiagnosticEntry {
  severity: ValidationSeverity   // Reuses existing enum: fatal | warning | info
  category: string               // e.g., "exit_code", "proton_error", "crash_report"
  title: string                  // Human-readable summary (1 line)
  detail: string                 // Explanation of what happened (paths sanitized per BR-10)
  suggestion: string             // What the user should do (paths sanitized per BR-10)
  matched_pattern: string?       // The log line that triggered this (optional)
                                 // Sanitized per BR-10: $HOME -> ~, capped at 512 chars
                                 // with word-boundary truncation + ellipsis
}

CrashReportRef {
  path: string                   // Sanitized per BR-10
  size_bytes: u64
  modified_at: string            // ISO 8601 timestamp
}
```

- `DiagnosticEntry` parallels the existing `LaunchValidationIssue { message, help, severity }` structure, extended with category and matched_pattern for post-launch context.
- `DiagnosticReport` wraps the entries with launch context (method, exit status, Proton version) to enable contextual display and clipboard export.

### BR-10: Proton Version Context

- **Rule**: For Proton launch methods (`proton_run`, `steam_applaunch`), diagnostics include the Proton version string in the `DiagnosticReport`. Read from the `version` file in `$STEAM_COMPAT_DATA_PATH/` (or the resolved compat data path per BR-4 method-dependent resolution).
- **Rule**: Proton version is **informational context**, not used for pattern matching logic. Patterns are version-agnostic (see BR-2). The version string is included in clipboard export to help with bug reports (e.g., "Proton 9.0-4" or "GE-Proton9-20").
- **Rule**: If the `version` file is missing or unreadable, `proton_version` is `None`. This is not an error — some standalone Proton setups or custom WINE builds may not have this file.
- **Rule**: Proton GE (GloriousEggroll) custom builds have different error behaviors than official Proton. The version string distinguishes these ("GE-Proton*" vs "proton-*") for user-facing context but does not change diagnostic logic.

### BR-11: Diagnostic Data Sensitivity

This rule governs what information diagnostics may expose to the user and through clipboard export.

- **Rule**: **Crash dump contents are never read or displayed.** Wine/Proton minidumps include thread stack contents that may contain passwords, API tokens, or session keys from game process memory. Diagnostics report crash dump _existence and metadata_ (file count, total size, timestamps) only.
- **Rule**: **Path sanitization in structured output.** Diagnostic `suggestion` and `detail` fields must replace `$HOME` (or the literal home directory path) with `~` before display and clipboard export. This prevents information disclosure when users share diagnostic output on ProtonDB, Reddit, or Steam forums. Raw log streaming in ConsoleView is pre-existing behavior and out of scope for this rule.
- **Rule**: **Environment variable presence, not values.** Diagnostic messages report _whether_ a relevant variable is set, not _what_ it is set to. Example: `"STEAM_COMPAT_DATA_PATH is not set"` (correct) vs `"STEAM_COMPAT_DATA_PATH is /home/yandy/.local/share/Steam/..."` (incorrect). Exception: variable names and generic path components (e.g., `pfx/`, `crashreports/`) may appear.
- **Rule**: **`matched_pattern` field sanitization and length cap.** When a diagnostic includes the log line that triggered it, the line must be path-sanitized (`$HOME` -> `~`) before inclusion. Users may copy-paste this line into support forums. After sanitization, the field is capped to **512 characters** — truncate at the last word boundary before 512 and append `…`. If no word boundary exists (single token longer than 512), hard-cut at 512. Rationale: WINE DLL load traces can exceed 1KB per line with verbose path chains; 512 chars captures the diagnostic-relevant error message while bounding IPC payload size (50 findings x 512 chars = ~25KB max).
- **Rule**: **No external data transmission.** Diagnostics are computed and displayed entirely locally. No diagnostic data is sent to any external service, telemetry endpoint, or network destination.
- **Edge case**: The existing `LaunchPreview.to_display_toml()` clipboard feature already includes unsanitized paths (game path, prefix path, etc.). BR-10 sanitization applies to the _new_ diagnostic clipboard output only; retrofitting path sanitization to existing preview output is a separate concern.

## Workflows

### Primary Workflow: Launch Failure with Diagnostics

1. User clicks "Launch Game" or "Launch Trainer" in `LaunchPanel`
2. `useLaunchState` dispatches `game-start` or `trainer-start`, validating via `validate_launch` IPC
3. `launch_game` / `launch_trainer` Tauri command spawns child process
4. `spawn_log_stream` -> `stream_log_lines` polls log file every 500ms, emitting `launch-log` events
5. `ConsoleView` receives events, appends lines to display
6. **Process exits** (`child.try_wait()` returns `Some(status)`)
7. `stream_log_lines` performs final log read
8. **NEW**: Diagnostic analysis runs:
   a. Read complete log content from `log_path`
   b. Analyze exit code (translate signals)
   c. Run pattern detection against full log buffer
   d. Check for crash reports if Proton method and non-zero exit
   e. Build `Vec<DiagnosticEntry>` sorted by severity then priority
9. **NEW**: Emit `launch-diagnostics` event with diagnostic payload to frontend
10. **NEW**: `ConsoleView` or `LaunchPanel` displays structured diagnostics

### Error Recovery Workflow

1. User sees diagnostic with actionable suggestion (e.g., "Delete prefix and recreate")
2. User follows suggestion, possibly closing CrossHook or adjusting profile
3. User re-launches; `useLaunchState` resets diagnostics on new launch
4. If same failure recurs, diagnostics are re-generated fresh (no memory of previous attempt)

### Native Launch Workflow (Simplified)

1. Native launches skip Proton-specific patterns entirely
2. Exit code analysis still applies (SIGSEGV, SIGABRT, etc.)
3. No crash report collection
4. Fewer patterns to match (no WINE/Proton-specific patterns)

## Domain Model

### Entities

- **LaunchDiagnostic**: Collection of `DiagnosticEntry` items for a single launch attempt
- **DiagnosticEntry**: A single diagnostic finding (exit code interpretation, pattern match, or crash report)
- **DiagnosticPattern**: A predefined pattern definition (regex, category, severity, suggestion template)
- **CrashReportRef**: File path + timestamp reference to a Proton crash dump

### State Transitions

```
LaunchPhase states (existing):
  Idle -> GameLaunching -> WaitingForTrainer -> TrainerLaunching -> SessionActive
                |                                    |
                v                                    v
              Idle (on failure)               WaitingForTrainer (on failure)

Diagnostic lifecycle (new, per-launch):
  None -> Analyzing -> Complete

  - None: No diagnostics (launch hasn't exited or exited with 0)
  - Analyzing: Process exited, running diagnostic analysis
  - Complete: Diagnostics ready for display
```

### Failure Modes Taxonomy

| Category                             | Detectability | Examples                                                       |
| ------------------------------------ | ------------- | -------------------------------------------------------------- |
| **Process Signal** (exit codes 128+) | HIGH          | SIGABRT, SIGSEGV, SIGKILL, SIGTERM                             |
| **Proton Runtime** (pattern-matched) | HIGH          | ntdll/kernel32 load failure, DLL missing, .NET/vcredist absent |
| **GPU/Graphics** (pattern-matched)   | HIGH          | Vulkan init failure, vkCreateInstance                          |
| **Configuration** (pattern-matched)  | MEDIUM        | Wrong architecture, file permissions, Flatpak sandbox          |
| **Compatibility** (pattern-matched)  | MEDIUM        | Anti-cheat interference (EAC, BattlEye)                        |
| **Environment** (pattern-matched)    | MEDIUM        | Steam not running, prefix corruption/init failure              |
| **Timing** (heuristic)               | LOW           | Short runtime (< 5s) + non-zero exit                           |
| **Trainer-specific** (heuristic)     | LOW           | Trainer version mismatch (no reliable log signature)           |
| **Unknown**                          | -             | Non-zero exit with no matched patterns (fallback diagnostic)   |

## Existing Codebase Integration

### Key Integration Points

| File                                                  | Role                                          | Integration                                                                                                                                                              |
| ----------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src-tauri/src/commands/launch.rs:121-172`            | `stream_log_lines()`                          | **Primary hook**: Add diagnostic analysis after the final log read (line 162-171), before the function returns. `child.try_wait()` at line 149 provides the exit status. |
| `src-tauri/src/commands/launch.rs:18-23`              | `LaunchResult`                                | **Extend or complement**: Currently has `succeeded: bool, message: String`. Diagnostics could be emitted as a separate event rather than modifying this struct.          |
| `crates/crosshook-core/src/launch/request.rs:143-156` | `ValidationSeverity`, `LaunchValidationIssue` | **Reuse pattern**: `DiagnosticEntry` should align with this severity model. Consider making severity a shared enum.                                                      |
| `crates/crosshook-core/src/steam/diagnostics.rs`      | `DiagnosticCollector`                         | **Reuse**: The collector pattern (add diagnostics, add hints, dedupe, finalize) maps directly to post-launch diagnostic collection.                                      |
| `src/hooks/useLaunchState.ts:14-18`                   | `LaunchState` type                            | **Extend**: Add optional diagnostics field to state.                                                                                                                     |
| `src/types/launch.ts:42-44`                           | `LaunchFeedback` type                         | **Extend**: Add `{ kind: 'diagnostic'; entries: DiagnosticEntry[] }` variant.                                                                                            |
| `src/components/ConsoleView.tsx`                      | Log display                                   | **Extend**: Display diagnostic summary after log streaming completes.                                                                                                    |
| `src/components/LaunchPanel.tsx:731-757`              | Feedback display                              | **Extend**: The existing validation/runtime feedback rendering can serve as a template for diagnostic display.                                                           |

### Existing Patterns to Leverage

1. **Severity + message + help** (`LaunchValidationIssue`): Proven UI pattern from #39 with badge + title + help text rendering in `LaunchPanel`.
2. **Event-based streaming** (`launch-log`): Add a parallel `launch-diagnostics` event for structured diagnostic data.
3. **DiagnosticCollector** (`steam/diagnostics.rs`): Diagnostic collection + deduplication + hints. Can be generalized or used as inspiration.
4. **Copy-to-clipboard** (`LaunchPreview.to_display_toml()`): Diagnostic reports should support the same clipboard export pattern for bug reports.
5. **Reducer-based state** (`useLaunchState`): Clean state machine for adding diagnostic lifecycle without disrupting existing flow.

### Critical Observations from Codebase

1. **`stream_log_lines` discards exit status**: Line 150 matches `Ok(Some(_))` and breaks but does NOT capture or propagate the exit code. This is the **primary gap** that must be addressed.
2. **Log file is the only output channel**: Both stdout and stderr are redirected to the log file via `attach_log_stdio()`. All WINE/Proton output lands in this file.
3. **Shell scripts log with `[steam-helper]` prefix**: Pattern matching must handle both prefixed helper messages and raw WINE output.
4. **`steam_applaunch` indirection**: The helper script launches `steam -applaunch` in the background and doesn't directly observe the game process exit code. Game crashes are only visible through log patterns, not exit codes.
5. **Proton `run` method is direct**: `build_proton_game_command` spawns `proton run game.exe` directly. Exit code is the actual game/WINE exit code.
6. **No existing Tauri event for process completion**: The frontend currently has no signal that the process has exited. It only stops receiving `launch-log` events.

## Success Criteria

1. **Exit code visibility**: Every non-zero exit code produces at least one human-readable diagnostic entry.
2. **Pattern coverage**: All 12 failure patterns from BR-2 are implemented. All HIGH-detectability patterns (6 of 12) must have zero known false positives. MEDIUM patterns may produce occasional false positives at `warning` severity.
3. **No false positives**: Pattern matching on accumulated log only (post-exit), not during streaming.
4. **Severity alignment**: Diagnostic severity uses the same enum and visual treatment as validation severity.
5. **Zero disruption**: The diagnostic layer is additive; existing launch flow, log streaming, and validation continue unchanged.
6. **Clipboard support**: Diagnostic summary can be copied for bug reports.
7. **Crash report awareness**: Crash dumps in `$STEAM_COMPAT_DATA_PATH/crashreports/` are surfaced as metadata references (count, size, timestamp — never contents).
8. **Proton version context**: Diagnostic reports include Proton version string for Proton launch methods (per BR-10).
9. **Data sensitivity**: All diagnostic output complies with BR-11 (path sanitization, env var presence-only, matched_pattern capped at 512 chars, no external transmission).

## Open Questions

1. **Should diagnostics block the UI or be shown inline?** The ConsoleView currently has no "end of stream" marker. Diagnostics need a clear visual separation from log output.
2. **Should pattern matching be extensible via community profile taps?** Community profiles already define game-specific configurations; game-specific diagnostic patterns could be a natural extension but adds complexity.
3. **How to handle `steam_applaunch` exit ambiguity?** The helper script exits 0 if it successfully asked Steam to launch; the actual game may crash later. **Phase 1**: Diagnostics only fire on non-zero exit. **Phase 1.1**: Exit-0 scanning for Proton methods with info severity ceiling (deferred to avoid false-positive noise).
4. **Should diagnostic patterns be updatable without app releases?** Shipping patterns in the binary is simpler but means new patterns require a new AppImage release. Scope creep risk per BR-6 suggests starting with compiled patterns.
5. ~~**Rate of diagnostic event emission**~~: **RESOLVED** — Single `launch-diagnostic` event with all diagnostics, following the `update-complete` event pattern. Events only, no new Tauri commands in Phase 1.
6. **Crash report freshness heuristic**: 5-minute window or launch-start-relative filtering? Recording the launch timestamp and filtering to "modified after launch started" would eliminate stale reports more reliably.
7. ~~**Severity mapping**~~: **RESOLVED** — Diagnostics reuse `ValidationSeverity` directly (Fatal/Warning/Info). No separate enum, no mapping layer. Frontend already renders these badges.
8. **Log size cap**: 2MB analysis cap is proposed for Steam Deck memory. Should this be configurable, or is a fixed cap sufficient? Power users analyzing complex failures may want the full log scanned.
9. **`$PROTON_CRASH_REPORT_DIR` override**: Should crash report collection also check this user-configurable env var? It can point outside the compatdata tree, introducing path traversal risk. If supported, the path must be validated (e.g., must be under `$HOME` or a known Steam directory).
10. **Path sanitization scope**: BR-10 sanitizes diagnostic output but not existing features (LaunchPreview clipboard, ConsoleView raw log). Should path sanitization be retrofitted to existing clipboard exports as a follow-up?
