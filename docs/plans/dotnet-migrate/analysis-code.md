# Code Analysis: dotnet-migrate

## Executive Summary

ChooChoo Loader is a 4,264-line C# WinForms application organized into three manager classes (ProcessManager, InjectionManager, MemoryManager) that handle Win32 P/Invoke operations, coordinated by a 2,800-line monolithic MainForm that owns all UI construction, state management, profile persistence, settings, and command-line parsing. The migration to .NET 9 requires three categories of code changes: (1) mechanical DllImport-to-LibraryImport conversion across 29 declaration sites with `partial` keyword additions, (2) SDK-style csproj rewrite with file deletions (AssemblyInfo.cs, packages.config), and (3) optional but recommended service extraction from MainForm to improve testability.

---

## Existing Code Structure

### Related Components

- `/src/ChooChooEngine.App/ChooChooEngine.App.csproj`: Classic 75-line .NET Framework 4.8 project file with explicit `<Compile Include>` items, `<Reference>` entries, and `AllowUnsafeBlocks=true`. Complete rewrite target.
- `/src/ChooChooEngine.sln`: Solution file using legacy project type GUID `{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}`. May need update for SDK-style.
- `/src/ChooChooEngine.App/Program.cs`: 44-line entry point with `Mutex` single-instance enforcement and `Application.Run(new MainForm(args))`. Needs `partial` keyword if it will contain LibraryImport (unlikely, but required if moved).
- `/src/ChooChooEngine.App/Core/ProcessManager.cs`: 505-line process lifecycle manager. 11 P/Invoke declarations, 2 Win32 structs (STARTUPINFO, PROCESS_INFORMATION), 13 Win32 constants, 6 launch strategies, event system. Primary P/Invoke migration target.
- `/src/ChooChooEngine.App/Injection/InjectionManager.cs`: 353-line DLL injection engine. 12 P/Invoke declarations (6 duplicated from ProcessManager, plus GetProcAddress, GetModuleHandle, LoadLibrary, FreeLibrary, WaitForSingleObject, GetExitCodeThread), timer-based monitoring, thread-safe lock. Critical ANSI encoding gotcha in injection path.
- `/src/ChooChooEngine.App/Memory/MemoryManager.cs`: 368-line memory manager. 3 P/Invoke declarations (ReadProcessMemory, WriteProcessMemory, VirtualQueryEx), 1 Win32 struct (MEMORY_BASIC_INFORMATION), binary state save/restore. Cleanest migration target.
- `/src/ChooChooEngine.App/Forms/MainForm.cs`: 2,800-line monolith. All UI construction (~1,400 lines in `ConfigureUILayout()`), all event wiring, profile CRUD, settings I/O, CLI parsing, nested `ProfileInputDialog` class. Service extraction target.
- `/src/ChooChooEngine.App/Forms/MainForm.Designer.cs`: 52-line minimal designer file. Only sets form dimensions, colors, and name. All real UI is programmatic. No changes needed.
- `/src/ChooChooEngine.App/UI/ResumePanel.cs`: 107-line custom Panel with proper `Dispose(bool)` for GDI+ resources. No changes needed. Reference implementation for IDisposable quality.
- `/src/ChooChooEngine.App/Properties/AssemblyInfo.cs`: 36-line assembly metadata. DELETE -- replaced by csproj `<PropertyGroup>` properties.
- `/src/ChooChooEngine.App/packages.config`: Single SharpDX 4.2.0 reference. DELETE -- SharpDX is not referenced in any source file.

### File Organization Pattern

```
src/ChooChooEngine.App/
  ChooChooEngine.App.csproj     # Project definition
  Program.cs                    # Entry point (namespace: ChooChooEngine.App)
  Core/ProcessManager.cs        # namespace: ChooChooEngine.App.Core
  Injection/InjectionManager.cs # namespace: ChooChooEngine.App.Injection
  Memory/MemoryManager.cs       # namespace: ChooChooEngine.App.Memory
  Forms/MainForm.cs             # namespace: ChooChooEngine.App.Forms
  Forms/MainForm.Designer.cs    # namespace: ChooChooEngine.App.Forms (partial)
  UI/ResumePanel.cs             # namespace: ChooChooEngine.App.UI
  Properties/AssemblyInfo.cs    # (to be deleted)
  Utils/                        # Empty folder, referenced in csproj
```

