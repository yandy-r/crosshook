# Task Structure Analysis: dotnet-migrate

## Executive Summary

The dotnet-migrate feature decomposes cleanly into 3 phases with 14 discrete tasks totaling modifications to 8 source files, deletion of 2 files plus 2 directories, and creation of 1 new csproj (complete rewrite). Phase 1 (Foundation) contains 3 fully independent tasks that can run in parallel. Phase 2 (Architecture) contains 6 tasks, of which 4 service extraction tasks can run in parallel after the event subscription bug fix. Phase 3 (Modernization) contains 5 tasks, of which 3 P/Invoke conversion tasks are fully independent. The critical path runs through Phase 1 Task 1 (csproj conversion) since every subsequent task depends on the project building under .NET 9.

## Recommended Phase Structure

### Phase 1: Foundation

**Purpose**: Get the project building on .NET 9 with `dotnet build` and configure self-contained publish. Zero behavioral changes.

**Suggested Tasks**:

#### Task 1.1: SDK-Style csproj Conversion

- **Files**: `src/ChooChooEngine.App/ChooChooEngine.App.csproj` (complete rewrite)
- **Scope**: Replace the 75-line classic MSBuild csproj with the ~30-line SDK-style csproj targeting `net9.0-windows`. Must include: `<UseWindowsForms>true</UseWindowsForms>`, `<AllowUnsafeBlocks>true</AllowUnsafeBlocks>`, assembly metadata properties (Title, Description, Copyright, Version) migrated from AssemblyInfo.cs, self-contained publish properties (`RuntimeIdentifier`, `SelfContained`, `PublishSingleFile`, `EnableCompressionInSingleFile`, `IncludeNativeLibrariesForSelfExtract`), and Debug/Release conditional property groups.
- **Validation**: `dotnet build src/ChooChooEngine.sln -c Release` succeeds with zero errors.
- **Gotcha**: The solution file (`ChooChooEngine.sln`) uses the classic project type GUID `{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}`. SDK-style projects typically use `{9A19103F-16F7-4668-BE54-9A1E7A4F7556}`, but the old GUID works fine with `dotnet build` -- no sln change is strictly required. Test both before deciding.

#### Task 1.2: Delete Obsolete Files

- **Files**: Delete `src/ChooChooEngine.App/Properties/AssemblyInfo.cs`, delete `src/ChooChooEngine.App/packages.config`
- **Scope**: AssemblyInfo.cs metadata is replaced by csproj `<PropertyGroup>` properties in Task 1.1. The packages.config references only SharpDX 4.2.0, which has zero source references (confirmed by grep: no XInput, SharpDX, or gamepad references in any .cs file). The `Utils/` folder listed in the old csproj as an empty folder include can also be removed if it exists on disk.
- **Validation**: `dotnet build` still succeeds after deletion. No compile errors.
- **Dependency**: Logically pairs with Task 1.1 but can be a separate commit.

#### Task 1.3: Clean Build Artifacts

- **Files**: Delete `src/ChooChooEngine.App/bin/` directory, delete `src/ChooChooEngine.App/obj/` directory
- **Scope**: The old bin/obj contain .NET Framework 4.8 build artifacts (including NuGet restore files referencing SharpDX). A clean rebuild is required to avoid stale file conflicts with the new SDK-style project.
- **Validation**: `dotnet build` and `dotnet publish` produce output in the new `net9.0-windows/win-x64/` path structure.
- **Gotcha**: The existing `bin/Debug/Settings/AppSettings.ini` is a runtime-generated settings file, not a build artifact. It should not be committed but verify `.gitignore` covers it. Current `.gitignore` already includes `bin/` and `obj/`.

**Parallelization**: All 3 tasks can run in parallel. Task 1.2 and 1.3 have no code dependencies on Task 1.1 (they are pure deletions), but Task 1.1 must be merged first for the build to succeed.

---

### Phase 2: Architecture

**Purpose**: Extract business logic from the 2,800-line MainForm.cs monolith into testable services, fix 4 known bugs, and add IDisposable to ProcessManager.

**Dependencies from Phase 1**: Project must build with `dotnet build` on .NET 9.

**Suggested Tasks**:

#### Task 2.1: Fix Double Event Subscription Bug

