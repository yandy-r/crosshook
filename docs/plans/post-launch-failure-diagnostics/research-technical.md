# Post-Launch Failure Diagnostics — Technical Specification

## Executive Summary

CrossHook's launch pipeline currently streams raw log lines to the frontend with no structured analysis of failures. When a game or trainer process exits with a non-zero code, the user sees hundreds of unprocessed WINE/Proton debug lines with no actionable guidance. This specification designs a new `launch/diagnostics` submodule in `crosshook-core` that: (1) analyzes exit codes and Unix signals, (2) pattern-matches common WINE/Proton failure modes in log output, and (3) surfaces structured `DiagnosticReport` objects to the frontend via a new Tauri event channel. Crash report collection from Proton prefixes is deferred to Phase 2.

The design follows the established `ValidationError` / `LaunchValidationIssue` pattern from `#39` for consistency — reusing `ValidationSeverity` and the `{message, help, severity}` shape for actionable suggestions. The data-driven pattern catalog follows the `LAUNCH_OPTIMIZATION_DEFINITIONS` precedent in `optimizations.rs`.

---

## Architecture Design

### Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│  Frontend (React/TypeScript)                            │
│                                                         │
│  ConsoleView ← listen("launch-log")                    │
│  DiagnosticBanner ← listen("launch-diagnostic")  [NEW] │
│  useLaunchState ← LaunchPhase + DiagnosticReport  [MOD]│
└────────────────┬────────────────────────────────────────┘
                 │ Tauri IPC / Events
┌────────────────┴────────────────────────────────────────┐
│  src-tauri/src/commands/launch.rs                [MOD]  │
│                                                         │
│  stream_log_lines() ─→ on child exit:                   │
│    1. Capture ExitStatus (code + signal)                │
│    2. safe_read_tail(log_path, 2 MiB)                   │
│    3. Call launch::diagnostics::analyze()               │
│    4. Emit "launch-diagnostic" event                    │
│    5. Emit "launch-complete" event                      │
└────────────────┬────────────────────────────────────────┘
                 │ Uses
┌────────────────┴────────────────────────────────────────┐
│  crosshook-core/src/launch/diagnostics/  [NEW SUBMOD]   │
│                                                         │
│  mod.rs          ─ Public API: analyze()                │
│  exit_codes.rs   ─ Unix signal + exit code translation  │
│  patterns.rs     ─ Data-driven failure pattern catalog  │
│  models.rs       ─ Data types (DiagnosticReport, etc.)  │
└─────────────────────────────────────────────────────────┘
```

### New Submodule: `crosshook-core/src/launch/diagnostics/`

Located at `crates/crosshook-core/src/launch/diagnostics/`, co-located with the launch pipeline it serves. This follows the codebase convention of domain-scoped modules (`launch/`, `steam/`, `profile/`, `export/`) rather than a top-level `diagnostics/` module.

**Key design principles**:

1. **Post-hoc analyzer**: Runs once after child process exits, not during streaming. Avoids CPU overhead during gaming.
2. **Pure analysis function**: `analyze(exit_code, signal, log_content, method) -> DiagnosticReport` — no I/O, no side effects, fully testable without WINE/Proton.
3. **Data-driven pattern catalog**: Static `const FAILURE_PATTERN_DEFINITIONS` array following the `LAUNCH_OPTIMIZATION_DEFINITIONS` precedent in `optimizations.rs`.
4. **Reuses existing severity model**: `ValidationSeverity` from `launch/request.rs` — no new severity enum.

### Integration Points

1. **`stream_log_lines()` in `launch.rs`** (lines 121-172): The current function discards exit status at line 150 (`Ok(Some(_)) => break`). This is the primary integration point — capture `ExitStatus`, read the log tail, call `diagnostics::analyze()`, and emit the result.

2. **`useLaunchState` hook**: Will listen for the new `launch-diagnostic` event and store the report in state. The `LaunchFeedback` type union will gain a new `diagnostic` kind alongside existing `validation` and `runtime` kinds.

3. **`DiagnosticCollector` in `steam/diagnostics.rs`**: Used internally during analysis for deduplication of pattern matches, then converted to the typed `DiagnosticReport` output. The collector is a processing tool, not a return type.

4. **`crosshook-cli`**: The same `analyze()` function can be called from `crosshook-cli/src/main.rs` where exit status is already captured, enabling CLI diagnostic output with zero additional logic.

---

## Data Models

### Rust Types (`crosshook-core/src/launch/diagnostics/models.rs`)

```rust
use serde::{Deserialize, Serialize};

use crate::launch::request::ValidationSeverity;