Namespace convention: `ChooChooEngine.App.{Layer}` where Layer maps 1:1 to folder name. One primary class per file, with related enums and EventArgs classes defined at the bottom of the same file (outside the class, inside the namespace block).

---

## Implementation Patterns

### Pattern: P/Invoke Declaration (DllImport)

**Description**: All Win32 API calls follow a consistent structure: `[DllImport("library.dll")] private static extern ReturnType FunctionName(params);`. Declarations are grouped in `#region Win32 API` blocks at the top of each class, followed by `[StructLayout]` structs, followed by `UPPER_SNAKE_CASE` constants.

**Example**: `/src/ChooChooEngine.App/Core/ProcessManager.cs` lines 13-112

**Apply to**: Phase 3 P/Invoke migration. Every `[DllImport]` becomes `[LibraryImport]` on a `static partial` method. The class must be marked `partial`. The `extern` keyword is removed.

**Conversion template**:

```
BEFORE:  [DllImport("kernel32.dll")]
         private static extern IntPtr OpenProcess(int dwDesiredAccess, bool bInheritHandle, int dwProcessId);

AFTER:   [LibraryImport("kernel32.dll")]
         private static partial IntPtr OpenProcess(int dwDesiredAccess,
             [MarshalAs(UnmanagedType.Bool)] bool bInheritHandle, int dwProcessId);
```

### Pattern: Duplicated P/Invoke Declarations

**Description**: 6 Win32 functions and 6 constants are declared identically in both ProcessManager.cs and InjectionManager.cs. WriteProcessMemory appears in all three managers. These should be consolidated into a shared NativeInterop class during migration.

**Duplicated functions**: `OpenProcess`, `CloseHandle`, `CreateRemoteThread`, `WriteProcessMemory`, `VirtualAllocEx`, `VirtualFreeEx`

**Duplicated constants**: `PROCESS_CREATE_THREAD`, `PROCESS_QUERY_INFORMATION`, `PROCESS_VM_OPERATION`, `PROCESS_VM_WRITE`, `PROCESS_VM_READ`, `PROCESS_ALL_ACCESS`, `MEM_COMMIT`, `MEM_RESERVE`, `MEM_RELEASE`, `PAGE_READWRITE`

**Example**: `/src/ChooChooEngine.App/Core/ProcessManager.cs` lines 15-34 vs `/src/ChooChooEngine.App/Injection/InjectionManager.cs` lines 17-42

**Apply to**: P/Invoke consolidation task. Create `NativeInterop/Kernel32.cs` and `NativeInterop/Dbghelp.cs` as `internal static partial class` files.

### Pattern: Event-Driven Communication

**Description**: Each manager exposes `EventHandler<TEventArgs>` events with custom EventArgs classes. Protected virtual `On{EventName}` methods fire via null-conditional invoke. MainForm subscribes to all events and routes them to `LogToConsole()` / `UpdateStatus()`.

**Example**: `/src/ChooChooEngine.App/Core/ProcessManager.cs` lines 118-121 (event declarations), 464-484 (On\* methods)

**Event args classes**:

- `ProcessEventArgs(Process process)` -- defined in ProcessManager.cs lines 497-505
- `InjectionEventArgs(string dllPath, string message = null)` -- defined in InjectionManager.cs lines 343-353
- `MemoryEventArgs(IntPtr address, uint size, string message = null)` -- defined in MemoryManager.cs lines 356-368

**Apply to**: Service extraction -- extracted services should preserve this event pattern.

### Pattern: Constructor Injection Dependency Chain

**Description**: ProcessManager is the root dependency. InjectionManager and MemoryManager receive ProcessManager via constructor. MainForm creates all three in `InitializeManagers()`.

