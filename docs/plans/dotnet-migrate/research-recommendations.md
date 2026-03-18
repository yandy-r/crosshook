# Recommendations: dotnet-migrate

## Executive Summary

The most pragmatic migration path for ChooChoo Loader is a **conservative, phased migration to .NET 8 LTS with WinForms, remaining a WINE-hosted Windows binary**. The core Win32 P/Invoke operations (kernel32.dll process injection, memory manipulation, remote thread creation) are fundamentally Windows-only -- they cannot be ported to native Linux without rewriting the entire injection engine using ptrace/LD_PRELOAD, which would be a different application entirely. The biggest wins come from modernizing the project structure (SDK-style csproj), separating the 2800-line MainForm monolith into a proper Core/UI architecture, adding a test harness, and adopting self-contained deployment to eliminate end-user .NET runtime installation headaches in WINE/Proton prefixes.

## Implementation Recommendations

### Recommended Approach

**Conservative Migration (Option A)**: Migrate to .NET 8 LTS with WinForms, keeping the application as a Windows binary that runs under WINE/Proton. This preserves all existing functionality, minimizes risk, and delivers the most value for the least effort. The kernel32.dll P/Invoke surface area is well-supported in .NET 8, and WinForms is a first-class citizen on modern .NET for Windows targets.

The migration should be paired with an architectural refactoring that separates the core engine (ProcessManager, InjectionManager, MemoryManager) from the UI layer (MainForm), enabling future UI framework swaps without touching business logic.

### Technology Choices

| Component        | Recommendation                                             | Rationale                                                                                                                                                                                                                                                  |
| ---------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Target Framework | .NET 8 LTS (net8.0-windows)                                | LTS support through Nov 2026; .NET 9 is STS and expires May 2026. .NET 10 LTS arrives Nov 2025 and could be a follow-up target.                                                                                                                            |
| UI Framework     | WinForms (keep)                                            | Avalonia would eliminate the WINE dependency but cannot call kernel32.dll P/Invoke natively on Linux. Since the app _must_ run in WINE anyway for process injection, WinForms is the simplest and cheapest choice.                                         |
| P/Invoke         | LibraryImport (source generator) + CsWin32 for type safety | LibraryImport replaces DllImport with compile-time marshalling code, enables Native AOT compatibility, and produces debuggable generated code. CsWin32 provides correct struct layouts, constants, and SafeHandle patterns from official Windows metadata. |
| Deployment       | Self-contained single-file publish                         | Eliminates the need for users to install .NET Desktop Runtime inside their WINE/Proton prefix -- the single biggest friction point in the current setup.                                                                                                   |
| Build System     | SDK-style csproj + dotnet CLI                              | Replaces the legacy MSBuild csproj. Enables `dotnet build`, `dotnet publish`, NuGet PackageReference, and modern CI/CD.                                                                                                                                    |
| Configuration    | System.Text.Json or TOML                                   | Replace hand-rolled INI parsing (settings.ini, AppSettings.ini, .profile files) with a proper serialization format.                                                                                                                                        |
| Testing          | xUnit + Moq/NSubstitute                                    | The project currently has zero tests. The Core layer can be tested in isolation once separated from the UI.                                                                                                                                                |

### Phasing Strategy

1. **Phase 1 - Foundation (1-2 weeks)**: Convert project to SDK-style csproj targeting net8.0-windows. Replace packages.config (SharpDX) with PackageReference. Verify the application builds with `dotnet build` and runs identically under WINE. No behavioral changes.

2. **Phase 2 - Architecture (2-3 weeks)**: Extract Core/UI separation. Move ProcessManager, InjectionManager, MemoryManager into a `ChooChooEngine.Core` class library project. Define interfaces (IProcessManager, IInjectionManager, IMemoryManager). Break MainForm.cs (2800 lines) into smaller focused components. Replace INI file parsing with a proper settings model. Add xUnit test project with initial tests for the Core layer.

3. **Phase 3 - Modernization (1-2 weeks)**: Migrate DllImport declarations to LibraryImport source generators. Consider adopting CsWin32 for type-safe Win32 API access. Enable nullable reference types. Adopt file-scoped namespaces and other C# 12 language features. Configure self-contained single-file publish. Set up CI/CD pipeline (GitHub Actions).

