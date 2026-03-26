# Documentation Research: update-game

## Overview

The update-game feature has comprehensive planning documentation across six dedicated research files and a feature spec, all located under `docs/plans/update-game/`. The codebase itself is well-documented through CLAUDE.md project guidelines, a feature doc for Steam/Proton workflows, a quickstart guide, and well-structured Rust code with module-level doc comments. Configuration files (tauri.conf.json, capabilities/default.json, Cargo.toml manifests, package.json) are small and straightforward. No new dependencies are required. The implementation closely mirrors the existing `install` module, which serves as the primary code-level template.

---

## Architecture Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md`: **Primary project reference.** Defines the full architecture tree, key patterns (Tauri IPC, TOML persistence, Steam discovery, launch methods, workspace crate separation), code conventions (Rust snake_case, React PascalCase, TypeScript strict mode), build commands, commit/changelog hygiene rules, and label taxonomy. This is the single source of truth for how the project is structured.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md`: **End-to-end workflow documentation** covering all three launch methods (`steam_applaunch`, `proton_run`, `native`), auto-populate/Steam discovery, launcher export lifecycle, console view, and troubleshooting. Critical for understanding how Proton prefix environment variables work, how `proton_run` mode operates (which is what update-game reuses), and how the console view streams output.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md`: User-facing quickstart covering supported environments (Linux Desktop, Steam Deck), installation, profile creation, and launch modes. Provides context on user expectations and the conceptual model of profiles, prefixes, and launch methods.

---

## Feature Specification

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/feature-spec.md`: **The definitive spec for implementation.** Contains:
  - Executive summary of the feature (run Windows update .exe against existing Proton prefix)
  - External dependencies (Proton CLI, environment variables, no new libraries)
  - Business requirements (7 business rules, edge cases table, success criteria checklist)
  - Technical specifications (architecture diagram, data models for `UpdateGameRequest`/`UpdateGameResult`/`UpdateGameError`/`UpdateGameValidationError`, API design for both Tauri commands)
  - System integration section listing all files to create, modify, and reuse as-is
  - UX considerations (primary workflow, error recovery, UI patterns table, accessibility, performance UX)
  - Recommendations (3-phase implementation, technology decisions table, quick wins)
  - Risk assessment (7 technical risks with likelihood/impact/mitigation)
  - Task breakdown preview (3 phases: Foundation backend, Core frontend, Integration/testing)
  - **5 resolved decisions**: UI placement (Install page section), log streaming (real-time via `spawn_log_stream` from Phase 1), event channel (new `update-log` event), working directory (updater's parent), profile scope (`proton_run` only)

---

## Research Reports

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/research-business.md`: User stories, 7 business rules with validation/exception details, edge cases (prefix deleted, game running, DLLs needed, GUI installer, silent patcher, Steam-managed prefix, empty prefix path), primary and alternative workflows, domain model with state transitions (`idle -> preparing -> running_update -> completed/failed`), and a detailed codebase integration analysis listing 15 related files with specific function references and line numbers.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/research-external.md`: Proton CLI verbs (`run`, `waitforexitandrun`, `runinprefix`, `getcompatpath`, `createprefix`), required and optional environment variables with how CrossHook sets each, protontricks and umu-launcher assessments (neither is a dependency), library assessment (all existing deps sufficient), integration patterns with pseudocode for `build_update_command`, prefix directory reference layout, and 7 gotchas (prefix locking, version mismatch, working directory, DLL overrides, compatdata structure, file permissions, MSI installers).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/research-technical.md`: Architecture design with component diagram and data flow, complete Rust model definitions (4 structs/enums with field-level doc comments), TypeScript type definitions including `UPDATE_GAME_VALIDATION_MESSAGES` and `UPDATE_GAME_VALIDATION_FIELD` lookup maps, full Tauri IPC API design for both commands, core service pseudocode for `update_game` and `build_update_command`, system constraints (prefix management, env vars, gamepad navigation, working directory), frontend design (`useUpdateGame` hook and `UpdateGamePanel` component), codebase change matrix (7 files to create, 5 to modify, 6 reused as-is), and 5 technical decisions with options/recommendations/rationale.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/research-ux.md`: Primary and alternative user workflows with step-by-step system responses, competitive analysis of 5 launchers (Lutris, Heroic, Bottles, Playnite, Steam) with strengths/weaknesses/lessons, UI/UX best practices (proximity, progressive disclosure, confirmation dialogs, form validation, component reuse), gamepad/controller navigation requirements, Steam Deck-specific considerations (resolution, input priority, file picker, scroll behavior), error handling table (9 error states with messages and recovery actions), performance UX (progress indicators, console output, loading states), and the Install-vs-Update page decision with rationale.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/research-recommendations.md`: Recommended implementation approach (3 phases), technology choices table, phasing strategy, quick wins, improvement ideas (prefix backup, update history, auto-detect executables, community tap integration), 8-row risk assessment matrix, integration challenges, 3 alternative approaches (Option A: Install page section, Option B: Dedicated page, Option C: Profile action context menu) with pros/cons, full task breakdown preview with critical path dependencies, key decisions needed, and open questions.