**Example**: `/src/ChooChooEngine.App/Forms/MainForm.cs` lines 266-286

```
_processManager = new ProcessManager();
_injectionManager = new InjectionManager(_processManager);
_memoryManager = new MemoryManager(_processManager);
```

**Apply to**: Service extraction. New services (ProfileService, SettingsService, etc.) should follow the same constructor injection pattern. Interfaces should be introduced: `IProcessManager`, etc.

### Pattern: Two-Tier Error Handling

**Description**: Manager layer catches all exceptions, logs via `Debug.WriteLine`, returns `bool`/`null`, and fires events. UI layer catches exceptions, logs via `LogToConsole()`, and shows `MessageBox`. No exceptions are ever rethrown from manager methods. No custom exception types exist.

**Example**: `/src/ChooChooEngine.App/Core/ProcessManager.cs` lines 139-163 (manager try/catch returning bool), `/src/ChooChooEngine.App/Forms/MainForm.cs` lines 2105-2226 (UI try/catch with MessageBox)

**Apply to**: Maintain this pattern during migration. Consider adding Result<T> pattern in the future but not during initial migration.

### Pattern: Guard Clauses

**Description**: All manager methods begin with guard clauses checking process state before Win32 API calls.

**Example**: `/src/ChooChooEngine.App/Core/ProcessManager.cs` lines 193-195

```
if (_process == null || _process.HasExited)
    return false;
```

**Apply to**: Preserve during migration. With nullable reference types enabled, these will align with the compiler's null analysis.

### Pattern: INI-Style File Persistence

**Description**: Three hand-rolled file formats using `Key=Value` pairs and `[Section]` headers. All paths are relative to `Application.StartupPath`.

**Formats**:

1. Profiles (`.profile`): Simple `Key=Value` per line, no sections. Keys: `GamePath`, `TrainerPath`, `Dll1Path`, `Dll2Path`, `LaunchInject1`, `LaunchInject2`, `LaunchMethod`.
2. Recent files (`settings.ini`): Section headers `[RecentGamePaths]`, `[RecentTrainerPaths]`, `[RecentDllPaths]` with one path per line.
3. App settings (`Settings/AppSettings.ini`): `Key=Value` per line. Keys: `AutoLoadLastProfile`, `LastUsedProfile`.

**Example**: `/src/ChooChooEngine.App/Forms/MainForm.cs` lines 1586-1751 (profile CRUD), 1483-1584 (recent files), 2703-2788 (app settings)

**Apply to**: Service extraction candidates -- `ProfileService`, `RecentFilesService`, `SettingsService`.

### Pattern: Cross-Thread UI Marshaling

**Description**: `UpdateStatus()` and `LogToConsole()` check `InvokeRequired` and call `Invoke(new Action<string>(...))` to marshal to the UI thread. Required because manager events can fire from `System.Timers.Timer` thread pool threads.

**Example**: `/src/ChooChooEngine.App/Forms/MainForm.cs` lines 1804-1841

**Apply to**: Preserve during migration. This pattern is still required in .NET 9 WinForms.

### Pattern: Programmatic UI Construction

**Description**: The 52-line designer file only sets form dimensions. All ~1,400 lines of actual UI construction happen in `ConfigureUILayout()` using `TableLayoutPanel`, `Panel`, `FlowLayoutPanel`, and manual control creation with dark theme styling.

**Example**: `/src/ChooChooEngine.App/Forms/MainForm.cs` lines 494-1361

**Apply to**: No changes needed for migration. This pattern eliminates designer migration risk entirely.

---

## Integration Points

### Files to Create

- `/src/ChooChooEngine.App/NativeInterop/Kernel32.cs`: Consolidated P/Invoke declarations for kernel32.dll. `internal static partial class Kernel32` containing all 17 kernel32 function declarations, Win32 structs, and constants currently duplicated across managers.
- `/src/ChooChooEngine.App/NativeInterop/Dbghelp.cs`: P/Invoke declaration for Dbghelp.dll. `internal static partial class Dbghelp` containing `MiniDumpWriteDump` and its constants.
- `/src/global.json` (optional): Pin .NET 9 SDK version for reproducible builds.