- **Files**: `src/ChooChooEngine.App/Forms/MainForm.cs`
- **Scope**: `InitializeManagers()` (lines 274-283) and `RegisterEventHandlers()` (lines 1370-1387) both subscribe to the same ProcessManager, InjectionManager, MemoryManager, and ResumePanel events. This causes every handler to fire twice. Fix: remove the duplicate subscriptions from `RegisterEventHandlers()` for manager/panel events, keeping only the button click and radio button subscriptions in `RegisterEventHandlers()`. Alternatively, consolidate all subscriptions into one method.
- **Validation**: Manual test: launch a process and verify console log shows each event message exactly once (not twice).
- **Priority**: Do this first in Phase 2 -- it is a bug that affects all other MainForm testing.

#### Task 2.2: Implement IDisposable on ProcessManager

- **Files**: `src/ChooChooEngine.App/Core/ProcessManager.cs`, `src/ChooChooEngine.App/Forms/MainForm.cs`
- **Scope**: ProcessManager holds `_processHandle` (IntPtr from `OpenProcess`) but never implements IDisposable. If the consumer does not call `DetachFromProcess()`, the handle leaks. Add `IDisposable` implementation with: (1) `Dispose(bool disposing)` pattern calling `CloseProcessHandle()`, (2) a finalizer as safety net, (3) `_disposed` flag to prevent double-close. Update MainForm.cs `OnFormClosing()` to call `_processManager?.Dispose()` instead of just `DetachFromProcess()`.
- **Validation**: Code review confirms the dispose pattern. Optional: run under a debugger and verify `CloseHandle` is called on form close.
- **Gotcha**: ResumePanel already implements a correct `Dispose(bool)` pattern (see `UI/ResumePanel.cs` lines 94-106) -- use it as the reference implementation.

#### Task 2.3: Extract ProfileService from MainForm

- **Files**: Create `src/ChooChooEngine.App/Services/ProfileService.cs`, modify `src/ChooChooEngine.App/Forms/MainForm.cs`
- **Scope**: Extract the following methods from MainForm into a new `ProfileService` class: `LoadProfiles()` (lines 1586-1622), `SaveProfile(string)` (lines 1624-1672), `LoadProfile(string)` (lines 1674-1751), the delete logic from `BtnDelete_Click` (lines 2045-2103). The service should take a `basePath` parameter (replacing `Application.StartupPath` hard-coding) and return data objects rather than directly manipulating UI controls. MainForm retains the UI wiring and calls ProfileService methods.
- **Validation**: Profile save/load/delete works identically to before.
- **Gotcha**: `LoadProfile()` currently calls `SetComboBoxValue()` and directly sets checkbox states -- the extracted service should return a `ProfileData` record/class, and MainForm applies it to UI. The nested `ProfileInputDialog` class (lines 104-209) should remain in MainForm (it is pure UI).

#### Task 2.4: Extract CommandLineParser from MainForm

- **Files**: Create `src/ChooChooEngine.App/Services/CommandLineParser.cs`, modify `src/ChooChooEngine.App/Forms/MainForm.cs`
- **Scope**: Extract `ProcessCommandLineArguments()` (lines 2602-2701) into a standalone class that parses `-p` and `-autolaunch` arguments and returns a `CommandLineOptions` record with `ProfileToLoad`, `AutoLaunchPath`, and `AutoLaunchRequested` properties. MainForm calls the parser in its constructor and acts on the returned options.
- **Validation**: Run with `-p "TestProfile"` and `-autolaunch /path/to/game.exe` arguments and verify correct behavior.
- **Gotcha**: The current `-autolaunch` parser consumes all remaining arguments after the flag (lines 2650-2654) -- this is intentional for paths with spaces. Preserve this behavior. The spec mentions `-dllinject` is documented in README but not implemented -- decide whether to add it here or defer.

#### Task 2.5: Extract RecentFilesService from MainForm

