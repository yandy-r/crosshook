# Documentation Research: dotnet-migrate

> Background inventory only. The active migration decisions now live in `feature-spec.md`, `parallel-plan.md`, and the refreshed `analysis-*` docs. Older exploratory documents in this directory still contain alternatives that were intentionally deferred from the current plan.

CrossHook Loader has thorough documentation for the .NET Framework 4.8 to .NET 9 migration. The project contains 6 detailed research documents in `docs/plans/dotnet-migrate/`, 3 AI-agent instruction files (CLAUDE.md, AGENTS.md, .cursorrules -- all identical), a comprehensive README, release notes, and a full suite of environment/tooling configuration files. The research phase is complete and covers business logic, external APIs, technical specifications, UX/framework options, and strategic recommendations. No additional architecture docs or internal wikis exist outside these files.

## Feature Research Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/feature-spec.md`: The master specification synthesizing all research. Covers executive summary, external dependencies (APIs, libraries, documentation links), business requirements (user stories, business rules, edge cases, success criteria), full technical specifications (architecture overview, P/Invoke migration matrix with all 19 APIs, SDK-style csproj template, build commands, files to create/modify/delete), UX considerations (WinForms vs Avalonia decision, competitive landscape, performance impact, Steam Deck), recommendations (phased strategy, technology decisions, quick wins, bugs found), risk assessment, and 3-phase task breakdown preview. **27KB, most comprehensive single document.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-business.md`: Core functionality inventory. Enumerates all 10 business rules that must be preserved, 8 edge cases, 4 critical workflows (game+trainer launch, DLL injection step-by-step, profile management, CLI auto-launch), the domain model with component dependencies, .NET Framework dependency analysis (framework-specific vs portable vs WinForms), complete P/Invoke inventory table (18-19 unique APIs), patterns to follow (event-driven, thread-safe UI, namespace layering, field naming), cross-platform feasibility assessment (concludes native Linux is not viable), and success criteria. **25KB.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-external.md`: External API and dependency research. Covers the .NET 8/9 migration path (key changes table, P/Invoke compatibility, WinForms support), CsWin32 source generator (setup, configuration, pros/cons), WINE/Proton compatibility (self-contained .NET under WINE, CoreCLR WINE issues, MiniDumpWriteDump limitations, WINE bug database findings), LibraryImport patterns (DllImport-to-LibraryImport conversion rules, StringMarshalling specifics for each API), self-contained deployment details, NativeAOT analysis, breaking changes catalog, and integration risks. **48KB, the largest research document.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-technical.md`: Detailed technical specifications. Contains the SDK-style .csproj conversion (full XML template), target framework selection rationale (net9.0-windows), complete P/Invoke migration matrix with per-API migration notes (CreateProcess UTF-16, GetProcAddress UTF-8, LoadLibrary StringMarshalling), WINE compatibility matrix for all 19 APIs, WinForms migration details (designer compatibility, all controls used with .NET 9 status, System.Drawing types, ApplicationConfiguration), build system changes (packages.config to PackageReference, MSBuild properties, build/publish commands), files to create/modify/delete tables, all 6 technical decisions with rationale, WINE runtime considerations, and recommended migration sequence. **39KB.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-ux.md`: UX and UI framework research. Analyzes current WinForms-on-WINE workflow with pain points, post-migration flows for both WinForms and Avalonia paths, Steam Deck Gaming Mode constraints, comprehensive comparison of 4 UI framework options (WinForms on .NET 8/9, Avalonia UI, Terminal UI, .NET MAUI) with pros/cons and confidence levels, competitive analysis of 7 tools (Lutris, Heroic, Bottles, NexusMods.App, Vortex, Mod Organizer 2, BepInEx), error handling UX matrix (10 error states with current behavior and recommendations), performance UX tables (startup time, runtime, deployment models), must-have/should-have/nice-to-have UX recommendations, and 60+ external source links. **43KB.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-recommendations.md`: Strategic recommendations. Evaluates 3 implementation approaches (Option A: Conservative WinForms migration, Option B: Native Linux with Avalonia, Option C: Hybrid with pluggable UI) with effort estimates and risk levels, recommends Option A. Details technology choices table, 3-phase phasing strategy with timelines, quick wins list, architecture improvements (Core/UI separation, testing infrastructure, IDisposable pattern, event subscription duplication bug), future enhancements (structured logging, async/await, Result pattern, profile format), risk assessment table, integration challenges, alternative approaches analysis, task breakdown preview for all 3 phases, key decisions needed, and open questions. **26KB.**

