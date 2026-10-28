# CrossHook — Agent rules

Normative guidelines for AI agents in this repository. For stack overview, directory map, and pattern detail, see [`AGENTS.md`](AGENTS.md) (and [`.cursorrules`](.cursorrules) if present).

## Precedence

1. System, developer, and explicit user instructions for the task.
2. This file and repo policy in `AGENTS.md` / `.cursorrules`.
3. General best practices when nothing above conflicts.

## Worktrees

**Strong preference**: work in a git worktree for any non-trivial task (multi-step features, refactors, changes touching multiple files). Worktrees keep the main checkout clean for parallel work, let multiple agents run concurrently without stepping on each other, and make it trivial to abandon a failed attempt (`git worktree remove`). Fall back to the main checkout only when the task is a one-liner, must observe the current working tree state, or worktree creation is blocked (detached HEAD, shallow clone, submodule issues).

- **Preferred parent**: `~/.claude-worktrees/` for all agent-managed worktrees, named `crosshook-<branch>/`. Keeps them outside every repo and trivially bulk-clean.
- **Manual creation**: when invoking `git worktree add` yourself, target `~/.claude-worktrees/crosshook-<branch>/` — never a path inside the current repo.
- **Harness-created worktrees** (`isolation: "worktree"`, `EnterWorktree`): the only way to relocate these is a `WorktreeCreate` hook in `~/.claude/settings.json`. No environment variable or settings key controls the parent directory directly — the hook receives the intended path and returns a replacement path.
- **Repo hygiene**: if the harness has already created `<repo>/.claude/worktrees/`, add `.claude/worktrees/` to `.gitignore` before committing.

### Worktree setup prerequisites

Certain repo scripts (`./scripts/lint.sh`, `./scripts/format.sh`) require extra setup when run from a worktree root because `node_modules` isn't shared with the main checkout:

- **In worktree root**: `npm install -D --no-save typescript@{current-project-version} biome`
- `cd src/crosshook-native && npm ci`

## MUST / MUST NOT

