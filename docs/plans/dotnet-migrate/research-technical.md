# Technical Specifications: dotnet-migrate

## Executive Summary

Migrate ChooChoo Loader from .NET Framework 4.8 (classic MSBuild) to .NET 9 (SDK-style) with `net9.0-windows` TFM, self-contained single-file publish for `win-x64`. All 20+ kernel32/Dbghelp P/Invoke declarations remain fully functional under modern .NET on WINE/Proton because they target the same Windows ABI; the migration converts them from `[DllImport]` to `[LibraryImport]` source-generated stubs. WinForms remains the UI framework via `<UseWindowsForms>true</UseWindowsForms>`. SharpDX 4.2.0 (archived/abandoned) must be removed or replaced since it is not referenced in any source file.

---

## Architecture Design

### Project Structure (Post-Migration)

The solution structure stays essentially the same, with files reorganized only where SDK-style conventions differ:

```
src/
  ChooChooEngine.sln                     (updated: project type GUID)
  ChooChooEngine.App/
    ChooChooEngine.App.csproj            (complete rewrite: SDK-style)
    Program.cs                           (minor: ApplicationConfiguration)
    Core/ProcessManager.cs               (P/Invoke modernization)
    Injection/InjectionManager.cs        (P/Invoke modernization)
    Memory/MemoryManager.cs              (P/Invoke modernization)
    Forms/MainForm.cs                    (minor namespace adjustments)
    Forms/MainForm.Designer.cs           (no changes needed)
    UI/ResumePanel.cs                    (no changes needed)
    NativeMethods.txt                    (NEW - if using CsWin32)
```

Files to remove:

- `Properties/AssemblyInfo.cs` -- replaced by `<GenerateAssemblyInfo>` in SDK-style csproj
- `packages.config` -- replaced by `<PackageReference>` in csproj
- `obj/` and `bin/` directories (clean rebuild required)

### SDK-Style .csproj Conversion

Replace the entire 75-line classic .csproj with this SDK-style equivalent:

```xml
<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <TargetFramework>net9.0-windows</TargetFramework>
    <OutputType>WinExe</OutputType>
    <UseWindowsForms>true</UseWindowsForms>
    <RootNamespace>ChooChooEngine.App</RootNamespace>
    <AssemblyName>choochoo</AssemblyName>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>

    <!-- Assembly metadata (replaces AssemblyInfo.cs) -->
    <AssemblyTitle>ChooChoo Injection Engine</AssemblyTitle>
    <Description>Game modding and trainer management utility</Description>
    <Copyright>Copyright 2025</Copyright>
    <Version>5.0.0</Version>
    <FileVersion>5.0.0.0</FileVersion>

    <!-- Publishing: self-contained single-file for WINE/Proton -->
    <RuntimeIdentifier>win-x64</RuntimeIdentifier>
    <SelfContained>true</SelfContained>
    <PublishSingleFile>true</PublishSingleFile>
    <IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>
    <EnableCompressionInSingleFile>true</EnableCompressionInSingleFile>
  </PropertyGroup>

  <PropertyGroup Condition="'$(Configuration)' == 'Debug'">
    <DebugType>full</DebugType>
    <Optimize>false</Optimize>
    <DefineConstants>DEBUG;TRACE</DefineConstants>
  </PropertyGroup>

  <PropertyGroup Condition="'$(Configuration)' == 'Release'">
    <DebugType>pdbonly</DebugType>
    <Optimize>true</Optimize>
    <DefineConstants>TRACE</DefineConstants>
  </PropertyGroup>

  <!-- No PackageReference entries needed unless CsWin32 or Vortice is adopted -->
  <!-- SharpDX is removed: not referenced in source, archived project -->

</Project>
```

Key differences from the classic csproj:

- No explicit `<Compile Include>` items (SDK-style auto-globs `**/*.cs`)
- No `<Reference Include="System.*">` entries (implicit via TFM)
- No `<Import Project="Microsoft.CSharp.targets">` (implicit via SDK)
- `packages.config` replaced by inline `<PackageReference>` elements
- `AssemblyInfo.cs` replaced by `<PropertyGroup>` metadata

