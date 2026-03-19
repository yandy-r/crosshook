# Pattern Research: dotnet-migrate

> Background pattern analysis. The active migration decisions are frozen in `feature-spec.md` and `parallel-plan.md`.

This document catalogs the coding patterns, conventions, and architectural decisions in the CrossHook Loader codebase that are relevant to the .NET Framework 4.8 to .NET 9 migration. The codebase consists of 6 C# source files (~2800 lines in MainForm.cs, ~500 in ProcessManager.cs, ~350 in InjectionManager.cs, ~370 in MemoryManager.cs, ~110 in ResumePanel.cs, ~45 in Program.cs) with heavy Win32 P/Invoke usage, an event-driven manager architecture, and a monolithic WinForms UI.

## Relevant Files

- `/src/CrossHookkEngine.App/Program.cs`: Application entry point with single-instance Mutex enforcement
- `/src/CrossHookkEngine.App/Core/ProcessManager.cs`: Process lifecycle manager with 12 P/Invoke declarations, event publishing, and 6 launch method strategies
- `/src/CrossHookkEngine.App/Injection/InjectionManager.cs`: DLL injection manager with 13 P/Invoke declarations, timer-based monitoring, and thread-safe injection via lock
- `/src/CrossHookkEngine.App/Memory/MemoryManager.cs`: Process memory read/write/query with 3 P/Invoke declarations, state save/restore, and binary file serialization
- `/src/CrossHookkEngine.App/Forms/MainForm.cs`: ~2800-line monolithic WinForms form containing all UI construction, event wiring, profile management, settings persistence, command-line parsing, and application orchestration
- `/src/CrossHookkEngine.App/Forms/MainForm.Designer.cs`: Minimal designer file (only sets form properties; all controls are created programmatically in MainForm.cs)
- `/src/CrossHookkEngine.App/UI/ResumePanel.cs`: Custom Panel subclass with GDI+ rendering and IDisposable implementation
- `/src/CrossHookkEngine.App/Properties/AssemblyInfo.cs`: Classic assembly metadata (to be deleted during migration, replaced by csproj properties)
- `/src/CrossHookkEngine.App/packages.config`: Single dependency on SharpDX 4.2.0 (unused in source, to be removed)
- `/src/CrossHookkEngine.AppCrossHookokEngine.App.csproj`: Classic 75-line MSBuild project file targeting .NET Framework 4.8

## Architectural Patterns

### Manager Pattern with Dependency Chain

The core business logic is organized into three manager classes with a clear dependency hierarchy:

- `ProcessManager` is the root -- it owns the process handle and has no dependencies on other managers
- `InjectionManager` depends on `ProcessManager` (passed via constructor) to get process handles for injection
- `MemoryManager` depends on `ProcessManager` (passed via constructor) to get process handles for memory operations
- `MainForm` creates all three managers in `InitializeManagers()` and wires up their events

This chain means `ProcessManager` is the natural extraction point for testability -- it can be given an interface and mocked.

Example: `/src/CrossHookkEngine.App/Forms/MainForm.cs` lines 266-286

### Event-Driven Inter-Component Communication

Every manager exposes strongly-typed events using `EventHandler<TEventArgs>` with custom EventArgs classes defined in the same file as the manager. The pattern is consistent across all three managers:

- Events declared as public fields: `public event EventHandler<ProcessEventArgs> ProcessStarted;`
- Protected virtual `On{EventName}` methods that invoke via null-conditional: `ProcessStarted?.Invoke(this, e);`
- MainForm subscribes to all events in `InitializeManagers()` and `RegisterEventHandlers()` (note: these currently double-subscribe some events -- a bug)

Event args classes per file:

- `ProcessEventArgs` (in ProcessManager.cs) -- wraps `Process`
- `InjectionEventArgs` (in InjectionManager.cs) -- wraps `DllPath` and `Message`
- `MemoryEventArgs` (in MemoryManager.cs) -- wraps `Address`, `Size`, and `Message`

Example: `/src/CrossHookkEngine.App/Core/ProcessManager.cs` lines 118-121, 464-484

### P/Invoke Organization with Region Blocks

Every manager class that uses Win32 APIs follows this structure:

1. `#region Win32 API` at the top of the class
2. `[DllImport]` declarations (all `private static extern`)
3. `[StructLayout]` struct definitions for Win32 types
4. Constants using `UPPER_SNAKE_CASE` (e.g., `PROCESS_ALL_ACCESS`, `MEM_COMMIT`)
5. `#endregion`

Critical finding for migration: **P/Invoke declarations are heavily duplicated across files.** The following functions are declared in multiple managers:

| Function             | ProcessManager | InjectionManager | MemoryManager |
| -------------------- | -------------- | ---------------- | ------------- |
| `OpenProcess`        | Yes            | Yes              | No            |
| `CloseHandle`        | Yes            | Yes              | No            |
| `WriteProcessMemory` | Yes            | Yes              | Yes           |
| `VirtualAllocEx`     | Yes            | Yes              | No            |
| `VirtualFreeEx`      | Yes            | Yes              | No            |
| `CreateRemoteThread` | Yes            | Yes              | No            |

Constants like `PROCESS_ALL_ACCESS`, `MEM_COMMIT`, `MEM_RESERVE`, `PAGE_READWRITE` are also duplicated. This is the primary candidate for consolidation into a shared `NativeInterop/Kernel32.cs` (or similar) during migration.

Example: `/src/CrossHookkEngine.App/Core/ProcessManager.cs` lines 13-112, `/srcCrossHookokEngine.App/Injection/InjectionManager.cs` lines 15-65

### Strategy Pattern for Launch Methods

`ProcessManager.LaunchProcess()` uses a switch on the `LaunchMethod` enum to dispatch to one of 6 private launch strategy methods. The enum and `ProcessEventArgs` are defined in the same file, outside the class but inside the namespace.

Two of the strategies (`LaunchWithCreateThreadInjection`, `LaunchWithRemoteThreadInjection`) are placeholder stubs that delegate to `LaunchWithCreateProcess`.

Example: `/src/CrossHookkEngine.App/Core/ProcessManager.cs` lines 131-164, 365-460, 487-495

### Monolithic Form as Application Controller

`MainForm.cs` serves as the application's composition root and controller. It contains:

1. All UI control creation (programmatic, not designer-generated)
2. All event handler wiring
3. Profile CRUD operations (INI-style flat file I/O)
4. Settings persistence (INI-style flat file I/O)
5. Command-line argument parsing
6. Recent file management
7. Process list management
8. A nested `ProfileInputDialog` class (inner class inside MainForm)

This is the primary target for service extraction during migration. Logical extraction boundaries:

- **ProfileService**: `LoadProfiles()`, `SaveProfile()`, `LoadProfile()`, `DeleteProfile()` -- all use `Application.StartupPath + "/Profiles/"` and `.profile` file format
- **SettingsService**: `SaveAppSettings()`, `LoadAppSettings()`, `SaveRecentFiles()`, `LoadRecentFiles()` -- all use `Application.StartupPath` and INI-style parsing
- **CommandLineService**: `ProcessCommandLineArguments()` -- parses `-p` and `-autolaunch` flags

Example: `/src/CrossHookkEngine.App/Forms/MainForm.cs` lines 1586-1751 (profile management), 2602-2701 (CLI parsing), 2703-2788 (settings)

## Code Conventions

### Naming Conventions

- **Namespaces**: `CrossHookkEngine.App.{Layer}` where Layer is `Core`, `Injection`, `Memory`, `Forms`, `UI`
- **Private fields**: `_camelCase` with underscore prefix (e.g., `_processManager`, `_processHandle`, `_processHandleOpen`)
- **Private UI fields**: No underscore prefix for controls declared at field level (e.g., `tabControl`, `btnLaunch`, `cmbProfiles`, `radCreateProcess`). This is inconsistent with the underscore convention used for non-UI fields.
- **Win32 constants**: `UPPER_SNAKE_CASE` (e.g., `PROCESS_ALL_ACCESS`, `MEM_COMMIT`, `PAGE_READWRITE`)
- **Win32 structs**: `UPPER_SNAKE_CASE` matching Windows SDK names (e.g., `STARTUPINFO`, `PROCESS_INFORMATION`, `MEMORY_BASIC_INFORMATION`)
- **Event handler methods**: `{Source}_{EventName}` pattern (e.g., `ProcessManager_ProcessStarted`, `BtnLaunch_Click`)
- **Event raiser methods**: `On{EventName}` pattern (e.g., `OnProcessStarted`, `OnInjectionFailed`)
- **UI helper methods**: Descriptive verbs (e.g., `RefreshProcessList`, `LogToConsole`, `UpdateStatus`)
- **Enums**: PascalCase values (e.g., `LaunchMethod.CreateProcess`, `InjectionMethod.StandardInjection`)
- **Constants (non-Win32)**: `UPPER_SNAKE_CASE` for UI constants too (e.g., `DEFAULT_TEXT`, `MutexName` is an exception using PascalCase)
- **Local functions**: camelCase (e.g., used inside `ConfigureUILayout` as `SetActiveNavButton`, `ShowPanel`)

