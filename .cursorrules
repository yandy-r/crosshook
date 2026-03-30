# CrossHook - Project Guidelines

## Project Overview

CrossHook is a native Linux game launcher and trainer manager for Steam Deck, Linux, and macOS users. It orchestrates launching games alongside trainers, mods (FLiNG, WeMod, etc.), and patches through Steam/Proton, standalone Proton prefixes, or native execution. It is distributed as a Linux AppImage.

## Tech Stack

- **Backend**: Rust (workspace with `crosshook-core` library and `crosshook-cli` binary)
- **Frontend**: React 18 + TypeScript + Vite
- **Desktop Framework**: Tauri v2
- **Build System**: `cargo` + `npm` (Tauri CLI orchestrates both)
- **Source Root**: `src/crosshook-native/`
- **Output**: Linux AppImage (`dist/*.AppImage`)
- **CI**: `.github/workflows/release.yml` builds and publishes the AppImage on tag push

## Build Commands

```bash
# Development (starts Vite dev server + Tauri with hot reload)
./scripts/dev-native.sh

# Build AppImage (full production build)
./scripts/build-native.sh

# Build in container (for CI-like reproducibility)
./scripts/build-native-container.sh

# Binary-only build (no AppImage bundling)
./scripts/build-native.sh --binary-only

# Install system build dependencies
./scripts/install-native-build-deps.sh

# Run crosshook-core tests
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

## Architecture

```
src/crosshook-native/
  Cargo.toml                    # Rust workspace root (crosshook-core, crosshook-cli, src-tauri)
  package.json                  # React/Vite frontend (Tauri API v2, React 18)
  index.html                    # Vite entry HTML

  crates/crosshook-core/        # Shared Rust library (all business logic)
    src/
      community/                # Community profile taps (index.rs, taps.rs)
      export/                   # Launcher export — shell script + .desktop entry generation (launcher.rs, launcher_store.rs)
      install/                  # Game install workflow (discovery.rs, models.rs, service.rs)
      launch/                   # Launch orchestration (env.rs, request.rs, script_runner.rs, optimizations.rs)
      metadata/                 # SQLite metadata layer (db.rs, migrations.rs, models.rs, profile_sync.rs, launch_history.rs, launcher_sync.rs, health_store.rs, community_index.rs, collections.rs, cache_store.rs)
      profile/                  # Profile management (models.rs, toml_store.rs [includes rename, duplicate], legacy.rs, exchange.rs, community_schema.rs)
      settings/                 # App settings + recent files (mod.rs, recent.rs)
      steam/                    # Steam integration (discovery, libraries, manifest, proton, vdf, auto_populate, diagnostics)
      update/                   # Game update/patch workflow (models.rs, service.rs)
      logging.rs                # Structured logging
      lib.rs                    # Module root

  crates/crosshook-cli/         # CLI binary (standalone, no Tauri dependency)
    src/
      args.rs                   # CLI argument definitions
      main.rs                   # CLI entry point

  src-tauri/                    # Tauri v2 app shell
    src/
      commands/                 # IPC command handlers (collections.rs, community.rs, diagnostics.rs, export.rs, health.rs, install.rs, launch.rs, profile.rs, settings.rs, shared.rs, steam.rs, update.rs)
      lib.rs                    # Tauri setup, plugin registration, command routing
      main.rs                   # Tauri entry point
      paths.rs                  # Script path resolution
      startup.rs                # Auto-load profile on app start
    tauri.conf.json             # Tauri config (AppImage target, dark theme, 1280x800)
    capabilities/default.json   # Tauri permissions

  src/                          # React frontend
    App.tsx                     # Main app shell (sidebar nav: Profiles, Launch, Install, Health, Community, Compatibility, Settings)
    main.tsx                    # React entry point
    components/
      icons/                    # SVG icon components (SidebarIcons.tsx)
      layout/                   # Layout shells (Sidebar.tsx, ContentArea.tsx, ConsoleDrawer.tsx, PageBanner.tsx, ControllerPrompts.tsx)
      pages/                    # Page-level components
        CommunityPage.tsx       # Community tap browser page
        CompatibilityPage.tsx   # Game/trainer compatibility page
        HealthDashboardPage.tsx # Profile health dashboard with sortable metadata
        InstallPage.tsx         # Game install workflow page
        LaunchPage.tsx          # Game launch controls page
        ProfilesPage.tsx        # Profile management page (largest page component)
        SettingsPage.tsx        # App settings page
      ui/                       # Reusable UI primitives (CollapsibleSection, InstallField, ProtonPathField, ThemedSelect)
      AutoPopulate.tsx          # Steam library auto-discovery
      CommunityBrowser.tsx      # Browse and install community profile taps
      CompatibilityViewer.tsx   # Game/trainer compatibility info
      ConsoleView.tsx           # Launch log output viewer
      HealthBadge.tsx           # Profile health score badge
      InstallGamePanel.tsx      # Install Game sub-panel
      LauncherExport.tsx        # Export profile as shell script / .desktop entry
      LauncherPreviewModal.tsx  # Launcher script preview modal
      LaunchOptimizationsPanel.tsx # Curated optimization toggles for proton_run
      LaunchPanel.tsx           # Game launch controls
      PinnedProfilesStrip.tsx   # Quick-access pinned profiles strip
      ProfileActions.tsx        # Profile action bar (save, duplicate, delete, pin)
      ProfileFormSections.tsx   # Profile editor form sections
      ProfilePreviewModal.tsx   # Community profile preview modal
      ProfileReviewModal.tsx    # Install Game review modal
      SettingsPanel.tsx         # App settings management
      SteamLaunchOptionsPanel.tsx # Steam launch options generator
      UpdateGamePanel.tsx       # Update/patch game panel
    context/                    # React contexts
      PreferencesContext.tsx    # App preferences (pinned profiles, sidebar state)
      ProfileContext.tsx        # Profile state provider
      ProfileHealthContext.tsx  # Profile health data provider
    hooks/                      # React hooks
      useCommunityProfiles.ts   # Community tap state management
      useGamepadNav.ts          # Gamepad/controller navigation support
      useInstallGame.ts         # Install Game workflow state
      useLauncherManagement.ts  # Launcher export/delete/status state
      useLaunchState.ts         # Launch process state
      usePreviewState.ts        # Dry run / preview state
      useProfile.ts             # Profile CRUD state
      useProfileHealth.ts       # Profile health dashboard state
      useScrollEnhance.ts       # Smooth scroll behavior
      useUpdateGame.ts          # Update/patch game state
    styles/                     # CSS
      focus.css                 # Focus/keyboard navigation styles
      theme.css                 # Dark theme and layout
      variables.css             # CSS custom properties
    types/                      # TypeScript type definitions
      index.ts                  # Re-exports
      diagnostics.ts            # Diagnostic bundle types
      health.ts                 # Profile health types
      install.ts                # Install workflow types
      launch.ts                 # Launch-related types
      launch-optimizations.ts   # Optimization catalog types
      launcher.ts               # Launcher lifecycle types (info, delete, rename results)
      profile.ts                # Profile types
      profile-review.ts         # Install review modal types
      settings.ts               # Settings types
      update.ts                 # Update/patch types
