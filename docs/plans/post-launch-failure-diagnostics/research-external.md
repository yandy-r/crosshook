# External Research: Post-Launch Failure Diagnostics

## Executive Summary

This document covers external APIs, libraries, integration patterns, and constraints for implementing structured post-launch failure diagnostics in CrossHook. The feature requires: (1) exit code / signal translation, (2) WINE/Proton error pattern detection in log output, (3) top-10 failure mode detection, and (4) crash report collection from Proton prefixes.

The recommended approach uses **Rust's standard library `ExitStatusExt`** for signal analysis (zero new dependencies), **`regex` crate with `RegexSet`** for multi-pattern log scanning (one well-maintained dependency, already used transitively), and **filesystem inspection** for crash report collection (no external API needed). The `minidump` crate is available but should be deferred to a later phase given its dependency weight and the rarity of parseable crash dumps in typical Proton usage.

**Confidence**: High -- all primary technical approaches are well-documented, use stable APIs, and have been validated against the existing CrossHook codebase.

---

## Primary APIs

### 1.1 Linux Signal Codes (Exit Code Analysis)

**Source**: POSIX standard, Linux `signal(7)` man page
**Documentation**: <https://man7.org/linux/man-pages/man7/signal.7.html>
**Auth/Rate Limits**: N/A (local OS API)

On Unix, when a process is terminated by a signal, the exit status encodes the signal number. The convention is `exit_code = 128 + signal_number`:

| Exit Code | Signal  | Number | Meaning                              |
| --------- | ------- | ------ | ------------------------------------ |
| 134       | SIGABRT | 6      | Process aborted (crash, assert)      |
| 137       | SIGKILL | 9      | Killed by OS (OOM killer, `kill -9`) |
| 139       | SIGSEGV | 11     | Segmentation fault (bad memory)      |
| 136       | SIGFPE  | 8      | Floating point exception             |
| 143       | SIGTERM | 15     | Graceful termination request         |
| 132       | SIGILL  | 4      | Illegal instruction                  |
| 133       | SIGTRAP | 5      | Trace/breakpoint trap                |
| 135       | SIGBUS  | 7      | Bus error (memory alignment)         |
| 141       | SIGPIPE | 13     | Broken pipe                          |

**Confidence**: High -- POSIX standard, stable for decades.

**Rust API**: `std::os::unix::process::ExitStatusExt` (stable since Rust 1.0+)

```rust
use std::os::unix::process::ExitStatusExt;

// After child.try_wait() returns Some(status):
if let Some(signal) = status.signal() {
    // signal is the raw signal number (e.g., 6, 9, 11)
    let core_dumped = status.core_dumped(); // bool, stable since 1.58
}
if let Some(code) = status.code() {
    // Normal exit with code (0 = success, non-zero = error)
}
```

Key methods on `ExitStatusExt`:

- `signal() -> Option<i32>` -- signal number if killed by signal (WIFSIGNALED/WTERMSIG)
- `core_dumped() -> bool` -- whether a core dump was produced (stable since 1.58)
- `code() -> Option<i32>` -- exit code if process exited normally (None if signaled)
- `stopped_signal() -> Option<i32>` -- signal that stopped the process (WIFSTOPPED)
- `into_raw(self) -> i32` -- raw wait status integer

**No external crate required.** The `nix` crate provides `WaitStatus` enum with similar functionality but adds unnecessary dependency weight for this use case.

### 1.2 Proton Environment Variables and Log Locations

**Source**: Valve Proton README
**Documentation**: <https://github.com/ValveSoftware/Proton#runtime-config-options>

Key Proton environment variables relevant to diagnostics:

| Variable                           | Purpose                                                       | Default             |
| ---------------------------------- | ------------------------------------------------------------- | ------------------- |
| `PROTON_LOG`                       | Enable debug logging (set to `1` or WINEDEBUG channel string) | Disabled            |
| `PROTON_LOG_DIR`                   | Directory for log output                                      | `$HOME`             |
| `PROTON_CRASH_REPORT_DIR`          | Directory for crash report output                             | Not set             |
| `STEAM_COMPAT_DATA_PATH`           | Root of game's compatibility data (wine prefix parent)        | Set by Steam client |
| `STEAM_COMPAT_CLIENT_INSTALL_PATH` | Steam client installation root                                | Set by Steam client |

Log file location: `$PROTON_LOG_DIR/steam-$APPID.log` (or `$HOME/steam-$APPID.log` by default).

CrossHook already sets `STEAM_COMPAT_DATA_PATH` and `WINEPREFIX` in `script_runner.rs:apply_steam_proton_environment()`. The prefix structure is:

```
$STEAM_COMPAT_DATA_PATH/
  pfx/                      # Wine prefix root
    drive_c/                # Virtual C: drive
    user.reg                # User registry
    system.reg              # System registry
  version                   # Proton version file
  config_info               # Config metadata
```

**Crash report location**: `$STEAM_COMPAT_DATA_PATH/crashreports/` (varies by Proton version; some versions use `$PROTON_CRASH_REPORT_DIR` if set).

**Confidence**: High -- directly from Valve's official documentation and validated against CrossHook's existing code.

### 1.3 WINE Debug Output Format

**Source**: WineHQ Developer Guide, `wine/include/wine/debug.h`
**Documentation**: <https://wiki.winehq.org/Debug_Channels>

WINE debug output follows a structured format with four severity classes:

| Class   | Prefix   | Default | Purpose                             |
| ------- | -------- | ------- | ----------------------------------- |
| `err`   | `err:`   | ON      | Serious errors                      |
| `fixme` | `fixme:` | ON      | Unimplemented features              |
| `warn`  | `warn:`  | OFF     | Suspicious but non-fatal conditions |
| `trace` | `trace:` | OFF     | General debug tracing               |

**Line format** (with `+tid` channel enabled):

```
XXXX:XXXX:channel:class message text
```

Where `XXXX:XXXX` is process_id:thread_id in hexadecimal.

**Without `+tid`** (default for PROTON_LOG=1):

```
class:channel:function_name message text
```

Example patterns from real WINE/Proton output:

```
err:module:import_dll Library ntdll.dll (which is needed by ...) not found
fixme:ntdll:NtQuerySystemInformation info_class SYSTEM_PERFORMANCE_INFORMATION
err:seh:call_vectored_handlers unhandled exception
wine: could not load ntdll.so: cannot open shared object file: No such file or directory
wine: could not load kernel32.dll, status c0000135
```

**WINEDEBUG configuration syntax**: `[class][+/-]channel[,[class2][+/-]channel2]`

**Confidence**: High -- format is stable across WINE versions (documented in debug.h since WINE 1.x).

### 1.4 ProtonDB and AreWeAntiCheatYet Data

**ProtonDB**: <https://www.protondb.com/>

- Community-driven game compatibility reports
- No public API; data scraped from website
- Useful for _reference_ but not for programmatic integration

**AreWeAntiCheatYet**: <https://areweanticheatyet.com/>

- **Source**: <https://github.com/AreWeAntiCheatYet/AreWeAntiCheatYet>
- **Data format**: `games.json` at repository root
- **License**: MIT
- **Access**: Clone repo or fetch raw JSON from GitHub
- Tracks anti-cheat compatibility status per game (EasyAntiCheat, BattlEye, etc.)
- Could be used offline to detect anti-cheat-related launch failures

**Confidence**: Medium -- ProtonDB has no stable API; AreWeAntiCheatYet data is stable but manual integration needed.

---

## 2. Libraries and SDKs (Rust Crates)

### 2.1 `regex` -- Pattern Matching (RECOMMENDED)

**Crate**: <https://crates.io/crates/regex>
**Version**: 1.12.x (latest stable)
**Downloads**: 574M+ total
**License**: MIT/Apache-2.0
**Maintainer**: Rust project (BurntSushi)
**MSRV**: 1.65.0

**Why this crate**: The `regex` crate provides `RegexSet` which can match multiple patterns simultaneously in a single pass -- ideal for scanning log lines against dozens of error patterns without repeated compilation or scanning.

```rust
use regex::RegexSet;

let error_patterns = RegexSet::new(&[
    r"(?i)could not load .+\.dll",
    r"(?i)Bad EXE format",
    r"(?i)failed to initialize",
    r"(?i)status c0000135",           // DLL not found status
    r"(?i)err:module:import_dll",
    r"(?i)err:seh:.*unhandled exception",
    r"(?i)Application could not be started",
    r"(?i)MESA-INTEL:.*error",
    r"(?i)vkCreateInstance failed",
    r"(?i)anti.?cheat",
]).unwrap();

let matches: Vec<usize> = error_patterns.matches(log_line).into_iter().collect();
```

**Key `RegexSet` limitation**: Reports _which_ patterns matched but not byte offsets or captures. For lines that match, a second pass with individual `Regex` objects extracts details.

**Performance pattern**: Use `std::sync::LazyLock` (stable since Rust 1.80) or `once_cell::sync::Lazy` to compile patterns once:

```rust
use std::sync::LazyLock;
use regex::RegexSet;

static ERROR_PATTERNS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new(&[
        // patterns here
    ]).expect("compiled error pattern set")
});
```