### Quick Wins

- **SDK-style csproj conversion**: Immediately enables `dotnet build`, `dotnet publish`, and modern tooling. Impact: High -- unblocks all other modernization.
- **Self-contained publish**: `dotnet publish -r win-x64 --self-contained -p:PublishSingleFile=true` produces a single exe that requires zero .NET runtime installation in the WINE prefix. Impact: Critical -- resolves the most common user setup failure.
- **Nullable reference types**: Catches null-related bugs at compile time. The current code returns `null` from multiple MemoryManager methods without clear contracts. Impact: Medium -- improves code safety.
- **Remove SharpDX dependency**: The packages.config references SharpDX 4.2.0 but no code in the codebase appears to use it (no XInput/controller code exists in the source files). If confirmed unused, removing it simplifies the dependency graph. Impact: Low -- but reduces attack surface.

## Improvement Ideas

### Architecture Improvements

- **Core/UI Separation**: The MainForm.cs file is 2800 lines and owns all state, all event wiring, all profile management, all command-line parsing, and all UI layout. Extract: (1) a `ProfileService` to handle .profile and settings.ini file operations, (2) a `CommandLineParser` for -p, -autolaunch, -dllinject argument processing, (3) a `LaunchOrchestrator` that coordinates ProcessManager + InjectionManager for the launch workflow, (4) a `RecentFilesService` for MRU list management. Each of these is independently testable. The MainForm should only wire UI events to service calls.

- **Testing Infrastructure**: Create a `ChooChooEngine.Core.Tests` xUnit project. The ProcessManager, InjectionManager, and MemoryManager all depend on Win32 P/Invoke calls, so unit testing requires wrapping P/Invoke behind interfaces (e.g., `INativeProcessApi`) that can be mocked. The profile/settings/MRU/command-line logic has no P/Invoke dependency and can be tested immediately once extracted from MainForm.

- **Proper Dispose Pattern**: ProcessManager holds an unmanaged `_processHandle` (IntPtr from `OpenProcess`) but does not implement IDisposable. If the consumer forgets to call `DetachFromProcess()`, the handle leaks. Implementing IDisposable with a destructor/finalizer ensures cleanup.

- **Event Subscription Duplication**: In MainForm.cs, `InitializeManagers()` (lines 274-283) and `RegisterEventHandlers()` (lines 1370-1381) both subscribe to the same ProcessManager and InjectionManager events, resulting in double event firing. This is a bug that should be fixed during migration.

### Future Enhancements

- **Structured Logging**: Replace `LogToConsole()` string concatenation with a proper logging framework (Microsoft.Extensions.Logging or Serilog). Enables file logging, log levels, and structured output for debugging WINE issues.
- **Async/Await for Launch Operations**: The `BtnLaunch_Click` handler is synchronous on the UI thread. Long-running launch operations (especially `LaunchWithCmd` which calls `WaitForExit()`) can freeze the UI. Converting to async would improve responsiveness.
- **Proper Error Types**: Replace `bool` return values with Result<T> pattern or exceptions. Currently, `LaunchProcess()` returns `false` for any failure with no indication of what went wrong (permission denied? file not found? WINE compatibility issue?).
- **Profile Format Upgrade**: The custom key=value profile format lacks validation, versioning, and extensibility. Migrating to JSON (System.Text.Json) would provide schema validation, better error messages, and forward compatibility.

## Risk Assessment

### Technical Risks

