# Context Analysis: dotnet-migrate

## Executive Summary

ChooChoo Loader is a 4,264-line C# WinForms game trainer launcher and DLL injector (.NET Framework 4.8) for Proton/WINE on Linux/Steam Deck. The migration to .NET 9 is a conservative, phased effort across three stages: (1) SDK-style csproj conversion with self-contained single-file publish, (2) extracting 5 services from the 2,800-line MainForm monolith and fixing 4 known bugs, and (3) converting 29 `[DllImport]` declaration sites (19 unique Win32 APIs) to `[LibraryImport]` source generators with P/Invoke consolidation. The app must remain a Windows binary under WINE -- all process injection requires Windows kernel APIs.

## Architecture Context

- **System Structure**: Single-project solution (`ChooChooEngine.App`) with 6 source files organized into 4 layers: Core (ProcessManager -- process lifecycle, 505 lines), Injection (InjectionManager -- DLL injection, 353 lines), Memory (MemoryManager -- process memory R/W, 368 lines), and Forms/UI (MainForm 2,800-line monolith + 107-line ResumePanel). Program.cs (44 lines) is the entry point with Mutex single-instance enforcement.
- **Data Flow**: `Program.Main` -> `MainForm` creates `ProcessManager` (root), then `InjectionManager(processManager)` and `MemoryManager(processManager)` via constructor injection. All inter-component communication is event-driven (`EventHandler<TEventArgs>`). MainForm subscribes to manager events and marshals to UI thread via `InvokeRequired`/`Invoke`. Persistence uses 3 hand-rolled INI-style file formats relative to `Application.StartupPath`.
- **Integration Points**: (1) The 75-line classic `.csproj` is completely rewritten to ~30-line SDK-style. (2) 3 manager classes get `partial` keyword and P/Invoke conversion. (3) `AssemblyInfo.cs` and `packages.config` are deleted. (4) MainForm.cs gets 5 service extractions in Phase 2. (5) `CLAUDE.md`/`AGENTS.md` must be updated with new build commands.

## Critical Files Reference

- `/src/ChooChooEngine.App/ChooChooEngine.App.csproj`: Complete rewrite from classic 75-line to ~30-line SDK-style targeting `net9.0-windows`
- `/src/ChooChooEngine.App/Core/ProcessManager.cs`: 11 P/Invoke declarations, 2 Win32 structs, unmanaged handle leak (needs IDisposable)
- `/src/ChooChooEngine.App/Injection/InjectionManager.cs`: 11 P/Invoke declarations (6 duplicated from ProcessManager), critical `LoadLibraryA` ANSI encoding path
- `/src/ChooChooEngine.App/Memory/MemoryManager.cs`: 3 P/Invoke declarations, 1 struct (MEMORY_BASIC_INFORMATION)
- `/src/ChooChooEngine.App/Forms/MainForm.cs`: 2,800-line monolith -- all UI, state, profiles, settings, CLI parsing, launch orchestration; contains double event subscription bug
- `/src/ChooChooEngine.App/Program.cs`: Minimal entry point; keep explicit `EnableVisualStyles()` over `ApplicationConfiguration.Initialize()` for WINE compatibility
- `/src/ChooChooEngine.App/Properties/AssemblyInfo.cs`: DELETE -- metadata moves to csproj
- `/src/ChooChooEngine.App/packages.config`: DELETE -- SharpDX 4.2.0 is unused in source
- `/src/ChooChooEngine.App/UI/ResumePanel.cs`: No changes needed; good example of target code quality (proper IDisposable)
- `/CLAUDE.md`: Must update tech stack, build commands post-migration (also `AGENTS.md`, `.cursorrules`)

## Patterns to Follow

- **Manager Pattern with Constructor Injection**: ProcessManager is root with no dependencies. InjectionManager and MemoryManager receive ProcessManager via constructor. Preserve this chain. Example: `MainForm.cs` lines 266-286.
- **Event-Driven Communication**: Custom `EventArgs` classes per manager (ProcessEventArgs, InjectionEventArgs, MemoryEventArgs) defined in same file. Protected virtual `On{EventName}` methods invoke via null-conditional. Extracted services should continue this pattern.
- **P/Invoke Region Blocks**: Each manager has `#region Win32 API` with `[DllImport]` declarations, `[StructLayout]` structs, and `UPPER_SNAKE_CASE` constants. 6 APIs are duplicated across ProcessManager and InjectionManager -- consolidate into shared `NativeMethods` class during Phase 3.
- **Two-Tier Error Handling**: Manager layer returns `bool`/`null` + fires events + `Debug.WriteLine`. UI layer catches exceptions + `LogToConsole()` + `MessageBox.Show`. No custom exceptions.
- **INI-Style File Persistence**: Three formats: profiles (`.profile` key=value), recent files (`settings.ini` with section headers), app settings (`AppSettings.ini` key=value). All relative to `Application.StartupPath`. Preserve formats for backwards compatibility.
- **Programmatic UI Construction**: ~1,400 lines of manual control creation in `ConfigureUILayout()`. Only 52-line designer file. Eliminates designer migration risk.
- **Naming Conventions**: `_camelCase` fields, `UPPER_SNAKE_CASE` Win32 constants, `{Source}_{EventName}` handlers, `On{EventName}` raisers, PascalCase enums, `ChooChooEngine.App.{Layer}` namespaces.