### File Organization

- One primary class per file, with related enums and EventArgs classes at the bottom of the same file, inside the same namespace block but outside the primary class
- `#region` blocks used to organize sections within large classes (Win32 API, Launch Methods, Event Methods, Event Handlers, Helper Methods, UI Control Event Handlers)
- Using statements are explicit (no global usings) and ordered: System namespaces first, then project namespaces
- No file-scoped namespaces (uses block-scoped `namespace { }`)
- The empty `Utils/` folder is referenced in the csproj but contains no files

### Style Conventions

- Braces on new lines (Allman style)
- No expression-bodied members except for simple property getters (`public Process CurrentProcess => _process;`)
- Null checks use `== null` rather than `is null`
- String formatting uses interpolated strings (`$"Error: {ex.Message}"`)
- Collection initialization uses `new List<T>()` rather than collection expressions
- Object initializer syntax used for `ProcessStartInfo` but not consistently elsewhere
- `var` used sparingly (mostly in LINQ/Process operations in MainForm.cs), explicit types preferred in manager classes
- Single-line if statements without braces used for early returns: `if (_process == null) return false;`

## Error Handling

### Two-Tier Error Pattern

The codebase uses two distinct error handling approaches depending on the layer:

**Manager Layer (ProcessManager, InjectionManager, MemoryManager):**

- Methods return `bool` for success/failure or `null` for failed data retrieval
- Exceptions are caught with `catch (Exception ex)` (always catching the base `Exception` type)
- Errors are logged to `Debug.WriteLine($"Error {operation}: {ex.Message}")`
- Events are fired for both success and failure cases (`OnInjectionSucceeded` / `OnInjectionFailed`)
- No exceptions are ever rethrown from manager methods
- Guard clauses check preconditions and return false/null/empty collection before attempting operations

**UI Layer (MainForm):**

- Methods use `try/catch (Exception ex)` blocks
- Errors are logged via `LogToConsole($"Error {operation}: {ex.Message}")`
- User-facing errors use `MessageBox.Show()` with appropriate icons
- Some methods have conditional error display (`if (!_autoLaunchRequested)` -- suppress MessageBox during auto-launch)

**Summary**: No custom exception types are defined anywhere. No structured logging framework is used. The error reporting channel is:

- Manager classes: `Debug.WriteLine` + events
- MainForm: `LogToConsole` (appends to TextBox) + `MessageBox.Show` + `UpdateStatus` (StatusStrip label)

### InvokeRequired Pattern for Cross-Thread UI Updates

`UpdateStatus()` and `LogToConsole()` in MainForm check `InvokeRequired` and call `Invoke(new Action<string>(...))` to marshal to the UI thread. This is necessary because manager events can fire from timer threads (e.g., `InjectionManager._monitoringTimer`). The auto-launch timer also uses `BeginInvoke` for the same reason.

Example: `/src/CrossHookkEngine.App/Forms/MainForm.cs` lines 1804-1841

### Guard Clause Pattern

All manager methods follow a consistent guard clause pattern:

```
if (_process == null || _process.HasExited)
    return false;  // or return new List<T>(), or return null
```

This pattern is used before any Win32 API call to avoid passing invalid handles.

### Resource Cleanup

