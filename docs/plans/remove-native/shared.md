# Remove Native AppImage Distribution

CrossHook's current release path still treats AppImage as the primary Tauri bundle while Flatpak consumes a binary staged by the AppImage-oriented release job. Removing AppImage support should keep the production Tauri binary build required by Flatpak, preserve Flatpak host-tool/runtime behavior, and delete or rename AppImage-only build, release, desktop-launcher, and documentation surfaces. The main implementation boundary is packaging and release orchestration: CI must publish only the Flatpak bundle, local scripts must build and stage Flatpak inputs without AppImage bundling, and runtime cleanup must avoid deleting Flatpak migration and host-gateway code that still protects existing users and sandbox launches.

## Relevant Files

- .github/workflows/release.yml: Tag release graph currently builds AppImage, CLI tarball, and Flatpak.
- .github/pull_request_template.md: Contributor checklist still requires native binary and AppImage packaging checks.
- package.json: Root scripts expose `build:binary`, `build:appimage`, and Flatpak wrappers.
- README.md: Public install, build, CI, and release guidance is AppImage-first.
- CONTRIBUTING.md: Contributor build and verification guidance still references AppImage output.
- CLAUDE.md: Source-of-truth agent rules still describe AppImage distribution and native build commands.
- AGENTS.md: Agent-runtime copy preserves AppImage platform, command, and migration wording.
- .ai/rules/project.md: Vendor-neutral agent rules mirror AppImage distribution guidance.
- .cursor/rules/project.mdc: Cursor rules mirror AppImage build and packaging guidance.
- .cursorrules: Legacy Cursor rules mirror AppImage build and packaging guidance.
- scripts/build-native.sh: Mixed binary-build and AppImage-bundling helper; Flatpak needs only the `--no-bundle` binary path.
- scripts/build-flatpak.sh: Flatpak bundle helper currently calls `build-native.sh --binary-only` and stages the release binary.
- scripts/build-native-container.sh: Containerized AppImage build wrapper.
- scripts/build-native-container.Dockerfile: AppImage-focused native builder image.
- scripts/install-native-build-deps.sh: Native/AppImage host dependency installer.
- scripts/generate-crosshook-desktop.sh: AppImage desktop launcher generator.
- scripts/lib/build-paths.sh: Shared output path helper with AppImage bundle discovery helpers.
- scripts/generate-assets.sh: Branding asset generator still needed by Flatpak icons.
- scripts/lib/sync-tauri-icons.sh: Tauri AppImage icon sync helper, likely removable with AppImage bundling.
- scripts/lint.sh: Shell and host-gateway validation entrypoint that must track script deletions.
- src/crosshook-native/src-tauri/tauri.conf.json: Tauri config declares `bundle.targets: ["appimage"]`.
- src/crosshook-native/src-tauri/src/lib.rs: Startup contains AppImage-specific WebKit re-exec and comments plus Flatpak migration.
- src/crosshook-native/src-tauri/src/paths.rs: Runtime-helper resolution must continue to support Flatpak `/app/resources`.
- src/crosshook-native/crates/crosshook-core/src/platform/mod.rs: Flatpak detection and host gateway remain required.
- src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs: Legacy AppImage data import should remain or be explicitly retired by policy.
- src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs: Regression coverage for Flatpak first-run import.
- packaging/flatpak/dev.crosshook.CrossHook.yml: Flatpak manifest installs staged binary, helpers, desktop file, metainfo, and icons.
- packaging/flatpak/README.md: Packaging guide still says Flatpak ships alongside AppImage and CLI assets.
- docs/internal-docs/local-build-publish.md: Internal build and release guide is AppImage-centered.
- docs/getting-started/quickstart.md: User setup guide teaches AppImage download/run on desktop and Steam Deck.
- docs/internal-docs/steam-deck-validation-checklist.md: Validation examples run the AppImage directly.
- docs/architecture/adr-0001-platform-host-gateway.md: Host-tool gateway contract must stay intact for Flatpak.
- docs/architecture/adr-0002-flatpak-portal-contracts.md: Flatpak portal behavior must stay intact.
- docs/architecture/adr-0003-proton-download-manager.md: Proton Manager ADR describes AppImage plus Flatpak parity.
- docs/architecture/adr-0004-flatpak-per-app-isolation.md: Flatpak isolation ADR describes one-way import from AppImage-era data.
- docs/prps/prds/flatpak-distribution.prd.md: Historical Flatpak plan treats AppImage as primary and must be reconciled.

