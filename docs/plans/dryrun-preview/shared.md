# Dry Run / Preview Launch Mode

The preview feature adds a read-only `preview_launch` Tauri command that calls existing pure functions in `crosshook-core` -- `validate()`, `resolve_launch_directives()`, `build_steam_launch_options_command()` -- and assembles their outputs into a `LaunchPreview` struct displayed in a modal dialog. The primary backend gap is `validate()` being fail-fast (returns first error only); a new `validate_all()` collector function is needed for exhaustive validation. The frontend adds a "Preview Launch" button in `LaunchPanel.tsx`, a `usePreviewState` hook, and a preview modal with `CollapsibleSection` accordions reusing the `ProfileReviewModal` focus-trap/portal infrastructure. Environment collection requires new pure functions in `preview.rs` that mirror the `Command`-mutating helpers in `runtime_helpers.rs` but return tagged `PreviewEnvVar` data instead.

## Relevant Files

### Files to Create

- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`: Core preview module -- `LaunchPreview` struct, `PreviewEnvVar`, `EnvVarSource`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation`, `build_launch_preview()`, env collection helpers, `to_display_toml()`
- `src/crosshook-native/src/hooks/usePreviewState.ts`: React hook for preview invocation/state (loading, preview, error) using `useState` pattern like `useProfile.ts`

### Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs`: Add `pub mod preview;` and re-export `build_launch_preview`, `LaunchPreview`
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: Add `validate_all()` function with method-specific collectors (`collect_steam_issues`, `collect_proton_issues`, `collect_native_issues`)
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`: Add `Serialize, Deserialize` to `LaunchDirectives` derive (one-line change at line 17)
- `src/crosshook-native/src-tauri/src/commands/launch.rs`: Add sync `preview_launch` Tauri command wrapping `build_launch_preview()`
- `src/crosshook-native/src-tauri/src/lib.rs`: Register `commands::launch::preview_launch` in `invoke_handler` after line 90
- `src/crosshook-native/src/types/launch.ts`: Add `LaunchPreview`, `PreviewEnvVar`, `EnvVarSource`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation` interfaces
- `src/crosshook-native/src/components/LaunchPanel.tsx`: Add "Preview Launch" ghost button in `__actions` div + preview modal trigger and display

### Existing Files for Pattern Reference

- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: `LaunchRequest` (line 16-37), `validate()` (line 442-454), `ValidationError` enum (line 158-199), `LaunchValidationIssue` (line 151-156), helper validators (`require_game_path_if_needed`, etc.)
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`: `LaunchDirectives` (line 17-21), `resolve_launch_directives()` (line 267-283), `build_steam_launch_options_command()` (line 288-300), `LAUNCH_OPTIMIZATION_DEFINITIONS` (line 38-175)
- `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`: `WINE_ENV_VARS_TO_CLEAR` (31 vars), `REQUIRED_PROTON_VARS` (3), `LAUNCH_OPTIMIZATION_ENV_VARS` (14), `PASSTHROUGH_DISPLAY_VARS` (4)
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: `resolve_wine_prefix_path()` (line 94), `resolve_steam_client_install_path()` (line 157), `apply_host_environment()` (line 46), `apply_runtime_proton_environment()` (line 62)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: `stage_trainer_into_prefix()` (line 227-266) -- side-effecting, preview must replicate path computation only; `STAGED_TRAINER_ROOT = "CrossHook/StagedTrainers"` (line 22)
- `src/crosshook-native/src-tauri/src/commands/launch.rs`: `validate_launch` (line 25-28) -- sync thin-wrapper pattern to follow; `launch_game` (line 38-68); `LaunchResult` struct (line 18-23)
- `src/crosshook-native/src/hooks/useLaunchState.ts`: Reducer-based state machine -- reference pattern but preview uses simpler `useState`
- `src/crosshook-native/src/components/ui/CollapsibleSection.tsx`: `<details>`/`<summary>` wrapper with `title`, `defaultOpen`, `open`, `onToggle`, `meta` props; BEM classes `crosshook-collapsible__*`
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: Modal with portal rendering, focus trap (Tab cycling), `data-crosshook-focus-root="modal"`, backdrop dismiss, inert siblings, body scroll lock, `statusTone` prop
- `src/crosshook-native/src/components/pages/LaunchPage.tsx`: `buildLaunchRequest()` (line 10-43) constructs `LaunchRequest` from profile; returns `null` if game path empty
- `src/crosshook-native/src/types/launch.ts`: `LaunchRequest`, `LaunchPhase`, `LaunchValidationIssue`, `LaunchFeedback`, `LaunchResult` -- new types go alongside these
- `src/crosshook-native/src/styles/variables.css`: CSS custom properties -- `--crosshook-color-success` (#28c76f), `--crosshook-color-warning` (#f5c542), `--crosshook-color-danger` (#ff758f), `--crosshook-font-mono`, `--crosshook-touch-target-min` (48px)

## Relevant Patterns

**Tauri Command (Sync Thin Wrapper)**: Preview follows the `validate_launch` pattern -- sync function, takes `LaunchRequest`, returns `Result<T, String>`, calls `crosshook-core` function with `.map_err(|e| e.to_string())`. See [src/crosshook-native/src-tauri/src/commands/launch.rs](src/crosshook-native/src-tauri/src/commands/launch.rs) lines 25-28.

**Serde Enum Serialization**: Enums crossing IPC use `#[serde(rename_all = "snake_case")]` producing TypeScript string unions. See `ValidationSeverity` at [request.rs](src/crosshook-native/crates/crosshook-core/src/launch/request.rs) line 143-148. `EnvVarSource` must follow this exact pattern.

