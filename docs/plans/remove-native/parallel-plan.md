# Remove Native AppImage Distribution Implementation Plan

This plan removes AppImage as a supported CrossHook distribution path while preserving the production Tauri binary build that Flatpak still needs. The core strategy is to create a clearly named release-binary helper based on `tauri build --no-bundle`, make Flatpak packaging and release CI consume that helper, delete AppImage-only scripts/config/artifacts, and update live documentation so Flatpak is the only advertised distribution. Runtime cleanup must be narrow: remove AppImage-specific startup workarounds, but preserve Flatpak host-tool routing, portal behavior, `/app/resources` helper lookup, and legacy AppImage-era data import for existing users.

## Critically Relevant Files and Documentation

- .github/workflows/release.yml: Current tag release graph builds AppImage, CLI tarball, and Flatpak.
- .github/pull_request_template.md: Contributor checklist still asks for native/AppImage build checks.
- package.json: Root scripts expose AppImage and binary build aliases.
- scripts/build-native.sh: Mixed release-binary and AppImage bundling script.
- scripts/build-flatpak.sh: Canonical Flatpak local bundle helper.
- scripts/lib/build-paths.sh: Shared `DIST_DIR` and `CARGO_TARGET_DIR` resolver plus AppImage helper.
- scripts/build-native-container.sh: AppImage-only containerized build wrapper.
- scripts/build-native-container.Dockerfile: AppImage-only builder image.
- scripts/generate-crosshook-desktop.sh: AppImage desktop launcher generator.
- scripts/lib/sync-tauri-icons.sh: Tauri AppImage icon sync helper.
- scripts/install-native-build-deps.sh: Native/AppImage host dependency installer.
- scripts/generate-assets.sh: Branding asset generator still needed for Flatpak icons.
- scripts/lint.sh: Shell and host-gateway validation entrypoint.
- src/crosshook-native/src-tauri/tauri.conf.json: Tauri bundle config declares AppImage target.
- src/crosshook-native/src-tauri/src/lib.rs: Startup contains AppImage-specific WebKit re-exec logic.
- src/crosshook-native/src-tauri/src/paths.rs: Runtime helper resolver must keep Flatpak `/app/resources`.
- src/crosshook-native/crates/crosshook-core/src/platform/mod.rs: Flatpak detection and host gateway entrypoint.
- src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs: Legacy host/AppImage data import.
- src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs: Regression coverage for migration behavior.
- packaging/flatpak/dev.crosshook.CrossHook.yml: Flatpak manifest and staged file contract.
- packaging/flatpak/README.md: Flatpak packaging guide with AppImage coexistence wording.
- packaging/PKGBUILD: Non-Flatpak package recipe that conflicts with Flatpak-only distribution.
- README.md: Public install, build, and release artifact guidance.
- CONTRIBUTING.md: Contributor build and validation guidance.
- docs/getting-started/quickstart.md: User installation guide currently AppImage-first.
- docs/internal-docs/local-build-publish.md: Internal build/publish guide centered on AppImage.
- docs/internal-docs/steam-deck-validation-checklist.md: Validation guide runs AppImage directly.
- docs/architecture/adr-0001-platform-host-gateway.md: Host-command gateway contract to preserve.
- docs/architecture/adr-0002-flatpak-portal-contracts.md: Flatpak portal contract to preserve.
- docs/architecture/adr-0003-proton-download-manager.md: AppImage/Flatpak parity wording to update.
- docs/architecture/adr-0004-flatpak-per-app-isolation.md: Legacy data import and Flatpak isolation design.
- docs/prps/prds/flatpak-distribution.prd.md: Historical Flatpak PRD that treated AppImage as primary.
- CLAUDE.md: Source-of-truth agent rules for platform and command references.
- AGENTS.md: Agent-runtime copy that must stay aligned with `CLAUDE.md`.
- .ai/rules/project.md: Vendor-neutral agent rule copy.
- .cursor/rules/project.mdc: Cursor agent rule copy.
- .cursorrules: Legacy Cursor rule copy.
- src/crosshook-native/src/lib/mocks/README.md: Production mock sentinel documentation.

## Implementation Plan