### Target Framework Selection

| Option                 | Pros                                                                                                                            | Cons                                                       |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `net8.0-windows` (LTS) | LTS support until Nov 2026, widest community testing                                                                            | Missing .NET 9 improvements to System.Drawing and P/Invoke |
| `net9.0-windows` (STS) | Latest P/Invoke source generation improvements, ReadOnlySpan System.Drawing APIs, GDI+ bitmap effects, newest WinForms features | STS support ends May 2026; .NET 10 LTS arrives Nov 2025    |

**Recommendation: `net9.0-windows`**

Rationale: The project runs under WINE/Proton where the end-user never installs a .NET runtime (we publish self-contained). STS support lifetime is irrelevant because the binary ships the runtime. .NET 9 has better P/Invoke source generation, improved System.Drawing APIs, and WinForms improvements. When .NET 10 LTS ships, the retarget from net9.0 to net10.0 is a single line change.

---

## P/Invoke Migration

### Complete Win32 API Inventory

All P/Invoke declarations in the codebase, deduplicated across files:

| API Call              | DLL          | Source File(s)                                           | .NET 9 Status | Migration Path                                              |
| --------------------- | ------------ | -------------------------------------------------------- | ------------- | ----------------------------------------------------------- |
| `OpenProcess`         | kernel32.dll | ProcessManager.cs, InjectionManager.cs                   | Works         | Convert to `[LibraryImport]`                                |
| `CloseHandle`         | kernel32.dll | ProcessManager.cs, InjectionManager.cs                   | Works         | Convert to `[LibraryImport]`, wrap return in `SafeHandle`   |
| `CreateRemoteThread`  | kernel32.dll | ProcessManager.cs, InjectionManager.cs                   | Works         | Convert to `[LibraryImport]`                                |
| `WriteProcessMemory`  | kernel32.dll | ProcessManager.cs, InjectionManager.cs, MemoryManager.cs | Works         | Convert to `[LibraryImport]`                                |
| `VirtualAllocEx`      | kernel32.dll | ProcessManager.cs, InjectionManager.cs                   | Works         | Convert to `[LibraryImport]`                                |
| `VirtualFreeEx`       | kernel32.dll | ProcessManager.cs, InjectionManager.cs                   | Works         | Convert to `[LibraryImport]`                                |
| `OpenThread`          | kernel32.dll | ProcessManager.cs                                        | Works         | Convert to `[LibraryImport]`                                |
| `SuspendThread`       | kernel32.dll | ProcessManager.cs                                        | Works         | Convert to `[LibraryImport]`                                |
| `ResumeThread`        | kernel32.dll | ProcessManager.cs                                        | Works         | Convert to `[LibraryImport]`                                |
| `CreateProcess`       | kernel32.dll | ProcessManager.cs                                        | Works         | Convert to `[LibraryImport]`, use `StringMarshalling.Utf16` |
| `MiniDumpWriteDump`   | Dbghelp.dll  | ProcessManager.cs                                        | Works         | Convert to `[LibraryImport]`                                |
| `GetProcAddress`      | kernel32.dll | InjectionManager.cs                                      | Works         | Convert to `[LibraryImport]`, use `StringMarshalling.Utf8`  |
| `GetModuleHandle`     | kernel32.dll | InjectionManager.cs                                      | Works         | Convert to `[LibraryImport]`                                |
| `LoadLibrary`         | kernel32.dll | InjectionManager.cs                                      | Works         | Convert to `[LibraryImport]`, use `StringMarshalling.Utf16` |
| `FreeLibrary`         | kernel32.dll | InjectionManager.cs                                      | Works         | Convert to `[LibraryImport]`                                |
| `WaitForSingleObject` | kernel32.dll | InjectionManager.cs                                      | Works         | Convert to `[LibraryImport]`                                |
| `GetExitCodeThread`   | kernel32.dll | InjectionManager.cs                                      | Works         | Convert to `[LibraryImport]`                                |
| `ReadProcessMemory`   | kernel32.dll | MemoryManager.cs                                         | Works         | Convert to `[LibraryImport]`                                |
| `VirtualQueryEx`      | kernel32.dll | MemoryManager.cs                                         | Works         | Convert to `[LibraryImport]`                                |

