# Research: Flatpak-Only Development Workflow Feasibility

**Date**: 2026-04-12
**Scope**: Feasibility and best practices for Flatpak-only development workflows for Tauri v2 desktop apps on Linux. Evaluates trade-offs of dropping AppImage in favor of Flatpak-only distribution for CrossHook.

---

## Codebase Discovery

### Existing Flatpak Implementation State

CrossHook has completed Flatpak Phases 1-3 and is preparing for Phase 4 (Flathub submission). The codebase already has a dual-format (AppImage + Flatpak) release pipeline.

| Category         | File:Lines                                                                            | Pattern                                                       | Key Detail                                                                                                   |
| ---------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Flatpak PRD      | `docs/prps/prds/flatpak-distribution.prd.md:1-550`                                    | 4-phase rollout (Build, CI, Process Hardening, Flathub)       | Phase 3 complete; Phase 4 (Flathub) not started                                                              |
| Flatpak Spec     | `docs/prps/specs/flatpak-distribution-spec.md:1-461`                                  | Parallel spec with drift warnings vs PRD                      | Notes Flatpak is "secondary target"; AppImage remains primary                                                |
| Phase 3 Plan     | `docs/prps/plans/completed/flatpak-phase-3-process-execution-hardening.plan.md:1-416` | 8-task plan covering host wrappers, UI badges, helper scripts | Completed; 802 unit tests pass; manual Flatpak matrix deferred                                               |
| Phase 3 Rpt      | `docs/prps/reports/flatpak-phase-3-process-execution-hardening-report.md:1-69`        | 29 files changed across core, tauri, UI, scripts              | All 7 code tasks complete; Task 8 (manual Flatpak matrix) not run in CI                                      |
| Manifest         | `packaging/flatpak/dev.crosshook.CrossHook.yml:1-81`                                  | Pre-built binary `simple` buildsystem; GNOME 50               | Phase 1 approach: stages binary into Flatpak, no build-from-source                                           |
| CI Release       | `.github/workflows/release.yml:1-301`                                                 | 3-job pipeline: build-native -> build-flatpak -> publish      | AppImage + Flatpak built in parallel; both uploaded to GitHub Releases                                       |
| Build Script     | `scripts/build-flatpak.sh:1-297`                                                      | Stages binary + assets, runs flatpak-builder, produces bundle | Supports `--rebuild`, `--strict`, `--install-deps`, `--install`                                              |
| Platform         | `src/crosshook-native/crates/crosshook-core/src/platform.rs:1-1231`                   | 1230-line module: detection, host wrappers, XDG override      | `is_flatpak()`, `host_command()`, `host_command_with_env()`, sync/async variants, 20+ unit tests             |
| Tauri Config     | `src/crosshook-native/src-tauri/tauri.conf.json:32`                                   | `"targets": ["appimage"]`                                     | Bundle target is AppImage-only; Flatpak built separately via `scripts/build-flatpak.sh`                      |
| AppImage Re-exec | `src/crosshook-native/src-tauri/src/lib.rs:35-77`                                     | AppImage-specific GPU compat: re-exec with system WebKitGTK   | Checks `APPIMAGE` env var; system WebKitGTK preference for Intel+NVIDIA hybrid                               |
| Desktop Gen      | `scripts/generate-crosshook-desktop.sh:11-62`                                         | Generates desktop entry from AppImage; extracts icon          | Tightly coupled to AppImage as the launch target                                                             |
| Build Native     | `scripts/build-native.sh:10-204`                                                      | Produces AppImage via Tauri bundler + binary export           | `--binary-only` skips AppImage bundling (used by Flatpak CI job)                                             |
| README           | `packaging/flatpak/README.md:1-68`                                                    | GNOME runtime upgrade path + local prereqs                    | Documents `flatpak-builder`, `--strict` validation, smoke test flow                                          |
| XDG Override     | `src/crosshook-native/crates/crosshook-core/src/platform.rs:172-210`                  | Phase 1 shares XDG state between AppImage and Flatpak         | `override_xdg_for_flatpak_host_access()` rewrites XDG vars to host defaults; Phase 4 replaces with isolation |

### Traces

