# Integration Research: dotnet-migrate

ChooChoo Loader's external integration surface consists of 19 unique Win32 P/Invoke declarations (29 total declaration sites across 3 files), a custom INI-style file configuration system with two distinct file formats (`.profile` and `.ini`), one unused NuGet package (SharpDX 4.2.0), and a classic .NET Framework 4.8 MSBuild project structure. All P/Invoke calls target `kernel32.dll` (18 APIs) and `Dbghelp.dll` (1 API), both of which are core WINE-implemented DLLs. The file I/O layer uses `Application.StartupPath` as the root for all configuration data, which has a critical behavioral difference under .NET 9 single-file deployment that requires migration attention.

---

## Win32 API Surface

### Complete P/Invoke Inventory

#### ProcessManager.cs -- 11 declarations

| #   | API                  | DLL          | Signature                                                                                                                                                                                                                                                                                   | CharSet / Marshalling                            | SetLastError | Line  |
| --- | -------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | ------------ | ----- |
| 1   | `OpenProcess`        | kernel32.dll | `(int dwDesiredAccess, bool bInheritHandle, int dwProcessId) -> IntPtr`                                                                                                                                                                                                                     | None (default ANSI)                              | No           | 15-16 |
| 2   | `CloseHandle`        | kernel32.dll | `(IntPtr hObject) -> bool`                                                                                                                                                                                                                                                                  | None                                             | No           | 18-19 |
| 3   | `CreateRemoteThread` | kernel32.dll | `(IntPtr hProcess, IntPtr lpThreadAttributes, uint dwStackSize, IntPtr lpStartAddress, IntPtr lpParameter, uint dwCreationFlags, IntPtr lpThreadId) -> IntPtr`                                                                                                                              | None                                             | No           | 21-23 |
| 4   | `WriteProcessMemory` | kernel32.dll | `(IntPtr hProcess, IntPtr lpBaseAddress, byte[] lpBuffer, uint nSize, out UIntPtr lpNumberOfBytesWritten) -> bool`                                                                                                                                                                          | None                                             | No           | 25-27 |
| 5   | `VirtualAllocEx`     | kernel32.dll | `(IntPtr hProcess, IntPtr lpAddress, uint dwSize, uint flAllocationType, uint flProtect) -> IntPtr`                                                                                                                                                                                         | None                                             | No           | 29-31 |
| 6   | `VirtualFreeEx`      | kernel32.dll | `(IntPtr hProcess, IntPtr lpAddress, uint dwSize, uint dwFreeType) -> bool`                                                                                                                                                                                                                 | None                                             | No           | 33-34 |
| 7   | `OpenThread`         | kernel32.dll | `(int dwDesiredAccess, bool bInheritHandle, uint dwThreadId) -> IntPtr`                                                                                                                                                                                                                     | None                                             | No           | 36-37 |
| 8   | `SuspendThread`      | kernel32.dll | `(IntPtr hThread) -> uint`                                                                                                                                                                                                                                                                  | None                                             | No           | 39-40 |
| 9   | `ResumeThread`       | kernel32.dll | `(IntPtr hThread) -> uint`                                                                                                                                                                                                                                                                  | None                                             | No           | 42-43 |
| 10  | `CreateProcess`      | kernel32.dll | `(string lpApplicationName, string lpCommandLine, IntPtr lpProcessAttributes, IntPtr lpThreadAttributes, bool bInheritHandles, uint dwCreationFlags, IntPtr lpEnvironment, string lpCurrentDirectory, ref STARTUPINFO lpStartupInfo, out PROCESS_INFORMATION lpProcessInformation) -> bool` | None (defaults to ANSI -- **migration concern**) | Yes          | 45-49 |
| 11  | `MiniDumpWriteDump`  | Dbghelp.dll  | `(IntPtr hProcess, int ProcessId, IntPtr hFile, int DumpType, IntPtr ExceptionParam, IntPtr UserStreamParam, IntPtr CallbackParam) -> bool`                                                                                                                                                 | None                                             | Yes          | 51-53 |

#### InjectionManager.cs -- 12 declarations

