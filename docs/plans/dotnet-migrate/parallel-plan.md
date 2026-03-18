# dotnet-migrate Implementation Plan

Migrate ChooChoo Loader from .NET Framework 4.8 to .NET 9 across 3 phases with 14 tasks. Phase 1 converts the project to SDK-style csproj targeting `net9.0-windows` with self-contained single-file publish, eliminating the need for .NET runtime installation in WINE/Proton prefixes. Phase 2 extracts 5 services from the 2,800-line MainForm monolith, fixes 4 known bugs (double event subscription, handle leak, dead code stubs, missing CLI feature), and adds IDisposable to ProcessManager. Phase 3 converts 29 `[DllImport]` declaration sites to `[LibraryImport]` source generators, consolidates 6 duplicated P/Invoke APIs into a shared NativeMethods class, enables nullable reference types, and updates documentation. The app remains a Windows binary under WINE — all process injection requires Windows kernel APIs.

## Critically Relevant Files and Documentation

- src/ChooChooEngine.App/ChooChooEngine.App.csproj: Classic 75-line .NET Framework 4.8 csproj — complete rewrite to SDK-style
- src/ChooChooEngine.App/Core/ProcessManager.cs: 505-line process lifecycle manager with 11 DllImport declarations, 2 Win32 structs, no IDisposable (handle leak)
- src/ChooChooEngine.App/Injection/InjectionManager.cs: 353-line DLL injection engine with 12 DllImport declarations (6 duplicated from ProcessManager), critical LoadLibraryA ANSI encoding at line 248
- src/ChooChooEngine.App/Memory/MemoryManager.cs: 368-line process memory manager with 3 DllImport declarations, 1 Win32 struct
- src/ChooChooEngine.App/Forms/MainForm.cs: 2,800-line monolith — all UI, state, profiles, settings, CLI parsing; double event subscription bug at lines 274-283 and 1370-1387
- src/ChooChooEngine.App/Forms/MainForm.Designer.cs: 52-line minimal designer — no changes needed
- src/ChooChooEngine.App/UI/ResumePanel.cs: 107-line custom Panel with proper IDisposable — reference quality implementation
- src/ChooChooEngine.App/Program.cs: 44-line entry point with Mutex single-instance — keep explicit EnableVisualStyles() for WINE
- src/ChooChooEngine.App/Properties/AssemblyInfo.cs: Assembly metadata — DELETE (replaced by csproj properties)
- src/ChooChooEngine.App/packages.config: SharpDX 4.2.0 — DELETE (unused in source)
- CLAUDE.md: Project guidelines — must update build commands and tech stack post-migration
- docs/plans/dotnet-migrate/feature-spec.md: Master specification with P/Invoke migration matrix, csproj template, success criteria
- docs/plans/dotnet-migrate/research-technical.md: Per-API string marshalling notes and WINE compatibility matrix
- docs/plans/dotnet-migrate/research-patterns.md: Code conventions and ANSI encoding gotcha for LoadLibraryA

## Implementation Plan

### Phase 1: Foundation

#### Task 1.1: Convert to SDK-Style csproj and Delete Obsolete Files

Depends on [none]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/ChooChooEngine.App.csproj
- src/ChooChooEngine.App/Properties/AssemblyInfo.cs
- src/ChooChooEngine.App/packages.config
- docs/plans/dotnet-migrate/feature-spec.md (SDK-Style .csproj section)

**Instructions**

Files to Create

- src/ChooChooEngine.App/ChooChooEngine.App.csproj (complete rewrite)

Files to Modify

- src/ChooChooEngine.sln (verify project reference resolves — the classic GUID works fine with `dotnet build`)

Files to Delete

- src/ChooChooEngine.App/Properties/AssemblyInfo.cs
- src/ChooChooEngine.App/packages.config
- src/ChooChooEngine.App/bin/ (directory — stale .NET Framework artifacts)
- src/ChooChooEngine.App/obj/ (directory — stale intermediate files)

Replace the entire 75-line classic csproj with a ~30-line SDK-style file targeting `net9.0-windows`. The new csproj must include:

