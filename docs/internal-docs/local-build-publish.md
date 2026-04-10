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

The script applies the `WEBKIT_DISABLE_DMABUF_RENDERER=1` workaround automatically. If the Wayland launch fails, it falls back to X11. The release binary also undoes the linuxdeploy GTK plugin's forced `GDK_BACKEND=x11` on Wayland sessions to avoid blank-screen EGL failures on Intel+NVIDIA hybrid GPU systems.

## Build AppImage

Build the full AppImage (default output uses XDG paths, not the repo root):

- **Artifacts** (binary/AppImage copies): `$XDG_DATA_HOME/crosshook/artifacts` (fallback `~/.local/share/crosshook/artifacts`)
- **Cargo build tree**: `$XDG_CACHE_HOME/crosshook/build/cargo-target` (fallback `~/.cache/crosshook/build/cargo-target`)

CI and release builds still use `DIST_DIR=$GITHUB_WORKSPACE/dist` explicitly.

```bash
./scripts/build-native.sh
```

Requires `cargo`, `npm`, and `patchelf`.

Override locations when needed:

| Goal | Example |
| ---- | ------- |
| Legacy repo-local `dist/` | `DIST_DIR="$PWD/dist" ./scripts/build-native.sh` |
| Legacy workspace `target/` | `CARGO_TARGET_DIR="$PWD/src/crosshook-native/target" ./scripts/build-native.sh` |
| Ephemeral (`/tmp`) | `CROSSHOOK_BUILD_EPHEMERAL=1 ./scripts/build-native.sh` |
| Inspect resolved paths | `./scripts/build-native.sh --print-paths` |

The build writes two AppImage files into `DIST_DIR`:

- the versioned Tauri output, for example `CrossHook_0.2.0_amd64.AppImage`
- a stable alias, for example `CrossHook_amd64.AppImage`

The stable alias is intended for launchers and Steam shortcuts that should keep a fixed path across upgrades.

Generate a desktop launcher for local testing:

```bash
./scripts/generate-crosshook-desktop.sh
```

By default it writes `~/.local/share/applications/crosshook.desktop` and points to the stable alias in `DIST_DIR`. Use `--appimage` and `--output` to customize per machine.

Options:

| Flag             | Description                                        |
| ---------------- | -------------------------------------------------- |
| `--binary-only`  | Build the release binary without AppImage bundling |
| `--install-deps` | Install missing host build dependencies first      |
| `--yes`          | Non-interactive dependency installation            |
| `--print-paths`  | Print `DIST_DIR` and `CARGO_TARGET_DIR` and exit   |

## Container Build

Build inside a container for CI-like reproducibility:

```bash
./scripts/build-native-container.sh
```

Uses a managed cached builder image derived from `scripts/build-native-container.Dockerfile`. The script rebuilds that image only when the Dockerfile changes, then reuses it on subsequent runs. Supports Docker and Podman.

Options:

| Flag                        | Description                                                                          |
| --------------------------- | ------------------------------------------------------------------------------------ |
| `--runtime docker\|podman`  | Choose container runtime                                                             |
| `--image IMAGE`             | Use IMAGE directly instead of the managed cached builder image                       |
| `--base-image IMAGE`        | Base image used when building the managed cached builder image                       |
| `--rebuild-image`           | Force rebuilding the managed cached builder image                                    |
| `--install-node-modules`    | Force `npm ci` inside the container                                                  |
| `--keep-worktree-artifacts` | Keep `src/crosshook-native` build artifacts instead of cleaning them after the build |

Container builds now use host-resolved `DIST_DIR` and `CARGO_TARGET_DIR` (XDG defaults unless overridden), mounted into the builder container.

## CI/CD

`.github/workflows/release.yml` runs on `v*` tag push:

1. Sets up Node.js 20 and Rust stable
2. Installs Linux build prerequisites
3. Runs `cargo test -p crosshook-core`
4. Builds AppImage via `./scripts/build-native.sh`
5. Extracts the matching tagged section from `CHANGELOG.md`
6. Uploads the AppImage and publishes that changelog section as the GitHub Release body

## Artifact Shape

**GitHub Actions / tagged releases** write AppImages to `dist/` at the workspace root (`DIST_DIR` is set in `.github/workflows/release.yml`).

**Local default** (`./scripts/build-native.sh` without `DIST_DIR`): the same filenames appear under `$XDG_DATA_HOME/crosshook/artifacts`.

Output filenames:

- `CrossHook_<version>_amd64.AppImage`
- `CrossHook_amd64.AppImage` (stable alias)

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

1. Update the native workspace version to `X.Y.Z`
2. Regenerate `CHANGELOG.md` from git history using `.git-cliff.toml`
3. Validate the tagged release-notes section with `./scripts/validate-release-notes.sh`
4. Commit the release metadata update as `chore(release): prepare vX.Y.Z`
5. Create the annotated tag `vX.Y.Z`
6. If `--push` is used, push the branch first and the tag second

That keeps the tag-triggered GitHub Release workflow pointed at the commit that already contains the matching native app version and changelog update.

For release publishing, `CHANGELOG.md` is the single source of truth. The workflow uses `scripts/render-release-notes.sh` to extract the tagged section and publish it as the release notes body.
