# PR #18 Review: feat: add Steam / Proton trainer workflow

**Branch:** `feat/steam-detect` -> `main`
**Reviewed:** 2026-03-21
**Scope:** 31 files, +3155 / -401 lines
**PR Link:** #18

## Review Agents Deployed

| Agent                  | Focus                                      | Duration |
| ---------------------- | ------------------------------------------ | -------- |
| Code Reviewer          | Bugs, logic errors, CLAUDE.md compliance   | ~2.4 min |
| Silent Failure Hunter  | Swallowed exceptions, missing error checks | ~3.2 min |
| Test Coverage Analyzer | Test quality, gaps, edge cases             | ~2.2 min |
| Type Design Analyzer   | Encapsulation, invariants, API design      | ~3.4 min |
| Comment Analyzer       | Accuracy, staleness, maintainability       | ~3.4 min |

---

## Critical Issues (4 found)

Issues that represent bugs, crashes, or silent data corruption that should be fixed before merge.

### CR-1: Shell script exit code capture after `if` is unreliable

**File:** `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh:170-177`
**Also:** `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh:281-288`
**Source:** Code Reviewer, Comment Analyzer, Silent Failure Hunter
**Status:** Open

In both scripts, `$?` is captured after the `fi` of an `if` statement that tests the Proton run command. Under `set -e`, the `if` construct traps the non-zero exit code, so `$?` after `fi` reflects the `if` block's own exit status rather than the Proton command's exit code. The log message `"Trainer proton run exited with code $exit_code"` reports the wrong value.

```bash
# Current (broken)
if "$proton" run "$trainer_path"; then
    log "Trainer proton run exited successfully."
    exit 0
fi
exit_code=$?  # Always captures the if-block status, not Proton's

# Suggested fix
"$proton" run "$trainer_path"
exit_code=$?
if [ "$exit_code" -eq 0 ]; then
    log "Trainer proton run exited successfully."
    exit 0
fi
log "Trainer proton run exited with code $exit_code"
exit "$exit_code"
```

---

### CR-2: `Process.Start()` null dereference in path conversion methods

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:298-301`
**Also:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:335-338`
**Source:** Code Reviewer, Silent Failure Hunter
**Status:** Open

`Process.Start(ProcessStartInfo)` can return `null`. The return value is used immediately on the next line (`process.StandardOutput.ReadToEnd()`) without a null check, causing an unhandled `NullReferenceException`. The project already uses the `TryRequireStartedProcess` pattern elsewhere in `ProcessManager`.

```csharp
// Current (crash-prone)
using Process process = Process.Start(startInfo);
string output = process.StandardOutput.ReadToEnd().Trim();

// Suggested fix
using Process process = Process.Start(startInfo)
    ?? throw new InvalidOperationException(
        $"Failed to start winepath.exe for path conversion of '{trimmedPath}'.");
```

---

### CR-3: Empty bare `catch` swallows all exceptions during log tail read

**File:** `src/CrossHookEngine.App/Forms/MainForm.cs:2698-2701`
**Source:** Silent Failure Hunter
**Status:** Open

A bare `catch` block with no exception type catches everything -- including `OutOfMemoryException` and `AccessViolationException` -- while attempting to read the Steam helper log file for a failure tail. The user gets a terse exit code message with no log context and no indication that the log read itself failed.

```csharp
// Current
catch
{
    // ignore log read errors
}

// Suggested fix
catch (Exception ex)
{
    AppDiagnostics.LogError($"Failed to read steam helper log tail: {ex.Message}");
    failTail = "(log file could not be read)";
}
```

---

### CR-4: Fire-and-forget `Task.Run` with no exception handling in `StreamSteamHelperLogAsync`

**File:** `src/CrossHookEngine.App/Forms/MainForm.cs:2641` (call site)
**Also:** `src/CrossHookEngine.App/Forms/MainForm.cs:2728-2766` (method body)
**Source:** Code Reviewer, Silent Failure Hunter
**Status:** Open

`StreamSteamHelperLogAsync` is a `void`-returning method launched via `Task.Run` with the result discarded (`_ = Task.Run(...)`). The method body has no try-catch. If `FileStream`, `StreamReader.ReadLine`, or `LogToConsole` (cross-thread UI) throws, the exception vanishes silently and log streaming stops without any indication.

