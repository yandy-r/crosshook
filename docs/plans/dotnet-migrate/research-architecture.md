# Architecture Research: dotnet-migrate

> Background architecture analysis. Use `feature-spec.md` and `parallel-plan.md` for the actual in-scope execution plan.

## System Overview

CrossHook Loader is a 4,264-line C# WinForms application (.NET Framework 4.8) that serves as a game trainer launcher and DLL injector, designed to run under Proton/WINE on Linux/Steam Deck. The architecture is a single-project solution (`CrossHookEngine.App`) with four distinct layers -- Core (process management), Injection (DLL injection via LoadLibraryA/CreateRemoteThread), Memory (process memory read/write), and Forms/UI -- all orchestrated by a 2,800-line MainForm monolith that owns UI construction, state management, profile persistence, settings I/O, command-line parsing, and launch orchestration. Component communication is event-driven via `EventHandler<T>` delegates, with all Win32 interop implemented through 29 `[DllImport]` declaration sites covering 19 unique kernel32.dll/Dbghelp.dll APIs.

## Relevant Components

- `/src/CrossHookEngine.sln`: Solution file; single project, VS2022 format, project GUID `{FAE04EC0-...}` (C# classic)
- `/src/CrossHookEngine.App/CrossHookEngine.App.csproj`: Classic-format 75-line .csproj targeting .NET Framework 4.8; `AllowUnsafeBlocks=true`, `OutputType=WinExe`, `AssemblyName=crosshook`; references System.Drawing, System.Windows.Forms, System.Net.Http; includes explicit `<Compile>` items
- `/src/CrossHookEngine.App/Program.cs` (44 lines): Entry point; single-instance enforcement via named Mutex (`CrossHookEngineInjectorSingleInstance`); passes CLI args to MainForm
- `/src/CrossHookEngine.App/Core/ProcessManager.cs` (505 lines): Process lifecycle manager with 11 P/Invoke declarations (OpenProcess, CloseHandle, CreateRemoteThread, WriteProcessMemory, VirtualAllocEx, VirtualFreeEx, OpenThread, SuspendThread, ResumeThread, CreateProcess, MiniDumpWriteDump); 6 launch method implementations; event-based notifications; holds unmanaged `_processHandle` without IDisposable
- `/src/CrossHookEngine.App/Injection/InjectionManager.cs` (353 lines): DLL injection engine with 11 P/Invoke declarations (6 duplicated from ProcessManager); standard injection via LoadLibraryA + CreateRemoteThread + VirtualAllocEx + WriteProcessMemory; PE header parsing for 32/64-bit validation; thread-safe via `_injectionLock`; timer-based auto-injection monitoring
- `/src/CrossHookEngine.App/Memory/MemoryManager.cs` (368 lines): Process memory read/write with 3 P/Invoke declarations (ReadProcessMemory, WriteProcessMemory, VirtualQueryEx); memory region enumeration; save/restore memory state to binary files
- `/src/CrossHookEngine.App/Forms/MainForm.cs` (2800 lines): The monolith; contains all UI construction (~1400 lines in `ConfigureUILayout()`), all event handlers, profile CRUD, settings I/O, recent files MRU, command-line parsing, launch orchestration, and a nested `ProfileInputDialog` class
- `/src/CrossHookEngine.App/Forms/MainForm.Designer.cs` (52 lines): Minimal designer file; only sets AutoScale, BackColor, ClientSize, ForeColor, Name, Text; all real UI is constructed programmatically in MainForm.cs
- `/src/CrossHookEngine.App/UI/ResumePanel.cs` (107 lines): Custom Panel subclass; semi-transparent overlay shown on form deactivation ("CLICK TO RESUME"); custom OnPaint with GDI+ drawing; proper IDisposable for GDI resources
- `/src/CrossHookEngine.App/Properties/AssemblyInfo.cs` (35 lines): Assembly metadata attributes; to be deleted during migration (replaced by csproj properties)
- `/src/CrossHookEngine.App/packages.config`: Single dependency on SharpDX 4.2.0 (unused in source code; to be removed)
- `/src/CrossHookEngine.App/bin/Debug/Settings/AppSettings.ini`: Runtime settings file (AutoLoadLastProfile, LastUsedProfile)

## Data Flow

### Application Startup Flow

```
Program.Main(args)
  -> Mutex check (single instance)
  -> Application.EnableVisualStyles()
  -> Application.SetCompatibleTextRenderingDefault(false)
  -> new MainForm(args)
       -> InitializeComponent() (designer)
       -> ConfigureTabControl() (3 tabs: Main, Help, Tools)
       -> ConfigureUILayout() (~1400 lines of programmatic UI construction)
       -> ApplyDarkTheme()
       -> InitializeManagers()
            -> new ProcessManager()
            -> new InjectionManager(processManager)  [depends on ProcessManager]
            -> new MemoryManager(processManager)      [depends on ProcessManager]
            -> new ResumePanel()
            -> Subscribe to manager events
       -> RegisterEventHandlers()  [BUG: double-subscribes to same events]
       -> LoadRecentFiles()  [reads settings.ini, populates ComboBoxes]
       -> LoadProfiles()     [reads Profiles/*.profile, populates cmbProfiles]
       -> ProcessCommandLineArguments()  [parses -p, -autolaunch]
       -> LoadAppSettings()  [reads Settings/AppSettings.ini]
       -> CheckLayoutMode()  [responsive: compact < 950px width]
  -> Application.Run(mainForm)
```

### Launch Workflow (Critical Path)

```
BtnLaunch_Click
  -> Validate _selectedGamePath (optional) and _selectedTrainerPath
  -> ProcessManager.LaunchProcess(gamePath, workingDir, launchMethod)
       -> Switch on LaunchMethod enum:
            CreateProcess:  kernel32 CreateProcess P/Invoke
            CmdStart:       cmd.exe /c start, then find process by name
            ShellExecute:   Process.Start with UseShellExecute=true
            ProcessStart:   Process.Start with UseShellExecute=false
            (CreateThread/RemoteThread: stubs, fall through to CreateProcess)
       -> Opens process handle via OpenProcess(PROCESS_ALL_ACCESS)
       -> Fires ProcessStarted event
  -> If DLL injection checkboxes checked:
       -> InjectionManager.InjectDll(dllPath)
            -> ValidateDll: LoadLibrary in own process + PE header check
            -> lock(_injectionLock)
            -> InjectDllStandard:
                 GetModuleHandle("kernel32.dll")
                 GetProcAddress(handle, "LoadLibraryA")
                 VirtualAllocEx (allocate in target process)
                 WriteProcessMemory (write DLL path)
                 CreateRemoteThread (call LoadLibraryA)
                 WaitForSingleObject (wait for thread)
                 GetExitCodeThread (verify success)
                 VirtualFreeEx (cleanup)
            -> Fires InjectionSucceeded or InjectionFailed event
  -> If trainer path set:
       -> ProcessManager.LaunchProcess(trainerPath, ...)
  -> RefreshLoadedDllsList()
```

### Persistence Layer

Settings and state are persisted through three separate INI-style file mechanisms, all implemented inline in MainForm.cs:

1. **Recent Files** (`settings.ini` beside exe): Section-based INI with `[RecentGamePaths]`, `[RecentTrainerPaths]`, `[RecentDllPaths]`; read via `LoadRecentFiles()`, written via `SaveRecentFiles()`
2. **Profiles** (`Profiles/*.profile` directory): Key=Value pairs (GamePath, TrainerPath, Dll1Path, Dll2Path, LaunchInject1, LaunchInject2, LaunchMethod); read via `LoadProfile()`, written via `SaveProfile()`
3. **App Settings** (`Settings/AppSettings.ini`): Key=Value pairs (AutoLoadLastProfile, LastUsedProfile); read via `LoadAppSettings()`, written via `SaveAppSettings()`

All three use hand-rolled string parsing with `File.ReadAllLines()` / `StreamWriter`, no serialization framework.

### Event Flow Between Components

```
ProcessManager   --[ProcessStarted/Stopped/Attached/Detached]--> MainForm (UpdateStatus, LogToConsole, RefreshLoadedDllsList)
InjectionManager --[InjectionSucceeded/Failed]-----------------> MainForm (UpdateStatus, LogToConsole, RefreshLoadedDllsList)
MemoryManager    --[MemoryOperationSucceeded/Failed]-----------> MainForm (UpdateStatus, LogToConsole)
ResumePanel      --[Resumed]-----------------------------------> MainForm (UpdateStatus)
```

All event handlers in MainForm use `InvokeRequired`/`Invoke` pattern for thread safety, since `InjectionManager._monitoringTimer` fires on a ThreadPool thread.

## Integration Points

### Where Migration Changes Connect

1. **`.csproj` replacement**: The 75-line classic csproj at `/src/CrossHookEngine.App/CrossHookEngine.App.csproj` is replaced entirely with a ~30-line SDK-style file. This is the single most impactful change -- it enables `dotnet build`, removes explicit `<Compile>` items (SDK auto-globs), removes `<Reference>` items (implicit via TFM), and enables self-contained publish.

2. **P/Invoke modernization** (3 files): Each of the three manager classes must be made `partial` and have their `[DllImport] ... extern` declarations converted to `[LibraryImport] ... partial`. The 6 duplicated APIs (OpenProcess, CloseHandle, CreateRemoteThread, WriteProcessMemory, VirtualAllocEx, VirtualFreeEx) shared between ProcessManager and InjectionManager should be consolidated into a shared `NativeMethods` class during or after migration.

3. **File deletions**: `Properties/AssemblyInfo.cs` (metadata moves to csproj), `packages.config` (SharpDX removed, no replacement needed), and `bin/`/`obj/` directories (stale .NET Framework artifacts).

4. **MainForm.cs refactoring** (Phase 2): The 2,800-line monolith contains at least 5 extractable service boundaries:
   - **ProfileService** (~150 lines): `SaveProfile()`, `LoadProfile()`, `LoadProfiles()`, `BtnSave_Click`, `BtnLoad_Click`, `BtnDelete_Click`
   - **CommandLineParser** (~100 lines): `ProcessCommandLineArguments()`
   - **LaunchOrchestrator** (~120 lines): `BtnLaunch_Click()`, `InjectDll()`
   - **RecentFilesService** (~100 lines): `LoadRecentFiles()`, `SaveRecentFiles()`
   - **AppSettingsService** (~90 lines): `SaveAppSettings()`, `LoadAppSettings()`, `ChkAutoLoadLastProfile_CheckedChanged`

5. **Program.cs**: Minimal changes. The `[STAThread] Main` method, Mutex enforcement, and `Application.EnableVisualStyles()` / `SetCompatibleTextRenderingDefault(false)` pattern are preserved (recommended over `ApplicationConfiguration.Initialize()` for WINE compatibility).

### Components NOT Affected

- `Forms/MainForm.Designer.cs`: No changes needed (uses only standard WinForms APIs)
- `UI/ResumePanel.cs`: No changes needed (pure WinForms, proper IDisposable)
- Profile file format (.profile, settings.ini, AppSettings.ini): Backwards compatible, no format changes

## Key Dependencies

### External Dependencies

| Dependency           | Type                    | Current State                         | Migration Impact                                    |
| -------------------- | ----------------------- | ------------------------------------- | --------------------------------------------------- |
| .NET Framework 4.8   | Runtime                 | Target framework                      | Replaced by `net9.0-windows`                        |
| System.Windows.Forms | Framework lib           | Assembly reference                    | `<UseWindowsForms>true</UseWindowsForms>` in csproj |
| System.Drawing       | Framework lib           | Assembly reference                    | Implicit via TFM (Windows-only)                     |
| System.Net.Http      | Framework lib           | Assembly reference (unused in source) | Implicit via TFM                                    |
| kernel32.dll         | Win32 native            | 17 unique APIs via P/Invoke           | All compatible; convert DllImport to LibraryImport  |
| Dbghelp.dll          | Win32 native            | 1 API (MiniDumpWriteDump)             | Compatible; partial WINE support                    |
| SharpDX 4.2.0        | NuGet (packages.config) | Listed but **unused in source**       | Remove entirely                                     |
| MSBuild (classic)    | Build system            | `msbuild src/CrossHookEngine.sln`     | Replaced by `dotnet build`                          |

### Internal Dependency Graph

```
Program.cs
  └── MainForm (Forms)
        ├── ProcessManager (Core)     [no dependencies]
        ├── InjectionManager (Injection) [depends on ProcessManager]
        ├── MemoryManager (Memory)       [depends on ProcessManager]
        └── ResumePanel (UI)             [no dependencies]
```

`InjectionManager` and `MemoryManager` both receive a `ProcessManager` instance via constructor injection and call `ProcessManager.GetProcessHandle()` to obtain the native process handle for P/Invoke operations. This is the only inter-component dependency beyond MainForm's orchestration role.

### P/Invoke Duplication Map (29 declaration sites, 19 unique APIs)

| API                 | ProcessManager | InjectionManager | MemoryManager |
| ------------------- | -------------- | ---------------- | ------------- |
| OpenProcess         | yes            | yes              | -             |
| CloseHandle         | yes            | yes              | -             |
| CreateRemoteThread  | yes            | yes              | -             |
| WriteProcessMemory  | yes            | yes              | yes           |
| VirtualAllocEx      | yes            | yes              | -             |
| VirtualFreeEx       | yes            | yes              | -             |
| OpenThread          | yes            | -                | -             |
| SuspendThread       | yes            | -                | -             |
| ResumeThread        | yes            | -                | -             |
| CreateProcess       | yes            | -                | -             |
| MiniDumpWriteDump   | yes            | -                | -             |
| GetProcAddress      | -              | yes              | -             |
| GetModuleHandle     | -              | yes              | -             |
| LoadLibrary         | -              | yes              | -             |
| FreeLibrary         | -              | yes              | -             |
| WaitForSingleObject | -              | yes              | -             |
| GetExitCodeThread   | -              | yes              | -             |
| ReadProcessMemory   | -              | -                | yes           |
| VirtualQueryEx      | -              | -                | yes           |

The 6 shared APIs between ProcessManager and InjectionManager (10 duplicate declaration sites) are candidates for consolidation into a shared `NativeMethods` partial class during Phase 3.

## Architectural Patterns

- **Monolith UI with service composition**: MainForm constructs all three managers, wires all events, and owns all state. There are no interfaces, no dependency injection, and no separation of concerns within MainForm itself.
- **Constructor injection (manual)**: InjectionManager and MemoryManager receive ProcessManager via constructor parameters -- a simple but effective pattern.
- **Event-driven communication**: All inter-component communication uses C# events (`EventHandler<TEventArgs>`), with custom EventArgs classes per component (ProcessEventArgs, InjectionEventArgs, MemoryEventArgs).
- **P/Invoke isolation by concern**: Each manager class contains its own `#region Win32 API` block with only the P/Invoke declarations it needs. This leads to duplication but keeps each class self-contained.
- **INI-style file persistence**: Three separate hand-rolled INI parsers for profiles, recent files, and app settings. No shared parsing infrastructure.
- **Programmatic UI construction**: ~1,400 lines of manual control creation in `ConfigureUILayout()` with only a minimal 52-line designer file. This means the migration has no designer serialization format concerns.
- **Responsive layout**: Compact mode toggle at 950px width threshold with debounced resize timer, switching between two `TableLayoutPanel` configurations.
- **Win32 struct interop**: `STARTUPINFO`, `PROCESS_INFORMATION`, and `MEMORY_BASIC_INFORMATION` structs with `[StructLayout(LayoutKind.Sequential)]` for P/Invoke marshalling.
- **Launch method strategy pattern**: `LaunchMethod` enum with switch-based dispatch in `ProcessManager.LaunchProcess()` across 6 implementations (2 are stubs).

## Edge Cases and Gotchas

- **Double event subscription bug**: `InitializeManagers()` (line 266) and `RegisterEventHandlers()` (line 1363) both subscribe to ProcessManager and InjectionManager events, causing every handler to fire twice. This is a pre-existing bug that should be fixed during Phase 2 refactoring.
- **Handle leak**: `ProcessManager._processHandle` is obtained via `OpenProcess(PROCESS_ALL_ACCESS)` but the class does not implement `IDisposable`. If the form closes without calling `DetachFromProcess()`, the handle leaks. The `OnFormClosing` handler does call `DetachFromProcess()` but lacks a finalizer safety net.
- **Dead code stubs**: `LaunchWithCreateThreadInjection()`, `LaunchWithRemoteThreadInjection()` (ProcessManager), and `InjectDllManualMapping()` (InjectionManager) all silently fall through to their default implementations with no indication to the user.
- **Missing `-dllinject` CLI feature**: The README documents `-dllinject [Dll1.dll] [Dll2.dll] ...` but `ProcessCommandLineArguments()` does not implement it.
- **DLL validation side effects**: `ValidateDll()` calls `LoadLibrary()` on the DLL in CrossHook's own process to check validity. If the DLL's `DllMain` has side effects, this could cause unintended behavior.
- **ProfileInputDialog nested inside MainForm**: A 100-line `Form` subclass defined as a nested class inside MainForm.cs (lines 104-208). Should be extracted to its own file during refactoring.
- **Race condition in DLL validation cache**: `_validatedDlls` dictionary in InjectionManager is accessed without synchronization outside `_injectionLock`. Currently safe because all calls originate from the UI thread, but would break if async operations are introduced.
- **Thread safety in event handlers**: All MainForm event handlers use `InvokeRequired`/`Invoke` for cross-thread marshalling, which is correct. However, `InjectionManager._monitoringTimer` is a `System.Timers.Timer` that fires on a ThreadPool thread, meaning `InjectionManager.InjectAllDlls()` -> `InjectDll()` -> events fire from a non-UI thread.
- **`PopulateControls()` is dead code**: The method at line 1413 calls `LoadRecentFiles()`, `LoadProfiles()`, `RefreshProcessList()`, and `ShowCurrentEnvironmentModules()`, but is never called from anywhere.
- **Resize timer never stopped**: The `resizeTimer` created in the constructor (line 258) starts ticking every 100ms but is never explicitly stopped. The `MainForm_SizeChanged` handler (line 313) creates a _new_ timer with 200ms interval if `resizeTimer` is null, creating two separate timer codepaths.

## Other Docs

- `/docs/plans/dotnet-migrate/feature-spec.md`: Complete migration feature specification with phasing, decisions, risk assessment, and task breakdown
- `/docs/plans/dotnet-migrate/research-technical.md`: P/Invoke migration matrix, SDK-style csproj template, WinForms control compatibility table, WINE runtime considerations
- `/docs/plans/dotnet-migrate/research-business.md`: Core functionality inventory, user stories, business rules, critical workflow documentation
- `/docs/plans/dotnet-migrate/research-ux.md`: UI framework comparison (WinForms/Avalonia), competitive landscape, Steam Deck UX considerations
- `/docs/plans/dotnet-migrate/research-recommendations.md`: Migration strategy options, technology choices, phasing plan, architecture improvement ideas
- `/docs/plans/dotnet-migrate/research-external.md`: .NET 8/9 migration APIs, LibraryImport patterns, WINE compatibility research
- [Microsoft: Upgrade WinForms to .NET](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/)
- [Microsoft: P/Invoke Source Generation](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation)
- [Microsoft: Single-File Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)

# Note

This architecture research includes extraction ideas that are broader than the active migration scope. Use `feature-spec.md` and `parallel-plan.md` for the actual in-scope execution plan.
