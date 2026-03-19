# PR #8 Review: feat(build): complete dotnet migration plan

**Branch:** `feat/dotnet-migrate` -> `main`
**Reviewed:** 2026-03-19
**Scope:** 72 files, +4969 / -812 lines
**Closes:** #2, #3, #4, #5, #6, #7

## Review Agents Deployed

| Agent                  | Focus                                      | Duration |
| ---------------------- | ------------------------------------------ | -------- |
| Code Reviewer          | Bugs, logic errors, CLAUDE.md compliance   | ~3.4 min |
| Silent Failure Hunter  | Swallowed exceptions, missing error checks | ~3.5 min |
| Test Coverage Analyzer | Test quality, gaps, edge cases             | ~1.7 min |
| Type Design Analyzer   | Encapsulation, invariants, API design      | ~3.3 min |
| Comment Analyzer       | Accuracy, staleness, maintainability       | ~2.9 min |

---

## Critical Issues (10 found)

Issues that represent bugs, resource leaks, or non-functional features that should be fixed before merge.

### CR-1: Handle leak — `hThread` from `CreateProcess` never closed

**File:** `src/ChooChooEngine.App/Core/ProcessManager.cs:382-395`
**Source:** Code Reviewer, Silent Failure Hunter
**Status:** Fixed

`PROCESS_INFORMATION` returns both `hProcess` and `hThread`. The code saves `processInfo.hProcess` but never closes `processInfo.hThread`. Every process launch leaks a kernel thread handle.

**Fix:** Added `Kernel32Interop.CloseHandle(processInfo.hThread);` immediately after successful `CreateProcess`, before storing `hProcess`.

---

### CR-2: Buffer overread in `InjectDllStandard` — `WriteProcessMemory` size exceeds buffer

**File:** `src/ChooChooEngine.App/Injection/InjectionManager.cs:248-261`
**Source:** Code Reviewer, Comment Analyzer
**Status:** Fixed

`allocSize` includes space for a null terminator (`dllPathBytes.Length + 1`), but the same `allocSize` is passed as `nSize` to `WriteProcessMemory` while the source buffer `dllPathBytes` only contains `dllPathBytes.Length` bytes. This reads 1 byte past the buffer boundary.

**Fix:** Changed `nSize` to `(uint)dllPathBytes.Length`. The zero-initialized `VirtualAllocEx` memory already provides the null terminator.

---

### CR-3: `_resumePanel` created but never added to form Controls

**File:** `src/ChooChooEngine.App/Forms/MainForm.cs:273`
**Source:** Code Reviewer
**Status:** Fixed

`_resumePanel` is instantiated in `InitializeManagers()` and `Show()`/`Hide()` are called from `OnDeactivate`/`OnActivated`, but it is never added to any `Controls` collection. The pause/resume overlay feature is completely non-functional.

**Fix:** Added `Controls.Add(_resumePanel)` with `Dock = Fill` and `Visible = false` after creation. Added `BringToFront()` in `OnDeactivate` before `Show()`.

---

### CR-4: Auto-launch timer is GC-eligible before firing

**File:** `src/ChooChooEngine.App/Forms/MainForm.cs:2547-2558`
**Source:** Code Reviewer, Silent Failure Hunter
**Status:** Fixed

A `System.Timers.Timer` is created as a local variable. After the method returns, the timer has no rooted reference and may be garbage collected before firing. The timer is also never disposed.

**Fix:** Promoted to class field `_autoLaunchTimer`, set `AutoReset = false` (fires once), and added disposal in `OnFormClosing`.

---

### CR-5: Missing `SetLastError = true` on all P/Invoke declarations in `Kernel32Interop`

**File:** `src/ChooChooEngine.App/Interop/Kernel32Interop.cs:9-28`
**Source:** Silent Failure Hunter
**Status:** Fixed

`OpenProcess`, `CreateRemoteThread`, `WriteProcessMemory`, `VirtualAllocEx`, and `VirtualFreeEx` all lack `SetLastError = true`. When any of these fail, `Marshal.GetLastWin32Error()` returns stale data. The entire injection pipeline — the app's primary purpose — fails with zero diagnostic info.

