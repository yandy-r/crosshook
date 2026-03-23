# Todo

## 2026-03-23

### Goal

Implement `temp/non-steam-runner-plan.md` so the native app supports explicit `steam_applaunch`, `proton_run`, and `native` runner modes without regressing Steam-backed profiles or legacy `.profile` import.

### Plan

- [x] Capture the current runtime/profile and launch architecture, then verify the implementation batches against the plan constraints.
- [x] Implement the core profile/runtime contract in Rust and TypeScript, including `[runtime]` TOML support, explicit launch methods, and save-default normalization.
- [x] Implement method-specific launch validation and the non-Steam Proton backend launch path while preserving the current Steam launch flow.
- [x] Update the profile editor, launch panel, and app integration so each method exposes only the relevant fields and sends the correct launch request data.
- [x] Run targeted native verification (`cargo test -p crosshook-core`, `cargo check -p crosshook-native`, `npm run build`) and document the final outcome plus any residual gaps.

### Review

- Added explicit runtime persistence to the native profile model in `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` and `src/crosshook-native/src/types/profile.ts`, then updated `src/crosshook-native/src/hooks/useProfile.ts` to normalize legacy or blank launch methods into `steam_applaunch`, `proton_run`, or `native` without breaking missing-`[runtime]` TOML or legacy `.profile` imports.
- Replaced the Steam-only launch contract with a method-aware request model in `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`, including method-specific validation for Steam app launch, non-Steam Proton launch, and Linux-native launch plus targeted regression tests for the new branches.
- Extended `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` and `src/crosshook-native/src-tauri/src/commands/launch.rs` so Steam launches still use the existing helper scripts, while `proton_run` now launches the game directly with Proton and stages trainer files into the configured prefix before trainer launch. Native launch now runs the selected Linux executable directly and rejects Windows `.exe` paths.
- Updated `src/crosshook-native/src/components/ProfileEditor.tsx`, `src/crosshook-native/src/hooks/useLaunchState.ts`, `src/crosshook-native/src/components/LaunchPanel.tsx`, and `src/crosshook-native/src/App.tsx` so the UI is driven by an explicit runner selector instead of `steam.enabled`. Steam-only launcher/export controls now stay scoped to Steam mode, Proton mode exposes prefix/Proton/working-directory fields, and native mode avoids Steam/Proton copy.
- Verification:
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-core-test-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-check-target cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native`
- `env npm_config_cache=/tmp/npm-crosshook-native-cache npm_config_update_notifier=false npm run build` in `src/crosshook-native`
- Residual gap:
- This pass is compiler/test verified, but the new `proton_run` and `native` branches still need live manual validation against real Steam-backed, non-Steam Proton, and Linux-native profiles.

## 2026-03-23

### Goal

Implement Phase 4 of the `platform-native-ui` plan on top of the validated native workspace, focusing on the community profile schema, exchange flow, taps backend, and initial community UI surfaces.

### Plan

- [x] Validate `platform-native-ui` prerequisites and confirm the current native workspace still passes `cargo check` and `npm run build`.
- [x] Implement Task 4.1 community profile schema and the shared JSON schema artifact.
- [x] Implement Task 4.2 profile import/export exchange between community JSON and local TOML.
- [x] Implement the Rust phase 4 backend tasks: git-based taps/indexing.
- [x] Implement the Tauri community IPC layer and the initial React community browser/compatibility viewer surfaces.
- [x] Implement Task 4.6 trainer compatibility database viewer.
- [x] Integrate the phase 4 modules into the native app shell and run targeted native verification.

### Review

- Added the phase 4 community data model in `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` plus `schemas/community-profile.json`, then wired `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs` into `crosshook-core` so community manifests can be imported into and exported from the TOML profile store.
- Added the taps backend under `src/crosshook-native/crates/crosshook-core/src/community/` with git-backed tap sync, manifest indexing, and typed tap/index results. Settings persistence now includes `community_taps` so subscriptions are stored alongside the rest of the native app settings.
- Added `src/crosshook-native/src-tauri/src/commands/community.rs` and registered the community commands in the Tauri app. The command layer now wraps the shared Rust backend rather than duplicating community parsing/indexing logic locally.
- Added the initial community UI surfaces in `src/crosshook-native/src/components/CommunityBrowser.tsx`, `src/crosshook-native/src/hooks/useCommunityProfiles.ts`, and `src/crosshook-native/src/components/CompatibilityViewer.tsx`, then integrated them into `src/crosshook-native/src/App.tsx` so the browser and compatibility view share one backend-driven community state.
- Verification:
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo check --manifest-path src/crosshook-native/Cargo.toml`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- `env npm_config_cache=/tmp/npm-crosshook-native-cache npm run build`
- Residual note:
- One community-command worker stalled and a browser worker exhausted its context window. The final integrated implementation incorporates their useful file outputs where appropriate, but the completed phase-4 result comes from the validated repository state above rather than from raw worker output alone.