| Risk                                        | Likelihood | Impact   | Mitigation                                                                                                                                                                                                                                                                                                                            |
| ------------------------------------------- | ---------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| .NET 8 CoreCLR runtime crash under WINE     | Medium     | Critical | WINE 9.x has partial .NET 8 support but known issues with console encoding and wow64 mode. Self-contained publish avoids runtime installation issues. Test extensively on target Proton versions (GE-Proton, vanilla Proton 9+). Fall back to .NET Framework 4.8 build target if blockers found.                                      |
| P/Invoke behavioral changes (marshalling)   | Low        | High     | The kernel32.dll calls used (OpenProcess, CreateRemoteThread, WriteProcessMemory, VirtualAllocEx) have stable, well-defined signatures. LibraryImport changes `CharSet` to `StringMarshalling` and drops ANSI default -- the `LoadLibraryA` call in InjectionManager explicitly uses ASCII encoding, so this needs careful migration. |
| WinForms rendering differences under WINE   | Low        | Medium   | WinForms on .NET 8 uses the same GDI+ rendering pipeline as .NET Framework 4.8 on Windows. Under WINE, both are translated by the same WINE GDI layer, so rendering should be equivalent. The custom dark theme uses basic controls (Panel, Button, Label, ComboBox) that WINE handles well.                                          |
| Self-contained publish EXE size             | Low        | Low      | A self-contained WinForms app is approximately 60-80 MB. The current choochoo.exe is 80 KB because it depends on the system .NET Framework. The size increase is a trade-off for eliminating runtime installation. Trimming can reduce this to 30-40 MB.                                                                              |
| SharpDX removal breaks hidden functionality | Low        | Medium   | SharpDX 4.2.0 is listed in packages.config but grep shows zero XInput/controller/gamepad references in source code. The README mentions XInput support, suggesting it may have been removed from source but left in packages.config. Verify by building without it.                                                                   |
| Native AOT incompatibility                  | Medium     | Low      | Native AOT does not support WinForms. This rules out AOT as a deployment option. Self-contained + ReadyToRun is the best alternative for startup performance.                                                                                                                                                                         |

### Integration Challenges

- **WINE Proton version matrix**: Different Proton versions (vanilla, GE-Proton, Proton Experimental) bundle different WINE versions with varying .NET 8 compatibility. The self-contained publish approach mitigates this by not depending on an installed runtime, but CoreCLR itself still needs WINE to load and execute correctly.
- **CreateProcess under WINE**: The `LaunchWithCreateProcess` method uses raw `kernel32.CreateProcess` P/Invoke. Under WINE, this calls WINE's CreateProcess implementation, which translates to fork/exec on Linux. The `LaunchWithCmd` method shells out to `cmd.exe`, which under WINE is WINE's cmd.exe stub. Both should continue working, but edge cases with path translation (Windows paths vs. Linux paths in the WINE prefix) may surface.
- **MiniDumpWriteDump**: This Dbghelp.dll import may have incomplete WINE support. It is used in `ProcessManager.CreateMiniDump()` and may silently fail under WINE without producing an error. This is existing behavior, not a regression risk.
- **Placeholder injection methods**: `LaunchWithCreateThreadInjection` and `LaunchWithRemoteThreadInjection` are placeholders that both fall through to `LaunchWithCreateProcess`. These dead code paths should be either implemented or removed during migration.

## Alternative Approaches

### Option A: Conservative Migration (WinForms on .NET 8, still WINE)

- **Pros**: Lowest risk. All existing P/Invoke code works unchanged. WinForms is fully supported on .NET 8 for Windows targets. Self-contained deployment solves the biggest user pain point (runtime installation). Familiar codebase for contributors. Enables modern C# features (nullable, pattern matching, file-scoped namespaces).
- **Cons**: Still requires WINE/Proton. WinForms is in maintenance mode (no new features from Microsoft). UI remains Windows-native aesthetic. Cannot leverage native Linux capabilities (direct ptrace, native file dialogs).
- **Effort**: 4-7 weeks for full migration including architecture refactoring.
- **Risk Level**: Low.

### Option B: Native Linux with Avalonia UI

