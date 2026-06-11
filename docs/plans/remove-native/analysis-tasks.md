# Remove Native Analysis Tasks

## Executive Summary

Implement `remove-native` as a Flatpak-first packaging/release refactor, not as a removal of the Tauri Linux application or Flatpak host/runtime support. The critical sequencing is to preserve the production Tauri `--no-bundle` binary path required by Flatpak, then delete AppImage producers and release artifacts, then clean runtime/docs/agent surfaces that still describe AppImage as primary. No new persisted data is required; the only storage-sensitive decision is to keep legacy AppImage-era host XDG import for existing users migrating to Flatpak.

## Recommended Phase Structure

### Phase 1: Define the Flatpak-only build primitive

- **Goal**: Replace the AppImage-oriented "native build" entrypoint with an explicit release-binary/staging path that still uses `tauri build --no-bundle`.
- **Primary task**: Rename or refactor `scripts/build-native.sh --binary-only` semantics into a Flatpak/binary helper, preserving production frontend embedding, asset generation needed by Flatpak, `DIST_DIR`, `CARGO_TARGET_DIR`, and existing fail-fast shell conventions.
- **Parallelism**: Can run alongside release-workflow analysis, but should land before CI and `package.json` script rewrites.
- **Validation**: Run the new binary helper, confirm `dist/crosshook-native` exists, and confirm generated frontend assets still exist under `src/crosshook-native/dist/assets/`.

### Phase 2: Update Flatpak packaging to consume the new primitive

- **Goal**: Make `scripts/build-flatpak.sh` the canonical local packaging entrypoint for distribution.
- **Primary task**: Point `scripts/build-flatpak.sh` at the renamed/refactored binary helper; preserve staging of `crosshook-native`, runtime helpers, icons, desktop file, metainfo, and `packaging/flatpak/dev.crosshook.CrossHook.yml`.
- **Parallelism**: Depends on Phase 1 helper shape. Metadata/docs updates under `packaging/flatpak/README.md` can start in parallel after the contract is known.
- **Validation**: Run `./scripts/build-flatpak.sh --strict` where Flatpak tooling is available; otherwise run metadata validators directly if installed.

### Phase 3: Remove AppImage-only build and local integration surfaces

- **Goal**: Delete code paths that only exist to produce, wrap, or launch AppImage artifacts.
- **Primary tasks**: Remove AppImage bundling logic from `scripts/build-native.sh` or replace the file with the new helper; delete `scripts/build-native-container.sh`, `scripts/build-native-container.Dockerfile`, `scripts/generate-crosshook-desktop.sh`, and dead AppImage helpers from `scripts/lib/build-paths.sh`; remove `build:appimage` from `package.json`.
- **Parallelism**: Mostly independent once Phase 1 is complete. Coordinate with docs tasks because removed script names appear widely.
- **Validation**: Run `./scripts/lint.sh --shell --host-gateway` or full `./scripts/lint.sh`; search for references to deleted script names before finishing.

### Phase 4: Rewrite release CI around Flatpak-only artifacts

- **Goal**: Change `.github/workflows/release.yml` from AppImage + CLI + Flatpak publishing to Flatpak-only publishing.
- **Primary tasks**: Rename/re-scope `build-native` to a binary/staging producer or fold staging into the Flatpak job; remove `native-release-assets`; remove AppImage and likely CLI tarball release uploads; keep version checks, `cargo test -p crosshook-core`, production mock sentinel, Flatpak metadata validation, Flatpak bundle build, and `fail_on_unmatched_files: true`.
- **Parallelism**: Depends on Phase 1/2 command contracts. Can be reviewed independently from runtime cleanup.
- **Validation**: Use `actionlint` if available, inspect artifact names for unmatched AppImage/CLI globs, and verify the publish job downloads only the Flatpak bundle artifact.

### Phase 5: Remove AppImage-specific Tauri runtime/config branches