**Fix:** Wrap the entire body of `StreamSteamHelperLogAsync` in a try-catch that logs to `AppDiagnostics.LogError`.

---

## Important Issues (8 found)

Issues that could cause incorrect behavior, confusing error messages, or maintainability problems.

### IMP-1: `ResolveDosDevicesPath` silently returns unresolved path on any exception

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:402-404`
**Source:** Silent Failure Hunter
**Status:** Open

`catch (Exception)` swallows every exception from `ConvertToUnixPath` and returns the raw `dosdevices` path. This unresolved path (containing `/dosdevices/c:/...`) flows through to shell scripts and Proton, producing cryptic runtime errors with no diagnostic trace.

**Fix:** At minimum, log via `AppDiagnostics.LogError` with the original path and exception. Consider narrowing the catch to `InvalidOperationException`.

---

### IMP-2: `EscapeDesktopExecArgument` incomplete per freedesktop spec

**File:** `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs:340-347`
**Source:** Code Reviewer, Comment Analyzer
**Status:** Open

The Desktop Entry Specification requires escaping `$`, backtick, and `%` in the `Exec` field. The current implementation only handles `\`, space, and `"`. Paths containing `$` or backticks would produce silently broken `.desktop` entries.

```csharp
// Add to the Replace chain:
.Replace("$", "\\$")
.Replace("`", "\\`")
.Replace("%", "%%")
```

---

### IMP-3: Sequential `NormalizeSteamHostPath` calls leave UI state inconsistent on partial failure

**File:** `src/CrossHookEngine.App/Forms/MainForm.cs:1921-1928`
**Source:** Silent Failure Hunter
**Status:** Open

During profile load, three `NormalizeSteamHostPath` calls run sequentially. If the first succeeds but the second throws (via the `winepath.exe` subprocess), the remaining fields retain stale values from the previous profile. The user sees a "Profile loaded" success message but the form state is a mix of old and new values.

**Fix:** Either normalize all paths before applying to UI (atomic update), or catch per-field and log which path failed.

---

### IMP-4: `BuildSteamLaunchRequest` path normalization throws without field-specific context

**File:** `src/CrossHookEngine.App/Forms/MainForm.cs:2474-2478`
**Source:** Silent Failure Hunter
**Status:** Open

If any `NormalizeSteamHostPath` call throws during request building, the outer catch shows `Error launching: {ex.Message}` without identifying which Steam field has the bad path. The user cannot diagnose the problem.

**Fix:** Wrap each normalization in individual try-catch with field-specific error messages, or pre-validate paths before building the request.

---

### IMP-5: `GetSteamClientInstallPath` silently returns empty string with no diagnostic trace

**File:** `src/CrossHookEngine.App/Forms/MainForm.cs:2502-2518`
**Source:** Silent Failure Hunter
**Status:** Open

When environment auto-detection fails (no `STEAM_COMPAT_CLIENT_INSTALL_PATH`, no `HOME`, no `UserProfile`), returns empty string silently. This can cause the export service to write launcher files into the WINE prefix instead of the real host home.

**Fix:** Log a warning via `AppDiagnostics.LogInfo` when auto-detection falls through.

---

### IMP-6: Unused variable `prefix` in `ResolveDosDevicesPath`

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:377`
**Source:** Code Reviewer, Comment Analyzer
**Status:** Open

`string prefix = unixPath[..(markerIndex + marker.Length)];` is assigned but never read. Dead code.

**Fix:** Remove the line.

---

### IMP-7: `QuoteForCommand` and `QuoteForProcessStart` are identical methods

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:408-418`
**Source:** Code Reviewer, Type Design Analyzer
**Status:** Open

Both methods have identical implementations. Having two identically-bodied methods with different names is confusing.

**Fix:** Consolidate into a single method or have one delegate to the other. If intentionally separate for future divergence, add a comment.

---

### IMP-8: Generated `.sh` script not marked executable

**File:** `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs:265-275`
**Source:** Silent Failure Hunter
**Status:** Open