1. `<TargetFramework>net9.0-windows</TargetFramework>`
2. `<OutputType>WinExe</OutputType>` and `<UseWindowsForms>true</UseWindowsForms>`
3. `<AllowUnsafeBlocks>true</AllowUnsafeBlocks>` (required for P/Invoke)
4. `<Nullable>enable</Nullable>` and `<ImplicitUsings>enable</ImplicitUsings>`
5. Assembly metadata migrated from AssemblyInfo.cs: `<AssemblyTitle>`, `<Description>`, `<Copyright>`, `<Version>6.0.0</Version>`
6. Self-contained publish properties: `<RuntimeIdentifier>win-x64</RuntimeIdentifier>`, `<SelfContained>true</SelfContained>`, `<PublishSingleFile>true</PublishSingleFile>`, `<IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>`, `<EnableCompressionInSingleFile>true</EnableCompressionInSingleFile>`

Delete AssemblyInfo.cs (metadata now in csproj), packages.config (SharpDX 4.2.0 has zero source references), and clean bin/obj directories. Verify `.gitignore` covers bin/ and obj/.

Validate: `dotnet build src/ChooChooEngine.sln -c Release` succeeds with zero errors. `dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Release -r win-x64 --self-contained -p:PublishSingleFile=true` produces a single exe.

Gotcha: SDK-style projects auto-glob all `*.cs` files — no explicit `<Compile Include>` items needed. The empty `Utils/` folder reference in the old csproj is dropped.

#### Task 1.2: Verify WINE/Proton Compatibility

Depends on [1.1]

**READ THESE BEFORE TASK**

- docs/plans/dotnet-migrate/research-external.md (WINE Compatibility section)
- docs/plans/dotnet-migrate/feature-spec.md (Success Criteria)

**Instructions**

Files to Modify

- (none — this is a verification task)

Test the self-contained published exe under WINE/Proton on Arch Linux:

1. Run the published exe under Proton 9+ and verify WinForms UI renders correctly
2. Verify file dialogs (OpenFileDialog) open and return paths
3. Verify profile save/load with INI files works
4. Verify single-instance Mutex enforcement works
5. Verify `Application.StartupPath` returns the correct directory under single-file publish
6. Test process listing (RefreshProcessList)
7. If possible, test DLL injection end-to-end with a real game

Document any WINE compatibility issues found. If blockers are discovered, the old csproj can be restored from git history.

### Phase 2: Architecture Refactoring

#### Task 2.1: Fix Double Event Subscription Bug

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Forms/MainForm.cs (lines 266-286 InitializeManagers, lines 1363-1395 RegisterEventHandlers)

**Instructions**

Files to Modify

- src/ChooChooEngine.App/Forms/MainForm.cs

The `InitializeManagers()` method (lines 274-283) and `RegisterEventHandlers()` method (lines 1370-1387) both subscribe to the same ProcessManager, InjectionManager, MemoryManager, and ResumePanel events. This causes every event handler to fire twice.

Fix: Remove the duplicate manager/panel event subscriptions from `RegisterEventHandlers()`. Keep only the button click and radio button subscriptions in `RegisterEventHandlers()`. Alternatively, consolidate all event subscriptions into a single method.

Also check: `btnRefreshProcesses.Click` may be wired both inline (around line 547) and in `RegisterEventHandlers()` (around line 1390). Remove the duplicate.

Validate: Launch a process and verify console log shows each event message exactly once.

#### Task 2.2: Implement IDisposable on ProcessManager

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Core/ProcessManager.cs (CloseProcessHandle method, lines 355-363)
- src/ChooChooEngine.App/UI/ResumePanel.cs (reference Dispose implementation, lines 94-106)

**Instructions**

Files to Modify

- src/ChooChooEngine.App/Core/ProcessManager.cs
- src/ChooChooEngine.App/Forms/MainForm.cs

ProcessManager holds unmanaged `_processHandle` (IntPtr from `OpenProcess`) but does not implement IDisposable. If `DetachFromProcess()` is not called, the handle leaks.

