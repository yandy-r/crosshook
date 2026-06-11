# Context Analysis: remove-native

## Executive Summary

Remove AppImage as a supported distribution and release artifact while preserving the production Tauri binary build semantics that Flatpak consumes. The implementation should delete or rename AppImage-only scripts, workflow jobs, release assets, launcher helpers, docs, and runtime workarounds, but keep Flatpak host-gateway, portal, helper-resource, and legacy data-migration behavior unless a separate product decision retires existing AppImage-user migration. Treat "native" carefully: in this repo it can mean Linux/Tauri runtime, development binary, launch mode, or AppImage packaging.

## Architecture Context

- **System Structure**
  - CrossHook is a Tauri v2 Linux desktop app under `src/crosshook-native/`, with React/Vite frontend, thin `src-tauri` IPC/startup glue, and core business logic in `src/crosshook-native/crates/crosshook-core/`.
  - Current release architecture is AppImage-first: `.github/workflows/release.yml` builds AppImage and CLI assets, extracts/stages the Tauri binary for Flatpak, then publishes AppImage, CLI tarball, and Flatpak.
  - Flatpak packaging already exists in `packaging/flatpak/` and installs a prebuilt `crosshook-native` binary, runtime helpers, desktop/metainfo files, and icons.
  - Tauri config in `src/crosshook-native/src-tauri/tauri.conf.json` currently declares AppImage bundling, but Tauri still needs to build the production binary with embedded frontend assets for Flatpak.

- **Data Flow**
  - Current CI flow: tag push -> `build-native` -> `scripts/build-native.sh` AppImage build -> copy release binary into `dist/crosshook-native` -> upload Flatpak staging -> `build-flatpak` -> publish all release assets.
  - Desired CI flow: tag push -> version/core validation -> `tauri build --no-bundle` production binary -> production mock-code sentinel -> stage Flatpak inputs -> build Flatpak bundle -> publish only `CrossHook_<version>_amd64.flatpak`.
  - Local Flatpak flow should keep the staging pattern in `scripts/build-flatpak.sh`: build or reuse the production binary, stage assets into a temp root, validate desktop/metainfo, run `flatpak-builder`, then `flatpak build-bundle`.
  - No TOML settings or SQLite schema changes are expected; filesystem build artifacts and release assets change, but application persistence stays unchanged.

- **Integration Points**
  - GitHub Releases via `.github/workflows/release.yml` and `softprops/action-gh-release@v2` are the public release surface.
  - Flatpak toolchain remains required: `flatpak`, `flatpak-builder`, GNOME Platform/Sdk 50, desktop file validation, AppStream validation, and the release workflow Flatpak builder image.
  - Tauri remains required as a production frontend embedding/build tool; plain `cargo build --release` is not equivalent to `tauri build --no-bundle`.
  - Runtime Flatpak integration remains in `crosshook-core::platform`, `flatpak_migration`, portal modules, runtime helper lookup, and launch execution branches.

## Critical Files Reference