- **Files**: Create `src/ChooChooEngine.App/Services/RecentFilesService.cs`, modify `src/ChooChooEngine.App/Forms/MainForm.cs`
- **Scope**: Extract `LoadRecentFiles()` (lines 1483-1549) and `SaveRecentFiles()` (lines 1551-1584) into a service. The service manages three MRU lists (game paths, trainer paths, DLL paths) using the INI-style `settings.ini` format with `[RecentGamePaths]`, `[RecentTrainerPaths]`, `[RecentDllPaths]` section headers. Also extract `SaveAppSettings()` (lines 2704-2732) and `LoadAppSettings()` (lines 2734-2788) which handle `Settings/AppSettings.ini`.
- **Validation**: Recent files persist correctly across app restarts. Auto-load last profile setting persists.
- **Gotcha**: `LoadRecentFiles()` currently writes directly to UI controls (`cmbGamePath.Items.Add(line)`, etc.) on lines 1520-1537. The extracted service must return data, and MainForm populates the controls.

#### Task 2.6: Remove or Implement Dead Code Stubs

- **Files**: `src/ChooChooEngine.App/Core/ProcessManager.cs`
- **Scope**: Three stub methods in ProcessManager fall through to their default implementations: `LaunchWithCreateThreadInjection` (lines 416-421), `LaunchWithRemoteThreadInjection` (lines 423-428), and `InjectDllManualMapping` in InjectionManager (lines 297-302). The feature-spec recommends removing them unless there is a specific need. Remove the stubs, remove the corresponding `LaunchMethod` enum values (`CreateThreadInjection`, `RemoteThreadInjection`), and remove the corresponding radio buttons from MainForm.cs (`radCreateThreadInjection`, `radRemoteThreadInjection`). Also remove the `ManualMapping` enum value from `InjectionMethod`.
- **Files also touched**: `src/ChooChooEngine.App/Injection/InjectionManager.cs`, `src/ChooChooEngine.App/Forms/MainForm.cs`
- **Validation**: Build succeeds. Remaining 4 launch methods work correctly.
- **Gotcha**: If these stubs should be implemented rather than removed, that is a separate feature, not a migration task. The recommendation is to remove.

**Parallelization**: Tasks 2.3, 2.4, 2.5 can run fully in parallel (they extract independent code sections from MainForm). Task 2.1 should run first (fixes bug that would confuse testing). Task 2.2 and 2.6 can run in parallel with each other and with 2.3-2.5.

---

### Phase 3: Modernization

**Purpose**: Convert P/Invoke declarations from `[DllImport]` to `[LibraryImport]` source generators, consolidate duplicate declarations, enable nullable reference types, adopt C# 12 features, and update documentation.

**Dependencies from Phase 2**: Phase 2 should be substantially complete, especially Task 2.6 (dead code removal affects which P/Invoke declarations exist).

**Suggested Tasks**:

#### Task 3.1: Convert ProcessManager P/Invoke to LibraryImport

- **Files**: `src/ChooChooEngine.App/Core/ProcessManager.cs`
- **Scope**: Convert 11 `[DllImport]` declarations to `[LibraryImport]` with `partial` methods. Add `partial` keyword to the class declaration. Key conversions: (1) `CreateProcess` needs `StringMarshalling = StringMarshalling.Utf16` for string parameters, (2) `MiniDumpWriteDump` targets `Dbghelp.dll` (not kernel32), (3) all other kernel32 imports are straightforward. Convert `byte[]` parameters to use `[MarshalAs]` or span-based patterns where appropriate. Preserve `SetLastError = true` where present.
- **Validation**: `dotnet build` succeeds. Process launch and suspend/resume work under WINE.
- **Gotcha**: `[LibraryImport]` requires the class to be `partial`. The `LaunchMethod` enum and `ProcessEventArgs` class defined at file-bottom outside the class boundary are fine -- only `ProcessManager` itself needs `partial`.

#### Task 3.2: Convert InjectionManager P/Invoke to LibraryImport

- **Files**: `src/ChooChooEngine.App/Injection/InjectionManager.cs`
- **Scope**: Convert 11 `[DllImport]` declarations (plus 2 in the `#region Additional P/Invoke` block). Add `partial` keyword to the class. Critical conversion: `LoadLibrary` currently uses `CharSet = CharSet.Auto` but the injection code explicitly calls `LoadLibraryA` with `Encoding.ASCII.GetBytes()` (line 248). The `[LibraryImport]` conversion must use `StringMarshalling = StringMarshalling.Utf8` (which maps to the ANSI/UTF-8 expected by `LoadLibraryA`) or keep the manual byte encoding. `GetProcAddress` should use `StringMarshalling.Utf8` (it takes an ANSI function name).
- **Validation**: DLL injection end-to-end test under WINE/Proton succeeds.
- **Gotcha**: `InjectDllStandard()` calls `GetProcAddress(GetModuleHandle("kernel32.dll"), "LoadLibraryA")` -- the string "LoadLibraryA" must remain ANSI. This is the most sensitive P/Invoke in the entire codebase.