**Fix:** Added `SetLastError = true` to all five `Kernel32Interop` declarations and introduced `Win32ErrorHelper` so injection/open-process failures now include the Win32 error code and message. `InjectionManager` now reports `VirtualAllocEx`, `WriteProcessMemory`, and `CreateRemoteThread` failures explicitly, and `ProcessManager.OpenProcessHandle()` logs the `OpenProcess` failure details.

---

### CR-6: Missing `SetLastError = true` on `OpenThread` in `ProcessManager`

**File:** `src/ChooChooEngine.App/Core/ProcessManager.cs:16-17`
**Source:** Silent Failure Hunter
**Status:** Fixed

When `OpenThread` returns `IntPtr.Zero`, the thread is silently skipped. `SuspendProcess()` returns `true` even when threads fail to open, so the user sees "process suspended" but it keeps running.

**Fix:** Added `SetLastError = true` to `OpenThread` and routed suspend/resume through `TryExecuteThreadOperation(...)`, which logs `OpenThread` failures with the Win32 error and reports the overall operation as failed.

---

### CR-7: Missing `SetLastError = true` on `ReadProcessMemory`/`WriteProcessMemory`/`VirtualQueryEx` in `MemoryManager`

**File:** `src/ChooChooEngine.App/Memory/MemoryManager.cs:15-27`
**Source:** Silent Failure Hunter
**Status:** Fixed

All three P/Invoke declarations lack `SetLastError = true`. Memory operations fail with generic "Failed to read/write memory" messages.

**Fix:** Added `SetLastError = true` to all three declarations. `ReadMemory()` and `WriteMemory()` now surface the underlying Win32 error on API failure and separately report short transfers; `QueryMemoryRegions()` now reports unexpected `VirtualQueryEx` failures instead of collapsing them into a generic message.

---

### CR-8: `SuspendThread`/`ResumeThread` return values silently discarded

**File:** `src/ChooChooEngine.App/Core/ProcessManager.cs:210,234`
**Source:** Silent Failure Hunter
**Status:** Fixed

Both return `(DWORD)-1` on failure but the values are never checked. The methods return `true` as long as no managed exception is thrown, regardless of whether threads were actually suspended/resumed.

**Fix:** Added `SetLastError = true` to both declarations and made `SuspendProcess()` / `ResumeProcess()` fail when `SuspendThread` or `ResumeThread` returns `uint.MaxValue`, with the Win32 error logged per thread.

---

**Validation for CR-5 through CR-8**

- `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Debug`
- `DOTNET_ROLL_FORWARD=Major DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~InteropLibraryImportTests|FullyQualifiedName~ProcessManagerThreadOperationTests|FullyQualifiedName~Win32ErrorHelperTests"`

---

### CR-9: `WaitForSingleObject` return value discarded during injection

**File:** `src/ChooChooEngine.App/Injection/InjectionManager.cs:272`
**Source:** Silent Failure Hunter
**Status:** Fixed

Can return `WAIT_TIMEOUT`, `WAIT_FAILED`, or `WAIT_ABANDONED` — all silently ignored. On timeout (common with anti-cheat), the code checks the exit code of a still-running thread, yielding `STILL_ACTIVE` (259) which is misinterpreted as success.

**Fix:** Added explicit wait-result handling for `WAIT_OBJECT_0`, `WAIT_TIMEOUT`, `WAIT_ABANDONED`, and `WAIT_FAILED`, plus `GetExitCodeThread` validation. The injection path now fails with a concrete diagnostic when the remote `LoadLibraryA` thread does not complete successfully instead of treating timeout/failure as success.

---

### CR-10: PE header parsing reads wrong field — comment says "COFF header" but reads Optional Header magic

**File:** `src/ChooChooEngine.App/Injection/InjectionManager.cs:222-224`
**Source:** Comment Analyzer
**Status:** Fixed

After `fs.Position += 20` (skipping the entire 20-byte COFF header), the code reads 2 bytes. The comment says it reads COFF `Characteristics`, but it actually reads the Optional Header `Magic` field. Checking `IMAGE_FILE_32BIT_MACHINE` (0x0100) against the PE32 magic (0x010B) happens to produce roughly correct results by coincidence (`0x010B & 0x0100 == 0x0100`), but this is fragile.

