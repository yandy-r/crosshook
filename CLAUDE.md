# CrossHook — Agent rules

Normative guidelines for AI agents in this repository. For stack overview, directory map, and pattern detail, see [`AGENTS.md`](AGENTS.md) (and [`.cursorrules`](.cursorrules) if present).

## Precedence

1. System, developer, and explicit user instructions for the task.
2. This file and repo policy in `AGENTS.md` / `.cursorrules`.
3. General best practices when nothing above conflicts.

## MUST / MUST NOT

- **Platform**: CrossHook is a **native Linux** desktop app (Tauri v2, AppImage). It does **not** run under Wine/Proton; it **orchestrates** launching Windows games via Proton/Wine.
- **Architecture**: Business logic lives in `crosshook-core`. Keep `crosshook-cli` and `src-tauri` thin (IPC and CLI only).
- **Tauri IPC**: Expose backend operations as `#[tauri::command]` handlers with **`snake_case` names** matching frontend `invoke()` calls. Use **Serde** on all types that cross the IPC boundary.
- **Secrets**: **Never** commit `.env`, `.env.encrypted`, or `.env.keys`.
- **Issues**: Use the YAML form templates under [`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/). **Do not** create title-only or template-bypass issues. If `gh issue create --template` fails (e.g. `no templates found`), create the issue via GitHub API/tooling with a body that mirrors the form fields, then apply correct labels—**not** a vague one-liner.
- **Pull requests**: Follow [`.github/pull_request_template.md`](.github/pull_request_template.md). **Always** link the related issue (e.g. `Closes #…`). **Label** PRs using the taxonomy below—**never** invent ad-hoc labels.
- **Releases**: Before tagging, run `./scripts/prepare-release.sh …` (regenerates/validates `CHANGELOG.md` per repo scripts). The release workflow validates changelog sections; noisy commits should fail prep, not ship to GitHub Releases.
- **Commits / changelog**: `CHANGELOG.md` is driven by **git-cliff** and release notes. Use **Conventional Commits** for user-facing work (`feat(…)`, `fix(…)`, `docs(…)`, `build(…)`, …). Avoid vague titles (`fix stuff`, `Update README.md`, …) for changes that may ship. For user-visible changes, **write the title as you want it to appear** in `CHANGELOG.md`.
- **Internal docs commits**: Commits that change files under `./docs/plans`, `./docs/research`, or `./docs/internal` **must** use a `docs(internal): …` prefix. Other non-user-facing churn: prefer `chore(…): …` or `docs(internal): …` so it stays out of release notes.
- **MCP**: When an MCP server fits the task (GitHub, docs, browser, etc.), **prefer it**. **Read** each tool’s schema/descriptor before calling. If MCP is missing or unsuitable, use `gh`, repo scripts, or other local tools—**do not** block on MCP.
- **Research and planning**: Feature research and plans **must** be: **feature-complete** (no deferred work), **testable**, **maintainable**, **documented**, **data-driven**, **modular**, and **reusable**.

## SHOULD (implementation)

- **Rust**: `snake_case`; modules as directories with `mod.rs`; errors via `Result` with `anyhow` or project error types.
- **React / TypeScript**: `PascalCase` components, `camelCase` hooks/functions; respect strict TS; wrap `invoke()` in hooks for stateful UI; CSS variables in `src/crosshook-native/src/styles/variables.css`; BEM-like `crosshook-*` classes.
- **Verification**: After substantive Rust changes, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`. There is **no** configured frontend test framework—use dev/build scripts when UI behavior matters.
- **Environment**: Repo may use `direnv` / `.envrc` and `dotenvx` for local secrets; do not bypass secret-handling conventions.

## Research and planning quality bar

Feature research and plans **must** be: **feature-complete** (no deferred work), **testable**, **maintainable**, **documented**, **data-driven**, **modular**, and **reusable**.

## Labels (use only these families)

- `type:` bug, feature, docs, refactor, compatibility, build, migration
- `area:` injection, memory, process, ui, build, profiles, cli
- `platform:` steam-deck, linux, macos, wine, proton
- `priority:` critical, high, medium, low
- `status:` needs-triage, in-progress, blocked, needs-info
- Standalone: `good first issue`, `help wanted`, `duplicate`, `wontfix`

## Commands (short reference)

```bash
./scripts/dev-native.sh
./scripts/build-native.sh
./scripts/build-native-container.sh
./scripts/build-native.sh --binary-only
./scripts/install-native-build-deps.sh
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

Primary source root: `src/crosshook-native/`. CI release workflow: `.github/workflows/release.yml`.
