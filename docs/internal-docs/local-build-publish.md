# Local Build And Publish

This document captures the local build and publish workflow for CrossHook Native (Rust + Tauri v2 + React/TypeScript) and how it feeds the GitHub Releases packaging flow.

## Prerequisites

- Rust toolchain (stable)
- Node.js 20+ and npm
- Linux build dependencies: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`

## Development

Start the Tauri dev server with hot-reload:

```bash
./scripts/dev-native.sh
```

The script applies the `WEBKIT_DISABLE_DMABUF_RENDERER=1` workaround automatically. If the Wayland launch fails, it falls back to X11.

## Build AppImage

Build the full AppImage to `dist/`:

```bash
./scripts/build-native.sh
```

Requires `cargo`, `npm`, and `patchelf`.

Options:

| Flag | Description |
| ---- | ----------- |
| `--binary-only` | Build the release binary without AppImage bundling |
| `--install-deps` | Install missing host build dependencies first |
| `--yes` | Non-interactive dependency installation |

## Container Build

Build inside a container for CI-like reproducibility:

```bash
./scripts/build-native-container.sh
```

Uses the `rust:1-bookworm` image by default and bootstraps a current Rust toolchain inside the container if the image does not provide one. Supports Docker and Podman.

Options:

| Flag | Description |
| ---- | ----------- |
| `--runtime docker\|podman` | Choose container runtime |
| `--image IMAGE` | Override the container image |
| `--install-node-modules` | Force `npm ci` inside the container |

## CI/CD

`.github/workflows/release.yml` runs on `v*` tag push:

1. Sets up Node.js 20 and Rust stable
2. Installs Linux build prerequisites
3. Runs `cargo test -p crosshook-core`
4. Builds AppImage via `./scripts/build-native.sh`
5. Uploads AppImage to GitHub Release

## Artifact Shape

Output: `dist/CrossHook_<version>_amd64.AppImage`

- Self-contained Linux binary; no runtime dependencies needed for end users.
- User state stored in `~/.config/crosshook/` (profiles, settings).

## Prepare A Release

Use the repo-local release prep script to generate `CHANGELOG.md`, commit it, create the annotated release tag, and optionally push in the correct order.

Prerequisites:

- `git-cliff` installed locally, for example with `cargo install git-cliff --locked`
- A clean git worktree

Examples:

```bash
./scripts/prepare-release.sh --version 0.2.0
./scripts/prepare-release.sh --tag v0.2.0 --push
```

The script sequence is:

1. Regenerate `CHANGELOG.md` from git history using `.git-cliff.toml`
2. Commit the changelog update as `chore(release): prepare vX.Y.Z`
3. Create the annotated tag `vX.Y.Z`
4. If `--push` is used, push the branch first and the tag second

That keeps the tag-triggered GitHub Release workflow pointed at the commit that already contains the matching changelog update.