/// Structured interpretation of a process exit code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitCodeInfo {
    /// Raw exit code from the process (0-255), if available.
    pub code: Option<i32>,
    /// Unix signal number that terminated the process, if any.
    pub signal: Option<i32>,
    /// Human-readable label, e.g. "SIGSEGV (Segmentation fault)".
    pub label: String,
    /// One-line explanation of what this exit typically means.
    pub description: String,
    /// Whether the process produced a core dump (Unix only).
    pub core_dumped: bool,
    /// Severity based on exit type.
    pub severity: ValidationSeverity,
}

/// A log pattern that was matched, with context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternMatch {
    /// Internal identifier for the failure pattern, e.g. "ntdll_load_failure".
    pub pattern_id: String,
    /// Human-readable title, e.g. "Missing ntdll.dll".
    pub title: String,
    /// The matched log line(s) that triggered this pattern (sanitized for display).
    pub matched_lines: Vec<String>,
    /// Line numbers in the log file where matches were found.
    pub line_numbers: Vec<usize>,
    /// Severity of this pattern.
    pub severity: ValidationSeverity,
}

/// A single actionable suggestion for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionableSuggestion {
    /// Short imperative action, e.g. "Reinstall the Proton version".
    pub action: String,
    /// Longer explanation of why this may help.
    pub reason: String,
    /// Optional link to documentation or community resource.
    pub doc_url: Option<String>,
}

/// Represents a failure mode category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    // --- Process Signal category ---
    /// Process was killed by a Unix signal.
    SignalKill,
    /// Out of memory or OOM killed.
    OutOfMemory,

    // --- Proton Runtime category ---
    /// WINE/Proton couldn't load a critical DLL.
    ProtonDllLoadFailure,
    /// Executable format not recognized.
    BadExeFormat,
    /// WINEPREFIX path is wrong or corrupted.
    WinePrefixInvalid,
    /// Proton version incompatible with the game.
    ProtonVersionMismatch,

    // --- Configuration category ---
    /// Wrong executable architecture (32-bit vs 64-bit).
    ArchitectureMismatch,
    /// File permission issue prevents execution.
    FilePermissionDenied,
    /// Flatpak sandbox blocking filesystem access.
    FlatpakSandboxRestriction,

    // --- Compatibility category ---
    /// .NET Framework or runtime missing in prefix.
    DotNetMissing,
    /// Visual C++ redistributable missing.
    VcRedistMissing,
    /// Anti-cheat system interfering with trainer.
    AntiCheatInterference,
    /// Trainer version doesn't match game version.
    TrainerVersionMismatch,
    /// Launch timing issue (game not ready when trainer starts).
    LaunchTimingFailure,

    // --- Fallback ---
    /// Generic crash with no recognized pattern.
    UnknownCrash,
}

/// Complete diagnostic report generated after a launch failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    /// Exit code analysis.
    pub exit_info: ExitCodeInfo,
    /// Detected failure mode, if any pattern was matched.
    pub failure_mode: Option<FailureMode>,
    /// All matched log patterns, ordered by severity (max 50).
    pub pattern_matches: Vec<PatternMatch>,
    /// Actionable suggestions, ordered by relevance.
    pub suggestions: Vec<ActionableSuggestion>,
    /// Summary sentence for the diagnostic banner.
    pub summary: String,
    /// Overall severity of the diagnostic report.
    pub severity: ValidationSeverity,
    /// ISO 8601 timestamp of when the analysis was performed.
    pub analyzed_at: String,
    /// The launch method used (steam_applaunch, proton_run, native).
    pub launch_method: String,
    /// Target identifier (game slug or app ID).
    pub target: String,
}

/// Max diagnostic entries per analysis run (security bound).
pub const MAX_DIAGNOSTIC_ENTRIES: usize = 50;
/// Max log bytes to read for pattern analysis (2 MiB).
pub const MAX_LOG_BYTES: usize = 2 * 1024 * 1024;
```

### Rust Types (`crosshook-core/src/launch/diagnostics/exit_codes.rs`)

```rust
use crate::launch::request::ValidationSeverity;
use super::models::ExitCodeInfo;

/// Analyze a process exit status into a structured ExitCodeInfo.
/// Pure function — no I/O, fully testable.
///
/// `core_dumped` is available via `ExitStatusExt::core_dumped()` on Unix.
pub fn analyze_exit_status(
    code: Option<i32>,
    signal: Option<i32>,
    core_dumped: bool,
) -> ExitCodeInfo {
    if let Some(signal) = signal {
        let mut info = analyze_signal(signal);
        info.core_dumped = core_dumped;
        return info;
    }

    match code {
        Some(0) => ExitCodeInfo {
            code: Some(0),
            signal: None,
            core_dumped: false,
            label: "Success".to_string(),
            description: "Process exited normally.".to_string(),
            severity: ValidationSeverity::Info,
        },
        Some(1) => ExitCodeInfo {
            code: Some(1),
            signal: None,
            core_dumped,
            label: "General error".to_string(),
            description: "Process exited with a general error.".to_string(),
            severity: ValidationSeverity::Warning,
        },
        // Codes 128+N indicate killed by signal N
        Some(code) if code > 128 => {
            let mut info = analyze_signal(code - 128);
            info.core_dumped = core_dumped;
            info
        }
        Some(code) => ExitCodeInfo {
            code: Some(code),
            signal: None,
            core_dumped,
            label: format!("Exit code {code}"),
            description: format!("Process exited with code {code}."),
            severity: ValidationSeverity::Warning,
        },
        None => ExitCodeInfo {
            code: None,
            signal: None,
            core_dumped: false,
            label: "Unknown".to_string(),
            description: "Process exit status could not be determined.".to_string(),
            severity: ValidationSeverity::Warning,
        },
    }
}