## 2026-03-23

### Goal

Implement Phase 3 of the `platform-native-ui` plan only after validating that the native workspace from phases 1 and 2 is present and passing its current build/test gates.

### Plan

- [x] Validate the `platform-native-ui` planning prerequisites and confirm the phase 1-2 native workspace artifacts exist.
- [x] Re-run the native workspace verification gates (`cargo check`, `cargo test -p crosshook-core`, `npm run build`) before unlocking phase 3.
- [x] Implement Task 3.12 structured logging in `crosshook-core`.
- [x] Implement Phase 3 settings persistence (Rust) in `crosshook-core`.
- [x] Implement Phase 3 recent files tracking (Rust) in `crosshook-core`.
- [x] Implement Phase 3 CLI argument parsing in `crosshook-cli`.
- [x] Implement Task 3.3 auto-load profile startup helper.
- [x] Implement Task 3.10 native AppImage CI/CD integration in `.github/workflows/native-build.yml`.
- [x] Implement Task 3.5 React settings panel in `src/crosshook-native/src/components/SettingsPanel.tsx`.
- [x] Implement Task 3.4 settings IPC wrappers in `src/crosshook-native/src-tauri/src/commands/settings.rs`.
- [x] Implement Task 3.7 controller/gamepad navigation in `src/crosshook-native/src/hooks/useGamepadNav.ts` and `src/crosshook-native/src/styles/focus.css`.
- [x] Implement the remaining Phase 3 core tasks: theme assets and packaging scaffolding.
- [x] Wire the dependent Phase 3 integration tasks: startup auto-load, settings IPC, and settings UI.
- [x] Add the remaining distribution artifacts for Phase 3 and re-run targeted native verification.

### Review