**Total unique P/Invoke declarations: 19** (some duplicated across files -- 29 total declaration sites)

### Modern P/Invoke Patterns

#### DllImport to LibraryImport Conversion

Every `[DllImport]` must become a `[LibraryImport]` on a `static partial` method. Key syntax changes:

```csharp
// BEFORE (.NET Framework 4.8)
[DllImport("kernel32.dll")]
private static extern IntPtr OpenProcess(int dwDesiredAccess, bool bInheritHandle, int dwProcessId);

// AFTER (.NET 9)
[LibraryImport("kernel32.dll")]
[return: MarshalAs(UnmanagedType.SysInt)]
private static partial IntPtr OpenProcess(int dwDesiredAccess,
    [MarshalAs(UnmanagedType.Bool)] bool bInheritHandle, int dwProcessId);
```

Critical conversion rules:

- `extern` keyword is removed, replaced with `partial` keyword
- The containing class must also be declared `partial`
- `bool` parameters need explicit `[MarshalAs(UnmanagedType.Bool)]`
- `CharSet = CharSet.Auto` becomes `StringMarshalling = StringMarshalling.Utf16`
- `CharSet = CharSet.Ansi` becomes `StringMarshalling = StringMarshalling.Utf8` (or explicit entry point)
- `SetLastError = true` is preserved as-is on `[LibraryImport]`
- Structs used in P/Invoke remain the same (`[StructLayout(LayoutKind.Sequential)]`)

#### Specific Migration Notes per API

**CreateProcess** (ProcessManager.cs, line 46):

- Current: `CharSet` not specified (defaults to ANSI on .NET Framework)
- Migration: Add `StringMarshalling = StringMarshalling.Utf16` and use `CreateProcessW` entry point, or keep ANSI with explicit `EntryPoint = "CreateProcessA"`
- Recommendation: Use UTF-16 (`CreateProcessW`) for correct WINE path handling

**LoadLibrary** (InjectionManager.cs, line 44):

- Current: `CharSet = CharSet.Auto`
- Migration: `StringMarshalling = StringMarshalling.Utf16` (Auto resolved to Unicode on NT)

**GetProcAddress** (InjectionManager.cs, line 22):

- Current: No CharSet (defaults ANSI)
- Migration: `StringMarshalling = StringMarshalling.Utf8` since GetProcAddress only accepts ANSI strings

#### CsWin32 Alternative

Instead of manually converting all 19 P/Invoke declarations, the project could adopt Microsoft's CsWin32 source generator:

- Package: `Microsoft.Windows.CsWin32` (NuGet, currently beta)
- Add a `NativeMethods.txt` file listing needed functions (one per line)
- CsWin32 auto-generates correct, strongly-typed P/Invoke wrappers with `SafeHandle` support
- Generates supporting structs (STARTUPINFO, PROCESS_INFORMATION, MEMORY_BASIC_INFORMATION)

**Recommendation**: Manual `[LibraryImport]` conversion is preferred for this project. The codebase has only 19 unique P/Invoke calls, and manual conversion preserves the existing code organization pattern (`#region Win32 API` blocks). CsWin32 would generate code in a different location, breaking the established pattern. The manual approach also avoids a beta-stage dependency.

### WINE Compatibility Matrix

All P/Invoke calls target kernel32.dll and Dbghelp.dll, which are core WINE-implemented DLLs. The WINE compatibility status for each:

