# Business Logic Research: dotnet-migrate

## Executive Summary

ChooChoo Loader is a WinForms-based game trainer launcher and DLL injector that runs as a Windows binary under Proton/WINE on Linux, macOS, and Steam Deck. The application's core value proposition -- launching games alongside trainers and injecting DLLs into game processes -- is fundamentally tied to the Windows Win32 API via 26 P/Invoke declarations across three manager classes. Migration to modern .NET (8/9) is feasible as a Windows-targeted binary (the app must still run under Proton/WINE), since all P/Invoke calls target `kernel32.dll` and `Dbghelp.dll` which are implemented by WINE, and WinForms is supported on .NET 8+ (Windows only). True native cross-platform (running natively on Linux without WINE) is not viable because the entire injection and process manipulation stack depends on Windows kernel APIs that have no Linux equivalents.

## User Stories

### Primary User: Linux/Steam Deck Gamer

- As a Linux gamer, I want to launch game trainers (FLiNG, WeMod) alongside my games so that I can use cheat/mod features that normally only work on Windows.
- As a Steam Deck user, I want to configure game + trainer + DLL combinations as profiles so that I can launch modded games with a single button press from Gaming Mode.
- As a Proton user, I want to select from multiple process launch methods (CreateProcess, ShellExecute, CMD Start, etc.) so that I can work around WINE compatibility issues with specific games or trainers.
- As a gamer, I want DLL architecture validation (32-bit vs 64-bit) before injection so that I do not waste time on mismatched binaries.

### Secondary User: Fork Maintainer

- As a fork maintainer, I want to migrate to modern .NET so that I can use the `dotnet` CLI for building, take advantage of SDK-style projects, and access modern C# language features and NuGet tooling.
- As a fork maintainer, I want to move away from .NET Framework 4.8 so that WINE bottles do not need legacy .NET Framework installations, since modern .NET self-contained deployments bundle the runtime.
- As a fork maintainer, I want to simplify the build pipeline so that CI/CD can use `dotnet build` and `dotnet publish` instead of requiring MSBuild with .NET Framework targeting packs.

## Business Rules

### Core Rules (Must Preserve)

1. **Single-Instance Enforcement**: Only one instance of ChooChoo may run at a time, enforced via a named `Mutex` (`ChooChooEngineInjectorSingleInstance`). If a second instance is launched, it must show a message and exit. (`Program.cs`, lines 10-30)
2. **DLL Architecture Validation**: Before injection, the app must verify that the target DLL's architecture (32-bit vs 64-bit) matches the process architecture by reading the PE header. Mismatched DLLs must be refused. (`InjectionManager.cs`, `ValidateDll` and `IsDll64Bit` methods)
3. **Process Handle Lifecycle**: Process handles obtained via `OpenProcess(PROCESS_ALL_ACCESS)` must be explicitly closed via `CloseHandle` to prevent resource leaks. The `_processHandleOpen` flag tracks handle state. (`ProcessManager.cs`, lines 337-363)
4. **Injection via LoadLibraryA + CreateRemoteThread**: The standard injection path allocates memory in the target process (`VirtualAllocEx`), writes the DLL path (`WriteProcessMemory`), creates a remote thread calling `LoadLibraryA`, waits for completion, then frees the allocated memory. This is the critical path and must be preserved exactly. (`InjectionManager.cs`, `InjectDllStandard`, lines 240-295)
5. **Thread-Safe Injection**: DLL injection operations are serialized via `lock (_injectionLock)` to prevent concurrent injection attempts from corrupting process state. (`InjectionManager.cs`, line 129)
6. **Profile Persistence**: Profiles are saved as `.profile` files in a `Profiles/` directory alongside the executable, using a simple `Key=Value` format. The profile stores: GamePath, TrainerPath, Dll1Path, Dll2Path, LaunchInject1/2 flags, and LaunchMethod. (`MainForm.cs`, `SaveProfile`/`LoadProfile` methods)
7. **Recent Files (MRU) Persistence**: Recently used game, trainer, and DLL paths are stored in `settings.ini` with INI-style `[Section]` headers. Only paths to files that still exist are loaded. (`MainForm.cs`, `LoadRecentFiles`/`SaveRecentFiles`)
8. **App Settings Persistence**: Auto-load preference and last used profile name are stored in `Settings/AppSettings.ini`. (`MainForm.cs`, `SaveAppSettings`/`LoadAppSettings`)
9. **Command-Line Interface**: The app must support `-p "ProfileName"` (load a specific profile), `-autolaunch <path>` (auto-launch a game after 1-second delay), and `-dllinject` (inject specified DLLs). (`MainForm.cs`, `ProcessCommandLineArguments`, lines 2602-2701)
10. **Multiple Launch Methods**: Six distinct process launch methods must be supported: CreateProcess (P/Invoke), CMD Start, CreateThread Injection, RemoteThread Injection, ShellExecute, and ProcessStart (.NET). This allows users to work around WINE compatibility issues. (`ProcessManager.cs`, `LaunchMethod` enum and corresponding methods)