**Fix:** Reworked the parser to read the Optional Header magic intentionally through `TryReadIsDll64Bit(Stream)`. It now returns `false` for PE32 (`0x10B`), `true` for PE32+ (`0x20B`), and rejects unknown/invalid headers instead of relying on the accidental `0x0100` bit overlap.

---

**Validation for CR-9 through CR-10**

- `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Debug`
- `DOTNET_ROLL_FORWARD=Major DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~InjectionManagerTests"`

---

## Important Issues (14 found)

Issues that represent missing functionality, dead code, or design problems that should be addressed.

### IM-1: Auto-load last profile feature is not implemented

**File:** `src/ChooChooEngine.App/Forms/MainForm.cs:2588-2608`
**Source:** Code Reviewer
**Status:** Open

`LoadAppSettings()` sets `_autoLoadLastProfile` and `_lastUsedProfile`, but no code ever checks `_autoLoadLastProfile` to actually load the profile on startup. The UI checkbox exists but the feature does nothing.

---

### IM-2: `PopulateControls()` defined but never called — dead code

**File:** `src/ChooChooEngine.App/Forms/MainForm.cs:1415-1432`
**Source:** Code Reviewer
**Status:** Open

This method adds the status strip, calls `RefreshProcessList()`, and `ShowCurrentEnvironmentModules()`. Since it's never called: the status strip is orphaned, the process dropdown starts empty, and environment modules aren't shown.

---

### IM-3: `MainForm_SizeChanged` and `MainForm_ResizeEnd` never wired to events

**File:** `src/ChooChooEngine.App/Forms/MainForm.cs:318,335`
**Source:** Code Reviewer
**Status:** Open

These resize handlers are defined but never subscribed in `RegisterEventHandlers()`. The resize debouncing logic never executes.

---

### IM-4: Constructor resize timer conflicts with `MainForm_SizeChanged` timer

**File:** `src/ChooChooEngine.App/Forms/MainForm.cs:260-265,318-333`
**Source:** Code Reviewer
**Status:** Open

Two competing timer strategies: the constructor creates a timer at 100ms (never started), while `MainForm_SizeChanged` (never wired) creates a different timer at 200ms. Needs unification.

---

### IM-5: `bool.Parse` throws `FormatException` on corrupted INI values

**File:** `src/ChooChooEngine.App/Services/AppSettingsService.cs:46`, `ProfileService.cs:100,105`
**Source:** Code Reviewer, Silent Failure Hunter, Test Coverage Analyzer
**Status:** Open

User-editable INI files with values like "yes", "1", or empty strings crash profile/settings loading. Use `bool.TryParse` instead.

---

### IM-6: `InjectDllManualMapping` silently falls back to standard injection

**File:** `src/ChooChooEngine.App/Injection/InjectionManager.cs:297-302`
**Source:** Silent Failure Hunter
**Status:** Open

Users select Manual Mapping to avoid anti-cheat detection (no `LoadLibrary` call). The silent fallback to standard injection defeats the purpose and could get users banned.

---

### IM-7: `LaunchWithCreateThreadInjection`/`LaunchWithRemoteThreadInjection` are undisclosed stubs

**File:** `src/ChooChooEngine.App/Core/ProcessManager.cs:425-437`
**Source:** Silent Failure Hunter, Comment Analyzer
**Status:** Open

Both silently delegate to `LaunchWithCreateProcess`. The user's explicit launch method selection is overridden without notification.

---

### IM-8: `Process.Start()` null return not handled

**File:** `src/ChooChooEngine.App/Core/ProcessManager.cs:448-466`
**Source:** Silent Failure Hunter
**Status:** Open

`Process.Start()` can return null when the process is reused. The result is assigned directly to `_process` — subsequent calls throw `NullReferenceException`.

---

### IM-9: `MiniDumpWriteDump` return value discarded

**File:** `src/ChooChooEngine.App/Core/ProcessManager.cs:258-259`
**Source:** Silent Failure Hunter
**Status:** Open

Returns `true` unconditionally even if the dump failed. User gets a 0-byte file with no error.

---

