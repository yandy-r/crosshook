# Documentation Research: remove-native

## Architecture Docs

Required reading:

- `docs/architecture/adr-0001-platform-host-gateway.md`: Defines the Flatpak host-command gateway, denylisted tools, env-threading invariants, and enforcement via `scripts/check-host-gateway.sh`. Important because removing AppImage must not remove Flatpak host-spawn abstractions or the "native Linux app orchestrates Proton/Wine" product model.
- `docs/architecture/adr-0002-flatpak-portal-contracts.md`: Defines GameMode and Background portal contracts, their separation from host-command execution, and runtime-only capability state. Required for preserving Flatpak-only runtime behavior after native packaging removal.
- `docs/architecture/adr-0003-proton-download-manager.md`: Documents Proton Manager parity across AppImage and Flatpak, in-process download/extract decisions, SQLite `proton_release_catalog`, and "no bundled Proton" policy. Needs wording updates from "AppImage + Flatpak parity" to Flatpak-only distribution while preserving native feature semantics.
- `docs/architecture/adr-0004-flatpak-per-app-isolation.md`: Defines default Flatpak per-app data isolation, one-way import from the old AppImage tree, host prefix roots, and opt-in shared mode. Required because AppImage removal changes how migration docs should describe legacy data.

Nice-to-have:

- `docs/research/flatpak-bundling/10-evidence.md`: Evidence index for Flatpak architecture claims.
- `docs/research/flatpak-bundling/11-patterns.md`: Categorizes what should remain host-delegated versus in-process/native.
- `docs/research/flatpak-bundling/12-risks.md`: Risk register for Flatpak permissions, host execution, and portal behavior.
- `docs/research/flatpak-bundling/14-recommendations.md`: Implementation recommendation history for Flatpak phases, host gateway, and Proton management.
- `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md`: Portal ground truth used by ADR-0002.
- `docs/research/flatpak-bundling/16-protontricks-flatpak-viability.md`: Useful if removal work touches prefix dependency tooling or host-tool guidance.

Docs that need updates:

- `CLAUDE.md`: Still calls CrossHook a Tauri v2 AppImage app and lists `build-native.sh`, `build-native-container.sh`, and `build-native.sh --binary-only` in the short command reference.
- `AGENTS.md`: Mirrors/stores the same AppImage-specific platform, command, stack overview, and Flatpak migration wording. It also describes importing "host AppImage data" and the `verify:no-mocks` sentinel after every AppImage build.
- `.ai/rules/project.md`: Agent-facing project rules still identify AppImage as the package model.
- `.cursor/rules/project.mdc`: Cursor rule copy still references AppImage packaging, AppImage build commands, and AppImage-specific mock sentinel wording.

## API Docs

Required reading:

- `src/crosshook-native/src/lib/mocks/README.md`: Describes browser dev mode IPC mocks. Important because release validation currently scans production bundles for mock sentinels after AppImage/native build output.
- `docs/TESTING.md`: Canonical frontend testing entry point. Mentions browser dev mode is not WebKitGTK parity and still requires `./scripts/dev-native.sh` before merging UI changes.
- `src/crosshook-native/tests/README.md`: Playwright smoke guide; repeats that browser dev mode is not WebKitGTK parity and points to `./scripts/dev-native.sh`.

Nice-to-have:

- `docs/internal-docs/profile-collections-browser-mocks.md`: Contains the older `verify:no-mocks` release workflow excerpt and says the sentinel scans after the AppImage build. Update if the production bundle verification moves to the Flatpak-only release pipeline.

Docs that need updates:

- No standalone backend API reference was found for packaging commands. The relevant "API" surface is the documented script interface in `README.md`, `CONTRIBUTING.md`, `docs/internal-docs/local-build-publish.md`, and `scripts/build-flatpak.sh`.

## Development Guides

Required reading:

- `CONTRIBUTING.md`: Build and PR guide. It instructs contributors to run `./scripts/build-native.sh`, describes AppImage outputs, says UI changes are verified by dev app or AppImage, and has PR checklist references to `build-native.sh --binary-only` and AppImage packaging.
- `packaging/flatpak/README.md`: Flatpak packaging guide. It says the release publish job attaches Flatpak "alongside AppImage and CLI assets"; documents AppImage-to-Flatpak data import and AppImage/Flatpak shared mode; and is the source for GNOME runtime upgrade steps.
- `docs/internal-docs/local-build-publish.md`: Most AppImage-heavy internal guide. It covers AppImage icon generation, `build-native.sh`, containerized native builds, generated desktop launchers from AppImages, release CI AppImage steps, and expected AppImage artifact filenames.
- `.github/workflows/release.yml`: Not prose, but it is the authoritative release workflow. It currently has `build-native`, exports the Flatpak binary from native/AppImage cargo output, uploads AppImage and CLI tarball assets, builds Flatpak from staging assets, then publishes all three assets.
- `.github/pull_request_template.md`: Contributor-facing PR checklist still requires `build-native.sh --binary-only` and full AppImage when touching build/packaging.

Nice-to-have:

- `.github/copilot-instructions.md`: Mostly PR conventions, but it points agents back to `CLAUDE.md`/`AGENTS.md` for host gateway and architecture rules.
- `.github/ISSUE_TEMPLATE/bug_report.yml` and `.github/ISSUE_TEMPLATE/feature_request.yml`: Include "Build / Packaging" area choices; no AppImage wording, but useful if issue templates are adjusted for Flatpak-only reports.
- `docs/internal-docs/steam-deck-validation-checklist.md`: Uses `./CrossHook_amd64.AppImage` in gamescope validation examples. Should become Flatpak-run or Flatpak-installed validation instructions.
- `docs/getting-started/quickstart.md`: User-facing setup guide currently teaches AppImage download/run for Linux Desktop and Steam Deck, and says AppImage does not require Flatpak.
- `docs/TESTING.md` and `src/crosshook-native/tests/README.md`: Keep if dev/native Tauri launch remains, but update naming if `dev-native.sh` or "native" terminology changes.

Docs that need updates:

- `README.md`: Download, Quick Start, Build, Browser Dev Mode, CI, and Release Notes sections are AppImage-first or AppImage-only.
- `CONTRIBUTING.md`: Build setup, project architecture table, testing guidance, and PR checklist are AppImage/native-build oriented.
- `docs/internal-docs/local-build-publish.md`: Likely needs a full rewrite into Flatpak-only local build, staging, release, and artifact-shape guidance.
- `packaging/flatpak/README.md`: Must stop presenting Flatpak as alongside AppImage and revise migration/shared-mode wording for legacy AppImage data.
- `.github/pull_request_template.md`: Replace native/AppImage checks with Flatpak-only build, metadata validation, and relevant Rust/frontend checks.

## README Files

Required reading:

- `README.md`: Primary user and contributor entry point. Current Download/Quick Start are AppImage-only; Build explains AppImage generation; CI says release builds and uploads AppImage; Release Notes say a single AppImage artifact is published.
- `src/crosshook-native/README.md`: Package-level scripts reference. It is mostly current for frontend tests and dev mode, but still uses `dev-native.sh` naming.
- `packaging/flatpak/README.md`: Flatpak package README and runtime upgrade guide. Required for replacing "alongside AppImage and CLI assets" with Flatpak-only release flow.
- `src/crosshook-native/src/lib/mocks/README.md`: Mock system README. Required if `verify:no-mocks` location changes from AppImage build to Flatpak/release bundle verification.
- `src/crosshook-native/tests/README.md`: Smoke test README. Required if dev command names or parity wording change.
- `docs/research/tauri-webkitgtk-e2e-spike/README.md`: Nice-to-have only unless implementation changes native/Tauri E2E strategy.

Docs that need updates:

- `README.md`: Highest-priority public rewrite.
- `packaging/flatpak/README.md`: Highest-priority packaging rewrite.
- `src/crosshook-native/README.md`: Minor script/name cleanup only if scripts change.
- `src/crosshook-native/src/lib/mocks/README.md`: Update release sentinel references if release workflow changes.

## Must-Read Documents

- `README.md`: Establishes current public AppImage promise and all user-facing wording to remove.
- `CONTRIBUTING.md`: Establishes contributor build/test checklist and AppImage assumptions.
- `packaging/flatpak/README.md`: Establishes current Flatpak build and runtime upgrade process.
- `.github/workflows/release.yml`: Establishes current AppImage/native/Flatpak release dependency graph and artifact list.
- `.github/pull_request_template.md`: Establishes contributor validation expectations that will become wrong after AppImage removal.
- `docs/internal-docs/local-build-publish.md`: Establishes internal local build/release workflow and AppImage artifact naming.
- `docs/architecture/adr-0001-platform-host-gateway.md`: Required to preserve Flatpak host execution correctly.
- `docs/architecture/adr-0002-flatpak-portal-contracts.md`: Required to preserve portal behavior correctly.
- `docs/architecture/adr-0004-flatpak-per-app-isolation.md`: Required to update AppImage legacy migration wording without breaking the data model.
- `docs/prps/prds/flatpak-distribution.prd.md`: Source-of-truth planning doc for Flatpak distribution; currently says AppImage remains primary, so it must be reconciled with `remove-native`.

## Documentation Gaps

- There is no current Flatpak-only release guide. `docs/internal-docs/local-build-publish.md` is AppImage-centered, and `packaging/flatpak/README.md` still assumes AppImage/native staging assets feed Flatpak.
- There is no explicit "legacy AppImage user migration after AppImage removal" user-facing doc. ADR-0004 explains first-run import technically, but README/Quickstart should explain what users should expect.
- The Flatpak distribution PRD/spec are stale for this feature: they describe Flatpak as secondary and AppImage as primary. The spec also admits several sections drift from code.
- Release artifact documentation is inconsistent with current workflow and future goal: README says a single AppImage, `packaging/flatpak/README.md` says Flatpak alongside AppImage and CLI, and `.github/workflows/release.yml` currently publishes AppImage, CLI tarball, and Flatpak.
- Contributor guidance is split and stale: `CONTRIBUTING.md` says there is no frontend test framework, while `docs/TESTING.md` and `src/crosshook-native/README.md` document Vitest and Playwright.
- Agent/runtime rules duplicate AppImage wording across `CLAUDE.md`, `AGENTS.md`, `.ai/rules/project.md`, and `.cursor/rules/project.mdc`. Because `CLAUDE.md` is the source of truth, update it first, then sync the generated/copy surfaces according to repo policy.
- The term "native" is overloaded: it means native Linux app/runtime, native launch mode, native Tauri dev, and AppImage/native distribution scripts. The implementation plan should reserve "native" for runtime/launch semantics and remove it from build/distribution naming where it means AppImage.