## Architecture Docs

No dedicated architecture documentation exists outside the files listed above. The architectural knowledge is distributed across:

- `CLAUDE.md` (project overview and architecture section with file tree)
- `feature-spec.md` (architecture overview diagrams: current vs post-migration)
- `research-business.md` (component dependency diagram, domain model)
- `research-technical.md` (project structure post-migration, codebase change inventory)

## README Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/README.md`: User-facing documentation. Covers what CrossHook is (Proton/WINE Trainer and DLL Loader), why it is needed for Proton/WINE, features at a glance table, Quick Start guides for Steam Deck (with Proton), macOS (with Whisky -- now deprecated), WeMod/trainer fix instructions (removing wine-mono, installing .NET Framework), customization and artwork, v5.0 new features (DLL injection overhaul, XInput handling, CLI enhancements, UI and stability improvements). **11KB.** Note: mentions XInput handling features that are not actually implemented in the current source code (SharpDX removed, no controller code exists).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/release_notes.md`: v5.0 "Touch-First Release" notes. Documents major changes vs v4.0a: touch-first interface redesign, renamed to crosshook.exe, removed legacy TV Mode and controller support, process management enhancements, core stability fixes, documentation updates. Lists core features: DLL injection system, touch and mouse interface, CLI features, UI and stability. Installation instructions for Windows, Steam Deck, and macOS. **3.6KB.**

## Configuration Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md`: AI agent project guidelines. Defines tech stack (.NET Framework 4.8, WinForms, MSBuild classic csproj), solution/project paths, build commands (msbuild, NOT dotnet CLI), architecture file tree with descriptions, key patterns (Win32 P/Invoke, event-driven, AllowUnsafeBlocks, single-instance Mutex), code conventions (namespace pattern, field naming, Win32 constants, P/Invoke organization, event args), and important notes (Windows-only binary, no test framework, env management with direnv/dotenvx, never commit .env files). **2.6KB.** This file MUST be updated post-migration to reflect new build commands and tech stack.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md`: Identical content to CLAUDE.md. Same guidelines for a different AI agent system. **2.6KB.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.cursorrules`: Identical content to CLAUDE.md and AGENTS.md. Same guidelines for Cursor IDE. **2.6KB.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.vscode/settings.json`: Minimal VS Code settings. Sets `dotnet.preferCSharpExtension: true` and a Peacock color. **4 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.envrc`: direnv configuration. Loads `.env` via `source` and `.env.encrypted` via `dotenvx get`. Contains commented-out templates for PATH, virtualenv, Go, Ruby, cloud providers, Docker, and helper functions. **105 lines.** Mostly boilerplate; no project-specific env vars are configured.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.env.example`: Template env file. Contains commented-out placeholders for APP_ENV, DB_HOST, API_KEY, REDIS_URL, and feature flags. **31 lines.** All values are commented out; no project-specific variables are defined. This is generic boilerplate not currently used by the C# application.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.gitignore`: .NET-focused gitignore. Excludes build artifacts (Debug, Release, bin, obj), .NET Core artifacts (project.lock.json, artifacts), NuGet packages, .env file, MSBuild logs, test results. **61 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.markdownlint.json`: Markdown linting rules. Disables MD002 (first heading level), MD003 (heading style), MD013 (line length), MD041 (first line heading), MD033 (inline HTML), MD036 (emphasis as heading), MD040 (code block language), MD042 (empty links), MD051 (link fragments). Enables most formatting rules. Uses 2-space list indentation. **187 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.markdownlintignore`: Ignores node_modules, vendor, dist, build, .git, .vscode, IDE files, OS files, logs, Jekyll, and package lock files. **43 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.prettierrc`: Prettier formatting config. Preserves prose wrap, 2-space tabs, 120 char print width, single quotes, trailing commas, semicolons. Markdown files preserve prose wrap at 120 width. **20 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.prettierignore`: Ignores node_modules, .git, .cache, .direnv, venv, coverage, dist, build, tmp. **11 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.claude/settings.local.json`: Claude Code local permissions. Allows plan-workflow skills (feature-research, plan-workflow) and specific bash commands for the dotnet-migrate feature. **12 lines.**

