# Code Analysis: Remove Native AppImage Distribution

## Executive Summary

CrossHook's AppImage surface is concentrated in release orchestration, local build scripts, Tauri bundle configuration, runtime startup workarounds, and documentation. Flatpak still depends on a production Tauri binary produced with `tauri build --no-bundle`, so implementation should rename and preserve the release-binary path while deleting AppImage bundling, AppImage asset naming, and AppImage-only launcher/container helpers. Do not remove Flatpak host-gateway, `/app/resources` helper resolution, portal behavior, or one-way legacy data import; those are Flatpak compatibility paths, not AppImage distribution code.

## Existing Code Structure

- `.github/workflows/release.yml`: tag-triggered release workflow with `build-native`, `build-flatpak`, and `publish-release`; `build-native` currently runs tests, builds AppImage, exports the Flatpak binary, packages CLI tarball, uploads native assets, and stages Flatpak inputs.
- `scripts/build-native.sh`: mixed-purpose script; `--binary-only` is the required Flatpak-compatible production binary path, while the default branch performs AppImage icon sync, `tauri build`, bundle discovery, and stable AppImage alias copying.
- `scripts/build-flatpak.sh`: Flatpak bundle builder; stages `crosshook-native`, runtime helpers, icons, desktop/metainfo files, and manifest before running `flatpak-builder` and `flatpak build-bundle`.
- `scripts/lib/build-paths.sh`: shared output resolver for `DIST_DIR`, `CARGO_TARGET_DIR`, `CROSSHOOK_DATA_HOME`, and `CROSSHOOK_CACHE_HOME`; last function `crosshook_appimage_bundle_dirs()` is AppImage-only.
- `scripts/build-native-container.sh` and `scripts/build-native-container.Dockerfile`: containerized AppImage build path that shells back to `./scripts/build-native.sh` without `--binary-only`.
- `scripts/generate-crosshook-desktop.sh`: AppImage desktop-entry generator that discovers `CrossHook_<arch>.AppImage`, extracts AppImage icons, and writes an `Exec=` path to the AppImage.
- `src/crosshook-native/src-tauri/tauri.conf.json`: Tauri config has `bundle.active: true`, `bundle.targets: ["appimage"]`, `resources`, and `icon`; no-bundle builds still use `build.frontendDist`.
- `src/crosshook-native/src-tauri/src/lib.rs`: startup contains Flatpak migration first, then legacy Tauri app-id migration, then AppImage-specific WebKit re-exec and linuxdeploy comments, plus general WebKit/GDK env workarounds.
- `src/crosshook-native/src-tauri/src/paths.rs`: runtime-helper resolver tries Tauri resources first, then Flatpak `/app/resources`, then development scripts; the Flatpak branch must remain.
- `package.json`: root script names expose `build:binary`, `build:appimage`, `flatpak:build`, `flatpak:install`, and `flatpak:update`.
- `packaging/flatpak/dev.crosshook.CrossHook.yml`: Flatpak manifest expects staged pre-built binary and three runtime helper scripts; this remains the distribution target.

## Implementation Patterns (with examples)

- **Preserve no-bundle production binary builds**: `scripts/build-native.sh --binary-only` documents why `cargo tauri build --no-bundle` is required: plain `cargo build --release` does not embed `frontendDist` and falls back to `devUrl`. A replacement/renamed helper should keep this exact Tauri CLI fallback sequence: `cargo tauri`, then local `node_modules/.bin/tauri`, then `npx tauri`.
- **Keep explicit build path initialization**: build scripts source `scripts/lib/build-paths.sh`, call `crosshook_build_paths_init`, export `CARGO_TARGET_DIR`, and let caller-provided `DIST_DIR`/`CARGO_TARGET_DIR` win. Keep that pattern for any renamed release-binary script and for Flatpak packaging.
- **Stage Flatpak inputs deterministically**: both `release.yml` and `scripts/build-flatpak.sh` use `install -Dm755` / `install -Dm644` into a staging directory. Keep staging explicit for binary, `runtime-helpers/*.sh`, icons, desktop file, metainfo, and manifest.
- **Fail fast in shell**: scripts use `set -euo pipefail`, `die()`, `log()`, argument parsing with `case`, and preflight `command -v` checks. New or renamed scripts should follow that style rather than embedding logic in workflow YAML only.
- **Metadata validation has strict/local modes**: `scripts/build-flatpak.sh` treats `desktop-file-validate` and `appstreamcli validate` warnings as non-fatal unless `--strict` or `CROSSHOOK_FLATPAK_VALIDATE_STRICT` is set. CI should stay strict by directly running validators before Flatpak build.
- **Release version authority is native Cargo manifests**: `release.yml` and `scripts/prepare-release.sh` validate/update `src/crosshook-native/Cargo.toml`, `crates/crosshook-core/Cargo.toml`, `crates/crosshook-cli/Cargo.toml`, and `src-tauri/Cargo.toml`. Removing AppImage should not move version authority to root `package.json`.
- **Mock-code sentinel is distribution-agnostic**: `release.yml` scans `src/crosshook-native/dist/assets/*.js` after the production Tauri build. Rename the step away from AppImage wording, but keep the sentinel after the no-bundle binary build.
- **Flatpak resource fallback is runtime behavior**: `src-tauri/src/paths.rs` checks `BaseDirectory::Resource`, then `/app/resources` only when `platform::is_flatpak()`. Do not collapse this into development-path fallback.