1. Add `IDisposable` implementation to ProcessManager following the `Dispose(bool disposing)` pattern (use ResumePanel.cs as reference)
2. In `Dispose(bool)`: call `CloseProcessHandle()` to release the Win32 handle
3. Add a `_disposed` bool flag to prevent double-close
4. Add a finalizer `~ProcessManager()` as a safety net calling `Dispose(false)`
5. Update MainForm's `OnFormClosing` to call `_processManager?.Dispose()` instead of just `DetachFromProcess()`

#### Task 2.3: Extract ProfileService from MainForm

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Forms/MainForm.cs (lines 1586-1751 profile methods)
- docs/plans/dotnet-migrate/research-integration.md (Profile System section)

**Instructions**

Files to Create

- src/ChooChooEngine.App/Services/ProfileService.cs

Files to Modify

- src/ChooChooEngine.App/Forms/MainForm.cs

Extract profile management logic from MainForm into a new `ProfileService` class in namespace `ChooChooEngine.App.Services`:

1. `LoadProfiles(string basePath)`: Returns list of available profile names from `{basePath}/Profiles/*.profile`
2. `SaveProfile(string basePath, string name, ProfileData data)`: Writes key=value pairs to `{basePath}/Profiles/{name}.profile`
3. `LoadProfile(string basePath, string name)`: Reads and returns `ProfileData` record/class
4. `DeleteProfile(string basePath, string name)`: Deletes the .profile file

Create a `ProfileData` class/record with: GamePath, TrainerPath, Dll1Path, Dll2Path, LaunchInject1, LaunchInject2, LaunchMethod.

MainForm retains the UI wiring — calls ProfileService methods and maps ProfileData to/from UI controls.

Gotcha: The file format is `Key=Value` pairs (one per line, no section headers). Preserve exact format for backwards compatibility. The `ProfileInputDialog` nested class should remain in MainForm (it is pure UI).

#### Task 2.4: Extract CommandLineParser from MainForm

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Forms/MainForm.cs (lines 2602-2701 ProcessCommandLineArguments)

**Instructions**

Files to Create

- src/ChooChooEngine.App/Services/CommandLineParser.cs

Files to Modify

- src/ChooChooEngine.App/Forms/MainForm.cs

Extract `ProcessCommandLineArguments()` into a standalone class:

1. Create `CommandLineParser` class in `ChooChooEngine.App.Services` namespace
2. Create `CommandLineOptions` record with: `ProfileToLoad` (string?), `AutoLaunchPath` (string?), `AutoLaunchRequested` (bool)
3. Method `Parse(string[] args)` returns `CommandLineOptions`
4. MainForm calls parser in constructor and acts on returned options

Gotcha: The current `-autolaunch` parser consumes all remaining arguments after the flag (for paths with spaces). Preserve this behavior. The `-dllinject` flag is documented in README but not implemented — defer implementation to a separate task.

#### Task 2.5: Extract RecentFilesService and SettingsService from MainForm

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Forms/MainForm.cs (lines 1483-1584 recent files, lines 2703-2788 app settings)
- docs/plans/dotnet-migrate/research-integration.md (File-Based Configuration section)

**Instructions**

Files to Create

- src/ChooChooEngine.App/Services/RecentFilesService.cs

Files to Modify

- src/ChooChooEngine.App/Forms/MainForm.cs

Extract recent files and app settings management:

1. `RecentFilesService` handles `settings.ini` (MRU lists with `[Section]` headers) and `Settings/AppSettings.ini` (key=value app preferences)
2. `LoadRecentFiles(string basePath)`: Returns `RecentFiles` object with three string lists (game paths, trainer paths, DLL paths)
3. `SaveRecentFiles(string basePath, RecentFiles data)`: Writes INI-style sections
4. `LoadAppSettings(string basePath)`: Returns `AppSettings` object (AutoLoadLastProfile bool, LastUsedProfile string)
5. `SaveAppSettings(string basePath, AppSettings data)`: Writes key=value pairs

MainForm populates combo boxes from the returned data objects instead of services writing directly to UI controls.

