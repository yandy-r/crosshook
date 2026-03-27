# Documentation Research: Dry Run / Preview Launch Mode

## Overview

This document catalogs all documentation relevant to implementing the dryrun-preview feature (#40). The feature adds a read-only "Preview Launch" mode that shows exactly what CrossHook will do before clicking Launch, using existing pure computation functions. A comprehensive feature spec and five research files from the feature-research phase provide the primary reference material. This index covers feature research, project documentation, inline code documentation, and configuration files.

---

## Architecture Docs

### Feature Research (Primary — `docs/plans/dryrun-preview/`)

| File                                                    | Purpose                                                      | Key Contents                                                                                                                                                                                                                                                                                                                                                             |
| ------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `docs/plans/dryrun-preview/feature-spec.md`             | **Master spec** — the single source of truth for the feature | Executive summary, business rules, data models (Rust + TypeScript), API design, architecture overview, UX design, phased task breakdown, risk assessment                                                                                                                                                                                                                 |
| `docs/plans/dryrun-preview/research-business.md`        | Business logic deep-dive                                     | 6 user stories, 9 business rules, 5 edge case rules, 3 workflows (preview, error recovery, copy-sharing), domain model entity map, existing function signatures with line numbers, data flow diagrams                                                                                                                                                                    |
| `docs/plans/dryrun-preview/research-technical.md`       | Complete technical specification                             | Architecture data flow, `LaunchPreview` struct (Rust + TS), `validate_all()` spec, Tauri command API, `build_launch_preview()` implementation sketch, system constraints (performance, serialization, staleness, Steam Deck, CLI), files to create/modify list                                                                                                           |
| `docs/plans/dryrun-preview/research-ux.md`              | UX patterns and Steam Deck considerations                    | Modal vs panel analysis, accordion section design, Terraform-inspired summary banner, env var display patterns, command chain rendering, validation result severity icons, competitive analysis (Terraform/Docker/VS Code/Lutris/Steam), Steam Deck 1280x800 constraints, gamepad navigation, controller button prompts, copy-to-clipboard UX                            |
| `docs/plans/dryrun-preview/research-external.md`        | External tool comparison and UI libraries                    | Terraform plan output format (symbol system, JSON schema), Pulumi preview, Docker Compose dry-run, Ansible check/diff, game launcher patterns (SteamTinkerLaunch/Lutris/Bottles/Heroic), React UI libraries (syntax highlighting, terminal renderers, collapsible sections, JSON viewers, diff viewers), copy-to-clipboard patterns, Tauri IPC serialization constraints |
| `docs/plans/dryrun-preview/research-recommendations.md` | Implementation approach, alternatives, risk assessment       | Single `preview_launch` command recommendation (vs multi-command, dry_run flag, client-side), phased implementation plan (A=MVP, B=polish, C=cross-feature), risk matrix (technical, UX, maintenance), alternative approaches comparison table, cross-feature synergies (#36, #49, #38)                                                                                  |

### Project-Level Documentation

| File                                                        | Relevance                           | Key Contents for Implementers                                                                                                                                                                                           |
| ----------------------------------------------------------- | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CLAUDE.md`                                                 | **Project conventions** — must-read | Workspace architecture map, build commands, Tauri IPC pattern, TOML persistence, code conventions (Rust snake_case, React PascalCase), commit message requirements (conventional commits for git-cliff), label taxonomy |
| `docs/features/steam-proton-trainer-launch.doc.md`          | Launch method reference             | Three launch methods (steam_applaunch, proton_run, native), two-step launch flow, Steam Launch Options generation, trainer loading modes (SourceDirectory vs CopyToPrefix), optimization toggle behavior                |
| `docs/getting-started/quickstart.md`                        | User-facing context                 | Installation, profile creation, launch flow from user perspective — helps understand the user journey preview enhances                                                                                                  |
| `docs/research/additional-features/implementation-guide.md` | Feature prioritization context      | Shows #40 as Phase 1 priority alongside #39 (already done), dependency map, estimated effort, cross-feature relationships                                                                                               |
| `docs/research/additional-features/deep-research-report.md` | Strategic context                   | 8-perspective research on #40's value proposition (rated P0, 5/8 perspectives, "Codebase Ready"), positions preview as differentiation opportunity vs other Linux game launchers                                        |

---

## Development Guides

### Inline Code Documentation

These files contain critical doc comments that explain the architecture and behavior the preview must mirror:

| File                                                  | Line(s)  | Key Documentation                                                                                                                          |
| ----------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/launch/mod.rs`             | L1       | Module doc: `//! Launch orchestration primitives.` — lists all public re-exports that preview will consume                                 |
| `crates/crosshook-core/src/launch/request.rs`         | L1-60    | `LaunchRequest` struct definition with all fields, `SteamLaunchConfig`, `RuntimeLaunchConfig` — the input type for preview                 |
| `crates/crosshook-core/src/launch/request.rs`         | ~L74     | `resolved_method()` — auto-detection logic (empty → checks app_id/exe extension)                                                           |
| `crates/crosshook-core/src/launch/request.rs`         | ~L442    | `validate()` — fail-fast validation; preview needs companion `validate_all()`                                                              |
| `crates/crosshook-core/src/launch/optimizations.rs`   | L17-27   | `LaunchDirectives` struct — currently MISSING `Serialize`/`Deserialize` derives (one-line change needed)                                   |
| `crates/crosshook-core/src/launch/optimizations.rs`   | L183-187 | `resolve_launch_directives_for_method()` doc comment explaining env/wrapper alignment semantics                                            |
| `crates/crosshook-core/src/launch/optimizations.rs`   | L285-287 | `build_steam_launch_options_command()` doc comment — produces the `%command%` string                                                       |
| `crates/crosshook-core/src/launch/env.rs`             | L1-7     | `WINE_ENV_VARS_TO_CLEAR` doc comments — explains variable clearing strategy and sync requirement with shell scripts                        |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | L8       | `DEFAULT_HOST_PATH` doc comment                                                                                                            |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | ~L94     | `resolve_wine_prefix_path()` — pfx path resolution logic preview must replicate                                                            |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | ~L157    | `resolve_steam_client_install_path()` — cascade discovery logic                                                                            |
| `crates/crosshook-core/src/launch/script_runner.rs`   | L298-299 | `stage_trainer_into_prefix()` support file staging doc — preview must predict but NOT execute                                              |
| `src/types/launch.ts`                                 | L1-67    | TypeScript types for `LaunchRequest`, `LaunchPhase`, `LaunchValidationIssue`, `LaunchResult`, `LaunchFeedback` — all types preview extends |
| `src-tauri/src/commands/launch.rs`                    | L1-50    | Existing Tauri commands (`validate_launch`, `build_steam_launch_options_command`, `launch_game`) — the pattern preview follows             |
| `src-tauri/src/lib.rs`                                | L70-109  | `invoke_handler` registration — where `preview_launch` must be added                                                                       |

### Configuration Files

| File                                  | Relevance                                                                            |
| ------------------------------------- | ------------------------------------------------------------------------------------ |
| `src-tauri/tauri.conf.json`           | Window config: 1280x800, dark theme, AppImage target, resource bundling              |
| `src-tauri/capabilities/default.json` | Permissions: `core:default`, `dialog:default` — preview needs no new permissions     |
| `package.json`                        | Frontend dependencies — React 18, Vite, Tauri API v2; no new deps needed for preview |

---

## Must-Read Documents (Prioritized Reading List)

An implementer should read these in this order:

1. **`docs/plans/dryrun-preview/feature-spec.md`** — Start here. Contains the complete spec: data models, API design, UI patterns, task breakdown, and resolved design decisions.

2. **`CLAUDE.md`** — Project conventions, architecture map, build commands, code style. Required context for any code changes.

3. **`docs/plans/dryrun-preview/research-technical.md`** — Complete data model definitions, `build_launch_preview()` implementation sketch, `validate_all()` specification, files-to-create/modify matrix.

4. **`crates/crosshook-core/src/launch/request.rs`** — The `LaunchRequest` struct and `validate()` function that preview wraps. Read the validation dispatch logic to understand how `validate_all()` should work.

5. **`crates/crosshook-core/src/launch/optimizations.rs`** — `LaunchDirectives`, optimization definitions, `resolve_launch_directives()`. The one-line `Serialize`/`Deserialize` addition starts here.

6. **`src-tauri/src/commands/launch.rs`** — Existing Tauri command pattern. Preview command follows identical thin-wrapper pattern.

7. **`docs/plans/dryrun-preview/research-ux.md`** — Modal design, section layout, gamepad navigation, controller prompts. Essential for frontend implementation.

8. **`docs/plans/dryrun-preview/research-business.md`** — User stories, business rules, edge cases, domain model. Reference during validation logic implementation.

9. **`src/types/launch.ts`** — Existing TypeScript types the preview extends. New `LaunchPreview` interface goes here.

10. **`docs/plans/dryrun-preview/research-recommendations.md`** — Alternative approaches considered (and rejected), risk assessment, cross-feature synergies with #36/#49.

---

## Documentation Gaps

1. **No `validate_all()` specification in code** — The existing `validate()` is documented but `validate_all()` (the exhaustive collector) only exists in the research docs. The research-technical.md provides a sketch, but method-specific collector functions (`collect_steam_issues`, `collect_proton_issues`, `collect_native_issues`) need to be extracted from the existing `validate_steam_applaunch()` / `validate_proton_run()` / `validate_native()` functions in `request.rs`.

2. **No `to_display_toml()` specification** — The feature-spec mentions TOML-format clipboard output and a `to_display_toml()` method on `LaunchPreview`, but no format specification exists. The research-ux.md discusses plain-text and structured text formats but doesn't define the exact TOML structure.

3. **No `CollapsibleSection` component documentation** — The existing component (added in commit `79cba3c`) is referenced throughout the UX research but has no standalone documentation. Implementers need to read the component source directly.

4. **No `ProfileReviewModal` pattern documentation** — The modal infrastructure (focus trapping, gamepad nav, portal rendering) is referenced as the foundation for the preview modal but isn't documented. Implementers need to study the component source for the focus trap and gamepad integration patterns.

5. **No runtime_helpers.rs preview adaptation guide** — The `apply_host_environment()`, `apply_proton_environment()` etc. functions in `runtime_helpers.rs` mutate `Command` objects. The preview needs pure equivalents that return `PreviewEnvVar` vectors. The technical research says "mirror these as pure functions" but doesn't document the exact mapping.

6. **No integration test specification** — The feature-spec mentions integration tests to prevent frontend type drift but doesn't specify test scenarios. The `cargo test -p crosshook-core` framework exists but specific preview test cases need design.

---

## Related Feature Plans (For Context)

These plans follow the same research/plan/implement pattern and demonstrate established conventions:

| Plan Directory                     | Feature              | Relevance                                                                                                                   |
| ---------------------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `docs/plans/proton-optimizations/` | Launch optimizations | Most architecturally similar — added `LaunchDirectives`, optimization definitions, and UI toggles in the same launch module |
| `docs/plans/ui-enhancements/`      | UI improvements      | Established the `CollapsibleSection` component and dark theme patterns preview builds on                                    |
| `docs/plans/profile-modal/`        | Profile review modal | Established the modal infrastructure (focus trap, gamepad nav) preview reuses                                               |