## Integration Points (files to create/modify/delete)

- Modify `.github/workflows/release.yml`: rename `build-native` to a release-binary or Flatpak-staging job; remove AppImage build step, `APPIMAGE_CARGO_TARGET_DIR`, AppImage artifact upload/download, CLI tarball packaging if Flatpak-only also means no CLI release asset, and AppImage file from `softprops/action-gh-release`; keep tests, version validation, production binary build, mock sentinel, Flatpak staging artifact, `build-flatpak`, and release-note publication.
- Modify `scripts/build-native.sh` or replace it with a renamed helper: keep only the no-bundle binary behavior and `--print-paths`/dependency install as needed; remove `APPIMAGE_EXTRACT_AND_RUN`, `stable_appimage_name`, `appimage_arch_suffix`, `patchelf` requirement for default builds, icon sync for Tauri bundling, `cargo tauri build` bundling, AppImage discovery, and stable alias copy.
- Modify `scripts/build-flatpak.sh`: point `--rebuild` and missing-binary fallback at the renamed binary helper if one is created; update help text from `build-native.sh --binary-only` to the new command; keep staging and bundle build logic.
- Modify `scripts/lib/build-paths.sh`: keep path initialization; delete `crosshook_appimage_bundle_dirs()` once no script calls it.
- Modify `src/crosshook-native/src-tauri/tauri.conf.json`: remove AppImage bundle target/config if no longer bundling with Tauri; keep `build.beforeBuildCommand`, `build.frontendDist`, app window/security config, and any resource declaration only if no-bundle or dev still requires it. Confirm Flatpak manifest remains the source for installing runtime helpers.
- Modify `src/crosshook-native/src-tauri/src/lib.rs`: remove the `APPIMAGE` WebKit re-exec block and linuxdeploy-specific comment/workaround if it applies only to AppImage; keep Flatpak migration, legacy app-id migration, and general `WEBKIT_DISABLE_DMABUF_RENDERER` behavior if still needed for Tauri/WebKitGTK and Flatpak.
- Modify `package.json`: remove `build:appimage`; rename `build:binary` if the underlying script is renamed; keep `flatpak:*`, lint, format, and release scripts aligned with docs.
- Modify `.github/pull_request_template.md`: replace AppImage checklist item with Flatpak bundle/build checks and keep `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.
- Modify docs/rules: `README.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `AGENTS.md`, `.ai/rules/project.md`, `.cursor/rules/project.mdc`, `.cursorrules`, `packaging/flatpak/README.md`, `docs/internal-docs/local-build-publish.md`, `docs/getting-started/quickstart.md`, `docs/internal-docs/steam-deck-validation-checklist.md`, ADRs, and the historical Flatpak PRD need AppImage distribution wording reconciled to Flatpak-only.
- Delete candidates after references are gone: `scripts/build-native-container.sh`, `scripts/build-native-container.Dockerfile`, `scripts/generate-crosshook-desktop.sh`, and `scripts/lib/sync-tauri-icons.sh` if no remaining Tauri bundle/icon path uses it.
- No new persisted data, SQLite migrations, TOML settings, or runtime state files are required.

## Code Conventions