### Edge Cases

- **Auto-launch without game path**: If `-autolaunch` is used with a profile that has a trainer but no game, the trainer should still launch without prompting the user. (`MainForm.cs`, lines 2109-2161)
- **Missing DLLs on profile load**: When loading recent files, only file paths that still exist on disk are added to the combo boxes. Missing files are silently skipped. (`MainForm.cs`, `LoadRecentFiles`, line 1517)
- **CreateThread / RemoteThread injection placeholders**: `LaunchWithCreateThreadInjection` and `LaunchWithRemoteThreadInjection` are currently stubs that fall back to `LaunchWithCreateProcess`. These are placeholders for future implementation. (`ProcessManager.cs`, lines 416-428)
- **Manual Mapping injection placeholder**: `InjectDllManualMapping` falls back to standard injection. This is a stub. (`InjectionManager.cs`, lines 297-302)
- **Process list filtering**: System processes with PID <= 4 and the app's own process are excluded from the running process list. Some processes that throw access errors are silently skipped. (`MainForm.cs`, `RefreshProcessList`)
- **DLL validation via LoadLibrary in own process**: The `ValidateDll` method loads the DLL into ChooChoo's own process space to verify it is a valid library, then immediately unloads it. This could be problematic if the DLL has side effects in its DllMain. (`InjectionManager.cs`, lines 164-202)
- **Concurrent access to validated DLLs cache**: `_validatedDlls` dictionary is accessed without synchronization outside the `_injectionLock`, which could theoretically cause issues if validation is called from multiple threads (currently unlikely given single-threaded UI). (`InjectionManager.cs`, line 69)
- **Window deactivation shows resume panel**: When the form loses focus, a semi-transparent "CLICK TO RESUME" overlay appears. This is a WINE/Proton UX pattern to help users refocus the app. (`MainForm.cs`, `OnDeactivate`/`OnActivated`)

## Workflows

### Critical Workflow: Game + Trainer Launch

1. User opens ChooChoo (single instance check via Mutex)
2. UI initializes: managers created, events wired, recent files and profiles loaded, command-line args processed, app settings loaded
3. User selects Game Path via Browse dialog (or from recent files combo box)
4. User selects Trainer Path via Browse dialog (or from recent files combo box)
5. User optionally selects DLL(s) and checks "Inject" checkbox(es)
6. User optionally selects a Launch Method (default: CreateProcess via P/Invoke)
7. User clicks "Launch"
8. App launches game via selected method (`ProcessManager.LaunchProcess`)
9. If DLL injection checkboxes are checked, DLLs are injected into the launched process via `InjectionManager.InjectDll`
10. App launches trainer via the same launch method
11. Console log shows status of each operation
12. Loaded DLLs/modules list refreshes to show injected modules

### Critical Workflow: DLL Injection

