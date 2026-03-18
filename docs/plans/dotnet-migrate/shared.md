# dotnet-migrate

ChooChoo Loader is a 4,264-line C# WinForms application (.NET Framework 4.8) that serves as a game trainer launcher and DLL injector for Proton/WINE on Linux/Steam Deck. The migration to .NET 9 involves three phases: (1) SDK-style csproj conversion with self-contained single-file publish, (2) extracting 5 service boundaries from the 2,800-line MainForm monolith and fixing 4 known bugs, and (3) converting 29 `[DllImport]` declaration sites (19 unique Win32 APIs across kernel32.dll and Dbghelp.dll) to `[LibraryImport]` source generators with P/Invoke consolidation. The app must remain a Windows binary under WINE — all process injection (CreateRemoteThread, LoadLibraryA, WriteProcessMemory) requires Windows kernel APIs implemented by WINE.

## Relevant Files

- /src/ChooChooEngine.App/ChooChooEngine.App.csproj: Classic 75-line .NET Framework 4.8 csproj — complete rewrite to ~30-line SDK-style targeting net9.0-windows
- /src/ChooChooEngine.sln: Solution file — may need project type GUID update for SDK-style
- /src/ChooChooEngine.App/Program.cs: 44-line entry point with Mutex single-instance and WinForms bootstrap — minimal changes (add partial if needed)
- /src/ChooChooEngine.App/Core/ProcessManager.cs: 505-line process lifecycle manager with 11 P/Invoke declarations — convert DllImport to LibraryImport, add IDisposable for handle cleanup
- /src/ChooChooEngine.App/Injection/InjectionManager.cs: 353-line DLL injection engine with 11 P/Invoke declarations (6 duplicated from ProcessManager) — convert DllImport, preserve LoadLibraryA ANSI encoding
- /src/ChooChooEngine.App/Memory/MemoryManager.cs: 368-line process memory manager with 3 P/Invoke declarations — convert DllImport to LibraryImport
- /src/ChooChooEngine.App/Forms/MainForm.cs: 2,800-line monolith containing all UI, state, profiles, settings, CLI parsing — extract 5 services, fix double event subscription bug
- /src/ChooChooEngine.App/Forms/MainForm.Designer.cs: 52-line minimal designer file — no changes needed (all UI is programmatic)
- /src/ChooChooEngine.App/UI/ResumePanel.cs: 107-line custom Panel with proper IDisposable — no changes needed, good example of target quality
- /src/ChooChooEngine.App/Properties/AssemblyInfo.cs: Assembly metadata — DELETE (replaced by csproj properties)
- /src/ChooChooEngine.App/packages.config: SharpDX 4.2.0 NuGet reference — DELETE (unused in source code)
- /CLAUDE.md: Project guidelines — must update build commands and tech stack post-migration
- /README.md: User documentation — mentions XInput features that no longer exist in source

## Relevant Patterns

**Manager Pattern with Constructor Injection**: ProcessManager is the root dependency. InjectionManager and MemoryManager both receive ProcessManager via constructor. MainForm creates all three in `InitializeManagers()`. See [/src/ChooChooEngine.App/Forms/MainForm.cs](/src/ChooChooEngine.App/Forms/MainForm.cs) lines 266-286.

**Event-Driven Communication**: All inter-component communication uses `EventHandler<TEventArgs>` with custom EventArgs classes (ProcessEventArgs, InjectionEventArgs, MemoryEventArgs) defined in the same file as their manager. Protected virtual `On{EventName}` methods fire via null-conditional `?.Invoke`. See [/src/ChooChooEngine.App/Core/ProcessManager.cs](/src/ChooChooEngine.App/Core/ProcessManager.cs) lines 118-121, 464-484.

**P/Invoke Organization**: Each manager has `#region Win32 API` blocks containing `[DllImport]` declarations, `[StructLayout]` structs, and `UPPER_SNAKE_CASE` constants. 6 APIs are duplicated across ProcessManager and InjectionManager — consolidate into shared NativeMethods class. See [/src/ChooChooEngine.App/Core/ProcessManager.cs](/src/ChooChooEngine.App/Core/ProcessManager.cs) lines 13-112.

**Two-Tier Error Handling**: Manager layer returns `bool`/`null` + fires events + `Debug.WriteLine`. UI layer catches exceptions + `LogToConsole()` + `MessageBox.Show`. No custom exceptions, no structured logging.

**INI-Style File Persistence**: Three hand-rolled file formats: profiles (.profile key=value), recent files (settings.ini with section headers), app settings (AppSettings.ini key=value). All relative to `Application.StartupPath`. See [/src/ChooChooEngine.App/Forms/MainForm.cs](/src/ChooChooEngine.App/Forms/MainForm.cs) lines 1586-1751.

**Programmatic UI Construction**: ~1,400 lines of manual control creation in `ConfigureUILayout()`. The 52-line designer file only sets form dimensions. This eliminates designer migration risk.

## Relevant Docs

**/docs/plans/dotnet-migrate/feature-spec.md**: You _must_ read this when working on any migration task — it contains the master specification with P/Invoke migration matrix, SDK-style csproj template, files to create/modify/delete, success criteria, and the 5 open decisions.

**/CLAUDE.md**: You _must_ read this when working on code changes — it defines current conventions, architecture, and build commands. Must be updated post-migration.

**/docs/plans/dotnet-migrate/research-technical.md**: You _must_ read this when working on P/Invoke conversion — it has per-API string marshalling notes, LibraryImport conversion patterns, and the WINE compatibility matrix.

**/docs/plans/dotnet-migrate/research-recommendations.md**: You _must_ read this when working on architecture refactoring — it details the 4 bugs to fix, 5 service extraction boundaries, and the 3-phase implementation strategy.

**/docs/plans/dotnet-migrate/research-patterns.md**: You _must_ read this when working on code conventions — it documents naming patterns, error handling approaches, and ANSI encoding gotcha for LoadLibraryA injection path.