### Phase 1: Establish the Flatpak Binary Build Primitive

#### Task 1.1: Create a Flatpak release binary helper Depends on [none]

**READ THESE BEFORE TASK**

- scripts/build-native.sh
- scripts/build-flatpak.sh
- scripts/lib/build-paths.sh
- src/crosshook-native/src-tauri/tauri.conf.json

**Instructions**

Files to Create

- scripts/build-release-binary.sh

Files to Modify

- scripts/build-native.sh
- scripts/lib/build-paths.sh

Create `scripts/build-release-binary.sh` as the canonical helper for producing `DIST_DIR/crosshook-native` using the existing `tauri build --no-bundle` behavior from `scripts/build-native.sh --binary-only`. Preserve the Tauri CLI fallback order (`cargo tauri`, local `node_modules/.bin/tauri`, then `npx tauri`), `crosshook_build_paths_init`, caller-provided `DIST_DIR`/`CARGO_TARGET_DIR`, `--print-paths`, and clear fail-fast shell style. Remove AppImage-only functions and default bundling behavior from `scripts/build-native.sh`; either convert it into a compatibility shim that tells users to run `scripts/build-release-binary.sh` or delete it in a later task after callers are updated. Keep `scripts/lib/build-paths.sh` path initialization, but do not remove `crosshook_appimage_bundle_dirs()` until all callers are gone.

#### Task 1.2: Update root build aliases for Flatpak-only commands Depends on [1.1]

**READ THESE BEFORE TASK**

- package.json
- scripts/build-release-binary.sh
- scripts/build-flatpak.sh

**Instructions**

Files to Create

- none

Files to Modify

- package.json

Replace the root `build:binary` script with the new release-binary helper created by Task 1.1 and remove `build:appimage`. Keep `flatpak:build`, `flatpak:install`, `flatpak:update`, and `flatpak:run` as the user-facing distribution commands. If a binary script remains exposed, name it as an internal Flatpak prerequisite rather than a standalone distribution path.

#### Task 1.3: Retire AppImage-oriented dependency installer Depends on [1.1]

**READ THESE BEFORE TASK**

- scripts/install-native-build-deps.sh
- scripts/build-release-binary.sh
- scripts/build-flatpak.sh

**Instructions**

Files to Create

- none

Files to Modify

- scripts/install-native-build-deps.sh

Decide whether `scripts/install-native-build-deps.sh` should remain as a release-binary build dependency helper or be converted into a clear deprecation shim that points users at `scripts/build-flatpak.sh --install-deps`. Remove AppImage-specific wording and packages that are only needed for AppImage bundling. If the helper remains, document it as a Tauri release-binary prerequisite helper, not a distribution installer.

### Phase 2: Make Flatpak Packaging Consume the New Primitive

#### Task 2.1: Repoint local Flatpak packaging at the release binary helper Depends on [1.1]

**READ THESE BEFORE TASK**

- scripts/build-flatpak.sh
- scripts/build-release-binary.sh
- packaging/flatpak/dev.crosshook.CrossHook.yml

**Instructions**

Files to Create

- none

Files to Modify

- scripts/build-flatpak.sh

Update `scripts/build-flatpak.sh` so `--rebuild` and missing-binary fallback call `scripts/build-release-binary.sh` from Task 1.1 instead of `scripts/build-native.sh --binary-only`. Rewrite usage text and log messages to describe the release binary as a Flatpak packaging input, not an AppImage/native build shortcut. Preserve strict metadata validation, staging via `install -Dm...`, runtime helper staging, icon staging, manifest staging, `flatpak-builder`, and `flatpak build-bundle`.

#### Task 2.2: Preserve and document Flatpak asset generation boundaries Depends on [2.1]

**READ THESE BEFORE TASK**

- scripts/generate-assets.sh
- scripts/build-flatpak.sh
- packaging/flatpak/dev.crosshook.CrossHook.yml
- packaging/flatpak/README.md

**Instructions**

Files to Create

- none

Files to Modify

- packaging/flatpak/README.md
- scripts/build-flatpak.sh

