# Documentation Research: launcher-delete

This document catalogs all documentation relevant to implementing the launcher-delete feature (launcher lifecycle management: auto-delete/rename launchers when profiles change, plus manual UI controls). It is organized by document type, with reading priority indicated.

## Feature Research

These are the primary specification and research documents produced during the launcher-delete feature research phase. They live in `docs/plans/launcher-delete/`.

- `/docs/plans/launcher-delete/feature-spec.md`: **Consolidated feature specification.** Executive summary, external dependencies (Freedesktop .desktop spec, XDG paths), business rules with edge case table, data models (LauncherInfo, LauncherDeleteResult, LauncherRenameResult), full Tauri IPC API design (5 new commands + 2 modified commands), system integration map (files to create and modify), UX workflows (profile delete cascade, profile rename cascade, manual management), phased implementation plan (3 phases, ~18-22 tasks), risk assessment, and open decisions.
- `/docs/plans/launcher-delete/research-business.md`: User stories, business rules (6 core rules), primary and alternative workflows, domain model (GameProfile to Launcher relationship diagram), lifecycle event table, existing codebase integration analysis (9 related files with roles), patterns to follow (7 patterns), components to leverage (7 reusable functions), data model gaps, and success criteria.
- `/docs/plans/launcher-delete/research-technical.md`: Architecture component diagram, data model design (stateless slug derivation vs. tracked registry), file path convention table, Rust type definitions for LauncherInfo/LauncherDeleteResult/LauncherRenameResult, Tauri IPC command signatures (check/delete/rename/list + profile_delete cascade + profile_rename), system constraints (atomic rename, permissions, desktop environment interaction, slug collision), 5 technical decisions with rationale, codebase change manifest (1 file to create, ~10 files to modify), and open questions.
- `/docs/plans/launcher-delete/research-ux.md`: User workflow descriptions (profile delete with launcher cleanup, profile rename with launcher update, manual management), destructive action patterns (tiered severity: low/medium/high), button labeling and microcopy guidelines, rename cascade notification patterns, status indicator design (Exported/Not Exported/Stale/Error), gamepad/controller accessibility considerations (A=confirm, B=cancel, focus trap, 44px targets), error handling table (9 error states with recovery actions), performance UX (optimistic updates, loading states), competitive analysis (Steam, Lutris, Heroic Games Launcher, Linux desktop managers), and prioritized recommendations (5 must-have, 4 should-have, 4 nice-to-have).
- `/docs/plans/launcher-delete/research-external.md`: Freedesktop Desktop Entry Specification v1.5 (required/optional fields, file naming rules, preservation rule, Hidden flag, TryExec behavior), XDG Base Directory Specification (XDG_DATA_HOME, directory creation rules), desktop cache invalidation (update-desktop-database, inotify behavior per DE), xdg-desktop-menu and desktop-file-validate tools, Rust crate evaluation (4 crates evaluated, none recommended -- use std::fs + existing directories crate), integration patterns with code examples (delete, rename, discover, path resolution), file management safety (atomic rename guarantees, idempotent deletion, TOCTOU analysis), permissions table (0644 for .desktop, 0755 for scripts), and 8 gotchas with impact/mitigation.
- `/docs/plans/launcher-delete/research-recommendations.md`: Implementation strategy (manifest registry approach recommended for v1, with phased alternative), technology choices table, phasing strategy (3 phases: Foundation, Rename + Manual Management, Integrity + Polish), quick wins (cascade delete, launcher status indicator, confirmation dialog), improvement ideas (5 related features, 4 future enhancements), risk assessment (7 technical risks, 3 integration challenges, 3 security considerations), 4 alternative approaches with tradeoff analysis (manifest vs. profile-embedded vs. filesystem scanning vs. hybrid), task breakdown preview (~18-22 tasks), and key decisions needed.

## Architecture Docs