| API                 | WINE Status           | Notes                                                                 |
| ------------------- | --------------------- | --------------------------------------------------------------------- |
| OpenProcess         | Fully implemented     | Core process API                                                      |
| CloseHandle         | Fully implemented     | Core handle API                                                       |
| CreateRemoteThread  | Implemented           | May have limitations with certain flag combinations                   |
| WriteProcessMemory  | Fully implemented     | Core memory API                                                       |
| ReadProcessMemory   | Fully implemented     | Core memory API                                                       |
| VirtualAllocEx      | Fully implemented     | Core memory API                                                       |
| VirtualFreeEx       | Fully implemented     | Core memory API                                                       |
| VirtualQueryEx      | Fully implemented     | Core memory API                                                       |
| OpenThread          | Fully implemented     | Core thread API                                                       |
| SuspendThread       | Fully implemented     | Core thread API                                                       |
| ResumeThread        | Fully implemented     | Core thread API                                                       |
| CreateProcess       | Fully implemented     | Core process API, critical for WINE                                   |
| GetProcAddress      | Fully implemented     | Core module API                                                       |
| GetModuleHandle     | Fully implemented     | Core module API                                                       |
| LoadLibrary         | Fully implemented     | Core module API                                                       |
| FreeLibrary         | Fully implemented     | Core module API                                                       |
| WaitForSingleObject | Fully implemented     | Core synchronization API                                              |
| GetExitCodeThread   | Fully implemented     | Core thread API                                                       |
| MiniDumpWriteDump   | Partially implemented | WINE's Dbghelp has limited dump support; may produce incomplete dumps |

**Critical consideration**: The P/Invoke ABI does not change between .NET Framework 4.8 and .NET 9. Both produce the same native function calls. The `[LibraryImport]` source generator creates compile-time marshalling code that makes the same underlying OS calls. WINE sees identical native calls regardless of the .NET version, so existing WINE compatibility is preserved.

---

## WinForms Migration

### Designer Compatibility

The WinForms designer in .NET 9 is fully supported in Visual Studio 2022 (17.8+). Key notes:

- `MainForm.Designer.cs` (52 lines) requires **no changes** -- it uses only standard WinForms APIs (`AutoScaleDimensions`, `AutoScaleMode`, `ClientSize`, `BackColor`, `ForeColor`)
- `MainForm.cs` constructs most UI programmatically (not via Designer) -- approximately 1400 lines of manual control creation in `ConfigureUILayout()`. This code is framework-agnostic and will work unchanged on .NET 9
- The designer file is minimal because the UI is built in code, reducing migration risk

### Control Changes

WinForms controls used in the project and their .NET 9 status:

| Control/API                            | Usage                        | .NET 9 Status | Changes Needed |
| -------------------------------------- | ---------------------------- | ------------- | -------------- |
| `Form`                                 | MainForm, ProfileInputDialog | Compatible    | None           |
| `Panel`                                | Multiple layout panels       | Compatible    | None           |
| `TableLayoutPanel`                     | Main layout, sub-layouts     | Compatible    | None           |
| `FlowLayoutPanel`                      | Launch methods               | Compatible    | None           |
| `TabControl` / `TabPage`               | Main/Help/Tools tabs         | Compatible    | None           |
| `Button`                               | Multiple buttons             | Compatible    | None           |
| `ComboBox`                             | File path selectors          | Compatible    | None           |
| `CheckBox`                             | Inject toggles               | Compatible    | None           |
| `RadioButton`                          | Launch method selection      | Compatible    | None           |
| `TextBox`                              | Console output               | Compatible    | None           |
| `ListBox`                              | Loaded DLLs list             | Compatible    | None           |
| `Label`                                | Multiple labels              | Compatible    | None           |
| `StatusStrip` / `ToolStripStatusLabel` | Status bar                   | Compatible    | None           |
| `OpenFileDialog`                       | File browsing                | Compatible    | None           |
| `MessageBox`                           | Dialogs                      | Compatible    | None           |
| `Timer`                                | Resize debounce              | Compatible    | None           |
| `System.Timers.Timer`                  | Monitoring, auto-launch      | Compatible    | None           |

All WinForms controls and APIs used in this project have direct equivalents in .NET 9 WinForms with no breaking changes.

### System.Drawing Migration

System.Drawing types used in the project:

| Type                                     | Usage                        | .NET 9 Status             |
| ---------------------------------------- | ---------------------------- | ------------------------- |
| `Color` / `Color.FromArgb()`             | Dark theme colors            | Compatible (Windows-only) |
| `Font`                                   | UI fonts                     | Compatible                |
| `SizeF` / `Size` / `Point` / `Rectangle` | Layout geometry              | Compatible                |
| `Brush` / `SolidBrush`                   | ResumePanel painting         | Compatible                |
| `StringFormat`                           | Text rendering               | Compatible                |
| `Graphics`                               | Custom painting, tab drawing | Compatible                |
| `Pen`                                    | Tab border drawing           | Compatible                |
| `ContentAlignment`                       | Text alignment               | Compatible                |

