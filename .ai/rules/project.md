# CrossHook — Agent rules

Vendor-neutral mirror of the project rules. Canonical source: [`CLAUDE.md`](../../CLAUDE.md). Cursor variant: [`.cursor/rules/project.mdc`](../../.cursor/rules/project.mdc). Stack / directory / SQLite detail: [`AGENTS.md`](../../AGENTS.md).

## Precedence

1. System, developer, and explicit user instructions for the task.
2. This file and repo policy in `AGENTS.md` / `CLAUDE.md`.
3. General best practices when nothing above conflicts.

## MUST / MUST NOT

- **Platform**: CrossHook is a **native Linux** desktop app (Tauri v2, Flatpak distribution). It does **not** run under Wine/Proton; it **orchestrates** launching Windows games via Proton/Wine.
- **Architecture**: Business logic lives in `crosshook-core`. Keep `crosshook-cli` and `src-tauri` thin (IPC and CLI only).
- **Trainer execution parity**: Treat trainer subprocesses by their **actual runtime path**, not just the parent game launch method. Steam profiles still launch trainers through Proton, so Steam trainer launches must stay aligned with `proton_run` semantics for `effective_trainer_gamescope()`, launch optimization env, and `runtime.working_directory`.
- **Tauri IPC**: Expose backend operations as `#[tauri::command]` handlers with **`snake_case` names** matching frontend `invoke()` calls. Use **Serde** on all types that cross the IPC boundary.
- **Secrets**: **Never** commit `.env`, `.env.encrypted`, or `.env.keys`.
- **Issues**: Open all CrossHook issues on GitHub (`origin`, `https://github.com/yandy-r/crosshook`) using the forms under [`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/). **Do not** create title-only or template-bypass issues. If `gh issue create --template` fails (e.g. `no templates found`), create the issue via the GitHub API with a body that mirrors the form fields, then apply the correct labels. `Forgejo #N` references in older docs and code comments point at the retired Forgejo tracker and are historical only. For feature issues that introduce or change **persisted** data, the issue body must include a **Storage boundary** subsection (classify each datum as TOML settings, SQLite metadata, or runtime-only) and a short **Persistence & usability** subsection (migration/backward compatibility, offline expectations, degraded behavior when persistence is unavailable, and what users can view or edit).
- **Pull requests**: Open all CrossHook PRs on GitHub and follow [`.github/pull_request_template.md`](.github/pull_request_template.md). **Always** link the related issue (e.g. `Closes #…`, or `Part of #…` for child PRs of tracker issues). **Label** PRs using the taxonomy below—**never** invent ad-hoc labels. **PR titles must be Conventional Commits** (`<type>[scope]: <description>`) and are validated by [`.github/workflows/pr-title.yml`](.github/workflows/pr-title.yml); placeholder prefixes like `[WIP]`, `Draft:`, or `Initial plan` are rejected and will block merge. Merges are squash-only, so the PR title lands as the commit subject in `CHANGELOG.md` verbatim—write it as it should read in `git log`. Use GitHub's native draft PR status for work-in-progress, not a title prefix.
- **Releases**: Before tagging, run `./scripts/prepare-release.sh …` (regenerates/validates `CHANGELOG.md` per repo scripts). Branches and release tags push to GitHub `origin`; tags trigger `.github/workflows/release.yml`, which publishes the Flatpak bundle to the GitHub Release. The release workflow validates changelog sections; noisy commits should fail prep, not ship to releases.
- **Commits / changelog**: `CHANGELOG.md` is driven by **git-cliff**. Use **Conventional Commits** for user-facing work. For user-visible changes, write the title as you want it to appear in `CHANGELOG.md`.
- **Internal docs commits**: Files under `docs/plans`, `docs/research`, or `docs/internal` must use `docs(internal): …`. Other non-user-facing churn: prefer `chore(…): …`.
- **MCP**: When an MCP server fits the task, prefer it. Read each tool's schema/descriptor before calling.
- **Research and planning**: Feature research and plans must be feature-complete, testable, maintainable, documented, data-driven, modular, and reusable.
- **Persistence planning**: Every plan/research artifact must classify new or changed data as user-editable preferences (TOML), operational/history/cache metadata (SQLite), or ephemeral runtime state (memory). Include migration, offline, degraded-fallback, and user-visibility notes.
- **Large features**: Split into smaller phases and tasks with clear dependencies and order of execution.

## SHOULD (implementation)

- **Rust**: `snake_case`; modules as directories with `mod.rs`; errors via `Result` with `anyhow` or project error types.
- **React / TypeScript**: `PascalCase` components, `camelCase` hooks/functions; strict TS; wrap `invoke()` in hooks; BEM-like `crosshook-*` CSS classes. New `overflow-y: auto` containers must be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts` or scroll enhancement will target the wrong container.
- **Verification**: After substantive Rust changes, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`. There is no configured frontend test framework — use dev/build scripts when UI behavior matters.
- **Environment**: Repo may use `direnv` / `.envrc` and `dotenvx` for local secrets; do not bypass secret-handling conventions.

## Labels (use only these families)

- `type:` bug, feature, docs, refactor, compatibility, build, migration
- `area:` injection, memory, process, ui, build, profiles, cli, launch, security
- `platform:` steam-deck, linux, macos, wine, proton
- `priority:` critical, high, medium, low
- `status:` needs-triage, in-progress, blocked, needs-info
- Standalone: `good first issue`, `help wanted`, `duplicate`, `wontfix`

## Verification

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` and `./scripts/lint.sh` before marking work complete.