- `/docs/features/steam-proton-trainer-launch.doc.md`: Documents the current launcher export system in full. Covers launcher output locations (`~/.local/share/crosshook/launchers/` for scripts, `~/.local/share/applications/` for .desktop entries), the naming pattern (`<slug>-trainer.sh` and `crosshook-<slug>-trainer.desktop`), generated script structure (environment variables and exec pattern), and usage workflow. This is essential context for understanding what launcher artifacts exist and how they are structured.
- `/docs/getting-started/quickstart.md`: User-facing guide covering all three launch modes (steam_applaunch, proton_run, native), profile creation workflow, external launcher export section, and profile TOML structure. Important for understanding which launch methods support launchers (steam_applaunch and proton_run, but NOT native).

## API Docs (Code-Level)

Documentation embedded in the codebase that an implementer must understand:

- `/src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`: Core export logic. Contains `SteamExternalLauncherExportRequest` and `SteamExternalLauncherExportResult` structs (IPC boundary types), `SteamExternalLauncherExportValidationError` enum, `export_launchers()` (the main export function), `validate()`, `sanitize_launcher_slug()` (the deterministic slug derivation), `resolve_display_name()` (display name fallback chain), `resolve_target_home_path()`, `combine_host_unix_path()`, `build_desktop_entry_content()`, `build_trainer_script_content()`, and `write_host_text_file()`. Several private functions need visibility elevation to `pub(crate)` for the new `launcher_store` module to reuse them.
- `/src/crosshook-native/crates/crosshook-core/src/export/mod.rs`: Module root with public re-exports. Must be extended with `pub mod launcher_store;` and re-exports of new types.
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` with `save()`, `load()`, `delete()`, `list()` methods plus `ProfileStoreError` enum. No `rename()` method exists today -- this must be added. The `delete()` method is a plain `fs::remove_file` with no lifecycle hooks. The `profile_path()` helper derives the TOML file path from a profile name.
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile` struct and all nested section types. The `SteamSection` contains a `LauncherSection` with `icon_path` and `display_name` fields -- these are the inputs for launcher slug derivation. The `LaunchSection` has a `method` field that determines whether launcher export is applicable.
- `/src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`: Profile module public API surface. Re-exports `ProfileStore`, `ProfileStoreError`, `GameProfile`, `LauncherSection`, and all section types.
- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Module root for crosshook-core. Lists all public modules: `community`, `export`, `install`, `launch`, `logging`, `profile`, `settings`, `steam`.
- `/src/crosshook-native/src-tauri/src/commands/export.rs`: Tauri command wrappers for `validate_launcher_export` and `export_launchers`. Thin delegation to crosshook-core with `.map_err(|e| e.to_string())` error mapping. New commands (`check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers`) must follow this same pattern.
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Tauri commands for `profile_list`, `profile_load`, `profile_save`, `profile_delete`, `profile_import_legacy`. The `profile_delete` command is a thin wrapper that does NOT load the profile first or perform any launcher cleanup -- this is the primary integration point for cascade delete. A new `profile_rename` command must be added.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri app setup and command registration. All managed state (`ProfileStore`, `SettingsStore`, `RecentFilesStore`, `CommunityTapStore`) is initialized here. The `invoke_handler` macro lists all registered commands -- new commands must be added to this array.

## Frontend Docs (Code-Level)