**Confidence**: High -- industry-standard Rust crate, maintained by Rust project, linear-time guarantees.

### 2.2 `aho-corasick` -- Multi-Pattern Literal Matching (ALTERNATIVE)

**Crate**: <https://crates.io/crates/aho-corasick>
**Version**: 1.1.x
**License**: MIT/Unlicense
**Maintainer**: BurntSushi (same as regex)

For literal string matching (not regex), `aho-corasick` is faster than `RegexSet` because it uses a finite state machine optimized for exact string searches with SIMD acceleration.

```rust
use aho_corasick::AhoCorasick;

let patterns = &[
    "could not load ntdll",
    "Bad EXE format",
    "failed to initialize",
    "status c0000135",
];
let ac = AhoCorasick::new(patterns).unwrap();

for mat in ac.find_iter(log_line) {
    println!("Pattern {} matched at [{}, {})", mat.pattern(), mat.start(), mat.end());
}
```

**Note**: `aho-corasick` is already a transitive dependency of `regex`, so it adds no new dependency weight if `regex` is used.

**Recommendation**: Use `RegexSet` for patterns that need regex flexibility (case-insensitive, wildcards); use `AhoCorasick` for exact literal strings like specific error messages.

**Confidence**: High -- well-maintained, already in dependency tree via regex.

### 2.3 `nom` -- Parser Combinator (EVALUATED, NOT RECOMMENDED)

**Crate**: <https://crates.io/crates/nom>
**Version**: 7.x
**License**: MIT
**Maintainer**: Geoffroy Couprie (rust-bakery)

`nom` is a parser combinator framework for building structured parsers. While powerful for parsing binary formats or complex grammars, it is **overkill for line-by-line log scanning**. WINE debug output is line-oriented text with well-known prefixes -- `regex` or `aho-corasick` is simpler and faster for this use case.

**When nom would be appropriate**: Parsing minidump binary format (but `minidump` crate already does this). Parsing structured WINE relay trace output (unlikely to be needed in Phase 1).

**Confidence**: High -- assessment based on comparing WINE log format simplicity against nom's complexity cost.

### 2.4 `pest` -- PEG Parser (EVALUATED, NOT RECOMMENDED)

**Crate**: <https://pest.rs/> / <https://crates.io/crates/pest>
**Version**: 2.x
**License**: MIT/Apache-2.0

`pest` uses Parsing Expression Grammars defined in external `.pest` files. Like `nom`, it is designed for structured grammar parsing and adds significant complexity for what is fundamentally line-based pattern matching.

**Confidence**: High -- not suited for log line scanning.

### 2.5 `minidump` / `minidump-processor` -- Crash Dump Parsing (PHASE 2)

**Crate**: <https://crates.io/crates/minidump>
**Version**: 0.26.1 (Nov 2025)
**License**: MIT
**Maintainer**: rust-minidump project (491 GitHub stars, active development)
**Dependencies**: `scroll`, `prost`, `memmap2`, `thiserror`, `uuid`, `debugid`, `time`, `procfs-core`

The `minidump` crate parses Microsoft minidump format files (`.dmp`) as produced by Breakpad/Crashpad. Key API:

```rust
use minidump::Minidump;

let mut dump = Minidump::read_path("crash.dmp")?;
let system_info = dump.get_stream::<MinidumpSystemInfo>()?;
let exception = dump.get_stream::<MinidumpException>()?;
let threads = dump.get_stream::<MinidumpThreadList>().unwrap_or_default();
```

Available streams: `MinidumpSystemInfo`, `MinidumpException`, `MinidumpThreadList`, `MinidumpMemoryList`, `MinidumpModuleList`, plus 13 additional streams.

**Critical caveat for Wine/Proton**: Sentry discovered that Wine's TEB (Thread Environment Block) implementation incorrectly reports stack boundaries, causing Breakpad/Crashpad to generate massive dumps (500MB+ instead of 50-80KB). This means:

1. Crash dumps from Proton games may be abnormally large
2. Stack traces from Wine-hosted processes may be unreliable
3. The `minidump-processor` stackwalking may produce inaccurate results for Wine processes

**Recommendation**: Defer minidump parsing to Phase 2. For Phase 1, simply detect the _existence_ of crash dump files and report their paths/sizes/timestamps to the user. Full parsing adds significant dependency weight (~10 crates) with limited reliability in the Wine environment.

**Confidence**: Medium -- crate is well-maintained but Wine compatibility issues reduce the value of deep parsing.

### 2.6 Standard Library -- Process Exit Handling (NO DEPENDENCY)