1. `InjectDll` is called with a DLL path
2. DLL existence is verified on disk
3. DLL is validated: loaded into ChooChoo's own process to check validity, PE header is read to determine architecture (32/64-bit), architecture compatibility is checked
4. Process handle is obtained from `ProcessManager.GetProcessHandle()`
5. `LoadLibraryA` address is resolved via `GetProcAddress(GetModuleHandle("kernel32.dll"), "LoadLibraryA")`
6. Memory is allocated in target process via `VirtualAllocEx` (MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE)
7. DLL path bytes (ASCII) are written to allocated memory via `WriteProcessMemory`
8. Remote thread is created in target process calling `LoadLibraryA` with the allocated memory as parameter via `CreateRemoteThread`
9. Thread is waited on via `WaitForSingleObject` (5000ms timeout)
10. Exit code is checked via `GetExitCodeThread` (0 = failure, non-zero = DLL module handle)
11. Remote thread handle is closed, allocated memory is freed (in finally block)
12. Success/failure events are raised

### Critical Workflow: Profile Management

1. User configures paths (game, trainer, DLLs), checkboxes, and launch method
2. User clicks Save, enters a profile name in `ProfileInputDialog`
3. Profile is written as `Key=Value` pairs to `Profiles/{name}.profile`
4. On subsequent launch, user can select profile from dropdown and click Load
5. All paths, checkboxes, and launch method are restored from file
6. If "Auto-load last used profile" is checked, the last profile is automatically loaded on startup

### Critical Workflow: Command-Line Auto-Launch

1. ChooChoo is launched with arguments: `choochoo.exe -p "MyProfile" -autolaunch "C:\path\to\game.exe"`
2. Profile is loaded first (restores all settings)
3. Game path is set from the `-autolaunch` argument
4. After a 1-second delay (via `System.Timers.Timer`), `BtnLaunch_Click` is programmatically invoked
5. Window is minimized after launch

## Domain Model

### Key Entities

- **ProcessManager** (`Core/ProcessManager.cs`): Manages the lifecycle of external processes. Responsibilities include launching processes via six different methods, attaching to existing processes by PID, suspending/resuming all threads of a process, killing processes, creating mini-dumps, and enumerating process modules and threads. Holds a single `_process` reference and `_processHandle`. **Win32 dependencies**: `OpenProcess`, `CloseHandle`, `CreateRemoteThread`, `WriteProcessMemory`, `VirtualAllocEx`, `VirtualFreeEx`, `OpenThread`, `SuspendThread`, `ResumeThread`, `CreateProcess`, `MiniDumpWriteDump` (Dbghelp.dll). Also uses `System.Diagnostics.Process` (.NET BCL).

- **InjectionManager** (`Injection/InjectionManager.cs`): Handles DLL injection into target processes. Supports standard injection (LoadLibraryA via CreateRemoteThread) and a placeholder for manual mapping. Includes DLL validation (architecture check via PE header parsing), a monitoring timer for auto-injection, and a validation cache. Takes a `ProcessManager` dependency. **Win32 dependencies**: `OpenProcess`, `GetProcAddress`, `GetModuleHandle`, `VirtualAllocEx`, `VirtualFreeEx`, `WriteProcessMemory`, `CreateRemoteThread`, `CloseHandle`, `LoadLibrary`, `FreeLibrary`, `WaitForSingleObject`, `GetExitCodeThread`.

- **MemoryManager** (`Memory/MemoryManager.cs`): Reads and writes process memory, queries memory regions, and supports saving/restoring memory state to/from files. Takes a `ProcessManager` dependency. **Win32 dependencies**: `ReadProcessMemory`, `WriteProcessMemory`, `VirtualQueryEx`. Note: The memory save/restore functionality is wired up to events in MainForm but does not appear to be directly exposed in the current UI. It may be used by trainers or for future features.

- **MainForm** (`Forms/MainForm.cs`): The monolithic WinForms UI (~2800 lines). Contains all UI layout (programmatic, not designer-generated beyond basic form setup), event handling, profile management, settings persistence, command-line argument processing, and coordinates ProcessManager, InjectionManager, and MemoryManager. Uses `System.Windows.Forms`, `System.Drawing`, and `System.IO`.

