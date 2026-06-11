# Integration Research: remove-native

## API Endpoints

- No public HTTP API endpoints are involved in the AppImage removal. The external release API surface is GitHub Releases, driven by `.github/workflows/release.yml` and release-body generation from `CHANGELOG.md` via `scripts/render-release-notes.sh`.
- Tauri IPC remains the runtime API boundary. AppImage removal should not change command names or IPC payloads, but startup behavior in `src/crosshook-native/src-tauri/src/lib.rs` has AppImage-only environment branches that can be removed while preserving Flatpak migration/event behavior.
- The Flatpak migration completion event `flatpak-migration-complete` is emitted from `src/crosshook-native/src-tauri/src/lib.rs` after `crosshook_core::flatpak_migration::run()` imports prior host data. Keep this event and payload contract.
- Runtime helper script lookup is an internal Tauri resource API boundary: `src/crosshook-native/src-tauri/src/paths.rs` first asks Tauri `BaseDirectory::Resource`, then falls back to `/app/resources` under Flatpak. This must stay valid for the Flatpak manifest's runtime-helper installs.
- Launch IPC internally branches on Flatpak in `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`: Steam trainer launches under Flatpak reuse the direct Proton trainer builder and record as `proton_run`; gamescope PID capture is also gated on `platform::is_flatpak()`. These are Flatpak behavior, not AppImage distribution behavior, and should remain.

## Database Schema