Since the application targets `net9.0-windows` (not cross-platform), `System.Drawing` works without changes. The Windows-only restriction (introduced in .NET 6) is not a concern because this is inherently a Windows-only application running under WINE.

### Resource Management

The project does not use `.resx` resource files. All resources (fonts, colors, strings) are defined inline in code. No resource migration is needed.

### ApplicationConfiguration (Program.cs)

.NET 9 WinForms introduces `ApplicationConfiguration.Initialize()` as a replacement for the manual `EnableVisualStyles()` / `SetCompatibleTextRenderingDefault()` pattern. The migration is:

```csharp
// BEFORE (.NET Framework 4.8)
Application.EnableVisualStyles();
Application.SetCompatibleTextRenderingDefault(false);

// AFTER (.NET 9) - either approach works
// Option A: Modern one-liner
ApplicationConfiguration.Initialize();

// Option B: Keep explicit calls (still supported, no change needed)
Application.EnableVisualStyles();
Application.SetCompatibleTextRenderingDefault(false);
```

**Recommendation**: Keep the explicit calls (Option B) for maximum WINE compatibility. `ApplicationConfiguration.Initialize()` reads from the csproj at build time, which is less transparent.

---

## Build System Changes

### packages.config to PackageReference

Current `packages.config`:

```xml
<packages>
  <package id="SharpDX" version="4.2.0" targetFramework="net48" />
</packages>
```

**SharpDX analysis**: SharpDX 4.2.0 is listed in packages.config but is **not referenced in any source file**. A grep of the entire `src/` directory shows zero imports or usages of SharpDX namespaces. The README mentions "XInput Handling" but the actual source code does not contain any XInput/DirectInput/SharpDX code. This package should be **removed entirely** during migration.

If XInput support is added in the future, the replacement would be `Vortice.XInput` (from the Vortice.Windows project), which supports .NET 8/9.

Migration steps:

1. Delete `packages.config`
2. Do not add any `<PackageReference>` entries (SharpDX is unused)
3. If CsWin32 is desired later, add: `<PackageReference Include="Microsoft.Windows.CsWin32" Version="0.3.*-beta" PrivateAssets="all" />`

### MSBuild Properties

New properties in the SDK-style csproj (not present in classic):

| Property                                 | Value     | Purpose                                    |
| ---------------------------------------- | --------- | ------------------------------------------ |
| `<UseWindowsForms>`                      | `true`    | Enables WinForms SDK references            |
| `<Nullable>`                             | `enable`  | Enables nullable reference type analysis   |
| `<ImplicitUsings>`                       | `enable`  | Auto-imports common namespaces             |
| `<RuntimeIdentifier>`                    | `win-x64` | Target platform for self-contained publish |
| `<SelfContained>`                        | `true`    | Bundle .NET runtime with the exe           |
| `<PublishSingleFile>`                    | `true`    | Produce single executable                  |
| `<IncludeNativeLibrariesForSelfExtract>` | `true`    | Include native libs inside single-file     |
| `<EnableCompressionInSingleFile>`        | `true`    | Compress the single-file bundle            |

### Build and Publish Commands

```bash
# Build (replaces msbuild command)
dotnet build src/ChooChooEngine.sln -c Release

# Publish self-contained single-file exe
dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj \
  -c Release \
  -r win-x64 \
  --self-contained true \
  -p:PublishSingleFile=true

# Output location:
# src/ChooChooEngine.App/bin/Release/net9.0-windows/win-x64/publish/choochoo.exe
```

---

## Codebase Changes

### Files to Create

| Path                                               | Purpose                                                  |
| -------------------------------------------------- | -------------------------------------------------------- |
| `src/ChooChooEngine.App/ChooChooEngine.App.csproj` | Complete rewrite as SDK-style (see Architecture section) |
| `src/global.json`                                  | (Optional) Pin .NET SDK version for reproducible builds  |