- **ResumePanel** (`UI/ResumePanel.cs`): A custom `Panel` subclass that displays a semi-transparent "CLICK TO RESUME" overlay when the app loses focus. Uses GDI+ drawing (`System.Drawing`). This addresses a Proton/WINE UX issue where users may lose track of the app window.

- **ProfileInputDialog** (nested class in `MainForm.cs`): A simple modal dialog for entering profile names. WinForms `Form` subclass.

### Component Dependencies

```
Program.cs
  -> MainForm (creates and runs)
    -> ProcessManager (no external dependencies)
    -> InjectionManager (depends on ProcessManager)
    -> MemoryManager (depends on ProcessManager)
    -> ResumePanel (standalone UI component)
```

All three manager classes communicate with MainForm via C# events (EventHandler<T>). MainForm subscribes to events from all managers and updates the UI accordingly (using `InvokeRequired`/`Invoke` for thread safety).

## Existing Codebase Integration

### .NET Framework Dependencies

**Framework-Specific (requires migration work)**:

- **Project format**: Classic `.csproj` with `ToolsVersion="15.0"` and MSBuild XML imports. Must be converted to SDK-style project format.
- **AssemblyInfo.cs**: Contains `[assembly:]` attributes that would be auto-generated in SDK-style projects.
- **`packages.config`**: References `SharpDX 4.2.0` (targetFramework="net48"). SharpDX is archived/unmaintained. This dependency appears unused in the current source code (no `using SharpDX` found) -- likely a remnant from removed TV Mode/controller support.
- **References**: Classic `<Reference Include="System.Deployment" />` and others that become implicit in modern .NET.

**Portable (.NET Standard / .NET 8+ compatible)**:

- `System.Threading.Mutex` -- available in .NET 8+
- `System.Diagnostics.Process` -- available in .NET 8+
- `System.IO` (FileStream, BinaryReader/Writer, StreamWriter, File, Directory, Path) -- available in .NET 8+
- `System.Runtime.InteropServices` (DllImport, StructLayout, Marshal) -- fully supported in .NET 8+
- `System.Timers.Timer` -- available in .NET 8+
- `System.ComponentModel` -- available in .NET 8+
- INI-style file parsing (custom, no third-party library) -- portable
- PE header parsing (custom binary reading) -- portable

**WinForms (Windows-only in .NET 8+)**:

- `System.Windows.Forms` -- supported on .NET 8+ but **Windows-only** (requires `<UseWindowsForms>true</UseWindowsForms>` in project file)
- `System.Drawing` -- supported on .NET 8+ via `System.Drawing.Common` but **Windows-only** on .NET 7+
- All form controls (TabControl, ComboBox, Button, Panel, etc.)
- `OpenFileDialog`, `MessageBox`
- GDI+ drawing in ResumePanel

### P/Invoke Inventory

All P/Invoke calls target Windows DLLs that WINE implements. These will work identically under .NET 8+ since P/Invoke is a core CLR feature.

