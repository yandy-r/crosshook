# CrossHook — Agent rules

Normative guidelines for AI agents in this repository. Stack overview, directory map, and pattern detail are in the sections below. Cross-reference [`.cursorrules`](.cursorrules) for cursor-specific tooling (kept in sync with this file).

## Precedence

1. System, developer, and explicit user instructions for the task.
2. This file and repo policy in `AGENTS.md` / `.cursorrules`.
3. General best practices when nothing above conflicts.

## MUST / MUST NOT

- **Platform**: CrossHook is a **native Linux** desktop app (Tauri v2, AppImage). It does **not** run under Wine/Proton; it **orchestrates** launching Windows games via Proton/Wine.
- **Architecture**: Business logic lives in `crosshook-core`. Keep `crosshook-cli` and `src-tauri` thin (IPC and CLI only).
- **Tauri IPC**: Expose backend operations as `#[tauri::command]` handlers with **`snake_case` names** matching frontend `invoke()` calls. Use **Serde** on all types that cross the IPC boundary.
- **Secrets**: **Never** commit `.env`, `.env.encrypted`, or `.env.keys`.
- **Issues**: Use the YAML form templates under [`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/). **Do not** create title-only or template-bypass issues. If `gh issue create --template` fails (e.g. `no templates found`), create the issue via GitHub API/tooling with a body that mirrors the form fields, then apply correct labels—**not** a vague one-liner. For feature issues that introduce or change **persisted** data, the issue body must include a **Storage boundary** subsection (classify each datum as TOML settings, SQLite metadata, or runtime-only) and a short **Persistence & usability** subsection (migration/backward compatibility, offline expectations, degraded behavior when persistence is unavailable, and what users can view or edit).
- **Pull requests**: Follow [`.github/pull_request_template.md`](.github/pull_request_template.md). **Always** link the related issue (e.g. `Closes #…`). **Label** PRs using the taxonomy below—**never** invent ad-hoc labels.
- **Releases**: Before tagging, run `./scripts/prepare-release.sh …` (regenerates/validates `CHANGELOG.md` per repo scripts). The release workflow validates changelog sections; noisy commits should fail prep, not ship to GitHub Releases.
- **Commits / changelog**: `CHANGELOG.md` is driven by **git-cliff** and release notes. Use **Conventional Commits** for user-facing work (`feat(…)`, `fix(…)`, `docs(…)`, `build(…)`, …). Avoid vague titles (`fix stuff`, `Update README.md`, …) for changes that may ship. For user-visible changes, **write the title as you want it to appear** in `CHANGELOG.md`.
- **Internal docs commits**: Commits that change files under `./docs/plans`, `./docs/research`, or `./docs/internal` **must** use a `docs(internal): …` prefix. Other non-user-facing churn: prefer `chore(…): …` or `docs(internal): …` so it stays out of release notes.
- **MCP**: When an MCP server fits the task (GitHub, docs, browser, etc.), **prefer it**. **Read** each tool’s schema/descriptor before calling. If MCP is missing or unsuitable, use `gh`, repo scripts, or other local tools—**do not** block on MCP.
- **Research and planning**: Feature research and plans **must** be: **feature-complete** (no deferred work), **testable**, **maintainable**, **documented**, **data-driven**, **modular**, and **reusable**.
- **Persistence planning**: Every feature plan/research artifact must classify new or changed data as one of: user-editable preferences (TOML settings), operational/history/cache metadata (SQLite metadata DB), or ephemeral runtime state (memory only). Plans must include a short persistence/usability section covering migration/backward compatibility, offline behavior, degraded fallback behavior, and user visibility/editability expectations.
- **Large features**: Must be split into smaller, manageable phases and tasks, with clear dependencies and a clear order of execution.

## SHOULD (implementation)

- **Rust**: `snake_case`; modules as directories with `mod.rs`; errors via `Result` with `anyhow` or project error types.
- **React / TypeScript**: `PascalCase` components, `camelCase` hooks/functions; respect strict TS; wrap `invoke()` in hooks for stateful UI; CSS variables in `src/crosshook-native/src/styles/variables.css`; BEM-like `crosshook-*` classes. **Route layout**: reuse the shared contract in `src/crosshook-native/src/styles/layout.css` (`crosshook-page-scroll-shell--fill`, `crosshook-route-stack`, `crosshook-route-stack__body--scroll` / `__body--fill`, `crosshook-route-card-host` / `crosshook-route-card-scroll` when a primary card must fill the body without stretching inner grids, `crosshook-route-footer`) instead of one-off viewport height chains on new pages. **Scroll containers**: WebKitGTK scroll is managed by `useScrollEnhance` (`src/crosshook-native/src/hooks/useScrollEnhance.ts`). Any new `overflow-y: auto` container **must** be added to the `SCROLLABLE` selector in that hook, or the enhanced scroll will target a parent container instead, causing dual-scroll jank. Inner scroll containers should also use `overscroll-behavior: contain` to prevent scroll-chaining to outer containers.
- **Verification**: After substantive Rust changes, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`. There is **no** configured frontend test framework—use dev/build scripts when UI behavior matters.
- **Environment**: Repo may use `direnv` / `.envrc` and `dotenvx` for local secrets; do not bypass secret-handling conventions.

## Research and planning quality bar

Feature research and plans **must** be: **feature-complete** (no deferred work), **testable**, **maintainable**, **documented**, **data-driven**, **modular**, and **reusable**.

For storage changes, plans must also:

- Explicitly classify each datum as TOML settings, SQLite metadata, or runtime-only state.
- Include a persistence/usability section that addresses migration/backward compatibility, offline expectations, degraded/failure fallback, and what users can view or edit.

## Labels (use only these families)

- `type:` bug, feature, docs, refactor, compatibility, build, migration
- `area:` injection, memory, process, ui, build, profiles, cli, launch, security
- `platform:` steam-deck, linux, macos, wine, proton
- `priority:` critical, high, medium, low
- `status:` needs-triage, in-progress, blocked, needs-info
- Standalone: `good first issue`, `help wanted`, `duplicate`, `wontfix`

## Commands (short reference)

```bash
./scripts/dev-native.sh
./scripts/dev-native.sh --browser    # browser-only dev mode (no Rust toolchain), loopback only
./scripts/build-native.sh
./scripts/build-native-container.sh
./scripts/build-native.sh --binary-only
./scripts/install-native-build-deps.sh
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

Primary source root: `src/crosshook-native/`. CI release workflow: `.github/workflows/release.yml`.

### Browser Dev Mode

`./scripts/dev-native.sh --browser` (or `--web`) starts Vite at `http://localhost:5173` with all `invoke()` and `listen()` calls served by hand-rolled mock handlers — no Rust toolchain or running Tauri backend is required. The server binds loopback only; `--host 0.0.0.0` is unsupported per security policy (BR-9). Because mock handlers return synthetic data, real Tauri behavior must be re-verified with `./scripts/dev-native.sh` (no flag) before merging any UI changes. To add or extend handlers for new commands, see `src/crosshook-native/src/lib/mocks/README.md`. The CI sentinel `verify:no-mocks` runs after every AppImage build and will refuse any production bundle that contains mock code — keeping the mock layer strictly development-only.

---

## Stack Overview

| Layer                  | Technology                         | Notes                                                                                         |
| ---------------------- | ---------------------------------- | --------------------------------------------------------------------------------------------- |
| Desktop shell          | **Tauri v2**                       | Rust backend + WebView frontend; packaged as AppImage                                         |
| Core business logic    | **Rust** (`crosshook-core` crate)  | All launch orchestration, profile management, community taps, metadata persistence, settings  |
| IPC layer              | **Tauri commands** (`src-tauri`)   | Thin wrappers over `crosshook-core`; `snake_case` command names; Serde on all boundary types  |
| Frontend               | **React 18 + TypeScript** (strict) | Vite dev server; `invoke()` wrapped in custom hooks; BEM-like `crosshook-*` CSS classes       |
| Persistence — settings | **TOML** (`settings.toml`)         | User-editable preferences; deserialized via `AppSettingsData`                                 |
| Persistence — metadata | **SQLite** via `rusqlite`          | WAL mode, `0600` permissions; schema migrations in `crosshook-core`; see SQLite section below |
| CLI                    | `crosshook-cli` crate              | Thin wrapper over `crosshook-core`; no business logic                                         |

---

## Directory Map

```
src/crosshook-native/              # Primary source root
├── src-tauri/
│   └── src/
│       ├── commands/              # Tauri IPC command handlers (one file per domain)
│       │   ├── launch.rs
│       │   ├── profile.rs
│       │   ├── community.rs
│       │   ├── settings.rs
│       │   └── ...
│       └── lib.rs                 # Tauri app setup; registers all commands
├── crates/
│   └── crosshook-core/
│       └── src/
│           ├── launch/            # Launch orchestration, directives, validation
│           ├── profile/           # Profile load/save, override layers, export
│           ├── metadata/          # MetadataStore, migrations, schema versioning
│           ├── steam/             # Steam manifest parsing, app discovery
│           ├── community/         # Tap management, community profile fetch/merge
│           ├── settings/          # AppSettingsData TOML serde
│           ├── offline/           # Offline readiness snapshots, cache management
│           ├── export/            # Launcher store, stale detection
│           ├── onboarding/        # Guided first-run flows
│           ├── install/           # Installation helpers
│           └── update/            # Self-update logic
└── src/                           # React/TypeScript frontend
    ├── hooks/                     # Custom React hooks wrapping invoke()
    ├── components/                # PascalCase React components
    ├── styles/                    # variables.css (CSS custom properties), theme.css
    └── types/                     # TypeScript interface definitions
```

---

## SQLite Metadata DB

**Location**: `~/.local/share/crosshook/metadata.db`
**Mode**: WAL (write-ahead logging)
**Permissions**: `0600` (owner read/write only)
**Current schema version**: 13
**Access**: `MetadataStore::try_new()` in `crosshook-core`
**Migrations**: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`

### Table inventory

| Table                            | Since schema | Purpose                                                                |
| -------------------------------- | :----------: | ---------------------------------------------------------------------- |
| `profiles`                       |      v1      | Core profile records                                                   |
| `profile_name_history`           |      v1      | Rename audit trail                                                     |
| `launchers`                      |      v3      | Known launcher executables                                             |
| `launch_operations`              |      v3      | Per-launch history and diagnostics                                     |
| `community_taps`                 |      v4      | Subscribed community tap sources                                       |
| `community_profiles`             |      v4      | Fetched community profile snapshots                                    |
| `external_cache_entries`         |      v4      | Generic HTTP response cache (512 KiB payload cap per entry)            |
| `collections`                    |      v4      | Named profile collections                                              |
| `collection_profiles`            |      v4      | Collection ↔ profile membership                                        |
| `health_snapshots`               |      v6      | Periodic profile health check results                                  |
| `version_snapshots`              |      v9      | Game/trainer version correlation records; includes `trainer_file_hash` |
| `bundled_optimization_presets`   |     v10      | Built-in optimization preset definitions                               |
| `profile_launch_preset_metadata` |     v10      | Per-profile preset activation state                                    |
| `config_revisions`               |     v11      | TOML snapshots with SHA-256 for config history/rollback                |
| `optimization_catalog`           |     v12      | Data-driven optimization catalog entries                               |
| `trainer_hash_cache`             |     v13      | SHA-256 hash per trainer per profile                                   |
| `offline_readiness_snapshots`    |     v13      | Offline readiness state snapshots                                      |
| `community_tap_offline_state`    |     v13      | Per-tap offline availability state                                     |

### Persistence design classification

When implementing features, classify every new datum before writing code:

| Kind of data                           | Layer                               | Examples                                                  |
| -------------------------------------- | ----------------------------------- | --------------------------------------------------------- |
| User-editable preferences              | `settings.toml` (`AppSettingsData`) | Toggle switches, API keys, default paths                  |
| Operational / history / cache metadata | SQLite `MetadataStore`              | Launch logs, health snapshots, HTTP cache, version hashes |
| Ephemeral runtime state                | In-memory only                      | Active launch handle, transient UI loading flags          |

Do **not** cache binary blobs (images, archives) in `external_cache_entries` — payloads over 512 KiB store `NULL payload_json` silently. Use the filesystem with a tracking table for large binaries.