Gotcha: `LoadRecentFiles()` only adds paths to combo boxes if `File.Exists(line)` is true (line 1517-1520). Preserve this filtering behavior — it should happen in the service, not the UI.

#### Task 2.6: Remove Dead Code Stubs

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Core/ProcessManager.cs (lines 416-428 stub methods, lines 487-495 LaunchMethod enum)
- src/ChooChooEngine.App/Injection/InjectionManager.cs (lines 297-302 ManualMapping stub, lines 337-341 InjectionMethod enum)

**Instructions**

Files to Modify

- src/ChooChooEngine.App/Core/ProcessManager.cs
- src/ChooChooEngine.App/Injection/InjectionManager.cs
- src/ChooChooEngine.App/Forms/MainForm.cs

Remove dead code:

1. Delete `LaunchWithCreateThreadInjection()` and `LaunchWithRemoteThreadInjection()` stub methods from ProcessManager
2. Remove `CreateThreadInjection` and `RemoteThreadInjection` values from `LaunchMethod` enum
3. Remove the corresponding `case` entries from the `switch` in `LaunchProcess()`
4. Delete `InjectDllManualMapping()` stub from InjectionManager
5. Remove `ManualMapping` from `InjectionMethod` enum
6. Remove the corresponding radio buttons (`radCreateThreadInjection`, `radRemoteThreadInjection`) from MainForm's UI construction
7. Remove the dead `PopulateControls()` method from MainForm (never called)

Validate: Build succeeds. Remaining 4 launch methods (CreateProcess, CmdStart, ShellExecute, ProcessStart) work correctly.

Gotcha: Removing `CreateThreadInjection` and `RemoteThreadInjection` from the `LaunchMethod` enum will break `Enum.TryParse` for existing `.profile` files that serialized those values (e.g., `LaunchMethod=CreateThreadInjection`). Add fallback handling in profile loading that maps unknown/removed enum values to the default `CreateProcess` method.

### Phase 3: P/Invoke Modernization

#### Task 3.1: Convert ProcessManager DllImport to LibraryImport

Depends on [2.6]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Core/ProcessManager.cs (lines 13-112 Win32 API region)
- docs/plans/dotnet-migrate/research-technical.md (P/Invoke Migration section)
- docs/plans/dotnet-migrate/analysis-code.md (P/Invoke Migration Matrix)

**Instructions**

Files to Modify

- src/ChooChooEngine.App/Core/ProcessManager.cs

Convert all `[DllImport]` declarations to `[LibraryImport]` source generators:

1. Add `partial` keyword to `ProcessManager` class declaration
2. For each P/Invoke method: replace `[DllImport("kernel32.dll")]` with `[LibraryImport("kernel32.dll")]`, replace `private static extern` with `private static partial`, add `[MarshalAs(UnmanagedType.Bool)]` to all `bool` parameters and return types
3. Special handling for `CreateProcess`: add `StringMarshalling = StringMarshalling.Utf16` (converts to `CreateProcessW` for proper WINE path handling). Also add `[MarshalAs(UnmanagedType.LPWStr)]` to string fields in `STARTUPINFO` struct.
4. `MiniDumpWriteDump` targets `Dbghelp.dll` (not kernel32) — use `[LibraryImport("Dbghelp.dll", SetLastError = true)]`
5. Structs (`STARTUPINFO`, `PROCESS_INFORMATION`) and constants remain unchanged

Validate: `dotnet build` succeeds. Process launch and suspend/resume work.

#### Task 3.2: Convert InjectionManager DllImport to LibraryImport

Depends on [2.6]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Injection/InjectionManager.cs (lines 15-65 Win32 API region, lines 312-318 additional region)
- docs/plans/dotnet-migrate/research-patterns.md (ANSI encoding gotcha)
- docs/plans/dotnet-migrate/analysis-code.md (P/Invoke Migration Matrix)

**Instructions**

Files to Modify

- src/ChooChooEngine.App/Injection/InjectionManager.cs

Convert all `[DllImport]` declarations to `[LibraryImport]`:

1. Add `partial` keyword to `InjectionManager` class declaration
2. Standard conversion for all P/Invoke methods (remove `extern`, add `partial`, add `[MarshalAs]` on bools)
3. `LoadLibrary`: use `StringMarshalling = StringMarshalling.Utf16` (this is the validation-only call at line 176)
4. `GetProcAddress`: use `StringMarshalling = StringMarshalling.Utf8` (this API only accepts ANSI function names)
5. `GetModuleHandle`: use `StringMarshalling = StringMarshalling.Utf16`

**CRITICAL**: The `InjectDllStandard()` method (line 243) calls `GetProcAddress(GetModuleHandle("kernel32.dll"), "LoadLibraryA")` and at line 248 uses `Encoding.ASCII.GetBytes(dllPath)`. This ANSI encoding is intentional for WINE compatibility. Do NOT change the `"LoadLibraryA"` string or the `Encoding.ASCII.GetBytes` call. The `LoadLibrary` P/Invoke conversion to UTF-16 only affects the local validation call (line 176), not the injection path.

Validate: DLL injection end-to-end test under WINE succeeds.

#### Task 3.3: Convert MemoryManager DllImport to LibraryImport

Depends on [2.6]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Memory/MemoryManager.cs (lines 15-37 Win32 API region)

**Instructions**

Files to Modify

- src/ChooChooEngine.App/Memory/MemoryManager.cs

Convert 3 `[DllImport]` declarations to `[LibraryImport]`:

1. Add `partial` keyword to `MemoryManager` class declaration
2. Convert `ReadProcessMemory`, `WriteProcessMemory`, `VirtualQueryEx` — straightforward conversions with no string marshalling concerns
3. Add `[MarshalAs(UnmanagedType.Bool)]` to bool return types on ReadProcessMemory and WriteProcessMemory
4. `MEMORY_BASIC_INFORMATION` struct and all memory constants remain unchanged (all blittable types)

Validate: Memory read/write operations work correctly.

#### Task 3.4: Consolidate Duplicate P/Invoke into Shared NativeMethods

Depends on [3.1, 3.2, 3.3]

**READ THESE BEFORE TASK**

- src/ChooChooEngine.App/Core/ProcessManager.cs
- src/ChooChooEngine.App/Injection/InjectionManager.cs
- src/ChooChooEngine.App/Memory/MemoryManager.cs
- docs/plans/dotnet-migrate/research-architecture.md (P/Invoke Duplication Map)

**Instructions**

Files to Create

- src/ChooChooEngine.App/NativeInterop/Kernel32.cs
- src/ChooChooEngine.App/NativeInterop/Dbghelp.cs

Files to Modify

- src/ChooChooEngine.App/Core/ProcessManager.cs
- src/ChooChooEngine.App/Injection/InjectionManager.cs
- src/ChooChooEngine.App/Memory/MemoryManager.cs

Create a shared `internal static partial class Kernel32` in `ChooChooEngine.App.NativeInterop` namespace containing the 6 APIs duplicated between ProcessManager and InjectionManager: `OpenProcess`, `CloseHandle`, `CreateRemoteThread`, `WriteProcessMemory`, `VirtualAllocEx`, `VirtualFreeEx`. Also move shared constants (`PROCESS_ALL_ACCESS`, `MEM_COMMIT`, `MEM_RESERVE`, `MEM_RELEASE`, `PAGE_READWRITE`).

Create `internal static partial class Dbghelp` with `MiniDumpWriteDump` and its constants.

Update the 3 manager classes to call `Kernel32.OpenProcess(...)`, `Kernel32.CloseHandle(...)`, etc. Remove the duplicated declarations from each manager. Each manager retains only its unique P/Invoke declarations (e.g., InjectionManager keeps GetProcAddress, GetModuleHandle, LoadLibrary; ProcessManager keeps OpenThread, SuspendThread, ResumeThread, CreateProcess).

Move Win32 structs to the NativeInterop files: `STARTUPINFO` and `PROCESS_INFORMATION` to Kernel32.cs, `MEMORY_BASIC_INFORMATION` to Kernel32.cs.