fn analyze_signal(signal: i32) -> ExitCodeInfo {
    let (label, description, severity) = match signal {
        6 => (
            "SIGABRT (Aborted)",
            "Process called abort() — typically an assertion failure or unrecoverable error in WINE/Proton.",
            ValidationSeverity::Fatal,
        ),
        9 => (
            "SIGKILL (Force killed)",
            "Process was forcefully terminated, often by the OOM killer or the system. Check available memory.",
            ValidationSeverity::Fatal,
        ),
        11 => (
            "SIGSEGV (Segmentation fault)",
            "Process attempted to access invalid memory. Common with Proton version mismatches or corrupted prefixes.",
            ValidationSeverity::Fatal,
        ),
        15 => (
            "SIGTERM (Terminated)",
            "Process was asked to terminate. This may be normal if the user closed the game.",
            ValidationSeverity::Info,
        ),
        _ => (
            "Unknown signal",
            "Process was terminated by an unrecognized signal.",
            ValidationSeverity::Warning,
        ),
    };

    ExitCodeInfo {
        code: Some(128 + signal),
        signal: Some(signal),
        label: label.to_string(),
        description: description.to_string(),
        severity,
    }
}
```

### Rust Types (`crosshook-core/src/launch/diagnostics/patterns.rs`)

Follows the `LAUNCH_OPTIMIZATION_DEFINITIONS` data-driven pattern from `optimizations.rs`:

```rust
use crate::launch::request::ValidationSeverity;
use super::models::{FailureMode, PatternMatch, ActionableSuggestion, MAX_DIAGNOSTIC_ENTRIES};

struct FailurePatternDef {
    id: &'static str,
    title: &'static str,
    /// Case-insensitive substring markers — any match triggers this pattern.
    /// For WINE-specific patterns, markers can include the `err:` prefix
    /// (e.g., `"err:module:import_dll"`) to reduce false positives from game
    /// output that happens to contain similar text. The WINE debug format
    /// (`err:channel:function message`) is stable across Wine/Proton versions.
    markers: &'static [&'static str],
    failure_mode: FailureMode,
    severity: ValidationSeverity,
    /// (action, reason) pairs for suggestions.
    suggestions: &'static [(&'static str, &'static str)],
    /// Launch methods this pattern applies to. Empty = all methods.
    applies_to_methods: &'static [&'static str],
}