`File.WriteAllText` creates files with default permissions (0644). The generated script has a `#!/usr/bin/env bash` shebang but is not executable. Running it directly from a terminal fails with "permission denied." The `.desktop` entry uses `/bin/bash <script>` which may work, but direct execution does not.

**Fix:** Call `File.SetUnixFileMode` (available in .NET 7+) after writing the script, or document the limitation.

---

## Test Coverage Gaps (9 found)

Gaps in test coverage that could allow regressions.

### TC-1: `ConvertToUnixPath` / `ConvertToWindowsPath` fast-path logic untested (Criticality: 9)

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:268-346`
**Source:** Test Coverage Analyzer

Three code paths exist: Unix passthrough, Z-drive strip, and `winepath.exe` fallback. Only the Z-drive path is indirectly tested via `NormalizeSteamHostPath`. Missing: `ConvertToUnixPath("")`, `ConvertToUnixPath("/already/unix")`, `ConvertToUnixPath("Z:\\")`, `ConvertToWindowsPath("C:/mixed/slashes")`.

---

### TC-2: `SteamLaunchService.Validate` -- 5 of 7 branches untested (Criticality: 8)

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:66-106`
**Test:** `tests/CrossHookEngine.App.Tests/SteamLaunchServiceTests.cs`

Only `SteamAppId` and `TrainerHostPath` validation are tested. Missing: `GamePath` (with `LaunchTrainerOnly` interaction), `TrainerPath`, `SteamCompatDataPath`, `SteamProtonPath`, `SteamClientInstallPath`.

---

### TC-3: `ExportLaunchers` exception paths untested (Criticality: 8)

**File:** `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs:110-141`

`ExportLaunchers` throws `InvalidOperationException` on validation failure and empty `targetHomePath`. Neither path is tested.

---

### TC-4: `LooksLikeWindowsPath` edge cases untested (Criticality: 7)

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:354-361`

No direct tests. Missing: `"C:"` (length 2), `"1:\\path"` (digit drive letter), `"C:/forward"` (forward slash variant).

---

### TC-5: `QuoteForCommand` / `ToShellSingleQuotedLiteral` untested (Criticality: 7)

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:408-418`
**Also:** `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs:334-338`

These form the command-injection boundary. No tests for embedded quotes, single quotes, null, or empty string input.

---

### TC-6: `ResolveDisplayName` / `SanitizeLauncherSlug` untested (Criticality: 6)

**File:** `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs:160-206`

Display name fallback chain (preferred name -> trainer filename -> app ID) and slug sanitization (empty input, consecutive separators, special characters) have no direct tests.

---

### TC-7: `WaitForProcessReady` timeout path with `RequireMainWindow` untested (Criticality: 6)

**File:** `src/CrossHookEngine.App/Core/ProcessManager.cs:603-615`

The most common real-world failure (process alive but no main window before timeout) is not tested.

---

### TC-8: `SteamExternalLauncherExportService.Validate` -- 6 of 8 branches untested (Criticality: 5)

**File:** `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs:55-108`

Only `TargetHomePath` and icon extension validation are tested. Missing: `TrainerPath`, `SteamAppId`, `SteamCompatDataPath`, `SteamProtonPath`, `SteamClientInstallPath`, icon file existence.

---

