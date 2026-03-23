# Documentation Research: platform-native-ui

All documentation relevant to implementing the platform-native Tauri v2 UI for CrossHook, organized by category. This index covers the full repository including prior planning, strategic research, feature documentation, build/deployment files, and well-documented source code.

## Architecture Docs

- `/CLAUDE.md`: Project guidelines and current architecture overview. Defines the tech stack (C#, net9.0-windows, WinForms), namespace conventions, build commands, code patterns (P/Invoke, event-driven, single-instance Mutex), and file layout. Must be updated when the native app ships.
- `/AGENTS.md`: Identical to CLAUDE.md. Defines the same project conventions for a different AI agent system.
- `/docs/plans/dotnet-migrate/research-architecture.md`: Full system overview of the existing C# codebase (4,264 lines). Component diagram, application startup flow, launch workflow critical path (BtnLaunch_Click through injection), data flow for profiles/settings/CLI. Documents MainForm as a 2,800-line monolith. Essential for understanding what the Tauri app must replicate or replace.
- `/docs/plans/dotnet-migrate/analysis-code.md`: Source-level findings from the dotnet migration. Identifies bugs (double event subscription, missing IDisposable on ProcessManager), planning constraints (profile compatibility, enum serialization), and scope boundaries. Useful for knowing which behaviors to carry forward vs fix.
- `/docs/plans/dotnet-migrate/analysis-context.md`: Corrected planning assumptions for the dotnet migration. Documents current-state facts (persistence root at `Application.StartupPath`, remote injection uses `LoadLibraryA` + ASCII encoding, MainForm bottleneck). Identifies high-risk areas relevant to native app design.
- `/docs/plans/dotnet-migrate/shared.md`: Working decisions for the dotnet migration. Documents dual-artifact publishing (win-x64 + win-x86), profile format preservation, launch method compatibility constraints, and CLI contract (`-p`, `-autolaunch`). The native app must respect these same compatibility constraints for profile interop.

## Feature Docs

- `/docs/features/steam-proton-trainer-launch.doc.md`: End-user documentation for the Steam/Proton trainer workflow. Covers the two-phase launch flow (game first, then trainer), generated launcher export, current limitations (no DLL injection in Steam mode), and troubleshooting. Defines the core user workflow that the native UI must implement.
- `/docs/getting-started/quickstart.md`: End-user quickstart guide covering Linux/Steam Deck, macOS/Whisky, and external launcher export. Documents the current UX for adding CrossHook as a non-Steam game, forcing Proton, and launching trainers. Shows what the native app will simplify/eliminate.
- `/docs/plans/documentation-strategy.md`: Audit of existing docs. Identifies gaps: no architecture docs, no API docs, no screenshots. Lists the prioritized workstreams (README updates, feature docs expansion, quickstart creation). Relevant for understanding what documentation the native app should improve upon.

## Prior Research & Planning

### Platform-Native UI Research (primary -- all directly relevant)

- `/docs/plans/platform-native-ui/feature-spec.md`: **The master specification.** Executive summary, external dependencies (Steam CLI, Proton CLI, Linux process APIs, library table), business requirements (user stories, business rules, 7 edge cases, success criteria), full technical spec (architecture diagram, TOML data model, Win32-to-Linux API mapping table, Rust trait APIs, CLI interface, project directory structure), UX workflows (primary launch flow, error recovery table, UI patterns table, accessibility requirements, performance targets), recommendations (phased implementation, technology decision table), risk assessment, 4-phase task breakdown. ~33KB.
- `/docs/plans/platform-native-ui/research-technical.md`: Deep technical specifications. Architecture component diagram, Win32 P/Invoke to Linux equivalent mapping tables (ProcessManager: 10 APIs, InjectionManager: 8 APIs, MemoryManager: 5 APIs), data model definitions (GameProfile TOML, SteamAutoPopulateResult), Rust trait API designs, packaging strategy (AppImage vs Flatpak analysis), files to create/preserve listing. ~43KB.
- `/docs/plans/platform-native-ui/research-ux.md`: UX research. User workflows (profile launch, first-time setup, profile creation, in-session management), Linux desktop design standards (GNOME HIG, KDE HIG), competitive analysis (Lutris, Heroic, Bottles, WeMod, Playnite), Steam Deck UX constraints (Gaming Mode, controller navigation, Gamescope), error handling patterns, convergent UI architecture approach. ~57KB.
- `/docs/plans/platform-native-ui/research-business.md`: Business logic analysis. User stories (3 tiers), 7 business rules with validation criteria, edge cases (multiple Steam libraries, ambiguous App ID, custom Proton, Flatpak Steam, mounted drives), full workflow documentation (primary Steam launch, profile quick launch, external launcher export, error recovery), domain model (9 key entities, state machine, lifecycle events), existing codebase integration points (7 services + 3 shell scripts + 3 core managers), patterns to follow, components to leverage. ~23KB.
- `/docs/plans/platform-native-ui/research-recommendations.md`: Strategic recommendations. Recommended approach (thin orchestration frontend delegating to Bash scripts), technology choices table (Tauri v2 selected), 4-phase implementation strategy with timelines, quick wins (CLI launcher, desktop entry generator, profile format docs), WinForms disposition (freeze at current features), 5 alternative approaches analyzed (GTK4, Tauri, Python+GTK4, Avalonia, Hybrid daemon), risk assessment table, task breakdown preview for all 4 phases, key decisions needed, open questions. ~30KB.
- `/docs/plans/platform-native-ui/research-external.md`: External API and library research. Steam CLI/protocol documentation, direct Proton CLI execution patterns, Linux process memory APIs (ptrace, process_vm_readv/writev, /proc), Yama LSM/ptrace_scope details, D-Bus interfaces, UI framework comparison (Tauri v2, GTK4/Relm4, Electron), Rust crate recommendations (nix, steam-vdf-parser, tokio), distribution strategy analysis (AppImage vs Flatpak vs native packages). ~48KB.

### .NET Migration Planning (background architecture knowledge)

- `/docs/plans/dotnet-migrate/feature-spec.md`: Master spec for the .NET Framework 4.8 to .NET 9 migration. Documents decision freeze items, goals/non-goals, verified codebase facts, P/Invoke migration matrix for all 19 APIs, SDK-style csproj template, WINE compatibility matrix, files to create/modify/delete. Relevant for understanding the current C# codebase architecture that the Tauri app draws domain logic from.
- `/docs/plans/dotnet-migrate/research-business.md`: Business rules inventory. All 10 rules that must be preserved, 4 critical workflows, domain model with component dependencies, complete P/Invoke inventory, cross-platform feasibility assessment. Contains the definitive list of business rules the native app inherits.
- `/docs/plans/dotnet-migrate/research-technical.md`: SDK-style csproj conversion spec, complete P/Invoke migration matrix with per-API notes, WinForms control compatibility table, build system changes. Documents the exact Win32 APIs the native app must map to Linux equivalents.
- `/docs/plans/dotnet-migrate/research-external.md`: .NET migration path details, CsWin32 source generator analysis, WINE/Proton compatibility data, LibraryImport patterns, NativeAOT analysis. Relevant background on how the C# codebase uses Win32 APIs under WINE.
- `/docs/plans/dotnet-migrate/research-ux.md`: UI framework comparison (WinForms, Avalonia, TUI, MAUI), competitive analysis of 7 tools (Lutris, Heroic, Bottles, NexusMods.App, Vortex, MO2, BepInEx), Steam Deck constraints. Contains competitive analysis data reused in the platform-native-ui research.
- `/docs/plans/dotnet-migrate/research-recommendations.md`: Evaluates 3 implementation approaches for migration. Recommends conservative WinForms migration. Architecture improvement ideas (Core/UI separation, IDisposable, async/await) partially inform the native app design.
- `/docs/plans/dotnet-migrate/research-docs.md`: Documentation inventory for the dotnet migration. Lists all project files with descriptions and identifies must-read documents. Useful meta-index of the documentation landscape.
- `/docs/plans/dotnet-migrate/parallel-plan.md`: 14-task parallel implementation plan for the dotnet migration. Useful for understanding the codebase refactoring status.
- `/docs/plans/dotnet-migrate/phase1-smoke-gate.md`: WINE/Proton smoke test gate criteria. Relevant for understanding runtime validation requirements.
- `/docs/plans/dotnet-migrate/analysis-tasks.md`: Task-level analysis for dotnet migration. Useful for understanding planned codebase changes.

### Strategic Feature Research (shapes long-term roadmap)

- `/research/crosshook-feature-enhancements/report.md`: **Strategic research report.** Executive synthesis from 8 research personas. Three decisive findings: (1) community profile sharing is highest-leverage feature (6/8 personas agree), (2) multi-tier modification system has zero disconfirming evidence, (3) accessibility framing transforms positioning. Contains the "Dual Cockpit" architecture vision (native Linux outer orchestrator + WINE inner engine) which directly maps to the Tauri app concept. Task timeline recommendations, leverage point analysis.
- `/research/crosshook-feature-enhancements/objective.md`: Research objective definition. Documents current architecture, core research questions (code optimizations, UI/UX, technical features, business drivers, industry trends), evidence standards.
- `/research/crosshook-feature-enhancements/synthesis/innovation.md`: Novel hypotheses from cross-persona synthesis. Contains the "Dual Cockpit" architecture proposal (Hypothesis 7) -- native Linux orchestrator that manages Steam/Proton/profiles while delegating injection to WINE. Also covers "Modification Spectrum" architecture (Hypothesis 1), accessibility-as-legitimacy framing (Hypothesis 2), emulator save states (Hypothesis 3), Homebrew tap model for profiles (Hypothesis 4). All inform the native app's feature roadmap.
- `/research/crosshook-feature-enhancements/synthesis/crucible-analysis.md`: Analysis of Competing Hypotheses (ACH). Evaluates 7 strategic hypotheses. Eliminates "status quo" and "full architectural migration." Recommends Community Platform (H4) + Multi-Tier Modification (H2) as the composite strategy.
- `/research/crosshook-feature-enhancements/synthesis/pattern-recognition.md`: Meta-pattern analysis. Documents "Translation Layer Lifecycle" (CrossHook in early Phase 2 -- must transform from bridge to platform). "Dual-Mode Paradox" pattern. Predicts that platform transformation is the only survival path.
- `/research/crosshook-feature-enhancements/synthesis/negative-space.md`: Knowledge gaps analysis. Critical unanswered questions: injection success rates (unmeasured), market size (estimates range 15K-200K+), WINE API trajectory. Identifies 10-month knowledge gap in web research.
- `/research/crosshook-feature-enhancements/synthesis/contradiction-mapping.md`: Contradiction analysis across personas. Documents tensions between competing approaches.
- `/research/crosshook-feature-enhancements/synthesis/tension-mapping.md`: Tension mapping between architectural choices.
- `/research/crosshook-feature-enhancements/persona-findings/analogist.md`: Cross-domain analogies (Homebrew taps, DAW plugin hosts, Docker). Patterns for community-driven tool ecosystems.
- `/research/crosshook-feature-enhancements/persona-findings/archaeologist.md`: Injection technique comparison table (tiered compatibility). Historical injection method analysis.
- `/research/crosshook-feature-enhancements/persona-findings/contrarian.md`: Challenges to assumptions. CreateRemoteThread reliability critique (~65% end-to-end). Market size skepticism (15K-40K TAM estimate).
- `/research/crosshook-feature-enhancements/persona-findings/futurist.md`: Future technology trends. Avalonia migration path, container-based game environments.
- `/research/crosshook-feature-enhancements/persona-findings/journalist.md`: Current ecosystem analysis. Steam Deck UI patterns, controller navigation standards.
- `/research/crosshook-feature-enhancements/persona-findings/historian.md`: Historical evolution of trainers and mod loaders. Scene trainer heritage, memory save/restore as differentiator.
- `/research/crosshook-feature-enhancements/persona-findings/negative-space.md`: Missing features analysis. Accessibility void, setup friction (13-step process), community sharing absence.
- `/research/crosshook-feature-enhancements/persona-findings/systems-thinker.md`: Systems analysis. Compatibility database as highest-leverage feature, CLI-first as infrastructure positioning.
- `/research/crosshook-feature-enhancements/evidence/verification-log.md`: Evidence verification tracking for all research claims.

## Development Guides

- `/docs/internal-docs/local-build-publish.md`: Local build and publish workflow. Covers .NET 9 SDK setup, `dotnet build`, `./scripts/publish-dist.sh`, dual RID output (win-x64, win-x86), release preparation with `git-cliff` and `./scripts/prepare-release.sh`. Relevant for understanding the existing CI/CD pipeline that the native app will extend.
- `/docs/internal-docs/parallel-plan-review.md`: Code review of the 14-task dotnet migration plan against actual source. Verified line numbers, file paths, and technical claims. Documents which tasks are high-quality vs need rework.
- `/tasks/lessons.md`: Accumulated lessons from development sessions. Contains critical runtime discoveries: Wine-hosted .NET apps cannot `Process.Start("/bin/bash")` directly; dosdevices symlink resolution gotchas; `Application.StartupPath` behavior under WINE; Proton environment variable stripping requirements; Steam path normalization edge cases. Essential reading for anyone implementing Steam/Proton integration in the native app.

## README and Project Files

- `/README.md`: User-facing project documentation. Download instructions, quickstart links, feature list, build commands, release notes. Concise (~2.8KB) after recent documentation strategy refactoring.
- `/CHANGELOG.md`: Generated changelog via `git-cliff`. Documents release history. Currently at v0.1.0.
- `/release_notes.md`: v5.0 "Touch-First Release" notes. Documents major changes vs v4.0a including renamed executable, removed legacy TV Mode and controller support, process management enhancements.
- `/.github/pull_request_template.md`: PR template with summary, type-of-change checkboxes, testing environment matrix, build verification checklist, conditional checks for Injection/Memory/Core/UI changes. Defines the testing contract the native app PRs must follow.
- `/.github/ISSUE_TEMPLATE/bug_report.yml`: YAML form template for bug reports. Must be used for all bug issues.
- `/.github/ISSUE_TEMPLATE/feature_request.yml`: YAML form template for feature requests. Must be used for native UI feature issues.
- `/.github/ISSUE_TEMPLATE/compatibility_report.yml`: YAML form template for compatibility reports. Relevant for trainer/game compatibility tracking.
- `/.github/ISSUE_TEMPLATE/config.yml`: Disables blank issues. All issues must use templates.

## Build and Deployment Docs

- `/scripts/publish-dist.sh`: Build script. Produces dual-RID self-contained publishes (win-x64, win-x86). Auto-detects local .NET SDK. The native app will need its own build/publish script for Tauri.
- `/scripts/prepare-release.sh`: Release preparation. Generates CHANGELOG.md with `git-cliff`, creates annotated tags, optionally pushes. The native app should integrate into this release flow.
- `/scripts/generate-assets.sh`: SVG-to-PNG asset rendering (uses librsvg2-bin and ImageMagick). Run during CI release builds.
- `/.github/workflows/release.yml`: GitHub Actions release workflow. Triggers on `v*` tags or manual dispatch. Sets up .NET 9, generates assets, runs `publish-dist.sh`, creates GitHub Release with dual zip artifacts. The native app will need additional jobs for Tauri/AppImage builds.
- `/.github/workflows/claude.yml`: Claude Code agent workflow. Triggers on issue/PR comments containing `@claude`.
- `/.github/workflows/claude-code-review.yml`: Automated PR code review workflow using Claude.

## Code Documentation (Well-Documented Source Files)

These source files contain critical domain logic and inline documentation that directly inform the native app implementation:

- `/src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: ~900 lines. The most valuable domain logic to port. Steam library discovery, VDF manifest parsing, game-to-AppID matching, compatdata path derivation, Proton version resolution, compat tool mapping extraction. Contains VDF parser implementation.
- `/src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Launch command construction, path conversion (Windows/Unix), dosdevices symlink resolution, environment variable management, list of ~30 WINE variables to strip. Critical domain knowledge for the native Rust implementation.
- `/src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: Generates trainer launch scripts and `.desktop` entries. The `BuildTrainerScriptContent` method encodes the exact proven Proton launch pattern.
- `/src/CrossHookEngine.App/Services/ProfileService.cs`: Profile CRUD with `.profile` file format (12-field key=value text). Profile name validation. The file format is the compatibility contract the native app must respect.
- `/src/CrossHookEngine.App/Services/AppSettingsService.cs`: Simple key=value settings persistence (`AutoLoadLastProfile`, `LastUsedProfile`).
- `/src/CrossHookEngine.App/Services/RecentFilesService.cs`: MRU file lists with section-based INI format.
- `/src/CrossHookEngine.App/Services/CommandLineParser.cs`: Parses `-p <profile>` and `-autolaunch <path>`. The native app CLI should support the same contract.
- `/src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: Full game+trainer launch orchestration script. Handles Steam launch, process detection, startup delay, trainer staging, environment cleanup, Proton invocation. The native app invokes this directly in Phase 1.
- `/src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: Trainer-only launch script. Spawns detached host runner with minimal environment to escape WINE session.
- `/src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: Actual trainer execution. Stages trainer into compatdata, cleans environment, runs `$proton run $trainer_path`.
- `/src/CrossHookEngine.App/Core/ProcessManager.cs`: 505 lines. Process lifecycle with 11 P/Invoke declarations, 6 launch methods. The native app needs a Linux-native equivalent using `/proc` and signals.
- `/src/CrossHookEngine.App/Injection/InjectionManager.cs`: 353 lines. DLL injection via LoadLibraryA + CreateRemoteThread. Windows-only -- not directly portable, but documents the injection algorithm.
- `/src/CrossHookEngine.App/Memory/MemoryManager.cs`: 368 lines. Process memory read/write via Win32 APIs. Linux equivalent uses `process_vm_readv`/`process_vm_writev` or `/proc/<pid>/mem`.
- `/src/CrossHookEngine.App/Forms/MainForm.cs`: ~3600 lines. The UI monolith. Lines 2648-2946 contain `BuildSteamLaunchRequest`, `LaunchSteamModeAsync`, `RunSteamLaunchHelper`, `StreamSteamHelperLogAsync` -- the full launch orchestration that the native UI must replicate. Not directly reusable but defines all workflows and state transitions.

## PR Reviews

- `/docs/pr-reviews/pr-18-review.md`: Review of PR #18 (Steam/Proton trainer workflow). Documents 4 critical issues found including shell script exit code capture bug in the runtime helpers. Relevant for understanding known issues in the scripts the native app will invoke.
- `/docs/pr-reviews/pr-8-review.md`: Review of PR #8 (dotnet migration plan).

## Must-Read Documents

Implementers MUST read these documents before starting native UI development, in priority order:

1. **`/docs/plans/platform-native-ui/feature-spec.md`** -- The master specification. Contains the architecture diagram, data models, Rust trait APIs, project structure, phased task breakdown, technology decisions, and success criteria. This is the single most important document.

2. **`/docs/plans/platform-native-ui/research-business.md`** -- Business rules, edge cases, workflows, domain model, and codebase integration points. Defines WHAT the app must do and WHICH existing code to reuse/port.

3. **`/docs/plans/platform-native-ui/research-recommendations.md`** -- Technology choices, phasing strategy, risk assessment, task breakdown, and alternative approaches. Defines HOW the app should be built.

4. **`/CLAUDE.md`** -- Current project conventions, code patterns, and build commands. Must be understood to maintain consistency and will need updating for the native app.

5. **`/docs/features/steam-proton-trainer-launch.doc.md`** -- The user-facing workflow the native app must replicate. Defines the two-phase launch flow, generated launchers, and current limitations.

6. **`/tasks/lessons.md`** -- Runtime discoveries from live debugging. Contains critical gotchas about Wine path handling, dosdevices resolution, environment variable stripping, and Proton integration that are directly relevant to the Rust implementation.

7. **`/research/crosshook-feature-enhancements/report.md`** -- Strategic research report. Contains the "Dual Cockpit" vision that architecturally validates the native UI approach, plus the community profile sharing strategy (highest-leverage feature) and multi-tier modification system.

### Nice-to-Have Reading

- `/docs/plans/platform-native-ui/research-technical.md` -- Deep Win32-to-Linux API mapping tables. Reference material for Rust implementation.
- `/docs/plans/platform-native-ui/research-ux.md` -- Competitive analysis (Lutris, Heroic, Bottles), Steam Deck UX constraints, convergent UI patterns. Reference for UI design decisions.
- `/docs/plans/platform-native-ui/research-external.md` -- External API details (Steam CLI, ptrace, Yama LSM, D-Bus), Rust crate recommendations. Reference for dependency selection.
- `/docs/plans/dotnet-migrate/research-architecture.md` -- Full C# codebase architecture overview. Useful context for understanding the existing system being replaced.
- `/research/crosshook-feature-enhancements/synthesis/innovation.md` -- Novel feature hypotheses (Modification Spectrum, Homebrew Taps, Save States). Shapes Phase 3-4 roadmap.

## Documentation Gaps

The following areas lack documentation that would help native UI implementers:

1. **No Tauri v2 project setup guide**: The feature spec defines the project structure (`src/crosshook-native/`) but does not document the Tauri v2 + React + Vite initialization steps, recommended Tauri plugins, or IPC command patterns.

2. **No Rust crate evaluation**: The feature spec lists recommended crates (`nix`, `steam-vdf-parser`, `tokio`, `serde`, `clap`, `tracing`) but lacks comparison notes or version pinning rationale. The `steam-vdf-parser` crate specifically needs validation -- research mentions `keyvalues-parser` as an alternative.

3. **No AppImage packaging guide**: Distribution is via AppImage but no documentation covers the AppImage build pipeline, linuxdeploy configuration, or bundling of WebKitGTK dependencies.

4. **No CI/CD plan for native app**: The existing release workflow (`release.yml`) only handles .NET/Windows artifacts. No documentation covers how the Tauri build, AppImage packaging, and Linux artifact upload will integrate.

5. **No profile format migration spec**: The feature spec states TOML is the native format with legacy `.profile` import, but no schema document or migration tool spec exists.

6. **No controller/gamepad input mapping doc**: The UX research describes controller navigation patterns but lacks a concrete input mapping document (which Tauri/React APIs to use, how to detect controller type, gamepad button-to-action mapping).

7. **No testing strategy**: No test framework is currently configured for the C# codebase. The native app needs a testing plan covering Rust unit tests (`cargo test`), frontend tests, and integration tests for Steam/Proton workflows. No documentation addresses this.

8. **No architecture decision records (ADRs)**: Technology decisions (Tauri over GTK4, React over Svelte, TOML over JSON) are documented in research files but not as formal ADRs with context/decision/consequences format.

9. **Known shell script bugs**: PR #18 review identified a critical exit code capture bug in `steam-host-trainer-runner.sh` and `steam-launch-helper.sh`. These bugs are documented in the PR review but not tracked as issues or marked as fixed.

10. **No screenshots or UI mockups**: The documentation strategy explicitly deferred screenshots and annotated UI callouts. No wireframes or mockups exist for the native app.