**Serde Struct Serialization**: IPC structs have no `rename_all` -- Rust's native `snake_case` matches TypeScript directly. Input types derive `Serialize + Deserialize + Default` with `#[serde(default)]` on fields. Output-only types derive `Serialize` only. See `LaunchResult` at [commands/launch.rs](src/crosshook-native/src-tauri/src/commands/launch.rs) line 18-23.

**React Hook (Simple useState)**: Preview hook uses `useState` for `{ status, preview, error }` -- simpler than `useLaunchState`'s reducer. Follow `useProfile.ts` pattern: individual `useState` hooks, `invoke()` in async function, error normalization. See [src/crosshook-native/src/hooks/useProfile.ts](src/crosshook-native/src/hooks/useProfile.ts).

**Validation Dispatch**: `validate()` dispatches by `resolved_method()` to method-specific functions. `validate_all()` mirrors this dispatch but uses `Vec<LaunchValidationIssue>` collector instead of `Result<(), ValidationError>` short-circuit. See [request.rs](src/crosshook-native/crates/crosshook-core/src/launch/request.rs) line 442-454.

**CollapsibleSection Accordion**: Native `<details>`/`<summary>` with `defaultOpen` for sections expanded by default, `meta` slot for count badges. See [CollapsibleSection.tsx](src/crosshook-native/src/components/ui/CollapsibleSection.tsx).

**Modal Infrastructure**: Portal rendering, focus trap, gamepad nav via `data-crosshook-focus-root="modal"`, inert sibling management. See [ProfileReviewModal.tsx](src/crosshook-native/src/components/ProfileReviewModal.tsx).

**BEM CSS + Data Attributes**: Classes follow `crosshook-{component}__{element}--{modifier}`. State-driven styling uses `data-severity`, `data-phase`, etc. See [LaunchPanel.tsx](src/crosshook-native/src/components/LaunchPanel.tsx) line 120-137.

**Module Re-export**: New modules are declared with `pub mod` in `mod.rs` and key types re-exported with `pub use`. See [launch/mod.rs](src/crosshook-native/crates/crosshook-core/src/launch/mod.rs).

**Test Fixtures**: Tests use `tempfile::TempDir` for filesystem fixtures, factory functions like `steam_request()` / `proton_request()`, and `ScopedCommandSearchPath` for PATH isolation. See [request.rs](src/crosshook-native/crates/crosshook-core/src/launch/request.rs) line 655+.

## Relevant Docs

**docs/plans/dryrun-preview/feature-spec.md**: You _must_ read this first -- it is the single source of truth for the feature: complete data models, API design, business rules, UX patterns, resolved decisions, and task breakdown.

**CLAUDE.md**: You _must_ read this when working on any code changes -- project architecture, build commands, Tauri IPC patterns, code conventions, commit message requirements.

**docs/plans/dryrun-preview/research-technical.md**: You _must_ read this when implementing backend types and the Tauri command -- contains complete Rust struct definitions, `build_launch_preview()` implementation sketch, and `validate_all()` specification.

**docs/plans/dryrun-preview/research-ux.md**: You _must_ read this when implementing the frontend modal -- modal vs panel analysis, accordion layout, Terraform summary banner, Steam Deck gamepad navigation, controller button prompts.

**docs/plans/dryrun-preview/research-business.md**: You _must_ read this when implementing validation logic -- 9 business rules, 5 edge case rules, existing function signatures with line numbers, domain model entity map.

**docs/plans/dryrun-preview/research-patterns.md**: You _must_ read this when following codebase conventions -- Tauri command patterns, serde conventions, React hook patterns, CSS design system, testing patterns.

**docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when working on method-specific preview sections -- documents the three launch methods, two-step flow, trainer loading modes.

## Gotchas

- **`chrono` not in workspace**: Feature spec assumes `chrono::Utc::now().to_rfc3339()` but `chrono` is not in `Cargo.toml`. Either add `chrono = "0.4"` or use `std::time::SystemTime` with manual formatting.
- **`validate_proton_run()` calls `resolve_launch_directives()`**: At `request.rs:505`, proton validation internally calls directive resolution. For `validate_all()`, directive errors must be collected alongside path errors, not treated separately.
- **Steam vs Proton env differ**: `apply_steam_proton_environment()` (script_runner.rs) hardcodes `compatdata_path + "/pfx"` while `apply_runtime_proton_environment()` (runtime_helpers.rs) uses `resolve_wine_prefix_path()` heuristic. Preview must use the correct resolution per method.
- **`stage_trainer_into_prefix()` has side effects**: File copies at script_runner.rs:227-266. Preview computes staged path via string manipulation: `C:\CrossHook\StagedTrainers\{stem}\{filename}`.
- **`LAUNCH_OPTIMIZATION_DEFINITIONS` is private**: Cannot be iterated from `preview.rs`, but `resolve_launch_directives()` is public and returns the resolved output.
- **All validation severities are Fatal**: `ValidationError::severity()` always returns `Fatal`. Consider introducing `Warning` for non-blocking issues (missing optional wrapper).