**Entry Points**: The release pipeline (`release.yml:195-246`) triggers `build-flatpak` as a separate CI job that depends on `build-native`. The build-native job exports the binary to `dist/crosshook-native` (line 118-123), which the Flatpak job consumes via artifact upload/download. Locally, `scripts/build-flatpak.sh` orchestrates the same staging + flatpak-builder flow.

**Data Flow**: The Tauri bundler produces an AppImage (via `tauri.conf.json` target `appimage`). The Flatpak flow bypasses the Tauri bundler entirely: it takes the raw release binary from `$CARGO_TARGET_DIR` and manually stages it with helper scripts, icons, desktop entry, and metainfo into a flatpak-builder staging directory. The Flatpak manifest then installs these into `/app/`.

**State Changes**: `platform.rs:204-210` mutates process env vars (`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`) at startup in Flatpak mode. This is the critical shared-state bridge between AppImage and Flatpak data paths. Removing AppImage would remove the need for this bridge (Phase 4 per-app isolation becomes the only model).

**Contracts**: The `lib.rs:49` AppImage GPU re-exec path checks `std::env::var_os("APPIMAGE")` -- this code path is AppImage-specific and would become dead code if AppImage were dropped. The `is_flatpak()` detection (`platform.rs:31-33`) uses `FLATPAK_ID` and `/.flatpak-info`, which are Flatpak-runtime-provided signals unrelated to AppImage.

**Patterns**: Dual-distribution architecture -- the native build produces both a Tauri-bundled AppImage and a raw binary; the Flatpak packaging consumes only the raw binary. This pattern means the Flatpak build is already independent of AppImage bundling.

### AppImage-Specific Code Inventory

Code paths that exist solely for AppImage or reference AppImage directly:

| File                                    | Lines   | Purpose                                                                             | Impact if AppImage Dropped                                           |
| --------------------------------------- | ------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `src-tauri/src/lib.rs`                  | 35-77   | AppImage GPU compat re-exec (checks `APPIMAGE` env, re-execs with system WebKitGTK) | Dead code; removable                                                 |
| `src-tauri/tauri.conf.json`             | 32      | `"targets": ["appimage"]`                                                           | Would change or be removed                                           |
| `scripts/build-native.sh`               | 16-204  | AppImage naming, searching, copying after Tauri build                               | Major refactor needed; Flatpak build would become the primary output |
| `scripts/generate-crosshook-desktop.sh` | 11-62   | Generates desktop entry from AppImage, extracts icon                                | Obsolete; Flatpak has its own committed desktop entry                |
| `scripts/install-native-build-deps.sh`  | 10      | Installs AppImage build deps (patchelf, etc.)                                       | Would slim down to Flatpak toolchain only                            |
| `release.yml`                           | 111-161 | AppImage build, mock verification, CLI packaging                                    | AppImage artifact upload removed; CLI tarball likely stays           |
| `release.yml`                           | 287-290 | AppImage listed in release asset upload                                             | Remove AppImage line                                                 |
| `platform.rs`                           | 172-210 | XDG override to share data between AppImage and Flatpak                             | Phase 4 per-app isolation replaces this; bridge no longer needed     |
| `crosshook-cli/src/args.rs`             | 72      | "bundled AppImage scripts" help text                                                | Cosmetic update                                                      |

### Gaps

- GAP: No existing Phase 4 plan document. Phase 4 (Flathub submission) is described only as a section in the PRD (`flatpak-distribution.prd.md` section 5.4 and section 11 Phase 4) -- no standalone plan has been written.
- GAP: No performance benchmarks comparing AppImage vs Flatpak launch time, startup overhead, or `flatpak-spawn --host` latency for the 12 external binary calls.
- GAP: No download telemetry or analytics to measure current AppImage vs Flatpak adoption split. The PRD hypothesizes 10%+ Flatpak downloads within two release cycles but has no baseline data.
- GAP: The Phase 3 manual Flatpak verification matrix (Task 8) has not been executed. The report states "Manual Task 8 checklist deferred to host environment."
- GAP: No automated integration test framework for Flatpak. All Flatpak verification is manual per the plan and report.
- GAP: No build-from-source Flatpak manifest exists. Current Phase 1 manifest uses pre-built binary. Phase 4 Flathub submission requires `flatpak-cargo-generator.py` + `flatpak-node-generator` for offline builds -- this work has not started.

---

## External Findings

### Market Context