- **Goal**: Stop configuring or special-casing AppImage while preserving Flatpak runtime behavior.
- **Primary tasks**: Remove `bundle.targets: ["appimage"]` from `src/crosshook-native/src-tauri/tauri.conf.json`; remove `APPIMAGE` WebKitGTK re-exec and AppImage-only comments from `src/crosshook-native/src-tauri/src/lib.rs`; keep Flatpak migration, Flatpak event emission, `WEBKIT_DISABLE_DMABUF_RENDERER` behavior if still generally needed, and runtime-helper lookup in `src/crosshook-native/src-tauri/src/paths.rs`.
- **Parallelism**: Can run alongside docs work, but should follow Phase 1 because Tauri config changes can affect the binary helper.
- **Validation**: Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`; run a production binary build; verify `scripts/check-host-gateway.sh` still passes.

### Phase 6: Update public, contributor, internal, and agent documentation

- **Goal**: Make Flatpak the only supported distribution story and clearly label legacy AppImage data import as migration support.
- **Primary tasks**: Rewrite AppImage-first sections in `README.md`, `CONTRIBUTING.md`, `packaging/flatpak/README.md`, `docs/internal-docs/local-build-publish.md`, `docs/getting-started/quickstart.md`, `docs/internal-docs/steam-deck-validation-checklist.md`, `.github/pull_request_template.md`, `CLAUDE.md`, `AGENTS.md`, `.ai/rules/project.md`, `.cursor/rules/project.mdc`, and `.cursorrules`.
- **Parallelism**: Can split by doc audience once script and CI names are stable: public docs, contributor/release docs, architecture/agent rules.
- **Validation**: Search for live AppImage guidance with `rg -n "AppImage|build-native|build-native-container|generate-crosshook-desktop|build:appimage"` and intentionally leave only historical/changelog/legacy-migration references.

### Phase 7: Final cross-surface validation and dead-reference sweep

- **Goal**: Ensure no AppImage distribution path remains and Flatpak behavior is still protected.
- **Primary tasks**: Run repo searches for AppImage artifacts, deleted scripts, release globs, package script names, and stale CI artifact names; run shell lint/host gateway checks, core Rust tests, typecheck if frontend config changed, and Flatpak build/metadata validation where tooling is present.
- **Parallelism**: Sequential final gate after all implementation/docs tasks.
- **Validation**: Produce evidence for the final implementation report: commands run, unavailable tooling noted, and any intentionally retained legacy-migration references listed.

## Task Granularity Recommendations

- Keep build-script refactoring as one focused task because `scripts/build-native.sh`, `scripts/build-flatpak.sh`, `scripts/lib/build-paths.sh`, and `package.json` share command names and output paths.
- Keep release CI as its own task. `.github/workflows/release.yml` has artifact dependency edges that are easy to break if mixed with docs cleanup.
- Keep runtime cleanup separate from packaging scripts. `src/crosshook-native/src-tauri/src/lib.rs` contains both removable AppImage startup logic and required Flatpak migration/event logic.
- Split documentation by audience rather than by file count: public install/build docs, contributor/release docs, architecture/ADR docs, and agent-runtime rule copies.
- Do not make a broad "remove native" task. In this repo, "native" also means Linux/Tauri runtime and non-Flatpak dev execution; tasks should target AppImage distribution surfaces explicitly.
- Treat `packaging/PKGBUILD` as a scope checkpoint. It is not AppImage, but it is a non-Flatpak package recipe; if the product goal is literally Flatpak-only distribution, include removal or deprecation in the plan instead of leaving it accidental.
- Keep `flatpak_migration` as a compatibility task, not a deletion task. Removing AppImage builds does not remove existing users' host data.

## Dependency Analysis

- Phase 1 must precede Phases 2, 3, and 4 because the Flatpak script, package scripts, and release workflow need a stable production-binary command.
- Phase 2 must precede final release CI validation because CI should either reuse the Flatpak staging script or mirror its staging contract exactly.
- Phase 3 can run after Phase 1 and in parallel with Phase 5, but docs should not finalize command examples until deleted/renamed scripts are settled.
- Phase 4 depends on the Phase 1/2 artifact contract and should preserve the production mock sentinel from the old native/AppImage job.
- Phase 5 depends on understanding whether Tauri resources are still needed for `tauri build --no-bundle`; do not remove runtime-helper resources before verifying `src-tauri/src/paths.rs` and Flatpak `/app/resources` fallback behavior.
- Phase 6 depends on implementation decisions for CLI tarball retention, `packaging/PKGBUILD`, script names, and migration wording.
- Phase 7 is sequential and should be the last gate because stale AppImage references will be expected until scripts, CI, runtime, and docs all land.

## File-to-Task Mapping

- **Build primitive**: `scripts/build-native.sh`, `scripts/lib/build-paths.sh`, `scripts/generate-assets.sh`, `scripts/lib/sync-tauri-icons.sh`, `package.json`.
- **Flatpak packaging**: `scripts/build-flatpak.sh`, `packaging/flatpak/dev.crosshook.CrossHook.yml`, `packaging/flatpak/dev.crosshook.CrossHook.desktop`, `packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml`, `packaging/flatpak/README.md`.
- **AppImage-only deletion**: `scripts/build-native-container.sh`, `scripts/build-native-container.Dockerfile`, `scripts/generate-crosshook-desktop.sh`, AppImage bundle discovery in `scripts/lib/build-paths.sh`, AppImage package script entries in `package.json`.
- **Release CI**: `.github/workflows/release.yml`, `.github/pull_request_template.md`, `scripts/render-release-notes.sh`, `scripts/validate-release-notes.sh`, `scripts/prepare-release.sh`.
- **Runtime/config cleanup**: `src/crosshook-native/src-tauri/tauri.conf.json`, `src/crosshook-native/src-tauri/src/lib.rs`, `src/crosshook-native/src-tauri/src/paths.rs`, `src/crosshook-native/crates/crosshook-core/src/platform/`, `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/`.
- **Runtime validation coverage**: `src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs`, `src/crosshook-native/crates/crosshook-core/src/platform/tests/`, `scripts/check-host-gateway.sh`, `scripts/lint.sh`.
- **Public docs**: `README.md`, `docs/getting-started/quickstart.md`, `docs/internal-docs/steam-deck-validation-checklist.md`.
- **Contributor/internal docs**: `CONTRIBUTING.md`, `docs/internal-docs/local-build-publish.md`, `docs/TESTING.md`, `src/crosshook-native/README.md`, `src/crosshook-native/tests/README.md`, `src/crosshook-native/src/lib/mocks/README.md`.
- **Architecture/agent docs**: `docs/architecture/adr-0001-platform-host-gateway.md`, `docs/architecture/adr-0002-flatpak-portal-contracts.md`, `docs/architecture/adr-0003-proton-download-manager.md`, `docs/architecture/adr-0004-flatpak-per-app-isolation.md`, `docs/prps/prds/flatpak-distribution.prd.md`, `CLAUDE.md`, `AGENTS.md`, `.ai/rules/project.md`, `.cursor/rules/project.mdc`, `.cursorrules`.
- **Scope checkpoint**: `packaging/PKGBUILD`, `.gitignore`, `CHANGELOG.md`.

## Optimization Opportunities

- Collapse the current release graph by producing Flatpak staging assets directly from a single binary/staging job, or by letting the Flatpak job invoke the same binary helper before `flatpak-builder`.
- Rename around responsibilities: `build-release-binary` or `build-flatpak-binary` is clearer than continuing to expose "native" as a distribution command.
- Keep the production mock sentinel as a named release validation step independent of AppImage. Its value is checking bundled frontend assets, not checking a specific package format.
- Preserve `scripts/generate-assets.sh` and remove only the Tauri/AppImage icon sync if it becomes dead. Flatpak still needs generated hicolor icons.
- Use existing validators instead of adding bespoke checks: `scripts/lint.sh`, `scripts/check-host-gateway.sh`, `scripts/build-flatpak.sh --strict`, core Rust tests, and release workflow artifact matching.
- Update `CLAUDE.md` first for agent policy, then sync `AGENTS.md`, `.ai/rules/project.md`, `.cursor/rules/project.mdc`, and `.cursorrules` to avoid divergent policy prose.
- Leave historical references in changelog, completed PRPs, and archived research unless they are actively misleading current setup instructions.

## Implementation Strategy Recommendations

- Preserve the production Tauri binary build by continuing to use `tauri build --no-bundle`; do not replace it with `cargo build --release`.
- Treat Flatpak host-gateway, portal, runtime-helper, and migration code as required Flatpak behavior. Avoid deleting `platform::is_flatpak()` branches unless they are demonstrably AppImage-only.
- Decide early whether the CLI tarball and `packaging/PKGBUILD` remain supported. The stated goal is Flatpak-only distribution, so the default plan should remove release publishing for the CLI tarball and explicitly handle non-Flatpak package recipes.
- Make AppImage deletion observable through release artifact names: the publish job should match only `CrossHook_<version>_amd64.flatpak`, and unmatched AppImage/CLI globs should fail rather than silently pass.
- Keep legacy migration wording precise: "legacy AppImage-era host XDG data import" is a compatibility bridge for existing users, not ongoing AppImage support.
- Run validation in increasing scope: focused binary/Flatpak script checks, shell/host-gateway checks, core Rust tests, workflow/static checks, then final stale-reference searches.
- Honor `--no-worktree` in the generated implementation plan by omitting worktree setup sections and per-task worktree annotations.