- Added `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs` with self-contained community profile import/export helpers, explicit serde-friendly error types, schema-version validation, and round-trip tests.
- The exchange module reads and writes `CommunityProfileManifest` JSON, validates required manifest sections before deserializing, rejects future schema versions, exports named local `GameProfile` entries to JSON with derived metadata, and imports community JSON into the TOML profile store using the source file stem as the profile name.
- Added `serde_json` to `crosshook-core` so the module can parse and write community profile manifests.
- Verified with:
- `rustfmt --edition 2021 --check src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-exchange-check-target cargo test --manifest-path /tmp/crosshook-exchange-check/Cargo.toml`
- Added `src/crosshook-native/crates/crosshook-core/src/logging.rs` with a `tracing_subscriber`-based initializer that targets `~/.local/share/crosshook/logs/crosshook.log`, supports optional stdout mirroring, and performs simple size-based rotation without introducing a separate rotation crate.
- Exposed the logging module from `crosshook-core` and added the `tracing-subscriber` dependency with `env-filter`, `fmt`, and `time` features.
- Verified with:
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --lib`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core logging`
- Added a self-contained TOML-backed settings store in `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` with `SettingsStore`, `AppSettingsData`, and typed errors.
- The store resolves `~/.config/crosshook/settings.toml`, creates the parent directory on load/save, returns defaults when the file is missing, and preserves missing-field defaults through serde.
- Verified with:
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core settings`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- Added `src/crosshook-native/crates/crosshook-core/src/settings/recent.rs` with a TOML-backed recent-files store, a `RecentFilesData` model, load-time pruning of missing paths, and a 10-entry cap per list.
- Verified the module in isolation with `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-recent-harness-target cargo test` in a throwaway harness that imports the file by absolute path.
- Added `src/crosshook-native/crates/crosshook-cli/src/args.rs` with a full clap parser for `launch`, `profile`, and `steam` plus global `--verbose`, `--json`, and `--config` flags.
- Kept the existing launch path functional in `src/crosshook-native/crates/crosshook-cli/src/main.rs` and added placeholder handlers for the new `profile` and `steam` subcommands so the binary now accepts the planned CLI surface without extra business logic.
- Added parser tests covering `launch --profile`, `profile import --legacy-path`, and `steam auto-populate --game-path`.
- Verification:
- `rustfmt --edition 2021 --check src/crosshook-native/crates/crosshook-cli/src/args.rs src/crosshook-native/crates/crosshook-cli/src/main.rs`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-cli` blocked by unrelated pre-existing errors in `src/crosshook-native/crates/crosshook-core/src/logging.rs`
- Added `src/crosshook-native/src-tauri/src/startup.rs` with a helper that loads settings, checks the auto-load toggle, validates the last-used profile against the saved profile list, and returns `Option<String>` for the eventual `"auto-load-profile"` event payload.
- Added tests covering auto-load disabled, missing profiles, blank names, and a successful match.
- Added `.github/workflows/native-build.yml` so tag pushes on `v*` run the native Linux toolchain, install Tauri/AppImage prerequisites, test `crosshook-core`, build the AppImage through `scripts/build-native.sh`, and upload the result to the GitHub Release.
- Verified the workflow file syntax with `ruby -e "require 'yaml'; YAML.load_file('.github/workflows/native-build.yml'); puts 'native-build.yml: ok'"`.
- Added `packaging/PKGBUILD` for AUR-style source builds from GitHub releases. The package builds the Tauri app with `cargo tauri build`, installs the native binary to `/usr/bin/crosshook-native`, copies the bundled helper scripts into `/usr/lib/crosshook-native/runtime-helpers/`, and installs the MIT license file.
- Verified the PKGBUILD syntax with `bash -n packaging/PKGBUILD`.
- Added `src/crosshook-native/src/components/SettingsPanel.tsx` as a self-contained prop-driven settings panel for auto-load, profiles-directory messaging, and recent file history display.
- Verified with:
- `env npm_config_cache=/tmp/npm-crosshook-native-cache npm run build`
- Added `src/crosshook-native/src-tauri/src/commands/settings.rs` with `settings_load`, `settings_save`, `recent_files_load`, and `recent_files_save` wrappers over the new Rust stores.
- The command signatures are ready for `State<'_, SettingsStore>` and `State<'_, RecentFilesStore>` injection from `lib.rs`; registration wiring was intentionally left untouched.
- Verified with:
- `rustfmt --edition 2021 --check src/crosshook-native/src-tauri/src/commands/settings.rs`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native`
- Added `src/crosshook-native/src/hooks/useGamepadNav.ts` and `src/crosshook-native/src/styles/focus.css` for Steam Deck auto-detection, focus traversal, confirm/back button mapping, and high-contrast focus rings sized for controller-first layouts.
- Verified with:
- `env npm_config_cache=/tmp/npm-crosshook-native-cache npm run build`
- Wired the Phase 3 pieces into the live native app surface: `src/crosshook-native/src-tauri/src/lib.rs` now initializes logging, manages the settings and recent-files stores, registers the new settings IPC commands, and schedules the startup `"auto-load-profile"` event after the frontend listener can receive it.
- Updated the frontend integration in `src/crosshook-native/src/App.tsx`, `src/crosshook-native/src/main.tsx`, `src/crosshook-native/src/hooks/useProfile.ts`, and `src/crosshook-native/src/types/settings.ts` so the app loads settings/recent-files state, shows the new settings panel, tracks last-used profile and recent paths, imports the new theme/focus styles, and avoids clobbering `last_used_profile` during startup by disabling the old auto-select-first-profile behavior.
- Added the remaining Phase 3 distribution assets: `src/crosshook-native/src-tauri/tauri.conf.json` now bundles AppImage targets, `scripts/build-native.sh` builds and copies the AppImage into `dist/`, `packaging/PKGBUILD` covers AUR packaging, and `.github/workflows/native-build.yml` covers release-tagged native CI.
- Integrated verification:
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo check --manifest-path src/crosshook-native/Cargo.toml`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- `env npm_config_cache=/tmp/npm-crosshook-native-cache npm run build`
- `bash -n scripts/build-native.sh`
- `node -e "JSON.parse(require('fs').readFileSync('src/crosshook-native/src-tauri/tauri.conf.json', 'utf8')); console.log('tauri.conf.json: ok')"`
- `bash -n packaging/PKGBUILD`
- `ruby -e "require 'yaml'; YAML.load_file('.github/workflows/native-build.yml'); puts 'native-build.yml: ok'"`
- Remaining validation gap:
- `git diff --check` still reports whitespace-style issues across the broader native workspace diff, so that repo-level whitespace cleanup remains unresolved.