---

## Configuration Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/tauri.conf.json`: Tauri v2 app configuration. Window: 1280x800, dark theme, single "main" window. Bundle: AppImage target only. Build: Vite dev server at localhost:5173. Resources: runtime-helpers shell scripts. No special security CSP. **No changes needed for update-game** -- new Tauri commands are registered in Rust, not in this config.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/capabilities/default.json`: Tauri permission grants for the main window. Currently grants `core:default` and `dialog:default`. **No changes needed** -- the new `update_game` and `validate_update_request` commands are registered via `invoke_handler` (not capability-gated beyond `core:default`).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/Cargo.toml`: Workspace root. Members: `crosshook-core`, `crosshook-cli`, `src-tauri`. Current version: 0.2.2, resolver 2. **No changes needed** -- update module lives inside the existing `crosshook-core` crate.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml`: Core library dependencies: `directories 5`, `serde 1` (derive), `serde_json 1`, `toml 0.8`, `tokio 1` (fs, process, rt, sync), `tracing 0.1`, `tracing-subscriber 0.3`. Dev-dep: `tempfile 3`. **No new dependencies needed.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/Cargo.toml`: Tauri app shell dependencies: `crosshook-core` (path), `serde 1`, `serde_json 1`, `tauri 2`, `tauri-plugin-dialog 2`, `tauri-plugin-fs 2`, `tauri-plugin-shell 2`, `tokio 1` (fs, process, time), `tracing 0.1`. **No new dependencies needed.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/package.json`: Frontend dependencies: React 18, `@tauri-apps/api 2`, `@tauri-apps/plugin-dialog 2`, `@tauri-apps/plugin-fs 2`, `@tauri-apps/plugin-shell 2`, `@radix-ui/react-select`, `@radix-ui/react-tabs`, `react-resizable-panels`. TypeScript 5.6, Vite 8. **No new dependencies needed.**

---

## Code Documentation

### Well-Documented Modules Relevant to Implementation

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/mod.rs`: Module root with doc comment (`//! Install-game domain contracts and shared data models.`) and clean re-exports of all public types/functions. **Pattern to follow** for `update/mod.rs`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs`: 316 lines. Contains `validate_install_request`, `install_game`, `build_install_command`, `provision_prefix`, and all validation helpers. Well-tested (3 test functions covering prefix creation, validation errors, and end-to-end install). **The primary template** for `update/service.rs` -- the update version skips `provision_prefix`, `discover_game_executable_candidates`, and `build_reviewable_profile`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs`: 257 lines. Defines `InstallGameRequest`, `InstallGameResult`, `InstallGameError`, `InstallGameValidationError` with full `message()` and `Display` implementations, `From` conversion, and a test for `reviewable_profile`. **The primary template** for `update/models.rs`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: 191 lines. Provides all the Proton command-building primitives: `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `resolve_wine_prefix_path`, `resolve_compat_data_path`, `apply_working_directory`, `attach_log_stdio`, `resolve_steam_client_install_path`. No doc comments beyond the `DEFAULT_HOST_PATH` constant, but function signatures are self-documenting. **Reused directly -- no modifications needed.**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/install.rs`: 73 lines. Thin Tauri command layer with `install_game`, `validate_install_request`, `install_default_prefix_path`, `create_log_path`, and `install_log_target_slug`. **Direct template** for `commands/update.rs`. The `create_log_path` function may need to be extracted to a shared utility or duplicated.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`: 120+ lines. Contains `launch_game`, `launch_trainer`, and the critical `spawn_log_stream` / `stream_log_lines` functions for real-time log output via `launch-log` Tauri events. **The log streaming pattern** the feature spec says to use for update-game (via a new `update-log` event).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: 107 lines. Tauri setup: initializes stores (ProfileStore, SettingsStore, RecentFilesStore, CommunityTapStore), registers plugins (shell, dialog, fs), registers 32 Tauri commands in `invoke_handler`. **Must be modified** to add `commands::update::update_game` and `commands::update::validate_update_request`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs`: Module declaration file for Tauri commands. Currently exports: community, export, install, launch, profile, settings, steam. **Must be modified** to add `pub mod update;`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Module root for the core library. Currently exports: community, export, install, launch, logging, profile, settings, steam. **Must be modified** to add `pub mod update;`.