```

### Key Patterns

- **Tauri IPC**: All backend operations are exposed as Tauri commands (`#[tauri::command]`) invoked from React via `@tauri-apps/api`
- **TOML persistence**: Profiles and settings are stored as TOML files in `~/.config/crosshook/`
- **SQLite metadata**: Persistent metadata layer (`metadata/`) using rusqlite for stable profile identity, launch history, launcher sync, health snapshots, community catalog, and collections
- **Steam discovery**: Scans Linux filesystem for Steam libraries, app manifests, and Proton installations via VDF parsing
- **Launch methods**: `steam_applaunch` (Steam client), `proton_run` (standalone Proton prefix), `native` (direct execution)
- **Launch optimizations**: Curated toggle catalog mapped to Proton env vars and host wrappers; `proton_run` applies directly, `steam_applaunch` generates a copy/paste line for Steam Launch Options
- **Community taps**: Git-based profile sharing repositories with index manifests; taps support commit pinning
- **Gamepad navigation**: Full controller support via the `useGamepadNav` hook for Steam Deck usage
- **Launcher export**: Generates standalone `.sh` scripts and `.desktop` entries from profiles with stale detection
- **Launcher lifecycle**: `launcher_store.rs` manages check/delete/rename/list/orphan-detection for exported launchers; profile deletion and renaming cascade to launcher cleanup via Tauri commands
- **Profile health**: Health scores computed from path validity, config completeness, and staleness; persisted via SQLite with trend analysis
- **Page-based UI**: Sidebar navigation with dedicated pages (Profiles, Launch, Install, Health, Community, Compatibility, Settings) replacing the old tab layout
- **Workspace crate separation**: `crosshook-core` contains all business logic; `crosshook-cli` and `src-tauri` are thin consumers

## Code Conventions

### Rust

- `snake_case` for functions, variables, modules
- Modules organized as directories with `mod.rs`
- Error handling via `Result<T, E>` with `anyhow` or custom error types
- Tauri commands: `snake_case` function names matching frontend `invoke()` calls
- Serde derive macros for all types that cross the IPC boundary

### React / TypeScript