## 2026-03-22

### Goal

Implement the `platform-native-ui` plan by scaffolding the native Rust/Tauri workspace, porting the MVP launch/profile flows, completing Phase 2 Steam auto-populate and launcher export, and verifying the new native target builds cleanly.

### Plan

- [x] Scaffold `src/crosshook-native/` with the Rust workspace, Tauri app shell, shared types, and bundled helper-script paths.
- [x] Port the Phase 1 profile storage, launch request validation, shell-command construction, Tauri IPC, and initial React profile/launch/log UI.
- [x] Port the full Phase 2 Steam discovery pipeline, auto-populate/export IPC, and related React UI.
- [ ] Add Phase 3 settings/theme/logging/distribution work and the initial Phase 4 community profile foundations if integration stays stable.
- [x] Verify the native workspace with targeted formatting/build/test commands and record the checkpoint review.

### Review

- Added a new native workspace under `src/crosshook-native/` with a Cargo workspace, `crosshook-core`, `crosshook-cli`, a Tauri v2 shell, React/Vite frontend scaffolding, helper-script bundling, and a local `.gitignore` for generated assets.
- Implemented the Phase 1 core path: legacy `.profile` loading, TOML profile storage/import, launch request validation, clean shell-command builders, launch environment constants, profile and launch Tauri IPC commands, the headless CLI launcher, and runtime helper path resolution.
- Implemented the initial native frontend shell: shared TypeScript contracts, profile editing hook/component, two-step launch hook/panel, and live console log view. The app shell now composes the profile editor, launch flow, and console in one layout.
- Implemented the full Phase 2 Steam path: VDF parser, Steam root discovery, Steam library discovery, manifest matching, Proton install discovery/resolution, diagnostic collection, the auto-populate orchestrator, the Rust launcher export service, Tauri IPC for auto-populate/export, and the React `AutoPopulate` and `LauncherExport` surfaces integrated into the shared profile editor.
- Verification:
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo check --manifest-path src/crosshook-native/Cargo.toml`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core steam::auto_populate`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core steam::manifest`
- `env CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/crosshook-native-target cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core steam::proton`
- `env npm_config_cache=/tmp/npm-crosshook-native-cache npm_config_update_notifier=false npm install`
- `env npm_config_cache=/tmp/npm-crosshook-native-cache npm_config_update_notifier=false npm run build`

## 2026-03-22

### Goal

Add a Steam-mode auto-populate action that detects Steam App ID, compatdata path, and Proton path from the selected game executable while preserving the existing manual Steam fields and browse flow.

### Plan

- [x] Add a dedicated Steam auto-populate service for library, manifest, compatdata, and Proton detection.
- [x] Add a Steam settings button that runs auto-populate, applies only high-confidence matches, and shows detailed diagnostics/manual hints.
- [x] Cover the new detection logic with targeted unit tests for manifests, compatdata derivation, and Proton mapping resolution.
- [x] Verify the relevant test suite and a Debug build.

### Review

- Added `SteamAutoPopulateService` to detect Steam App ID, compatdata, and Proton from the selected game executable by scanning Steam roots, `libraryfolders.vdf`, `appmanifest_*.acf`, and Steam compat-tool mappings.
- Added an `Attempt Auto Populate` button to the Steam settings UI. It only fills high-confidence matches, leaves ambiguous or missing fields unchanged, logs diagnostics to the console, and shows a detailed results popup with manual path hints.
- Kept the existing manual Steam fields and browse actions unchanged so users can still override or complete the configuration themselves.
- Added targeted tests for successful detection, default Proton fallback, conflicting Proton mappings, and unmatched game paths.
- Verified with:
- `env PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj --filter SteamAutoPopulateServiceTests`
- `env PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`
- `env PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" dotnet build src/CrossHookEngine.sln -c Debug`
- Build note: `dotnet build` succeeded with an existing `NU1900` warning caused by NuGet vulnerability-cache writes targeting a read-only path outside the repo.