- **Pros**: Eliminates WINE dependency for the UI layer. Native Linux/macOS/Windows support. Modern XAML-based UI framework with active development. Better rendering, theming, and accessibility. True cross-platform distribution.
- **Cons**: The entire P/Invoke surface area (CreateRemoteThread, WriteProcessMemory, VirtualAllocEx, LoadLibrary, OpenProcess) is Windows-only. These are not "portability concerns" -- they are fundamental Windows kernel APIs with no Linux equivalent. To inject DLLs into Windows game processes on Linux, you _still need WINE_. The game processes run inside WINE, so the injector must also run inside WINE (or use WINE's API surface). Going native means rewriting the injection engine to use ptrace/LD_PRELOAD, which only works for native Linux processes, not WINE-hosted Windows game processes. This is effectively a different product.
- **Effort**: 3-6 months. Requires a complete rewrite of the injection engine, not just a UI port.
- **Risk Level**: Very High. Fundamentally changes the product architecture. May not be feasible for WINE-hosted game targets.

### Option C: Hybrid (Core library + pluggable UI)

- **Pros**: Clean architecture with interfaces between Core and UI. Future-proofs the codebase for potential UI framework changes. Core library can be tested independently. Could support a CLI mode (no UI at all) for power users and automation.
- **Cons**: Adds architectural complexity for a small project (~3500 lines of code total). The pluggable UI abstraction is premature if Avalonia cannot actually replace WinForms for this use case (see Option B analysis). Over-engineering risk for a project maintained by a small team.
- **Effort**: 5-8 weeks (includes Option A + abstraction layer).
- **Risk Level**: Medium. The abstraction itself is low risk, but it may encourage a future Avalonia migration that is not technically viable (see Option B).

### Recommendation

**Option A (Conservative Migration)** is the clear winner. The fundamental constraint is that ChooChoo Loader injects DLLs into Windows game processes running under WINE. This requires the injector itself to run inside the same WINE environment, using Windows APIs. No amount of native Linux porting changes this. The migration should focus on: (1) modern .NET tooling and language features, (2) self-contained deployment, (3) architectural cleanup to make the code maintainable, and (4) a test harness.

Borrow the best idea from Option C -- separating Core from UI via interfaces -- as part of Phase 2, but do not build a pluggable UI abstraction. Simply extract the logic from MainForm.cs into service classes behind interfaces, primarily for testability.

## Task Breakdown Preview

### Phase 1: Foundation

- **Task group**: Project file modernization.
  - Convert `ChooChooEngine.App.csproj` from legacy format to SDK-style targeting `net8.0-windows`.
  - Replace `packages.config` with PackageReference.
  - Remove or verify SharpDX dependency.
  - Remove `AssemblyInfo.cs` (properties move to csproj).
  - Verify `dotnet build` and `dotnet run` produce a working application.
  - Test the built EXE under WINE/Proton on Linux.
  - Configure self-contained single-file publish (`dotnet publish -r win-x64 --self-contained -p:PublishSingleFile=true`).
- **Parallel opportunities**: Self-contained publish testing can run alongside csproj conversion verification. CI/CD pipeline setup (GitHub Actions) can proceed in parallel.

### Phase 2: Core Implementation

- **Task group**: Architecture refactoring and code modernization.
  - Create `ChooChooEngine.Core` class library project.
  - Move ProcessManager, InjectionManager, MemoryManager to Core project.
  - Define interfaces: IProcessManager, IInjectionManager, IMemoryManager.
  - Extract `ProfileService` from MainForm.cs profile load/save logic.
  - Extract `CommandLineParser` from MainForm.cs argument processing.
  - Extract `LaunchOrchestrator` for launch workflow coordination.
  - Extract `RecentFilesService` for MRU management.
  - Fix double event subscription bug (InitializeManagers + RegisterEventHandlers).
  - Implement IDisposable on ProcessManager for handle cleanup.
  - Replace INI parsing with System.Text.Json settings model.
  - Enable nullable reference types (`<Nullable>enable</Nullable>`).
  - Adopt C# 12 features: file-scoped namespaces, primary constructors where appropriate.
  - Break MainForm.cs into smaller partial classes or user controls.
- **Dependencies**: Phase 1 must complete first. Core extraction and UI refactoring can proceed in parallel once the project builds on .NET 8.

### Phase 3: Integration and Testing

- **Task group**: P/Invoke modernization, testing, and polish.
  - Create `ChooChooEngine.Core.Tests` xUnit project.
  - Add unit tests for ProfileService, CommandLineParser, LaunchOrchestrator, RecentFilesService.
  - Wrap P/Invoke calls behind `INativeProcessApi` interface for mockability.
  - Migrate DllImport to LibraryImport source generators.
  - Evaluate CsWin32 for struct/constant generation.
  - Add integration test that verifies the app launches under WINE in CI.
  - Configure ReadyToRun compilation for improved startup time.
  - Update CLAUDE.md and AGENTS.md build instructions.
  - Update README.md with new setup instructions (no runtime installation needed).
  - Performance testing: compare startup time and memory usage between .NET Framework 4.8 and .NET 8 self-contained builds.

## Key Decisions Needed

- **.NET 8 vs .NET 10 LTS**: .NET 8 is LTS with support through Nov 2026. .NET 10 LTS arrives Nov 2025 (already available). Targeting .NET 8 is safer today; upgrading to .NET 10 later is a one-line csproj change. Decision: start with .NET 8, upgrade to .NET 10 as a follow-up.
- **SharpDX removal**: The packages.config lists SharpDX 4.2.0 but no source code references it. If the XInput controller support was removed from source but the package reference was left behind, it should be removed. If XInput support is planned for re-addition, consider using [Vortice.XInput](https://github.com/amerkoleci/Vortice.Windows) instead (SharpDX is archived/unmaintained).
- **Profile format migration**: Changing from .profile/.ini to JSON is a breaking change for existing users. Options: (a) support both formats with a one-time migration on first launch, (b) keep INI format but add validation, (c) switch to JSON and document the breaking change in release notes.
- **CI/CD platform**: GitHub Actions is the natural choice given the repo is on GitHub. The build must target `win-x64` and ideally test under WINE in CI. Decision: use `windows-latest` runner for builds, optionally add an `ubuntu-latest` runner with WINE for smoke testing.

## Open Questions

- Has anyone tested the specific combination of self-contained .NET 8 WinForms + CreateRemoteThread DLL injection under Proton GE? The self-contained CoreCLR may initialize differently than an installed runtime, potentially affecting process handle inheritance.
- Is the XInput/controller support mentioned in the README still desired? If so, what replaced SharpDX in the source code, or was it removed entirely?
- What Proton versions should be the minimum supported baseline? Proton 9+ is recommended in the README, but GE-Proton and Proton Experimental have different WINE versions with different .NET 8 compatibility.
- Should the `-dllinject` command-line argument (mentioned in README but not implemented in the current `ProcessCommandLineArguments()` method) be implemented as part of this migration?
- Is the `ManualMapping` injection method (currently a placeholder that falls through to `StandardInjection`) planned for implementation, or should it be removed?

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/ChooChooEngine.App.csproj`: Legacy-format project file targeting .NET Framework 4.8 -- primary migration target.
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Program.cs`: Entry point with Mutex single-instance and WinForms bootstrap (45 lines, minimal changes needed).
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Core/ProcessManager.cs`: 506 lines, 11 DllImport declarations, 2 Win32 structs, process lifecycle management. Core extraction candidate.
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Injection/InjectionManager.cs`: 354 lines, 12 DllImport declarations, LoadLibraryA-based DLL injection. Core extraction candidate. Contains the `LoadLibraryA` string that needs attention during LibraryImport migration (ASCII encoding).
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Memory/MemoryManager.cs`: 369 lines, 3 DllImport declarations, memory read/write/save/restore. Core extraction candidate.
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Forms/MainForm.cs`: 2800-line monolith containing all UI, state management, event handling, profile management, settings, and command-line parsing. Primary refactoring target.
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Forms/MainForm.Designer.cs`: Minimal designer file (53 lines) -- most UI is constructed in MainForm.cs code-behind.
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/UI/ResumePanel.cs`: 108-line custom Panel with proper IDisposable pattern. Good example of the target quality level.
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/packages.config`: Contains only SharpDX 4.2.0 reference (possibly unused).
- `/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Properties/AssemblyInfo.cs`: Assembly metadata that moves into csproj during SDK-style migration.

## Other Docs

- [P/Invoke source generation (LibraryImport) - Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation)
- [CsWin32 - Microsoft source generator for Win32 APIs](https://github.com/microsoft/CsWin32)
- [.NET Upgrade Assistant Overview](https://learn.microsoft.com/en-us/dotnet/core/porting/upgrade-assistant-overview)
- [Native AOT deployment overview](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/) (not applicable to WinForms, but relevant for understanding constraints)
- [Self-contained single file deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)
- [Avalonia UI (for future reference, not recommended for this migration)](https://avaloniaui.net/)
- [WINE .NET 8 console issues](https://forum.winehq.org/viewtopic.php?t=39029)
- [WinForms .NET 8/9 coreclr.dll crash reports under WINE](https://github.com/dotnet/runtime/issues/112598)
- [Proton .NET Framework support issues](https://github.com/ValveSoftware/Proton/issues/1786)
- [.NET Migration Guide: Framework 4.8 to .NET 10](https://wojciechowski.app/en/articles/dotnet-migration-guide)
