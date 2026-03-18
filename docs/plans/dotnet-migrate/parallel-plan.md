# dotnet-migrate Implementation Plan

This plan is intentionally narrower than the original package. It treats the migration as a staged modernization with hard runtime gates, not as an all-at-once rewrite.

## Phase Structure

1. Decision freeze
2. Foundation and WINE/Proton smoke gate
3. Behavior-preserving cleanup and low-risk extraction
4. P/Invoke modernization
5. Optional cleanup after regression proof

## Execution Rules

- Nothing in Phase 2 starts until the published build passes the Phase 1 smoke gate.
- `MainForm.cs` is the bottleneck. Tasks that edit it should be sequenced, not treated as parallel-friendly.
- `ProcessManager.cs`, `InjectionManager.cs`, and `MemoryManager.cs` interop conversions can run in parallel once the cleanup work is stable.
- Optional cleanup stays out of the critical path.

## Phase 0: Decision Freeze

### Task 0.1: Freeze Release Architecture Policy

Depends on: none

The migration release supports dual artifacts:

- `win-x64`
- `win-x86`

This is the explicit release policy for the migration because the current codebase is `AnyCPU` and `InjectionManager.ValidateDll()` depends on the loader process bitness. Do not collapse this to `win-x64` unless 32-bit compatibility is intentionally being dropped in a separate decision.

### Task 0.2: Freeze CLI Compatibility Policy

Depends on: none

Resolve the current mismatch between code and docs:

- implemented today: `-p`, `-autolaunch`
- documented but missing: `-dllinject`

Acceptable outcomes:

1. Implement `-dllinject` in Phase 2.
2. Remove the claim from docs for the migration release.

## Phase 1: Foundation And Smoke Gate

### Task 1.1: Convert To SDK-Style Project

Depends on: 0.1

Files:

- `src/ChooChooEngine.App/ChooChooEngine.App.csproj`
- `src/ChooChooEngine.App/Properties/AssemblyInfo.cs`
- `src/ChooChooEngine.App/packages.config`
- optionally `src/ChooChooEngine.sln`

Scope:

- Rewrite the project file as SDK-style targeting `net9.0-windows`.
- Preserve `WinExe`, WinForms, and `AllowUnsafeBlocks`.
- Move only the agreed assembly metadata into the SDK project.
- Decide whether `ComVisible(false)` and assembly `Guid` stay in a slimmed `AssemblyInfo.cs` or are intentionally removed.
- Remove `packages.config` only after confirming SharpDX is still unused.
- Clean stale `bin/` and `obj/` artifacts after the project conversion.

Validation:

- `dotnet build src/ChooChooEngine.sln -c Release`

### Task 1.2: Update Contributor And Release Docs

Depends on: 1.1

Files:

- `CLAUDE.md`
- `README.md`
- any migration-specific contributor notes

Scope:

- Replace the old “`dotnet build` will NOT work” guidance.
- Document the chosen publish architecture policy.
- Resolve the `-dllinject` documentation mismatch according to Task 0.2.
- Do not bundle unrelated product-doc cleanup into this task.

### Task 1.3: WINE/Proton Smoke Gate

Depends on: 1.1

This is a required gate, not an optional note.

Smoke checklist:

- published build starts under WINE/Proton
- main WinForms UI renders
- profile list loads
- settings load/save path still resolves correctly
- process list refresh works
- single-instance `Mutex` behavior still works

Stretch validation:

- trainer launch
- DLL validation
- end-to-end injection against a known-safe test target

If this gate fails, stop and re-plan before Phase 2.

## Phase 2: Cleanup And Low-Risk Extraction

### Task 2.1: Fix Event Wiring And Form Lifetime Bugs

Depends on: 1.3

Files:

- `src/ChooChooEngine.App/Forms/MainForm.cs`

Scope:

- Remove duplicate manager event subscriptions.
- Remove duplicate `btnRefreshProcesses.Click` hookup.
- Remove `FormClosing += (s, e) => OnFormClosing(e)`.
- Keep the `OnFormClosing` override as the single shutdown path.

Validation:

- event handlers fire once
- closing the form does not recurse

### Task 2.2: Fix Resource Ownership And Disposal

Depends on: 2.1

Files:

- `src/ChooChooEngine.App/Core/ProcessManager.cs`
- `src/ChooChooEngine.App/Injection/InjectionManager.cs`
- `src/ChooChooEngine.App/Forms/MainForm.cs`

Scope:

- Add explicit disposal for `ProcessManager` unmanaged handle ownership.
- Add explicit disposal for `InjectionManager` timer ownership.
- Update form shutdown to call the right cleanup entry points.

Validation:

- handle/timer cleanup is explicit in code review
- shutdown path remains single and deterministic

### Task 2.3: Extract Persistence Services

Depends on: 2.2

Files:

- create `src/ChooChooEngine.App/Services/ProfileService.cs`
- create `src/ChooChooEngine.App/Services/RecentFilesService.cs`
- create `src/ChooChooEngine.App/Services/AppSettingsService.cs`
- modify `src/ChooChooEngine.App/Forms/MainForm.cs`

Scope:

- Move file parsing/writing out of `MainForm`.
- Preserve existing formats exactly.
- Keep compatibility with persisted `LaunchMethod` names.
- Do not change profile schema.

Validation:

- existing profiles still load
- existing settings still load
- recent file filtering still ignores missing paths

### Task 2.4: Extract CommandLineParser

Depends on: 2.3 and 0.2

Files:

- create `src/ChooChooEngine.App/Services/CommandLineParser.cs`
- modify `src/ChooChooEngine.App/Forms/MainForm.cs`

Scope:

- Extract parsing for `-p` and `-autolaunch`.
- If Task 0.2 chose implementation, add `-dllinject` here.
- If Task 0.2 chose doc correction, keep behavior unchanged and document the omission.

Validation:

- argument parsing matches current behavior
- `-autolaunch` still consumes the remaining command text

### Task 2.5: Add Tests For Extracted Pure Logic

Depends on: 2.3 and 2.4

Files:

- new test project and test files as needed

Scope:

- test profile parsing/writing
- test settings parsing/writing
- test recent-files parsing
- test command-line parsing

This task covers extracted pure logic only. It does not attempt to unit-test WINE behavior or raw P/Invoke.

## Phase 3: P/Invoke Modernization

### Task 3.1: Convert ProcessManager To LibraryImport

Depends on: 2.2

Scope:

- convert `ProcessManager` declarations to `[LibraryImport]`
- handle `CreateProcess` marshalling intentionally
- keep behavior equivalent

### Task 3.2: Convert InjectionManager To LibraryImport

Depends on: 2.2

Scope:

- convert imported declarations used by `InjectionManager`
- document separately:
  - local validation import marshalling
  - remote `LoadLibraryA` injection path
- do not silently rewrite the remote injection path to Unicode

### Task 3.3: Convert MemoryManager To LibraryImport

Depends on: 2.2

Scope:

- convert `ReadProcessMemory`, `WriteProcessMemory`, and `VirtualQueryEx`

### Task 3.4: WINE/Proton Regression Gate

Depends on: 3.1, 3.2, 3.3

Required checks:

- launch/attach still work
- DLL validation still works
- DLL injection still works for supported ASCII-path scenarios
- suspend/resume still work

## Phase 4: Optional Cleanup

### Task 4.1: Optional Interop Deduplication

Depends on: 3.4

Possible scope:

- consolidate duplicate interop declarations into shared files

This is not required for the migration release.

### Task 4.2: Optional Nullable And Style Cleanup

Depends on: 3.4

Possible scope:

- targeted nullable annotations
- file-scoped namespaces
- minor doc cleanup

Keep this separate from the migration critical path.

## Parallelization Notes

- True parallel work:
  - Task 3.1, Task 3.2, Task 3.3
  - Task 1.2 can proceed while Task 1.3 is being prepared
- Sequential work because of shared write scope:
  - Task 2.1 through Task 2.4 all touch `MainForm.cs`
- Optional work:
  - Phase 4 should not delay the release candidate