## Cross-Cutting Concerns

- **WINE Compatibility**: All 19 P/Invoke calls target kernel32.dll/Dbghelp.dll, both core WINE-implemented DLLs. The P/Invoke ABI is unchanged between .NET Framework and .NET 9. Self-contained publish makes the exe look like a native Windows binary to WINE. `MiniDumpWriteDump` has limited WINE support (pre-existing limitation). Keep explicit `EnableVisualStyles()` in Program.cs for WINE compatibility.
- **String Marshalling (Critical)**: `CreateProcess` must use `StringMarshalling.Utf16` (switch to `CreateProcessW`). `GetProcAddress` must use `StringMarshalling.Utf8` (ANSI-only API). `LoadLibrary` uses `CharSet.Auto` which resolves to Unicode. The injection path in `InjectDllStandard` deliberately uses `LoadLibraryA` with `Encoding.ASCII.GetBytes` -- this ANSI encoding is intentional for WINE compatibility and must be preserved.
- **STARTUPINFO Struct**: String fields (`lpReserved`, `lpDesktop`, `lpTitle`) need `[MarshalAs(UnmanagedType.LPWStr)]` when converting to `StringMarshalling.Utf16`.
- **Application.StartupPath**: Behaves identically under .NET 9 single-file publish (returns exe directory) when using `<IncludeNativeLibrariesForSelfExtract>true</IncludeNativeLibrariesForSelfExtract>`. Must validate under WINE/Proton. Fallback: `Path.GetDirectoryName(Environment.ProcessPath)`.
- **Thread Safety**: `InjectionManager._monitoringTimer` is a `System.Timers.Timer` (fires on ThreadPool). All MainForm handlers use `InvokeRequired`/`Invoke`. `_validatedDlls` dictionary accessed without synchronization outside `_injectionLock` -- safe today (single-threaded callers) but fragile.
- **Testing**: Zero tests exist. No test project or framework. Testability barriers: no interfaces, static P/Invoke, monolithic MainForm, direct file system coupling. Phase 2 extractions enable testing. Recommended: xUnit + NSubstitute/Moq.
- **Build System**: Current: `msbuild src/ChooChooEngine.sln`. Post-migration: `dotnet build src/ChooChooEngine.sln -c Release`. Publish: `dotnet publish -c Release -r win-x64 --self-contained -p:PublishSingleFile=true`.
- **Version**: Bump from 5.0 to 6.0.0 to signal the .NET 9 migration.

## Parallelization Opportunities

- **Phase 1 (Foundation)**: csproj rewrite, file deletions, and build verification are sequential. CI/CD setup and WINE/Proton testing can run in parallel once build succeeds.
- **Phase 2 (Architecture)**: All 5 service extractions from MainForm (ProfileService, CommandLineParser, LaunchOrchestrator, RecentFilesService, AppSettingsService) can proceed in parallel once boundaries are agreed. Bug fixes (double event subscription, IDisposable on ProcessManager) can also run in parallel with extractions. xUnit test project setup is independent.
- **Phase 3 (Modernization)**: P/Invoke conversion of ProcessManager, InjectionManager, and MemoryManager are independent of each other. Consolidation into shared `NativeMethods` class must happen after individual conversions. Nullable reference types, C# 12 features, and documentation updates can proceed in parallel.
- **Shared coordination files**: `MainForm.cs` is the bottleneck -- multiple Phase 2 extractions touch it. Coordinate extraction boundaries carefully to avoid merge conflicts. The SDK-style `.csproj` is also a coordination point (Phase 1 blocks everything).

## Implementation Constraints

