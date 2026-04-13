# Contributing to CrossHook

Thanks for your interest in contributing to CrossHook! This guide covers everything you need to get
started -- from reporting bugs to submitting pull requests.

For general questions or discussion, head to
[GitHub Discussions](https://github.com/yandy-r/crosshook/discussions) rather than opening an issue.

## Reporting Issues

All issues must use one of the provided form templates -- blank issues are disabled.

- [Bug Report](https://github.com/yandy-r/crosshook/issues/new?template=bug_report.yml) -- something
  broken or behaving unexpectedly
- [Feature Request](https://github.com/yandy-r/crosshook/issues/new?template=feature_request.yml) --
  a new capability or improvement
- [Compatibility Report](https://github.com/yandy-r/crosshook/issues/new?template=compatibility_report.yml)
  -- game/trainer/platform compatibility findings

Before filing, search [existing issues](https://github.com/yandy-r/crosshook/issues) to avoid
duplicates.

## Development Setup

### Prerequisites

- **Rust** (stable toolchain)
- **Node.js** 20+ and **npm**
- **System libraries**: GTK3, WebKit2GTK 4.1, libsoup3, OpenSSL, patchelf
- **Optional**: [direnv](https://direnv.net/) (an `.envrc` is provided)

Install system dependencies automatically:

```bash
./scripts/install-native-build-deps.sh --yes
```

Supports pacman, apt, dnf, and zypper.

### Clone and Build

```bash
git clone https://github.com/yandy-r/crosshook.git
cd crosshook
./scripts/install-native-build-deps.sh --yes
./scripts/build-native.sh
```

By default, build outputs go to XDG locations (`./scripts/build-native.sh --print-paths`). CI uses `./dist`. Full AppImage builds run `./scripts/generate-assets.sh` first (needs `rsvg-convert` and ImageMagick); `install-native-build-deps.sh` installs those where available. For just the release binary (faster, skips icon regeneration and AppImage):

```bash
./scripts/build-native.sh --binary-only
```

### Development Mode

```bash
./scripts/dev-native.sh
```

Starts the Tauri dev server with hot-reload for both the React frontend and Rust backend. Handles
Wayland/X11 detection automatically.

If git hooks are not installed, the first `./scripts/dev-native.sh` run prints a one-line reminder.
Set `CROSSHOOK_SKIP_HOOK_CHECK=1` to silence it.

### Git hooks (recommended)

CI runs the same checks as [`lint.yml`](.github/workflows/lint.yml). Install **Lefthook** so
formatting and lint run on **staged files** before each commit:

```bash
./scripts/setup-dev-hooks.sh
```

The script installs [Lefthook](https://lefthook.dev/install/) if missing (e.g. via `go install`, `npm install -g`, or `pipx`). It is **not** published on crates.io — do not use `cargo install lefthook`.

Verify hooks are installed: `./scripts/setup-dev-hooks.sh --check` (exits non-zero with install
instructions if missing).

### Running Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

There is no frontend test framework configured. UI changes are verified by running the dev app or
building the AppImage.

## Project Architecture

All source lives under `src/crosshook-native/`:

| Directory                | Role                                                                                |
| ------------------------ | ----------------------------------------------------------------------------------- |
| `crates/crosshook-core/` | All business logic -- profiles, launch, steam, install, community, export, metadata |
| `crates/crosshook-cli/`  | Thin CLI binary wrapping core                                                       |
| `src-tauri/`             | Tauri IPC layer -- `#[tauri::command]` handlers, thin glue                          |
| `src/`                   | React/TypeScript frontend -- components, hooks, contexts, styles                    |
| `runtime-helpers/`       | Shell scripts bundled into the AppImage, run under Proton at launch time            |
| `scripts/`               | Dev, build, and release scripts                                                     |

**Key rule**: business logic belongs in `crosshook-core`. The CLI and Tauri crates must stay thin
(IPC and argument parsing only).

## Code Style

### Rust

- **Naming**: `snake_case` for functions, variables, modules
- **Modules**: directories with `mod.rs`
- **Errors**: `Result` with `anyhow` or project-specific error types
- **Formatting**: `cargo fmt` (standard defaults -- no `rustfmt.toml`)
- **Linting**: `cargo clippy` (standard defaults -- no `clippy.toml`)

### TypeScript / React

- **Components**: `PascalCase` (e.g., `ProfileEditor.tsx`)
- **Hooks and functions**: `camelCase` (e.g., `useProfile.ts`)
- **Strict mode**: TypeScript `strict: true` is enforced
- **Tauri IPC**: wrap `invoke()` calls in custom hooks for stateful UI
- **CSS variables**: defined in `src/crosshook-native/src/styles/variables.css`
- **Class names**: BEM-like `crosshook-*` convention

### Formatting and lint

From the repository root:

```bash
./scripts/format.sh          # rustfmt, Biome, Prettier (Markdown/JSON)
./scripts/lint.sh            # same stack as CI: rustfmt check, clippy, biome, tsc, shellcheck
./scripts/lint.sh --fix      # apply auto-fixes where supported (e.g. clippy, biome)
```

Prettier is configured via `.prettierrc` (2-space indent, single quotes, semicolons, 120-char
width). You can also use your editor’s format-on-save; the scripts above match CI.

Pull requests from this repository get an optional **Lint autofix** workflow
([`lint-autofix.yml`](.github/workflows/lint-autofix.yml)) that applies `./scripts/format.sh` and
pushes formatting-only commits. Fork PRs cannot receive automatic pushes; run the scripts locally
instead.

## Commit Conventions

CrossHook uses [Conventional Commits](https://www.conventionalcommits.org/). The changelog is
generated by [git-cliff](https://git-cliff.org/) from commit messages, so **write your commit title
as you want it to appear in the changelog**.

### Format

```
type(scope): concise description
```

### Common types

| Type       | When to use                                |
| ---------- | ------------------------------------------ |
| `feat`     | New user-facing feature                    |
| `fix`      | Bug fix                                    |
| `docs`     | Documentation changes                      |
| `build`    | Build system or dependency changes         |
| `refactor` | Code restructuring with no behavior change |
| `perf`     | Performance improvement                    |
| `test`     | Adding or updating tests                   |
| `ci`       | CI/CD changes                              |

### Internal docs

Changes under `docs/plans/`, `docs/research/`, or `docs/internal/` must use the
`docs(internal): ...` prefix so they are excluded from release notes.

### Avoid

- Vague messages: `fix stuff`, `Update README.md`, `misc changes`
- Non-conventional prefixes that git-cliff cannot categorize

## Pull Requests

PRs auto-populate from the [pull request template](.github/pull_request_template.md). When
submitting:

1. **Link the issue** -- every PR should reference an issue (`Closes #...`)
2. **Label the PR** -- use the [label taxonomy](#labels) below; do not invent ad-hoc labels
3. **Pass the build checklist**:
   - `./scripts/build-native.sh --binary-only` builds without errors
   - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes
4. **Area-specific checks** -- the PR template includes conditional items for changes touching
   launch, steam, profiles, UI components, and runtime helpers

Keep PRs focused. If your change spans multiple unrelated areas, consider splitting it.

## Labels

Use only labels from this taxonomy -- do not create new ones.

| Family      | Values                                                        |
| ----------- | ------------------------------------------------------------- |
| `type:`     | bug, feature, docs, refactor, compatibility, build, migration |
| `area:`     | injection, memory, process, ui, build, profiles, cli          |
| `platform:` | steam-deck, linux, macos, wine, proton                        |
| `priority:` | critical, high, medium, low                                   |
| `status:`   | needs-triage, in-progress, blocked, needs-info                |
| Standalone  | `good first issue`, `help wanted`, `duplicate`, `wontfix`     |

## Security

- **Never** commit `.env`, `.env.encrypted`, or `.env.keys`
- Report vulnerabilities privately via
  [GitHub Security Advisories](https://github.com/yandy-r/crosshook/security/advisories/new)

## License

CrossHook is licensed under the [MIT License](LICENSE). By contributing, you agree that your
contributions will be licensed under the same terms.