Make the Flatpak packaging docs and script comments clear that `assets/icon-128.png`, `assets/icon-256.png`, and `assets/icon-512.png` remain Flatpak inputs generated by `scripts/generate-assets.sh`. Do not keep AppImage icon-sync wording. Keep GNOME runtime version guidance aligned between the manifest, script default, and CI image.

### Phase 3: Delete AppImage-Only Local Build Surfaces

#### Task 3.1: Remove AppImage builder scripts and dead helpers Depends on [1.2, 2.1]

**READ THESE BEFORE TASK**

- scripts/build-native-container.sh
- scripts/build-native-container.Dockerfile
- scripts/generate-crosshook-desktop.sh
- scripts/lib/sync-tauri-icons.sh
- scripts/lib/build-paths.sh

**Instructions**

Files to Create

- none

Files to Modify

- scripts/lib/build-paths.sh

Files to Delete

- scripts/build-native-container.sh
- scripts/build-native-container.Dockerfile
- scripts/generate-crosshook-desktop.sh
- scripts/lib/sync-tauri-icons.sh

Delete the AppImage container builder, AppImage desktop launcher generator, and Tauri AppImage icon sync helper after confirming no live caller remains. Remove `crosshook_appimage_bundle_dirs()` from `scripts/lib/build-paths.sh` once `scripts/build-native.sh` no longer uses it. Do not remove `scripts/generate-assets.sh`; Flatpak still consumes its generated icons.

#### Task 3.2: Remove or retire non-Flatpak package recipes Depends on [none]

**READ THESE BEFORE TASK**

- packaging/PKGBUILD
- packaging/flatpak/README.md
- README.md

**Instructions**

Files to Create

- none

Files to Modify

- README.md

Files to Delete

- packaging/PKGBUILD

Delete `packaging/PKGBUILD` so Flatpak is the only in-repo distribution recipe. Update any README/package documentation that references Arch packaging or non-Flatpak package outputs. If there is a strong reason to keep the PKGBUILD as a historical artifact, move it to documentation instead of leaving it as an active packaging recipe, but the default implementation should remove it.

### Phase 4: Rewrite Release CI for Flatpak-Only Publishing

#### Task 4.1: Replace AppImage release producer with binary and Flatpak staging Depends on [1.1, 2.1, 2.2]

**READ THESE BEFORE TASK**

- .github/workflows/release.yml
- scripts/build-release-binary.sh
- scripts/build-flatpak.sh
- packaging/flatpak/dev.crosshook.CrossHook.yml

**Instructions**

Files to Create

- none

Files to Modify

- .github/workflows/release.yml

Rename the `build-native` job to a responsibility-based name such as `build-release-binary` or `stage-flatpak-assets`. Remove `APPIMAGE_CARGO_TARGET_DIR`, the full AppImage build step, AppImage output paths, and `native-release-assets`. Keep version verification, Rust setup, Node setup, Linux build prerequisites needed for the no-bundle Tauri binary, `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`, the production mock-code sentinel against `src/crosshook-native/dist/assets/*.js`, and Flatpak staging assets. Stage `dist/crosshook-native` from the new helper instead of extracting it from an AppImage build target.

#### Task 4.2: Publish only the Flatpak release asset Depends on [4.1]

**READ THESE BEFORE TASK**

- .github/workflows/release.yml
- scripts/render-release-notes.sh
- scripts/validate-release-notes.sh

**Instructions**

Files to Create

- none

Files to Modify

- .github/workflows/release.yml

Update `build-flatpak` dependencies and artifact downloads to use the renamed staging artifact. Remove CLI tarball packaging/upload and all AppImage release globs from `publish-release`, leaving only `dist/CrossHook_${{ env.RELEASE_VERSION }}_amd64.flatpak` or the exact bundle name produced by the Flatpak job. Keep `fail_on_unmatched_files: true`, release-note rendering, and release-note validation. The finished workflow should make it impossible for a tag release to publish AppImage or CLI distribution assets.

### Phase 5: Remove AppImage Runtime and Tauri Bundle Configuration

#### Task 5.1: Remove Tauri AppImage bundle target Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/tauri.conf.json
- scripts/build-release-binary.sh
- src/crosshook-native/src-tauri/src/paths.rs
- packaging/flatpak/dev.crosshook.CrossHook.yml