## 2026-03-22

### Goal

Fix Steam auto-populate follow-up issues so the UI shows host Unix paths instead of misleading Wine drive paths and Proton detection includes system-wide Steam compatibility tool directories such as `/usr/share/steam`.

### Plan

- [x] Fix Steam auto-populate host-path normalization so diagnostics and field values preserve host Unix paths.
- [x] Extend Proton tool discovery to scan system-wide Steam compatibility tool roots in addition to the configured Steam install.
- [x] Add regression tests for system Proton discovery and Unix-path preservation.
- [x] Verify the targeted tests, full test suite, and Debug build.

### Review

- Reworked Steam auto-populate path normalization to preserve host Unix-style paths instead of running them back through Windows `Path.GetFullPath`, which had been turning `/...` paths into misleading `Z:/...` display strings in the Wine-hosted UI.
- Extended Proton discovery so it still scans the configured Steam install, but now also checks system-wide compatibility tool roots such as `/usr/share/steam/compatibilitytools.d` and `/usr/local/share/steam/compatibilitytools.d`.
- Added a library-aware fallback for selected game paths that still come through as `compatdata/.../dosdevices/d:/...`, so auto-populate can resolve them back onto the discovered Steam libraries before manifest matching.
- Hardened the shared mounted-drive scanner to skip unreadable mount entries instead of throwing when `/mnt` contains restricted directories.
- Added regression tests for both a system-installed compat tool like `proton-cachyos-slr` and the `dosdevices` game-path case reported during live validation.
- Verified with:
- `env PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj --filter SteamAutoPopulateServiceTests`
- `env PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`
- `env PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" dotnet build src/CrossHookEngine.sln -c Debug`

## 2026-03-21

### Goal

Allow Steam-mode users to choose an optional PNG/JPG launcher icon so exported desktop launchers can use a trainer or game image instead of the default icon.

### Plan

- [x] Add a Steam-mode launcher icon path field and browse button to the form.
- [x] Persist the icon path in Steam-enabled profiles.
- [x] Use the optional icon path in external launcher export when creating the `.desktop` entry.
- [x] Verify with targeted tests and a Debug build.

### Review

- Added an optional `Launcher Icon` field to the Steam mode form with a PNG/JPG file picker so users can select either a trainer icon or a game icon for exported launchers.
- Steam-enabled profiles now save and load the launcher icon path alongside the existing Steam fields.
- External launcher export now validates the optional icon path and writes it into the generated `.desktop` entry `Icon=` field when present, otherwise it falls back to `applications-games`.
- Verified with:
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj --filter "ProfileServiceTests|SteamExternalLauncherExportServiceTests"`
- `dotnet build src/CrossHookEngine.sln -c Debug`
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`

## 2026-03-21

### Goal

Turn CrossHook into a generator for external Steam trainer launchers by writing a known-good host shell script and a matching `.desktop` entry under the user's home directory.

### Plan

- [x] Add a service that generates the trainer shell script and `.desktop` entry from the current Steam configuration.
- [x] Add a Steam-mode UI action that exports both artifacts using the current profile/config.
- [x] Cover the launcher export content and validation in the test suite.
- [x] Verify the build and full test suite.

### Review

- Added `SteamExternalLauncherExportService` to generate a trainer script under `~/.local/share/crosshook/launchers/` and a matching desktop entry under `~/.local/share/applications/`.
- Added a Steam settings button that creates both artifacts from the current Steam fields and logs the resulting file paths.
- The generated script intentionally uses the known-good manual `proton run` flow instead of CrossHook's in-app Steam trainer launch path.
- Verified with:
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj --filter SteamExternalLauncherExportServiceTests`
- `dotnet build src/CrossHookEngine.sln -c Debug`
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`

## 2026-03-21

### Goal

Move the actual Steam trainer launch out of the helper process CrossHook starts and into a detached native host runner.

### Plan

- [x] Keep CrossHook's Steam trainer entrypoint lightweight so it only spawns a detached host runner and exits.
- [x] Add a native host runner script that stages the trainer into compatdata and performs the actual `proton run`.
- [x] Pass both the Windows trainer path and normalized host trainer path through the Steam launch contract.
- [x] Verify the build and full test suite after the runner split.