#### Task 3.3: Convert MemoryManager P/Invoke to LibraryImport

- **Files**: `src/ChooChooEngine.App/Memory/MemoryManager.cs`
- **Scope**: Convert 3 `[DllImport]` declarations (`ReadProcessMemory`, `WriteProcessMemory`, `VirtualQueryEx`). Add `partial` keyword to the class. These are straightforward conversions with no string marshalling concerns. The `MEMORY_BASIC_INFORMATION` struct and memory constants remain unchanged.
- **Validation**: Memory read/write operations work correctly.

#### Task 3.4: Consolidate Duplicate P/Invoke Declarations

- **Files**: Create `src/ChooChooEngine.App/NativeInterop/Kernel32.cs`, modify ProcessManager.cs, InjectionManager.cs, MemoryManager.cs
- **Scope**: 6 APIs are duplicated between ProcessManager and InjectionManager: `OpenProcess`, `CloseHandle`, `CreateRemoteThread`, `WriteProcessMemory`, `VirtualAllocEx`, `VirtualFreeEx`. Additionally, `WriteProcessMemory` appears in all 3 managers. Create a shared `internal static partial class Kernel32` containing all deduplicated P/Invoke declarations, Win32 structs (`STARTUPINFO`, `PROCESS_INFORMATION`, `MEMORY_BASIC_INFORMATION`), and constants (`PROCESS_ALL_ACCESS`, `MEM_COMMIT`, etc.). Update the 3 manager classes to call `Kernel32.OpenProcess(...)` etc. Consider also a `Dbghelp.cs` for the single `MiniDumpWriteDump` call.
- **Validation**: Build succeeds. All functionality unchanged.
- **Dependency**: Depends on Tasks 3.1, 3.2, 3.3 being complete (or at least coordinate to avoid merge conflicts).

#### Task 3.5: Enable Nullable, C# 12 Features, and Update Documentation

- **Files**: `src/ChooChooEngine.App/ChooChooEngine.App.csproj`, all `.cs` files (nullable annotations), `CLAUDE.md`, `README.md`
- **Scope**: (1) Enable `<Nullable>enable</Nullable>` in csproj (may already be set from Task 1.1). (2) Fix nullable warnings across all files -- key areas: `MemoryManager.ReadMemory()` returns `byte[]?`, `QueryMemoryRegions()` returns `List<MemoryRegion>?`, several MainForm fields initialized to `null`. (3) Adopt file-scoped namespaces across all files. (4) Use target-typed `new` expressions where appropriate. (5) Update CLAUDE.md with new tech stack (.NET 9), build commands (`dotnet build`/`dotnet publish`), and architecture changes. (6) Update README.md to remove XInput references and update installation instructions for self-contained exe.
- **Validation**: `dotnet build` with zero warnings. Documentation accurately reflects post-migration state.
- **Gotcha**: This is the largest task and could be split further (nullable annotations alone will touch every file). Consider splitting: 3.5a (nullable + language features) and 3.5b (documentation updates).

**Parallelization**: Tasks 3.1, 3.2, 3.3 can run fully in parallel (each touches a different manager file). Task 3.4 depends on 3.1-3.3 completion. Task 3.5 can start in parallel with 3.1-3.3 for the documentation portion.

---

## Task Granularity Recommendations

- **1-3 files per task** is maintained across all 14 tasks.
- Phase 1 tasks are small (1-2 files each, mostly deletions) and quick to validate.
- Phase 2 service extraction tasks (2.3, 2.4, 2.5) each create 1 new file and modify 1 existing file (MainForm.cs), which means they touch the same file and require careful merge coordination despite being logically independent.
- Phase 3 P/Invoke tasks (3.1, 3.2, 3.3) are perfectly independent: each modifies exactly 1 file with no overlap.
- Task 3.5 is the broadest and can be split into 2-3 sub-tasks if needed.

---

## Dependency Analysis