### Files to Modify

- `/src/ChooChooEngine.App/ChooChooEngine.App.csproj`: **Complete rewrite** from 75-line classic format to ~30-line SDK-style targeting `net9.0-windows`. Add `<UseWindowsForms>true</UseWindowsForms>`, `<AllowUnsafeBlocks>true</AllowUnsafeBlocks>`, `<Nullable>enable</Nullable>`, `<ImplicitUsings>enable</ImplicitUsings>`, self-contained publish properties, and assembly metadata from AssemblyInfo.cs.
- `/src/ChooChooEngine.sln`: Project type GUID may need update from `{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}` to SDK-style equivalent, though both work with `dotnet build`.
- `/src/ChooChooEngine.App/Core/ProcessManager.cs`: (1) Add `partial` to class declaration. (2) Convert 11 `[DllImport]` to `[LibraryImport]` with `static partial` methods. (3) Add `[MarshalAs(UnmanagedType.Bool)]` to all `bool` parameters on P/Invoke methods. (4) Add `StringMarshalling = StringMarshalling.Utf16` to `CreateProcess`. (5) Remove `extern` keyword. (6) If consolidating P/Invoke: remove declarations and reference shared `Kernel32`/`Dbghelp` classes instead. (7) Optionally implement `IDisposable` for `_processHandle` cleanup.
- `/src/ChooChooEngine.App/Injection/InjectionManager.cs`: (1) Add `partial` to class declaration. (2) Convert 12 `[DllImport]` to `[LibraryImport]`. (3) Add `[MarshalAs(UnmanagedType.Bool)]` to `bool` params. (4) `LoadLibrary`: use `StringMarshalling.Utf16`. (5) `GetProcAddress`: use `StringMarshalling.Utf8`. (6) CRITICAL: The `InjectDllStandard` method at line 243 calls `GetProcAddress(GetModuleHandle("kernel32.dll"), "LoadLibraryA")` and uses `Encoding.ASCII.GetBytes` at line 248 -- this ANSI path must be preserved regardless of how `LoadLibrary` itself is marshalled. (7) Remove `extern` keyword.
- `/src/ChooChooEngine.App/Memory/MemoryManager.cs`: (1) Add `partial` to class declaration. (2) Convert 3 `[DllImport]` to `[LibraryImport]`. (3) Add `[MarshalAs(UnmanagedType.Bool)]` to `bool` params. (4) Remove `extern` keyword.
- `/src/ChooChooEngine.App/Forms/MainForm.cs`: (1) Fix double event subscription bug (lines 274-283 in `InitializeManagers()` duplicate subscriptions from lines 1370-1387 in `RegisterEventHandlers()`). (2) Optionally extract ProfileService, SettingsService, CommandLineParser. (3) Optionally extract nested `ProfileInputDialog` class to its own file.
- `/CLAUDE.md`: Update tech stack from ".NET Framework 4.8" to ".NET 9", build commands from `msbuild` to `dotnet build`, and note that `dotnet build` now works.

### Files to Delete

- `/src/ChooChooEngine.App/Properties/AssemblyInfo.cs`: Assembly metadata moves to csproj `<PropertyGroup>`.
- `/src/ChooChooEngine.App/packages.config`: SharpDX 4.2.0 is unused in source. No replacement needed.
- `/src/ChooChooEngine.App/bin/` and `/src/ChooChooEngine.App/obj/`: Clean rebuild required for framework change.

---

## Code Conventions

### Naming