**Instructions**

Files to Create

- none

Files to Modify

- src/crosshook-native/src-tauri/tauri.conf.json

Remove `bundle.targets: ["appimage"]` and any AppImage-only bundle configuration from Tauri config. Verify Tauri v2 config semantics before choosing the final shape: the finished config must not cause plain `tauri build` to emit AppImage or any other native distribution bundle. Preserve `build.beforeBuildCommand`, `build.frontendDist`, app window/security settings, and any configuration still needed for no-bundle production binary generation through the helper created in Task 1.1. Confirm runtime helpers remain installed by the Flatpak manifest and resolved by `src-tauri/src/paths.rs`.

#### Task 5.2: Delete AppImage startup workaround while preserving Flatpak startup Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/lib.rs
- docs/architecture/adr-0004-flatpak-per-app-isolation.md
- src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs

**Instructions**

Files to Create

- none

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Remove the `APPIMAGE` WebKitGTK re-exec block, AppImage/linuxdeploy-specific comments, and any environment handling that exists only for AppImage bundling. Preserve Flatpak migration startup, `flatpak-migration-complete` event emission, legacy app-id migration, and any general WebKitGTK/Wayland fallback still needed for Flatpak or development. Do not change Tauri IPC command registration.

### Phase 6: Update Live Documentation and Agent Rules

#### Task 6.1: Rewrite public install and quickstart docs Depends on [1.2, 2.2, 3.2, 4.2]

**READ THESE BEFORE TASK**

- README.md
- docs/getting-started/quickstart.md
- docs/internal-docs/steam-deck-validation-checklist.md
- packaging/flatpak/README.md
- docs/architecture/adr-0004-flatpak-per-app-isolation.md

**Instructions**

Files to Create

- none

Files to Modify

- README.md
- docs/getting-started/quickstart.md
- docs/internal-docs/steam-deck-validation-checklist.md
- packaging/flatpak/README.md

Replace AppImage download/run/build instructions with Flatpak install, build, update, and run guidance. Document that current releases publish Flatpak bundles only. Describe legacy AppImage-era host data import as a one-way compatibility bridge for existing users, not as ongoing dual-distribution support. Replace Steam Deck validation examples that run `CrossHook_amd64.AppImage` with Flatpak install/run and gamescope validation steps aligned with `scripts/build-flatpak.sh`.

#### Task 6.2: Rewrite contributor, PR, and internal release guidance Depends on [1.2, 2.2, 4.2]

**READ THESE BEFORE TASK**

- CONTRIBUTING.md
- .github/pull_request_template.md
- docs/internal-docs/local-build-publish.md
- src/crosshook-native/src/lib/mocks/README.md

**Instructions**

Files to Create

- none

Files to Modify

- CONTRIBUTING.md
- .github/pull_request_template.md
- docs/internal-docs/local-build-publish.md
- src/crosshook-native/src/lib/mocks/README.md

Replace AppImage packaging checks with the new release-binary helper, Flatpak strict build/metadata validation, `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`, and relevant lint/typecheck guidance. Update the mock-code sentinel docs so it is tied to production bundle validation rather than AppImage output. Remove stale artifact filenames for `.AppImage` and CLI release tarballs.

#### Task 6.3: Update architecture docs and historical Flatpak planning docs Depends on [4.2, 5.1, 5.2]

**READ THESE BEFORE TASK**

- docs/architecture/adr-0001-platform-host-gateway.md
- docs/architecture/adr-0002-flatpak-portal-contracts.md
- docs/architecture/adr-0003-proton-download-manager.md
- docs/architecture/adr-0004-flatpak-per-app-isolation.md
- docs/prps/prds/flatpak-distribution.prd.md

**Instructions**

Files to Create

- none

Files to Modify

- docs/architecture/adr-0001-platform-host-gateway.md
- docs/architecture/adr-0003-proton-download-manager.md
- docs/architecture/adr-0004-flatpak-per-app-isolation.md
- docs/prps/prds/flatpak-distribution.prd.md