- **Platform**: CrossHook is a **native Linux** desktop app (Tauri v2, Flatpak distribution). It does **not** run under Wine/Proton; it **orchestrates** launching Windows games via Proton/Wine.
- **Host-tool gateway**: Host-tool execution at the Flatpak boundary **must** route through the `crosshook-core` platform gateway (`src/crosshook-native/crates/crosshook-core/src/platform/gateway.rs`: `host_command`, `host_std_command`, `host_command_with_env`, `host_command_exists`, and friends). Direct `Command::new("<host-tool>")` for tools in the denylist (`proton`, `umu-run`, `gamescope`, `mangohud`, `winetricks`, `protontricks`, `gamemoderun`, `xdg-open`, `gio`, `kde-open`, `gnome-open`, `flatpak-spawn`) or `open::that`/`opener::open` outside the gateway is rejected by `scripts/check-host-gateway.sh`. See [`docs/architecture/adr-0001-platform-host-gateway.md`](docs/architecture/adr-0001-platform-host-gateway.md) for the full contract, scope boundary (does not apply to in-sandbox subprocess code), and escape hatches.
- **Architecture**: Domain logic lives in `crosshook-core`. `crosshook-app` holds bootstrap, the job registry, and the toolkit-independent services (`ProfileWriter`, `LaunchService`, settings, catalogs, diagnostics, health, recent files, onboarding, host tools, offline, startup scans, the background portal, storage, mods, collections, launcher export, version, Steam, community, Lutris, discovery and ProtonDB). Keep `src-tauri` and `crosshook-cli` thin. Profile, launch, settings, catalog, diagnostics, health, recent-files, onboarding, host-tools, offline, background, storage, mods, collections, launcher-export, version, Steam, community, Lutris, discovery, ProtonDB and game-metadata commands delegate to `crosshook-app`; other Tauri commands call `crosshook-core`. The CLI calls `crosshook-core`, and uses `stream_child_log` and `read_log_tail` for launch logs.
- **Profile writes**: Every profile mutation (save, delete, rename, duplicate, favorite, section/preset saves, rollback, legacy/Lutris/community imports, migration apply, ProtonDB accept) goes through `crosshook_app::services::profiles::ProfileWriter`, which validates and trims the profile name before any side effect and serializes writes under one lock; Tauri commands only delegate to it. Do not call `ProfileStore` mutators, core importers that write profiles, metadata observe, or config-revision helpers from `src-tauri`. Only full save, rename, duplicate, rollback, migration, imports and delete invalidate the cached health snapshot; section/optimization/command-argument saves, presets and ProtonDB accepts do not, and favorites are metadata-only. The `profiles-changed` event carries `{ reason: { kind, ... }, names }` (`ProfileChangeReason`, snake*case `kind`; `names: []` means conservative resync); frontend listeners parse it with `parseProfilesChanged` and skip health re-validation for `favorite_toggled`; the health listener coalesces other changes into one in-flight batch plus at most one trailing re-run, and drops stale results. See [`src/crosshook-native/crates/crosshook-app/README.md`](src/crosshook-native/crates/crosshook-app/README.md) § \_ProfileWriter*.
- **Community, Lutris, discovery and ProtonDB**: `CommunityService`, `LutrisService`, `DiscoveryService` and `ProtonDbService` live in `crosshook_app::services::{community,lutris,discovery,protondb}`; the 19 Tauri commands (`community_*`, `lutris_*`, `discovery_*`, `protondb_*`) are thin delegates and their names and Serde shapes are unchanged. `Services::new` builds **one** `ProfileWriter` and clones it into `profiles`, `community` and `lutris`, so every import shares the one write lock. `community_sync` runs as a supervised, cancellable `community-sync` job: concurrent requests queue (no `Conflict`), and cancelling the job (or app shutdown) kills the git child and aborts the sync. Cancellation is backend-only; Classic has no cancel button and there is no generic job-cancel IPC. Tap add validates and normalizes URL, branch and pin before anything is written and dedupes on the normalized `(url, branch, pinned_commit)` triple; `remove_tap` is service-only. Offline mode (`offline_mode`) makes `sync` return `Unavailable` before any git run and serves external search from fresh or stale cache only (`Unavailable` when nothing is cached); the catalog, indexed lists and ProtonDB are not gated. Containment: `community_import_profile` (writes) requires a regular `.json` file inside a subscribed tap workspace (`crosshook_core::fs_util::ensure_within_workspace`), while `community_prepare_import` is a read-only preview of a user-picked file that still accepts any local `.json` but reads its canonical path; the indexer skips symlinks. This narrows but does not eliminate the check-then-read race (no `openat2` pinned handles). Errors carry no paths, git stderr or SQL. See [`src/crosshook-native/crates/crosshook-app/README.md`](src/crosshook-native/crates/crosshook-app/README.md) § _Community, Lutris, discovery and ProtonDB_.
- **Launch**: Every game/trainer launch goes through `crosshook_app::services::launch::LaunchService` — Tauri commands are thin delegates plus a per-session relay mapping `LaunchEvent`s to `launch-log`/`launch-diagnostic`/`launch-complete`/`injection-log` (each payload carries `session_id`); `crosshook-cli` reuses only `stream_child_log`/`read_log_tail`. The service owns the one `LaunchSessionRegistry` (via `Services`; never `.manage(Arc<LaunchSessionRegistry>)` separately), spawns through the `ProcessSpawner` seam, gates the watchdog on the pure `gamescope_required(request, kind, env)` predicate (core's `should_skip_gamescope` decides builder args only), and has **no kill path** (no `killpg`/process groups/`JobRegistry` jobs/kill on the spawner). `Exited` is published exactly once, last, under a drop-safe finalizer guard.
- **Trainer execution parity**: Treat trainer subprocesses by their **actual runtime path**, not just the parent game launch method. Steam profiles still launch trainers through Proton, so Steam trainer launches must stay aligned with `proton_run` semantics for `effective_trainer_gamescope()`, launch optimization env, and `runtime.working_directory`. In Flatpak, if the shell-helper path diverges from the working `proton_run` trainer path, prefer reusing the direct Proton trainer builder and record/analyze the execution as `proton_run` rather than keeping a separate helper-only env reconstruction path. Parity extends to **lifecycle cleanup**: trainer launches register with `LaunchSessionRegistry` (`src/crosshook-native/crates/crosshook-core/src/launch/session/`) and spawn a gamescope watchdog when the same predicate the game path uses is true, so trainer-side gamescope is torn down on trainer exit _and_ on parent-game teardown via the registry's `cancel_linked_children` cascade. Never kill another session's process tree — each session owns its own `ShutdownTarget`.
- **Startup contract**: Startup runs in `crosshook_app::bootstrap` as two phases — `pre_runtime` (Flatpak gate + XDG override → first-run Flatpak migration → legacy app-id migration → UI `EnvPolicy`; synchronous, expected to run before any Tokio runtime, spawned thread, or GTK — a multi-threaded entry is warned about and recorded as `Degraded`, not fatal) then `App::bootstrap` (stores, logging, catalogs, reconciliation, background jobs; returns a `BootstrappedApp` that derefs to `App`). There are **no startup sleeps**: the React shell pulls the `get_startup_report` command on mount (auto-load profile, onboarding, Flatpak migration outcome, degraded stores, job results). Once-per-page-load consumers use the memoized report; job consumers (health, offline readiness) subscribe to the terminal `startup-job-finished` event (`{ job, state }`, emitted for `complete`, `failed`, and `skipped`) and then re-read a fresh report (`fetchStartupReport({ fresh: true })`) — never an immutable memo. `src-tauri` only bridges job-completion events. In `crosshook-core`, use `platform::base_dirs()` — never `BaseDirs::new()` directly. `scripts/check-startup-path.sh` enforces this. See [ADR-0004](docs/architecture/adr-0004-flatpak-per-app-isolation.md) § _Startup entry point_.
- **Tauri IPC**: Expose backend operations as `#[tauri::command]` handlers with **`snake_case` names** matching frontend `invoke()` calls. Use **Serde** on all types that cross the IPC boundary.
- **Secrets**: **Never** commit `.env`, `.env.encrypted`, or `.env.keys`.
- **Issues**: Open all CrossHook issues on GitHub (`origin`, `https://github.com/yandy-r/crosshook`) using the forms under [`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/). **Do not** create title-only or template-bypass issues. If `gh issue create --template` fails (e.g. `no templates found`), create the issue via the GitHub API with a body that mirrors the form fields, then apply the correct labels. `Forgejo #N` references in older docs and code comments point at the retired Forgejo tracker and are historical only. For feature issues that introduce or change **persisted** data, the issue body must include a **Storage boundary** subsection (classify each datum as TOML settings, SQLite metadata, or runtime-only) and a short **Persistence & usability** subsection (migration/backward compatibility, offline expectations, degraded behavior when persistence is unavailable, and what users can view or edit).
- **Pull requests**: Open all CrossHook PRs on GitHub and follow [`.github/pull_request_template.md`](.github/pull_request_template.md). **Always** link the related issue (e.g. `Closes #…`, or `Part of #…` for child PRs of tracker issues). **Label** PRs using the taxonomy below—**never** invent ad-hoc labels. **PR titles must be Conventional Commits** (`<type>[scope]: <description>`) and are validated by [`.github/workflows/pr-title.yml`](.github/workflows/pr-title.yml); placeholder prefixes like `[WIP]`, `Draft:`, or `Initial plan` are rejected and will block merge. Merges are squash-only, so the PR title lands as the commit subject in `CHANGELOG.md` verbatim—write it as it should read in `git log`. Use GitHub's native draft PR status for work-in-progress, not a title prefix.
- **Releases**: Before tagging, run `./scripts/prepare-release.sh …` (regenerates/validates `CHANGELOG.md` per repo scripts). Branches and release tags push to GitHub `origin`; tags trigger `.github/workflows/release.yml`, which publishes the Flatpak bundle to the GitHub Release. The release workflow validates changelog sections; noisy commits should fail prep, not ship to releases.
- **Commits / changelog**: `CHANGELOG.md` is driven by **git-cliff** and release notes. Use **Conventional Commits** for user-facing work (`feat(…)`, `fix(…)`, `docs(…)`, `build(…)`, …). Avoid vague titles (`fix stuff`, `Update README.md`, …) for changes that may ship. For user-visible changes, **write the title as you want it to appear** in `CHANGELOG.md`.
- **Internal docs commits**: Commits that change files under `./docs/plans`, `./docs/research`, or `./docs/internal` **must** use a `docs(internal): …` prefix. Other non-user-facing churn: prefer `chore(…): …` or `docs(internal): …` so it stays out of release notes.
- **MCP**: When an MCP server fits the task (GitHub, docs, browser, etc.), **prefer it**. **Read** each tool’s schema/descriptor before calling. If MCP is missing or unsuitable, use `gh`, repo scripts, or other local tools—**do not** block on MCP.
- **CI**: All CI runs on GitHub Actions under [`.github/workflows/`](.github/workflows/) (the repository is public, so Actions usage is free). Collaboration checks (`lint.yml`, `pr-title.yml`, `fixture-lint.yml`) run on PRs and pushes to `main`; `release.yml` publishes tagged releases.
- **`gh` CLI quoting**: Always **single-quote** `--jq` and `--template` expressions so the shell cannot glob-expand `[]`, pipe through `|`, or mangle `()` / `.`. Example: `gh issue list --json number,title --jq '.[] | "\(.number): \(.title)"'`—never leave `.[]` or the `|` unquoted. For multi-line filters, write the expression to a file and use `--jq "$(cat …)"` or a here-doc rather than long inline escapes.
- **GitHub tooling**: Use `gh` or the GitHub MCP server for issue, PR, and release work. When calls fail, diagnose auth, remote selection, and shell quoting before retrying.
- **Research and planning**: Feature research and plans **must** be: **feature-complete** (no deferred work), **testable**, **maintainable**, **documented**, **data-driven**, **modular**, and **reusable**.
- **Persistence planning**: Every feature plan/research artifact must classify new or changed data as one of: user-editable preferences (TOML settings), operational/history/cache metadata (SQLite metadata DB), or ephemeral runtime state (memory only). Plans must include a short persistence/usability section covering migration/backward compatibility, offline behavior, degraded fallback behavior, and user visibility/editability expectations.
- **Large features**: Split into smaller phases and tasks with clear dependencies and order of execution.
- **File size (~500 lines)**: Aim for **around 500 lines** per file as a soft cap. Files that drift meaningfully past that **must** be refactored into smaller modules unless the content is inherently contiguous (generated code, schemas, large test fixtures). The intent is maintainability, not a hard ceiling.
- **Modularity & reuse**: Code **must** decompose into small, cohesive units — submodules, libraries, or reusable components — with a clear public surface and minimal cross-module coupling. **No copy-paste duplication** (DRY): extract shared logic into a shared module. Prefer composition over inheritance. Avoid circular dependencies.
- **Single responsibility**: Each function, module, and component **must** have one clear reason to exist. Split when a unit grows more than one responsibility.

## SHOULD (implementation)

- **Default to worktrees**: For non-trivial work (multi-step features, refactors, changes touching multiple files), start in a git worktree at `~/.claude-worktrees/crosshook-<branch>/` instead of the main checkout. See [Worktrees](#worktrees) above for the full contract, including the setup prerequisites required for `./scripts/lint.sh` and `./scripts/format.sh` to succeed. Fall back to the main checkout only when the task is a one-liner, must observe the current working tree state, or worktree creation is blocked.
- **Naming**: Intention-revealing names for functions, types, and modules. Public APIs should read like documentation.
- **No dead code**: Remove unused code, imports, and commented-out blocks. Git preserves history.
- **Dependency hygiene**: Before adding a new dependency, check whether an existing one does the job. New deps need a justification (maintenance cost, license, security).
- **Fail fast at boundaries**: Validate inputs at module and system boundaries; propagate via typed errors. Never silently swallow errors.
- **Tests alongside changes**: New or modified behavior ships with tests in the same change.
- **Rust**: `snake_case`; modules as directories with `mod.rs`; errors via `Result` with `anyhow` or project error types.
- **React / TypeScript**: `PascalCase` components, `camelCase` hooks/functions; respect strict TS; wrap `invoke()` in hooks for stateful UI; CSS variables in `src/crosshook-native/src/styles/variables.css`; BEM-like `crosshook-*` classes. **Scroll containers**: WebKitGTK scroll is managed by `useScrollEnhance`. Any new `overflow-y: auto` container **must** be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts`, or the enhanced scroll will target a parent container instead, causing dual-scroll jank. Inner scroll containers should also use `overscroll-behavior: contain`.
- **Verification**: After substantive Rust changes, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`. There is **no** configured frontend test framework—use dev/build scripts when UI behavior matters.
- **Environment**: Repo may use `direnv` / `.envrc` and `dotenvx` for local secrets; do not bypass secret-handling conventions.

## Research and planning quality bar

Feature research and plans **must** be: **feature-complete** (no deferred work), **testable**, **maintainable**, **documented**, **data-driven**, **modular**, and **reusable**.

For storage changes, plans must also:

- Explicitly classify each datum as TOML settings, SQLite metadata, or runtime-only state.
- Include a persistence/usability section that addresses migration/backward compatibility, offline expectations, degraded/failure fallback, and what users can view or edit.

## SQLite Metadata DB (summary)

Operational metadata lives in **`~/.local/share/crosshook/metadata.db`** (WAL, `0600`). Migrations: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs`.

- **Current schema version**: **28**
- **Migration v27→v28**: adds nullable `config_revisions.hmac_signature` — HMAC-SHA256 tamper evidence for config history; the key is a `0600` file `config_history_hmac.key` next to `metadata.db` (never SQLite/TOML/IPC). Legacy rows stay `NULL` and verify hash-only (see [ADR-0005](docs/architecture/adr-0005-config-history-hmac.md)).
- **Migration v26→v27**: adds `prefix_version_restore_journal` — restart-safe Wine-prefix Windows-version restore records (see AGENTS.md § _Prefix dependency recovery_).
- **Migration v25→v26**: adds `profile_mods` — per-profile mod coexistence registry (machine-local; drives trainer coexistence advisories; never exported with community profiles).
- **Schema bump policy**: any schema version change must update `SUPPORTED_MAX_VERSION` in `migrations/mod.rs`, both docs, pass `scripts/check-schema-version.sh`, and preserve the core migration regression test invariant (latest migration == `SUPPORTED_MAX_VERSION`) — see [`docs/internal-docs/metadata-schema-policy.md`](docs/internal-docs/metadata-schema-policy.md).
- **Migration v24→v25**: adds nullable `community_profiles.trainer_loading_mode` for the discovery catalog loading-mode facet; resets tap watermarks (`last_head_commit`) so the next community sync backfills the column offline. No new tables.
- **Migration v23→v24**: adds `umu_gameid_lookup_cache` for the opt-in umu GAMEID HTTP resolver, including cached hits, misses, and stale fallback metadata.
- **Migration v22→v23**: evicts `proton_release_catalog` rows so additive DTO fields (e.g. `published_at`) repopulate on next fetch. No schema change.
- **Tables added in v22**: `proton_release_catalog` (native Proton download manager catalog cache; migration also evicts legacy `protonup:catalog:*` entries from `external_cache_entries`)
- **Tables added in v21**: `host_readiness_catalog`, `readiness_nag_dismissals`, `host_readiness_snapshots`

Full table inventory, persistence classification, and `external_cache_entries` payload limits: [`AGENTS.md`](AGENTS.md) § _SQLite Metadata DB_.

## Labels (use only these families)

- `type:` bug, feature, docs, refactor, compatibility, build, migration
- `area:` injection, memory, process, ui, build, profiles, cli, launch, security
- `platform:` steam-deck, linux, macos, wine, proton
- `priority:` critical, high, medium, low
- `status:` needs-triage, in-progress, blocked, needs-info
- Standalone: `good first issue`, `help wanted`, `duplicate`, `wontfix`

## Code quality tooling

- **Rust formatting**: `cargo fmt` via `rustfmt.toml` at `src/crosshook-native/`
- **Rust linting**: `cargo clippy` with workspace lints in `Cargo.toml` (`-D warnings` in CI)
- **TypeScript linting/formatting**: Biome (`biome.json` at `src/crosshook-native/`)
- **Markdown/JSON formatting**: Prettier (`.prettierrc` at repo root)
- **Pre-commit**: lefthook (`lefthook.yml` at repo root) — run `./scripts/setup-dev-hooks.sh` (see [install options](https://lefthook.dev/install/); not on crates.io)
- **CI**: `.github/workflows/lint.yml` runs Rust, TypeScript, ShellCheck, and smoke checks on PRs and pushes to `main`; `.github/workflows/release.yml` publishes Flatpak releases from tags pushed to `origin`.

## Commands (short reference)

```bash
./scripts/dev-native.sh
./scripts/dev-native.sh --browser    # browser-only dev mode (no Rust toolchain), loopback only
./scripts/build-release-binary.sh     # production Tauri binary used as Flatpak input
./scripts/build-flatpak.sh --rebuild --strict
./scripts/install-native-build-deps.sh
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
./scripts/lint.sh                    # check all linters
./scripts/lint.sh --fix              # auto-fix all
./scripts/check-host-gateway.sh      # check platform gateway contract (ADR-0001)
./scripts/format.sh                  # format all
npm test                             # Vitest (happy-dom)
npm run test:watch                   # Vitest watch
npm run test:coverage                # Vitest + coverage
npm run test:smoke                   # Playwright smoke (browser dev mode)
npm run test:smoke:update
npm run test:smoke:install
npm run typecheck
```

Primary source root: `src/crosshook-native/`. CI workflows: `.github/workflows/` (release: `.github/workflows/release.yml`). See [`AGENTS.md`](AGENTS.md) § _Browser Dev Mode_ for the mock layer, loopback-only binding, and the `verify:no-mocks` CI sentinel.

## Branching & releases

[`RELEASING.md`](RELEASING.md) is the source of truth for branches and releases. Branch
off `main` and PR back into it; every release is tagged from `main`. Never merge one
long-lived branch into another to sync it.