const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDef] = &[
    FailurePatternDef {
        id: "ntdll_load_failure",
        title: "Failed to load ntdll.dll",
        markers: &["could not load ntdll.dll", "ntdll.dll not found"],
        failure_mode: FailureMode::ProtonDllLoadFailure,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Reinstall the Proton version", "The Proton installation may be corrupted or incomplete."),
            ("Delete and recreate the WINE prefix", "A corrupted prefix can prevent DLL loading."),
        ],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    FailurePatternDef {
        id: "bad_exe_format",
        title: "Bad EXE format",
        markers: &["Bad EXE format", "not a valid Win32 application", "is not a Windows program"],
        failure_mode: FailureMode::BadExeFormat,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Check the executable architecture", "The game or trainer may be 64-bit while the prefix is 32-bit, or vice versa."),
            ("Verify you selected the correct .exe file", "Some games have multiple executables (launcher vs game)."),
        ],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    FailurePatternDef {
        id: "dotnet_missing",
        title: ".NET Framework missing",
        markers: &["System.IO.FileNotFoundException", "mscorlib.dll", "Could not load file or assembly"],
        failure_mode: FailureMode::DotNetMissing,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Install .NET in the prefix using winetricks or protontricks", "Many trainers require .NET Framework 4.x to run."),
        ],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    FailurePatternDef {
        id: "vcredist_missing",
        title: "Visual C++ runtime missing",
        markers: &["VCRUNTIME", "vcruntime140", "MSVCP140", "api-ms-win-crt"],
        failure_mode: FailureMode::VcRedistMissing,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Install Visual C++ Redistributable using protontricks", "The trainer needs vcredist to run."),
        ],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    FailurePatternDef {
        id: "wineprefix_invalid",
        title: "WINEPREFIX path issue",
        markers: &["WINEPREFIX is not a valid", "prefix is not initialized", "wineprefix not found"],
        failure_mode: FailureMode::WinePrefixInvalid,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Verify the prefix path in the profile settings", "The WINEPREFIX path may have been moved or deleted."),
            ("Launch the game through Steam once to recreate the prefix", "Steam creates the prefix on first run."),
        ],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    FailurePatternDef {
        id: "permission_denied",
        title: "Permission denied",
        markers: &["Permission denied", "EACCES", "Operation not permitted"],
        failure_mode: FailureMode::FilePermissionDenied,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Check file permissions on the game and trainer executables", "Ensure the files have execute permission (chmod +x)."),
            ("If running inside Flatpak, grant filesystem access", "Flatpak may restrict access to paths outside the sandbox."),
        ],
        applies_to_methods: &[],  // applies to all methods
    },
    FailurePatternDef {
        id: "flatpak_sandbox",
        title: "Flatpak sandbox restriction",
        markers: &["bwrap:", "flatpak-spawn", "No such file or directory inside sandbox"],
        failure_mode: FailureMode::FlatpakSandboxRestriction,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Grant Flatpak access to the trainer directory", "Use `flatpak override --filesystem=` to allow access."),
            ("Move the trainer into a Flatpak-accessible location", "~/Games or a custom path granted to Steam."),
        ],
        applies_to_methods: &[],
    },
    FailurePatternDef {
        id: "anti_cheat",
        title: "Anti-cheat interference",
        markers: &["EasyAntiCheat", "BattlEye", "anti-cheat", "VAC"],
        failure_mode: FailureMode::AntiCheatInterference,
        severity: ValidationSeverity::Warning,
        suggestions: &[
            ("Disable anti-cheat or play offline", "Most trainers cannot coexist with active anti-cheat systems."),
        ],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    FailurePatternDef {
        id: "arch_mismatch",
        title: "Architecture mismatch",
        markers: &["wrong ELF class", "ELFCLASS32", "ELFCLASS64", "cannot execute binary file"],
        failure_mode: FailureMode::ArchitectureMismatch,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Verify the Proton version matches the game architecture", "Some games require a specific 32/64-bit Proton build."),
        ],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    FailurePatternDef {
        id: "oom_killed",
        title: "Out of memory",
        markers: &["Out of memory", "Cannot allocate memory", "oom-kill"],
        failure_mode: FailureMode::OutOfMemory,
        severity: ValidationSeverity::Fatal,
        suggestions: &[
            ("Close other applications to free memory", "The Steam Deck has limited RAM; close background apps."),
            ("Increase swap space", "Adding swap can help prevent OOM kills on memory-constrained devices."),
        ],
        applies_to_methods: &[],
    },
];

/// Scan log content for known failure patterns using case-insensitive substring matching.
///
/// Returns matched patterns ordered by severity (Fatal first), capped at MAX_DIAGNOSTIC_ENTRIES.
/// The `method` parameter filters patterns by `applies_to_methods` (empty = match all).
///
/// Pure function — no I/O.
pub fn scan_log_patterns(
    log_content: &str,
    method: &str,
) -> (Vec<PatternMatch>, Option<FailureMode>) {
    let log_lower = log_content.to_lowercase();
    let mut matches = Vec::new();
    let mut primary_failure_mode: Option<FailureMode> = None;

    for def in FAILURE_PATTERN_DEFINITIONS {
        // Skip patterns that don't apply to this launch method.
        if !def.applies_to_methods.is_empty()
            && !def.applies_to_methods.contains(&method)
        {
            continue;
        }

        let mut matched_lines = Vec::new();
        let mut line_numbers = Vec::new();

        for (line_number, line) in log_content.lines().enumerate() {
            let line_lower = line.to_lowercase();
            if def.markers.iter().any(|marker| line_lower.contains(marker)) {
                matched_lines.push(sanitize_display_path(line));
                line_numbers.push(line_number + 1);
            }
        }

        if !matched_lines.is_empty() {
            if primary_failure_mode.is_none() {
                primary_failure_mode = Some(def.failure_mode.clone());
            }
            matches.push(PatternMatch {
                pattern_id: def.id.to_string(),
                title: def.title.to_string(),
                matched_lines,
                line_numbers,
                severity: def.severity,
            });
        }

        if matches.len() >= MAX_DIAGNOSTIC_ENTRIES {
            break;
        }
    }

    // Sort by severity: Fatal > Warning > Info
    matches.sort_by_key(|m| match m.severity {
        ValidationSeverity::Fatal => 0,
        ValidationSeverity::Warning => 1,
        ValidationSeverity::Info => 2,
    });

    (matches, primary_failure_mode)
}