Rust's `std::process::ExitStatus` with the Unix extension trait provides everything needed:

```rust
use std::os::unix::process::ExitStatusExt;

fn analyze_exit_status(status: std::process::ExitStatus) -> DiagnosticResult {
    if status.success() {
        return DiagnosticResult::clean_exit();
    }

    if let Some(signal) = status.signal() {
        let core_dumped = status.core_dumped();
        return match signal {
            6  => DiagnosticResult::signal("SIGABRT", "Process crashed (abort signal). This typically indicates an assertion failure or memory corruption in the game or trainer.", core_dumped),
            9  => DiagnosticResult::signal("SIGKILL", "Process was forcibly killed. This may indicate the system ran out of memory (OOM killer) or the process was killed manually.", core_dumped),
            11 => DiagnosticResult::signal("SIGSEGV", "Segmentation fault (invalid memory access). The game or trainer attempted to access memory it shouldn't. This is usually a bug in the software.", core_dumped),
            15 => DiagnosticResult::signal("SIGTERM", "Process received termination request. This is a normal shutdown signal.", core_dumped),
            _  => DiagnosticResult::signal(&format!("Signal {signal}"), "Process was terminated by an unexpected signal.", core_dumped),
        };
    }

    if let Some(code) = status.code() {
        return DiagnosticResult::exit_code(code);
    }

    DiagnosticResult::unknown()
}
```

**Confidence**: High -- standard library, zero dependencies, stable API.

---

## Integration Patterns

### 3.1 How Competing Tools Handle Error Diagnostics

#### Lutris (Python, ~30K stars)

**Source**: <https://github.com/lutris/lutris>

- **Pre-launch validation**: Validates executable paths, Wine prefix integrity, architecture compatibility (32-bit vs 64-bit), esync/fsync kernel support, and DXVK availability _before_ launch
- **Categorized exceptions**: Uses typed Python exceptions (`EsyncLimitError`, `FsyncUnsupportedError`, `MisconfigurationError`, `MissingExecutableError`, `MissingGameExecutableError`, `SymlinkNotUsableError`)
- **Short-run detection**: Detects "The game has run for a very short time, did it crash?" as a heuristic
- **No structured log parsing**: Does NOT parse WINE debug output for error patterns; delegates to raw log display
- **Environment setup**: Suppresses WINE debug output by default (`WINEDEBUG="-all"`) for performance

**Key pattern to adopt**: Pre-launch validation with typed errors (CrossHook already has `ValidationError`). Short-run-time heuristic is valuable.

#### Bottles (Python/GTK, ~6K stars)

**Source**: <https://github.com/bottlesdevs/Bottles>

- **Eagle analyzer**: Performs multi-stage analysis on executables, scanning for frameworks, runtimes, and known issues (anti-cheat, DRM). Shows "Source" and "Context" for each detection
- **Log levels**: Implements `[INFO]`, `[WARNING]`, `[ERROR]`, `[CRITICAL]` wrappers around WINE output
- **No WINE log parsing**: Like Lutris, passes WINE debug output directly to terminal/UI
- **winedbg integration**: Provides process listing and basic debugger output via embedded winedbg

**Key pattern to adopt**: Eagle's "Source + Context" pattern for presenting diagnostic findings is excellent UX -- each detection shows what was found and where.

#### Heroic Games Launcher (TypeScript/Electron)

**Source**: <https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher>

- Stores logs at `~/.config/heroic/` for native and `~/.var/app/com.heroicgameslauncher.hgl/config/heroic/` for Flatpak
- Uses Legendary backend for game launching, not direct Wine/Proton invocation
- Error reporting is primarily "launch failed" with log path -- minimal structured analysis

**Key takeaway**: Even major launchers do NOT do structured WINE log parsing. This makes CrossHook's feature genuinely novel and differentiating.

### 3.2 Real-Time Log Pattern Matching Architecture

The recommended architecture for CrossHook:

```
stream_log_lines() loop (already exists)
    |
    v
[Read new log chunk] --> [Pattern Scanner] --> [Emit raw "launch-log" event]
                              |
                              v
                    [Accumulate matched patterns]
                              |
                    (on process exit)
                              v
                    [Build DiagnosticReport]
                              |
                    [Emit "launch-diagnostics" event]
```

**Pattern**: Scan each line through `RegexSet` during the existing 500ms poll loop. Accumulate matches into a `Vec<DetectedPattern>`. On process exit, combine exit code analysis + accumulated patterns + crash file check into a single `DiagnosticReport` struct emitted as a Tauri event.

This approach:

- Adds minimal latency to existing poll loop (RegexSet scans in linear time)
- Does not modify the existing `launch-log` event stream
- Provides a single structured diagnostic payload at process completion
- Keeps pattern definitions in crosshook-core (testable without Tauri)

### 3.3 Flatpak Sandbox Detection

CrossHook needs to detect if it's running inside a Flatpak sandbox, since this affects filesystem access and can cause launch failures.

```rust
fn is_flatpak_environment() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
        || std::env::var("FLATPAK_ID").is_ok()
}
```

**Confidence**: High -- both methods are documented by Flatpak project.

---

## 4. Constraints and Gotchas

### 4.1 WINE Debug Output Format Stability

- The `err:`, `fixme:`, `warn:`, `trace:` prefixes are stable across all WINE versions
- The debug channel names (e.g., `module`, `seh`, `ntdll`) are stable but new channels are added over time
- Proton patches may add or modify debug output compared to vanilla WINE
- Thread ID format in log lines varies depending on `WINEDEBUG` configuration

**Mitigation**: Use loose regex patterns that match the class prefix without depending on exact formatting.

### 4.2 Proton Crash Dump Format Variations

- **Standard Proton**: Uses Breakpad-format minidumps (`.dmp` files)
- **Proton GE / custom builds**: May use different crash handlers or disable crash reporting
- **Wine's TEB bug**: Crash dumps from Wine processes can be abnormally large (500MB+) due to incorrect stack boundary reporting (see Sentry's research)
- **Crash dump location varies**: `$STEAM_COMPAT_DATA_PATH/crashreports/`, `$PROTON_CRASH_REPORT_DIR`, or game-specific locations
- **Not all crashes produce dumps**: Only games using Breakpad/Crashpad generate minidumps; many simply exit with a signal

**Mitigation**: Phase 1 should only detect crash dump file _existence_ and report metadata (path, size, timestamp). Deep parsing deferred to Phase 2.

### 4.3 Log File Access and Timing

- CrossHook's existing `stream_log_lines()` reads the log file every 500ms
- Log file may not exist until the process starts writing to it
- For `steam_applaunch` method, Steam may buffer log output
- Log encoding is typically UTF-8 but WINE can produce binary data mixed into text output
- Very large log files (100MB+) from verbose WINE debug output can cause memory pressure with `read_to_string()`

**Mitigation**: The existing chunk-based reading (tracking `last_len`) already handles incremental reads. Add guard for binary data and consider byte-level reading for very large files.

### 4.4 Anti-Cheat Detection Limitations

- Anti-cheat failures often manifest as silent exits (exit code 0) or generic errors
- EasyAntiCheat and BattlEye have different error signatures
- Some anti-cheat systems are game-specific in their Linux support
- AreWeAntiCheatYet data is community-maintained and may lag behind game updates

**Mitigation**: Anti-cheat detection should be a "hint" pattern (Medium confidence) rather than a definitive diagnosis.

### 4.5 Proton Version Compatibility

- Proton 5.x, 7.x, 8.x, 9.x have different behavior and error signatures
- Proton GE (GloriousEggroll) custom builds add patches that change error behavior
- The `version` file in `$STEAM_COMPAT_DATA_PATH/` can identify the Proton version
- Error patterns should be tested across multiple Proton versions

**Mitigation**: Include Proton version in diagnostic reports. Avoid version-specific pattern matching where possible.

---

## 5. Code Examples

### 5.1 Complete Exit Code Analyzer (Zero Dependencies)