| #   | API                   | DLL          | Signature                                                                                                                                                      | CharSet / Marshalling             | SetLastError | Line    |
| --- | --------------------- | ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- | ------------ | ------- |
| 1   | `OpenProcess`         | kernel32.dll | `(int dwDesiredAccess, bool bInheritHandle, int dwProcessId) -> IntPtr`                                                                                        | None                              | No           | 17-18   |
| 2   | `GetProcAddress`      | kernel32.dll | `(IntPtr hModule, string procName) -> IntPtr`                                                                                                                  | None (ANSI -- **must stay ANSI**) | No           | 20-21   |
| 3   | `GetModuleHandle`     | kernel32.dll | `(string lpModuleName) -> IntPtr`                                                                                                                              | None                              | No           | 23-24   |
| 4   | `VirtualAllocEx`      | kernel32.dll | `(IntPtr hProcess, IntPtr lpAddress, uint dwSize, uint flAllocationType, uint flProtect) -> IntPtr`                                                            | None                              | No           | 26-28   |
| 5   | `VirtualFreeEx`       | kernel32.dll | `(IntPtr hProcess, IntPtr lpAddress, uint dwSize, uint dwFreeType) -> bool`                                                                                    | None                              | No           | 30-31   |
| 6   | `WriteProcessMemory`  | kernel32.dll | `(IntPtr hProcess, IntPtr lpBaseAddress, byte[] lpBuffer, uint nSize, out UIntPtr lpNumberOfBytesWritten) -> bool`                                             | None                              | No           | 33-35   |
| 7   | `CreateRemoteThread`  | kernel32.dll | `(IntPtr hProcess, IntPtr lpThreadAttributes, uint dwStackSize, IntPtr lpStartAddress, IntPtr lpParameter, uint dwCreationFlags, IntPtr lpThreadId) -> IntPtr` | None                              | No           | 37-39   |
| 8   | `CloseHandle`         | kernel32.dll | `(IntPtr hObject) -> bool`                                                                                                                                     | None                              | Yes          | 41-42   |
| 9   | `LoadLibrary`         | kernel32.dll | `(string lpFileName) -> IntPtr`                                                                                                                                | `CharSet = CharSet.Auto`          | Yes          | 44-45   |
| 10  | `FreeLibrary`         | kernel32.dll | `(IntPtr hModule) -> bool`                                                                                                                                     | None                              | Yes          | 47-48   |
| 11  | `WaitForSingleObject` | kernel32.dll | `(IntPtr hHandle, uint dwMilliseconds) -> uint`                                                                                                                | None                              | Yes          | 314-315 |
| 12  | `GetExitCodeThread`   | kernel32.dll | `(IntPtr hThread, out uint lpExitCode) -> bool`                                                                                                                | None                              | Yes          | 317-318 |

#### MemoryManager.cs -- 3 declarations

| #   | API                  | DLL          | Signature                                                                                                          | CharSet / Marshalling | SetLastError | Line  |
| --- | -------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------ | --------------------- | ------------ | ----- |
| 1   | `ReadProcessMemory`  | kernel32.dll | `(IntPtr hProcess, IntPtr lpBaseAddress, byte[] lpBuffer, uint nSize, out UIntPtr lpNumberOfBytesRead) -> bool`    | None                  | No           | 15-17 |
| 2   | `WriteProcessMemory` | kernel32.dll | `(IntPtr hProcess, IntPtr lpBaseAddress, byte[] lpBuffer, uint nSize, out UIntPtr lpNumberOfBytesWritten) -> bool` | None                  | No           | 19-21 |
| 3   | `VirtualQueryEx`     | kernel32.dll | `(IntPtr hProcess, IntPtr lpAddress, out MEMORY_BASIC_INFORMATION lpBuffer, uint dwLength) -> IntPtr`              | None                  | No           | 23-25 |

### Duplicated APIs

The following P/Invoke declarations appear in multiple files with identical signatures:

| API                  | Files Where Declared                                                       | Notes                                                                |
| -------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `OpenProcess`        | ProcessManager.cs (L15), InjectionManager.cs (L17)                         | Identical signatures                                                 |
| `CloseHandle`        | ProcessManager.cs (L18), InjectionManager.cs (L41)                         | InjectionManager adds `SetLastError = true`; ProcessManager does not |
| `CreateRemoteThread` | ProcessManager.cs (L21), InjectionManager.cs (L37)                         | Identical signatures                                                 |
| `WriteProcessMemory` | ProcessManager.cs (L25), InjectionManager.cs (L33), MemoryManager.cs (L19) | Identical signatures across all 3 files                              |
| `VirtualAllocEx`     | ProcessManager.cs (L29), InjectionManager.cs (L26)                         | Identical signatures                                                 |
| `VirtualFreeEx`      | ProcessManager.cs (L33), InjectionManager.cs (L30)                         | Identical signatures                                                 |

**Migration opportunity**: These 6 duplicated APIs (14 declaration sites total) should be consolidated into a shared `NativeInterop/Kernel32.cs` static class during migration, reducing the total declaration sites from 29 to 19.

### Struct Definitions

#### STARTUPINFO (ProcessManager.cs, lines 55-76)

