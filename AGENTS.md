# CrossHook Native - Project Guidelines

This file mirrors `CLAUDE.md` for non-Claude AI agents. Refer to this for project context, conventions, and workflow rules.

## Project Overview

CrossHook Native is a Linux desktop application for launching game trainers (FLiNG, WeMod, etc.) alongside Steam/Proton games. It is a complete rewrite of the original WinForms-based CrossHook Loader. CrossHook itself runs natively on Linux — trainers and games run under Proton/WINE.

## Tech Stack

- **Backend**: Rust (workspace with two crates: `crosshook-core`, `crosshook-cli`)
- **Desktop Shell**: Tauri v2
- **Frontend**: React 18 + TypeScript + Vite
- **Build**: `cargo` + `npm`, produces a Linux AppImage
- **Source Root**: `src/crosshook-native/`

## Architecture

```
src/crosshook-native/
  Cargo.toml                # Rust workspace root
  package.json              # React/Vite frontend
  vite.config.ts            # Vite build configuration
  index.html                # Tauri webview entry

  crates/
    crosshook-core/         # Shared Rust library
      src/
        lib.rs              # Crate root, module re-exports
        logging.rs          # Structured logging
        community/          # Community profile taps and sharing
        export/             # Launcher export (shell scripts, .desktop entries)
        launch/             # Game + trainer launch orchestration
        profile/            # TOML profile management
        settings/           # App settings persistence
        steam/              # Steam library discovery, Proton version detection

    crosshook-cli/          # Standalone CLI binary
      src/
        args.rs             # CLI argument parsing
        main.rs             # CLI entry point

  src-tauri/                # Tauri v2 app shell
    src/
      main.rs               # Tauri bootstrap
      lib.rs                 # IPC command registration
      paths.rs               # XDG/platform path resolution
      startup.rs             # App initialization
      commands/              # Tauri IPC command handlers
        community.rs
        export.rs
        launch.rs
        profile.rs
        settings.rs
        steam.rs

  src/                      # React frontend
    main.tsx                # React entry point
    App.tsx                 # Root component with tab navigation
    components/
      AutoPopulate.tsx      # Steam library auto-populate UI
      CommunityBrowser.tsx  # Community tap browser
      CompatibilityViewer.tsx # Game/trainer compatibility reports
      ConsoleView.tsx       # Real-time runner output console
      LauncherExport.tsx    # Shell script / .desktop export UI
      LaunchPanel.tsx       # Launch controls
      ProfileEditor.tsx     # Profile creation and editing
      SettingsPanel.tsx     # App settings UI
    hooks/
      useCommunityProfiles.ts
      useGamepadNav.ts      # Controller/gamepad navigation
      useLaunchState.ts
      useProfile.ts
    styles/                 # CSS with crosshook-* BEM custom properties
    types/                  # TypeScript type definitions
```

## Build Commands

```bash
# Development mode (Tauri dev server with hot reload)
./scripts/dev-native.sh

# Production build (AppImage output)
./scripts/build-native.sh

# Binary-only build (no AppImage packaging)
./scripts/build-native.sh --binary-only

# Run core crate tests
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core

# Container-based build (for reproducible builds)
./scripts/build-native-container.sh
```

## Code Conventions

### Rust

- `snake_case` for functions, variables, modules
- Modules map 1:1 to feature domains (community, export, launch, profile, settings, steam)
- Tauri IPC commands live in `src-tauri/src/commands/` with one file per domain
- Error handling: return `Result<T, String>` from Tauri commands for frontend consumption

### React / TypeScript

- `PascalCase` for components and type names
- `camelCase` for hooks, functions, and variables
- One component per file in `src/components/`
- Custom hooks in `src/hooks/` prefixed with `use`
- CSS custom properties follow `crosshook-*` BEM naming

### General

- No `any` types in TypeScript — use proper types
- Throw errors early; do not use silent fallbacks
- Keep functions small and single-purpose

## Important Notes

- CrossHook is a **native Linux application**. It is NOT a Windows binary and does NOT run under WINE/Proton. Games and trainers run under Proton; CrossHook itself runs natively.
- Distributed as an AppImage for Linux desktop and Steam Deck.
- Three launch modes: **Steam App Launch** (via `steam://run`), **Proton Run** (direct `proton run`), **Native** (direct execution).
- Profiles and settings use TOML format.
- Community profiles use git-based taps for sharing.
- No test framework for the frontend. Rust tests exist in `crosshook-core`.
- Environment management uses `direnv` with `.envrc` and `dotenvx` for encrypted env vars.
- Never commit `.env`, `.env.encrypted`, or `.env.keys` files.

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
- Build verification checklist (`./scripts/build-native.sh`, `cargo test`)
- Conditional checks for launch/, steam/, profile/, components/, hooks/, and runtime-helpers/ changes

### Labels

Use the colon-prefixed label taxonomy — never create ad-hoc labels:

- `type:` bug, feature, docs, refactor, compatibility, build, migration
- `area:` injection, memory, process, ui, build, profiles, cli
- `platform:` steam-deck, linux, macos, wine, proton
- `priority:` critical, high, medium, low
- `status:` needs-triage, in-progress, blocked, needs-info
- Standalone: `good first issue`, `help wanted`, `duplicate`, `wontfix`