- **Namespaces**: `ChooChooEngine.App.{Layer}` (Core, Injection, Memory, Forms, UI)
- **Private non-UI fields**: `_camelCase` with underscore prefix (`_processManager`, `_processHandle`, `_validatedDlls`)
- **Private UI fields**: No underscore prefix (`tabControl`, `btnLaunch`, `cmbProfiles`) -- inconsistent with non-UI convention
- **Win32 constants**: `UPPER_SNAKE_CASE` (`PROCESS_ALL_ACCESS`, `MEM_COMMIT`, `PAGE_READWRITE`)
- **Win32 structs**: `UPPER_SNAKE_CASE` matching Windows SDK names (`STARTUPINFO`, `PROCESS_INFORMATION`, `MEMORY_BASIC_INFORMATION`)
- **Event handler methods**: `{Source}_{EventName}` (`ProcessManager_ProcessStarted`, `BtnLaunch_Click`)
- **Event raiser methods**: `On{EventName}` (`OnProcessStarted`, `OnInjectionFailed`)
- **Enums**: PascalCase values (`LaunchMethod.CreateProcess`, `InjectionMethod.StandardInjection`)
- **Local functions**: PascalCase (`CreateNavButton`, `SetActiveNavButton`, `ShowPanel` -- inside `ConfigureUILayout`)

### Error Handling

- Manager layer: `try/catch(Exception ex)` -> `Debug.WriteLine` -> return `bool`/`null` -> fire event
- UI layer: `try/catch(Exception ex)` -> `LogToConsole` -> `MessageBox.Show` -> `UpdateStatus`
- No custom exception types
- No exceptions rethrown from managers
- Guard clauses at method entry: `if (_process == null || _process.HasExited) return false;`
- `InvokeRequired` check for cross-thread UI updates

### Testing

- No test framework configured
- No test project exists
- `AllowUnsafeBlocks` enabled
- Testability barriers: no interfaces, static P/Invoke methods, monolithic MainForm, direct file system access
- Immediately testable if extracted: `ExtractProcessId()` (pure string parsing), profile `Key=Value` parsing, `IsDll64Bit()` (PE header parsing)

---

## Dependencies and Services

### Current Dependencies

- **Framework References**: System, System.Core, System.Drawing, System.Windows.Forms, System.Net.Http, System.Data, System.Deployment, System.Xml (all replaced by `net9.0-windows` TFM)
- **NuGet Packages**: SharpDX 4.2.0 (unused in source -- remove)
- **Win32 DLLs (runtime)**: kernel32.dll (17 APIs), Dbghelp.dll (1 API) -- provided by WINE at runtime

### Service Dependencies (Manager Layer)

```
ProcessManager (root, no dependencies)
  |
  +-- InjectionManager (depends on ProcessManager via constructor)
  |
  +-- MemoryManager (depends on ProcessManager via constructor)
```

MainForm creates all three and wires events. `InjectionManager` and `MemoryManager` call `_processManager.GetProcessHandle()` to obtain the Win32 handle needed for their P/Invoke operations.

### Implicit Services in MainForm (Extraction Candidates)

1. **ProfileService**: `LoadProfiles()`, `SaveProfile(string)`, `LoadProfile(string)`, `DeleteProfile` logic in `BtnDelete_Click`. Uses `Application.StartupPath/Profiles/*.profile`.
2. **RecentFilesService**: `LoadRecentFiles()`, `SaveRecentFiles()`. Uses `Application.StartupPath/settings.ini`.
3. **SettingsService**: `SaveAppSettings()`, `LoadAppSettings()`. Uses `Application.StartupPath/Settings/AppSettings.ini`.
4. **CommandLineParser**: `ProcessCommandLineArguments()`. Parses `-p <profile>` and `-autolaunch <path>`.
5. **LaunchOrchestrator**: `BtnLaunch_Click` logic (lines 2105-2226). Coordinates ProcessManager launch + InjectionManager inject + trainer launch.

---

## Gotchas and Warnings

- **Double event subscription (BUG)**: `InitializeManagers()` (lines 274-283) and `RegisterEventHandlers()` (lines 1370-1387) both subscribe to the same ProcessManager, InjectionManager, and MemoryManager events. Every handler fires twice. Additionally, `btnRefreshProcesses.Click` is wired both inline at line 547 and again at line 1390. Fix: remove the subscriptions from one location.