- **Technical Constraints**:
  - App must remain a Windows binary under WINE -- native Linux is not feasible (DLL injection requires Windows kernel APIs)
  - All 19 P/Invoke functions must work identically post-migration (zero regressions on the critical injection path: VirtualAllocEx -> WriteProcessMemory -> CreateRemoteThread -> LoadLibraryA)
  - Self-contained single-file publish is mandatory (eliminates WINE runtime installation -- the #1 user friction point)
  - `AllowUnsafeBlocks` must remain enabled
  - Profile file format (.profile, settings.ini, AppSettings.ini) must remain backwards-compatible
  - `Encoding.ASCII.GetBytes` for `LoadLibraryA` injection path must be preserved
  - LibraryImport requires `partial` on both the method and containing class
  - `bool` params in P/Invoke need explicit `[MarshalAs(UnmanagedType.Bool)]`
  - Classes containing `[LibraryImport]` must not use `extern` -- use `static partial` instead
- **Business Constraints**:
  - Single-instance Mutex enforcement must work under WINE
  - CLI flags (`-p`, `-autolaunch`) must work identically
  - DLL architecture validation (32/64-bit PE header checking) must be preserved
  - Thread-safe injection via `_injectionLock` must be preserved

## Bugs to Fix During Migration

1. **Double event subscription** (Phase 2): `InitializeManagers()` and `RegisterEventHandlers()` both subscribe to the same ProcessManager/InjectionManager events, causing handlers to fire twice. Location: `MainForm.cs` lines 274-283 and 1370-1381.
2. **Handle leak** (Phase 2): ProcessManager holds unmanaged `_processHandle` via `OpenProcess` but does not implement `IDisposable`. Add IDisposable with finalizer safety net.
3. **Dead code stubs** (Phase 2): `LaunchWithCreateThreadInjection`, `LaunchWithRemoteThreadInjection` (ProcessManager), and `InjectDllManualMapping` (InjectionManager) silently fall through to defaults. Remove or implement.
4. **Missing CLI feature** (Phase 2): `-dllinject` is documented in README but not implemented in `ProcessCommandLineArguments()`.
5. **Additional dead code**: `PopulateControls()` method in MainForm is never called. `Utils/` folder is empty but referenced in csproj.

## Key Recommendations

- **Phase organization**: Phase 1 (Foundation) -> Phase 2 (Architecture) -> Phase 3 (Modernization). Phase 1 must complete before Phase 2 starts. Phase 3 can begin once Phase 2 is substantially complete.
- **Phase 1 is the critical path**: Get `dotnet build` working first with zero behavioral changes. This unblocks everything else. The csproj rewrite is the single most impactful change.
- **Target .NET 9** (`net9.0-windows`): Self-contained deployment makes STS/LTS lifecycle irrelevant. .NET 9 has better LibraryImport source generation. Plan to retarget to .NET 10 LTS (1-line change).
- **Keep WinForms**: Minimal migration risk. Core/UI separation in Phase 2 enables future Avalonia migration without touching business logic. Avalonia is the long-term direction but not for this migration.
- **Manual LibraryImport conversion over CsWin32**: Only 19 APIs -- CsWin32 is overkill and introduces a beta dependency. Manual conversion preserves existing code organization.
- **Consolidate P/Invoke in Phase 3**: Create a shared `NativeInterop/Kernel32.cs` (internal static partial class) with the 6 duplicated APIs + constants. Do this after individual file conversions, not before.
- **Test extracted services first**: ProfileService, CommandLineParser, RecentFilesService have no P/Invoke dependencies and are immediately testable. P/Invoke-dependent code requires interface wrapping for testability.
- **Validate under WINE/Proton after Phase 1**: Full functional verification of the self-contained exe before proceeding to refactoring. This catches WINE/CoreCLR compatibility issues early.
- **Update CLAUDE.md/AGENTS.md/.cursorrules after Phase 1**: These files explicitly say "dotnet build will NOT work" which will be wrong post-migration.

## Decisions Already Made

| Decision             | Choice                                | Rationale                                                           |
| -------------------- | ------------------------------------- | ------------------------------------------------------------------- |
| Target Framework     | `net9.0-windows`                      | Self-contained makes lifecycle irrelevant; better source generation |
| UI Framework         | WinForms (keep for now)               | Minimal risk; architect for future Avalonia                         |
| P/Invoke Approach    | Manual `[LibraryImport]`              | Only 19 APIs; CsWin32 is overkill                                   |
| Deployment           | Self-contained single-file            | Eliminates #1 user pain point                                       |
| SharpDX              | Remove entirely                       | Zero source references; archived project                            |
| Profile Format       | Keep .ini                             | JSON is future enhancement; preserve backwards compatibility        |
| Dead Stubs           | Remove (unless specific need)         | Simplifies codebase                                                 |
| P/Invoke Duplication | Consolidate into shared NativeMethods | Reduces 29 declaration sites to 19                                  |
| Program.cs Init      | Keep explicit `EnableVisualStyles()`  | More transparent for WINE compatibility                             |
| Nullable             | Enable with warnings                  | Catches null bugs without breaking build                            |

## Reference Documents

| Document                      | When to Read                      | Key Content                                                                                       |
| ----------------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------- |
| `feature-spec.md`             | Before any task                   | Master spec: P/Invoke matrix, csproj template, success criteria, decisions                        |
| `CLAUDE.md`                   | Before any code change            | Current conventions, architecture, build commands                                                 |
| `research-technical.md`       | During P/Invoke conversion        | Per-API string marshalling notes, WINE compatibility matrix, migration sequence                   |
| `research-recommendations.md` | During architecture refactoring   | 4 bugs to fix, 5 service boundaries, 3-phase strategy                                             |
| `research-patterns.md`        | During code conventions work      | Naming patterns, error handling, ANSI encoding gotcha                                             |
| `research-integration.md`     | During P/Invoke and file I/O work | Complete P/Invoke inventory with line numbers, file format specs, Application.StartupPath concern |
| `research-architecture.md`    | For dependency understanding      | Data flow diagrams, component relationships, edge cases                                           |
| `research-business.md`        | For business rule preservation    | 10 business rules, 4 critical workflows, domain model                                             |
| `research-ux.md`              | For future Avalonia planning      | Framework comparison, competitive landscape (not needed for current migration)                    |
| `research-external.md`        | For WINE/Proton specifics         | .NET 8/9 WINE compatibility, CsWin32 analysis, breaking changes                                   |