### TC-9: `ResolveGameExecutableName` edge cases (Criticality: 5)

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:108-122`

Only Windows backslash paths tested. Missing: forward slash paths, bare filenames, empty/null input.

---

## Type Design Observations

### Type Design Ratings

| Type                                              | Encapsulation | Invariant Expression | Usefulness | Enforcement |
| ------------------------------------------------- | :-----------: | :------------------: | :--------: | :---------: |
| `SteamLaunchRequest`                              |       3       |          2           |     6      |      3      |
| `SteamLaunchValidationResult` / `ExecutionResult` |       8       |          5           |     7      |      6      |
| `SteamExternalLauncherExportRequest`              |       3       |          2           |     6      |      3      |
| `SteamExternalLauncherExportService`              |       7       |          7           |     8      |      7      |
| `SteamLaunchService`                              |       7       |          6           |     8      |      6      |
| `ProcessReadinessOptions` / `Result`              |       7       |          6           |     9      |      7      |
| `ProfileData`                                     |       2       |          2           |     5      |      1      |
| `ProfileService`                                  |       8       |          7           |     9      |      8      |

### Top Type Design Recommendations

1. **Replace `LaunchTrainerOnly`/`LaunchGameOnly` booleans with a `SteamLaunchMode` enum** (`Full`, `TrainerOnly`, `GameOnly`). Eliminates the impossible state where both are true, improves readability, simplifies validation. Small change, high impact.

2. **Change `ProfileData.LaunchMethod` from `string` to the existing `LaunchMethod` enum.** Closes a type safety gap. Update serialization to use `Enum.TryParse<LaunchMethod>`.

3. **Add defensive `Validate` calls at the top of `CreateHelperStartInfo` and `CreateTrainerStartInfo`**, matching the pattern already used in `SteamExternalLauncherExportService.ExportLaunchers`. Two-line change per method.

---

## Comment & Documentation Issues (5 found)

### DOC-1: Feature doc undercounts required export fields

**File:** `docs/features/steam-proton-trainer-launch.doc.md:73,78`
**Source:** Comment Analyzer

The limitation and troubleshooting sections omit `SteamClientInstallPath` and `TargetHomePath` as required fields for export. A user whose environment auto-detection fails would check the wrong fields.

---

### DOC-2: Missing comment explaining `_steamTrainerLaunchPending` state machine

**File:** `src/CrossHookEngine.App/Forms/MainForm.cs:113`
**Source:** Comment Analyzer

This boolean drives the two-phase Steam launch flow (first click = game, second click = trainer) but has no explanatory comment. A future maintainer could easily misunderstand the two-phase pattern.

---

### DOC-3: `wait_for_process` function defined but never called

**File:** `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh:214`
**Source:** Comment Analyzer

Dead code -- the function is defined with a timeout-based polling loop but never invoked. Similarly, `game_timeout_seconds` and `trainer_timeout_seconds` variables are parsed from CLI args but never used.

---

### DOC-4: Environment variable clear lists inconsistent between C# and shell scripts

**File:** `src/CrossHookEngine.App/Services/SteamLaunchService.cs:244-245` vs shell scripts
**Source:** Comment Analyzer

The C# `GetEnvironmentVariablesToClear()` list includes `WINE_HEAP_DELAY_FREE` and `WINEFSYNC_SPINCOUNT`, but neither shell script clears them. If the lists should match, two variables are missing. If they intentionally differ, there's no comment explaining why.

---

### DOC-5: Documentation calls shared fields "Steam fields"

**File:** `docs/features/steam-proton-trainer-launch.doc.md:29-34`
**Source:** Comment Analyzer

The workflow says "Fill in the Steam fields" and lists Game Path and Trainer Path alongside Steam-specific fields. In the UI, Game Path and Trainer Path are shared fields outside the Steam section. Should say "Fill in the Game Path and Trainer Path as usual, then fill in the Steam-specific fields."

---

## Medium-Priority Issues (6 found)

Issues worth tracking that may be acceptable given pre-production status.

### MED-1: Multiple catch blocks log only to ephemeral console, not `AppDiagnostics`

**Files:** `src/CrossHookEngine.App/Forms/MainForm.cs` at lines 3383, 3405, 1803, 1844, 1765, 2932, 3142
**Source:** Silent Failure Hunter

Seven catch blocks across `SaveAppSettings`, `LoadAppSettings`, `LoadProfiles`, `LoadRecentFiles`, `RefreshProcessList`, `InjectDll`, and `ShowCurrentEnvironmentModules` log only to `LogToConsole` (ephemeral) but not to `AppDiagnostics.LogError` (persistent). After the form closes, all evidence of these failures is gone.

---

### MED-2: Profile boolean parse failures silently default to `false`

**File:** `src/CrossHookEngine.App/Services/ProfileService.cs:101-123`
**Source:** Silent Failure Hunter

`bool.TryParse` failures silently retain the default `false` value with no logging. A profile with `UseSteamMode=yes` (instead of `True`) silently disables Steam mode. This is tested and documented as intentional, but a diagnostic log entry would help users who hand-edit profiles.

---

### MED-3: `ProcessReadinessResult` has a fragile 6-parameter constructor

**File:** `src/CrossHookEngine.App/Core/ProcessManager.cs:839-851`
**Source:** Type Design Analyzer

The `bool, int, bool, bool, string, bool` constructor signature is prone to argument ordering errors during refactoring. Consider named construction or a builder pattern.

---

### MED-4: Request types (`SteamLaunchRequest`, `SteamExternalLauncherExportRequest`, `ProfileData`) are anemic DTOs

**Source:** Type Design Analyzer

All three share the "open data bag" pattern: public get/set on all properties, no construction-time validation, total reliance on external validation. Pragmatically defensible for WinForms incremental form-building, but provides no safety net for new call sites that forget to validate.

---

### MED-5: `SteamExternalLauncherExportServiceTests.ExportLaunchers_WritesTrainerScriptAndDesktopEntryToUserHome` has 13+ assertions

**Source:** Test Coverage Analyzer

A single test validates display name, slug, script path, desktop entry path, file existence, script content (6 assertions), and desktop content (3 assertions). If an early assertion fails, remaining assertions never run, making debugging harder. Consider splitting.

---

### MED-6: Missing comment on `WINEPREFIX` unset-then-re-export in shell script

**File:** `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh:265,277`
**Source:** Comment Analyzer

`WINEPREFIX` is unset (line 265) then immediately re-exported (line 277). Without a comment, a reader could think the unset is a mistake. Add: "WINEPREFIX is re-exported below with the correct compatdata-based value."

---

## Strengths

The review agents identified several strong patterns in this PR:

- **Validation-first architecture**: Both services validate all required fields before proceeding, with clear field-specific error messages. `ExportLaunchers` re-validates defensively.
- **Shell scripts use `set -euo pipefail`** with clear `fail` messages and thorough argument validation.
- **Naming conventions followed**: Namespaces (`CrossHookEngine.App.Services`), private fields (`_camelCase`), and event args patterns all match CLAUDE.md conventions.
- **Excellent round-trip profile test**: `SaveProfile_ThenLoadProfile_RoundTripsPersistedValues` verifies all 12 fields including new Steam fields.
- **Security-aware testing**: Path traversal tests for profile names, environment variable clearing for WINE isolation.
- **Clean service/UI separation**: `SteamLaunchService` and `SteamExternalLauncherExportService` are static classes with pure-logic methods, making them testable without WinForms dependencies.
- **Proper resource disposal**: `_trainerProcessManager` is detached and disposed in `OnFormClosing`.
- **Thread-safe UI updates**: `LogToConsole` correctly marshals to the UI thread via `InvokeRequired`/`Invoke`.
- **TestWorkspace helper** provides isolated temp directories with automatic cleanup.

---

## Recommended Action

### Before Merge (Critical)

1. Fix shell script exit code capture pattern (CR-1)
2. Add null check on `Process.Start` returns (CR-2)
3. Replace bare `catch` with typed catch + logging (CR-3)
4. Add try-catch to `StreamSteamHelperLogAsync` (CR-4)

### Should Fix (Important)

5. Add `AppDiagnostics.LogError` to `ResolveDosDevicesPath` catch (IMP-1)
6. Complete `EscapeDesktopExecArgument` per freedesktop spec (IMP-2)
7. Atomic profile load or per-field error handling (IMP-3)
8. Remove dead `prefix` variable (IMP-6)
9. Consolidate duplicate quoting methods (IMP-7)

### Should Add Tests

10. `ConvertToUnixPath`/`ConvertToWindowsPath` fast paths (TC-1)
11. Remaining `Validate` branches in both services (TC-2, TC-8)
12. Shell quoting boundary methods (TC-5)
13. `SanitizeLauncherSlug` and `ResolveDisplayName` (TC-6)

### Consider

14. Replace boolean pair with `SteamLaunchMode` enum (Type Design)
15. Change `ProfileData.LaunchMethod` from string to enum (Type Design)
16. Remove dead `wait_for_process` function and unused timeout variables (DOC-3)
17. Synchronize env var clear lists between C# and shell scripts (DOC-4)
