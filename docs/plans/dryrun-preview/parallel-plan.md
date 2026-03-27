# Dry Run / Preview Launch Mode Implementation Plan

This feature adds a read-only `preview_launch` Tauri command that assembles outputs from existing pure functions (`validate()`, `resolve_launch_directives()`, `build_steam_launch_options_command()`) into a `LaunchPreview` struct, displayed in a modal dialog with collapsible sections. Two files are created (`preview.rs`, `usePreviewState.ts`) and seven files are modified. The primary backend work is a new `validate_all()` collector function (existing `validate()` is fail-fast) and pure environment collection helpers in `preview.rs` that mirror the `Command`-mutating runtime helpers. The frontend reuses `ProfileReviewModal` focus-trap/portal infrastructure with `CollapsibleSection` accordions.

## Critically Relevant Files and Documentation

- docs/plans/dryrun-preview/feature-spec.md: Complete feature specification — data models, business rules, UX design, resolved decisions
- docs/plans/dryrun-preview/research-technical.md: Rust/TypeScript data models, Tauri command spec, `build_launch_preview()` implementation sketch
- docs/plans/dryrun-preview/research-ux.md: Modal design, accordion sections, Steam Deck gamepad nav, controller prompts
- docs/plans/dryrun-preview/research-business.md: User stories, validation logic, domain model, existing function signatures
- docs/plans/dryrun-preview/research-patterns.md: Tauri command patterns, serde conventions, React hook patterns, CSS/BEM conventions
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: `LaunchRequest`, `validate()`, `ValidationError` (26 variants), `LaunchValidationIssue`, helper validators
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs: `LaunchDirectives`, `resolve_launch_directives()`, `build_steam_launch_options_command()`
- src/crosshook-native/crates/crosshook-core/src/launch/env.rs: Environment variable constant arrays (WINE_ENV_VARS_TO_CLEAR, REQUIRED_PROTON_VARS, etc.)
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: `resolve_wine_prefix_path()`, `apply_host_environment()`, `apply_runtime_proton_environment()`
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: `stage_trainer_into_prefix()` (side-effecting — preview replicates path computation only), `STAGED_TRAINER_ROOT`
- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs: Module declarations and re-exports
- src/crosshook-native/src-tauri/src/commands/launch.rs: Existing Tauri commands (`validate_launch` pattern to follow)
- src/crosshook-native/src-tauri/src/lib.rs: Command registration in `invoke_handler`
- src/crosshook-native/src/types/launch.ts: Existing TypeScript launch types
- src/crosshook-native/src/hooks/useLaunchState.ts: Launch state machine (reference for hook pattern)
- src/crosshook-native/src/components/LaunchPanel.tsx: Launch UI — where preview button and modal go
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Accordion component for preview sections
- src/crosshook-native/src/components/ProfileReviewModal.tsx: Modal infrastructure (portal, focus trap, gamepad nav)
- src/crosshook-native/src/components/pages/LaunchPage.tsx: `buildLaunchRequest()` — constructs the request preview uses
- src/crosshook-native/src/styles/variables.css: CSS custom properties for colors, fonts, touch targets

## Implementation Plan

### Phase 1: Foundation Types

#### Task 1.1: Backend types and module wiring Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/dryrun-preview/feature-spec.md (Technical Specifications → Data Models section)
- docs/plans/dryrun-preview/research-technical.md (Data Models section)
- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (lines 143-156 for ValidationSeverity and LaunchValidationIssue patterns)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/preview.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs

Define all preview data types in new `preview.rs` module:

1. **`EnvVarSource` enum** — `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` with `#[serde(rename_all = "snake_case")]`. Variants: `ProtonRuntime`, `LaunchOptimization`, `Host`, `SteamProton`. Follow exact `ValidationSeverity` pattern at `request.rs:143-149`.