```
[StructLayout(LayoutKind.Sequential)]
Fields: cb (int), lpReserved (string), lpDesktop (string), lpTitle (string),
        dwX/dwY/dwXSize/dwYSize (int), dwXCountChars/dwYCountChars (int),
        dwFillAttribute (int), dwFlags (int), wShowWindow/cbReserved2 (short),
        lpReserved2 (IntPtr), hStdInput/hStdOutput/hStdError (IntPtr)
```

**Migration note**: The `string` fields (`lpReserved`, `lpDesktop`, `lpTitle`) will need `[MarshalAs(UnmanagedType.LPTStr)]` or explicit marshalling when converting to `[LibraryImport]` with `StringMarshalling.Utf16`.

#### PROCESS_INFORMATION (ProcessManager.cs, lines 78-85)

```
[StructLayout(LayoutKind.Sequential)]
Fields: hProcess (IntPtr), hThread (IntPtr), dwProcessId (int), dwThreadId (int)
```

No migration issues -- all blittable types.

#### MEMORY_BASIC_INFORMATION (MemoryManager.cs, lines 27-37)

```
[StructLayout(LayoutKind.Sequential)]
Fields: BaseAddress (IntPtr), AllocationBase (IntPtr), AllocationProtect (uint),
        RegionSize (IntPtr), State (uint), Protect (uint), Type (uint)
```

No migration issues -- all blittable types.

### Constants Definitions

#### Process Access Rights (ProcessManager.cs L87-93, InjectionManager.cs L50-56 -- DUPLICATED)

| Constant                    | Value      | Used In                                                |
| --------------------------- | ---------- | ------------------------------------------------------ |
| `PROCESS_CREATE_THREAD`     | `0x0002`   | Both files (declared but only PROCESS_ALL_ACCESS used) |
| `PROCESS_QUERY_INFORMATION` | `0x0400`   | Both files (declared but unused)                       |
| `PROCESS_VM_OPERATION`      | `0x0008`   | Both files (declared but unused)                       |
| `PROCESS_VM_WRITE`          | `0x0020`   | Both files (declared but unused)                       |
| `PROCESS_VM_READ`           | `0x0010`   | Both files (declared but unused)                       |
| `PROCESS_ALL_ACCESS`        | `0x1F0FFF` | Both files (actually used in OpenProcess calls)        |

#### Thread Access Rights (ProcessManager.cs L96-99)

| Constant                | Value      | Used In                                                  |
| ----------------------- | ---------- | -------------------------------------------------------- |
| `THREAD_SUSPEND_RESUME` | `0x0002`   | ProcessManager.cs (used in SuspendProcess/ResumeProcess) |
| `THREAD_GET_CONTEXT`    | `0x0008`   | ProcessManager.cs (declared but unused)                  |
| `THREAD_SET_CONTEXT`    | `0x0010`   | ProcessManager.cs (declared but unused)                  |
| `THREAD_ALL_ACCESS`     | `0x1F03FF` | ProcessManager.cs (declared but unused)                  |

#### Memory Allocation Constants (ProcessManager.cs L102-106, InjectionManager.cs L58-63, MemoryManager.cs L39-53 -- DUPLICATED)

| Constant                 | Value        | Declared In                                              |
| ------------------------ | ------------ | -------------------------------------------------------- |
| `MEM_COMMIT`             | `0x1000`     | All 3 files                                              |
| `MEM_RESERVE`            | `0x2000`     | ProcessManager.cs, InjectionManager.cs, MemoryManager.cs |
| `MEM_RELEASE`            | `0x8000`     | ProcessManager.cs, InjectionManager.cs                   |
| `MEM_FREE`               | `0x10000`    | MemoryManager.cs only                                    |
| `PAGE_READWRITE`         | `0x04`       | All 3 files                                              |
| `PAGE_EXECUTE_READWRITE` | `0x40`       | InjectionManager.cs, MemoryManager.cs                    |
| `PAGE_EXECUTE`           | `0x10`       | MemoryManager.cs only                                    |
| `PAGE_EXECUTE_READ`      | `0x20`       | MemoryManager.cs only                                    |
| `PAGE_EXECUTE_WRITECOPY` | `0x80`       | MemoryManager.cs only                                    |
| `PAGE_NOACCESS`          | `0x01`       | MemoryManager.cs only                                    |
| `PAGE_READONLY`          | `0x02`       | MemoryManager.cs only                                    |
| `PAGE_WRITECOPY`         | `0x08`       | MemoryManager.cs only                                    |
| `PAGE_GUARD`             | `0x100`      | MemoryManager.cs only                                    |
| `CREATE_SUSPENDED`       | `0x00000004` | ProcessManager.cs (declared but unused)                  |

#### MiniDump Constants (ProcessManager.cs L109-110)

| Constant                 | Value        |
| ------------------------ | ------------ |
| `MiniDumpNormal`         | `0x00000000` |
| `MiniDumpWithFullMemory` | `0x00000002` |