/// Replace $HOME with ~ in paths for display to avoid information disclosure
/// in user screenshots. Also truncates lines longer than 500 chars.
fn sanitize_display_path(line: &str) -> String {
    let sanitized = if let Ok(home) = std::env::var("HOME") {
        line.replace(&home, "~")
    } else {
        line.to_string()
    };

    if sanitized.len() > 500 {
        format!("{}...", &sanitized[..497])
    } else {
        sanitized
    }
}
```

### TypeScript Types (`src/types/diagnostics.ts`)

```typescript
import type { LaunchValidationSeverity } from './launch';

export interface ExitCodeInfo {
  code: number | null;
  signal: number | null;
  core_dumped: boolean;
  label: string;
  description: string;
  severity: LaunchValidationSeverity;
}

export type FailureMode =
  | 'signal_kill'
  | 'out_of_memory'
  | 'proton_dll_load_failure'
  | 'bad_exe_format'
  | 'wine_prefix_invalid'
  | 'proton_version_mismatch'
  | 'architecture_mismatch'
  | 'file_permission_denied'
  | 'flatpak_sandbox_restriction'
  | 'dot_net_missing'
  | 'vc_redist_missing'
  | 'anti_cheat_interference'
  | 'trainer_version_mismatch'
  | 'launch_timing_failure'
  | 'unknown_crash';

export interface PatternMatch {
  pattern_id: string;
  title: string;
  matched_lines: string[];
  line_numbers: number[];
  severity: LaunchValidationSeverity;
}

export interface ActionableSuggestion {
  action: string;
  reason: string;
  doc_url: string | null;
}

export interface DiagnosticReport {
  exit_info: ExitCodeInfo;
  failure_mode: FailureMode | null;
  pattern_matches: PatternMatch[];
  suggestions: ActionableSuggestion[];
  summary: string;
  severity: LaunchValidationSeverity;
  analyzed_at: string;
  launch_method: string;
  target: string;
}
```

---

## API Design

### New Tauri Events

No new Tauri `#[tauri::command]` functions needed in Phase 1. Diagnostics are emitted as events from the existing `stream_log_lines()` function.

| Event Name          | Payload                                            | Direction          | Description                                                 |
| ------------------- | -------------------------------------------------- | ------------------ | ----------------------------------------------------------- |
| `launch-diagnostic` | `DiagnosticReport`                                 | Backend → Frontend | Emitted once after child process exits with non-zero status |
| `launch-complete`   | `{ code: number \| null, signal: number \| null }` | Backend → Frontend | Emitted when child process exits (success or failure)       |

### Modified Functions

#### `stream_log_lines()` in `src-tauri/src/commands/launch.rs`

**Current** (line 149-150):

```rust
match child.try_wait() {
    Ok(Some(_)) => break,
```

**Proposed**:

```rust
match child.try_wait() {
    Ok(Some(status)) => {
        // Extract exit code, signal, and core dump status
        let code = status.code();
        let (signal, core_dumped) = {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                (status.signal(), status.core_dumped())
            }
            #[cfg(not(unix))]
            { (None, false) }
        };

        // Emit completion event (always, success or failure)
        let _ = app.emit("launch-complete", serde_json::json!({
            "code": code,
            "signal": signal,
        }));

        // Run diagnostics only on non-zero exit
        if !status.success() {
            let log_content = safe_read_tail(&log_path, MAX_LOG_BYTES).await;
            let report = crosshook_core::launch::diagnostics::analyze(
                code,
                signal,
                core_dumped,
                &log_content,
                &launch_method,
            );
            let _ = app.emit("launch-diagnostic", &report);
        }
        break;
    }
```

#### `spawn_log_stream()` signature change

```rust
// Current
fn spawn_log_stream(app: AppHandle, log_path: PathBuf, child: tokio::process::Child)

// Proposed — add launch_method for diagnostic context
fn spawn_log_stream(app: AppHandle, log_path: PathBuf, child: tokio::process::Child, launch_method: String)
```

#### Public API: `crosshook-core::launch::diagnostics::analyze()`

```rust
/// Analyze a failed launch and produce a diagnostic report.
///
/// This is the primary entry point for post-launch diagnostics.
/// Pure function — no I/O, no side effects, fully testable.
pub fn analyze(
    exit_code: Option<i32>,
    signal: Option<i32>,
    log_content: &str,
    launch_method: &str,
) -> DiagnosticReport {
    let exit_info = exit_codes::analyze_exit_status(exit_code, signal);
    let (pattern_matches, failure_mode) = patterns::scan_log_patterns(log_content, launch_method);
    let suggestions = build_suggestions(&exit_info, &pattern_matches, &failure_mode);
    let summary = build_summary(&exit_info, &failure_mode);
    let severity = determine_overall_severity(&exit_info, &pattern_matches);

    DiagnosticReport {
        exit_info,
        failure_mode,
        pattern_matches,
        suggestions,
        summary,
        severity,
        analyzed_at: chrono::Utc::now().to_rfc3339(),
        launch_method: launch_method.to_string(),
        target: String::new(),  // Populated by caller
    }
}
```

