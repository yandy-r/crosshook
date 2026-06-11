# Architecture Research: remove-native

## System Overview

CrossHook is a Linux desktop app built as a Tauri v2 shell (`src/crosshook-native/src-tauri/`), React/Vite frontend (`src/crosshook-native/src/`), and Rust workspace centered on `crosshook-core` (`src/crosshook-native/crates/crosshook-core/`). The current distribution architecture still treats the Tauri/AppImage path as the primary build: `src/crosshook-native/src-tauri/tauri.conf.json` enables `bundle.targets: ["appimage"]`, `.github/workflows/release.yml` has a `build-native` job that creates an AppImage and CLI tarball, and the Flatpak job consumes a staged binary produced by that native job.

Flatpak packaging is already present under `packaging/flatpak/` and is implemented as a pre-built-binary bundle path. The implementation plan should keep the Tauri binary build needed by Flatpak, but remove AppImage bundling, AppImage release assets, AppImage local launcher helpers, and AppImage-specific runtime workarounds. Be careful with naming: many files use "native" to mean the Tauri app source root or binary crate, not necessarily AppImage distribution.

## Relevant Components

- `.github/workflows/release.yml`: Tag-triggered release pipeline. Current graph is `build-native` -> `build-flatpak` -> `publish-release`; `build-native` builds AppImage, extracts/stages the binary for Flatpak, packages CLI tarball, runs production mock sentinel, and uploads `native-release-assets`.
- `scripts/build-native.sh`: Main Tauri build helper. It has two modes: `--binary-only` uses `tauri build --no-bundle` and copies `crosshook-native`; default mode generates assets, syncs the Tauri icon, runs full Tauri bundling, finds `.AppImage`, and writes versioned plus stable AppImage aliases.
- `scripts/build-flatpak.sh`: Local Flatpak bundle helper. It stages `DIST_DIR/crosshook-native`, runtime helpers, icons, desktop file, metainfo, and manifest, then runs `flatpak-builder` and `flatpak build-bundle`. It currently auto-runs `scripts/build-native.sh --binary-only` when the release binary is missing.
- `scripts/build-native-container.sh` and `scripts/build-native-container.Dockerfile`: AppImage-focused container build path. The script runs default `build-native.sh`; the Dockerfile installs linux/Tauri/AppImage toolchain prerequisites.
- `scripts/install-native-build-deps.sh`: Installs host packages for the native Tauri/AppImage target, including AppImage-specific tooling such as `patchelf`, `librsvg`, and image tooling.
- `scripts/lib/build-paths.sh`: Shared output path resolver. It is not AppImage-only, but includes `crosshook_appimage_bundle_dirs()` and AppImage artifact naming assumptions used by `build-native.sh` and desktop launcher generation.
- `scripts/generate-crosshook-desktop.sh`: Local AppImage desktop-file generator. It resolves `CrossHook_<arch>.AppImage`, extracts an embedded AppImage icon, and writes `Exec=` to the AppImage path.
- `scripts/generate-assets.sh` and `scripts/lib/sync-tauri-icons.sh`: Shared branding pipeline. `generate-assets.sh` is still needed for Flatpak icons; `sync-tauri-icons.sh` exists for Tauri bundle icon sync and may become unnecessary if Tauri bundling is removed.
- `src/crosshook-native/src-tauri/tauri.conf.json`: Tauri build config. `bundle.active`, `bundle.targets: ["appimage"]`, `bundle.resources`, and `bundle.icon` drive AppImage bundling/resource placement.
- `src/crosshook-native/src-tauri/src/lib.rs`: Runtime startup. Contains Flatpak first-run migration plus AppImage-only WebKitGTK re-exec and AppImage DMA-BUF comments/handling. The generic `WEBKIT_DISABLE_DMABUF_RENDERER` fallback may still be useful, but the `APPIMAGE`/`__CROSSHOOK_SYS_WEBKIT` branch is AppImage-specific.
- `src/crosshook-native/src-tauri/src/paths.rs`: Runtime-helper resolution. Uses Tauri `BaseDirectory::Resource`, Flatpak `/app/resources` fallback, then development path. If Tauri resource bundling is disabled for AppImage removal, keep the Flatpak fallback and dev path behavior.
- `src/crosshook-native/crates/crosshook-core/src/platform/`: Flatpak/native boundary abstraction. `is_flatpak()`, host command gateway, XDG override, and path normalization support both non-Flatpak and Flatpak runs. This should not be deleted wholesale; it is central to Flatpak correctness.
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/`: Imports existing AppImage host data into the Flatpak sandbox on first launch. Removal requires a product decision: keep for migration from historical AppImage users, or retire after a documented cutoff.
- `packaging/flatpak/dev.crosshook.CrossHook.yml`: Flatpak manifest. Installs staged binary to `/app/bin/crosshook-native`, runtime helpers to `/app/resources/`, desktop/metainfo files, and hicolor icons.
- `packaging/flatpak/README.md`: Flatpak packaging notes. Currently says the release publish job attaches Flatpak alongside AppImage and CLI assets and documents AppImage-to-Flatpak shared/migration behavior.
- `README.md`, `CONTRIBUTING.md`, `docs/internal-docs/local-build-publish.md`, `docs/internal-docs/steam-deck-validation-checklist.md`: User/developer docs with AppImage download, build, local launcher, release asset shape, and validation instructions.
- `packaging/PKGBUILD`: Arch-style package recipe for the Tauri binary and runtime helpers. It is not AppImage, but it is a non-Flatpak distribution path and should be included in scope decisions if Flatpak is the only supported distribution path.
- `package.json`: Root convenience scripts expose `build:binary`, `build:appimage`, and Flatpak scripts. `build:binary` may remain as an internal Flatpak prerequisite if renamed; `build:appimage` should go.

## Data Flow

1. Release starts on `v*` tag push in `.github/workflows/release.yml`.
2. `build-native` installs Node/Rust/Linux prerequisites, validates Cargo manifest versions, runs `cargo test -p crosshook-core`, then runs `./scripts/build-native.sh`.
3. Default `build-native.sh` runs frontend build through Tauri, produces the release binary, performs AppImage bundling, copies `CrossHook_<version>_amd64.AppImage` into `dist`, and leaves frontend assets in `src/crosshook-native/dist/`.
4. The workflow copies `${APPIMAGE_CARGO_TARGET_DIR}/x86_64-unknown-linux-gnu/release/crosshook-native` into `dist/crosshook-native` for Flatpak staging. This means Flatpak currently depends on the AppImage build job output even though it only needs the binary.
5. The same job runs the production mock sentinel against `src/crosshook-native/dist/assets/*.js`; this verification is logically distribution-agnostic and should move to the Flatpak/binary build path rather than disappear.
6. `build-native` also builds `crosshook-cli`, packages `dist/crosshook_<version>_linux_amd64.tar.gz`, uploads AppImage/CLI as `native-release-assets`, and stages Flatpak inputs under `dist/flatpak-staging`.
7. `build-flatpak` downloads `flatpak-staging-assets`, validates staged desktop/metainfo, runs `flatpak-builder` inside `ghcr.io/flathub-infra/flatpak-github-actions:gnome-50`, and uploads `CrossHook_<version>_amd64.flatpak`.
8. `publish-release` downloads both `native-release-assets` and `flatpak-release-asset`, renders release notes from `CHANGELOG.md`, validates them, and uploads AppImage, CLI tarball, and Flatpak files to GitHub Releases.
9. Local Flatpak builds take a similar but simpler path: `scripts/build-flatpak.sh --rebuild` calls `scripts/build-native.sh --binary-only`, stages files into a temp directory, and runs `flatpak-builder` plus `flatpak build-bundle`.

The desired Flatpak-only flow should collapse this so CI builds/tests the Tauri release binary once, runs the production bundle sentinel, stages that binary plus Flatpak assets, builds the Flatpak bundle, and publishes only the `.flatpak` unless the CLI tarball is explicitly retained as a non-distribution developer artifact.

## Integration Points

- Release workflow rewrite: rename or replace `build-native` with a binary/staging job that runs `scripts/build-native.sh --binary-only` or a renamed equivalent, removes AppImage output expectations, removes `native-release-assets`, and makes `publish-release` depend on/download only Flatpak assets.
- Tauri config cleanup: remove AppImage target configuration from `src/crosshook-native/src-tauri/tauri.conf.json`; preserve `frontendDist`, app identifier, security config, and any resource handling needed for dev or Flatpak.
- Build script split/rename: convert `scripts/build-native.sh` into an internal binary build helper or create a clearer `scripts/build-binary.sh`; delete default AppImage bundling logic, AppImage naming helpers, AppImage output discovery, and `APPIMAGE_EXTRACT_AND_RUN`.
- Flatpak script update: point `scripts/build-flatpak.sh` at the renamed binary helper, update help text/comments that call this a "native release binary" path, and keep staging contract aligned with `packaging/flatpak/dev.crosshook.CrossHook.yml`.
- Container build removal: delete `scripts/build-native-container.sh` and `scripts/build-native-container.Dockerfile` unless a generic Flatpak/binary builder is still wanted; current wording and command path are AppImage-specific.
- Dependency installer cleanup: either delete `scripts/install-native-build-deps.sh` or rename/scope it to binary build prerequisites. Do not remove Flatpak dependency installation from `scripts/build-flatpak.sh --install-deps`.
- Desktop launcher removal: remove `scripts/generate-crosshook-desktop.sh` because it is AppImage-specific and overlaps with the committed Flatpak desktop entry.
- Runtime startup cleanup: remove the `APPIMAGE` WebKitGTK re-exec branch from `src/crosshook-native/src-tauri/src/lib.rs`; revisit comments that say the release binary or AppImage needs the DMA-BUF workaround. Flatpak already sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` in the manifest.
- Platform wording cleanup: update comments in `platform/mod.rs`, `platform/gateway.rs`, `platform/detect.rs`, `platform/xdg.rs`, and ADR/docs from "AppImage and Flatpak" to "non-Flatpak dev/binary and Flatpak" where the abstraction remains valid.
- Migration policy decision: `flatpak_migration` and ADR-0004 intentionally import AppImage-era data. If AppImage support is removed, this code may still be valuable for existing users moving to Flatpak, but docs should describe it as legacy AppImage import rather than ongoing dual-distribution parity.
- Docs update: replace AppImage download/build/release instructions in `README.md`, `CONTRIBUTING.md`, `docs/internal-docs/local-build-publish.md`, `packaging/flatpak/README.md`, `AGENTS.md`, `.cursor/rules/project.mdc`, `.ai/rules/project.md`, and `.cursorrules` with Flatpak-only build/install/release guidance.
- Root scripts update: remove `build:appimage` from `package.json`; decide whether `build:binary` remains internal or is renamed to avoid user-facing native distribution messaging.
- Ignore/artifact cleanup: `.gitignore` has `*.AppImage` and native dist target ignores that may become obsolete; keep generated `src/crosshook-native/dist/` ignores for frontend build output.
- Release notes tooling: `scripts/prepare-release.sh`, `scripts/render-release-notes.sh`, and `scripts/validate-release-notes.sh` are distribution-agnostic and should remain unless docs mention AppImage output.

## Key Dependencies

- Tauri CLI and Rust workspace: `scripts/build-native.sh --binary-only` relies on `tauri build --no-bundle` so `generate_context!()` embeds the production frontend. A plain `cargo build --release` is not equivalent because it can fall back to `devUrl` at runtime.
- Node/Vite frontend build: Tauri `beforeBuildCommand` runs `npm run build` from `src/crosshook-native/package.json`; the production mock sentinel depends on generated JS assets under `src/crosshook-native/dist/assets/`.
- Flatpak toolchain: `flatpak-builder`, `flatpak`, GNOME Platform/Sdk `50`, `desktop-file-validate`, and `appstreamcli` are required for Flatpak packaging validation/building.
- Flatpak GitHub Action image: `.github/workflows/release.yml` uses `ghcr.io/flathub-infra/flatpak-github-actions:gnome-50`, which must stay aligned with `runtime-version: "50"` in `packaging/flatpak/dev.crosshook.CrossHook.yml` and `CROSSHOOK_FLATPAK_RUNTIME_VERSION` in `scripts/build-flatpak.sh`.
- Runtime helper scripts: `src/crosshook-native/runtime-helpers/*.sh` must still be staged into `/app/resources/` for Flatpak and resolved by `src/crosshook-native/src-tauri/src/paths.rs`.
- Host-tool gateway: Flatpak distribution depends on `crosshook-core::platform` functions and `scripts/check-host-gateway.sh` to route Proton/Wine/gamescope/mangohud/etc. through `flatpak-spawn --host`.
- Branding assets: `assets/icon-128.png`, `assets/icon-256.png`, and `assets/icon-512.png` are Flatpak packaging inputs. `scripts/generate-assets.sh` remains useful, but Tauri AppImage icon sync can be removed if no Tauri bundle is produced.
- Release version authority: `scripts/prepare-release.sh` updates Cargo manifests under `src/crosshook-native/` and regenerates `CHANGELOG.md`; this remains the release metadata path even after AppImage removal.
