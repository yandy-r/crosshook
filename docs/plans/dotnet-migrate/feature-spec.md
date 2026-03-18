# Feature Spec: dotnet-migrate

## Executive Summary

ChooChoo Loader can be migrated from .NET Framework 4.8 to modern .NET (8 or 9) with moderate effort. All 19 unique Win32 P/Invoke calls (kernel32.dll, Dbghelp.dll) are fully compatible with .NET 8/9 — the ABI is unchanged. The biggest user-facing win is **self-contained single-file deployment**, which eliminates the need to install .NET Framework or wine-mono in the WINE/Proton prefix. The migration involves converting the classic 75-line `.csproj` to a ~30-line SDK-style format, converting `[DllImport]` to `[LibraryImport]` source generators, removing the unused SharpDX dependency, and deleting `AssemblyInfo.cs`/`packages.config`. The app must remain a Windows binary running under WINE — true native Linux is not feasible because DLL injection into WINE-hosted game processes requires Windows kernel APIs. A key architectural decision is whether to keep WinForms (minimal migration effort) or migrate to Avalonia UI (native Linux rendering, larger effort, but eliminates WINE for the UI layer).

## External Dependencies

### APIs and Services

#### .NET 9 SDK / Runtime

- **Documentation**: <https://learn.microsoft.com/en-us/dotnet/core/whats-new/dotnet-9>
- **WinForms Migration Guide**: <https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/>
- **P/Invoke Source Generation**: <https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation>
- **Key Feature**: Self-contained single-file publish bundles the runtime into the exe, so WINE never needs a .NET installation

#### CsWin32 Source Generator (Optional)

- **Documentation**: <https://github.com/microsoft/CsWin32>
- **Purpose**: Auto-generates type-safe P/Invoke wrappers from Windows metadata
- **Status**: Beta (0.3.x). Not required — manual `[LibraryImport]` conversion is simpler for 19 APIs

#### Avalonia UI (If UI migration chosen)

- **Documentation**: <https://docs.avaloniaui.net/>
- **Platforms**: <https://docs.avaloniaui.net/docs/overview/supported-platforms>
- **Purpose**: Cross-platform .NET UI framework with native Linux rendering via Skia
- **Validation**: NexusMods.App (production game modding tool) uses Avalonia on Linux/Steam Deck

### Libraries and SDKs

| Library              | Version | Purpose                                 | Installation                                        |
| -------------------- | ------- | --------------------------------------- | --------------------------------------------------- |
| Microsoft.NET.Sdk    | 9.0     | SDK-style project support               | Built into .NET 9 SDK                               |
| System.Windows.Forms | 9.0     | WinForms (if keeping)                   | `<UseWindowsForms>true</UseWindowsForms>` in csproj |
| Avalonia             | 11.x    | Cross-platform UI (if migrating)        | `dotnet new avalonia.app`                           |
| Vortice.XInput       | 2.x     | XInput replacement for SharpDX (future) | `dotnet add package Vortice.XInput`                 |

### External Documentation