- **ANSI encoding in injection path (CRITICAL)**: `InjectionManager.InjectDllStandard()` at line 243 resolves `LoadLibraryA` by name and at line 248 encodes the DLL path with `Encoding.ASCII.GetBytes`. This MUST remain ANSI/ASCII -- the remote process expects an ANSI string from `LoadLibraryA`. When converting `LoadLibrary` (the validation call at line 176) to `[LibraryImport]`, that one can use UTF-16. But the injection path's use of `GetProcAddress(..., "LoadLibraryA")` and `Encoding.ASCII.GetBytes` must not change.

- **ProcessManager lacks IDisposable**: The `_processHandle` (IntPtr from `OpenProcess` at line 344) is only closed via `CloseProcessHandle()` called from `DetachFromProcess()`. If the form closes without detaching, or the app crashes, the handle leaks. Consider implementing `IDisposable` with a finalizer.

- **`PopulateControls()` is dead code**: The method at lines 1413-1430 is never called from anywhere. It duplicates initialization already done in the constructor. Should be removed.

- **LaunchWithCmd race condition**: At line 404, after `cmd.exe` exits, it calls `Process.GetProcessesByName()` to find the launched process. This is inherently racy and may pick the wrong process.

- **Placeholder launch methods**: `LaunchWithCreateThreadInjection` (line 416) and `LaunchWithRemoteThreadInjection` (line 423) both fall through to `LaunchWithCreateProcess`. They are dead code stubs.

- **PE header parsing may be incorrect**: `IsDll64Bit()` in InjectionManager.cs at line 223 does `fs.Position += 20` after the 4-byte PE signature read, then reads `br.ReadUInt16()`. The offset arithmetic (PE sig 4 bytes + skip 20 bytes = offset 24 into COFF header) lands at the `Characteristics` field position, but the COFF header `Characteristics` is at offset 18 from the start of the COFF header (after 4-byte PE sig). The skip of 20 bytes puts the position at byte 24 from the COFF start, which is past the COFF header and into the Optional Header. The `IMAGE_FILE_32BIT_MACHINE` check at that position would be reading the Optional Header magic number, not the Characteristics field. This may produce incorrect results for some DLLs.

- **InjectionManager mixes timer types**: Uses `System.Timers.Timer` for `_monitoringTimer` (fires on ThreadPool thread), while MainForm uses `System.Windows.Forms.Timer` (fires on UI thread). The `System.Timers.Timer` events require `InvokeRequired` marshaling when updating UI.

- **Utils/ folder is empty**: Referenced in csproj `<Folder Include="Utils\" />` but contains no files. SDK-style csproj does not need this entry.

- **No defensive copy of args**: The `_args` constructor parameter at line 224 is stored by reference. Not a practical issue but noted for nullable analysis.

- **`string.Empty` vs nullable**: Many fields are initialized to `string.Empty` (e.g., `_lastUsedProfile`, `_selectedGamePath`). With nullable reference types enabled, these should be annotated properly.

---

## Task-Specific Guidance

### For csproj Conversion (Phase 1)