2. **Structs** — all with `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`:
   - `PreviewEnvVar { key: String, value: String, source: EnvVarSource }`
   - `ProtonSetup { wine_prefix_path: String, compat_data_path: String, steam_client_install_path: String, proton_executable: String }`
   - `PreviewTrainerInfo { path: String, host_path: String, loading_mode: String, staged_path: Option<String> }`
   - `PreviewValidation { passed: bool, issues: Vec<LaunchValidationIssue> }`
   - `LaunchPreview` — the main struct with all fields from the feature spec (resolved_method, validation, environment as `Option<Vec<PreviewEnvVar>>`, cleared_variables, wrappers as `Option<Vec<String>>`, effective_command as `Option<String>`, directives_error as `Option<String>`, steam_launch_options, proton_setup, working_directory, game_executable, game_executable_name, trainer as `Option<PreviewTrainerInfo>`, generated_at, **display_text: String** — pre-rendered TOML output for clipboard copy, populated by `to_display_toml()` in `build_launch_preview()`)

3. **Module wiring** — in `mod.rs`, add `pub mod preview;` and `pub use preview::{build_launch_preview, LaunchPreview};`

4. **LaunchDirectives Serde** — in `optimizations.rs` line 17, change `#[derive(Debug, Clone, PartialEq, Eq, Default)]` to `#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]`. Add `use serde::{Serialize, Deserialize};` if not already imported.

5. **Chrono dependency** — check if `chrono` is in `crosshook-core/Cargo.toml`. If not, add `chrono = "0.4"` under `[dependencies]` for the `generated_at` timestamp. Alternative: use `std::time::SystemTime` with manual ISO 8601 formatting (see `src-tauri/src/commands/shared.rs` for existing `SystemTime` usage pattern).

Verify: `cargo check -p crosshook-core` compiles with no errors.

#### Task 1.2: Frontend TypeScript types Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/dryrun-preview/feature-spec.md (Technical Specifications → TypeScript section)
- src/crosshook-native/src/types/launch.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/types/launch.ts

Add TypeScript interfaces at the end of the file, alongside existing launch types:

1. `EnvVarSource` — string union type: `'proton_runtime' | 'launch_optimization' | 'host' | 'steam_proton'`
2. `PreviewEnvVar` — `{ key: string; value: string; source: EnvVarSource }`
3. `ProtonSetup` — `{ wine_prefix_path: string; compat_data_path: string; steam_client_install_path: string; proton_executable: string }`
4. `PreviewTrainerInfo` — `{ path: string; host_path: string; loading_mode: string; staged_path: string | null }`
5. `PreviewValidation` — `{ passed: boolean; issues: LaunchValidationIssue[] }`
6. `LaunchPreview` — full interface with all fields. `Option<T>` maps to `T | null`. Use `LaunchValidationIssue` from existing types. `resolved_method` typed as `'steam_applaunch' | 'proton_run' | 'native'`. Include `display_text: string` — pre-rendered TOML output for clipboard copy.

Verify: TypeScript compilation passes (no IDE errors).

### Phase 2: Core Backend Logic

#### Task 2.1: Exhaustive validation collector (`validate_all`) Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/dryrun-preview/research-business.md (Existing Codebase Analysis → Function Signatures section)
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (full file — understand validate() at line 442, method-specific validators at lines 456-524, helper functions, ValidationError enum at line 158)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs

Implement `validate_all()` and method-specific collector functions:

1. **`pub fn validate_all(request: &LaunchRequest) -> Vec<LaunchValidationIssue>`** — mirrors `validate()` dispatch at line 442 but collects all issues. Check method string validity first (push `UnsupportedMethod` issue if invalid, return early since method dispatch is impossible). Then dispatch to method-specific collectors.

2. **`fn collect_steam_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>)`** — mirrors `validate_steam_applaunch()` (line 456). Call each check independently (game path, trainer paths, app_id, compatdata_path, proton_path, steam_client_path), converting each `Err` to a pushed issue. Also call `reject_launch_optimizations_for_method()` and push any error.