### Files to Modify

| Path                                                   | Changes                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/ChooChooEngine.sln`                               | Update project type GUID from `{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}` to SDK-style (or keep -- both work with `dotnet build`)                                                                                                                                                                                                                             |
| `src/ChooChooEngine.App/Program.cs`                    | Add `partial` to class declaration (required for any class containing `[LibraryImport]`). Optionally replace `EnableVisualStyles`/`SetCompatibleTextRenderingDefault` with `ApplicationConfiguration.Initialize()`.                                                                                                                                        |
| `src/ChooChooEngine.App/Core/ProcessManager.cs`        | (1) Add `partial` to class declaration. (2) Convert 11 `[DllImport]` to `[LibraryImport]` with `partial` methods. (3) Add `[MarshalAs(UnmanagedType.Bool)]` to bool params. (4) Update `CreateProcess` to use `StringMarshalling.Utf16`. (5) Remove `extern` keyword from all P/Invoke methods.                                                            |
| `src/ChooChooEngine.App/Injection/InjectionManager.cs` | (1) Add `partial` to class declaration. (2) Convert 11 `[DllImport]` to `[LibraryImport]` with `partial` methods. (3) Add `[MarshalAs(UnmanagedType.Bool)]` to bool params. (4) Update `LoadLibrary` to use `StringMarshalling.Utf16`. (5) Update `GetProcAddress` to use `StringMarshalling.Utf8`. (6) Remove `extern` keyword from all P/Invoke methods. |
| `src/ChooChooEngine.App/Memory/MemoryManager.cs`       | (1) Add `partial` to class declaration. (2) Convert 3 `[DllImport]` to `[LibraryImport]` with `partial` methods. (3) Add `[MarshalAs(UnmanagedType.Bool)]` to bool params. (4) Remove `extern` keyword from all P/Invoke methods.                                                                                                                          |
| `src/ChooChooEngine.App/Forms/MainForm.cs`             | No required changes. Optional: add file-scoped namespace, update to target-typed `new()`, use `is not null` patterns.                                                                                                                                                                                                                                      |
| `src/ChooChooEngine.App/Forms/MainForm.Designer.cs`    | No changes needed.                                                                                                                                                                                                                                                                                                                                         |
| `src/ChooChooEngine.App/UI/ResumePanel.cs`             | No changes needed.                                                                                                                                                                                                                                                                                                                                         |
| `CLAUDE.md`                                            | Update tech stack, build commands, and architecture notes to reflect .NET 9                                                                                                                                                                                                                                                                                |
| `AGENTS.md`                                            | Same updates as CLAUDE.md                                                                                                                                                                                                                                                                                                                                  |

### Files to Delete

| Path                                                | Reason                                                                                                                                                  |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/ChooChooEngine.App/Properties/AssemblyInfo.cs` | Replaced by `<PropertyGroup>` in SDK-style csproj. SDK auto-generates assembly attributes.                                                              |
| `src/ChooChooEngine.App/packages.config`            | Replaced by `<PackageReference>` in csproj (empty in this case).                                                                                        |
| `src/ChooChooEngine.App/bin/`                       | Must be cleaned; incompatible build artifacts from .NET Framework 4.8.                                                                                  |
| `src/ChooChooEngine.App/obj/`                       | Must be cleaned; contains .NET Framework-specific intermediate files (`.NETFramework,Version=v4.8.AssemblyAttributes.cs`, stale `project.assets.json`). |

---

## Dependencies

### NuGet Packages

| Package                   | Version    | Purpose                            | Status                                                        |
| ------------------------- | ---------- | ---------------------------------- | ------------------------------------------------------------- |
| SharpDX                   | 4.2.0      | XInput (gamepad)                   | **REMOVE** -- not referenced in source code; archived project |
| Microsoft.Windows.CsWin32 | 0.3.x-beta | (Optional) Auto-generated P/Invoke | Not needed if manually converting                             |
| Vortice.XInput            | 2.x        | (Future) XInput replacement        | Only if gamepad support is re-added                           |

