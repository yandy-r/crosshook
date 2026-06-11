# Local Build And Publish

This document captures the local build and publish workflow for CrossHook Native
(Rust + Tauri v2 + React/TypeScript). Flatpak is the supported distribution
artifact; the Tauri release binary is an internal packaging input.

## Prerequisites

- Rust toolchain (stable)
- Node.js 20+ and npm
- Linux build dependencies: GTK3, WebKit2GTK 4.1, libsoup3, OpenSSL, patchelf
- Flatpak packaging tools: `flatpak`, `flatpak-builder`, `desktop-file-utils`,
  `appstreamcli`, `rsvg-convert`, and ImageMagick

Install release-binary prerequisites:

```bash
./scripts/install-native-build-deps.sh --yes
```

Install Flatpak packaging prerequisites and the GNOME runtime:

```bash
./scripts/build-flatpak.sh --install-deps
```

## Development

Start the Tauri dev server with hot-reload:

```bash
./scripts/dev-native.sh
```

For browser-only frontend work, run:

```bash
./scripts/dev-native.sh --browser
```

Browser dev mode uses local IPC mocks. Production bundle validation rejects mock
code before release assets are staged.

## Release Binary

Build the production Tauri binary used by Flatpak packaging:

```bash
./scripts/build-release-binary.sh
```

The helper runs `tauri build --no-bundle` so the production frontend is embedded
in `DIST_DIR/crosshook-native`. By default, build outputs go to XDG locations.
Use `./scripts/build-release-binary.sh --print-paths` to inspect the resolved
`DIST_DIR` and `CARGO_TARGET_DIR`.

Common overrides:

| Goal                   | Example                                                                                 |
| ---------------------- | --------------------------------------------------------------------------------------- |
| Repo-local output      | `DIST_DIR="$PWD/dist" ./scripts/build-release-binary.sh`                                |
| Workspace cargo target | `CARGO_TARGET_DIR="$PWD/src/crosshook-native/target" ./scripts/build-release-binary.sh` |
| Ephemeral output       | `CROSSHOOK_BUILD_EPHEMERAL=1 ./scripts/build-release-binary.sh`                         |
| Inspect resolved paths | `./scripts/build-release-binary.sh --print-paths`                                       |

`scripts/build-native.sh` is only a compatibility shim for the old command and
forwards to `scripts/build-release-binary.sh`.

## Flatpak Build

Build the local Flatpak bundle from a fresh release binary:

```bash
./scripts/build-flatpak.sh --rebuild --strict
```

`--strict` fails on desktop-file or AppStream metadata validation errors. Without
`--strict`, local metadata validator failures are warnings; CI treats them as
release blockers.

Flatpak icon inputs are generated from SVG sources by `./scripts/generate-assets.sh`
and staged from:

- `assets/icon-128.png`
- `assets/icon-256.png`
- `assets/icon-512.png`

The Flatpak helper stages the release binary, runtime helpers, icons, desktop
file, AppStream metadata, and manifest before running `flatpak-builder` and
`flatpak build-bundle`.

Install and run a local bundle:

```bash
./scripts/build-flatpak.sh --rebuild --strict --install
flatpak run dev.crosshook.CrossHook
```

## Validation

Run the checks relevant to your change:

```bash
./scripts/lint.sh
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
(cd src/crosshook-native && npm run typecheck)
./scripts/build-release-binary.sh
./scripts/build-flatpak.sh --rebuild --strict
```

Use the Flatpak build for packaging changes. Use the release-binary build when
you only need to verify the production Tauri binary that Flatpak consumes.

## CI/CD

`.github/workflows/release.yml` runs on `v*` tag push:

1. Sets up Node.js 20 and Rust stable
2. Installs release-binary and Flatpak packaging prerequisites
3. Runs `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
4. Builds the release binary with `./scripts/build-release-binary.sh`
5. Validates the production bundle mock-code sentinel
6. Stages Flatpak inputs and validates desktop/AppStream metadata
7. Builds and publishes the Flatpak release asset
8. Publishes release notes rendered from `CHANGELOG.md`

Tagged releases publish a single Flatpak bundle named
`CrossHook_<version>_amd64.flatpak`.

## Prepare A Release

Use the repo-local release prep script to generate `CHANGELOG.md`, commit it,
create the annotated release tag, and optionally push in the correct order.

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

For release publishing, `CHANGELOG.md` is the single source of truth. The
workflow uses `scripts/render-release-notes.sh` to extract the tagged section
and publish it as the release notes body.