```rust
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExitDiagnostic {
    pub category: ExitCategory,
    pub signal_name: Option<String>,
    pub signal_number: Option<i32>,
    pub exit_code: Option<i32>,
    pub core_dumped: bool,
    pub summary: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ExitCategory {
    CleanExit,
    ErrorExit,
    SignalCrash,
    SignalKilled,
    Unknown,
}

pub fn analyze_exit(status: ExitStatus) -> ExitDiagnostic {
    if status.success() {
        return ExitDiagnostic {
            category: ExitCategory::CleanExit,
            signal_name: None,
            signal_number: None,
            exit_code: Some(0),
            core_dumped: false,
            summary: "Process exited normally.".into(),
            suggestion: String::new(),
        };
    }

    if let Some(sig) = status.signal() {
        let core = status.core_dumped();
        let (name, summary, suggestion) = match sig {
            4  => ("SIGILL",  "Illegal instruction.",           "The executable may be compiled for a different CPU architecture."),
            6  => ("SIGABRT", "Process aborted (crash).",       "An assertion failed or memory was corrupted. Check if the game/trainer version matches the installed game version."),
            7  => ("SIGBUS",  "Bus error (memory alignment).",  "The process tried to access memory with incorrect alignment. This may indicate a Wine/Proton bug."),
            8  => ("SIGFPE",  "Floating point exception.",      "A math error occurred (division by zero). This is usually a bug in the software."),
            9  => ("SIGKILL", "Process was forcibly killed.",    "The system may have run out of memory (OOM), or the process was killed manually. Check system memory usage."),
            11 => ("SIGSEGV", "Segmentation fault.",            "Invalid memory access. Try a different Proton version or verify game files."),
            13 => ("SIGPIPE", "Broken pipe.",                   "The process lost connection to a pipe or socket. This is usually transient."),
            15 => ("SIGTERM", "Termination requested.",         "The process was asked to stop. This may be normal shutdown behavior."),
            _  => ("Unknown", "Terminated by signal.",          "An unexpected signal was received."),
        };
        return ExitDiagnostic {
            category: if sig == 9 || sig == 15 { ExitCategory::SignalKilled } else { ExitCategory::SignalCrash },
            signal_name: Some(name.into()),
            signal_number: Some(sig),
            exit_code: None,
            core_dumped: core,
            summary: format!("{name} (signal {sig}): {summary}{}",
                if core { " Core dump generated." } else { "" }),
            suggestion: suggestion.into(),
        };
    }

    if let Some(code) = status.code() {
        return ExitDiagnostic {
            category: ExitCategory::ErrorExit,
            signal_name: None,
            signal_number: None,
            exit_code: Some(code),
            core_dumped: false,
            summary: format!("Process exited with error code {code}."),
            suggestion: match code {
                1   => "General error. Check the log output for details.".into(),
                2   => "Misuse of shell command or missing argument.".into(),
                126 => "Command found but not executable. Check file permissions.".into(),
                127 => "Command not found. The executable path may be incorrect.".into(),
                _   => format!("Non-zero exit code {code}. Check log output for details."),
            },
        };
    }

    ExitDiagnostic {
        category: ExitCategory::Unknown,
        signal_name: None,
        signal_number: None,
        exit_code: None,
        core_dumped: false,
        summary: "Process exit status could not be determined.".into(),
        suggestion: "The process may have been detached or the status was lost.".into(),
    }
}
```

### 5.2 Multi-Pattern Log Scanner with RegexSet

```rust
use std::sync::LazyLock;
use regex::RegexSet;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PatternMatch {
    pub pattern_id: usize,
    pub category: &'static str,
    pub description: &'static str,
    pub suggestion: &'static str,
    pub severity: Severity,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum Severity { Error, Warning, Info }

struct PatternDef {
    regex: &'static str,
    category: &'static str,
    description: &'static str,
    suggestion: &'static str,
    severity: Severity,
}

const PATTERNS: &[PatternDef] = &[
    // DLL loading failures
    PatternDef {
        regex: r"(?i)could not load .+\.(dll|so)",
        category: "dll_load_failure",
        description: "A required library failed to load.",
        suggestion: "The Wine prefix may be corrupted. Try deleting the prefix and re-running, or install the missing runtime with protontricks.",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)Bad EXE format",
        category: "exe_format",
        description: "The executable format is incompatible.",
        suggestion: "The executable may be for a different architecture (32-bit vs 64-bit). Check the Proton version and ensure it supports the required architecture.",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)err:module:import_dll",
        category: "missing_dll",
        description: "A DLL dependency could not be imported.",
        suggestion: "Install the missing Visual C++ runtime or .NET Framework using protontricks.",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)status c0000135",
        category: "dll_not_found",
        description: "DLL not found (NTSTATUS 0xC0000135).",
        suggestion: "A required DLL is missing from the Wine prefix. Use protontricks to install the appropriate runtime (vcrun, dotnet).",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)err:seh:.*unhandled exception",
        category: "unhandled_exception",
        description: "An unhandled exception occurred in the process.",
        suggestion: "The game or trainer crashed due to an unhandled error. Try a different Proton version.",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)vkCreateInstance failed|vulkan.*not available|MESA.*error",
        category: "vulkan_failure",
        description: "Vulkan graphics initialization failed.",
        suggestion: "Ensure your GPU drivers are up to date and Vulkan is properly installed. Try PROTON_USE_WINED3D=1 as a fallback.",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)anti.?cheat|EasyAntiCheat|BattlEye",
        category: "anti_cheat",
        description: "Anti-cheat software detected.",
        suggestion: "This game may use anti-cheat that is not compatible with Linux/Proton. Check areweanticheatyet.com for compatibility status.",
        severity: Severity::Warning,
    },
    PatternDef {
        regex: r"(?i)mscoree\.dll.*not found|mono.*not found",
        category: "dotnet_missing",
        description: ".NET Framework or Mono runtime not found.",
        suggestion: "Install .NET Framework in the Wine prefix using protontricks (e.g., `protontricks <appid> dotnet48`).",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)prefix.*(?:not found|does not exist|invalid|corrupted)",
        category: "prefix_invalid",
        description: "The Wine prefix appears to be missing or corrupted.",
        suggestion: "Delete the compatibility data folder and let Proton recreate it on next launch.",
        severity: Severity::Error,
    },
    PatternDef {
        regex: r"(?i)permission denied|EACCES|Operation not permitted",
        category: "permissions",
        description: "A file permission error was detected.",
        suggestion: "Check file permissions on the game directory and Wine prefix. Ensure the files are on a Linux-native filesystem (ext4, btrfs), not NTFS.",
        severity: Severity::Error,
    },
];

static PATTERN_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new(PATTERNS.iter().map(|p| p.regex))
        .expect("failed to compile error pattern set")
});

pub fn scan_line(line: &str) -> Vec<PatternMatch> {
    PATTERN_SET
        .matches(line)
        .into_iter()
        .map(|idx| PatternMatch {
            pattern_id: idx,
            category: PATTERNS[idx].category,
            description: PATTERNS[idx].description,
            suggestion: PATTERNS[idx].suggestion,
            severity: PATTERNS[idx].severity.clone(),
        })
        .collect()
}
```