### Independent Tasks (Can Run in Parallel)

| Parallel Group             | Tasks         | Constraint                                                 |
| -------------------------- | ------------- | ---------------------------------------------------------- |
| Phase 1                    | 1.1, 1.2, 1.3 | All independent; merge 1.1 first for build                 |
| Phase 2 Service Extraction | 2.3, 2.4, 2.5 | All independent but touch MainForm.cs (merge coordination) |
| Phase 2 Fixes              | 2.1, 2.2, 2.6 | All independent; 2.1 recommended first                     |
| Phase 3 P/Invoke           | 3.1, 3.2, 3.3 | Fully independent (separate files)                         |

### Sequential Dependencies

```
Phase 1 (any task) --> Phase 2 (all tasks)
Phase 2 Task 2.6   --> Phase 3 Tasks 3.1, 3.2 (dead code removal changes P/Invoke surface)
Phase 3 Tasks 3.1 + 3.2 + 3.3 --> Phase 3 Task 3.4 (consolidation requires all conversions done)
```

### Potential Bottlenecks

1. **MainForm.cs contention**: Tasks 2.1, 2.3, 2.4, 2.5, 2.6 all modify `MainForm.cs`. While logically independent (they touch different methods/regions), merge conflicts are likely if run on separate branches. Recommendation: use a single `phase-2` branch with sequential commits, or establish clear line-range ownership.
2. **Task 3.4 (P/Invoke consolidation)** depends on all three P/Invoke conversion tasks completing first. It cannot start until 3.1, 3.2, and 3.3 are merged.
3. **Task 3.5 (nullable/docs)** touches every `.cs` file for nullable annotations, so it should run last or be carefully rebased.

---

## File-to-Task Mapping

### Files to Create

| File                                                    | Suggested Task              | Phase | Dependencies  |
| ------------------------------------------------------- | --------------------------- | ----- | ------------- |
| `src/ChooChooEngine.App/ChooChooEngine.App.csproj`      | Task 1.1 (complete rewrite) | 1     | None          |
| `src/ChooChooEngine.App/Services/ProfileService.cs`     | Task 2.3                    | 2     | Phase 1       |
| `src/ChooChooEngine.App/Services/CommandLineParser.cs`  | Task 2.4                    | 2     | Phase 1       |
| `src/ChooChooEngine.App/Services/RecentFilesService.cs` | Task 2.5                    | 2     | Phase 1       |
| `src/ChooChooEngine.App/NativeInterop/Kernel32.cs`      | Task 3.4                    | 3     | Tasks 3.1-3.3 |

### Files to Modify

| File                                                   | Suggested Task(s)                                                                                                                                                                  | Phase | Dependencies |
| ------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | ------------ |
| `src/ChooChooEngine.App/Core/ProcessManager.cs`        | Task 2.2 (IDisposable), Task 2.6 (remove stubs), Task 3.1 (LibraryImport), Task 3.4 (consolidate)                                                                                  | 2, 3  | Phase 1      |
| `src/ChooChooEngine.App/Injection/InjectionManager.cs` | Task 2.6 (remove ManualMapping stub), Task 3.2 (LibraryImport), Task 3.4 (consolidate)                                                                                             | 2, 3  | Phase 1      |
| `src/ChooChooEngine.App/Memory/MemoryManager.cs`       | Task 3.3 (LibraryImport), Task 3.4 (consolidate)                                                                                                                                   | 3     | Phase 2      |
| `src/ChooChooEngine.App/Forms/MainForm.cs`             | Task 2.1 (fix double subscription), Task 2.2 (dispose call), Task 2.3 (extract profiles), Task 2.4 (extract CLI), Task 2.5 (extract recent files), Task 2.6 (remove radio buttons) | 2     | Phase 1      |
| `src/ChooChooEngine.App/Program.cs`                    | Task 3.5 (nullable, file-scoped namespace)                                                                                                                                         | 3     | Phase 2      |
| `src/ChooChooEngine.App/Forms/MainForm.Designer.cs`    | Task 3.5 (file-scoped namespace)                                                                                                                                                   | 3     | Phase 2      |
| `src/ChooChooEngine.App/UI/ResumePanel.cs`             | Task 3.5 (file-scoped namespace, nullable)                                                                                                                                         | 3     | Phase 2      |
| `src/ChooChooEngine.sln`                               | Task 1.1 (optional GUID update)                                                                                                                                                    | 1     | None         |
| `CLAUDE.md`                                            | Task 3.5                                                                                                                                                                           | 3     | Phase 2      |
| `README.md`                                            | Task 3.5                                                                                                                                                                           | 3     | Phase 2      |