#### Security Helper: `safe_read_tail()`

Added to `src-tauri/src/commands/shared.rs` or a new `src-tauri/src/io_helpers.rs`:

```rust
/// Read at most `max_bytes` from the end of a file.
/// Returns empty string on any I/O error.
pub async fn safe_read_tail(path: &Path, max_bytes: usize) -> String {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return String::new(),
    };

    let file_size = metadata.len() as usize;
    if file_size <= max_bytes {
        tokio::fs::read_to_string(path).await.unwrap_or_default()
    } else {
        // Read last max_bytes
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let mut file = match tokio::fs::File::open(path).await {
            Ok(f) => f,
            Err(_) => return String::new(),
        };
        let offset = file_size - max_bytes;
        if file.seek(std::io::SeekFrom::Start(offset as u64)).await.is_err() {
            return String::new();
        }
        let mut buffer = vec![0u8; max_bytes];
        let bytes_read = file.read(&mut buffer).await.unwrap_or(0);
        buffer.truncate(bytes_read);
        // Find the first newline to avoid splitting a line
        let start = buffer.iter().position(|&b| b == b'\n').map(|i| i + 1).unwrap_or(0);
        String::from_utf8_lossy(&buffer[start..]).into_owned()
    }
}
```

### LaunchFeedback Extension

The frontend `LaunchFeedback` type union gains a new `diagnostic` kind:

```typescript
// Current (types/launch.ts:42-44)
export type LaunchFeedback =
  | { kind: 'validation'; issue: LaunchValidationIssue }
  | { kind: 'runtime'; message: string };

// Proposed
export type LaunchFeedback =
  | { kind: 'validation'; issue: LaunchValidationIssue }
  | { kind: 'runtime'; message: string }
  | { kind: 'diagnostic'; report: DiagnosticReport };
```

---

## System Constraints

### Performance

| Concern              | Constraint                                      | Mitigation                                                                                                                                          |
| -------------------- | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Log file size        | WINE debug output can produce 10-50MB log files | `safe_read_tail()` caps at 2 MiB. Exit code analysis needs no log.                                                                                  |
| Pattern matching CPU | Scanning strings with multiple patterns         | Case-insensitive substring matching (`str::contains` on lowercased content). 10 patterns x 3 markers = ~30 searches on 2 MiB ≈ <50ms on Steam Deck. |
| Memory               | Steam Deck has 16GB RAM shared with GPU         | Log content read once, analyzed, and dropped. DiagnosticReport <10KB serialized.                                                                    |
| Output bound         | Many patterns could match in a large log        | Capped at `MAX_DIAGNOSTIC_ENTRIES` (50) per analysis run.                                                                                           |
| Blocking             | Analysis runs in async context                  | Pure computation on a string — fast enough for async runtime. Move to `spawn_blocking` if profiling shows >10ms.                                    |

### Steam Deck Specifics

- **Screen**: 1280x800 — diagnostic banner must be concise. Summary + expandable details.
- **Input**: Gamepad — suggestions must not require keyboard. "Copy to clipboard" OK; "type this command" is not.
- **Storage**: eMMC/SD card — `safe_read_tail()` uses seek, not full file read.

### Security

- **Bounded reads**: `safe_read_tail(path, 2 MiB)` prevents memory spikes from large logs.
- **Path sanitization**: `sanitize_display_path()` replaces `$HOME` with `~` in matched lines to prevent information disclosure in screenshots.
- **Line truncation**: Matched lines capped at 500 chars to prevent oversized event payloads.
- **No regex from user input**: All patterns are compile-time static strings.
- **Output bounds**: Max 50 diagnostic entries per analysis.

---

## Codebase Changes

### Files to Create

| File                                                         | Purpose                                                                                     |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/diagnostics/mod.rs`        | Submodule root, public `analyze()` API, re-exports                                          |
| `crates/crosshook-core/src/launch/diagnostics/models.rs`     | Data types: DiagnosticReport, FailureMode, ExitCodeInfo, PatternMatch, ActionableSuggestion |
| `crates/crosshook-core/src/launch/diagnostics/exit_codes.rs` | Exit code + signal → ExitCodeInfo (pure function)                                           |
| `crates/crosshook-core/src/launch/diagnostics/patterns.rs`   | `FAILURE_PATTERN_DEFINITIONS` catalog + `scan_log_patterns()`                               |
| `src/types/diagnostics.ts`                                   | TypeScript type definitions                                                                 |

### Files to Modify

| File                                      | Change                                                                                                                                               |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs` | Add `pub mod diagnostics;`                                                                                                                           |
| `src-tauri/src/commands/launch.rs`        | Modify `stream_log_lines()` to capture exit status + run diagnostics. Modify `spawn_log_stream()` to accept `launch_method`. Add `safe_read_tail()`. |
| `src/types/launch.ts`                     | Extend `LaunchFeedback` with `diagnostic` kind                                                                                                       |
| `src/types/index.ts`                      | Re-export diagnostics types                                                                                                                          |
| `src/hooks/useLaunchState.ts`             | Add `diagnosticReport` to state, listen for `launch-diagnostic` and `launch-complete` events                                                         |
| `src/components/ConsoleView.tsx`          | Display diagnostic banner when report is available                                                                                                   |