The migrated project should have **zero NuGet dependencies** for the initial migration, since SharpDX is unused and all required libraries (WinForms, System.Drawing, System.Net.Http) are included in the `net9.0-windows` TFM.

---

## Technical Decisions

### Decision 1: LibraryImport vs DllImport

- **Options**: (A) Keep `[DllImport]` as-is, (B) Convert to `[LibraryImport]`, (C) Use CsWin32 generator
- **Recommendation**: (B) Convert to `[LibraryImport]`
- **Rationale**: `[DllImport]` still works on .NET 9 but generates runtime IL stubs. `[LibraryImport]` generates compile-time marshalling code, enabling better performance, AOT compatibility, and debuggability. The conversion is mechanical (19 declarations). CsWin32 is overkill for 19 APIs and introduces a beta dependency.

### Decision 2: Self-Contained vs Framework-Dependent

- **Options**: (A) Framework-dependent (requires .NET runtime in WINE prefix), (B) Self-contained (bundles runtime)
- **Recommendation**: (B) Self-contained
- **Rationale**: The application runs under Proton/WINE. Installing .NET 9 runtime into a WINE prefix is unreliable and poorly documented. Self-contained eliminates the runtime dependency entirely. The exe size increases (~60-80MB) but this is acceptable for a desktop tool. Single-file publishing further simplifies distribution.

### Decision 3: .NET 9 vs .NET 8

- **Options**: (A) .NET 8 LTS, (B) .NET 9 STS
- **Recommendation**: (B) .NET 9, with plan to retarget to .NET 10 LTS when available
- **Rationale**: Self-contained deployment means the support lifecycle is irrelevant (the runtime ships with the binary). .NET 9 has P/Invoke source generation improvements and System.Drawing enhancements. Retargeting to .NET 10 LTS (expected Nov 2025) is a single-line TFM change.

### Decision 4: SharpDX Removal vs Replacement

- **Options**: (A) Remove SharpDX entirely, (B) Replace with Vortice.XInput, (C) Replace with direct XInput P/Invoke
- **Recommendation**: (A) Remove entirely
- **Rationale**: SharpDX 4.2.0 is listed in packages.config but no source file imports or uses any SharpDX type. The README mentions XInput handling, but the code does not implement it. If XInput gamepad support is needed in the future, it should be added as a separate feature with Vortice.XInput or direct P/Invoke of xinput1_4.dll.

### Decision 5: Nullable Reference Types

- **Options**: (A) Disable (`<Nullable>disable</Nullable>`), (B) Enable with warnings, (C) Enable with errors
- **Recommendation**: (B) Enable with warnings (`<Nullable>enable</Nullable>`)
- **Rationale**: The codebase has many null checks already (e.g., `if (_process == null)`). Enabling nullable analysis will surface missing null checks as warnings without breaking the build. This can be tightened to errors after the initial migration is stable.

### Decision 6: Implicit Usings

- **Options**: (A) Disable and keep explicit `using` statements, (B) Enable
- **Recommendation**: (B) Enable
- **Rationale**: SDK-style projects with `<ImplicitUsings>enable</ImplicitUsings>` auto-import `System`, `System.Collections.Generic`, `System.IO`, `System.Linq`, `System.Threading`, `System.Threading.Tasks`, and others. This removes most `using` statements at the top of each file. The remaining framework-specific usings (`System.Windows.Forms`, `System.Drawing`, `System.Runtime.InteropServices`, `System.Diagnostics`) must stay explicit.

---

## WINE/Proton Runtime Considerations

### Self-Contained .NET 9 Under WINE

The critical architectural advantage of self-contained publishing is that WINE never needs to "know" about .NET. The published `choochoo.exe` is a native Windows PE executable that:

1. Contains the CoreCLR runtime as embedded native code
2. Contains all managed assemblies (WinForms, System.Drawing, etc.) bundled inside
3. Loads kernel32.dll, user32.dll, gdi32.dll from WINE's implementation
4. Makes P/Invoke calls through WINE's DLL implementations

From WINE's perspective, the .NET 9 self-contained exe looks identical to a native C++ Windows application. There is no dependency on `mscorlib.dll`, `clr.dll`, or any .NET Framework GAC assemblies.