### 5.3 Crash Report File Discovery

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
pub struct CrashReport {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: Option<String>, // ISO 8601
}

pub fn discover_crash_reports(compat_data_path: &str) -> Vec<CrashReport> {
    let search_dirs = [
        Path::new(compat_data_path).join("crashreports"),
        Path::new(compat_data_path).join("pfx").join("drive_c").join("crashreports"),
    ];

    let mut reports = Vec::new();
    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "dmp" || ext == "log" || ext == "txt") {
                    if let Ok(meta) = entry.metadata() {
                        reports.push(CrashReport {
                            path: path.clone(),
                            size_bytes: meta.len(),
                            modified: meta.modified().ok().map(|t| {
                                chrono::DateTime::<chrono::Utc>::from(t)
                                    .format("%Y-%m-%dT%H:%M:%SZ")
                                    .to_string()
                            }),
                        });
                    }
                }
            }
        }
    }
    reports.sort_by(|a, b| b.modified.cmp(&a.modified)); // newest first
    reports
}
```

---

## 6. Dependency Recommendation Summary

| Crate                    | Version | Purpose                           | Phase | New Dep?                                | Recommendation                     |
| ------------------------ | ------- | --------------------------------- | ----- | --------------------------------------- | ---------------------------------- |
| `regex`                  | 1.12.x  | Log pattern matching (`RegexSet`) | 1     | Yes (1 crate + aho-corasick transitive) | **STRONGLY RECOMMENDED**           |
| `std::os::unix::process` | stdlib  | Exit code / signal analysis       | 1     | No                                      | **USE (zero cost)**                |
| `chrono`                 | 0.4.x   | Crash report timestamps           | 1     | No (already in Cargo.toml)              | **USE (already present)**          |
| `minidump`               | 0.26.x  | Crash dump parsing                | 2+    | Yes (~10 transitive crates)             | **DEFER**                          |
| `nom`                    | 7.x     | Parser combinator                 | --    | --                                      | **NOT RECOMMENDED**                |
| `pest`                   | 2.x     | PEG parser                        | --    | --                                      | **NOT RECOMMENDED**                |
| `nix`                    | 0.29.x  | Unix process API                  | --    | --                                      | **NOT NEEDED** (stdlib sufficient) |

---

## 7. Open Questions

1. **Log file encoding**: Should the scanner handle non-UTF-8 bytes in WINE debug output, or is `lossy` conversion acceptable?
2. **Pattern extensibility**: Should users be able to add custom error patterns, or is a curated built-in list sufficient for v1?
3. **Proton version detection**: Should diagnostics read `$STEAM_COMPAT_DATA_PATH/version` to include Proton version in reports?
4. **Crash report age filtering**: Should crash report discovery filter by modification time (e.g., only reports from last 5 minutes)?
5. **AreWeAntiCheatYet integration**: Is it worth bundling or fetching the games.json for anti-cheat detection, or is regex-based log scanning sufficient?
6. **PROTON_LOG interaction**: Should CrossHook set `PROTON_LOG=1` automatically for diagnostic launches, or let users opt in?

---

## Search Queries Executed

1. "Linux signal codes exit status Rust process handling SIGABRT SIGSEGV SIGKILL"
2. "Proton WINE common error patterns 'could not load' 'Bad EXE format' ntdll.dll error database"
3. "Rust crate regex nom pest log parsing pattern matching comparison"
4. "Rust minidump crate breakpad crash dump parsing library"
5. "Lutris Bottles game launcher error diagnostics detection WINE Proton failure handling"
6. "Proton Steam crashreports directory crash dump format STEAM_COMPAT_DATA_PATH structure"
7. "WINE debug output format WINEDEBUG channels log format documentation"
8. "Rust nix crate process signal handling ExitStatus exit code signal number"
9. "Lutris source code wine log analysis error detection diagnostics"
10. "Rust aho-corasick crate multi-pattern string matching log scanning performance"
11. "Proton WINE .NET runtime missing vcredist prefix error detection"
12. "ProtonDB error reports common failure patterns database API"
13. "Proton log file PROTON_LOG location output format"
14. "Rust std process ExitStatusExt signal unix os"
15. "Bottles source code wine error handling diagnostics detection"
16. "Valve Proton source code proton script error handling"
17. "Proton environment variables PROTON_CRASH_LOG_DIR PROTON_LOG_DIR documentation"
18. "Rust regex RegexSet multiple pattern matching simultaneous"
19. "Linux detect Flatpak sandbox running inside container"
20. "Anti-cheat detection Linux Proton EasyAntiCheat BattlEye"
21. "AreWeAntiCheatYet API database anti-cheat game compatibility"
22. "Proton GE proton-ge-custom crash log format minidump winedbg crash handler"
23. "Heroic Games Launcher error handling game launch failure diagnostics"
24. "Rust once_cell lazy_static regex pattern set compiled static performance"
25. "Rust regex crate version downloads maintenance status"

---

## Sources

### Signal and Process Handling

- [Linux signal(7) man page](https://man7.org/linux/man-pages/man7/signal.7.html)
- [Rust ExitStatusExt documentation](https://doc.rust-lang.org/std/os/unix/process/trait.ExitStatusExt.html)
- [Rust ExitStatus documentation](https://doc.rust-lang.org/stable/std/process/struct.ExitStatus.html)
- [nix crate WaitStatus](https://docs.rs/nix/latest/nix/sys/wait/enum.WaitStatus.html)
- [SIGSEGV: Linux Segmentation Fault (Komodor)](https://komodor.com/learn/sigsegv-segmentation-faults-signal-11-exit-code-139/)

### WINE/Proton Documentation

- [Proton GitHub Repository](https://github.com/ValveSoftware/Proton)
- [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
- [Proton GE Custom](https://github.com/GloriousEggroll/proton-ge-custom)
- [WineHQ Debug Channels](https://wiki.winehq.org/Debug_Channels)
- [Wine debug.h source](https://github.com/wine-mirror/wine/blob/master/include/wine/debug.h)
- [Sentry: Not So Mini-Dumps (Wine crash dump analysis)](https://blog.sentry.io/not-so-mini-dumps-how-we-found-missing-crashes-on-steamos/)

### Rust Crates

- [regex crate](https://crates.io/crates/regex) / [docs](https://docs.rs/regex/latest/regex/)
- [RegexSet documentation](https://docs.rs/regex/latest/regex/struct.RegexSet.html)
- [aho-corasick crate](https://crates.io/crates/aho-corasick) / [GitHub](https://github.com/BurntSushi/aho-corasick)
- [minidump crate](https://crates.io/crates/minidump) / [GitHub](https://github.com/rust-minidump/rust-minidump)
- [nom parser combinator](https://github.com/rust-bakery/nom)
- [pest parser](https://pest.rs/)
- [lazy-regex crate](https://crates.io/crates/lazy-regex)

### Competing Tools

- [Lutris GitHub](https://github.com/lutris/lutris)
- [Bottles GitHub](https://github.com/bottlesdevs/Bottles)
- [Bottles Logs & Debugger docs](https://docs.usebottles.com/utilities/logs-and-debugger)
- [Heroic Games Launcher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)

### Anti-Cheat and Compatibility

- [AreWeAntiCheatYet](https://areweanticheatyet.com/) / [GitHub](https://github.com/AreWeAntiCheatYet/AreWeAntiCheatYet)
- [ProtonDB Troubleshooting FAQ](https://www.protondb.com/help/troubleshooting-faq)
- [ProtonTricks](https://protontricks.com/)

### Flatpak

- [Flatpak Sandbox Permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)