Update ADR wording that describes CrossHook as AppImage-first or AppImage plus Flatpak parity. Preserve technical decisions for host-tool gateway, portal contracts, Proton Manager, and Flatpak isolation. In ADR-0004, keep migration semantics but reword the source as legacy host/AppImage-era data. Mark the old Flatpak PRD as superseded by Flatpak-only distribution if rewriting the entire historical plan would obscure useful context.

#### Task 6.4: Sync agent-runtime rules from the source of truth Depends on [6.1, 6.2, 6.3]

**READ THESE BEFORE TASK**

- CLAUDE.md
- AGENTS.md
- .ai/rules/project.md
- .cursor/rules/project.mdc
- .cursorrules

**Instructions**

Files to Create

- none

Files to Modify

- CLAUDE.md
- AGENTS.md
- .ai/rules/project.md
- .cursor/rules/project.mdc
- .cursorrules

Update `CLAUDE.md` first because it is the source of truth, then sync the agent-runtime copies without duplicating policy prose beyond each file's established format. Replace AppImage platform and command references with Flatpak-only distribution guidance, preserve the native Linux app distinction, and keep Flatpak host-gateway and browser dev mode rules intact.

### Phase 7: Final Validation and Dead-Reference Sweep

#### Task 7.1: Sweep stale AppImage and deleted-script references Depends on [6.4, 3.1, 4.2, 5.1, 5.2]

**READ THESE BEFORE TASK**

- docs/plans/remove-native/shared.md
- docs/plans/remove-native/analysis-code.md
- docs/plans/remove-native/analysis-tasks.md

**Instructions**

Files to Create

- none

Files to Modify

- none

Run final searches for live references to `AppImage`, `appimage`, `APPIMAGE`, `build:appimage`, `build-native-container`, `generate-crosshook-desktop`, `sync-tauri-icons`, `crosshook_appimage_bundle_dirs`, AppImage release globs, and CLI release artifact globs. Classify remaining hits as either historical/changelog/planning references, legacy migration references, or active bugs. Fix active bugs before proceeding to final validation; the final state should have no active AppImage build, release, install, or local launcher instructions.

#### Task 7.2: Run targeted code and packaging validation Depends on [7.1]

**READ THESE BEFORE TASK**

- scripts/lint.sh
- scripts/check-host-gateway.sh
- scripts/build-flatpak.sh
- src/crosshook-native/Cargo.toml
- src/crosshook-native/package.json

**Instructions**

Files to Create

- none

Files to Modify

- none

Run the validation commands that match the changed surface: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`, `./scripts/lint.sh --shell --host-gateway` or full `./scripts/lint.sh`, `cd src/crosshook-native && npm run typecheck && npm test` when frontend/Tauri config changes are in scope, and `./scripts/build-flatpak.sh --strict` where Flatpak tooling is available. If Flatpak tooling is unavailable locally, run whatever metadata validators are installed and document the tooling gap. Confirm the new release-binary helper creates `DIST_DIR/crosshook-native` and that production JS assets exist for the mock sentinel after the stale-reference sweep is clean.

## Advice

- The safest first implementation choice is to introduce `scripts/build-release-binary.sh` and then update callers; deleting `scripts/build-native.sh` immediately makes it harder to distinguish AppImage bundling from the production binary build Flatpak needs.
- Do not treat every `native` string as removable. CrossHook remains a native Linux/Tauri application, and some "native" references are runtime/dev-mode terminology rather than AppImage distribution.
- Keep the production mock-code sentinel after the no-bundle Tauri build. Its security value is checking frontend bundle contents, not validating AppImage packaging.
- Removing AppImage release builds does not remove existing users' host XDG data. Keep `flatpak_migration` unless a separate decision explicitly drops migration support.
- `packaging/PKGBUILD` is not AppImage, but it is a second distribution recipe; removing it aligns the repository with the stated Flatpak-only target.
- Avoid broad docs rewrites of `CHANGELOG.md`, completed PRPs, or archived research. Live install/build/release docs should change; historical records can keep historical AppImage mentions.
- Validate release workflow artifact names carefully: `fail_on_unmatched_files: true` only helps if the file list contains the intended Flatpak-only glob.