### Known WINE Issues

1. **Console UTF-8 initialization**: .NET 8+ aggressively sets console encoding to UTF-8, which can cause `System.IO.IOException` under WINE. Mitigation: This is a console app issue; WinForms apps (OutputType: WinExe) do not trigger this code path.

2. **MiniDumpWriteDump**: WINE's `Dbghelp.dll` has limited minidump support. The `MiniDumpWithFullMemory` flag may produce incomplete dumps. This is a pre-existing limitation, not introduced by migration.

3. **Process enumeration**: `Process.GetProcesses()` behavior under WINE may differ from native Windows (may show WINE internal processes). This is pre-existing behavior.

### Testing Strategy for WINE/Proton

After migration, the following must be verified under Proton:

1. Application launches and displays WinForms UI correctly
2. Process listing works (`RefreshProcessList()`)
3. File dialog (`OpenFileDialog`) opens and returns paths
4. Profile save/load with INI files works
5. Process launch via `CreateProcess` P/Invoke works
6. DLL injection via `CreateRemoteThread` + `LoadLibraryA` works
7. Memory read/write operations work
8. Single-instance Mutex enforcement works

---

## Migration Sequence (Recommended Order)

1. **Clean build artifacts**: Delete `bin/` and `obj/` directories
2. **Create SDK-style csproj**: Replace the entire `.csproj` file
3. **Delete AssemblyInfo.cs**: Metadata now in csproj
4. **Delete packages.config**: SharpDX removed, no replacements
5. **Update .sln file**: Verify project reference still resolves
6. **Verify build**: `dotnet build` should succeed with warnings
7. **Convert P/Invoke declarations**: ProcessManager, InjectionManager, MemoryManager
8. **Add partial keywords**: To all classes containing `[LibraryImport]`
9. **Address nullable warnings**: Add null checks or `!` suppressions
10. **Test under WINE/Proton**: Full functional verification
11. **Update documentation**: CLAUDE.md, AGENTS.md, README.md

---

## Open Questions

- Should the project adopt `global.json` to pin the .NET 9 SDK version for reproducible builds?
- Is XInput/gamepad support actually needed? The README mentions it but no code implements it. If so, Vortice.XInput or direct P/Invoke is the path forward.
- Should the `ProfileInputDialog` nested class be extracted to its own file during migration for cleaner organization?
- Should the massive `ConfigureUILayout()` method (~800 lines) be refactored into smaller methods during or after migration?
- Is Native AOT compilation (`<PublishAot>true</PublishAot>`) desirable? It would produce a truly native exe but may have issues with WinForms reflection-based patterns and WINE's PE loader. Likely not worth the risk for this project.
- Should `CreateProcess` be changed to `CreateProcessW` (Unicode) during the P/Invoke migration? This would better handle non-ASCII file paths under WINE.

---

## Relevant External Documentation

- [Microsoft: Upgrade .NET Framework WinForms to .NET](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/)
- [Microsoft: P/Invoke Source Generation](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation)
- [Microsoft: Breaking Changes in .NET 8](https://learn.microsoft.com/en-us/dotnet/core/compatibility/8.0)
- [Microsoft: Breaking Changes in .NET 9](https://learn.microsoft.com/en-us/dotnet/core/compatibility/9.0)
- [Microsoft: What's New in WinForms .NET 9](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/whats-new/net90)
- [Microsoft: Single-File Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)
- [Microsoft: CsWin32 Source Generator](https://github.com/microsoft/CsWin32)
- [NuGet: Migrating packages.config to PackageReference](https://learn.microsoft.com/en-us/nuget/consume-packages/migrate-packages-config-to-package-reference)
- [GitHub: Proton .NET/WinForms Issues](https://github.com/ValveSoftware/Proton/labels/.NET-winforms)
- [WineHQ: .NET 8 Console Issues](https://forum.winehq.org/viewtopic.php?t=39029)
- [GitHub: LibraryImport Migration Discussion](https://github.com/dotnet/runtime/issues/75052)
- [Vortice.Windows (SharpDX replacement)](https://github.com/amerkoleci/Vortice.Windows)