### String Marshalling Requirements

| API                                        | Current Marshalling                                  | Actual Requirement                             | Migration Action                                                                                    |
| ------------------------------------------ | ---------------------------------------------------- | ---------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `CreateProcess` (ProcessManager.cs)        | Default (ANSI on .NET Framework)                     | Should be Unicode for WINE path handling       | Use `StringMarshalling.Utf16` with `EntryPoint = "CreateProcessW"`                                  |
| `GetProcAddress` (InjectionManager.cs)     | Default (ANSI)                                       | MUST remain ANSI (Win32 API only accepts ANSI) | Use `StringMarshalling = StringMarshalling.Utf8` (or explicit `EntryPoint = "GetProcAddress"`)      |
| `GetModuleHandle` (InjectionManager.cs)    | Default (ANSI)                                       | Should be Unicode                              | Use `StringMarshalling.Utf16` with `EntryPoint = "GetModuleHandleW"`                                |
| `LoadLibrary` (InjectionManager.cs)        | `CharSet = CharSet.Auto` (resolves to Unicode on NT) | Unicode                                        | Use `StringMarshalling.Utf16` (maps to `LoadLibraryW`)                                              |
| `STARTUPINFO.lpReserved/lpDesktop/lpTitle` | Implicit string marshalling                          | Unicode when using `CreateProcessW`            | Add `[MarshalAs(UnmanagedType.LPWStr)]` or use `StringMarshalling.Utf16` on the containing P/Invoke |

**Critical gotcha**: In `InjectionManager.InjectDllStandard()` (line 248), the DLL path is encoded with `Encoding.ASCII.GetBytes()` and injected via `LoadLibraryA`. This works today but will break for paths containing non-ASCII characters. If `CreateProcess` is migrated to use `CreateProcessW` (Unicode), the injection path should also be updated to use `LoadLibraryW` and `Encoding.Unicode.GetBytes()` for consistency, though this requires also changing the `GetProcAddress` call from `"LoadLibraryA"` to `"LoadLibraryW"`.

---

## File-Based Configuration

### Overview

The application uses three categories of file-based configuration, all stored relative to `Application.StartupPath`:

```
{Application.StartupPath}/
    settings.ini              -- Recent file paths (MRU lists)
    Settings/
        AppSettings.ini       -- Application preferences
    Profiles/
        {name}.profile        -- Per-game injection profiles
```

### Profile System

**File location**: `{Application.StartupPath}/Profiles/{name}.profile`

**Format**: Custom key=value pairs (one per line, no section headers)

**Read by**: `MainForm.LoadProfile()` (line 1674)
**Written by**: `MainForm.SaveProfile()` (line 1624)
**Managed by**: `MainForm.LoadProfiles()` (line 1586) -- directory scan for `*.profile` files

**File format example**:

```ini
GamePath=C:\Games\SomeGame\game.exe
TrainerPath=C:\Trainers\SomeTrainer.exe
Dll1Path=C:\Mods\mod1.dll
Dll2Path=C:\Mods\mod2.dll
LaunchInject1=True
LaunchInject2=False
LaunchMethod=CreateProcess
```

**Fields stored**:

| Key             | Type               | Source                              |
| --------------- | ------------------ | ----------------------------------- |
| `GamePath`      | string (file path) | `_selectedGamePath`                 |
| `TrainerPath`   | string (file path) | `_selectedTrainerPath`              |
| `Dll1Path`      | string (file path) | `_selectedDll1Path`                 |
| `Dll2Path`      | string (file path) | `_selectedDll2Path`                 |
| `LaunchInject1` | bool               | `chkLaunchInject1.Checked`          |
| `LaunchInject2` | bool               | `chkLaunchInject2.Checked`          |
| `LaunchMethod`  | enum string        | `_launchMethod` (LaunchMethod enum) |

**Parsing**: Split on `=` with `StringSplitOptions` limit of 2 parts (handles `=` in values). Uses `bool.Parse()` and `Enum.TryParse<LaunchMethod>()` for typed values.

**Edge case**: Profile file validation only checks `File.Exists(profilePath)` -- there is no format validation, no version field, and no handling of missing keys. A corrupted or partially-written profile file will silently skip unrecognized keys.

### Settings System

#### settings.ini (Recent Files MRU)

**File location**: `{Application.StartupPath}/settings.ini`

**Format**: Custom INI with section headers but no key=value pairs -- raw file paths as values.

**Read by**: `MainForm.LoadRecentFiles()` (line 1483)
**Written by**: `MainForm.SaveRecentFiles()` (line 1551)

**File format example**:

```ini
[RecentGamePaths]
C:\Games\Game1\game.exe
C:\Games\Game2\game.exe

[RecentTrainerPaths]
C:\Trainers\Trainer1.exe

[RecentDllPaths]
C:\Mods\mod1.dll
```

**Sections**: `RecentGamePaths`, `RecentTrainerPaths`, `RecentDllPaths`

**Parsing**: Section detection via `[` prefix and `]` suffix. Lines starting with `;` are treated as comments. Blank lines are skipped. Each non-section, non-comment line is treated as a file path and validated with `File.Exists()` before adding to the MRU list.

**Edge case**: The `File.Exists()` check on load means paths to removable media or network drives that are temporarily offline are silently dropped from the MRU list and never restored.

#### AppSettings.ini (Application Preferences)

**File location**: `{Application.StartupPath}/Settings/AppSettings.ini`

**Format**: Simple key=value pairs (no section headers).

**Read by**: `MainForm.LoadAppSettings()` (line 2734)
**Written by**: `MainForm.SaveAppSettings()` (line 2704)

**File format example**:

```ini
AutoLoadLastProfile=True
LastUsedProfile=MyGame
```

**Fields stored**:

| Key                   | Type   | Source                 |
| --------------------- | ------ | ---------------------- |
| `AutoLoadLastProfile` | bool   | `_autoLoadLastProfile` |
| `LastUsedProfile`     | string | `_lastUsedProfile`     |

**Edge case**: Uses `bool.Parse()` which throws `FormatException` on invalid values. The outer try-catch will catch this and log it, but the settings will revert to defaults.

### File I/O Patterns

- **All reads**: `File.ReadAllLines()` + manual parsing (no streaming)
- **All writes**: `StreamWriter` with `using` blocks
- **Path construction**: `Path.Combine(Application.StartupPath, ...)` throughout
- **Directory creation**: `Directory.CreateDirectory()` with existence checks
- **No file locking**: Concurrent read/write is not guarded (single-instance Mutex mitigates this)
- **No atomic writes**: Files are written directly (no temp file + rename pattern)
- **Encoding**: Default system encoding (UTF-8 on .NET 9 vs potentially different on .NET Framework 4.8)

### Application.StartupPath Migration Concern

**This is a critical migration concern.** `Application.StartupPath` behaves differently under .NET 9 single-file deployment:

- **.NET Framework 4.8**: Returns the directory containing the .exe
- **.NET 9 single-file**: Returns the directory containing the .exe (same behavior)
- **.NET 9 single-file with extraction**: Returns the temp extraction directory (DIFFERENT)

Since the recommended publish configuration uses `<IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>`, `Application.StartupPath` should still return the correct directory. However, this must be validated under WINE/Proton. If issues arise, the fallback is `Path.GetDirectoryName(Environment.ProcessPath)` which is the .NET 6+ reliable way to get the exe directory.

---

## External Services and Dependencies

### NuGet Packages

#### Current: packages.config

| Package | Version | Target Framework | Used in Source                                                  | Status     |
| ------- | ------- | ---------------- | --------------------------------------------------------------- | ---------- |
| SharpDX | 4.2.0   | net48            | **NOT USED** -- zero imports, zero references in any `.cs` file | **REMOVE** |

**SharpDX analysis**: The package is declared in `packages.config` but no source file contains `using SharpDX`, `SharpDX.`, or any SharpDX type reference. The README mentions "XInput Handling" but the codebase has no XInput implementation. SharpDX itself is an archived/abandoned project (last release was 2019, repository archived on GitHub). It does not support .NET 5+.

**Migration action**: Delete `packages.config`. Do not add any `<PackageReference>` entries. The migrated project will have zero NuGet dependencies.

**Future consideration**: If XInput/gamepad support is added later, use `Vortice.XInput` (actively maintained, supports .NET 8/9) or direct P/Invoke of `xinput1_4.dll`.

### Framework Assemblies

Current `.csproj` references and their .NET 9 equivalents:

| .NET Framework 4.8 Reference    | Used In Source                                                | .NET 9 Equivalent                                          | Migration Action                 |
| ------------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------- | -------------------------------- |
| `System`                        | Core types everywhere                                         | Implicit via TFM                                           | Remove reference (auto-included) |
| `System.Core`                   | LINQ (MainForm.cs uses `System.Linq`)                         | Implicit via TFM                                           | Remove reference                 |
| `System.Xml.Linq`               | Not used in source                                            | N/A                                                        | Remove reference                 |
| `System.Data.DataSetExtensions` | Not used in source                                            | N/A                                                        | Remove reference                 |
| `Microsoft.CSharp`              | Not used in source                                            | N/A                                                        | Remove reference                 |
| `System.Data`                   | Not used in source                                            | N/A                                                        | Remove reference                 |
| `System.Deployment`             | Not used in source                                            | N/A (ClickOnce -- removed in .NET Core)                    | Remove reference                 |
| `System.Drawing`                | ResumePanel.cs, MainForm.cs, MainForm.Designer.cs             | `System.Drawing.Common` via `<UseWindowsForms>`            | Auto-included with WinForms      |
| `System.Net.Http`               | Not used in source                                            | Implicit via TFM                                           | Remove reference                 |
| `System.Windows.Forms`          | MainForm.cs, MainForm.Designer.cs, ResumePanel.cs, Program.cs | `Microsoft.WindowsDesktop.App.Ref` via `<UseWindowsForms>` | Auto-included                    |
| `System.Xml`                    | Not used in source                                            | N/A                                                        | Remove reference                 |