- Shell scripts: `bash`, `set -euo pipefail`, local `usage`, `die`, and `log` helpers; prefer arrays for package-manager commands and `install -Dm...` for staging.
- Workflow YAML: keep release jobs explicit and artifact handoffs named; use `if-no-files-found: error` and `fail_on_unmatched_files: true` for release assets.
- Rust: keep Tauri IPC/startup glue thin; Flatpak-specific behavior belongs in `crosshook-core::platform` and `crosshook_core::flatpak_migration`, not duplicated in `src-tauri`.
- Docs/rules: `CLAUDE.md` is source of truth; `AGENTS.md`, `.ai/rules/project.md`, `.cursor/rules/project.mdc`, and `.cursorrules` mirror or preserve runtime-specific notes and must be consistent.
- Validation commands should use existing repo shape: root `npm run lint` for full lint, `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` for core behavior, `npm run flatpak:build` / `scripts/build-flatpak.sh --strict` for packaging.

## Dependencies and Services

- Tauri v2 remains required for production binary generation because it embeds the React `dist` output into `crosshook-native`.
- Flatpak build requires `flatpak`, `flatpak-builder`, GNOME Platform/Sdk `50`, `desktop-file-validate`, and `appstreamcli` for strict validation.
- Native release binary build requires Rust stable, Node/npm, GTK3/WebKitGTK/libsoup/OpenSSL development packages, and frontend dependencies under `src/crosshook-native`.
- Icon assets for Flatpak still come from `assets/icon-128.png`, `assets/icon-256.png`, and `assets/icon-512.png`; `scripts/generate-assets.sh` remains relevant when those are missing or branding changes.
- GitHub release publishing still depends on `scripts/render-release-notes.sh`, `scripts/validate-release-notes.sh`, `CHANGELOG.md`, and `softprops/action-gh-release`.
- Host-tool gateway and Flatpak portal behavior depend on `crosshook_core::platform`, `flatpak-spawn --host`, and the Flatpak manifest finish args; these are not AppImage surfaces.

## Gotchas and Warnings

- Do not replace `tauri build --no-bundle` with `cargo build --release`; it will produce a binary that tries to load the dev server instead of embedded frontend assets.
- Do not delete Flatpak migration because it mentions AppImage data. ADR-0004 intentionally imports legacy host XDG data for existing users moving to Flatpak.
- Do not delete `src-tauri/src/paths.rs` Flatpak `/app/resources` fallback or runtime helper staging; Steam/trainer helper launch paths depend on it inside the sandbox.
- Removing `bundle.resources` from `tauri.conf.json` is only safe if no-bundle builds and runtime helper resolution no longer rely on Tauri resources; Flatpak itself installs helpers through the manifest.
- `patchelf`, ImageMagick, and `librsvg2-bin` may be AppImage-only for Tauri bundling, but `generate-assets.sh` may still need `rsvg-convert`/ImageMagick for Flatpak icons when assets are regenerated.
- `build-native` in release YAML is a misleading name, but not fully removable; it contains tests, version checks, binary export, mock sentinel, and Flatpak staging.
- Search terms must include both `AppImage` and generic `native` wording. Some user-facing text says "native build" when it means Tauri dev/release binary; do not rewrite all "native" occurrences blindly because CrossHook is still a native Linux app.
- Historical docs such as `docs/prps/prds/flatpak-distribution.prd.md` should be marked historical or updated carefully; they contain intentional old decisions that may be useful context even after product direction changes.

## Task-Specific Guidance

- Start by introducing or renaming the binary-build command so Flatpak has a stable producer (`crosshook-native` in `DIST_DIR`) independent of AppImage terminology.
- Then update `release.yml` around that producer: tests/version validation -> no-bundle binary -> mock sentinel -> Flatpak staging -> Flatpak bundle -> publish only `.flatpak`.
- Delete AppImage-only scripts only after `rg "build-native-container|generate-crosshook-desktop|sync-tauri-icons|crosshook_appimage_bundle_dirs|build:appimage"` has no live callers.
- Update docs in two passes: first public/user build and install paths (`README.md`, quickstart, Flatpak README, local build/publish), then agent/rule mirrors and PR template.
- Verify with at least: `rg -n "AppImage|appimage|APPIMAGE|build:appimage|build-native-container|generate-crosshook-desktop|sync-tauri-icons"`, `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`, root `npm run lint` or targeted script lint for changed shell files, and `./scripts/build-flatpak.sh --strict` when Flatpak tooling is available.