| Source                | Approach                                                                                             | Strengths                                                            | Weaknesses                                                                               | URL                                                                                                                                 |
| --------------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Bottles               | Flatpak-only distribution; zero filesystem perms (portals only)                                      | Maximum sandboxing; clean Flathub standing; GNOME Adwaita native     | Requires reimplementing all file access via portals; reduced flexibility for power users | [Bottles Flathub](https://flathub.org/apps/com.usebottles.bottles)                                                                  |
| Lutris                | Dual format (Flatpak + native packages); `--filesystem=home` + `--talk-name=org.freedesktop.Flatpak` | Broadest Linux game tool with Flathub acceptance despite broad perms | Still ships native deb/rpm alongside Flatpak; complex permission model                   | [Lutris Flathub manifest](https://github.com/flathub/net.lutris.Lutris/blob/beta/net.lutris.Lutris.yml)                             |
| Heroic Games Launcher | Multi-format (AppImage, Flatpak, deb, rpm); Electron-based                                           | Developer stated multi-format was "best decision" for reach          | Must maintain all packaging paths; tighter Flatpak perms because Wine is bundled         | [Heroic GamingOnLinux Interview](https://www.gamingonlinux.com/2023/01/an-interview-with-the-creator-of-the-heroic-games-launcher/) |
| Pomodorolm            | Flatpak + Snap; Tauri v2; only confirmed Tauri v2 Flathub app                                        | Documented full submission experience; "pretty serious" review       | Required build-from-source manifest with offline dep generators for Rust, npm, and Elm   | [Pomodorolm blog](https://vincent.jousse.org/blog/en/packaging-tauri-v2-flatpak-snapcraft-elm/)                                     |
| DuckStation           | Dropped AppImage in favor of Flatpak                                                                 | Simplified maintenance; better user experience on immutable distros  | Lost users who preferred single-file portability                                         | [DuckStation AppImage vs Flatpak](https://pulsegeek.com/articles/duckstation-appimage-vs-flatpak-on-linux/)                         |

### Technical Insights

**KEY_INSIGHT**: Flathub requires build-from-source manifests for submission. CrossHook's current Phase 1 manifest uses a pre-built binary (`buildsystem: simple` with `type: dir`). Phase 4 / Flathub submission requires `flatpak-cargo-generator.py` for Rust offline deps and `flatpak-node-generator` for npm offline deps. The Pomodorolm developer characterized this process as "quite a challenge."
**APPLIES_TO**: Phase 4 Flathub submission; any Flatpak-only workflow targeting Flathub
**GOTCHA**: The build-from-source manifest is a separate, more complex artifact from the Phase 1 manifest. Maintaining both is necessary until Flathub submission completes.
**SOURCE**: [Pomodorolm Flatpak blog](https://vincent.jousse.org/blog/en/packaging-tauri-v2-flatpak-snapcraft-elm/)

**KEY_INSIGHT**: The Tauri v2 official Flatpak guide recommends using GNOME runtime 46 and building from a `.deb` package. CrossHook uses GNOME 50 and builds from a pre-built binary. The `.deb` approach is an alternative that may be simpler for Flathub but adds a `.deb` packaging step.
**APPLIES_TO**: Flatpak manifest strategy and Flathub submission
**GOTCHA**: The official Tauri docs say GNOME 46 includes "all dependencies of the standard Tauri app with their correct versions." CrossHook uses GNOME 50 which is newer; compatibility is confirmed by working CI builds.
**SOURCE**: [Tauri v2 Flatpak docs](https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/distribute/flatpak.mdx)

**KEY_INSIGHT**: `flatpak-builder` caches build results per module and rebuilds from the first changed module forward. For CrossHook's `simple` buildsystem with a pre-built binary, the cache is trivial -- the entire build is one module that always rebuilds. The cache is meaningful only for build-from-source manifests with multiple dependency modules.
**APPLIES_TO**: Development iteration speed with flatpak-builder
**GOTCHA**: Cache invalidation is documented as "unpredictable" in some cases. Adding a post-install command to the last module can cause all modules to be rebuilt. SDK updates can also invalidate the entire cache unless `--rebuild-on-sdk-change` is used.
**SOURCE**: [flatpak-builder cache issue](https://github.com/flatpak/flatpak-builder/issues/19), [Flatpak Builder docs](https://docs.flatpak.org/en/latest/flatpak-builder-command-reference.html)

**KEY_INSIGHT**: `flatpak-builder --run build-dir manifest.yml <command>` runs a command inside the build environment with the manifest's `finish-args` permissions applied (except filesystem permissions). This allows testing without exporting/installing. Additionally, `flatpak build appdir bash` gives a shell in the build directory for quick iteration.
**APPLIES_TO**: Development iteration and debugging inside the Flatpak sandbox
**GOTCHA**: The `--run` environment is a "build" environment, so the app may not have access to all permissions it requests. Filesystem permissions are notably excluded. For full permission testing, install and `flatpak run` is required.
**SOURCE**: [flatpak wiki Tips & Tricks](https://github.com/flatpak/flatpak/wiki/Tips-&-Tricks)

**KEY_INSIGHT**: AppImage requires FUSE 2 to run. Newer distros ship FUSE 3. Immutable distros (Fedora Silverblue, SteamOS/Bazzite) often lack FUSE 2 entirely. The FUSE 2-to-3 migration is an ongoing unresolved ecosystem problem. Workarounds (`--appimage-extract-and-run`, `APPIMAGE_EXTRACT_AND_RUN=1`) exist but add friction.
**APPLIES_TO**: Decision to drop AppImage; risk assessment for immutable distro users
**GOTCHA**: CrossHook already sets `APPIMAGE_EXTRACT_AND_RUN=1` in `scripts/build-native.sh:10` -- this is a workaround for the FUSE problem during build, not for end-user execution.
**SOURCE**: [AppImage FUSE docs](https://docs.appimage.org/user-guide/troubleshooting/fuse.html), [Fedora Silverblue AppImage issue](https://discussion.fedoraproject.org/t/appimage-wont-open-silverblue-41-beta/131648)

**KEY_INSIGHT**: Flatpak debugging requires the SDK (installed via `--devel` flag) for GDB, strace, and Valgrind. GDB inside the sandbox cannot use debug symbols from the host's debuginfod servers. `coredumpctl` is not Flatpak-aware. This is a meaningful regression from AppImage/native debugging where all host tools work directly.
**APPLIES_TO**: Developer workflow; debugging production issues reported by users
**GOTCHA**: `flatpak run --command=sh --devel` gives a debug shell, but debug packages (`.Debug` extensions for the runtime) must be installed separately. Core dump analysis requires manual extraction from sandbox namespaces.
**SOURCE**: [Flatpak debugging docs](https://docs.flatpak.org/en/latest/debugging.html)

**KEY_INSIGHT**: `flatpak-spawn --host` routes commands through D-Bus to the Flatpak session helper, adding ~140ms per call (documented in CrossHook PRD). Environment variables set via `.env()` / `.envs()` on the `Command` object are silently dropped by `flatpak-spawn --host` -- they must be passed as `--env=KEY=VALUE` arguments. CrossHook already handles this in `platform.rs` via `host_command_with_env()`.
**APPLIES_TO**: Runtime overhead of Flatpak-only distribution; correctness of process execution
**GOTCHA**: The Zed editor's Flatpak build (April 2026) experienced a regression where host shell environment was no longer imported correctly after an update, despite `flatpak-spawn --host` working for direct commands. Host environment inheritance is fragile.
**SOURCE**: [flatpak-spawn man page](https://man7.org/linux/man-pages/man1/flatpak-spawn.1.html), [Zed Flatpak issue](https://github.com/zed-industries/zed/issues/53238)

**KEY_INSIGHT**: Flatpak development has restarted after a stagnation period. Sebastian Wick (Red Hat) acknowledged Flatpak "reached a stagnant phase" in early 2025 but confirmed renewed development activity with new maintainers. Key new features include OCI pre-install support, `systemd-appd` for nested sandboxing, and XDG Intents for inter-app communication.
**APPLIES_TO**: Long-term viability of Flatpak as sole distribution format
**GOTCHA**: The stagnation raised ecosystem concerns. The project is recovering but the governance situation should be monitored.
**SOURCE**: [Flatpak development restarts](https://linuxiac.com/flatpak-development-restarts-with-fresh-energy-and-clear-direction/)

### Trends & Shifts

- **Immutable distro growth strongly favors Flatpak.** Fedora Silverblue, SteamOS/Bazzite, Universal Blue (Aurora, Bluefin), and Vanilla OS all use Flatpak as the primary or sole app installation mechanism. Valve's SteamOS influence means "most people's first experience with Linux will be Valve's or a spinoff" -- these users need Flatpak. Evidenced by [Fedora Silverblue is the future](https://www.xda-developers.com/fedora-silverblue-future-of-desktop-linux/), [Hacker News immutable discussion](https://news.ycombinator.com/item?id=44069860).

- **AppImage FUSE 2 dependency is an increasing liability.** Ubuntu 24.04+, Fedora 43+, and immutable distros are moving to FUSE 3. AppImage's FUSE 2 requirement creates user-facing friction that is not getting resolved. The AppImageKit FUSE 3 migration issue (#1120) has been open since 2021 with no resolution. Evidenced by [AppImageKit FUSE3 issue](https://github.com/AppImage/AppImageKit/issues/1120).

- **Tauri v2 Flatpak ecosystem is maturing but shallow.** Only one confirmed Tauri v2 app (Pomodorolm) has completed Flathub submission. The official Tauri docs now cover Flatpak distribution. The community discussion thread on Flathub Discourse shows early-stage engagement but no consensus on best practices. Evidenced by [Tauri Flathub discussion](https://discourse.flathub.org/t/help-tauri-implement-flatpak-support/5993).

- **Some projects are dropping AppImage in favor of Flatpak.** DuckStation is a notable example. Others (like Heroic) explicitly maintain multi-format distribution because it maximizes reach. The Heroic developer stated that multi-format support was "the best decision" for the project. Evidenced by [DuckStation comparison](https://pulsegeek.com/articles/duckstation-appimage-vs-flatpak-on-linux/), [Heroic interview](https://www.gamingonlinux.com/2023/01/an-interview-with-the-creator-of-the-heroic-games-launcher/).

- **Flatpak project governance recovered after 2025 stagnation scare.** Development is active again with new maintainers and a clear direction. RHEL 10 integration (via OCI pre-install) signals enterprise commitment. Evidenced by [Flatpak development restarts](https://linuxiac.com/flatpak-development-restarts-with-fresh-energy-and-clear-direction/).

---

## Trade-Off Analysis: Flatpak-Only vs AppImage+Flatpak

### What CrossHook Gains by Dropping AppImage

| Gain                           | Detail                                                                                                                                       |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Simplified build pipeline      | One packaging format instead of two; `release.yml` drops from 3 jobs to 2 (build + publish)                                                  |
| Removed AppImage-specific code | `lib.rs:35-77` GPU re-exec hack, `generate-crosshook-desktop.sh`, AppImage search/copy logic in `build-native.sh`                            |
| XDG override elimination       | `platform.rs:172-210` shared-state bridge between AppImage and Flatpak becomes unnecessary; Phase 4 per-app isolation becomes the only model |
| No FUSE dependency             | Eliminates the FUSE 2 compatibility issue entirely                                                                                           |
| Single data path               | No risk of two instances (AppImage + Flatpak) writing to the same config/DB concurrently                                                     |
| Faster Phase 4                 | Per-app isolation and Flathub submission become simpler without the AppImage compatibility constraint                                        |

### What CrossHook Loses by Dropping AppImage

| Loss                      | Detail                                                                                                                    |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Single-file portability   | AppImage is a single executable; Flatpak requires the Flatpak runtime installed                                           |
| Users without Flatpak     | Some distros do not ship Flatpak by default (Arch, Gentoo, some minimal installs); these users must install Flatpak first |
| Direct debugging ease     | GDB, strace, coredumpctl work directly on AppImage; Flatpak requires SDK, `.Debug` extensions, `--devel` mode             |
| Developer iteration speed | `cargo build --release` + run directly is faster than `build-flatpak.sh` staging + flatpak-builder + install              |
| Tauri bundler features    | AppImage is a first-class Tauri bundle target with auto-update support; Flatpak is custom build infrastructure            |
| Unknown user base impact  | No download telemetry to know what percentage of current users use AppImage vs Flatpak                                    |

### What Stays the Same

| Aspect                  | Detail                                                                                           |
| ----------------------- | ------------------------------------------------------------------------------------------------ |
| Native dev workflow     | `./scripts/dev-native.sh` runs the Tauri dev server directly, not via Flatpak; this is unchanged |
| Browser dev mode        | `./scripts/dev-native.sh --browser` is Flatpak-agnostic                                          |
| `cargo test`            | Unit tests run outside any sandbox; unchanged                                                    |
| CI build time (roughly) | AppImage build time is similar to binary-only build + Flatpak packaging time                     |
| CLI binary distribution | `crosshook` CLI is a separate Cargo artifact; distributed as tarball regardless of GUI packaging |

---

## Development Workflow Specifics

### Current Dev Workflow (No Flatpak Involvement)

1. `./scripts/dev-native.sh` -- Tauri dev server with hot-reload for frontend, full Rust rebuild for backend changes
2. `./scripts/dev-native.sh --browser` -- Browser-only dev with mock IPC, no Rust toolchain needed
3. `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` -- Unit tests
4. None of these steps involve Flatpak

### Flatpak Testing Workflow (For Sandbox-Specific Behavior)

1. `./scripts/build-flatpak.sh` -- Builds the Flatpak bundle (requires prior `build-native.sh --binary-only` or uses `--rebuild`)
2. `flatpak install --user --reinstall dist/CrossHook_amd64.flatpak` -- Install locally
3. `flatpak run dev.crosshook.CrossHook` -- Run in sandbox
4. `flatpak run --command=bash dev.crosshook.CrossHook` -- Debug shell inside sandbox
5. Alternative: `flatpak-builder --run <build-dir> <manifest> bash` -- Shell with manifest permissions (except filesystem)

### What a "Flatpak-Only Dev Workflow" Means

Dropping AppImage does not mean developing inside a Flatpak sandbox. It means:

- The **release artifact** is a `.flatpak` bundle only (no `.AppImage`)
- The **CI pipeline** produces only Flatpak + CLI tarball
- The **daily dev workflow** (`dev-native.sh`, `cargo test`) is unchanged -- these run natively
- Sandbox-specific testing (`host_command()`, `flatpak-spawn --host`) still requires building and installing the Flatpak bundle, as it does today

### flatpak-builder Iteration Tools

| Tool                                                 | What It Does                                                   | Limitation                                                             |
| ---------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `flatpak-builder --run build-dir manifest.yml <cmd>` | Runs a command inside build env with `finish-args` permissions | Filesystem permissions not applied; not identical to installed Flatpak |
| `flatpak build appdir bash`                          | Opens shell in build directory without export/install          | "Build" environment, not "run" environment; permissions differ         |
| `flatpak-builder --keep-build-dirs`                  | Preserves intermediate build dirs for debugging                | Increases disk usage                                                   |
| `flatpak-builder --ccache`                           | Enables ccache for C/C++ builds (needs ccache in SDK)          | Not applicable to Rust/cargo builds                                    |
| `flatpak-builder --force-clean`                      | Forces clean rebuild; ignores cache                            | Slower; used when cache is suspect                                     |
| Cache behavior                                       | Caches per-module results; rebuilds from first changed module  | Known unpredictable invalidation; SDK updates can flush entire cache   |

---

## Gaps

- GAP: No download telemetry to measure AppImage vs Flatpak adoption. The decision to drop AppImage would be made without data on how many users rely on each format.
- GAP: No Phase 4 plan document exists. Dropping AppImage accelerates the need for Phase 4 (Flathub becomes the sole discovery/install path for non-CLI users), but the plan has not been written.
- GAP: No build-from-source Flatpak manifest. Flathub requires this. The current pre-built-binary manifest is insufficient for Flathub submission. The effort to create offline dep generators (`flatpak-cargo-generator.py`, `flatpak-node-generator`) is undocumented and unestimated.
- GAP: No benchmarks for `flatpak-spawn --host` overhead across the 12 external binary calls. The PRD estimates ~140ms per call but no measurements exist.
- GAP: The Phase 3 manual verification matrix has not been executed. The full Flatpak functional test suite is untested in a real sandbox.
- GAP: No investigation of Flatpak auto-update mechanisms. AppImage has no built-in update; Flatpak has differential updates when installed from a repository. But CrossHook distributes via GitHub Releases bundles, not a Flatpak repo -- so Flatpak's update advantage does not apply until Flathub submission.
- GAP: No assessment of whether Flatpak-only distribution affects the Steam Deck use case differently from desktop Linux. SteamOS uses Flatpak, but Steam Deck users may have specific constraints (controller-only, limited storage) not evaluated.
- GAP: Could not find authoritative data on what percentage of Linux desktop users have Flatpak available vs what percentage can run AppImages.

---

## Sources

### Internal Documents

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/prps/prds/flatpak-distribution.prd.md`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/prps/specs/flatpak-distribution-spec.md`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/prps/plans/completed/flatpak-phase-3-process-execution-hardening.plan.md`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/prps/reports/flatpak-phase-3-process-execution-hardening-report.md`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/packaging/flatpak/dev.crosshook.CrossHook.yml`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/packaging/flatpak/README.md`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/workflows/release.yml`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/scripts/build-flatpak.sh`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/platform.rs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/tauri.conf.json`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/scripts/build-native.sh`

### External Sources

- [Snap vs Flatpak vs AppImage Comparison (2026)](https://computingforgeeks.com/snap-vs-flatpak-vs-appimage/)
- [Pomodorolm Tauri v2 Flatpak Packaging Blog](https://vincent.jousse.org/blog/en/packaging-tauri-v2-flatpak-snapcraft-elm/)
- [Tauri v2 Flatpak Distribution Guide](https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/distribute/flatpak.mdx)
- [Flatpak Builder Command Reference](https://docs.flatpak.org/en/latest/flatpak-builder-command-reference.html)
- [flatpak-builder Cache Invalidation Issue](https://github.com/flatpak/flatpak-builder/issues/19)
- [Flatpak Tips & Tricks Wiki](https://github.com/flatpak/flatpak/wiki/Tips-&-Tricks)
- [Flatpak Debugging Documentation](https://docs.flatpak.org/en/latest/debugging.html)
- [flatpak-spawn Man Page](https://man7.org/linux/man-pages/man1/flatpak-spawn.1.html)
- [Zed Editor Flatpak Shell Issue (April 2026)](https://github.com/zed-industries/zed/issues/53238)
- [AppImage FUSE Documentation](https://docs.appimage.org/user-guide/troubleshooting/fuse.html)
- [AppImageKit FUSE 3 Issue (#1120)](https://github.com/AppImage/AppImageKit/issues/1120)
- [Fedora Silverblue AppImage FUSE Issue](https://discussion.fedoraproject.org/t/appimage-wont-open-silverblue-41-beta/131648)
- [Fedora F43 fuse-libs Default Issue](https://discussion.fedoraproject.org/t/f43-beta-fuse-libs-not-installed-by-default/164719)
- [DuckStation AppImage vs Flatpak](https://pulsegeek.com/articles/duckstation-appimage-vs-flatpak-on-linux/)
- [Heroic Games Launcher Creator Interview](https://www.gamingonlinux.com/2023/01/an-interview-with-the-creator-of-the-heroic-games-launcher/)
- [Flathub Tauri Support Discussion](https://discourse.flathub.org/t/help-tauri-implement-flatpak-support/5993)
- [Flatpak Development Restarts (2025)](https://linuxiac.com/flatpak-development-restarts-with-fresh-energy-and-clear-direction/)
- [Why AppImages Should Stop (TechRepublic)](https://www.techrepublic.com/article/why-i-have-a-problem-with-appimages-on-linux/)
- [dont-use-appimages (GitHub repo)](https://github.com/boredsquirrel/dont-use-appimages)
- [Why I Shipped as AppImage (dev.to, 2026)](https://dev.to/jamie_folsom_fc88c37582d8/why-i-shipped-a-linux-desktop-app-as-an-appimage-and-skipped-snapflatpak-53kb)
- [Linux Journal: Future of Flatpak and Snap](https://www.linuxjournal.com/content/future-linux-software-will-flatpak-and-snap-replace-native-desktop-apps)
- [Fedora Silverblue as Future of Desktop Linux](https://www.xda-developers.com/fedora-silverblue-future-of-desktop-linux/)
- [Rust Flatpak Recompile Issue (Flathub Discourse)](https://discourse.flathub.org/t/rust-not-recompile-the-project-every-time-flatpak-builder-is-ran/3504)
- [Tauri v2 Hot Reload Discussion (#11732)](https://github.com/tauri-apps/tauri/discussions/11732)
- [Flatpak as Developer Platform](https://docs.flatpak.org/en/latest/flatpak-devel.html)