- `.github/workflows/release.yml`: Restructure away from AppImage/native release assets and publish Flatpak-only artifacts.
- `.github/pull_request_template.md`: Replace AppImage/native build checklist with Flatpak and relevant validation checks.
- `package.json`: Remove `build:appimage`; rename or rescope `build:binary` as an internal Flatpak prerequisite.
- `scripts/build-native.sh`: Split, rename, or reduce to production Tauri no-bundle binary build; remove default AppImage bundling path.
- `scripts/build-flatpak.sh`: Keep as the primary distribution build command; point at the surviving binary build helper and update AppImage/native wording.
- `scripts/lib/build-paths.sh`: Keep shared `DIST_DIR`/`CARGO_TARGET_DIR` helpers; remove AppImage bundle discovery helpers when unused.
- `scripts/build-native-container.sh`: AppImage-specific container wrapper; candidate for deletion.
- `scripts/build-native-container.Dockerfile`: AppImage-specific build image; candidate for deletion.
- `scripts/install-native-build-deps.sh`: Remove or rescope if it only supports AppImage/native packaging; Flatpak install deps belong in `build-flatpak.sh --install-deps`.
- `scripts/generate-crosshook-desktop.sh`: AppImage launcher generator; remove in favor of committed Flatpak desktop entry/install guidance.
- `scripts/generate-assets.sh`: Keep; Flatpak still uses generated icons.
- `scripts/lib/sync-tauri-icons.sh`: Likely removable if it only supports Tauri AppImage bundle icons.
- `src/crosshook-native/src-tauri/tauri.conf.json`: Remove AppImage bundle target while preserving build/frontend/resource semantics needed by Flatpak and dev.
- `src/crosshook-native/src-tauri/src/lib.rs`: Remove AppImage WebKit re-exec and AppImage-specific comments; preserve Flatpak migration event and non-AppImage startup behavior.
- `src/crosshook-native/src-tauri/src/paths.rs`: Preserve Flatpak `/app/resources` and dev runtime-helper lookup.
- `src/crosshook-native/crates/crosshook-core/src/platform/`: Preserve Flatpak detection and host-tool gateway abstractions.
- `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/`: Preserve one-way import for legacy AppImage-era users unless explicitly retired.
- `src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs`: Regression coverage for migration behavior to keep green.
- `packaging/flatpak/dev.crosshook.CrossHook.yml`: Keep aligned with the staged binary, helpers, icons, desktop file, and GNOME runtime version.
- `packaging/flatpak/README.md`: Rewrite from "Flatpak alongside AppImage/CLI" to Flatpak-only packaging/release guidance.
- `README.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `AGENTS.md`, `.ai/rules/project.md`, `.cursor/rules/project.mdc`, `.cursorrules`: Remove AppImage-first platform/build/release wording and sync agent-rule copies from the source of truth.
- `docs/internal-docs/local-build-publish.md`: Rewrite AppImage-centered build/publish guide into Flatpak-only release guidance.
- `docs/getting-started/quickstart.md`: Replace AppImage download/run setup with Flatpak install/run and legacy migration expectations.
- `docs/internal-docs/steam-deck-validation-checklist.md`: Replace direct AppImage execution examples with Flatpak validation steps.
- `docs/architecture/adr-0001-platform-host-gateway.md`: Keep host-gateway contract intact; update AppImage/Flatpak parity wording only.
- `docs/architecture/adr-0002-flatpak-portal-contracts.md`: Preserve portal behavior and runtime-only state assumptions.
- `docs/architecture/adr-0003-proton-download-manager.md`: Reconcile AppImage plus Flatpak parity wording with Flatpak-only distribution.
- `docs/architecture/adr-0004-flatpak-per-app-isolation.md`: Keep isolation/import design; reword as legacy AppImage/host XDG migration.
- `docs/prps/prds/flatpak-distribution.prd.md`: Historical plan now conflicts with Flatpak-only goal and should be reconciled.

## Patterns to Follow

- Preserve `tauri build --no-bundle` for release binary creation; it is the established way to embed production frontend assets for Flatpak.
- Keep build scripts fail-fast with `set -euo pipefail`, `die`/`log` helpers, explicit usage blocks, env var documentation, and early preflight checks.
- Keep Flatpak staging explicit and inspectable: binary, runtime helpers, icons, desktop file, metainfo, and manifest copied into a temporary build root.
- Keep optional metadata validators warning locally but failing in strict/CI mode.
- Rename by responsibility: prefer `build-release-binary` or Flatpak staging terminology over blanket removal of every `native` reference.
- Preserve existing validation surfaces: `scripts/lint.sh`, `scripts/check-host-gateway.sh`, `scripts/build-flatpak.sh --strict`, release artifact matching, and existing Rust tests.
- Update source-of-truth docs first where applicable, then sync agent-runtime copies according to repo policy.

## Cross-Cutting Concerns

- Backward compatibility: existing AppImage users may have data under host XDG paths; Flatpak first-run import is still valuable after AppImage release removal.
- Security/runtime correctness: Flatpak host-tool routing through `crosshook-core/src/platform/` must remain because Flatpak becomes the only distribution path.
- Release artifact shape: "Flatpak only" implies removing AppImage and likely CLI release uploads unless CLI retention is explicitly rescoped.
- Terminology drift: AppImage, native binary, native launch, and native Linux app are currently mixed across scripts/docs; implementation should reserve "native" for runtime semantics and use Flatpak/binary terms for packaging.
- Production mock sentinel: currently attached to the AppImage/native build job, but the check is distribution-agnostic and must move with the production Tauri binary build.
- Runtime helper resources: deleting Tauri bundle resources must not break Flatpak `/app/resources` helper resolution or dev fallback paths.
- Historical docs: avoid rewriting changelog/completed PRP history unless explicitly in scope; focus live docs and authoritative rules.

## Parallelization Opportunities

- CI/script cleanup can proceed in parallel with documentation cleanup once the target command names and artifact shape are chosen.
- Runtime AppImage startup cleanup can proceed independently from release workflow work, as long as Flatpak migration and helper lookup contracts are preserved.
- Docs can be split by audience: public README/quickstart, contributor/PR checklist, Flatpak packaging guide, internal build/publish guide, and agent-rule sync.
- Validation updates can run in parallel: shell/lint workflow path checks, Flatpak strict build/metadata validation, and Rust platform/migration tests.
- Dead-script removal can be batched separately from retained-script renaming to reduce review risk.

## Implementation Constraints

- Do not replace the Flatpak release binary with plain Cargo output; keep production Tauri no-bundle semantics.
- Do not remove `crosshook-core::platform`, host gateway functions, `is_flatpak()` behavior, portal modules, or Flatpak launch/session branches as AppImage cleanup.
- Do not remove `flatpak_migration` without an explicit migration-retirement decision.
- Keep `packaging/flatpak/dev.crosshook.CrossHook.yml`, `scripts/build-flatpak.sh`, and release workflow runtime version/container image aligned.
- Keep `scripts/generate-assets.sh` and Flatpak icon assets; they are not AppImage-only.
- Keep release metadata tooling such as `scripts/prepare-release.sh`, `scripts/render-release-notes.sh`, and `scripts/validate-release-notes.sh` unless concrete AppImage-only logic is found.
- Update `scripts/lint.sh` and shellcheck inputs when scripts are deleted or renamed.
- Preserve Tauri IPC command names, Serde boundary types, and frontend mock adapter behavior; AppImage removal is packaging/startup scope, not IPC scope.
- No storage migrations should be introduced for this feature.

## Key Recommendations

- First decide the surviving binary helper name and release artifact policy: Flatpak-only should remove AppImage and CLI release uploads unless CLI distribution is explicitly retained.
- Convert `scripts/build-native.sh --binary-only` into the canonical release-binary helper, then make `scripts/build-flatpak.sh` and CI consume that helper.
- Rewrite `.github/workflows/release.yml` around binary build, mock sentinel, Flatpak staging/build, and Flatpak-only release upload with unmatched-file failure enabled.
- Remove AppImage-only scripts and helpers together: container builder, desktop launcher generator, AppImage bundle discovery, AppImage root package script, and Tauri AppImage bundle target.
- Clean `src-tauri/src/lib.rs` narrowly: remove `APPIMAGE` re-exec/workaround branches and comments, but preserve Flatpak migration event emission and general WebKit/dev behavior.
- Treat Flatpak migration as legacy-user compatibility, not active dual-distribution parity; update ADR-0004 and user docs accordingly.
- Make docs a first-class workstream because AppImage assumptions are spread across README, contributor docs, internal release docs, Flatpak docs, architecture ADRs, and agent-rule files.
- Validate with `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`, `./scripts/lint.sh --shell --host-gateway` or full lint, and `./scripts/build-flatpak.sh --strict` where Flatpak tooling is available.