| API Function          | Source DLL   | Used In                                         | Purpose                                           | Migration Status                       |
| --------------------- | ------------ | ----------------------------------------------- | ------------------------------------------------- | -------------------------------------- |
| `OpenProcess`         | kernel32.dll | ProcessManager, InjectionManager                | Open handle to target process                     | Compatible - P/Invoke works in .NET 8+ |
| `CloseHandle`         | kernel32.dll | ProcessManager, InjectionManager                | Close Win32 handles                               | Compatible                             |
| `CreateRemoteThread`  | kernel32.dll | ProcessManager, InjectionManager                | Create thread in remote process for DLL injection | Compatible                             |
| `WriteProcessMemory`  | kernel32.dll | ProcessManager, InjectionManager, MemoryManager | Write bytes to remote process memory              | Compatible                             |
| `VirtualAllocEx`      | kernel32.dll | ProcessManager, InjectionManager                | Allocate memory in remote process                 | Compatible                             |
| `VirtualFreeEx`       | kernel32.dll | ProcessManager, InjectionManager                | Free memory in remote process                     | Compatible                             |
| `OpenThread`          | kernel32.dll | ProcessManager                                  | Open handle to thread for suspend/resume          | Compatible                             |
| `SuspendThread`       | kernel32.dll | ProcessManager                                  | Suspend a thread                                  | Compatible                             |
| `ResumeThread`        | kernel32.dll | ProcessManager                                  | Resume a suspended thread                         | Compatible                             |
| `CreateProcess`       | kernel32.dll | ProcessManager                                  | Launch a new process                              | Compatible                             |
| `GetProcAddress`      | kernel32.dll | InjectionManager                                | Get function address in a module                  | Compatible                             |
| `GetModuleHandle`     | kernel32.dll | InjectionManager                                | Get handle to loaded module                       | Compatible                             |
| `LoadLibrary`         | kernel32.dll | InjectionManager                                | Load DLL for validation                           | Compatible                             |
| `FreeLibrary`         | kernel32.dll | InjectionManager                                | Unload validated DLL                              | Compatible                             |
| `WaitForSingleObject` | kernel32.dll | InjectionManager                                | Wait for remote thread completion                 | Compatible                             |
| `GetExitCodeThread`   | kernel32.dll | InjectionManager                                | Get thread return value                           | Compatible                             |
| `ReadProcessMemory`   | kernel32.dll | MemoryManager                                   | Read bytes from remote process memory             | Compatible                             |
| `VirtualQueryEx`      | kernel32.dll | MemoryManager                                   | Query memory region information                   | Compatible                             |
| `MiniDumpWriteDump`   | Dbghelp.dll  | ProcessManager                                  | Create process mini-dump file                     | Compatible                             |

**Total**: 18 unique Win32 API functions across 2 DLLs (`kernel32.dll`, `Dbghelp.dll`). All are fully supported via P/Invoke in .NET 8+ and implemented by WINE.

### Patterns to Follow

- **Event-driven communication**: All inter-component communication uses `EventHandler<T>` with dedicated `EventArgs` subclasses. This pattern should be preserved.
- **Thread-safe UI updates**: `InvokeRequired`/`Invoke` pattern for cross-thread UI access. This remains the same in .NET 8+ WinForms.
- **P/Invoke organization**: Win32 API declarations are grouped in `#region Win32 API` blocks with associated constants and structs. This pattern is clean and should be preserved (or optionally consolidated into a shared `NativeMethods` class).
- **Namespace layering**: `ChooChooEngine.App.{Layer}` (Core, Injection, Memory, Forms, UI). This should be preserved.
- **Field naming**: `_camelCase` for private fields, `UPPER_SNAKE_CASE` for Win32 constants.
- **File-based configuration**: Simple INI-style files for settings, profiles, and recent files. No database, no JSON, no XML config. This is appropriate for a portable single-exe tool.

## Cross-Platform Feasibility Assessment

### Can ChooChoo Become a Native Linux App?

**No.** The application's core functionality is intrinsically Windows-specific:

1. **DLL injection** (LoadLibraryA, CreateRemoteThread, VirtualAllocEx, WriteProcessMemory) has no Linux equivalent. Linux process injection uses `ptrace` and `dlopen`, which are completely different APIs.
2. **Process memory manipulation** (ReadProcessMemory, WriteProcessMemory, VirtualQueryEx) would require `/proc/{pid}/mem` on Linux, a different paradigm.
3. **Process/thread control** (SuspendThread, ResumeThread, CreateProcess with Win32 structs) requires Linux-specific alternatives (signals, ptrace).
4. **The target processes are Windows games running under WINE/Proton.** Even if ChooChoo ran natively on Linux, it would need to inject into WINE process spaces, which would require operating through WINE's implementation of these APIs anyway.

**The correct architecture remains**: ChooChoo runs as a Windows binary under Proton/WINE, where it can use the Windows APIs that WINE implements to interact with other Windows binaries (games, trainers) running in the same WINE prefix.

### What Modern .NET Enables