### Frontend Code Documentation

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts`: TypeScript types for the install feature including `InstallGameRequest`, `InstallGameResult`, `InstallProfileReviewPayload`, `InstallGameExecutableCandidate`, `InstallGameValidationError`, validation message maps, field maps, and stage types. **Direct template** for `types/update.ts`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/index.ts`: Re-export barrel file for all type modules. **Must be modified** to add `export * from './update';`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts`: Large hook (580+ lines) managing install flow state machine. Exports `UseInstallGameResult` interface with request, validation, stage, result, derived booleans, and actions. **Template** for a much simpler `useUpdateGame.ts` (no candidate discovery, no prefix resolution, no review profile).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/InstallPage.tsx`: 451 lines. Page-level orchestration with Proton install loading, profile review modal management (dirty checking, confirmation flow), and profile save/navigate logic. Renders `InstallGamePanel` and `ProfileReviewModal`. **Must be modified** to add `UpdateGamePanel` below `InstallGamePanel`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx`: Contains `InstallField` and `ProtonPathField` sub-components. **These need to be extracted to shared components** for reuse by `UpdateGamePanel`, or duplicated.

---

## Related Planning Documentation

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/proton-optimizations/feature-spec.md`: Feature spec for the Proton launch optimizations feature. Relevant because it demonstrates the project's established feature spec format, the pattern for adding a new UI panel to an existing page, and how Proton environment variable management works.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-docs.md`: Research docs from the profile-modal feature. Demonstrates the expected format for this type of research document.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/research/additional-features/implementation-guide.md`: Feature implementation guide covering all planned features with dependency chains and quick-win classification. Provides context on where update-game fits in the broader feature roadmap.

---

## Must-Read Documents

These documents are **required reading** before implementing update-game, listed in recommended order:

| Priority | Document                                                                   | Topics                                                                                                                                                                                                    |
| -------- | -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1        | `docs/plans/update-game/feature-spec.md`                                   | Complete specification: data models, API contracts, file change matrix, resolved decisions, task breakdown                                                                                                |
| 2        | `CLAUDE.md`                                                                | Project architecture, code conventions, build commands, commit conventions, testing approach                                                                                                              |
| 3        | `docs/plans/update-game/research-technical.md`                             | Detailed Rust/TypeScript data model definitions, service pseudocode, Tauri command patterns, frontend hook/component design                                                                               |
| 4        | `docs/plans/update-game/research-business.md`                              | Codebase integration analysis with specific file paths and function names to reuse, difference table (install vs update)                                                                                  |
| 5        | `src/crosshook-native/crates/crosshook-core/src/install/service.rs`        | The primary code template -- read `build_install_command`, `validate_install_request`, and `install_game` to understand the pattern being followed                                                        |
| 6        | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | All runtime primitives being reused: `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `resolve_wine_prefix_path`, `apply_working_directory`, `attach_log_stdio` |
| 7        | `src/crosshook-native/src-tauri/src/commands/launch.rs`                    | The `spawn_log_stream` and `stream_log_lines` pattern for real-time log streaming via Tauri events (the spec requires this for update-game)                                                               |
| 8        | `src/crosshook-native/src-tauri/src/commands/install.rs`                   | Direct template for `commands/update.rs` including `create_log_path` and the `spawn_blocking` pattern                                                                                                     |

### Nice-to-Have Reading

| Document                                             | Topics                                                                                                |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `docs/plans/update-game/research-external.md`        | Proton verbs, environment variables, prefix directory layout, competitive launcher approaches         |
| `docs/plans/update-game/research-ux.md`              | Competitive analysis (Lutris, Heroic, Bottles), gamepad navigation requirements, error handling table |
| `docs/plans/update-game/research-recommendations.md` | Alternative UI approaches, risk matrix, phasing strategy, future enhancements                         |
| `docs/features/steam-proton-trainer-launch.doc.md`   | Full Steam/Proton workflow documentation, console view behavior, launcher export lifecycle            |
| `docs/getting-started/quickstart.md`                 | User-facing mental model of profiles, prefixes, and launch methods                                    |

---

## Documentation Gaps

### Missing or Incomplete

1. **No `analysis-code.md` or `analysis-tasks.md`**: Unlike the `profile-modal` and `proton-optimizations` feature plans which include code analysis and task analysis documents, the `update-game` plan directory lacks these. The feature spec partially covers this ground in its task breakdown and system integration sections, but a dedicated code analysis document mapping specific functions to implementation tasks would help parallelization.

2. **No `parallel-plan.md`**: Other feature plans include a parallelization plan for concurrent development. The feature spec's task breakdown identifies some parallelization opportunities but does not formalize them into a parallel execution plan.

3. **No `shared.md`**: Other feature plans include a shared context document. The update-game research files cross-reference each other adequately, but a shared document consolidating resolved decisions and shared context would reduce repetition.

4. **`create_log_path` duplication strategy undocumented**: The `create_log_path` helper in `commands/install.rs` is a private function. The feature spec says the update command needs the same function but does not specify whether to extract it to a shared utility (recommended) or duplicate it. The `install_log_target_slug` function has the same issue.

5. **ConsoleDrawer event subscription not documented**: The feature spec introduces a new `update-log` Tauri event but does not document how the frontend `ConsoleDrawer` (or `ConsoleView`) component currently subscribes to events and what changes are needed to also subscribe to `update-log`. The launch flow uses `launch-log`; the install flow does not stream at all. The update flow's `update-log` is a new pattern that requires frontend changes not fully specified.

6. **Shared component extraction details missing**: The feature spec says to extract `InstallField` and `ProtonPathField` from `InstallGamePanel.tsx` to shared components. However, no document specifies the exact extraction plan (target directory, import changes, whether to use a barrel file, how to handle the `InstallFieldProps` interface).

7. **Profile model field mapping not exhaustive**: The feature spec shows `UpdateGameRequest` with `prefix_path` and `proton_path`, but the research-business doc notes that `steam_applaunch` profiles store these as `steam.compatdata_path` and `steam.proton_path` while `proton_run` profiles use `runtime.prefix_path` and `runtime.proton_path`. The frontend `populateFromProfile` function must handle both, but the exact field-mapping logic is not spelled out in a single location.

8. **No test plan document**: While the feature spec lists success criteria and the task breakdown mentions tests, there is no dedicated test plan specifying which unit tests to write, what the test fixtures should look like, or how to structure the Tokio runtime test harness (which the install tests demonstrate but which is non-obvious for newcomers).

### Potentially Outdated

9. **Feature spec decision conflict**: The feature spec resolved Decision 1 as "UI placement on Install page" and Decision 2 as "real-time streaming from Phase 1," but `research-recommendations.md` recommends Option B (Dedicated Update Game Page) and `research-technical.md` Decision 4 recommends blocking (not streaming). **The feature spec takes precedence** as it contains the final resolved decisions, but implementers should be aware that the research docs contain earlier recommendations that were superseded.

10. **`steam_applaunch` profile scope**: `research-business.md` originally considered supporting `steam_applaunch` profiles for update. The feature spec resolved this as `proton_run` only, excluding both `native` and `steam_applaunch`. The research-business doc's workflow and success criteria still reference `steam_applaunch` support. **The feature spec's narrower scope takes precedence.**