3. **`fn collect_proton_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>)`** — mirrors `validate_proton_run()` (line 487). **Critical gotcha**: `validate_proton_run()` calls `resolve_launch_directives()` at line 505. The collector must `match` on the directive result and push the error as an issue (not propagate it). Collect game path, trainer paths, prefix path, proton path checks independently.

4. **`fn collect_native_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>)`** — mirrors `validate_native()` (line 510). Check trainer_only rejection, game path, .exe rejection, optimization rejection.

Pattern for each check: call the existing helper (e.g., `require_game_path_if_needed(request, must_exist)`), and if it returns `Err(e)`, push `e.issue()` to the vec. This avoids duplicating the check logic.

Add `validate_all` to the `pub use` re-exports in `mod.rs`.

Add inline unit tests in the same `#[cfg(test)] mod tests` block:

- Test that `validate_all()` returns empty vec for a valid steam request
- Test that `validate_all()` collects multiple issues when multiple fields are invalid
- Test that proton request with missing wrapper binary collects the directive error alongside other issues

Verify: `cargo test -p crosshook-core` passes.

#### Task 2.2: Build launch preview function and env helpers Depends on [1.1, 2.1]

**READ THESE BEFORE TASK**

- docs/plans/dryrun-preview/research-technical.md (Core Preview Function section — contains implementation sketch)
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs (apply_host_environment at line 46, apply_runtime_proton_environment at line 62, resolve_wine_prefix_path at line 94, env_value at line 184)
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs (stage_trainer_into_prefix at line 227 — path computation logic only, STAGED_TRAINER_ROOT at line 22)
- src/crosshook-native/crates/crosshook-core/src/launch/env.rs (WINE_ENV_VARS_TO_CLEAR, REQUIRED_PROTON_VARS)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/preview.rs (extend from Task 1.1)
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs (change `fn env_value` to `pub(crate) fn env_value` — 1-char visibility change so preview env helpers can reuse it instead of duplicating)

Implement the core preview function and all helper functions in `preview.rs`:

1. **`pub fn build_launch_preview(request: &LaunchRequest) -> Result<LaunchPreview, String>`** — the aggregate function. Calls:
   - `request.resolved_method()` for method string
   - `validate_all(request)` for exhaustive validation
   - `resolve_launch_directives(request)` — capture `Ok(directives)` or `Err` in `directives_error`
   - Build environment, wrappers, command from directives (or set to `None` on failure)
   - Build proton_setup, trainer_info, working_directory independently
   - Generate `generated_at` timestamp (chrono or SystemTime)
   - Assemble and return `LaunchPreview`

2. **Environment collection helpers** (each returns `Vec<PreviewEnvVar>`):
   - `collect_host_environment()` — reads `std::env::var()` for HOME, USER, LOGNAME, SHELL, PATH, DISPLAY, WAYLAND_DISPLAY, XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS. Tags all as `EnvVarSource::Host`.
   - `collect_runtime_proton_environment(request)` — resolves WINEPREFIX via `resolve_wine_prefix_path()`, STEAM_COMPAT_DATA_PATH, STEAM_COMPAT_CLIENT_INSTALL_PATH via `resolve_steam_client_install_path()`. Tags as `EnvVarSource::ProtonRuntime`.
   - `collect_steam_proton_environment(request)` — for steam_applaunch: WINEPREFIX = `{compatdata}/pfx`, STEAM_COMPAT_DATA_PATH = `{compatdata}`, STEAM_COMPAT_CLIENT_INSTALL_PATH. Tags as `EnvVarSource::SteamProton`. **Note**: resolution differs from proton_run — hardcoded `/pfx` join, not `resolve_wine_prefix_path()`.
   - `collect_optimization_environment(directives)` — maps `directives.env` pairs to `PreviewEnvVar` tagged as `EnvVarSource::LaunchOptimization`.

3. **`build_effective_command_string(request, method, directives)`** — builds human-readable command:
   - `proton_run`: `[wrappers...] /path/to/proton run /path/to/game.exe`
   - `steam_applaunch`: the `build_steam_launch_options_command()` output
   - `native`: just the game path