1. **Self-contained deployment**: Publish a single-file executable that bundles the .NET runtime. Users no longer need .NET Framework 4.8 installed in their WINE bottle.
2. **Smaller WINE requirement**: No need to install `wine-mono` or .NET Framework redistributables. The self-contained binary includes everything it needs.
3. **Modern C# features**: Pattern matching, records, nullable reference types, file-scoped namespaces, global usings, etc.
4. **SDK-style project**: Simpler `.csproj`, `dotnet build`/`dotnet publish` CLI, better NuGet integration.
5. **AOT compilation potential**: NativeAOT could produce a true native Windows binary with faster startup and smaller size (though WinForms compatibility with NativeAOT is limited).
6. **Better diagnostics**: Modern .NET has improved debugging, profiling, and error reporting.

## Success Criteria

- [ ] Project compiles and builds with `dotnet build` targeting .NET 8 or .NET 9
- [ ] SDK-style `.csproj` replaces classic project format
- [ ] WinForms UI renders and functions identically to the .NET Framework 4.8 build
- [ ] All 18 P/Invoke functions work correctly (tested under WINE/Proton)
- [ ] DLL injection workflow succeeds end-to-end on a real game under Proton
- [ ] Profile save/load/delete works correctly
- [ ] Recent files persistence works correctly
- [ ] Command-line arguments (-p, -autolaunch) work correctly
- [ ] Single-instance enforcement via Mutex works under WINE
- [ ] Self-contained publish produces a single executable that runs without pre-installed .NET in the WINE bottle
- [ ] Unused `SharpDX` dependency is removed
- [ ] No functional regression compared to the .NET Framework 4.8 build

## Open Questions

- Should the migration target .NET 8 (LTS, support until Nov 2026) or .NET 9 (STS, support until May 2026) or wait for .NET 10 (LTS, expected Nov 2025)?
- Is the `SharpDX` NuGet dependency actually used anywhere, or can it be safely removed? (Analysis suggests it is unused -- likely a remnant of removed controller/TV Mode support.)
- Should the P/Invoke declarations be consolidated into a shared `NativeMethods` static class to reduce duplication across ProcessManager and InjectionManager (several functions like `OpenProcess`, `CloseHandle`, `WriteProcessMemory`, `VirtualAllocEx`, `VirtualFreeEx`, `CreateRemoteThread` are declared in both)?
- Should the `MemoryManager.SaveMemoryState`/`RestoreMemoryState` functionality be preserved, simplified, or removed? It is wired to events but not exposed in the current UI.
- Should file-based INI configuration be replaced with JSON or kept as-is for simplicity?
- What is the minimum Proton/WINE version that should be supported with the modernized binary?
- Should the stub launch methods (CreateThreadInjection, RemoteThreadInjection) and ManualMapping injection be removed, left as stubs, or implemented?
- Should `AllowUnsafeBlocks` remain enabled? The current code does not appear to use `unsafe` blocks directly, though it is enabled in the project.

## Relevant Files

- `src/ChooChooEngine.App/Program.cs`: Entry point, single-instance Mutex enforcement
- `src/ChooChooEngine.App/Core/ProcessManager.cs`: Process lifecycle management, 11 P/Invoke declarations
- `src/ChooChooEngine.App/Injection/InjectionManager.cs`: DLL injection engine, 13 P/Invoke declarations, core injection algorithm
- `src/ChooChooEngine.App/Memory/MemoryManager.cs`: Process memory read/write/query, 3 P/Invoke declarations
- `src/ChooChooEngine.App/Forms/MainForm.cs`: Monolithic UI (~2800 lines), all workflows, profile/settings management
- `src/ChooChooEngine.App/Forms/MainForm.Designer.cs`: Minimal designer code (form dimensions and basic setup only)
- `src/ChooChooEngine.App/UI/ResumePanel.cs`: Custom overlay panel for focus recovery
- `src/ChooChooEngine.App/Properties/AssemblyInfo.cs`: Assembly metadata (will be replaced by project properties in SDK-style)
- `src/ChooChooEngine.App/ChooChooEngine.App.csproj`: Classic .NET Framework 4.8 project file (must be converted)
- `src/ChooChooEngine.sln`: Solution file
- `src/ChooChooEngine.App/packages.config`: NuGet references (SharpDX 4.2.0, likely unused)