## CI/CD and GitHub

- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/workflows/claude.yml`: GitHub Actions workflow for Claude Code agent. Triggers on issue/PR comments containing `@claude`. Uses `anthropics/claude-code-action@v1` with OAuth token. Read-only permissions on contents, PRs, issues, and actions. **51 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/workflows/claude-code-review.yml`: GitHub Actions workflow for automated PR code review. Triggers on PR opened/synchronize/ready_for_review/reopened. Uses Claude code-review plugin. Read-only permissions. **45 lines.**

## Code Documentation (Source Files)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Program.cs`: Entry point. Has one XML doc comment on `Main()`. Single-instance Mutex enforcement, WinForms bootstrap. **45 lines.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Properties/AssemblyInfo.cs`: Assembly metadata. Title "CrossHook Injection Engine", description "Game modding and trainer management utility", Copyright 2025, version 1.0.0.0. **36 lines.** Will be deleted during migration (replaced by csproj properties).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/CrossHookEngine.App.csproj`: Classic .NET Framework 4.8 project file. ToolsVersion 15.0, OutputType WinExe, AllowUnsafeBlocks true (both Debug and Release), references to System, System.Core, System.Drawing, System.Windows.Forms, etc. Explicit Compile includes for all .cs files. References packages.config. **75 lines.** Will be completely rewritten as SDK-style.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/packages.config`: NuGet references. Contains only `SharpDX 4.2.0` targeting net48. **4 lines.** SharpDX is unused in source code and will be removed.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.sln`: Visual Studio solution. Format version 12.00, VS 2022 (17.0). Single project reference with GUID `{FAE04EC0-...}` (C# project type). Debug/Release configurations for Any CPU. **24 lines.**

Source files with significant inline documentation (via `#region` blocks and comments):

- `Core/ProcessManager.cs` (506 lines): `#region Win32 API` blocks grouping 11 P/Invoke declarations with Win32 constants and structs. Comments on process lifecycle methods.
- `Injection/InjectionManager.cs` (354 lines): `#region Win32 API` blocks grouping 12 P/Invoke declarations. Detailed inline comments on the DLL injection algorithm, PE header parsing, and validation logic.
- `Memory/MemoryManager.cs` (369 lines): `#region Win32 API` blocks grouping 3 P/Invoke declarations. Comments on memory read/write operations and MEMORY_BASIC_INFORMATION struct.
- `Forms/MainForm.cs` (~2800 lines): The largest file. Contains inline comments explaining WINE/Proton UX workarounds, dark theme color rationale, compact mode layout logic, profile format structure, and command-line argument processing.
- `UI/ResumePanel.cs` (108 lines): Well-documented with proper IDisposable implementation and GDI+ drawing comments. Noted in research as an example of target code quality.

## Must-Read Documents

Implementers MUST read these documents before starting work, in this order:

1. **`/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/feature-spec.md`** -- The master specification. Read the executive summary, P/Invoke migration matrix, SDK-style csproj template, files to create/modify/delete lists, success criteria, phased task breakdown, and decisions needed. This is the single most important document.
2. **`/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md`** -- Current project conventions, architecture, build commands, and code conventions. This file itself must be updated post-migration.
3. **`/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-technical.md`** -- Detailed technical specifications including per-API migration notes, full csproj template, WinForms control compatibility table, and the recommended migration sequence (11 steps).
4. **`/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-recommendations.md`** -- Strategic decisions, 3-phase implementation plan with timelines, bugs to fix during migration (double event subscription, handle leak, dead code stubs, missing CLI feature), and risk assessment.
5. **`/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/dotnet-migrate/research-business.md`** -- Business rules that must be preserved, critical workflows, and the cross-platform feasibility conclusion (must remain a Windows binary under WINE).

Nice-to-have reading:

- `research-external.md` -- Deep dive into .NET migration APIs, CsWin32, WINE compatibility data, and LibraryImport patterns. Useful for the P/Invoke conversion phase.
- `research-ux.md` -- UI framework comparison and competitive analysis. Primarily relevant if considering Avalonia migration in the future; the current plan is to keep WinForms.
- `README.md` -- User-facing features and platform guides. Useful context but not directly actionable for implementation.
- `release_notes.md` -- v5.0 feature summary. Useful for understanding what was recently changed.

## Documentation Gaps

1. **No testing documentation**: No test framework is configured, no testing guide exists, and no test patterns are documented. The research recommends xUnit but provides no setup instructions or example test patterns for the P/Invoke-heavy code.
2. **No CI/CD build pipeline for the application**: The GitHub Actions workflows only handle Claude Code AI agent interactions (PR review and issue response). There is no workflow for building, testing, or publishing the .NET application. The research-recommendations.md mentions GitHub Actions CI/CD as a future task but no implementation details are provided.
3. **No WINE/Proton testing guide**: While research documents mention "test under WINE/Proton" as a success criterion, there is no step-by-step guide for setting up a WINE testing environment, no documented Proton version matrix, and no smoke test scripts.
4. **No developer setup guide**: No document explains how to set up a development environment for this project. The build command is in CLAUDE.md (msbuild) but there are no instructions for installing .NET Framework 4.8 targeting pack, Visual Studio requirements, or Linux development setup.
5. **CLAUDE.md, AGENTS.md, and .cursorrules are stale**: All three files describe the current .NET Framework 4.8 stack. They explicitly state "dotnet build will NOT work" -- this will be wrong after migration. These must be updated in Phase 1 or 3 of the migration.
6. **README.md has inaccurate feature claims**: The README describes XInput handling and controller support features that do not exist in the source code (SharpDX is unused, controller code was removed). The release notes for v5.0 confirm TV Mode and controller support were removed, but the README still references XInput.
7. **No API documentation**: No XML doc comments exist beyond a single `<summary>` on `Program.Main()`. The P/Invoke declarations, public methods on ProcessManager/InjectionManager/MemoryManager, and MainForm's public interface are undocumented.
8. **No changelog or version history**: The release_notes.md covers v5.0 only. No historical changelog exists for prior versions.
9. **.env.example is boilerplate**: The .env.example file contains generic database/API key placeholders that are not relevant to this C# WinForms application. No project-specific environment variables are documented.
10. **Conflicting .NET version recommendations**: The feature-spec.md recommends .NET 9 (STS), while research-recommendations.md recommends .NET 8 (LTS). The feature-spec.md resolves this by noting self-contained deployment makes lifecycle irrelevant, but the disagreement between research files may confuse implementers. The feature-spec.md is the authoritative answer: target .NET 9, plan to retarget to .NET 10 LTS.

# Note

This file catalogs the research set, but the active migration decisions now live in `feature-spec.md`, `parallel-plan.md`, and the refreshed `analysis-*` docs. Older exploratory documents in this directory still contain alternatives that were intentionally deferred from the current plan.