Validate: Build succeeds. All functionality unchanged. Total P/Invoke declaration sites reduced from 29 to 19.

#### Task 3.5: Enable Nullable, C# 12 Features, and Update Documentation

Depends on [3.4]

**READ THESE BEFORE TASK**

- CLAUDE.md
- README.md
- docs/plans/dotnet-migrate/feature-spec.md (Decisions Needed section)

**Instructions**

Files to Modify

- All .cs files (nullable annotations, file-scoped namespaces)
- CLAUDE.md
- README.md

1. **Nullable reference types**: Ensure `<Nullable>enable</Nullable>` is in csproj (set in Task 1.1). Address nullable warnings across all files. Key areas: `MemoryManager.ReadMemory()` returns `byte[]?`, `QueryMemoryRegions()` returns `List<MemoryRegion>?`, MainForm fields initialized to `null`. Add `?` annotations or `!` suppressions as appropriate.

2. **C# 12 features**: Convert all files to file-scoped namespaces (`namespace ChooChooEngine.App.Core;`). Use target-typed `new` expressions where type is obvious from context. Consider collection expressions for simple list initialization.

3. **Update CLAUDE.md**: Change tech stack from ".NET Framework 4.8" to ".NET 9 (net9.0-windows)". Change build commands from `msbuild` to `dotnet build`/`dotnet publish`. Update architecture file tree to reflect new files (Services/, NativeInterop/). Remove "dotnet build will NOT work" warning. Also update `AGENTS.md` and `.cursorrules` if they exist with same content.

4. **Update README.md**: Remove XInput/controller feature claims that no longer exist in source. Update installation instructions for self-contained exe (no .NET runtime needed in WINE prefix).

Validate: `dotnet build` succeeds with zero nullable warnings. Documentation accurately reflects post-migration state.

## Advice

- **Phase 1 is the critical path and highest-risk**: The SDK-style csproj conversion is a one-shot change that either works or doesn't under WINE. Keep the old csproj in git history for fallback. Test under WINE/Proton immediately after the build succeeds (Task 1.2).

- **MainForm.cs is the merge bottleneck in Phase 2**: Tasks 2.1, 2.3, 2.4, 2.5, and 2.6 all modify MainForm.cs. While logically independent (they touch different methods/regions), merge conflicts are likely if run on separate branches. Recommend working sequentially on a single branch, or establishing clear line-range ownership.

- **The LoadLibraryA ANSI encoding is the most sensitive P/Invoke**: In InjectionManager.InjectDllStandard(), the code deliberately uses `GetProcAddress(..., "LoadLibraryA")` with `Encoding.ASCII.GetBytes`. This is the actual DLL injection path that runs in the target game's process space. Changing this to Unicode would break injection. The `LoadLibrary` P/Invoke used for local validation (line 176) CAN safely use UTF-16.

- **Task 3.4 (P/Invoke consolidation) is optional but recommended**: The duplicate declarations work correctly as-is. Consolidation reduces maintenance burden but is not a functional requirement. If time-constrained, defer it.

- **WINE testing should happen after Phase 1 AND after Phase 3**: Phase 1 validates that .NET 9 runs under WINE at all. Phase 3 validates that the LibraryImport source-generated marshalling produces the same native calls as DllImport. Both are critical verification points.

- **Self-contained exe size will increase from ~80KB to ~60-80MB**: This is expected and acceptable — it eliminates the .NET runtime installation requirement in WINE, which is the #1 user pain point. Consider `<EnableCompressionInSingleFile>true</EnableCompressionInSingleFile>` to reduce size.

- **The ProfileInputDialog nested class should stay in MainForm for now**: It's pure UI and tightly coupled to the profile save flow. Extracting it to its own file is a nice-to-have, not part of the migration scope.

- **Application.StartupPath behavior under single-file publish**: With `<IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>`, it returns the exe's directory (correct behavior). Without this property, it returns a temp extraction directory, which would break profile/settings paths. This property is set in the csproj template.