- No SQLite schema change is expected. AppImage/native distribution is not represented as a schema dimension in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/`; the current schema advances through user version 24.
- `MetadataStore::try_new()` in `src/crosshook-native/crates/crosshook-core/src/metadata/store.rs` derives `crosshook/metadata.db` from `directories::BaseDirs`. Under Flatpak, this resolves to the sandbox data root unless `CROSSHOOK_FLATPAK_HOST_XDG=1` opt-in shared mode is set before stores initialize.
- Flatpak first-run migration copies host AppImage-era data into the sandbox in `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/`. It includes `crosshook/metadata.db`, `metadata.db-wal`, and `metadata.db-shm`; includes `community`, `media`, and `launchers`; skips `prefixes`, `artifacts`, `cache`, `logs`, and `runtime-helpers`.
- Removing AppImage releases raises a product decision, not a DB migration requirement: whether to keep one-way import wording/logic for existing AppImage users. Keeping it is the safest backward-compatibility path until enough Flatpak releases have shipped.

## External Services

- GitHub Actions release workflow `.github/workflows/release.yml` is the largest integration boundary:
  - `build-native` currently builds the AppImage, runs core tests, exports `dist/crosshook-native` for Flatpak, runs the mock-code sentinel against `src/crosshook-native/dist/assets/*.js`, builds/packages the CLI tarball, uploads `native-release-assets`, and stages `flatpak-staging-assets`.
  - `build-flatpak` depends on `build-native`, downloads `flatpak-staging-assets`, validates staged desktop/metainfo, builds the `.flatpak` with `flatpak/flatpak-github-actions/flatpak-builder@v6`, and uploads `flatpak-release-asset`.
  - `publish-release` downloads `native-release-assets` and `flatpak-release-asset`, then uploads AppImage, CLI tarball, and Flatpak assets via `softprops/action-gh-release@v2`.
- Release asset shape should change from AppImage + CLI tarball + Flatpak to Flatpak-only unless the CLI tarball is intentionally retained. The user goal says Flatpak is the only distribution path, so the implementation plan should remove `native-release-assets`, AppImage upload paths, and likely CLI release upload unless explicitly rescoped.
- GitHub Releases documentation currently advertises AppImage downloads in `README.md`, `packaging/flatpak/README.md`, `docs/internal-docs/local-build-publish.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.cursor/rules/project.mdc`, and `.ai/rules/project.md`.
- Flatpak external tooling remains required: `flatpak`, `flatpak-builder`, `desktop-file-validate`, `appstreamcli`, GNOME Platform/Sdk runtime 50, and the release workflow container `ghcr.io/flathub-infra/flatpak-github-actions:gnome-50`.
- Tauri remains an external build tool, but `src/crosshook-native/src-tauri/tauri.conf.json` currently has `bundle.targets: ["appimage"]`. For Flatpak-only packaging, use Tauri to build the production binary and frontend without AppImage bundling, and remove the AppImage bundle target/config dependency.

## Internal Services

- `scripts/build-native.sh` currently has two roles that should be separated or renamed:
  - `--binary-only` builds the production Tauri binary with `tauri build --no-bundle` so `frontendDist` is embedded. Flatpak depends on this behavior.
  - default mode generates assets, syncs `src-tauri/icons/icon.png`, runs full `tauri build`, searches for `.AppImage`, and copies versioned/stable AppImage aliases to `DIST_DIR`. This default AppImage path should be removed.
- `scripts/build-flatpak.sh` is Flatpak's local packaging entrypoint. It currently stages a prebuilt `crosshook-native` binary from `DIST_DIR`, and auto-runs `./scripts/build-native.sh --binary-only` when missing or when `--rebuild` is used. After removal, this should call the renamed binary-build helper or directly build the Flatpak binary path without mentioning AppImage/native distribution.
- `scripts/build-native-container.sh` and `scripts/build-native-container.Dockerfile` exist only to build native AppImages in a container. They should be removed unless retained solely as a generic binary builder, which would need renamed docs, usage, and cleanup behavior.
- `scripts/install-native-build-deps.sh` installs host dependencies for native Tauri/AppImage builds, including `patchelf`, WebKitGTK, `rsvg-convert`, and ImageMagick. Flatpak local build prerequisites belong in `scripts/build-flatpak.sh --install-deps`; this script can likely be removed or reduced to a developer Tauri binary prerequisite helper.
- `scripts/generate-crosshook-desktop.sh` is AppImage-only: it resolves `CrossHook_<arch>.AppImage`, extracts embedded icons via `--appimage-extract`, and writes `Exec=<AppImage>`. Remove it or replace with Flatpak install/run guidance.
- `scripts/lib/build-paths.sh` is still useful for `DIST_DIR` and `CARGO_TARGET_DIR`, but `crosshook_appimage_bundle_dirs()` becomes dead once AppImage bundling is removed.
- `scripts/generate-assets.sh` is still useful because Flatpak packaging stages `assets/icon-128.png`, `icon-256.png`, and `icon-512.png`; `scripts/lib/sync-tauri-icons.sh` is AppImage/Tauri-bundle-icon specific and may become unnecessary if Tauri bundle icons are no longer used.
- `src/crosshook-native/src-tauri/src/lib.rs` contains AppImage-only startup workarounds:
  - Re-exec from AppImage to prefer system WebKitGTK when `APPIMAGE` is set.
  - AppImage-specific comments around `WEBKIT_DISABLE_DMABUF_RENDERER`.
  - linuxdeploy GTK plugin `GDK_BACKEND=x11` workaround comments.
    Remove AppImage-specific branches/comments while preserving general WebKit/Wayland dev behavior if still needed for Tauri dev or Flatpak.
- The host-tool gateway in `src/crosshook-native/crates/crosshook-core/src/platform/` remains critical. Removing AppImage should not remove native-vs-Flatpak conditionals wholesale, because `is_flatpak()` still distinguishes sandboxed Flatpak from dev/test binary execution.
- Flatpak portals in `src/crosshook-native/src-tauri/src/background_portal.rs` and `src/crosshook-native/crates/crosshook-core/src/platform/portals/` remain Flatpak-only integration services and should stay.

## Configuration and CI

- Root `package.json` exposes AppImage/native distribution commands:
  - `build:binary` -> `scripts/build-native.sh --binary-only`
  - `build:appimage` -> `scripts/build-native-container.sh`
  - Flatpak commands call `scripts/build-flatpak.sh`.
    Rename or replace `build:binary` with a Flatpak-binary/package prerequisite command; remove `build:appimage`.
- `src/crosshook-native/src-tauri/tauri.conf.json` should stop declaring AppImage as a bundle target. The resource list for runtime helpers may still be relevant for Tauri resource resolution, but Flatpak explicitly installs helper scripts into `/app/resources`.
- `.github/workflows/release.yml` should be restructured around Flatpak:
  - Keep version verification and `cargo test -p crosshook-core`.
  - Build the production Tauri binary without AppImage bundling.
  - Keep the mock-code sentinel after production frontend build, but rename it away from AppImage language and ensure it still checks `src/crosshook-native/dist/assets/*.js`.
  - Stage Flatpak assets directly in the Flatpak job or upload a clearly named binary/staging artifact from a binary-build job.
  - Publish only `CrossHook_<version>_amd64.flatpak` unless CLI retention is intentionally out of scope.
- `.github/workflows/lint.yml` does not build AppImage, but its Linux build dependencies include Tauri/WebKit dev packages and `librsvg2-dev`. It can mostly stay; shellcheck will need updates after removing scripts.
- `.github/pull_request_template.md` checklist currently asks for `./scripts/build-native.sh --binary-only` and full `./scripts/build-native.sh` AppImage output. Replace with Flatpak build/smoke expectations.
- `scripts/prepare-release.sh`, `scripts/render-release-notes.sh`, and `scripts/validate-release-notes.sh` are release-metadata tooling and can remain. Their wording says "native workspace version"; that is accurate for the Rust/Tauri workspace but may need doc clarification now that the distribution is Flatpak-only.
- `packaging/flatpak/dev.crosshook.CrossHook.yml` currently documents "Phase 1 — pre-built binary" and "Flathub-ready manifest that builds from source ... Phase 4 work." If Flatpak is the only distribution path, this manifest/script dependency on a separately built binary is now a first-class release architecture decision and should be updated deliberately.
- `packaging/flatpak/README.md` must be updated: it currently says the Flatpak bundle is attached alongside AppImage and CLI assets and includes shared-mode wording for Flatpak/AppImage coexistence.
- `README.md`, `CONTRIBUTING.md`, `docs/internal-docs/local-build-publish.md`, `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.cursor/rules/project.mdc`, `.ai/rules/project.md`, and `.github/copilot-instructions.md` contain AppImage/native build guidance that will become stale.
- `.gitignore` ignores `*.AppImage`; leaving it is harmless but it becomes dead policy if no script produces AppImages.
- `CHANGELOG.md` and completed PRP/review history contain historical AppImage references. Do not rewrite history artifacts unless the implementation plan explicitly includes stale docs cleanup beyond live docs.

## Storage Boundary

- TOML settings: no settings schema change expected. Existing stores still resolve through `directories::BaseDirs`; Flatpak isolation changes the resolved root, not the data model.
- SQLite metadata: no schema migration expected. The relevant storage behavior is first-run import of AppImage-era `metadata.db` into the Flatpak sandbox, not a table change.
- Runtime-only state: Flatpak launch/session behavior, gamescope PID capture files, background portal grants, and mock-code sentinel checks are runtime/build concerns only.
- Filesystem data:
  - Flatpak default: `~/.var/app/dev.crosshook.CrossHook/{config,cache,data}/...`.
  - Host AppImage-era import source: `~/.config/crosshook/` and `~/.local/share/crosshook/`.
  - Host wine prefixes remain at `~/.local/share/crosshook/prefixes/` via `flatpak_migration::host_prefix_root()`.
  - Local build artifacts currently default to `$XDG_DATA_HOME/crosshook/artifacts` and `$XDG_CACHE_HOME/crosshook/build/cargo-target`; remove AppImage artifact outputs but keep a clear `DIST_DIR` target for Flatpak bundles.
- Backward compatibility: keep one-way import and `CROSSHOOK_FLATPAK_HOST_XDG=1` opt-in unless the product decision is to drop AppImage-user migration support immediately. Removing release builds does not erase existing users' local AppImage data.