- `PascalCase` for components, `camelCase` for hooks and functions
- TypeScript strict mode enabled
- Tauri `invoke()` calls wrapped in custom hooks for state management
- CSS custom properties defined in `variables.css`, BEM-like class names (`crosshook-*`)

### Commit Messages And Changelog Hygiene

- `CHANGELOG.md` is generated with `git-cliff` and published directly to GitHub Releases; commit messages must be deliberate because they become release notes.
- Use conventional commits for any user-facing change that should appear in the changelog, for example `feat(launcher): ...`, `fix(launch): ...`, `docs(quickstart): ...`, `build(release): ...`.
- Do not use vague or generic titles such as `Update README.md`, `fix stuff`, `misc cleanup`, or `bump version` for code that may ship in a release.
- Internal planning, research, task-tracking, release-prep churn, and other non-user-facing maintenance should use skipped forms such as `chore(release): ...`, `chore(...): ...`, or `docs(internal): ...` so they stay out of release notes.
- If a change is important to users, write the commit title the way you want it to read in `CHANGELOG.md`.

## Important Notes

- This is a **native Linux application** distributed as an AppImage -- it does NOT run under WINE/Proton itself
- The app manages launching Windows games via Proton/WINE, but the app binary is native Linux
- No test framework is configured for the frontend; Rust tests exist for `crosshook-core` (`cargo test -p crosshook-core`)
- Environment management uses `direnv` with `.envrc` and `dotenvx` for encrypted env vars
- Never commit `.env`, `.env.encrypted`, or `.env.keys` files

## Model Context Protocol (MCP)

- When MCP servers are available in the environment (for example GitHub, IDE browser, documentation, or other configured integrations), **use them for work they are meant to handle**—repository and issue/PR operations, live docs or API references, browser verification, and similar tasks—instead of relying on stale knowledge or extra manual steps when a tool already exposes the right capability.
- Before calling an MCP tool, read its schema or descriptor so parameters, auth, and side effects match what you intend.
- If a server is not enabled, not authenticated, or a better path exists (for example this repo’s `gh` / script conventions in **GitHub Workflow**), use that path and do not treat MCP absence as a blocker.

## GitHub Workflow

### Issue Templates

All issues MUST use the YAML form templates in `.github/ISSUE_TEMPLATE/`:

- **Bug Report** (`bug_report.yml`): Use `gh issue create --template bug_report.yml`
- **Feature Request** (`feature_request.yml`): Use `gh issue create --template feature_request.yml`
- **Compatibility Report** (`compatibility_report.yml`): Use `gh issue create --template compatibility_report.yml`

Blank issues are disabled via `config.yml`. Never bypass templates with `--title`-only issue creation.

Practical CLI limitation:

- `gh issue create` does not support combining `--template` with `--body` or `--body-file`.
- In this repo, `gh issue create --template ...` currently reports `no templates found` for the YAML issue forms, so the CLI is not discovering these form templates reliably.
- If this limitation blocks issue creation, use the GitHub API/tooling to create a fully structured issue body that mirrors the intended form fields, then apply the correct labels. Do not fall back to a vague or title-only issue.

### Pull Requests

PRs auto-populate from `.github/pull_request_template.md`. The template includes:

- `Closes #` issue linkage (always link the related issue)
- Type of Change checkboxes
- Build verification checklist (native build scripts)
- Conditional checks for launch/, profile/, steam/, and UI component changes
- All pull requests must be properly labeled with the appropriate labels.
- All pull requests must be properly linked to the appropriate issue.

CLI completion note:

- Zsh completion for `gh` may be loaded correctly while `gh` itself still returns no positional completions for PR or issue numbers.
- If `gh pr merge <TAB>` does not fill in PR identifiers, verify with `gh __complete pr merge ""`. If it returns only `:0`, that is a `gh` completion limitation, not necessarily a shell setup problem.

### Releases

- Before tagging a release, run `./scripts/prepare-release.sh ...`; it now regenerates `CHANGELOG.md` and validates the tagged section with `./scripts/validate-release-notes.sh`.
- The release workflow also validates the tagged `CHANGELOG.md` section before publishing assets, so noisy commits should fail release preparation instead of leaking into GitHub Releases.

### Labels

Use the colon-prefixed label taxonomy -- never create ad-hoc labels:

- `type:` bug, feature, docs, refactor, compatibility, build, migration
- `area:` injection, memory, process, ui, build, profiles, cli
- `platform:` steam-deck, linux, macos, wine, proton
- `priority:` critical, high, medium, low
- `status:` needs-triage, in-progress, blocked, needs-info
- Standalone: `good first issue`, `help wanted`, `duplicate`, `wontfix`