### IM-10: `Debug.WriteLine` is the sole error logging — stripped in Release builds

**File:** `ProcessManager.cs` (9 locations), `InjectionManager.cs` (2 locations)
**Source:** Silent Failure Hunter
**Status:** Open

All error logging in ProcessManager uses `Debug.WriteLine`, which is compiled out of Release builds. Every process/injection error becomes invisible in production.

---

### IM-11: No global unhandled exception handler

**File:** `src/ChooChooEngine.App/Program.cs:17-43`
**Source:** Silent Failure Hunter
**Status:** Open

No `Application.SetUnhandledExceptionMode` or `AppDomain.UnhandledException` handler. On Steam Deck/Proton, crashes produce no log.

---

### IM-12: `CurrentProcess` property leaks internal `Process` object

**File:** `src/ChooChooEngine.App/Core/ProcessManager.cs:106`
**Source:** Type Design Analyzer
**Status:** Open

Callers can `Kill()` the process directly, bypassing the manager's cleanup logic and leaving `_processHandle` dangling.

---

### IM-13: `InjectionManager` constructor has no null check on `processManager`

**File:** `src/ChooChooEngine.App/Injection/InjectionManager.cs` constructor
**Source:** Type Design Analyzer
**Status:** Open

Passing null won't fail until first use of `_processManager`, producing a confusing `NullReferenceException` far from the bug's origin.

---

### IM-14: Path traversal risk in `ProfileService.profileName`

**File:** `src/ChooChooEngine.App/Services/ProfileService.cs`
**Source:** Type Design Analyzer
**Status:** Open

Passing `"../../etc/passwd"` as a profile name constructs a path outside the intended directory. No sanitization of path separators or invalid filename characters.

---

## Test Coverage Gaps (8 found)

### TC-1: `ProfileService.DeleteProfile` — entirely untested (Criticality: 9/10)

**Status:** Open

---

### TC-2: `ProfileService.LoadProfile` missing file — `FileNotFoundException` untested (Criticality: 9/10)

**Status:** Open

---

### TC-3: `ProfileService.GetProfileNames` — entirely untested (Criticality: 8/10)

**Status:** Open

---

### TC-4: `AppSettingsService.LoadAppSettings` with corrupt boolean — crash untested (Criticality: 8/10)

**Status:** Open

---

### TC-5: `RecentFilesService.LoadRecentFiles` missing file — defaults untested (Criticality: 8/10)

**Status:** Open

---

### TC-6: No round-trip tests (Save then Load) for any service (Criticality: 7/10)

**Status:** Open

---

### TC-7: Unknown CLI flags silently ignored — behavior undocumented by tests (Criticality: 5/10)

**Status:** Open

---

### TC-8: Constructor null-argument guards untested across all services (Criticality: 5/10)

**Status:** Open

All gaps are low-effort (5-15 lines each) using the existing `TestWorkspace` helper.

---

## Type Design Scorecard

| Type               | Encapsulation | Invariants | Usefulness | Enforcement |   Avg   |
| ------------------ | :-----------: | :--------: | :--------: | :---------: | :-----: |
| Kernel32Interop    |       5       |     4      |     4      |      3      | **4.0** |
| AppSettingsService |       4       |     3      |     4      |      3      | **3.5** |
| CommandLineParser  |       4       |     3      |     4      |      2      | **3.3** |
| RecentFilesService |       3       |     3      |     4      |      2      | **3.0** |
| ProfileService     |       4       |     2      |     3      |      2      | **2.8** |
| ProcessManager     |       3       |     2      |     4      |      2      | **2.8** |
| InjectionManager   |       2       |     2      |     4      |      2      | **2.5** |
| MemoryManager      |       3       |     2      |     3      |      2      | **2.5** |

**Key cross-cutting issues:**

- All data transfer objects (`AppSettingsData`, `ProfileData`, `RecentFilesData`, `CommandLineOptions`) are fully mutable with zero validation
- Win32 constants duplicated across 3 files — should be centralized in `Kernel32Interop`
- INI parsing logic duplicated across 3 services — consider extracting shared utility
- Broad `catch (Exception)` in 15+ locations swallows catastrophic exceptions

---