### Dependencies

**No new crate dependencies.** The implementation uses:

- `str::contains` / `str::to_lowercase` for pattern matching (stdlib)
- `chrono` for timestamps (already a dependency)
- `serde` for serialization (already a dependency)
- `tokio::io` for `safe_read_tail()` seek-based reads (already a dependency)

---

## Technical Decisions

### 1. Module Placement: `launch/diagnostics/` vs Top-Level `diagnostics/`

| Option                             | Pros                                                                        | Cons                                                                                               |
| ---------------------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| **`launch/diagnostics/`** (chosen) | Domain-scoped, follows codebase convention, co-located with launch pipeline | Slightly deeper import path                                                                        |
| `diagnostics/` (top-level)         | Shorter import, easier to find                                              | Breaks domain module convention. If non-launch diagnostics emerge later, structure becomes unclear |

**Decision**: `launch/diagnostics/`. Post-launch diagnostics is tightly coupled to the launch pipeline — it consumes exit codes from launch processes and analyzes launch log output. The codebase convention is domain-scoped modules (`launch/`, `steam/`, `profile/`, `export/`). Future domain-specific diagnostics (e.g., profile health) would live in their respective domains.

### 2. Severity Enum: Reuse `ValidationSeverity` vs New `DiagnosticSeverity`

| Option                                  | Pros                                                                   | Cons                                                                     |
| --------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| **Reuse `ValidationSeverity`** (chosen) | Zero frontend type changes, consistent with #39, single severity model | "Fatal" is slightly less semantic for post-crash context than "Critical" |
| New `DiagnosticSeverity`                | Better semantic fit                                                    | Parallel enum maintenance, frontend must handle two severity types       |

**Decision**: Reuse `ValidationSeverity` (Fatal/Warning/Info). The frontend already renders this type via `LaunchValidationSeverity` in `types/launch.ts:34`. The naming difference ("Fatal" vs "Critical") is cosmetic and not worth a separate type. If a fourth tier is needed later, extend the shared enum once.

### 3. Substring Matching vs Regex

| Option                          | Pros                                                               | Cons                                                                           |
| ------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| **Substring matching** (chosen) | Zero dependencies, ~10x faster, sufficient for WINE error patterns | Cannot capture structured data from log lines                                  |
| Regex (`regex` crate)           | More flexible, ReDoS-safe in Rust's `regex`                        | Adds ~400KB dependency, slower compilation, overkill for known static patterns |

