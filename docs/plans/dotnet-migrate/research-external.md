# External API Research: dotnet-migrate

> Background research only. If this file conflicts with `feature-spec.md` or `parallel-plan.md`, follow those newer plan documents.

## Executive Summary

Migrating CrossHook Loader from .NET Framework 4.8 to .NET 9 (LTS: .NET 8) is technically feasible but requires careful handling of three areas: (1) converting the classic .csproj to SDK-style with `net9.0-windows` target and `UseWindowsForms`, (2) migrating ~20 Win32 P/Invoke declarations from `[DllImport]` to the modern `[LibraryImport]` source generator or adopting CsWin32 for type-safe generated bindings, and (3) validating that the self-contained Windows executable continues to function under WINE/Proton -- particularly the DLL injection flow using `CreateRemoteThread`/`VirtualAllocEx`/`WriteProcessMemory`, which has known reliability issues under WINE regardless of .NET version. The migration itself is straightforward for the codebase's size; the WINE/Proton runtime compatibility is the highest-risk dimension.

**Confidence**: High (migration path is well-documented by Microsoft; WINE compatibility is the uncertain factor)

---

## Primary APIs

### .NET 8/9 Migration Path

- **Documentation**: [Upgrade a .NET Framework WinForms app to .NET](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/) | [WinForms Porting Guidelines](https://github.com/dotnet/winforms/blob/main/docs/porting-guidelines.md)
- **Migration Guide (community)**: [.NET Migration Guide: Framework 4.8 to .NET 10](https://wojciechowski.app/en/articles/dotnet-migration-guide) | [Zenkins Step-by-Step](https://zenkins.com/insights/migrating-from-net-framework-to-net-8/)

#### Key Changes from .NET Framework to Modern .NET

| Area                | .NET Framework 4.8                                      | .NET 8/9                                                                 | Impact                                                          |
| ------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------- |
| Project format      | Classic verbose .csproj                                 | SDK-style lean .csproj                                                   | Must convert; file globbing replaces explicit `<Compile>` items |
| Target framework    | `<TargetFrameworkVersion>v4.8</TargetFrameworkVersion>` | `<TargetFramework>net9.0-windows</TargetFramework>`                      | Simple property change                                          |
| WinForms enablement | `System.Windows.Forms` assembly reference               | `<UseWindowsForms>true</UseWindowsForms>`                                | Property in .csproj                                             |
| Assembly info       | `Properties/AssemblyInfo.cs`                            | Auto-generated (or `<GenerateAssemblyInfo>false</GenerateAssemblyInfo>`) | Delete or disable auto-gen                                      |
| NuGet               | `packages.config`                                       | `<PackageReference>` in .csproj                                          | Must convert format                                             |
| Default font        | Microsoft Sans Serif, 8.25pt                            | Segoe UI, 9pt (~27% larger)                                              | Layout may break; use `<ApplicationDefaultFont>` to preserve    |
| P/Invoke            | `[DllImport]` with runtime IL stub                      | `[LibraryImport]` source-generated (recommended)                         | Optional but recommended migration                              |
| AllowUnsafeBlocks   | Per-configuration property                              | Project-level property                                                   | Already used; keep it                                           |

**Confidence**: High -- Microsoft official documentation covers this comprehensively.

#### P/Invoke Compatibility

The existing `[DllImport]` declarations will continue to work in .NET 8/9 without modification. `DllImport` is fully supported and not deprecated. However, Microsoft recommends migrating to `[LibraryImport]` for:

- Compile-time source generation (no runtime IL stub)
- Better AOT/trimming support
- Debuggable marshalling code
- Improved performance

**Key behavioral differences when migrating to `[LibraryImport]`:**

| DllImport Property  | LibraryImport Equivalent        | Notes                                    |
| ------------------- | ------------------------------- | ---------------------------------------- |
| `CharSet`           | `StringMarshalling`             | ANSI removed; UTF-8 is first-class       |
| `CallingConvention` | `[UnmanagedCallConv]` attribute | Separate attribute                       |
| `ExactSpelling`     | No equivalent                   | Method name / `EntryPoint` must be exact |
| `BestFitMapping`    | No equivalent                   | Only relevant for ANSI strings           |
| `PreserveSig`       | No equivalent                   | Signature always preserved               |
| Method modifier     | `static partial` (not `extern`) | Source generator requirement             |

**Critical note**: Windows API functions with A/W suffix variants (e.g., `CreateProcessA`/`CreateProcessW`) require explicit `EntryPoint` specification. The current code uses `CreateProcess` without suffix -- with `LibraryImport`, this will need `EntryPoint = "CreateProcessA"` or `EntryPoint = "CreateProcessW"`.

**Confidence**: High -- based on [Microsoft P/Invoke source generation docs](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation) and [migration discussion](https://github.com/dotnet/runtime/issues/75052).

#### WinForms Support in .NET 8/9

WinForms is fully supported on .NET 8 and .NET 9, but only on Windows (the `net9.0-windows` TFM). Key considerations:

- **Designer**: Uses out-of-process designer architecture in VS 2022. The designer runs in a separate `DesignToolsServer.exe` process matching the target .NET version. Custom control designers must use the `Microsoft.WinForms.Designer.SDK` NuGet package.
- **Default font**: Changed from Microsoft Sans Serif 8.25pt to Segoe UI 9pt. Forms will render ~27% larger. Fix: add `<ApplicationDefaultFont>Microsoft Sans Serif, 8.25pt</ApplicationDefaultFont>` to .csproj, or call `Application.SetDefaultFont()` in `Main()`.
- **32-bit COM/ActiveX**: Fully supported in .NET 8+ on 64-bit VS 2022.
- **DataBinding**: Object Data Sources recommended; Typed DataSets have limited support.

**Confidence**: High -- [WinForms migration docs](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/) and [designer differences](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/controls-design/designer-differences-framework).

---

### CsWin32 Source Generator

- **Documentation**: [GitHub - microsoft/CsWin32](https://github.com/microsoft/CsWin32) | [DeepWiki Usage Guide](https://deepwiki.com/microsoft/CsWin32/3-usage-guide) | [DeepWiki Configuration](https://deepwiki.com/microsoft/CsWin32/3.2-configuration-options)
- **NuGet**: [Microsoft.Windows.CsWin32 0.3.269](https://www.nuget.org/packages/Microsoft.Windows.CsWin32)
- **Latest release**: v0.3.269 (January 2026)

#### Purpose

CsWin32 is a C# source generator by Microsoft that generates type-safe, compile-time P/Invoke bindings from Win32 metadata. Instead of manually declaring `[DllImport]` or `[LibraryImport]` signatures with manually defined structs and constants, you list the API names you need and CsWin32 generates correct, complete bindings including:

- P/Invoke `extern` methods with proper signatures
- All required structs, enums, constants, and delegates
- `SafeHandle`-derived types for handle management
- Friendly overloads with idiomatic C# signatures
- XML documentation from Microsoft Learn

#### Compatibility

- .NET 5+ SDK required (for source generator support)
- C# 9+ language version required
- Also works with .NET Framework 4.5+ and .NET Standard 2.0 (with additional packages)
- `AllowUnsafeBlocks` must be enabled

#### Setup

```xml
<!-- .csproj -->
<PackageReference Include="Microsoft.Windows.CsWin32" Version="0.3.269">
  <PrivateAssets>all</PrivateAssets>
</PackageReference>
```

**NativeMethods.txt** (list APIs one per line):

```
OpenProcess
CloseHandle
CreateRemoteThread
WriteProcessMemory
ReadProcessMemory
VirtualAllocEx
VirtualFreeEx
VirtualQueryEx
GetProcAddress
GetModuleHandle
LoadLibrary
FreeLibrary
CreateProcess
OpenThread
SuspendThread
ResumeThread
WaitForSingleObject
GetExitCodeThread
MiniDumpWriteDump
```

**NativeMethods.json** (configuration):

```json
{
  "$schema": "https://aka.ms/CsWin32.schema.json",
  "public": false,
  "allowMarshaling": true,
  "useSafeHandles": true,
  "friendlyOverloads": {
    "enabled": true
  }
}
```

#### Generated API Access

After building, generated APIs are accessible through the `Windows.Win32.PInvoke` class:

```csharp
using Windows.Win32;
using Windows.Win32.System.Threading;
using Windows.Win32.System.Memory;

// SafeHandle-based, type-safe API
var processHandle = PInvoke.OpenProcess(
    PROCESS_ACCESS_RIGHTS.PROCESS_ALL_ACCESS,
    false,
    (uint)processId);
```

**Confidence**: High -- Microsoft-maintained, actively developed, 2.5k+ stars, 2.3k dependent projects.

---

### .NET Upgrade Assistant

- **Documentation**: [.NET Upgrade Assistant Overview](https://learn.microsoft.com/en-us/dotnet/core/porting/upgrade-assistant-overview) | [Install Guide](https://learn.microsoft.com/en-us/dotnet/core/porting/upgrade-assistant-install)
- **Status**: The standalone CLI tool is being deprecated. Microsoft now recommends the GitHub Copilot modernization chat agent in Visual Studio 2022 17.14+.

#### Capabilities

- Converts classic .csproj to SDK-style
- Updates `TargetFramework` to modern .NET
- Migrates `packages.config` to `PackageReference`
- Identifies incompatible APIs
- Generates assessment reports
- Updates namespace references

#### Limitations for This Project

- **Small codebase**: CrossHook Loader has ~7 source files. Manual migration is likely faster and more predictable than running the Upgrade Assistant.
- **P/Invoke heavy**: The tool does not convert `[DllImport]` to `[LibraryImport]` or CsWin32. This must be done manually.
- **No WINE/Proton awareness**: The tool has no concept of cross-platform WINE deployment.

**Recommendation**: Skip the Upgrade Assistant for this project. Manual migration is more appropriate given the small codebase size and specialized P/Invoke requirements.

**Confidence**: High -- based on [official docs](https://learn.microsoft.com/en-us/dotnet/core/porting/upgrade-assistant-overview) and [deprecation notice](https://www.gapvelocity.ai/blog/what-happened-to-the-.net-upgrade-assistant).

---

## Libraries and SDKs

### Recommended Libraries

#### 1. Microsoft.Windows.CsWin32 (Strongly Recommended)

- **Why**: Eliminates all manual P/Invoke declarations. Generates correct, type-safe bindings with SafeHandle support. Zero runtime dependencies.
- **Install**: `dotnet add package Microsoft.Windows.CsWin32 --version 0.3.269`
- **Docs**: [github.com/microsoft/CsWin32](https://github.com/microsoft/CsWin32)
- **Impact**: Replaces all 20+ manual `[DllImport]` declarations and hand-coded structs (`MEMORY_BASIC_INFORMATION`, `STARTUPINFO`, `PROCESS_INFORMATION`) with generated, verified bindings.

#### 2. Microsoft.Windows.Compatibility (Recommended)

- **Why**: Provides Windows-specific APIs that were moved out of the core .NET runtime. Includes `System.Drawing`, registry access, and other BCL APIs that WinForms apps commonly use.
- **Install**: `dotnet add package Microsoft.Windows.Compatibility`
- **Docs**: [Cross-platform targeting](https://learn.microsoft.com/en-us/dotnet/standard/library-guidance/cross-platform-targeting)

### Alternative Options

#### PInvoke.Kernel32 (dotnet/pinvoke)

- **Status**: No longer maintained. CsWin32 is the official replacement.
- **NuGet**: [PInvoke.Kernel32 0.7.124](https://www.nuget.org/packages/PInvoke.Kernel32/)
- **Recommendation**: Do not adopt. Use CsWin32 instead.

**Confidence**: High.

#### Vanara.PInvoke.Kernel32

- **NuGet**: [Vanara.PInvoke.Kernel32 4.2.1](https://www.nuget.org/packages/Vanara.PInvoke.Kernel32)
- **Pros**: Community-maintained, comprehensive Win32 coverage, good documentation.
- **Cons**: Ships runtime assemblies (unlike CsWin32 which generates at compile time). Adds deployment dependency.
- **Recommendation**: Viable alternative if CsWin32 source generation causes issues, but CsWin32 is preferred.

**Confidence**: Medium -- Vanara is well-regarded but CsWin32 has official Microsoft backing.

#### Manual LibraryImport (No External Library)

- **Pros**: Zero dependencies. Full control. Simple for small number of APIs.
- **Cons**: Must manually define all structs, constants, and function signatures. Error-prone.
- **Recommendation**: Acceptable fallback if CsWin32 is rejected. The project has ~20 P/Invoke declarations, which is manageable manually.

**Confidence**: High.

---

## Integration Patterns

### SDK-Style Project Migration

The current classic .csproj (75 lines) would convert to an SDK-style .csproj of approximately 15-20 lines.

#### Current Classic Format (abbreviated)

```xml
<?xml version="1.0" encoding="utf-8"?>
<Project ToolsVersion="15.0" xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <Import Project="$(MSBuildExtensionsPath)\..." />
  <PropertyGroup>
    <OutputType>WinExe</OutputType>
    <TargetFrameworkVersion>v4.8</TargetFrameworkVersion>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <!-- ... 30+ more lines ... -->
  </PropertyGroup>
  <ItemGroup>
    <Reference Include="System" />
    <Reference Include="System.Windows.Forms" />
    <!-- ... explicit file references ... -->
  </ItemGroup>
  <ItemGroup>
    <Compile Include="Core\ProcessManager.cs" />
    <Compile Include="Forms\MainForm.cs" />
    <!-- ... every file listed ... -->
  </ItemGroup>
</Project>
```

#### Target SDK-Style Format

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>WinExe</OutputType>
    <TargetFramework>net9.0-windows</TargetFramework>
    <UseWindowsForms>true</UseWindowsForms>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <RootNamespace>CrossHookEngine.App</RootNamespace>
    <AssemblyName>crosshook</AssemblyName>
    <ApplicationDefaultFont>Microsoft Sans Serif, 8.25pt</ApplicationDefaultFont>
    <GenerateAssemblyInfo>false</GenerateAssemblyInfo>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.Windows.CsWin32" Version="0.3.269">
      <PrivateAssets>all</PrivateAssets>
    </PackageReference>
  </ItemGroup>
</Project>
```

Key changes:

1. Remove all `<Compile Include="...">` items -- SDK-style auto-includes all `.cs` files via globbing.
2. Remove all `<Reference Include="System...">` items -- BCL references are implicit.
3. Remove `<Import>` elements -- handled by the SDK.
4. Add `<UseWindowsForms>true</UseWindowsForms>` instead of referencing `System.Windows.Forms`.
5. Delete `Properties/AssemblyInfo.cs` or set `<GenerateAssemblyInfo>false</GenerateAssemblyInfo>`.
6. Delete `packages.config` -- use `<PackageReference>` instead.
7. Set `<ApplicationDefaultFont>` to preserve the .NET Framework font and avoid layout changes.

**Confidence**: High -- [Scott Hanselman's guide](https://www.hanselman.com/blog/upgrading-an-existing-net-project-files-to-the-lean-new-csproj-format-from-net-core), [WinForms porting guidelines](https://github.com/dotnet/winforms/blob/main/docs/porting-guidelines.md).

### Multi-Targeting (.NET Framework 4.8 + .NET 9)

Multi-targeting is possible but not recommended for this project:

```xml
<PropertyGroup>
  <TargetFrameworks>net48;net9.0-windows</TargetFrameworks>
  <UseWindowsForms>true</UseWindowsForms>
</PropertyGroup>
```

With preprocessor directives for API differences:

```csharp
#if NET9_0_OR_GREATER
    [LibraryImport("kernel32.dll")]
    private static partial IntPtr OpenProcess(uint dwDesiredAccess,
        [MarshalAs(UnmanagedType.Bool)] bool bInheritHandle, uint dwProcessId);
#else
    [DllImport("kernel32.dll")]
    private static extern IntPtr OpenProcess(int dwDesiredAccess,
        bool bInheritHandle, int dwProcessId);
#endif
```

**Recommendation**: Avoid multi-targeting. This project is a standalone application (not a library consumed by others). A clean cut-over to .NET 9 is simpler, avoids conditional compilation complexity, and the existing .NET Framework 4.8 binary can remain available on the `main` branch history.

**Confidence**: High.

### P/Invoke Migration Patterns

#### Option A: CsWin32 (Recommended)

Replace all manual P/Invoke regions with CsWin32-generated code. This eliminates ~150 lines of boilerplate declarations across `ProcessManager.cs`, `InjectionManager.cs`, and `MemoryManager.cs`.

**Before** (current code in `InjectionManager.cs`):

```csharp
[DllImport("kernel32.dll")]
private static extern IntPtr OpenProcess(int dwDesiredAccess,
    bool bInheritHandle, int dwProcessId);

[DllImport("kernel32.dll")]
private static extern IntPtr VirtualAllocEx(IntPtr hProcess, IntPtr lpAddress,
    uint dwSize, uint flAllocationType, uint flProtect);

[StructLayout(LayoutKind.Sequential)]
private struct MEMORY_BASIC_INFORMATION { /* ... */ }

private const int PROCESS_ALL_ACCESS = 0x1F0FFF;
private const uint MEM_COMMIT = 0x1000;
```

**After** (with CsWin32):

```csharp
using Windows.Win32;
using Windows.Win32.System.Threading;
using Windows.Win32.System.Memory;
using Windows.Win32.Foundation;

// All constants, structs, and function signatures are generated.
// Use directly:
var handle = PInvoke.OpenProcess(
    PROCESS_ACCESS_RIGHTS.PROCESS_ALL_ACCESS, false, (uint)pid);

var allocatedMemory = PInvoke.VirtualAllocEx(
    handle, null, size,
    VIRTUAL_ALLOCATION_TYPE.MEM_COMMIT | VIRTUAL_ALLOCATION_TYPE.MEM_RESERVE,
    PAGE_PROTECTION_FLAGS.PAGE_READWRITE);
```

#### Option B: Manual LibraryImport Migration

**Before:**

```csharp
[DllImport("kernel32.dll")]
private static extern IntPtr OpenProcess(int dwDesiredAccess,
    bool bInheritHandle, int dwProcessId);
```

**After:**

```csharp
[LibraryImport("kernel32.dll")]
private static partial IntPtr OpenProcess(uint dwDesiredAccess,
    [MarshalAs(UnmanagedType.Bool)] bool bInheritHandle, uint dwProcessId);
```

Key changes for each declaration:

1. Replace `[DllImport]` with `[LibraryImport]`
2. Replace `extern` with `partial`
3. Add `[MarshalAs(UnmanagedType.Bool)]` for `bool` parameters (LibraryImport does not marshal `bool` by default)
4. For string parameters: add `StringMarshalling = StringMarshalling.Utf16` or use `[MarshalAs]`
5. For functions with A/W variants: specify `EntryPoint` explicitly

**Confidence**: High -- [P/Invoke source generation docs](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation).

---

### WINE/Proton Compatibility

This is the highest-risk area of the migration. The analysis is divided into three layers.

#### Layer 1: .NET Runtime under WINE

| Approach                                    | Compatibility | Notes                                                                                                                                                |
| ------------------------------------------- | ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| .NET Framework 4.8 on Wine Mono             | Good          | Wine Mono is a replacement for .NET Framework in Wine; actively maintained                                                                           |
| .NET Framework 4.8 installed via winetricks | Good          | `winetricks dotnet48` is well-tested                                                                                                                 |
| .NET 8/9 runtime installed via winetricks   | Problematic   | `winetricks dotnetdesktop8` has [reported failures](https://github.com/Winetricks/winetricks/issues/2178); apps may not detect the installed runtime |
| .NET 8/9 self-contained deployment          | Best option   | Bundles the runtime; no installation needed in Wine prefix                                                                                           |

**Recommendation**: Publish as self-contained, single-file executable targeting `win-x64`:

```bash
dotnet publish -c Release -r win-x64 \
  --self-contained true \
  /p:PublishSingleFile=true \
  /p:IncludeNativeLibrariesForSelfExtract=true
```

This eliminates the need to install the .NET runtime in the Wine prefix entirely. The published `.exe` contains the full .NET 9 runtime and runs directly under WINE/Proton.

**Confidence**: Medium -- Self-contained .NET 8 executables have been reported to work under WINE for console apps, but WinForms-specific compatibility with the self-contained runtime under WINE has limited documentation.

#### Layer 2: WinForms Rendering under WINE

WinForms rendering under WINE depends on two paths:

1. **Wine Mono WinForms** (for .NET Framework apps): Wine Mono includes its own WinForms implementation targeting X11. This is mature but diverges from upstream .NET WinForms.

2. **.NET 8/9 WinForms** (for modern .NET apps): The self-contained executable bundles the Windows WinForms implementation from the .NET Windows Desktop runtime. WINE must translate the underlying GDI/User32 calls. This generally works but has edge cases.

**Key consideration**: The current .NET Framework 4.8 app running under Wine Mono uses Wine's own WinForms implementation. After migration to .NET 9 self-contained, it would use the real Windows WinForms implementation translated through WINE's Win32 API layer. This is architecturally different and may produce different rendering behavior.

**Confidence**: Medium -- WinForms on WINE is functional but may have rendering differences.

#### Layer 3: Process Injection APIs under WINE

This is where the most significant risks lie. The project uses the following kernel32 functions for DLL injection:

| API                               | WINE Implementation   | Known Issues                                                                                                |
| --------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------- |
| `OpenProcess`                     | Implemented           | Generally works                                                                                             |
| `CloseHandle`                     | Implemented           | Generally works                                                                                             |
| `VirtualAllocEx`                  | Implemented           | [Tests exist in WINE source](https://github.com/wine-mirror/wine/blob/master/dlls/kernel32/tests/virtual.c) |
| `WriteProcessMemory`              | Implemented           | [Tests exist in WINE source](https://github.com/wine-mirror/wine/blob/master/dlls/kernel32/tests/process.c) |
| `ReadProcessMemory`               | Implemented           | Generally works                                                                                             |
| `VirtualQueryEx`                  | Implemented           | Generally works                                                                                             |
| `CreateRemoteThread`              | Implemented           | **Known segfault issues**; [WINE forum reports](https://forum.winehq.org/viewtopic.php?t=5925)              |
| `LoadLibrary` / `GetProcAddress`  | Implemented           | Works for Windows DLLs in the prefix                                                                        |
| `GetModuleHandle`                 | Implemented           | Generally works                                                                                             |
| `CreateProcess`                   | Implemented           | Generally works                                                                                             |
| `SuspendThread` / `ResumeThread`  | Implemented           | Generally works                                                                                             |
| `MiniDumpWriteDump` (Dbghelp.dll) | Partially implemented | May have limitations                                                                                        |

**Critical WINE limitation for DLL injection**:

1. **`CreateRemoteThread` can segfault** under WINE. The thread created by `CreateRemoteThread` may crash because WINE's thread creation does not fully replicate Windows thread initialization semantics.

2. **Linux security restrictions**: Injection only works if the injecting process is in the parent process tree of the target. Since CrossHook Loader uses `CreateProcess` to launch the game, this should be satisfied -- but only when using `LaunchMethod.CreateProcess`, not `ShellExecute` or `CmdStart`.

3. **DLL path resolution**: Under WINE/Proton, DLLs in the game directory are not loaded instead of system DLLs by default. The `WINEDLLOVERRIDES` environment variable may be needed.

**Important**: These WINE limitations exist regardless of whether the app runs on .NET Framework 4.8 or .NET 9. The migration to modern .NET does not worsen or improve the WINE DLL injection situation. The kernel32 functions are called through the same WINE compatibility layer either way.

**Confidence**: Medium -- WINE implements these APIs but with known reliability issues for the injection pattern specifically. This is inherent to WINE, not to the .NET version.

---

## Constraints and Gotchas

### 1. Default Font Change Breaks Layout

- **Impact**: Forms render ~27% larger with Segoe UI 9pt vs Microsoft Sans Serif 8.25pt.
- **Workaround**: Add `<ApplicationDefaultFont>Microsoft Sans Serif, 8.25pt</ApplicationDefaultFont>` to .csproj.
- **Confidence**: High.

### 2. `AssemblyInfo.cs` Conflicts

- **Impact**: SDK-style projects auto-generate assembly attributes. The existing `Properties/AssemblyInfo.cs` will cause duplicate attribute errors.
- **Workaround**: Either delete `AssemblyInfo.cs` and configure properties in .csproj, or set `<GenerateAssemblyInfo>false</GenerateAssemblyInfo>`.
- **Confidence**: High.

### 3. SafeHandle Constructor Requirement (.NET 8+)

- **Impact**: If using CsWin32 with SafeHandle-based APIs, SafeHandle-derived types must have a **public** parameterless constructor (changed in .NET 8).
- **Workaround**: CsWin32 handles this automatically. If using manual P/Invoke, ensure any custom SafeHandle types have public constructors.
- **Confidence**: High -- [Breaking change docs](https://learn.microsoft.com/en-us/dotnet/core/compatibility/interop/8.0/safehandle-constructor).

### 4. `bool` Marshalling in LibraryImport

- **Impact**: `[LibraryImport]` does not automatically marshal `bool` as 4-byte `BOOL`. You must add `[MarshalAs(UnmanagedType.Bool)]` to bool parameters, or the function call will fail silently.
- **Workaround**: Annotate all `bool` parameters with `[MarshalAs(UnmanagedType.Bool)]`.
- **Confidence**: High.

### 5. `CreateProcess` A/W Variant Resolution

- **Impact**: The current code declares `CreateProcess` without specifying `ExactSpelling`. .NET Framework's `DllImport` automatically tries both `CreateProcessA` and `CreateProcessW`. `LibraryImport` does not -- it uses the exact method name.
- **Workaround**: Specify `EntryPoint = "CreateProcessW"` (or `"CreateProcessA"`) explicitly. CsWin32 handles this automatically.
- **Confidence**: High.

### 6. WINE Self-Contained .NET 9 Runtime

- **Impact**: While self-contained deployment avoids installing .NET in the Wine prefix, the bundled runtime itself must work under WINE. .NET 8/9 runtime initialization has been [reported to have issues](https://forum.winehq.org/viewtopic.php?t=39029) with WINE, particularly around console initialization.
- **Workaround**: Test thoroughly. If runtime initialization fails under WINE, consider using a framework-dependent deployment with the .NET runtime installed via winetricks, or remaining on .NET Framework 4.8 for WINE deployment while using .NET 9 for native Windows.
- **Confidence**: Low -- limited documentation on self-contained .NET 8/9 WinForms under WINE.

### 7. WinForms Single-File Publishing

- **Impact**: WinForms requires native libraries that may not work when embedded in a single-file executable. The `IncludeNativeLibrariesForSelfExtract=true` flag is needed to extract native libraries at runtime.
- **Workaround**: Set `<IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>` in .csproj or publish command. See [dotnet/winforms#11473](https://github.com/dotnet/winforms/issues/11473).
- **Confidence**: High.

### 8. `packages.config` Must Be Removed

- **Impact**: The current project has a `packages.config` file. SDK-style projects use `<PackageReference>` exclusively.
- **Workaround**: The file currently lists no packages (it's likely empty or vestigial). Simply delete it.
- **Confidence**: High.

### 9. Obsolete Assembly References

- **Impact**: Several explicit assembly references in the current .csproj (`System.Data.DataSetExtensions`, `Microsoft.CSharp`, `System.Net.Http`, `System.Deployment`) are either unnecessary in modern .NET or included automatically.
- **Workaround**: Remove all explicit `<Reference>` items. They are either implicit in the SDK or not used by this project.
- **Confidence**: High -- per [WinForms porting guidelines](https://github.com/dotnet/winforms/blob/main/docs/porting-guidelines.md).

---

## Code Examples

### Complete Modern P/Invoke Pattern (Manual LibraryImport)

```csharp
using System.Runtime.InteropServices;

namespace CrossHookEngine.App.Interop;

/// <summary>
/// Kernel32 P/Invoke declarations using modern LibraryImport source generation.
/// </summary>
internal static partial class NativeMethods
{
    // Process management
    [LibraryImport("kernel32.dll", SetLastError = true)]
    internal static partial IntPtr OpenProcess(
        uint dwDesiredAccess,
        [MarshalAs(UnmanagedType.Bool)] bool bInheritHandle,
        uint dwProcessId);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool CloseHandle(IntPtr hObject);

    // Memory operations
    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool ReadProcessMemory(
        IntPtr hProcess,
        IntPtr lpBaseAddress,
        [Out] byte[] lpBuffer,
        uint nSize,
        out nuint lpNumberOfBytesRead);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool WriteProcessMemory(
        IntPtr hProcess,
        IntPtr lpBaseAddress,
        byte[] lpBuffer,
        uint nSize,
        out nuint lpNumberOfBytesWritten);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    internal static partial IntPtr VirtualAllocEx(
        IntPtr hProcess,
        IntPtr lpAddress,
        uint dwSize,
        uint flAllocationType,
        uint flProtect);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool VirtualFreeEx(
        IntPtr hProcess,
        IntPtr lpAddress,
        uint dwSize,
        uint dwFreeType);

    [LibraryImport("kernel32.dll")]
    internal static partial IntPtr VirtualQueryEx(
        IntPtr hProcess,
        IntPtr lpAddress,
        out MEMORY_BASIC_INFORMATION lpBuffer,
        uint dwLength);

    // DLL injection
    [LibraryImport("kernel32.dll", StringMarshalling = StringMarshalling.Utf16)]
    internal static partial IntPtr GetModuleHandle(string lpModuleName);

    [LibraryImport("kernel32.dll", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr GetProcAddress(IntPtr hModule, string procName);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    internal static partial IntPtr CreateRemoteThread(
        IntPtr hProcess,
        IntPtr lpThreadAttributes,
        uint dwStackSize,
        IntPtr lpStartAddress,
        IntPtr lpParameter,
        uint dwCreationFlags,
        IntPtr lpThreadId);

    [LibraryImport("kernel32.dll", SetLastError = true,
        StringMarshalling = StringMarshalling.Utf16)]
    internal static partial IntPtr LoadLibrary(string lpFileName);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool FreeLibrary(IntPtr hModule);

    // Thread management
    [LibraryImport("kernel32.dll")]
    internal static partial IntPtr OpenThread(
        uint dwDesiredAccess,
        [MarshalAs(UnmanagedType.Bool)] bool bInheritHandle,
        uint dwThreadId);

    [LibraryImport("kernel32.dll")]
    internal static partial uint SuspendThread(IntPtr hThread);

    [LibraryImport("kernel32.dll")]
    internal static partial uint ResumeThread(IntPtr hThread);

    // Synchronization
    [LibraryImport("kernel32.dll", SetLastError = true)]
    internal static partial uint WaitForSingleObject(IntPtr hHandle, uint dwMilliseconds);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool GetExitCodeThread(IntPtr hThread, out uint lpExitCode);

    // Process creation
    [LibraryImport("kernel32.dll", SetLastError = true, EntryPoint = "CreateProcessW",
        StringMarshalling = StringMarshalling.Utf16)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool CreateProcess(
        string? lpApplicationName,
        string? lpCommandLine,
        IntPtr lpProcessAttributes,
        IntPtr lpThreadAttributes,
        [MarshalAs(UnmanagedType.Bool)] bool bInheritHandles,
        uint dwCreationFlags,
        IntPtr lpEnvironment,
        string? lpCurrentDirectory,
        in STARTUPINFO lpStartupInfo,
        out PROCESS_INFORMATION lpProcessInformation);

    // Debug help
    [LibraryImport("Dbghelp.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool MiniDumpWriteDump(
        IntPtr hProcess,
        int ProcessId,
        IntPtr hFile,
        int DumpType,
        IntPtr ExceptionParam,
        IntPtr UserStreamParam,
        IntPtr CallbackParam);
}

// Structs remain the same (LayoutKind.Sequential)
[StructLayout(LayoutKind.Sequential)]
internal struct MEMORY_BASIC_INFORMATION
{
    public IntPtr BaseAddress;
    public IntPtr AllocationBase;
    public uint AllocationProtect;
    public IntPtr RegionSize;
    public uint State;
    public uint Protect;
    public uint Type;
}

[StructLayout(LayoutKind.Sequential)]
internal struct STARTUPINFO
{
    public int cb;
    public string lpReserved;
    public string lpDesktop;
    public string lpTitle;
    public int dwX;
    public int dwY;
    public int dwXSize;
    public int dwYSize;
    public int dwXCountChars;
    public int dwYCountChars;
    public int dwFillAttribute;
    public int dwFlags;
    public short wShowWindow;
    public short cbReserved2;
    public IntPtr lpReserved2;
    public IntPtr hStdInput;
    public IntPtr hStdOutput;
    public IntPtr hStdError;
}

[StructLayout(LayoutKind.Sequential)]
internal struct PROCESS_INFORMATION
{
    public IntPtr hProcess;
    public IntPtr hThread;
    public int dwProcessId;
    public int dwThreadId;
}
```

### CsWin32 NativeMethods.txt Example

```
// Process management
OpenProcess
CloseHandle

// Memory operations
ReadProcessMemory
WriteProcessMemory
VirtualAllocEx
VirtualFreeEx
VirtualQueryEx

// DLL loading and injection
GetModuleHandle
GetProcAddress
CreateRemoteThread
LoadLibrary
FreeLibrary

// Thread management
OpenThread
SuspendThread
ResumeThread

// Synchronization
WaitForSingleObject
GetExitCodeThread

// Process creation
CreateProcess

// Debug
MiniDumpWriteDump
```

### SDK-Style .csproj (Complete)

```xml
<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <OutputType>WinExe</OutputType>
    <TargetFramework>net9.0-windows</TargetFramework>
    <UseWindowsForms>true</UseWindowsForms>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <RootNamespace>CrossHookEngine.App</RootNamespace>
    <AssemblyName>crosshook</AssemblyName>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <ApplicationDefaultFont>Microsoft Sans Serif, 8.25pt</ApplicationDefaultFont>
  </PropertyGroup>

  <!-- Self-contained single-file publish settings -->
  <PropertyGroup Condition="'$(Configuration)' == 'Release'">
    <PublishSingleFile>true</PublishSingleFile>
    <SelfContained>true</SelfContained>
    <RuntimeIdentifier>win-x64</RuntimeIdentifier>
    <IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>
    <PublishReadyToRun>true</PublishReadyToRun>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.Windows.CsWin32" Version="0.3.269">
      <PrivateAssets>all</PrivateAssets>
    </PackageReference>
  </ItemGroup>

</Project>
```

### Modern Program.cs Entry Point

```csharp
using CrossHookEngine.App.Forms;

namespace CrossHookEngine.App;

static class Program
{
    private static Mutex? _mutex;
    private const string MutexName = "CrossHookEngineInjectorSingleInstance";

    [STAThread]
    static void Main(string[] args)
    {
        _mutex = new Mutex(true, MutexName, out bool createdNew);

        if (!createdNew)
        {
            MessageBox.Show(
                "CrossHook Injection Engine is already running!",
                "Already Running",
                MessageBoxButtons.OK,
                MessageBoxIcon.Information);
            return;
        }

        try
        {
            Application.SetHighDpiMode(HighDpiMode.SystemAware);
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new MainForm(args));
        }
        finally
        {
            _mutex.ReleaseMutex();
        }
    }
}
```

---

## Deployment Strategy for WINE/Proton

### Recommended: Self-Contained Single-File

```bash
# Build command
dotnet publish src/CrossHookEngine.App/CrossHookEngine.App.csproj \
  -c Release \
  -r win-x64 \
  --self-contained true \
  /p:PublishSingleFile=true \
  /p:IncludeNativeLibrariesForSelfExtract=true \
  /p:PublishReadyToRun=true \
  -o publish/
```

**Output**: A single `crosshook.exe` (~70-100MB) that contains the full .NET 9 runtime. This runs directly under WINE/Proton without installing any .NET runtime in the prefix.

**WINE/Proton launch**:

```bash
# Direct WINE
wine publish/crosshook.exe

# Steam launch options (for Proton)
WINEDLLOVERRIDES="winhttp=n,b" %command%
```

### Fallback: Framework-Dependent

If self-contained deployment has WINE issues, publish framework-dependent and install the runtime:

```bash
dotnet publish -c Release -r win-x64 --self-contained false
# Then install .NET Desktop Runtime 9 in the Wine prefix
```

**Confidence**: Medium -- self-contained is the cleanest approach but has the least WINE testing coverage.

---

## Open Questions

1. **Self-contained .NET 9 WinForms under WINE**: Has anyone successfully run a self-contained .NET 9 WinForms application under Wine 9.x or Proton 9.x? Limited community documentation exists for this specific scenario. **Requires hands-on testing.**

2. **CreateRemoteThread reliability under Proton**: The DLL injection pattern (`VirtualAllocEx` -> `WriteProcessMemory` -> `CreateRemoteThread` -> `LoadLibraryA`) has known segfault issues under WINE. This is not a .NET version issue but a fundamental WINE limitation. **Does the current .NET Framework 4.8 build reliably inject DLLs under Proton?** If not, the migration to .NET 9 will not make this worse.

3. **MiniDumpWriteDump under WINE**: `Dbghelp.dll` support in WINE is partial. The `MiniDumpWriteDump` function may not work correctly. **Is this feature actually used in production?**

4. **CsWin32 SafeHandle vs IntPtr for WINE**: CsWin32 generates SafeHandle-based overloads by default. SafeHandle adds finalizer-based cleanup which is generally better, but adds complexity. For WINE deployment, should `useSafeHandles` be disabled in `NativeMethods.json` to keep the API surface closer to the current IntPtr-based approach?

5. **Wine Mono vs .NET 9 runtime for WinForms**: The current app likely runs under Wine Mono's WinForms implementation. After migration, it would use the .NET 9 Windows Desktop WinForms implementation translated through WINE's GDI/User32 layer. **Which rendering path works better for this specific application?**

6. **Target .NET 8 (LTS) vs .NET 9**: .NET 8 is the current LTS release (supported until November 2026). .NET 9 is STS (supported until May 2026). .NET 10 (next LTS) ships November 2025. **Consider targeting .NET 8 for LTS stability, or wait for .NET 10 LTS.**

---

## Search Queries Executed

1. `.NET Framework 4.8 to .NET 8 migration guide official documentation 2025 2026`
2. `CsWin32 source generator Microsoft.Windows.CsWin32 NuGet P/Invoke .NET 8 modern`
3. `WinForms .NET 8 .NET 9 support Windows Desktop SDK migration from .NET Framework`
4. `.NET Upgrade Assistant tool dotnet upgrade-assistant 2025 capabilities limitations`
5. `P/Invoke DllImport LibraryImport .NET 8 migration breaking changes modern patterns`
6. `WINE Proton .NET 8 runtime compatibility dotnet modern framework Linux Steam Deck`
7. `dotnet .NET 8 running under WINE mono compatibility WinForms P/Invoke kernel32`
8. `WINE kernel32 CreateRemoteThread WriteProcessMemory VirtualAllocEx support compatibility`
9. `classic csproj to SDK-style migration .NET convert project file format`
10. `.NET 8 self-contained publish single file WINE Proton deployment Windows exe`
11. `LibraryImport attribute C# example kernel32 OpenProcess CreateRemoteThread P/Invoke .NET 7 8`
12. `wine-mono .NET Core .NET 8 coexistence WinForms application compatibility 2024 2025`
13. `CsWin32 NativeMethods.txt example OpenProcess WriteProcessMemory VirtualAllocEx CreateRemoteThread`
14. `"dotnet publish" "win-x64" self-contained WinForms WINE run Windows application Linux`
15. `PInvoke.Kernel32 NuGet package dotnet-pinvoke library alternative CsWin32`
16. `WINE DLL injection CreateRemoteThread .NET application game trainer Proton compatibility issues`
17. `BepInEx Wine Proton .NET runtime dotnet framework modern compatibility guide`
18. `.NET 8 9 breaking changes from .NET Framework P/Invoke interop SafeHandle IntPtr migration`
19. `.NET WinForms designer support Visual Studio 2022 .NET 8 9 limitations compared .NET Framework`
20. `.NET 8 WinForms default font change Microsoft Sans Serif Segoe UI migration impact`
21. `.NET 8 runtime install WINE prefix winetricks dotnet desktop runtime Windows`
22. `"net8.0-windows" OR "net9.0-windows" self-contained single file exe WINE Proton run successfully`
23. `.NET multi-targeting net48 net8.0-windows same project TargetFrameworks WinForms P/Invoke`
24. `migrate classic csproj SDK-style WinForms project example before after .NET Framework 4.8`

---

## Sources

### Microsoft Official Documentation

- [Upgrade a .NET Framework WinForms app to .NET](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/) (updated 2025-08-27)
- [P/Invoke source generation (LibraryImport)](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation) (updated 2025-12-04)
- [Native interoperability best practices](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/best-practices)
- [Target frameworks in SDK-style projects](https://learn.microsoft.com/en-us/dotnet/standard/frameworks)
- [Breaking changes in .NET 8](https://learn.microsoft.com/en-us/dotnet/core/compatibility/8.0)
- [Breaking changes in .NET 9](https://learn.microsoft.com/en-us/dotnet/core/compatibility/9.0)
- [SafeHandle breaking change (.NET 8)](https://learn.microsoft.com/en-us/dotnet/core/compatibility/interop/8.0/safehandle-constructor)
- [WinForms designer differences from .NET Framework](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/controls-design/designer-differences-framework)
- [.NET application publishing overview](https://learn.microsoft.com/en-us/dotnet/core/deploying/)
- [Single file deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)
- [.NET Upgrade Assistant Overview](https://learn.microsoft.com/en-us/dotnet/core/porting/upgrade-assistant-overview)

### GitHub Repositories

- [microsoft/CsWin32](https://github.com/microsoft/CsWin32) -- Source generator for Win32 P/Invoke
- [dotnet/winforms porting guidelines](https://github.com/dotnet/winforms/blob/main/docs/porting-guidelines.md) -- Official WinForms migration guide
- [dotnet/pinvoke](https://github.com/dotnet/pinvoke) -- Deprecated P/Invoke library (replaced by CsWin32)
- [wine-mono/wine-mono](https://github.com/madewokherd/wine-mono) -- Wine's .NET Framework replacement
- [DllImport to LibraryImport migration discussion](https://github.com/dotnet/runtime/issues/75052)
- [WINE kernel32 tests (virtual memory)](https://github.com/wine-mirror/wine/blob/master/dlls/kernel32/tests/virtual.c)
- [WINE kernel32 tests (process)](https://github.com/wine-mirror/wine/blob/master/dlls/kernel32/tests/process.c)

### NuGet Packages

- [Microsoft.Windows.CsWin32 0.3.269](https://www.nuget.org/packages/Microsoft.Windows.CsWin32)
- [PInvoke.Kernel32 0.7.124](https://www.nuget.org/packages/PInvoke.Kernel32/) (deprecated)
- [Vanara.PInvoke.Kernel32 4.2.1](https://www.nuget.org/packages/Vanara.PInvoke.Kernel32)

### Community and WINE Resources

- [WineHQ Forums: Injecting code into Windows process under Wine](https://forum.winehq.org/viewtopic.php?t=37212)
- [WineHQ Forums: DLL Injection segfaults on wine](https://forum.winehq.org/viewtopic.php?t=5925)
- [WineHQ Forums: .NET 8 console issues](https://forum.winehq.org/viewtopic.php?t=39029)
- [Winetricks: dotnetdesktop8 not work](https://github.com/Winetricks/winetricks/issues/2178)
- [Winetricks: dotnet8 not recognized by application](https://github.com/Winetricks/winetricks/issues/2276)
- [BepInEx: Running under Proton/Wine](https://docs.bepinex.dev/articles/advanced/proton_wine.html)
- [ProtonDB](https://www.protondb.com/)
- [Scott Hanselman: SDK-style project conversion](https://www.hanselman.com/blog/upgrading-an-existing-net-project-files-to-the-lean-new-csproj-format-from-net-core)
- [CsWin32 DeepWiki Usage Guide](https://deepwiki.com/microsoft/CsWin32/3-usage-guide)
- [CsWin32 DeepWiki Configuration Options](https://deepwiki.com/microsoft/CsWin32/3.2-configuration-options)

---

## Uncertainties and Gaps

1. **No first-hand evidence of .NET 9 self-contained WinForms under WINE**: Web searches returned no definitive success or failure reports for this specific scenario. This is the single biggest unknown. **Confidence: Low.**

2. **MiniDumpWriteDump WINE support depth**: Dbghelp.dll is partially implemented in WINE but the extent of MiniDumpWriteDump support is unclear. **Confidence: Low.**

3. **CreateRemoteThread segfault frequency**: Reports exist of segfaults but also of successful injection under WINE. The reliability appears to depend on WINE version, target process state, and timing. No quantitative data found. **Confidence: Low.**

4. **Wine 9.x / Proton 9.x improvements**: WINE and Proton are actively developed. Recent versions may have improved kernel32 compatibility, but specific changelogs for these APIs were not found. **Confidence: Low.**

5. **PublishReadyToRun under WINE**: ReadyToRun (R2R) pre-compilation may or may not work correctly under WINE's PE loader. If issues arise, disabling R2R is a safe fallback. **Confidence: Medium.**

# Note

This is background research, not the source of truth for the active migration plan. Where it conflicts with `feature-spec.md` or `parallel-plan.md`, follow those newer documents.