### Files to Delete

| File                                                | Suggested Task | Phase | Dependencies                        |
| --------------------------------------------------- | -------------- | ----- | ----------------------------------- |
| `src/ChooChooEngine.App/Properties/AssemblyInfo.cs` | Task 1.2       | 1     | Task 1.1 (metadata moved to csproj) |
| `src/ChooChooEngine.App/packages.config`            | Task 1.2       | 1     | Task 1.1 (SharpDX removed)          |
| `src/ChooChooEngine.App/bin/` (directory)           | Task 1.3       | 1     | None                                |
| `src/ChooChooEngine.App/obj/` (directory)           | Task 1.3       | 1     | None                                |

---

## Optimization Opportunities

1. **Combine Tasks 1.1 + 1.2 into a single commit**: The csproj rewrite and file deletions are tightly coupled -- the new csproj assumes AssemblyInfo.cs and packages.config do not exist. A single commit avoids an intermediate broken state.

2. **Batch Phase 2 MainForm extractions**: Since Tasks 2.3, 2.4, and 2.5 all modify MainForm.cs, working on them sequentially on the same branch avoids merge conflicts. Total MainForm line reduction: approximately 400-500 lines moved to services.

3. **Defer Task 3.4 (P/Invoke consolidation) if time-constrained**: The duplicate P/Invoke declarations work correctly as-is. Consolidation is a code quality improvement, not a functional requirement. It can be done post-migration as a cleanup task.

4. **Split Task 3.5**: The documentation updates (CLAUDE.md, README.md) can ship independently of nullable annotations. Documentation should ship immediately after Phase 1 completes so contributors have correct build instructions.

5. **Consider a global.json**: Adding `src/global.json` to pin the .NET 9 SDK version (e.g., `{"sdk": {"version": "9.0.100"}}`) prevents contributors from accidentally building with .NET 8 or 10. This is a 1-file, 3-line addition that can slot into Task 1.1.

---

## Implementation Strategy Recommendations

### Branching Strategy

- Create a `feature/dotnet-migrate` branch from `main`.
- Phase 1: Commit directly to feature branch (small, low-risk changes).
- Phase 2: Either commit sequentially or use short-lived sub-branches (`feature/dotnet-migrate-profile-service`, etc.) merged back to the feature branch.
- Phase 3: P/Invoke tasks can use parallel sub-branches since they touch independent files.
- Final: Merge `feature/dotnet-migrate` to `main` after all phases pass validation.

### Validation Checkpoints

| Checkpoint          | Validation                                                                                                                        |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| After Phase 1       | `dotnet build src/ChooChooEngine.sln -c Release` succeeds. `dotnet publish` produces self-contained exe. App launches under WINE. |
| After Task 2.1      | Events fire exactly once per action (verify via console log).                                                                     |
| After Tasks 2.3-2.5 | Profile save/load/delete, CLI args, recent files all work identically.                                                            |
| After Phase 2       | Full functional test: launch game, inject DLL, save/load profiles.                                                                |
| After Tasks 3.1-3.3 | `dotnet build` succeeds. All P/Invoke calls work under WINE.                                                                      |
| After Task 3.4      | No duplicate P/Invoke declarations remain. Build succeeds.                                                                        |
| After Phase 3       | Full regression test. Zero nullable warnings. Documentation updated.                                                              |

### Risk Mitigations

- **Keep the old csproj in git history**: The csproj rewrite is the highest-risk change. If .NET 9 under WINE has blockers, reverting to the old csproj restores .NET Framework 4.8 builds.
- **Test P/Invoke conversions individually**: Each manager's `[LibraryImport]` conversion should be tested in isolation before consolidation. The `LoadLibraryA` ANSI encoding in InjectionManager is the most sensitive area.
- **Do not change the INI file format in this migration**: The feature-spec considers JSON profiles as a future enhancement. Changing the file format during migration adds unnecessary risk. Keep INI format; the extracted services abstract the implementation.