4. **`build_proton_setup(request, method)`** — returns `Option<ProtonSetup>`. `None` for native. Resolves wine_prefix_path, compat_data_path, steam_client_install_path, proton_executable from request fields.

5. **`build_trainer_info(request, method)`** — returns `Option<PreviewTrainerInfo>`. `None` if trainer_path is empty. For `copy_to_prefix` mode, compute staged path: `C:\CrossHook\StagedTrainers\{stem}\{filename}` without calling `stage_trainer_into_prefix()`.

6. **`resolve_working_directory(request)`** — mirrors `runtime_helpers.rs:apply_working_directory()` logic: configured directory if non-empty, else parent of game path.

7. **`to_display_toml(&self) -> String`** on `LaunchPreview` — renders preview as structured TOML-like text for clipboard. Include sections for [preview], [validation], [environment], [command], [proton] matching profile data conventions.

Verify: `cargo check -p crosshook-core` compiles.

### Phase 3: IPC Integration

#### Task 3.1: Tauri command and registration Depends on [2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs (validate_launch at line 25 — pattern to follow)
- src/crosshook-native/src-tauri/src/lib.rs (invoke_handler at line 70)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/launch.rs
- src/crosshook-native/src-tauri/src/lib.rs

1. **Import** in `commands/launch.rs`: Add `build_launch_preview` and `LaunchPreview` to the `use crosshook_core::launch::{...}` import block. Use alias if needed (e.g., `build_launch_preview as build_launch_preview_core`) to avoid name collisions.

2. **Command** in `commands/launch.rs`: Add sync `preview_launch` following the `validate_launch` thin-wrapper pattern:

   ```rust
   #[tauri::command]
   pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
       build_launch_preview(&request).map_err(|error| error.to_string())
   }
   ```

3. **Registration** in `lib.rs`: Add `commands::launch::preview_launch` to the `invoke_handler` macro call, in the launch commands group (after line 90).

Verify: `cargo check` on the Tauri app compiles.

#### Task 3.2: Frontend preview state hook Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProfile.ts (simple useState hook pattern)
- src/crosshook-native/src/hooks/useLaunchState.ts (reference for invoke pattern, error normalization)
- src/crosshook-native/src/types/launch.ts (LaunchPreview and LaunchRequest types)

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/usePreviewState.ts

Create a simple `useState`-based hook (NOT reducer pattern):

1. **State**: `loading: boolean`, `preview: LaunchPreview | null`, `error: string | null`
2. **`requestPreview(request: LaunchRequest)`** — async function: set loading, call `invoke<LaunchPreview>('preview_launch', { request })`, set preview on success, set error on failure with `err instanceof Error ? err.message : String(err)` normalization.
3. **`clearPreview()`** — reset to initial state.
4. **Return** flat object: `{ loading, preview, error, requestPreview, clearPreview }`.

Follow `useProfile.ts` pattern for error handling and state management. Import `invoke` from `@tauri-apps/api/core` and `LaunchPreview`, `LaunchRequest` from `../types`.

Verify: Import resolves in editor without TypeScript errors.

### Phase 4: Frontend UI

#### Task 4.1: Preview button in LaunchPanel Depends on [3.1, 3.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LaunchPanel.tsx (full file — understand structure, BEM classes, action buttons at lines 101-118)
- src/crosshook-native/src/components/pages/LaunchPage.tsx (buildLaunchRequest at line 10 — the null guard)
- docs/plans/dryrun-preview/research-ux.md (Button placement section)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LaunchPanel.tsx

1. **Import** `usePreviewState` hook and `LaunchPreview` type.

2. **Add hook** in `LaunchPanel` component: `const { loading, preview, error, requestPreview, clearPreview } = usePreviewState();`

3. **Add "Preview Launch" button** in the `crosshook-launch-panel__actions` div (lines 101-118), after the primary launch button. Use `crosshook-button--ghost` class. Disabled when `!request || phase !== 'idle' || loading`. onClick calls `requestPreview(request)`.