- `ResumePanel` implements proper `Dispose(bool)` pattern for GDI+ resources (Font, Brush, StringFormat)
- `MainForm.OnFormClosing` manually stops monitoring and detaches from process
- `ProcessManager.CloseProcessHandle()` explicitly calls `CloseHandle` on the Win32 handle
- `OpenFileDialog` instances are used with `using` statements
- `FileStream`/`BinaryWriter`/`BinaryReader` are used with `using` statements
- No `IDisposable` implementation on `ProcessManager`, `InjectionManager`, or `MemoryManager` -- process handles may leak if the form is not closed properly

## Testing Approach

### Current State

- **No test framework is configured.** No test project exists in the solution.
- **No unit tests, integration tests, or any automated tests exist.**
- `AllowUnsafeBlocks` is enabled in the csproj.
- The single NuGet dependency (SharpDX 4.2.0) is not referenced in any source file and can be removed.

### Testability Barriers

1. **No interfaces**: Manager classes are concrete classes with no interface abstractions. `InjectionManager` and `MemoryManager` take `ProcessManager` as a concrete dependency.
2. **Static P/Invoke methods**: All Win32 API calls are `private static extern` inside each manager class, making them impossible to mock without wrapping.
3. **Monolithic MainForm**: Business logic (profiles, settings, CLI parsing) is embedded in the form class and tightly coupled to UI controls.
4. **File system coupling**: Profile/settings operations use `File.ReadAllLines`, `StreamWriter`, and `Application.StartupPath` directly with no abstraction.
5. **Process coupling**: `RefreshProcessList()` calls `Process.GetProcesses()` directly.

### Patterns to Enable Testing

1. **Extract interfaces for managers**: `IProcessManager`, `IInjectionManager`, `IMemoryManager` would allow mocking the manager dependencies.
2. **Centralize P/Invoke into a static NativeInterop class**: A shared `NativeMethods` or `Kernel32` class would consolidate the 26 duplicate P/Invoke declarations and could be wrapped behind an interface for testing.
3. **Extract services from MainForm**: `ProfileService`, `SettingsService`, `CommandLineParser` classes with interfaces would make business logic testable independent of WinForms.
4. **Use `IFileSystem` abstraction**: For profile/settings I/O, `System.IO.Abstractions` (or a minimal custom interface) would allow testing file operations without touching disk.
5. **Recommended test framework**: xUnit + NSubstitute or Moq, following modern .NET conventions. MSTest is also viable.

### What Can Be Tested Without Refactoring

- `IsDll64Bit()` (PE header parsing) -- takes a file path, returns bool. Could be tested with sample DLL files.
- `ExtractProcessId()` -- pure string parsing. Trivially testable.
- `ProcessCommandLineArguments()` -- depends on form state but the parsing logic could be extracted.
- Profile file format parsing/writing -- the `Key=Value` format is simple and testable if extracted.

## Patterns to Follow

### For P/Invoke Migration (DllImport to LibraryImport)

1. **Consolidate duplicate declarations** into a shared `NativeInterop/Kernel32.cs` (and `NativeInterop/Dbghelp.cs`) using `internal static partial class`.
2. **Convert `[DllImport]` to `[LibraryImport]`** with source generation. Key changes:
   - Class must be `partial`
   - Methods must be `static partial` (not `extern`)
   - `string` parameters need explicit `StringMarshalling = StringMarshalling.Utf16` (or use `Utf8` where ANSI was used with `LoadLibraryA`)
   - `bool` parameters need `[MarshalAs(UnmanagedType.Bool)]`
   - `out` struct parameters work the same way
3. **Preserve ANSI vs Unicode intent**: `InjectDllStandard` deliberately uses `LoadLibraryA` (ANSI) and `Encoding.ASCII.GetBytes`. This is intentional for WINE compatibility and must be preserved.
4. **Keep constants co-located**: Win32 constants can move to the consolidated native interop class but should remain as `private const` or `internal const`.

### For MainForm Service Extraction

1. **Preserve the event-driven pattern**: Extracted services should continue to fire events for status updates rather than returning result objects, to maintain the existing wiring pattern.
2. **Use constructor injection**: Follow the existing pattern where `InjectionManager(ProcessManager)` takes its dependency via constructor.
3. **INI file format must be preserved**: Existing `.profile` and `settings.ini` files use a simple `Key=Value` format (not true INI with section headers for profiles). The settings.ini for recent files does use `[Section]` headers. Both formats must continue to be readable.
4. **Nested class extraction**: `ProfileInputDialog` should be extracted to its own file under `Forms/` or `UI/`.