**Observation**: 6 of 11 referenced assemblies (`System.Xml.Linq`, `System.Data.DataSetExtensions`, `Microsoft.CSharp`, `System.Data`, `System.Deployment`, `System.Xml`) are not used in any source file. These are Visual Studio template defaults that were never cleaned up.

### System.Windows.Forms Usage Analysis

WinForms is used extensively in MainForm.cs (the largest file at ~2800 lines) and ResumePanel.cs. The UI is almost entirely programmatic -- the Designer file (`MainForm.Designer.cs`) is only 53 lines and sets just basic form properties.

**Controls used**:

- `Form` (MainForm, ProfileInputDialog nested class)
- `Panel`, `TableLayoutPanel`, `FlowLayoutPanel` (layout)
- `TabControl`, `TabPage` (main navigation)
- `Button`, `ComboBox`, `CheckBox`, `RadioButton` (inputs)
- `TextBox` (console output -- multiline)
- `ListBox` (loaded DLLs list)
- `Label` (text labels)
- `StatusStrip`, `ToolStripStatusLabel` (status bar)
- `OpenFileDialog` (file browsing)
- `MessageBox` (modal dialogs)
- `Timer` (resize debounce)

**Custom drawing**: Tab control uses `OwnerDrawFixed` mode with custom `DrawItem` handler for dark theme tabs (MainForm.cs, lines 456-480). ResumePanel overrides `OnPaint()` for custom semi-transparent overlay rendering.

**Thread marshalling**: `UpdateStatus()` and `LogToConsole()` both use `InvokeRequired` / `Invoke()` pattern for cross-thread UI updates. This pattern is identical in .NET 9 WinForms.

All controls and APIs are fully compatible with .NET 9 WinForms.

### System.Drawing Usage Analysis

| Type                                  | Usage Location                                    | .NET 9 Status                             |
| ------------------------------------- | ------------------------------------------------- | ----------------------------------------- |
| `Color` / `Color.FromArgb()`          | Dark theme throughout MainForm.cs, ResumePanel.cs | Compatible                                |
| `Font`                                | UI fonts in MainForm.cs, ResumePanel.cs           | Compatible                                |
| `Size`, `SizeF`, `Point`, `Rectangle` | Layout geometry                                   | Compatible                                |
| `Brush`, `SolidBrush`                 | ResumePanel.cs custom painting                    | Compatible                                |
| `StringFormat`, `StringAlignment`     | Text centering in ResumePanel.cs                  | Compatible                                |
| `Graphics`                            | Tab drawing, ResumePanel.OnPaint()                | Compatible                                |
| `Pen`                                 | Tab border drawing                                | Compatible                                |
| `ContentAlignment`                    | Text alignment on Labels                          | Compatible                                |
| `DockStyle`                           | Control docking                                   | Compatible (WinForms, not System.Drawing) |

All System.Drawing usage targets `net9.0-windows`, so the Windows-only restriction introduced in .NET 6 is not a concern.

---

## Build System

### Current MSBuild Configuration

**Solution file**: `src/ChooChooEngine.sln`