- `/src/crosshook-native/src/components/LauncherExport.tsx`: The launcher export UI component. Defines local `SteamExternalLauncherExportRequest` and `SteamExternalLauncherExportResult` interfaces (duplicated from the Rust types). Accepts `LauncherExportProps` with `profile`, `method`, `steamClientInstallPath`, `targetHomePath`, and `context`. Must be extended with launcher status indicator, delete/rename buttons, and launcher existence checking on mount.
- `/src/crosshook-native/src/components/ProfileEditor.tsx`: Contains the delete button that triggers `deleteProfile()` from the `useProfile` hook.
- `/src/crosshook-native/src/hooks/useProfile.ts`: Exports `UseProfileResult` interface with `profiles`, `selectedProfile`, `profileName`, `profile`, `dirty`, `saveProfile()`, `deleteProfile()`, `refreshProfiles()`, etc. The `deleteProfile()` function calls `profile_delete` and clears `last_used_profile`. Must be extended to detect rename scenarios (when `profileName !== selectedProfile`) and invoke `profile_rename`.
- `/src/crosshook-native/src/types/profile.ts`: TypeScript `GameProfile` interface mirroring the Rust struct. Includes `LaunchMethod` type union (`'' | 'steam_applaunch' | 'proton_run' | 'native'`). New `LauncherInfo`, `LauncherDeleteResult`, and `LauncherRenameResult` interfaces must be added here or in a new `launcher.ts` file.
- `/src/crosshook-native/src/types/settings.ts`: `AppSettingsData` with `last_used_profile` field. Relevant because profile delete clears this field.
- `/src/crosshook-native/src/types/index.ts`: Re-exports from all type modules. Any new type file must be added here.

## Development Guides

- `/CLAUDE.md`: Project-level instructions for AI assistants. Contains the full architecture diagram, tech stack description, build commands, code conventions (Rust: snake_case, serde derives, Result<T,E> with anyhow; TS: PascalCase components, camelCase hooks, strict mode), key patterns (Tauri IPC, TOML persistence, Steam discovery, launch methods, community taps, gamepad navigation, launcher export, workspace crate separation), and important notes (native Linux app, AppImage distribution, no frontend test framework).
- `/docs/internal-docs/local-build-publish.md`: Local build and publish workflow. Documents development (`./scripts/dev-native.sh`), AppImage build (`./scripts/build-native.sh`), container build, CI/CD pipeline, artifact shape, and release preparation. Relevant for understanding the build/test cycle implementers must follow.

## Configuration Files

- `/src/crosshook-native/Cargo.toml`: Workspace root with 3 members (crosshook-core, crosshook-cli, src-tauri). Version 0.2.0.
- `/src/crosshook-native/crates/crosshook-core/Cargo.toml`: Core library dependencies: `directories` v5 (XDG path resolution), `serde` v1, `serde_json` v1, `toml` v0.8, `tokio` v1, `tracing` v0.1, `tracing-subscriber` v0.3. Dev-dependencies: `tempfile` v3. No new dependencies are needed for launcher-delete.
- `/src/crosshook-native/src-tauri/tauri.conf.json`: Tauri configuration. Bundle target: AppImage only. Window: 1280x800, dark theme. Resources: `runtime-helpers/*.sh`. Security: CSP disabled.
- `/src/crosshook-native/src-tauri/capabilities/default.json`: Tauri permissions: `core:default` and `dialog:default`. No filesystem permission plugin is registered (Tauri commands use Rust std::fs directly, not the Tauri FS plugin for user-facing operations).
- `/.github/workflows/release.yml`: CI pipeline. Runs `cargo test -p crosshook-core` before building. New tests in crosshook-core will be automatically included.
- `/.github/pull_request_template.md`: PR template with checklist items relevant to this feature: "If touching crates/crosshook-core/src/profile/: Verified profile save/load/import" and "If touching src/components/ or src/hooks/: Verified UI renders correctly".

## README Files

- `/CLAUDE.md`: See "Development Guides" above -- this is the primary project instruction file.

## Prior Feature Research (install-game)

The `install-game` feature followed the same research pattern and provides a reference for how feature implementation was structured:

- `/docs/plans/install-game/feature-spec.md`: Consolidated spec for the install-game feature. Useful as a structural template for how the launcher-delete spec is organized.
- `/docs/plans/install-game/research-docs.md`: Documentation research for install-game. Shows the expected format and depth for this type of research document.
- `/docs/plans/install-game/research-architecture.md` through `research-ux.md`: Full research suite. Demonstrates the multi-file research methodology used in this project.

## Must-Read Documents

Documents that implementers MUST read before starting launcher-delete work, ordered by priority:

1. **`/docs/plans/launcher-delete/feature-spec.md`** -- The authoritative specification. Contains all data models, API contracts, file change manifest, phased task breakdown, and open decisions that must be resolved before implementation.
2. **`/CLAUDE.md`** -- Project conventions, architecture diagram, code patterns, and the workspace crate structure that governs where new code goes.
3. **`/docs/plans/launcher-delete/research-technical.md`** -- Architecture design, system constraints, codebase change list, and 5 technical decisions with rationale. Directly maps to implementation work.
4. **`/docs/plans/launcher-delete/research-business.md`** -- Business rules, existing codebase integration analysis, patterns to follow, and components to leverage. Identifies the exact functions to reuse and files to modify.
5. **`/docs/features/steam-proton-trainer-launch.doc.md`** -- Documents the current launcher export system including output paths, naming conventions, and script structure. Essential context for understanding what the new code must manage.
6. **`/docs/plans/launcher-delete/research-external.md`** -- Freedesktop .desktop spec requirements, XDG path conventions, file operation patterns with code examples, and 8 gotchas that affect implementation.
7. **`/docs/plans/launcher-delete/research-ux.md`** -- UX workflows, confirmation dialog patterns, status indicator design, gamepad accessibility requirements, and competitive analysis. Required for frontend implementation.
8. **`/docs/plans/launcher-delete/research-recommendations.md`** -- Implementation strategy, alternative approaches, risk assessment, and task breakdown. Required for planning the implementation order.

## Nice-to-Read Documents

- `/docs/getting-started/quickstart.md` -- General user context; helpful for understanding which launch methods produce launchers.
- `/docs/internal-docs/local-build-publish.md` -- Build and test workflow; helpful for CI integration.
- `/.github/pull_request_template.md` -- PR checklist; useful for pre-submission verification.
- `/docs/plans/install-game/feature-spec.md` -- Reference for how a similar feature was specified and implemented.

## Documentation Gaps

1. **No existing documentation for launcher file content format.** The .sh script and .desktop entry templates are defined only in Rust code (`build_trainer_script_content()` and `build_desktop_entry_content()` in `launcher.rs`). There is no external documentation of the exact generated content, making it harder to understand what content must be rewritten during rename operations.

2. **No documentation of `sanitize_launcher_slug()` behavior.** The slug derivation function is critical to the entire launcher-delete feature (it determines file paths), but its behavior (lowercase, replace non-alphanumeric with hyphens, trim) is documented only in the feature research, not in code comments or external docs.

3. **No documentation of `resolve_display_name()` fallback chain.** This function determines the display name used for slug derivation by checking `launcher_name`, then `steam_app_id`, then `trainer_path`. This chain is only documented in the research files, not in code-level documentation.

4. **No existing tests for the export module.** The crosshook-core crate has tests, but there are no existing test fixtures or patterns for the export module. New tests must establish the testing approach from scratch.

5. **The existing `export_launchers()` function hardcodes `~/.local/share/` paths** instead of using `BaseDirs::data_dir()`. Both the research-external and research-technical documents flag this as a refactoring candidate. The new `launcher_store` module should use `BaseDirs` for correctness, but this creates a potential path mismatch with launchers exported by the current code.

6. **No documentation of Tauri command registration requirements.** When adding new commands, the implementer must know to: (a) create the function in `commands/`, (b) register it in `lib.rs`'s `invoke_handler` macro, and (c) potentially update Tauri capabilities. This process is only inferrable from existing code, not documented.

7. **The feature-spec's "Decisions Needed" section (4 items) appears unresolved.** The spec lists decisions about manifest vs. stateless, explicit vs. implicit rename, confirmation UX, and rename cascade behavior with recommendations but no final decisions recorded. Implementers may need to confirm these with the project owner before proceeding.

8. **No frontend testing documentation or framework.** The CLAUDE.md notes "No test framework is configured for the frontend." The new UI components (launcher status indicator, delete/rename buttons, confirmation dialogs) cannot be unit-tested with the current setup. This gap affects the "Stale" status indicator and gamepad accessibility verification in particular.
