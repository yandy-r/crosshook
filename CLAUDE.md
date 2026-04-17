# CrossHook — Agent rules

Normative guidelines for AI agents in this repository. For stack overview, directory map, and pattern detail, see [`AGENTS.md`](AGENTS.md) (and [`.cursorrules`](.cursorrules) if present).

## Precedence

1. System, developer, and explicit user instructions for the task.
2. This file and repo policy in `AGENTS.md` / `.cursorrules`.
3. General best practices when nothing above conflicts.

## MUST / MUST NOT

- **Platform**: CrossHook is a **native Linux** desktop app (Tauri v2, AppImage). It does **not** run under Wine/Proton; it **orchestrates** launching Windows games via Proton/Wine.
- **Host-tool gateway**: Host-tool execution at the Flatpak boundary **must** route through `src/crosshook-native/crates/crosshook-core/src/platform.rs` (`host_command`, `host_std_command`, `host_command_with_env`, `host_command_exists`, and friends). Direct `Command::new("<host-tool>")` for tools in the denylist (`proton`, `umu-run`, `gamescope`, `mangohud`, `winetricks`, `protontricks`, `gamemoderun`) is rejected by `scripts/check-host-gateway.sh`. See [`docs/architecture/adr-0001-platform-host-gateway.md`](docs/architecture/adr-0001-platform-host-gateway.md) for the full contract, scope boundary (does not apply to in-sandbox subprocess code), and escape hatches.
- **Architecture**: Business logic lives in `crosshook-core`. Keep `crosshook-cli` and `src-tauri` thin (IPC and CLI only).
- **Trainer execution parity**: Treat trainer subprocesses by their **actual runtime path**, not just the parent game launch method. Steam profiles still launch trainers through Proton, so Steam trainer launches must stay aligned with `proton_run` semantics for `effective_trainer_gamescope()`, launch optimization env, and `runtime.working_directory`. In Flatpak, if the shell-helper path diverges from the working `proton_run` trainer path, prefer reusing the direct Proton trainer builder and record/analyze the execution as `proton_run` rather than keeping a separate helper-only env reconstruction path.
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
- **React / TypeScript**: `PascalCase` components, `camelCase` hooks/functions; respect strict TS; wrap `invoke()` in hooks for stateful UI; CSS variables in `src/crosshook-native/src/styles/variables.css`; BEM-like `crosshook-*` classes. **Scroll containers**: WebKitGTK scroll is managed by `useScrollEnhance`. Any new `overflow-y: auto` container **must** be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts`, or the enhanced scroll will target a parent container instead, causing dual-scroll jank. Inner scroll containers should also use `overscroll-behavior: contain`.
- **Verification**: After substantive Rust changes, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`. There is **no** configured frontend test framework—use dev/build scripts when UI behavior matters.
- **Environment**: Repo may use `direnv` / `.envrc` and `dotenvx` for local secrets; do not bypass secret-handling conventions.

## Research and planning quality bar

Feature research and plans **must** be: **feature-complete** (no deferred work), **testable**, **maintainable**, **documented**, **data-driven**, **modular**, and **reusable**.

For storage changes, plans must also:

- Explicitly classify each datum as TOML settings, SQLite metadata, or runtime-only state.
- Include a persistence/usability section that addresses migration/backward compatibility, offline expectations, degraded/failure fallback, and what users can view or edit.

## SQLite Metadata DB (summary)

Operational metadata lives in **`~/.local/share/crosshook/metadata.db`** (WAL, `0600`). Migrations: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`.

- **Current schema version**: **23**
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
- **CI**: `.github/workflows/lint.yml` runs Rust, TypeScript, and ShellCheck on every PR

## Commands (short reference)

```bash
./scripts/dev-native.sh
./scripts/dev-native.sh --browser    # browser-only dev mode (no Rust toolchain), loopback only
./scripts/build-native.sh
./scripts/build-native-container.sh
./scripts/build-native.sh --binary-only
./scripts/install-native-build-deps.sh
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
./scripts/lint.sh                    # check all linters
./scripts/lint.sh --fix              # auto-fix all
./scripts/check-host-gateway.sh      # check platform.rs gateway contract (ADR-0001)
./scripts/format.sh                  # format all
```

Primary source root: `src/crosshook-native/`. CI release workflow: `.github/workflows/release.yml`. CI lint workflow: `.github/workflows/lint.yml`. See [`AGENTS.md`](AGENTS.md) § _Browser Dev Mode_ for the mock layer, loopback-only binding, and the `verify:no-mocks` CI sentinel.