4. **Add state for modal visibility**: `const [showPreview, setShowPreview] = useState(false)`. Open modal when preview succeeds (use `useEffect` watching `preview`). Close on modal dismiss.

5. **Guard alignment**: Preview button disabled state must exactly match the launch button guard — disabled when `buildLaunchRequest()` returns null (no request prop) or launch is active (`phase !== Idle`).

Verify: Button appears in UI, disabled states work correctly.

#### Task 4.2: Preview modal display Depends on [4.1]

**READ THESE BEFORE TASK**

- docs/plans/dryrun-preview/research-ux.md (full file — modal structure, accordion sections, severity indicators, Steam Deck)
- src/crosshook-native/src/components/ProfileReviewModal.tsx (portal, focus trap, gamepad nav patterns)
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx (accordion component API)
- src/crosshook-native/src/styles/variables.css (color tokens, font-mono, touch-target-min)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LaunchPanel.tsx (extend from Task 4.1)

Render the preview modal when `showPreview && preview` is truthy. Build the modal following `ProfileReviewModal` patterns (portal rendering, focus trap, `data-crosshook-focus-root="modal"`, backdrop dismiss, inert siblings):

1. **Summary banner** (always visible, not collapsible): Profile name, resolved method, game executable. Terraform-style summary line: "Preview: N env vars, N wrappers, N checks passed, N warnings". Color-code counts using `--crosshook-color-success/warning/danger`.

2. **Validation Results** (`CollapsibleSection`, `defaultOpen={true}`): List all `preview.validation.issues` with severity icons (checkmark/warning/X) and `data-severity` attributes. Group by severity: errors first, then warnings, then info/passes.

3. **Command Chain** (`CollapsibleSection`, `defaultOpen={true}`): Display `preview.effective_command` in a monospace `<pre>` block with `--crosshook-color-surface` background. Show `preview.steam_launch_options` separately for steam_applaunch. Show `preview.directives_error` if present.

4. **Environment Variables** (`CollapsibleSection`, `defaultOpen={false}`): Key-value table of `preview.environment` with monospace font. Group by `source` tag. Show count in section `meta` slot. Show `preview.cleared_variables` as a separate sub-list.

5. **Proton / Runtime Setup** (`CollapsibleSection`, `defaultOpen={false}`): Show `preview.proton_setup` fields. **Hide entirely for `native` method** (BR-8). Show trainer info if present.

6. **Footer actions**:
   - "Copy Preview" (ghost button): calls `navigator.clipboard.writeText(preview.display_text)` — the `display_text` field is pre-rendered TOML from the backend, no JS rendering needed.
   - "Launch Now" (primary button, disabled if `!preview.validation.passed`): closes the modal (`setShowPreview(false)`, `clearPreview()`), then calls the existing `launchGame()` from `useLaunchState`. This leverages the existing launch state machine — the preview modal is just an inspection step before the same launch flow.
   - "Close" (ghost button): closes modal without launching.

7. **CSS**: Use existing CSS custom properties (`--crosshook-color-success/warning/danger`, `--crosshook-font-mono`, `--crosshook-color-surface`). Add preview-specific styles inline in `LaunchPanel.tsx` or in a `<style>` block using BEM classes (`crosshook-preview-modal__*`). No separate CSS file needed if styles are minimal; if >30 lines of CSS, create `src/crosshook-native/src/styles/preview.css` and import it.

8. **Gamepad**: Set `data-crosshook-focus-root="modal"` on modal surface. Ensure 48px min touch targets. Focus trap cycles through sections to footer.

9. **Staleness**: Show `preview.generated_at` timestamp. Consider subtle indicator if >60 seconds old.

Verify: Modal opens with real preview data, sections expand/collapse, gamepad navigation works.

### Phase 5: Testing

#### Task 5.1: Unit tests for preview backend Depends on [2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (test module at line 655+ — fixture pattern)
- src/crosshook-native/crates/crosshook-core/src/launch/preview.rs (the implementation to test)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/preview.rs (add `#[cfg(test)] mod tests`)