### For Modern C# Conventions

1. **File-scoped namespaces**: Convert `namespace CrossHookkEngine.App.Core { ... }` to `namespaceCrossHookokEngine.App.Core;`
2. **Nullable reference types**: Enable `<Nullable>enable</Nullable>`. Many fields initialized to `null` or `string.Empty` will need annotation.
3. **Primary constructors or init-only properties**: `EventArgs` classes are good candidates.
4. **Collection expressions**: Replace `new List<T>()` with `[]` where appropriate.
5. **Pattern matching**: Replace `obj as Type` + null check with `obj is Type typed`.
6. **Target-typed new**: Replace `new SolidBrush(Color.White)` with `new(Color.White)` where type is obvious from context.
7. **Global usings**: Common namespaces (`System`, `System.Diagnostics`, etc.) can use `<ImplicitUsings>enable</ImplicitUsings>`.

## Edge Cases and Gotchas

- **Double event subscription bug**: `InitializeManagers()` subscribes to all manager events, then `RegisterEventHandlers()` subscribes to the same events again. This causes every event handler to fire twice. Must be fixed during migration.
- **ProcessManager does not implement IDisposable**: The `_processHandle` (Win32 handle from `OpenProcess`) is only closed in `CloseProcessHandle()` which is called from `DetachFromProcess()`. If the application crashes or `DetachFromProcess()` is not called, the handle leaks. Consider implementing `IDisposable`.
- **InjectionManager uses both `System.Timers.Timer` and `System.Windows.Forms.Timer`**: The `_monitoringTimer` is a `System.Timers.Timer` (fires on ThreadPool), while `MainForm.resizeTimer` is a `System.Windows.Forms.Timer` (fires on UI thread). The events from `_monitoringTimer` require `InvokeRequired` marshaling.
- **LaunchWithCmd has a race condition**: After launching cmd.exe and waiting for it to exit, it calls `Process.GetProcessesByName()` to find the launched process. This is inherently racy and may pick up the wrong process.
- **Unused code**: The `Utils/` folder is declared in the csproj but is empty. The `PopulateControls()` method in MainForm appears to never be called (dead code). SharpDX 4.2.0 in packages.config is not referenced in any source file.
- **ANSI string encoding for DLL injection**: `InjectDllStandard` uses `Encoding.ASCII.GetBytes` and calls `LoadLibraryA`. This intentionally uses ANSI encoding. If migrating to `LibraryImport` with Unicode marshalling, the injection path must continue to use `LoadLibraryA` (not `LoadLibraryW`) since the remote process may expect ANSI.
- **Mixed access modifier on Win32 constants**: All constants are `private const` but some are duplicated across classes with identical values. After consolidation, they should be `internal const`.
- **PE header parsing in IsDll64Bit**: The method reads the COFF characteristics at an offset calculated as `peHeaderOffset + 24 (PE sig + COFF header) + 18 (characteristics offset)` but the code does `fs.Position += 20` after reading the 4-byte PE signature, which lands at the Machine field, not the characteristics. However, the code then reads `br.ReadUInt16()` which reads 2 bytes. The offset arithmetic should be verified against the PE spec during migration.
- **No defensive copy of \_args**: The `_args` array reference is stored directly from the constructor parameter. While not a practical issue, modern C# would prefer a defensive copy or `ReadOnlySpan`.

## Other Docs

- `/docs/plans/dotnet-migrate/research-technical.md`: Detailed SDK-style csproj conversion spec, LibraryImport conversion patterns, and publish configuration
- `/docs/plans/dotnet-migrate/feature-spec.md`: Business requirements, user stories, edge case matrix, and external dependency documentation
- `/docs/plans/dotnet-migrate/research-external.md`: External documentation and migration guides
- `/docs/plans/dotnet-migrate/research-recommendations.md`: Recommended approach and tooling decisions
- [Microsoft: P/Invoke Source Generation](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation)
- [Microsoft: Upgrade WinForms to .NET](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/)
- [Microsoft: Single-File Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)

# Note

This file remains useful for code-shape observations and interop patterns, but current migration decisions are frozen in `feature-spec.md` and `parallel-plan.md`.