- The existing csproj has `AllowUnsafeBlocks=true` in both Debug and Release configs -- consolidate to a single `<PropertyGroup>`.
- SDK-style auto-globs all `*.cs` files, so the explicit `<Compile Include>` items are not needed.
- The `<Folder Include="Utils\" />` entry can be dropped (empty folder, SDK-style doesn't need it).
- Assembly metadata from `Properties/AssemblyInfo.cs` maps to: `AssemblyTitle` -> `<AssemblyTitle>`, `AssemblyDescription` -> `<Description>`, `AssemblyCopyright` -> `<Copyright>`, `AssemblyVersion` -> `<Version>`, `Guid` -> not needed in SDK-style, `ComVisible` -> not needed.
- The `System.Deployment` reference in the old csproj is likely unused -- SDK-style won't include it.
- Publish profile: `RuntimeIdentifier=win-x64`, `SelfContained=true`, `PublishSingleFile=true`, `IncludeNativeLibrariesForSelfExtract=true`, `EnableCompressionInSingleFile=true`.

### For P/Invoke Migration (Phase 3)

- **Total work**: 29 declaration sites across 3 files (11 in ProcessManager, 12 in InjectionManager, 3 in MemoryManager, plus 3 additional in InjectionManager's second `#region`).
- **Classes needing `partial` keyword**: `ProcessManager`, `InjectionManager`, `MemoryManager`.
- **Bool parameters needing `[MarshalAs(UnmanagedType.Bool)]`**: `OpenProcess.bInheritHandle`, `CloseHandle` (returns bool), `WriteProcessMemory` (returns bool), `VirtualFreeEx` (returns bool), `CreateProcess.bInheritHandles`, `ReadProcessMemory` (returns bool), `FreeLibrary` (returns bool), `GetExitCodeThread` (out param is uint, not bool).
- **String marshalling requirements**:
  - `CreateProcess`: `StringMarshalling.Utf16` (use `CreateProcessW` for proper WINE path handling)
  - `LoadLibrary` (validation only): `StringMarshalling.Utf16`
  - `GetProcAddress`: `StringMarshalling.Utf8` (only accepts ANSI names)
  - `GetModuleHandle`: `StringMarshalling.Utf16`
- **ANSI injection path**: The `InjectDllStandard()` method at InjectionManager.cs line 243-248 must continue to use `"LoadLibraryA"` string and `Encoding.ASCII.GetBytes`. This is the actual injection path. The `LoadLibrary` P/Invoke at line 44 is only used for local DLL validation (line 176) and can safely use UTF-16.
- **Consolidation target**: If creating `NativeInterop/Kernel32.cs`, make it `internal static partial class` and use `internal` visibility on all methods so managers can access them. Constants become `internal const`. Structs become `internal` structs.

### For Service Extraction (Phase 2)

- **ProfileService** boundary: Lines 1586-1751 in MainForm.cs. Methods: `LoadProfiles()`, `SaveProfile(string name)`, `LoadProfile(string name)`, delete logic from `BtnDelete_Click`. Dependencies: file system access to `Application.StartupPath/Profiles/`. Returns profile data as a model object, not directly manipulating UI controls. MainForm maps model to UI.
- **SettingsService** boundary: Lines 2703-2788. Methods: `SaveAppSettings()`, `LoadAppSettings()`. Dependencies: file system access to `Application.StartupPath/Settings/AppSettings.ini`. Simple `Key=Value` format with `bool` and `string` values.
- **RecentFilesService** boundary: Lines 1483-1584. Methods: `LoadRecentFiles()`, `SaveRecentFiles()`. Dependencies: file system access to `Application.StartupPath/settings.ini`. Uses `[Section]` headers.
- **CommandLineParser** boundary: Lines 2602-2701. Pure parsing logic for `-p <profile>` and `-autolaunch <path>` arguments. Returns a parsed result object. No UI dependency.
- **Double event subscription fix**: Remove the event subscriptions from `RegisterEventHandlers()` (lines 1370-1387) since they duplicate what `InitializeManagers()` already does (lines 274-285). Also remove the duplicate `btnRefreshProcesses.Click` at line 1390 (already wired at line 547).
- **ProfileInputDialog extraction**: The nested class at lines 104-209 should be moved to its own file at `Forms/ProfileInputDialog.cs` or `UI/ProfileInputDialog.cs`.

---

## Complete P/Invoke Migration Matrix

| Function              | File(s)                                                  | Current Signature                                                                                                          | LibraryImport Notes                                                                          |
| --------------------- | -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `OpenProcess`         | ProcessManager:15, InjectionManager:18                   | `IntPtr OpenProcess(int, bool, int)`                                                                                       | Add `[MarshalAs(UnmanagedType.Bool)]` to `bInheritHandle`                                    |
| `CloseHandle`         | ProcessManager:19, InjectionManager:42                   | `bool CloseHandle(IntPtr)`                                                                                                 | Return `[MarshalAs(UnmanagedType.Bool)]`                                                     |
| `CreateRemoteThread`  | ProcessManager:22, InjectionManager:38                   | `IntPtr CreateRemoteThread(IntPtr, IntPtr, uint, IntPtr, IntPtr, uint, IntPtr)`                                            | No special marshalling                                                                       |
| `WriteProcessMemory`  | ProcessManager:26, InjectionManager:34, MemoryManager:20 | `bool WriteProcessMemory(IntPtr, IntPtr, byte[], uint, out UIntPtr)`                                                       | Return `[MarshalAs(UnmanagedType.Bool)]`                                                     |
| `VirtualAllocEx`      | ProcessManager:30, InjectionManager:27                   | `IntPtr VirtualAllocEx(IntPtr, IntPtr, uint, uint, uint)`                                                                  | No special marshalling                                                                       |
| `VirtualFreeEx`       | ProcessManager:34, InjectionManager:31                   | `bool VirtualFreeEx(IntPtr, IntPtr, uint, uint)`                                                                           | Return `[MarshalAs(UnmanagedType.Bool)]`                                                     |
| `OpenThread`          | ProcessManager:37                                        | `IntPtr OpenThread(int, bool, uint)`                                                                                       | `[MarshalAs(UnmanagedType.Bool)]` on `bInheritHandle`                                        |
| `SuspendThread`       | ProcessManager:40                                        | `uint SuspendThread(IntPtr)`                                                                                               | No special marshalling                                                                       |
| `ResumeThread`        | ProcessManager:43                                        | `uint ResumeThread(IntPtr)`                                                                                                | No special marshalling                                                                       |
| `CreateProcess`       | ProcessManager:46                                        | `bool CreateProcess(string, string, IntPtr, IntPtr, bool, uint, IntPtr, string, ref STARTUPINFO, out PROCESS_INFORMATION)` | `StringMarshalling.Utf16`, `[MarshalAs(UnmanagedType.Bool)]` on return and `bInheritHandles` |
| `MiniDumpWriteDump`   | ProcessManager:52                                        | `bool MiniDumpWriteDump(IntPtr, int, IntPtr, int, IntPtr, IntPtr, IntPtr)`                                                 | Library name: `"Dbghelp.dll"`, return `[MarshalAs(UnmanagedType.Bool)]`                      |
| `GetProcAddress`      | InjectionManager:22                                      | `IntPtr GetProcAddress(IntPtr, string)`                                                                                    | `StringMarshalling.Utf8` (ANSI-only API)                                                     |
| `GetModuleHandle`     | InjectionManager:25                                      | `IntPtr GetModuleHandle(string)`                                                                                           | `StringMarshalling.Utf16`                                                                    |
| `LoadLibrary`         | InjectionManager:44                                      | `IntPtr LoadLibrary(string)`                                                                                               | `StringMarshalling.Utf16`, `SetLastError = true`                                             |
| `FreeLibrary`         | InjectionManager:48                                      | `bool FreeLibrary(IntPtr)`                                                                                                 | Return `[MarshalAs(UnmanagedType.Bool)]`, `SetLastError = true`                              |
| `WaitForSingleObject` | InjectionManager:315                                     | `uint WaitForSingleObject(IntPtr, uint)`                                                                                   | `SetLastError = true`                                                                        |
| `GetExitCodeThread`   | InjectionManager:318                                     | `bool GetExitCodeThread(IntPtr, out uint)`                                                                                 | Return `[MarshalAs(UnmanagedType.Bool)]`, `SetLastError = true`                              |
| `ReadProcessMemory`   | MemoryManager:16                                         | `bool ReadProcessMemory(IntPtr, IntPtr, byte[], uint, out UIntPtr)`                                                        | Return `[MarshalAs(UnmanagedType.Bool)]`                                                     |
| `VirtualQueryEx`      | MemoryManager:24                                         | `IntPtr VirtualQueryEx(IntPtr, IntPtr, out MEMORY_BASIC_INFORMATION, uint)`                                                | No special marshalling                                                                       |