- [Upgrade .NET Framework WinForms to .NET](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/): Microsoft's official migration guide
- [P/Invoke Source Generation](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation): LibraryImport attribute documentation
- [Single-File Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview): Self-contained publish documentation
- [NuGet: packages.config to PackageReference](https://learn.microsoft.com/en-us/nuget/consume-packages/migrate-packages-config-to-package-reference): Migration guide
- [BepInEx Proton/WINE Guide](https://docs.bepinex.dev/articles/advanced/proton_wine.html): DLL injection patterns under WINE

## Business Requirements

### User Stories

**Primary User: Linux/Steam Deck Gamer**

- As a Linux gamer, I want ChooChoo to work without installing .NET Framework in my WINE bottle so that setup is simpler
- As a Steam Deck user, I want a single executable that just works under Proton so that I can launch modded games from Gaming Mode
- As a Proton user, I want all existing functionality (game launch, trainer launch, DLL injection, profiles) to work identically after migration

**Secondary User: Fork Maintainer**

- As a fork maintainer, I want to use `dotnet build`/`dotnet publish` so that CI/CD is simpler
- As a fork maintainer, I want modern C# features (nullable, pattern matching, file-scoped namespaces) for cleaner code
- As a fork maintainer, I want SDK-style projects so that dependency management uses NuGet PackageReference

### Business Rules

1. **All P/Invoke functionality must be preserved**: The 19 Win32 API calls (OpenProcess, CreateRemoteThread, WriteProcessMemory, VirtualAllocEx, LoadLibrary, etc.) are the core product. Zero regressions allowed.
2. **Single-instance enforcement via Mutex must work under WINE**: The named Mutex `ChooChooEngineInjectorSingleInstance` must prevent duplicate instances.
3. **Profile backwards compatibility**: Existing `.profile` and `settings.ini` files must continue to load correctly.
4. **Command-line interface preserved**: `-p "ProfileName"` and `-autolaunch <path>` must work identically.
5. **DLL architecture validation**: 32/64-bit PE header checking must continue to prevent mismatched injections.
6. **Thread-safe injection**: The `_injectionLock` serialization must be preserved.

### Edge Cases

| Scenario                                              | Expected Behavior                                | Notes                                        |
| ----------------------------------------------------- | ------------------------------------------------ | -------------------------------------------- |
| Existing .NET Framework 4.8 build alongside new build | Both should work in separate WINE prefixes       | Self-contained avoids conflicts              |
| WINE prefix without wine-mono                         | New build works (self-contained bundles runtime) | Major improvement over current               |
| Non-ASCII file paths under WINE                       | Must work with UTF-16 CreateProcessW             | Migration should switch from ANSI to Unicode |
| Self-contained exe on older Proton (< 9)              | May not work; document minimum Proton version    | Test matrix needed                           |

### Success Criteria

- [ ] Project builds with `dotnet build` targeting .NET 9
- [ ] SDK-style `.csproj` replaces classic project format
- [ ] All 19 P/Invoke functions work correctly under WINE/Proton
- [ ] DLL injection (LoadLibraryA + CreateRemoteThread) succeeds end-to-end on a real game under Proton
- [ ] Self-contained single-file publish produces a working exe without pre-installed .NET in WINE prefix
- [ ] Profile save/load/delete works correctly
- [ ] Command-line arguments (-p, -autolaunch) work correctly
- [ ] Single-instance Mutex enforcement works under WINE
- [ ] Unused SharpDX dependency removed
- [ ] No functional regression compared to .NET Framework 4.8 build

## Technical Specifications

### Architecture Overview

```
[Current Architecture]
Program.cs → MainForm (2800 lines: UI + state + logic + profiles + settings + CLI args)
               ├── ProcessManager (kernel32 P/Invoke: launch, attach, suspend, resume, kill)
               ├── InjectionManager (kernel32 P/Invoke: DLL injection via LoadLibraryA/CreateRemoteThread)
               ├── MemoryManager (kernel32 P/Invoke: read/write process memory)
               └── ResumePanel (custom WinForms overlay)

[Post-Migration Target]
Program.cs → MainForm (slimmed: UI event wiring only)
               ├── ProfileService (extracted from MainForm)
               ├── CommandLineParser (extracted from MainForm)
               ├── LaunchOrchestrator (extracted from MainForm)
               ├── RecentFilesService (extracted from MainForm)
               ├── ProcessManager (P/Invoke modernized: LibraryImport + IDisposable)
               ├── InjectionManager (P/Invoke modernized: LibraryImport)
               ├── MemoryManager (P/Invoke modernized: LibraryImport)
               └── ResumePanel (unchanged)
```

### P/Invoke Migration Matrix

All 19 unique Win32 API calls, with migration path:

| API Call            | DLL          | Used In                                         | .NET 9 Status | Migration                                                   |
| ------------------- | ------------ | ----------------------------------------------- | ------------- | ----------------------------------------------------------- |
| OpenProcess         | kernel32.dll | ProcessManager, InjectionManager                | Works         | `[LibraryImport]`, deduplicate                              |
| CloseHandle         | kernel32.dll | ProcessManager, InjectionManager                | Works         | `[LibraryImport]`, deduplicate                              |
| CreateRemoteThread  | kernel32.dll | ProcessManager, InjectionManager                | Works         | `[LibraryImport]`, deduplicate                              |
| WriteProcessMemory  | kernel32.dll | ProcessManager, InjectionManager, MemoryManager | Works         | `[LibraryImport]`, deduplicate                              |
| VirtualAllocEx      | kernel32.dll | ProcessManager, InjectionManager                | Works         | `[LibraryImport]`, deduplicate                              |
| VirtualFreeEx       | kernel32.dll | ProcessManager, InjectionManager                | Works         | `[LibraryImport]`, deduplicate                              |
| OpenThread          | kernel32.dll | ProcessManager                                  | Works         | `[LibraryImport]`                                           |
| SuspendThread       | kernel32.dll | ProcessManager                                  | Works         | `[LibraryImport]`                                           |
| ResumeThread        | kernel32.dll | ProcessManager                                  | Works         | `[LibraryImport]`                                           |
| CreateProcess       | kernel32.dll | ProcessManager                                  | Works         | `[LibraryImport]` + `StringMarshalling.Utf16`               |
| MiniDumpWriteDump   | Dbghelp.dll  | ProcessManager                                  | Partial WINE  | `[LibraryImport]` (may produce incomplete dumps under WINE) |
| GetProcAddress      | kernel32.dll | InjectionManager                                | Works         | `[LibraryImport]` + `StringMarshalling.Utf8`                |
| GetModuleHandle     | kernel32.dll | InjectionManager                                | Works         | `[LibraryImport]`                                           |
| LoadLibrary         | kernel32.dll | InjectionManager                                | Works         | `[LibraryImport]` + `StringMarshalling.Utf16`               |
| FreeLibrary         | kernel32.dll | InjectionManager                                | Works         | `[LibraryImport]`                                           |
| WaitForSingleObject | kernel32.dll | InjectionManager                                | Works         | `[LibraryImport]`                                           |
| GetExitCodeThread   | kernel32.dll | InjectionManager                                | Works         | `[LibraryImport]`                                           |
| ReadProcessMemory   | kernel32.dll | MemoryManager                                   | Works         | `[LibraryImport]`                                           |
| VirtualQueryEx      | kernel32.dll | MemoryManager                                   | Works         | `[LibraryImport]`                                           |

**Note**: 6 APIs are duplicated across ProcessManager and InjectionManager (29 total declaration sites for 19 unique APIs). These should be consolidated during migration.

### SDK-Style .csproj (Post-Migration)

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
    <Version>6.0.0</Version>

    <!-- Self-contained for WINE/Proton (no runtime installation needed) -->
    <RuntimeIdentifier>win-x64</RuntimeIdentifier>
    <SelfContained>true</SelfContained>
    <PublishSingleFile>true</PublishSingleFile>
    <IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>
    <EnableCompressionInSingleFile>true</EnableCompressionInSingleFile>
  </PropertyGroup>
</Project>
```

### Build Commands (Post-Migration)

```bash
# Build
dotnet build src/ChooChooEngine.sln -c Release

# Publish self-contained single-file exe
dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj \
  -c Release -r win-x64 --self-contained -p:PublishSingleFile=true

# Output: src/ChooChooEngine.App/bin/Release/net9.0-windows/win-x64/publish/choochoo.exe
```

### System Integration

#### Files to Create

- `src/ChooChooEngine.App/ChooChooEngine.App.csproj` (complete rewrite as SDK-style)
- `src/global.json` (optional: pin .NET 9 SDK version)

#### Files to Modify

- `src/ChooChooEngine.App/Core/ProcessManager.cs`: Add `partial` class, convert 11 `[DllImport]` to `[LibraryImport]`, implement `IDisposable` for handle cleanup
- `src/ChooChooEngine.App/Injection/InjectionManager.cs`: Add `partial` class, convert 11 `[DllImport]` to `[LibraryImport]`, fix `LoadLibraryA` string marshalling
- `src/ChooChooEngine.App/Memory/MemoryManager.cs`: Add `partial` class, convert 3 `[DllImport]` to `[LibraryImport]`
- `src/ChooChooEngine.App/Program.cs`: Add `partial` class (optional)
- `CLAUDE.md`: Update tech stack, build commands

#### Files to Delete

- `src/ChooChooEngine.App/Properties/AssemblyInfo.cs` (replaced by csproj metadata)
- `src/ChooChooEngine.App/packages.config` (SharpDX removed, PackageReference in csproj)
- `src/ChooChooEngine.App/bin/` and `obj/` (clean rebuild required)

## UX Considerations

### The Key UI Framework Decision

There is a fundamental tension in this migration:

**Option A: Keep WinForms (Conservative)**

- Minimal code changes, preserves existing UI
- App still runs entirely under WINE
- Self-contained deployment avoids .NET runtime installation in WINE
- Risk: .NET 8/9 WinForms under WINE is less tested than .NET Framework 4.8 via wine-mono
- Effort: Low (weeks)

**Option B: Migrate to Avalonia UI (Ambitious)**

- Native Linux rendering (no WINE for the UI layer)
- Validated by NexusMods.App (production game modding tool on Linux)
- Hybrid architecture: native UI + WINE helper process for injection
- Every successful Linux game tool (Lutris, Heroic, Bottles) uses native UI
- Risk: Near-100% UI rewrite, complex hybrid WINE architecture
- Effort: High (months)

**Recommendation**: Start with Option A (WinForms migration) to deliver self-contained deployment quickly. Architect with Core/UI separation so Option B (Avalonia) becomes viable later without touching business logic.

### Competitive Landscape

| Tool                   | UI Framework           | Linux UI Layer            | Game Execution |
| ---------------------- | ---------------------- | ------------------------- | -------------- |
| Lutris                 | Python/GTK             | Native                    | WINE/Proton    |
| Heroic                 | Electron               | Native                    | WINE/Proton    |
| Bottles                | Python/GTK4            | Native                    | WINE           |
| NexusMods.App          | C#/Avalonia            | Native                    | Native + WINE  |
| Vortex                 | Electron               | WINE (problematic)        | WINE/Proton    |
| **ChooChoo (current)** | **C#/WinForms**        | **WINE**                  | **WINE**       |
| **ChooChoo (Phase 1)** | **C#/WinForms .NET 9** | **WINE (self-contained)** | **WINE**       |
| **ChooChoo (Future)**  | **C#/Avalonia**        | **Native**                | **WINE**       |

### Performance Impact

| Metric          | .NET Framework 4.8 (wine-mono) | .NET 9 (self-contained, WINE) |
| --------------- | ------------------------------ | ----------------------------- |
| Cold start      | 1-3 seconds                    | 2-5 seconds (larger binary)   |
| Warm start      | <1 second                      | 1-2 seconds                   |
| Exe size        | ~80 KB (needs runtime)         | ~60-80 MB (self-contained)    |
| Runtime install | Required in WINE prefix        | Not needed                    |

### Steam Deck Considerations

- Self-contained exe simplifies Proton setup (no winetricks needed)
- Current compact mode layout (950px threshold) works at 1280x800
- Touch targets should be increased to 48x48px minimum for trackpad interaction
- Controller navigation not currently supported (SharpDX/XInput is unused)

## Recommendations

### Implementation Approach

**Recommended Strategy**: Conservative phased migration with Core/UI separation

**Phasing:**

1. **Phase 1 - Foundation**: Convert to SDK-style .csproj, verify builds with `dotnet build`, configure self-contained publish. Zero behavioral changes. Test under WINE/Proton.
2. **Phase 2 - Architecture**: Extract services from MainForm.cs (ProfileService, CommandLineParser, LaunchOrchestrator, RecentFilesService). Fix bugs found during research. Implement IDisposable on ProcessManager. Add xUnit test project.
3. **Phase 3 - Modernization**: Convert DllImport to LibraryImport. Enable nullable reference types. Adopt C# 12 features. Consolidate duplicate P/Invoke declarations. Update documentation.

### Technology Decisions

| Decision         | Recommendation                        | Rationale                                                                                                                                                     |
| ---------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Target Framework | `net9.0-windows`                      | Self-contained deployment makes STS/LTS irrelevant. .NET 9 has better LibraryImport source generation. Retarget to .NET 10 LTS (Nov 2025) is a 1-line change. |
| UI Framework     | WinForms (keep, for now)              | Minimal migration risk. Core/UI separation enables future Avalonia migration without touching business logic.                                                 |
| P/Invoke         | `[LibraryImport]` (manual conversion) | Compile-time marshalling, AOT-compatible, debuggable. Only 19 APIs — CsWin32 is overkill.                                                                     |
| Deployment       | Self-contained single-file            | Eliminates #1 user pain point (WINE runtime installation).                                                                                                    |
| SharpDX          | Remove entirely                       | Not referenced in any source file. Archived/unmaintained project.                                                                                             |
| WinForms Init    | Keep explicit `EnableVisualStyles()`  | More transparent than `ApplicationConfiguration.Initialize()` for WINE compatibility.                                                                         |

### Quick Wins

- **Self-contained publish**: Eliminates .NET runtime installation in WINE — the biggest user friction point
- **SDK-style csproj**: Enables `dotnet build`/`dotnet publish` and modern tooling
- **Remove SharpDX**: Zero-risk cleanup (not referenced in source)
- **Nullable reference types**: Catches null-related bugs at compile time (many null-returning methods exist)

### Bugs Found During Research

These should be fixed during migration:

1. **Double event subscription**: `InitializeManagers()` and `RegisterEventHandlers()` in MainForm.cs both subscribe to the same ProcessManager/InjectionManager events, causing handlers to fire twice
2. **Handle leak**: ProcessManager holds unmanaged `_processHandle` but doesn't implement `IDisposable`
3. **Dead code stubs**: `LaunchWithCreateThreadInjection`, `LaunchWithRemoteThreadInjection`, `InjectDllManualMapping` all fall through to their default implementations
4. **Missing CLI feature**: `-dllinject` is documented in README but not implemented in `ProcessCommandLineArguments()`

### Future Enhancements

- Avalonia UI migration (once Core/UI separation is done)
- Structured logging (replace `Debug.WriteLine` with Serilog or Microsoft.Extensions.Logging)
- Async launch operations (prevent UI freezes during long operations)
- Profile format upgrade (JSON instead of .ini for validation and extensibility)
- XInput controller support via Vortice.XInput (if desired)
- CI/CD pipeline with WINE smoke tests

## Risk Assessment

### Technical Risks

| Risk                                              | Likelihood | Impact   | Mitigation                                                                                                                 |
| ------------------------------------------------- | ---------- | -------- | -------------------------------------------------------------------------------------------------------------------------- |
| .NET 9 CoreCLR crash under WINE                   | Medium     | Critical | Self-contained avoids runtime install issues. Test on Proton 9+, GE-Proton. Fall back to .NET Framework build if blockers. |
| P/Invoke marshalling behavioral changes           | Low        | High     | All kernel32 APIs have stable signatures. `LoadLibraryA` ASCII encoding needs explicit StringMarshalling.Utf8.             |
| WinForms rendering differences under WINE         | Low        | Medium   | Same GDI+ pipeline. Custom dark theme uses basic controls WINE handles well.                                               |
| Self-contained exe size increase (80KB → 60-80MB) | Low        | Low      | Acceptable trade-off for eliminating runtime installation requirement.                                                     |
| SharpDX removal breaks hidden functionality       | Low        | Medium   | Grep confirms zero source references. Build without it to verify.                                                          |

### Integration Challenges

- **WINE Proton version matrix**: Different Proton versions bundle different WINE versions. Self-contained mitigates runtime issues, but CoreCLR itself still needs WINE to execute correctly.
- **MiniDumpWriteDump**: WINE's Dbghelp.dll has limited support. May produce incomplete dumps. Pre-existing limitation.
- **CreateProcess path translation**: Windows paths vs Linux paths in WINE prefix may surface edge cases.

### Security Considerations

- Never commit `.env`, `.env.keys`, or `.env.encrypted` files
- P/Invoke with `PROCESS_ALL_ACCESS` is intentional for the product's core functionality
- Self-contained deployment avoids dependency on potentially outdated wine-mono

## Task Breakdown Preview

### Phase 1: Foundation

**Focus**: Get the project building on .NET 9 with zero behavioral changes
**Tasks**:

- Convert `.csproj` to SDK-style targeting `net9.0-windows`
- Delete `AssemblyInfo.cs`, `packages.config`
- Clean `bin/` and `obj/` directories
- Remove SharpDX dependency
- Verify `dotnet build` succeeds
- Configure self-contained single-file publish
- Test under WINE/Proton on Arch Linux
  **Parallelization**: CI/CD setup can proceed alongside manual testing

### Phase 2: Architecture

**Focus**: Extract business logic from MainForm.cs, fix bugs, add tests
**Dependencies**: Phase 1 must complete
**Tasks**:

- Extract ProfileService, CommandLineParser, LaunchOrchestrator, RecentFilesService from MainForm.cs
- Implement IDisposable on ProcessManager
- Fix double event subscription bug
- Remove or implement stub methods (CreateThreadInjection, RemoteThreadInjection, ManualMapping)
- Create xUnit test project with tests for extracted services
  **Parallelization**: Service extraction and bug fixes can run in parallel

### Phase 3: Modernization

**Focus**: P/Invoke modernization, C# language features, documentation
**Dependencies**: Phase 2 should be substantially complete
**Tasks**:

- Convert all 19 `[DllImport]` to `[LibraryImport]` with `partial` methods
- Consolidate duplicate P/Invoke declarations across files
- Enable nullable reference types
- Adopt C# 12 features (file-scoped namespaces, target-typed new)
- Update CLAUDE.md, README.md with new build instructions
- Performance comparison: startup time and memory vs .NET Framework 4.8

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **.NET 8 LTS vs .NET 9 STS**
   - Options: .NET 8 (LTS, support through Nov 2026) or .NET 9 (STS, better LibraryImport)
   - Impact: Self-contained deployment makes lifecycle irrelevant. .NET 9 has better source generation.
   - Recommendation: .NET 9, with plan to retarget to .NET 10 LTS when comfortable

2. **UI Framework Strategy**
   - Options: (A) WinForms only, (B) WinForms now + Avalonia later, (C) Avalonia immediately
   - Impact: Option C requires 3-6 month UI rewrite and hybrid WINE architecture
   - Recommendation: Option B — migrate WinForms first, architect for future Avalonia

3. **Profile Format**
   - Options: (A) Keep .ini format, (B) Migrate to JSON, (C) Support both with auto-migration
   - Impact: JSON is better for validation but breaks existing user profiles
   - Recommendation: Keep .ini for now, JSON as a future enhancement

4. **Stub Methods**
   - Options: Remove dead stubs or implement them
   - Impact: Removing simplifies codebase; implementing adds features
   - Recommendation: Remove unless there's a specific need

5. **P/Invoke Consolidation**
   - Options: (A) Consolidate into shared NativeMethods class, (B) Keep per-component declarations
   - Impact: Consolidation reduces duplication (6 APIs x 2 files) but changes code organization
   - Recommendation: Consolidate into a shared `NativeMethods` static partial class

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): .NET 8/9 migration APIs, CsWin32, WINE compatibility, LibraryImport patterns
- [research-business.md](./research-business.md): Core functionality inventory, P/Invoke dependency map, cross-platform feasibility analysis
- [research-technical.md](./research-technical.md): Complete P/Invoke migration matrix, SDK-style .csproj template, WinForms control compatibility
- [research-ux.md](./research-ux.md): UI framework comparison (WinForms/Avalonia/Terminal/MAUI), competitive analysis, Steam Deck UX
- [research-recommendations.md](./research-recommendations.md): Migration strategy options, risk assessment, bug inventory, phasing plan