- Format Version 12.00 (Visual Studio 2017+)
- Single project: `ChooChooEngine.App`
- Project type GUID: `{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}` (C# classic)
- Configurations: `Debug|Any CPU`, `Release|Any CPU`

**Project file**: `src/ChooChooEngine.App/ChooChooEngine.App.csproj`

- ToolsVersion: 15.0
- OutputType: WinExe
- TargetFrameworkVersion: v4.8
- AssemblyName: `choochoo`
- RootNamespace: `ChooChooEngine.App`
- FileAlignment: 512
- AutoGenerateBindingRedirects: true
- Deterministic: true
- AllowUnsafeBlocks: true (both Debug and Release)
- PlatformTarget: AnyCPU

**Build outputs**:

- Debug: `bin\Debug\choochoo.exe`
- Release: `bin\Release\choochoo.exe`
- Pre-built release binary: `release/ChooChooEngine.App.exe` (in repo root)

**Build command** (current): `msbuild src/ChooChooEngine.sln /p:Configuration=Release`

**Build command** (post-migration): `dotnet build src/ChooChooEngine.sln -c Release`

### Explicit Compile Items (will become auto-globbed)

The classic `.csproj` explicitly lists every source file:

| Compile Item                    | SubType/DependentUpon        |
| ------------------------------- | ---------------------------- |
| `Core\ProcessManager.cs`        | --                           |
| `Forms\MainForm.cs`             | `SubType: Form`              |
| `Forms\MainForm.Designer.cs`    | `DependentUpon: MainForm.cs` |
| `Injection\InjectionManager.cs` | --                           |
| `Memory\MemoryManager.cs`       | --                           |
| `Program.cs`                    | --                           |
| `Properties\AssemblyInfo.cs`    | --                           |
| `UI\ResumePanel.cs`             | `SubType: Component`         |

SDK-style projects auto-glob all `**/*.cs` files, so these explicit includes are removed. The `SubType` and `DependentUpon` metadata for Designer files is still respected by SDK-style projects for Visual Studio integration.

### Empty Directories

The `.csproj` references an empty `Utils\` folder (`<Folder Include="Utils\" />`). This can be removed during migration.

### CI/CD

GitHub Actions workflows exist at:

- `.github/workflows/claude-code-review.yml` -- automated code review
- `.github/workflows/claude.yml` -- Claude integration

These are not .NET build workflows and do not need migration changes.

---

## Configuration

### Environment Variables

The project uses `direnv` with `.envrc` and `dotenvx` for encrypted environment variables. The env files are:

| File             | Purpose                       | Git Status |
| ---------------- | ----------------------------- | ---------- |
| `.env`           | Runtime environment variables | Gitignored |
| `.env.encrypted` | Encrypted env vars            | Gitignored |
| `.env.example`   | Template                      | Tracked    |
| `.env.keys`      | Encryption keys               | Gitignored |
| `.envrc`         | direnv configuration          | Tracked    |

These are for development environment management, not application runtime configuration. The application itself reads no environment variables -- all configuration is file-based (INI/profile files).

### Command Line Arguments

The application accepts command-line arguments parsed in `MainForm.ProcessCommandLineArguments()` (line 2602):

| Argument      | Format                              | Purpose                                                                 |
| ------------- | ----------------------------------- | ----------------------------------------------------------------------- |
| `-p`          | `-p "ProfileName"`                  | Load a specific profile on startup                                      |
| `-autolaunch` | `-autolaunch "C:\path\to\game.exe"` | Auto-launch a game executable (consumes all remaining args as the path) |

When `-autolaunch` is used, the application automatically launches the game, minimizes the window, and applies any DLL injections configured in the loaded profile.

### Runtime File System Requirements

The application requires write access to its own directory for:

1. `settings.ini` -- MRU file paths
2. `Settings/AppSettings.ini` -- application preferences
3. `Profiles/*.profile` -- user-created injection profiles

This is a concern for certain deployment scenarios (e.g., Program Files on Windows, read-only Flatpak sandboxes), but under WINE/Proton the application directory is typically writable.

---

## Relevant Files

- `src/ChooChooEngine.App/Core/ProcessManager.cs`: 11 P/Invoke declarations (kernel32 + Dbghelp), 2 structs (STARTUPINFO, PROCESS_INFORMATION), process lifecycle management
- `src/ChooChooEngine.App/Injection/InjectionManager.cs`: 12 P/Invoke declarations (kernel32), DLL injection via LoadLibraryA + CreateRemoteThread, DLL validation via PE header parsing
- `src/ChooChooEngine.App/Memory/MemoryManager.cs`: 3 P/Invoke declarations (kernel32), 1 struct (MEMORY_BASIC_INFORMATION), memory read/write/save/restore with binary serialization
- `src/ChooChooEngine.App/Forms/MainForm.cs`: ~2800 lines, all file I/O (profiles, settings, recent files), command-line argument parsing, UI construction, event handling
- `src/ChooChooEngine.App/Forms/MainForm.Designer.cs`: 53-line minimal designer file (form dimensions and dark theme base colors only)
- `src/ChooChooEngine.App/UI/ResumePanel.cs`: Custom WinForms Panel with System.Drawing rendering, proper IDisposable pattern
- `src/ChooChooEngine.App/Program.cs`: Entry point with single-instance Mutex enforcement, standard WinForms bootstrap
- `src/ChooChooEngine.App/Properties/AssemblyInfo.cs`: Assembly metadata (to be deleted -- replaced by csproj properties)
- `src/ChooChooEngine.App/ChooChooEngine.App.csproj`: Classic .NET Framework 4.8 project file (to be completely rewritten as SDK-style)
- `src/ChooChooEngine.App/packages.config`: Single unused SharpDX 4.2.0 reference (to be deleted)
- `src/ChooChooEngine.sln`: Solution file with single project

## Architectural Patterns

- **P/Invoke region blocks**: Every class with native calls groups declarations in `#region Win32 API` blocks, keeping them separate from managed code. This pattern should be preserved (or consolidated into a shared class).
- **Duplicated P/Invoke**: 6 APIs are declared identically in multiple files. This is a common .NET Framework pattern where each class declares its own imports. Migration should consolidate these.
- **Event-driven component communication**: ProcessManager, InjectionManager, and MemoryManager all expose `EventHandler<T>` events with dedicated EventArgs classes. MainForm subscribes to all of these in `RegisterEventHandlers()`.
- **Custom INI parsing**: The application implements its own INI file parser rather than using `System.Configuration` or any library. The parser supports section headers (`[Section]`), comments (`;`), and key=value pairs but not all INI features (no quoted values, no escape sequences, no multi-line values).
- **Application.StartupPath as root**: All configuration storage is relative to the application directory, not `%APPDATA%` or any user-specific location.
- **Single-instance enforcement**: Named Mutex (`ChooChooEngineInjectorSingleInstance`) in Program.cs prevents multiple instances.
- **Cross-thread UI invocation**: `InvokeRequired` / `Invoke()` pattern used consistently for thread safety in `UpdateStatus()` and `LogToConsole()`.

## Gotchas and Edge Cases

- **ASCII DLL path encoding** (InjectionManager.cs, L248): `Encoding.ASCII.GetBytes(dllPath)` will silently corrupt non-ASCII characters in file paths. If migrating `CreateProcess` to Unicode (`CreateProcessW`), the injection path should also be updated to use `LoadLibraryW` + Unicode encoding for consistency.
- **STARTUPINFO string fields**: The `lpReserved`, `lpDesktop`, and `lpTitle` fields in STARTUPINFO are declared as `string` with no explicit marshalling. Under .NET Framework 4.8 with ANSI default, these marshal as ANSI (`LPSTR`). When converting to `[LibraryImport]` with `StringMarshalling.Utf16`, these must be explicitly handled or the struct may need `[MarshalAs]` attributes on string fields.
- **Application.StartupPath under single-file publish**: When publishing as a single-file with `<IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>`, `Application.StartupPath` should return the exe directory. However, if extraction is used, it could return a temp directory, breaking all configuration file paths. Verify under WINE/Proton.
- **File.Exists() pruning of MRU paths**: `LoadRecentFiles()` silently drops paths that do not exist at load time. Paths to offline removable media or network drives are permanently lost.
- **bool.Parse() in LoadAppSettings**: No fallback -- if `AppSettings.ini` contains an invalid boolean value, `bool.Parse()` throws `FormatException`. The catch block logs but reverts to hardcoded defaults rather than the values that were successfully parsed before the exception.
- **Double event subscription**: `RegisterEventHandlers()` subscribes to ProcessManager/InjectionManager/MemoryManager events, but `InitializeManagers()` already subscribes to the same events. This results in double-firing of all event handlers. This is a pre-existing bug, not introduced by migration.
- **PE header parsing for DLL validation** (InjectionManager.cs, L204-238): The `IsDll64Bit()` method manually reads PE headers. The offset arithmetic (`fs.Position += 20`) skips past the COFF header to read characteristics, but the calculation appears correct. This code is not affected by the migration since it operates on raw bytes.
- **MiniDumpWriteDump under WINE**: WINE's `Dbghelp.dll` has limited minidump support. `MiniDumpWithFullMemory` may produce incomplete or empty dumps. This is a pre-existing limitation unrelated to migration.
- **Process ID filtering**: `RefreshProcessList()` skips processes with ID <= 4 and the current process. Under WINE, process IDs may differ from native Windows conventions.

## Other Docs

- `docs/plans/dotnet-migrate/research-technical.md`: Comprehensive technical specification covering SDK-style csproj conversion, P/Invoke migration details, WinForms compatibility matrix, and migration sequence
- `docs/plans/dotnet-migrate/feature-spec.md`: Feature specification with user stories, external dependencies, and library choices
- `docs/plans/dotnet-migrate/research-external.md`: External dependency and tooling research
- [Microsoft: P/Invoke Source Generation](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation): LibraryImport attribute documentation
- [Microsoft: Upgrade .NET Framework WinForms to .NET](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/): Official migration guide
- [Microsoft: Single-File Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview): Application.StartupPath behavior details
- [Microsoft: StringMarshalling Enum](https://learn.microsoft.com/en-us/dotnet/api/system.runtime.interopservices.stringmarshalling): Utf8 vs Utf16 vs Custom options