### Review

- `steam-launch-trainer.sh` is now only a lightweight launcher that starts a detached host runner with a clean Linux environment and returns immediately.
- Added `steam-host-trainer-runner.sh`, which performs the host-side trainer staging and the actual `proton run`, so the trainer launch no longer occurs inside the helper process CrossHook directly started.
- Extended `SteamLaunchRequest` and the Steam start-info builders to pass `TrainerHostPath` through the helper contract, and updated validation/tests accordingly.
- Verified with:
- `dotnet build src/CrossHookEngine.sln -c Debug`
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`

## 2026-03-21

### Goal

Fix Steam trainer staging so the staged copy is created with real host-path semantics instead of using Unix paths through the Wine-hosted .NET process.

### Plan

- [x] Stop staging the trainer inside CrossHook's Windows process.
- [x] Pass the normalized trainer host path into the Steam helper scripts and perform the copy there.
- [x] Update Steam launch tests for the new `--trainer-host-path` contract and validation.
- [x] Verify the test suite after the staging move.

### Review

- Steam trainer staging no longer happens inside the Wine-hosted .NET process. CrossHook now passes both the original Windows trainer path and a normalized host trainer path into the Steam helper scripts.
- The bash helpers now copy the trainer into `compatdata/pfx/drive_c/CrossHook/StagedTrainers` on the host side immediately before the Proton launch, then switch `trainer_path` to the prefix-local `C:\CrossHook\StagedTrainers\...` path.
- Added validation and command-construction coverage for the new `TrainerHostPath` contract in the Steam launch tests.
- Verified with:
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj --filter SteamLaunchServiceTests`
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`

## 2026-03-21

### Goal

Try Steam trainer launch with a staged standalone trainer copy inside the target compatdata so CrossHook runs a prefix-local Windows path instead of the originally selected external path.

### Plan

- [x] Add launch-time staging for file-based Steam trainers into `pfx/drive_c/CrossHook/StagedTrainers`.
- [x] Update the Steam launch flow to replace the trainer path with the staged `C:\...` path only for the actual trainer run.
- [x] Add automated coverage for trainer staging and missing-file failure handling.
- [x] Verify the Steam launch tests and the app build with the local .NET 9 SDK.

### Review

- Steam mode now stages file-based trainers into the selected compatdata under `pfx/drive_c/CrossHook/StagedTrainers` immediately before the trainer launch step, instead of changing the stored trainer path at selection time.
- The Steam helper still uses the existing Wine `start.exe /unix /bin/bash` bridge, but the trainer path passed into the helper is now replaced with a prefix-local `C:\CrossHook\StagedTrainers\...` path for the actual trainer run.
- Added `SteamLaunchService` staging coverage for a successful staged copy and a missing-source failure path.
- Verified with:
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj --filter SteamLaunchServiceTests`
- `dotnet build src/CrossHookEngine.sln -c Debug`
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`

## 2026-03-21

### Goal

Make Steam-mode trainer launch match the known-good direct `proton run` path closely enough to avoid the `Access Denied` memory-access failure when the game is launched through Steam.

### Plan

- [x] Keep a working Unix bridge from the Wine-hosted app and avoid invalid direct native process startup from CrossHook.
- [x] Push the Steam compatdata environment into `ProcessStartInfo` before helper startup and keep the helper scripts focused on the actual Proton launch.
- [x] Add targeted helper logging so Steam trainer runs capture the effective compatdata and process context.
- [x] Update Steam launch tests to validate the native bash path and explicit environment values.
- [x] Verify the relevant test suite with the local .NET 9 SDK.

### Review

- Steam-mode helper startup in `SteamLaunchService` keeps Wine's `start.exe /unix /bin/bash` bridge because direct `Process.Start("/bin/bash")` from CrossHook fails under Wine with `E_HANDLE`.
- Steam helper processes now receive explicit `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and `WINEPREFIX` values via `ProcessStartInfo.Environment`, while known Wine/Proton bridge variables are cleared before startup.
- Both bundled Steam helper scripts were simplified back to direct `proton run` for trainer startup and now log the effective compatdata, Proton path, and shell process context for easier runtime comparison against the known-good manual command.
- Updated the Steam launch tests to assert the bridged `/bin/bash` arguments and explicit Steam environment values.
- Verified with:
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj --filter SteamLaunchServiceTests`
- `dotnet build src/CrossHookEngine.sln -c Debug`
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`