**Decision**: Substring matching with `str::contains()` on lowercased content. WINE/Proton errors are identified by specific string markers. If a future pattern genuinely needs regex, add the `regex` crate at that time (Rust's `regex` is ReDoS-safe by design — O(m\*n) worst case, per security review).

### 4. Post-Hoc vs Real-Time Analysis

| Option                | Pros                                           | Cons                                                   |
| --------------------- | ---------------------------------------------- | ------------------------------------------------------ |
| **Post-hoc** (chosen) | Simple, no CPU during gaming, full log context | User waits until exit                                  |
| Real-time stream      | Immediate feedback                             | Complex state machine, CPU overhead, duplicate matches |

**Decision**: Post-hoc. Runs once after child exits. Aligns with the existing poll loop. Diagnostics appear within 500ms of exit.

### 5. Crash Report Collection: Phase 1 vs Deferred

| Option                           | Pros                                                               | Cons                                                                                      |
| -------------------------------- | ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| Phase 1                          | Complete feature                                                   | Adds filesystem I/O with user-controlled paths, path traversal risk, more testing surface |
| **Deferred to Phase 2** (chosen) | Simpler Phase 1, security review for path handling can be thorough | Feature gap — users don't see crash dumps initially                                       |

**Decision**: Defer crash report collection to Phase 2. Phase 1 focuses on exit code analysis + log pattern matching, which covers the highest-value use cases. Phase 2 adds `crash_reports.rs` with proper path canonicalization (`canonicalize()` + `starts_with()`) and symlink-safe file checks.

### 6. Event Architecture

| Option                  | Pros                                                | Cons                                     |
| ----------------------- | --------------------------------------------------- | ---------------------------------------- |
| **New events** (chosen) | Clean separation, follows `update-complete` pattern | Frontend listens to additional events    |
| Embed in `launch-log`   | No new events                                       | Mixes structured data with raw log lines |

**Decision**: New `launch-diagnostic` and `launch-complete` events. Follows the pattern established by `update-complete` in `update.rs` (line 141).

---

## Open Questions

1. **Log retention**: Should CrossHook persist diagnostic reports to `~/.config/crosshook/diagnostics/` for historical comparison? Currently logs go to volatile `/tmp/crosshook-logs/`. Defer to Phase 2.

2. **Trainer-specific patterns**: FLiNG version mismatch signatures, WeMod connection errors — include in Phase 1 pattern catalog or defer until community feedback identifies common trainer-specific failures?

3. **CLI consumer**: `crosshook-cli` already captures exit status. Should Phase 1 include CLI diagnostic output, or keep it Tauri-only? The pure `analyze()` function makes CLI integration trivial.

4. **`stream_log_lines` deduplication**: `launch.rs` and `update.rs` have near-identical streaming functions. Should this feature also extract a shared streaming utility? Related but orthogonal — recommend separate cleanup PR.

5. **Pattern contribution**: Should the pattern catalog be externalizable (e.g., TOML file) so community members can contribute patterns without rebuilding? Significant scope increase — defer to Phase 3.

---

## Relevant Files

| File                                                    | Role                                                                                 |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `src-tauri/src/commands/launch.rs:121-172`              | Primary integration point — `stream_log_lines()`, exit status discarded at line 150  |
| `crates/crosshook-core/src/launch/request.rs:143-156`   | `ValidationSeverity` enum + `LaunchValidationIssue` pattern to reuse                 |
| `crates/crosshook-core/src/launch/optimizations.rs:40+` | `LAUNCH_OPTIMIZATION_DEFINITIONS` — precedent for data-driven pattern catalog        |
| `crates/crosshook-core/src/launch/env.rs`               | WINE/Proton env vars referenced in diagnostic patterns                               |
| `crates/crosshook-core/src/steam/diagnostics.rs`        | `DiagnosticCollector` — internal dedup tool, used within analysis pipeline           |
| `src-tauri/src/commands/update.rs:139-141`              | `update-complete` event pattern — reference for `launch-complete`                    |
| `src-tauri/src/commands/shared.rs`                      | `create_log_path()` — logs at `/tmp/crosshook-logs/`, where `safe_read_tail()` reads |
| `src/components/ConsoleView.tsx`                        | Frontend log display — will show diagnostic banner                                   |
| `src/hooks/useLaunchState.ts`                           | Launch state management — will store DiagnosticReport                                |
| `src/types/launch.ts:34,42-44`                          | `LaunchValidationSeverity`, `LaunchFeedback` — types to extend                       |
| `src/utils/log.ts`                                      | Log payload normalization — diagnostic events use same Tauri event system            |
| `crates/crosshook-core/src/launch/runtime_helpers.rs`   | `resolve_wine_prefix_path()` — used in Phase 2 crash report discovery                |
| `crosshook-cli/src/main.rs`                             | CLI entry — potential Phase 1 consumer of `analyze()`                                |

## Team Feedback Incorporated

| Source                | Feedback                                                | Resolution                                                          |
| --------------------- | ------------------------------------------------------- | ------------------------------------------------------------------- |
| practices-researcher  | Module placement: `launch/diagnostics/` over top-level  | Adopted — domain-scoped modules follow codebase convention          |
| practices-researcher  | Reuse `ValidationSeverity` instead of new enum          | Adopted — zero frontend type changes, single severity model         |
| practices-researcher  | `DiagnosticCollector` as internal tool, not return type | Adopted — used for dedup during analysis, converted to typed output |
| security-researcher   | Bounded file reads (2 MiB max)                          | Adopted — `safe_read_tail()` with seek-based reads                  |
| security-researcher   | Path sanitization for display ($HOME → ~)               | Adopted — `sanitize_display_path()` in patterns.rs                  |
| security-researcher   | Bounded diagnostic output (50 entries max)              | Adopted — `MAX_DIAGNOSTIC_ENTRIES` constant                         |
| security-researcher   | Path canonicalization for crash report paths            | Deferred to Phase 2 with crash report collection                    |
| business-analyzer     | 6-category failure taxonomy                             | Adopted — `FailureMode` enum organized by category                  |
| business-analyzer     | Crash report time filter (last 5 minutes)               | Noted for Phase 2 implementation                                    |
| business-analyzer     | Timing rule: diagnostics only after process exit        | Adopted — post-hoc analysis design                                  |
| recommendations-agent | Data-driven pattern catalog (like optimizations.rs)     | Adopted — `FAILURE_PATTERN_DEFINITIONS` static array                |
| recommendations-agent | `applies_to_methods` field on patterns                  | Adopted — filters patterns by launch method                         |