## Relevant Patterns

**Production Binary Via Tauri No-Bundle**: Flatpak still needs `tauri build --no-bundle` so the production frontend is embedded; do not replace it with plain `cargo build --release`. See [scripts/build-native.sh](scripts/build-native.sh).

**Flatpak Staging Directory**: Flatpak packaging stages the binary, runtime helpers, icons, desktop file, metainfo, and manifest into a temporary directory before running `flatpak-builder`. See [scripts/build-flatpak.sh](scripts/build-flatpak.sh).

**Release Artifact Handoff**: GitHub Actions currently passes staged Flatpak inputs as artifacts between jobs; keep a clear artifact handoff if the binary build and Flatpak build remain separate. See [.github/workflows/release.yml](.github/workflows/release.yml).

**Fail-Fast Shell Scripts**: Build scripts use `set -euo pipefail`, local `die`/`log` helpers, explicit preflight checks, and documented env vars. See [scripts/build-flatpak.sh](scripts/build-flatpak.sh) and [scripts/prepare-release.sh](scripts/prepare-release.sh).

**Strict Optional Metadata Validation**: Flatpak desktop and AppStream validation can warn locally but fail with `--strict` or CI enforcement. See [scripts/build-flatpak.sh](scripts/build-flatpak.sh).

**Host-Tool Gateway Preservation**: Removing AppImage must not remove Flatpak host command routing or `is_flatpak()` branches needed for sandbox behavior. See [src/crosshook-native/crates/crosshook-core/src/platform/mod.rs](src/crosshook-native/crates/crosshook-core/src/platform/mod.rs).

**Legacy Migration Compatibility**: AppImage release removal is separate from preserving existing users' host XDG data import into the Flatpak sandbox. See [docs/architecture/adr-0004-flatpak-per-app-isolation.md](docs/architecture/adr-0004-flatpak-per-app-isolation.md).

## Relevant Docs

**README.md**: You _must_ read this when updating public installation, local build, release artifact, and CI wording.

**CONTRIBUTING.md**: You _must_ read this when updating contributor verification and build instructions.

**packaging/flatpak/README.md**: You _must_ read this when changing Flatpak build scripts, release assets, runtime version guidance, or migration wording.

**docs/internal-docs/local-build-publish.md**: You _must_ read this when rewriting local build and release publishing guidance.

**.github/workflows/release.yml**: You _must_ read this when removing AppImage assets from CI and publishing only Flatpak artifacts.

**docs/architecture/adr-0001-platform-host-gateway.md**: You _must_ read this when touching platform, host-command, or Flatpak launch behavior.

**docs/architecture/adr-0002-flatpak-portal-contracts.md**: You _must_ read this when touching Flatpak runtime and portal behavior.

**docs/architecture/adr-0004-flatpak-per-app-isolation.md**: You _must_ read this when deciding how legacy AppImage user data import is documented or retained.

**docs/prps/prds/flatpak-distribution.prd.md**: You _must_ read this when reconciling the old Flatpak-secondary plan with Flatpak-only distribution.

**src/crosshook-native/src/lib/mocks/README.md**: You _must_ read this when relocating or renaming the production mock-code sentinel in release CI.

## Storage Boundary

No new persisted data is required. TOML settings are unchanged, SQLite metadata schema is unchanged, and runtime-only state remains limited to build outputs, Flatpak staging directories, launch/session behavior, and portal state. The only storage-sensitive decision is backward compatibility: keep the existing one-way import from legacy host AppImage data into Flatpak sandbox storage unless a deliberate product decision removes that migration path.