## 2026-03-20

### Goal

Break down GitHub issue #10 into individually tracked child issues with detailed research anchors, concrete code references, and correct labels for follow-on AI agent work.

### Plan

- [x] Read issue #10 and its recommended slice table.
- [x] Validate relevant issue templates, label availability, and current repo guidance.
- [x] Map each recommended slice to current research anchors and implementation files.
- [x] Draft detailed child issue bodies for all recommended slices.
- [x] Create the child issues in GitHub with the intended labels and priorities.
- [x] Verify the created issues and record the results.

### Review

- Created descendant issues from parent tracker `#10`:
- `#11` `[Docs]: Research refresh for March 2026 external-state assumptions`
- `#12` `[Compat]: Proton/WINE injection compatibility matrix and minimal test harness`
- `#13` `[Feature]: Community profiles research: schema, tap model, and contribution flow`
- `#14` `[Feature]: First-run experience plan: wizard, auto-detection, and controller navigation`
- `#15` `[Feature]: Accessibility baseline and legitimacy framing plan`
- `#16` `[Feature]: Multi-tier modification architecture and injection abstraction plan`
- `#17` `[Refactor]: Framework-neutral core boundaries and split-architecture planning`
- Verified that each issue is open and has the intended labels, priority, parent reference, research anchors, and plain file references.
- Added a comment to parent issue `#10` linking the new child issues.

## 2026-03-20

### Goal

Fix trainer launch coordination for Steam/Proton games so the game remains the primary tracked process and post-launch actions wait for the game to stabilize before the trainer starts.

### Plan

- [x] Inspect the launch flow and confirm how game and trainer process tracking currently interact.
- [x] Add a readiness check for the launched game process before post-launch DLL injection and trainer startup.
- [x] Keep trainer process tracking separate so launching the trainer does not overwrite the game process context.
- [x] Extend automated coverage for the new readiness helper and repair the stale test project wiring needed to run the suite.
- [x] Verify the app build and the shared-source test project with the local .NET 9 SDK.

### Review

- Added process readiness polling in `ProcessManager` with explicit timeout, stabilization, module-visibility, and main-window diagnostics.
- Updated the main launch flow so post-launch DLL injection and trainer startup wait for game readiness, and trainer launch uses a dedicated secondary `ProcessManager`.
- Preserved the main game process as the primary injection and memory target after trainer launch.
- Repaired stale test project path and namespace issues that were preventing the existing test suite from compiling under the repo’s current layout.
- Verified with:
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`
- `dotnet build src/CrossHookEngine.sln -c Debug`

## 2026-03-20

### Goal

Add a practical Steam-mode launch path inside CrossHook and a first-class Steam-aware launch flow so Steam games use the same Launch button instead of the broken direct-EXE path.

### Plan

- [x] Extend profiles with Steam-mode metadata and a bundled Steam helper contract.
- [x] Add Steam controls to the main form and branch the Launch button between direct and Steam flows.
- [x] Package the Steam helper with the app output and release artifacts.
- [x] Add tests for Steam profile persistence and Steam helper command construction.
- [x] Verify the linked-source test suite and the app build with the local .NET 9 SDK.

### Review

- Added Steam profile fields: `UseSteamMode`, `SteamAppId`, `SteamCompatDataPath`, and `SteamProtonPath`.
- Added a packaged helper script under `runtime-helpers/steam-launch-helper.sh` and included it in build/publish output.
- Added a Steam-mode UI section in the trainer setup panel and routed the existing `Launch` button through a Steam helper path when Steam mode is enabled.
- Steam mode now validates App ID, compatdata path, Proton path, and trainer path before launch, and blocks in-app DLL injection with a clear limitation message.
- Added `SteamLaunchService` for validation, helper path conversion, and helper command construction.
- Expanded the test suite with Steam launch service tests and Steam profile round-trip assertions.
- Verified with:
- `dotnet test tests/CrossHookEngine.App.Tests/CrossHookEngine.App.Tests.csproj`
- `dotnet build src/CrossHookEngine.sln -c Debug`