## Comment Quality Issues (3 critical, 10 improvements, 6 recommended removals)

### Critical Comment Issues

1. **InjectionManager.cs:222-224** — "COFF header" comment describes wrong field (reads Optional Header magic) — **Status:** Open
2. **InjectionManager.cs:249** — Buffer size calculation is misleading (allocSize vs actual buffer length) — **Status:** Open
3. **.cursorrules:44** — States `DllImport` but codebase now uses `LibraryImport` (stale after migration) — **Status:** Open

### Notable Improvement Opportunities

- `Kernel32Interop.cs` has zero documentation — needs file-level summary explaining centralization purpose — **Status:** Open
- `CommandLineParser.cs` has zero comments — `-p` and `-autolaunch` semantics undocumented — **Status:** Open
- "COMPLETELY REDESIGNED" transitional comments in MainForm should become descriptive — **Status:** Open
- "In a real implementation" phrasing in stub methods is misleading — this IS the shipping implementation — **Status:** Open
- `Ctrl+T` handler does nothing, comment says "TV mode removed" — should remove the handler entirely — **Status:** Open

---

## Positive Observations

### Architecture

- SDK-style project migration is structurally sound
- Service extraction (`ProfileService`, `RecentFilesService`, `AppSettingsService`, `CommandLineParser`) follows good separation of concerns
- `Kernel32Interop` centralization is the right pattern (just needs completion)
- Test infrastructure (`TestWorkspace` helper) is well-designed with GUID isolation

### Code Quality

- `sealed` classes throughout prevent unintended inheritance
- `ArgumentNullException.ThrowIfNull` guards on all service constructors
- Namespace conventions follow `ChooChooEngine.App.{Layer}` pattern per CLAUDE.md
- INI parsing handles `=` in values correctly via `Split(new char[] { '=' }, 2)`

### Tests

- All 12 tests pass with substantive assertions (none vacuously pass)
- Good edge case coverage: malformed lines, values containing `=`, file existence filtering
- Clean test isolation via disposable workspaces

### Build & CI

- `publish-dist.sh` script is well-structured with cleanup trap
- Dual `win-x64`/`win-x86` artifact publishing preserves bitness-sensitive injection behavior
- Release workflow correctly configured

---

## Recommended Action Plan

### Before Merge (Critical)

1. **Fix CR-1:** Close `hThread` after `CreateProcess` — 1 line — **Status:** Fixed
2. **Fix CR-2:** Pass `dllPathBytes.Length` to `WriteProcessMemory` — 1 line — **Status:** Fixed
3. **Fix CR-3:** Add `_resumePanel` to form Controls — 1 line — **Status:** Fixed
4. **Fix CR-4:** Root the auto-launch timer as a class field — 3 lines — **Status:** Fixed
5. **Fix CR-5/6/7:** Add `SetLastError = true` to all P/Invoke declarations — mechanical — **Status:** Open
6. **Fix CR-10:** Correct PE header offset or use Optional Header magic intentionally — 2 lines — **Status:** Open

### Should Fix (Important)

7. **Fix IM-5:** Replace `bool.Parse` with `bool.TryParse` in services — 4 call sites — **Status:** Open
8. **Fix IM-2:** Call `PopulateControls()` from constructor or inline its logic — **Status:** Open
9. **Fix IM-3/4:** Wire resize handlers and unify timer strategy — **Status:** Open
10. **Fix IM-6/7:** Log warnings when stub methods are used, or disable their UI options — **Status:** Open
11. **Fix IM-10:** Replace `Debug.WriteLine` with `Trace.WriteLine` or event-based logging — **Status:** Open
12. **Fix IM-14:** Sanitize `profileName` against path traversal — **Status:** Open

### Follow-up Issues

13. Add the 8 missing test cases (TC-1 through TC-8) — **Status:** Open
14. Make data transfer objects immutable (`{ get; init; }`) — **Status:** Open
15. Centralize Win32 constants in `Kernel32Interop` — **Status:** Open
16. Add global unhandled exception handler in `Program.cs` — **Status:** Open
17. Check return values of `SuspendThread`, `ResumeThread`, `MiniDumpWriteDump`, `WaitForSingleObject` — **Status:** Open