Add comprehensive unit tests using existing fixture patterns:

1. **`preview_shows_resolved_method_for_steam_applaunch`** — steam request produces `resolved_method == "steam_applaunch"`
2. **`preview_shows_resolved_method_for_proton_run`** — proton request produces `resolved_method == "proton_run"`
3. **`preview_shows_resolved_method_for_native`** — native request produces `resolved_method == "native"`
4. **`preview_validation_passes_for_valid_request`** — valid request has `validation.passed == true` and empty issues
5. **`preview_validation_collects_multiple_issues`** — request with multiple invalid fields returns all issues (not just first)
6. **`preview_returns_partial_results_on_directive_failure`** — missing wrapper binary: `directives_error` is `Some(...)`, `environment` is `None`, but `validation` and `game_executable` are still populated
7. **`preview_trainer_info_with_copy_to_prefix`** — `copy_to_prefix` mode produces `staged_path == Some("C:\\CrossHook\\StagedTrainers\\{stem}\\{filename}")`
8. **`preview_hides_proton_for_native`** — native method has `proton_setup == None`
9. **`preview_includes_steam_launch_options`** — steam_applaunch includes `steam_launch_options == Some(...)`
10. **`preview_generated_at_is_recent`** — `generated_at` parses as a valid ISO 8601 timestamp

Use existing fixture factories: `steam_request()`, `proton_request()`, `native_request()` from `request.rs` tests. May need to make them `pub(crate)` or duplicate.

Use `ScopedCommandSearchPath` from `test_support` for tests involving binary availability checks.

Verify: `cargo test -p crosshook-core -- preview` passes all tests.

## Advice

- **`validate_proton_run()` chains into `resolve_launch_directives()`** at `request.rs:505`. The `validate_all()` collector for proton must catch this as a pushed issue, not a propagated error. This is the subtlest correctness requirement in the feature.

- **Steam vs Proton environment resolution differs**: `steam_applaunch` hardcodes `{compatdata}/pfx` for WINEPREFIX while `proton_run` uses `resolve_wine_prefix_path()` which checks if the path already ends in "pfx". Preview must use the correct resolution per method — don't unify them.

- **`LAUNCH_OPTIMIZATION_DEFINITIONS` is private**: Don't try to iterate it from `preview.rs`. Use `resolve_launch_directives()` which is public and returns the resolved `LaunchDirectives` — this gives you the env vars and wrappers without accessing the private array.

- **`stage_trainer_into_prefix()` is side-effecting**: Preview MUST NOT call this. Compute the staged path via string manipulation: `C:\CrossHook\StagedTrainers\{file_stem}\{file_name}` where `file_stem` is `Path::file_stem()` and `file_name` is `Path::file_name()` of the trainer host path.

- **The `to_display_toml()` method needs design**: The feature spec says TOML format for clipboard, but no exact format is defined. Model it on the profile TOML structure — `[preview]` header with method/game/timestamp, `[validation]` with check results, `[environment]` with key-value pairs, `[command]` with the effective string. Make it valid TOML that's also human-readable in Discord code blocks.

- **Test fixtures may need visibility changes**: The `steam_request()`, `proton_request()`, `native_request()` factory functions in `request.rs` tests are private to that test module. Either duplicate them in `preview.rs` tests, or change their visibility to `pub(crate)` and move to `test_support.rs`.

- **`chrono` vs `SystemTime`**: Check `shared.rs` in `src-tauri/src/commands/` — it uses `SystemTime` for timestamps, confirming the codebase doesn't depend on `chrono`. Using `SystemTime` avoids a new dependency but requires manual ISO 8601 formatting. Either approach is acceptable.

- **Frontend TOML rendering is resolved**: The `display_text: String` field on `LaunchPreview` is pre-rendered by `to_display_toml()` in the backend. The frontend simply reads `preview.display_text` for clipboard copy — no JS-side TOML rendering needed.
