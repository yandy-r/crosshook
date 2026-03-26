# Update Game Parallel Plan -- Task Quality Evaluation

## Task Quality Summary

| Metric                       | Count |
| ---------------------------- | ----- |
| **Total Tasks**              | 11    |
| **High Quality**             | 7     |
| **Needs Minor Improvements** | 3     |
| **Needs Significant Work**   | 1     |

Overall the plan is strong. File paths are real and verified against the current codebase. Line number references are accurate. The dependency graph is correct and the parallelization opportunities are well-identified. The Advice section at the bottom catches several non-obvious pitfalls. Most issues found are minor gaps or ambiguities rather than structural problems.

---

## Detailed Findings

### Task 1.1: Create update data models

**Verdict: High Quality**

| Criterion               | Pass | Notes                                                                         |
| ----------------------- | ---- | ----------------------------------------------------------------------------- |
| Clear Purpose           | Yes  | Creates the four core types for the update module                             |
| Specific File Changes   | Yes  | Single file: `update/models.rs`                                               |
| Actionable Instructions | Yes  | Every field, every variant, every derive macro, every trait impl is specified |
| Gotchas Documented      | Yes  | `#[serde(rename_all = "snake_case")]` noted; `From` impl included             |
| Appropriate Scope       | Yes  | One file, ~200 lines estimated                                                |

No issues found. The data model matches the feature spec exactly. The instruction to use `#[serde(default)]` on every field mirrors `install/models.rs` correctly. All 10 validation error variants and 6 error variants are listed.

---

### Task 1.2: Create update service with validation and command building

**Verdict: Needs Minor Improvements**

| Criterion               | Pass    | Notes                                                            |
| ----------------------- | ------- | ---------------------------------------------------------------- |
| Clear Purpose           | Yes     | Implements the three core functions plus private helpers         |
| Specific File Changes   | Yes     | Single file: `update/service.rs`                                 |
| Actionable Instructions | Yes     | Function signatures, parameter order, helper calls all specified |
| Gotchas Documented      | Partial | See below                                                        |
| Appropriate Scope       | Yes     | One file with tests, estimated ~300 lines                        |

**Issues:**

1. **Validation semantics differ from install but this is not called out explicitly.** The install module's `validate_prefix_path` (install/service.rs line 226-239) does NOT check `PrefixPathMissing` -- it silently passes if the prefix does not exist because install creates prefixes. The update module MUST return `PrefixPathMissing` if the prefix does not exist, since updates require an existing prefix. The plan lists the 10 variants (which implicitly covers this) but does not flag the behavioral difference from the install template. A developer copying from install could miss this.

2. **The `update_game` return type `Result<(UpdateGameResult, tokio::process::Child), UpdateGameError>` is unusual.** Returning a `tokio::process::Child` from a synchronous function is fine (it is just a handle), but the plan should note that `update_game` must NOT call `child.wait()` like `install_game` does (install/service.rs line 62). The Advice section mentions this but the task instruction body should reinforce it since a developer reading only Task 1.2 may not read the Advice section first.

3. **The `is_windows_executable` / `is_executable_file` duplication-vs-extraction choice is left open.** The plan says "choose whichever is simpler" which is fine for experienced developers but could cause inconsistency if multiple implementers work on different tasks. A concrete recommendation would be cleaner.

**Recommendation:** Add a one-line note in the task body: "Unlike install's `validate_prefix_path`, update's version MUST fail with `PrefixPathMissing` when the path does not exist on disk, because updates never create prefixes."

---

### Task 1.3: Create update module root and register

**Verdict: High Quality**

| Criterion               | Pass | Notes                                        |
| ----------------------- | ---- | -------------------------------------------- |
| Clear Purpose           | Yes  | Wire up the module and verify with tests     |
| Specific File Changes   | Yes  | Create `update/mod.rs`, modify `lib.rs`      |
| Actionable Instructions | Yes  | Exact re-exports listed, placement specified |
| Gotchas Documented      | Yes  | Verification step included                   |
| Appropriate Scope       | Yes  | Two files, trivial changes                   |

No issues found. The task even specifies the exact `cargo test` command to run for verification.

---

### Task 2.1: Extract shared log utilities and create Tauri update commands

**Verdict: Needs Minor Improvements**

| Criterion               | Pass    | Notes                                                              |
| ----------------------- | ------- | ------------------------------------------------------------------ |
| Clear Purpose           | Yes     | Creates the Tauri command layer with streaming support             |
| Specific File Changes   | Yes     | Create `commands/update.rs`, modify `commands/mod.rs` and `lib.rs` |
| Actionable Instructions | Partial | See below                                                          |
| Gotchas Documented      | Partial | See below                                                          |
| Appropriate Scope       | Yes     | Three files                                                        |

**Issues:**

1. **Title says "Extract shared log utilities" but instructions say "duplicate locally."** The title is misleading -- no extraction happens. The instructions correctly match the codebase pattern (both `launch.rs` and `install.rs` have independent `create_log_path` functions), but the title suggests a refactoring that does not occur. This could confuse a developer scanning task titles.

2. **`spawn_log_stream` signature mismatch.** The existing `spawn_log_stream` in `commands/launch.rs` (line 103) takes three parameters: `(app: AppHandle, log_path: PathBuf, child: Child)`. The plan says to call `spawn_log_stream(app, log_path, child, "update-log")` with four parameters (adding the event name). This means the duplicated function in `commands/update.rs` must have a different signature, or the plan needs to clarify that the event name is hardcoded in the copy. The Advice section says to "change the event name from `launch-log` to `update-log`" in the copy, which is correct, but the four-parameter call syntax in the instruction body is misleading.

3. **The `update-complete` event is mentioned but the exact payload shape is not specified.** The plan says "emit a final `update-complete` event with the exit code as payload." The developer needs to know the concrete type -- is it `i32`, `Option<i32>`, a JSON object? This matters because Task 2.4 subscribes to it and needs to deserialize the payload.

4. **The `slug` variable used in `create_log_path("update", &slug)` is not defined.** The install command uses `install_log_target_slug(&request.profile_name)` (install.rs line 32). The plan should specify how to derive the slug from `UpdateGameRequest.profile_name` -- likely duplicating the same slugification logic.

**Recommendation:** Fix the title to "Create Tauri update commands with streaming." Clarify the `spawn_log_stream` signature (3 params with hardcoded event name, not 4). Specify the `update-complete` event payload type. Specify how to create the slug.

---

### Task 2.2: Create TypeScript types for update

**Verdict: High Quality**

| Criterion               | Pass | Notes                                                  |
| ----------------------- | ---- | ------------------------------------------------------ |
| Clear Purpose           | Yes  | Frontend type definitions mirroring the Rust models    |
| Specific File Changes   | Yes  | Create `types/update.ts`, modify `types/index.ts`      |
| Actionable Instructions | Yes  | Every interface, union, map, and constant is specified |
| Gotchas Documented      | Yes  | "Match Rust `message()` exactly" noted                 |
| Appropriate Scope       | Yes  | Two files                                              |

One minor note: the plan says to use "PascalCase" for the validation error string literal union. This is correct -- it matches the install pattern exactly (`install.ts` uses `'ProfileNameRequired'`, `'InstallerPathMissing'`, etc.) while Rust uses `#[serde(rename_all = "snake_case")]`. The plan should clarify that the TypeScript union uses the Rust variant names as-is (PascalCase) since serde only affects the serialized JSON representation of the _enum wrapper_, not the discriminant string. Actually, looking more carefully at the install types, the TypeScript union values (`'ProfileNameRequired'`, etc.) are the Rust enum variant names, while the serde `snake_case` rename applies to the outer `InstallGameError` enum's serialized form. This is consistent and the plan gets it right.

---

### Task 2.3: Add update-log subscription to console components

**Verdict: Needs Minor Improvements**

| Criterion               | Pass    | Notes                                                           |
| ----------------------- | ------- | --------------------------------------------------------------- |
| Clear Purpose           | Yes     | Wire up console to display update log lines                     |
| Specific File Changes   | Yes     | Modify `ConsoleView.tsx` and `ConsoleDrawer.tsx`                |
| Actionable Instructions | Partial | See below                                                       |
| Gotchas Documented      | Yes     | "Both must get the listener" is explicitly called out in Advice |
| Appropriate Scope       | Yes     | Two files, small changes                                        |

**Issues:**

1. **The `update-complete` event subscription is mentioned ("Also add a `listen('update-complete', ...)` subscription") but it is unclear WHERE.** Should it go in ConsoleView, ConsoleDrawer, or both? The hook (Task 2.4) also subscribes to `update-complete`. If ConsoleView subscribes to it too, what does it do with the completion signal? The task says "that the hook will need for detecting update completion" but then tells you to add it to the console components, not the hook. This conflates two concerns and could lead to duplicate subscriptions or confusion about which component is responsible for the state transition.

2. **Line references are slightly off.** The plan says `ConsoleView.tsx` listen call is at line 48 and `ConsoleDrawer.tsx` at line 54. These are correct for the current codebase (verified: ConsoleView line 48 is `const unlistenPromise = listen<LogPayload>('launch-log', (event) => {`, ConsoleDrawer line 54 is `const unlistenPromise = listen<LogPayload>('launch-log', (event) => {`).

**Recommendation:** Remove the `update-complete` listener from this task. The `update-complete` event is a hook concern (state machine transition), not a console display concern. The console components only need `update-log` for displaying lines.

---

### Task 2.4: Create useUpdateGame hook

**Verdict: High Quality**

| Criterion               | Pass | Notes                                                                          |
| ----------------------- | ---- | ------------------------------------------------------------------------------ |
| Clear Purpose           | Yes  | State machine hook for the update workflow                                     |
| Specific File Changes   | Yes  | Single file: `hooks/useUpdateGame.ts`                                          |
| Actionable Instructions | Yes  | State shape, functions, derived values, profile loading strategy all specified |
| Gotchas Documented      | Yes  | N+1 invoke warning, "Do NOT use global ProfileContext"                         |
| Appropriate Scope       | Yes  | One file, estimated ~200 lines                                                 |

The profile filtering approach (load all profile names, then load each to check `launch.method`) is clearly documented as an intentional tradeoff with a future optimization path noted. The hook's `startUpdate` flow is well-specified: validate -> invoke update_game -> subscribe to `update-complete` event.

One observation: the `update-complete` event payload type still needs to be defined somewhere. Task 2.4 says "Subscribe to `'update-complete'` event to transition to `'complete'` or `'failed'` based on exit code" but the payload shape is not defined in Task 2.2 (types) or Task 2.1 (Tauri command). This is a gap that crosses multiple tasks.

---

### Task 3.1: Extract shared form field components

**Verdict: High Quality**

| Criterion               | Pass | Notes                                                                        |
| ----------------------- | ---- | ---------------------------------------------------------------------------- |
| Clear Purpose           | Yes  | Pure move refactor to enable reuse                                           |
| Specific File Changes   | Yes  | Create two files, modify one                                                 |
| Actionable Instructions | Yes  | Exact line ranges for extraction (64-111, 113-179), export pattern specified |
| Gotchas Documented      | Yes  | "Pure move refactor" + "run dev server to verify"                            |
| Appropriate Scope       | Yes  | Three files, no logic changes                                                |

Line ranges verified against the codebase:

- `InstallField` starts at line 64 and ends at line 111 -- correct.
- `ProtonPathField` starts at line 113 and ends at line 179 -- correct.

The only consideration: `ProtonPathField` imports `formatProtonInstallLabel` and `ProtonInstallOption` from `ProfileFormSections.tsx`. The extracted component will need these imports updated. This is not mentioned but is standard refactoring knowledge.

---

### Task 3.2: Create UpdateGamePanel component

**Verdict: Needs Significant Work**

| Criterion               | Pass       | Notes                                                   |
| ----------------------- | ---------- | ------------------------------------------------------- |
| Clear Purpose           | Yes        | The main UI component for the update feature            |
| Specific File Changes   | Yes        | Single file: `UpdateGamePanel.tsx`                      |
| Actionable Instructions | Partial    | See below                                               |
| Gotchas Documented      | Partial    | See below                                               |
| Appropriate Scope       | Borderline | Single file but estimated 300+ lines with many concerns |

**Issues:**

1. **The confirmation dialog is specified but no implementation path is given.** The plan says "show modal: Apply update to [profile]?" but does not specify which modal component to use. The codebase has `ProfileReviewModal` but that is specialized for profile review. The plan does not specify whether to:
   - Use a new generic confirmation dialog component
   - Use `window.confirm()` (not matching the app's visual style)
   - Use an inline confirmation state pattern
   - Use an existing UI library modal
     This is a meaningful design decision that a developer cannot resolve without guidance.

2. **The `protonInstalls` prop is mentioned ("Accept `protonInstalls` from parent (InstallPage already loads these)") but `InstallPage` currently does NOT pass proton installs to `InstallGamePanel`.** Looking at InstallPage.tsx (line 372-375), `InstallGamePanel` receives `onOpenProfileReview` and `onRequestInstallAction` -- no proton installs. The proton installs are loaded inside `InstallGamePanel` itself (line 280-315). Meanwhile, `InstallPage` loads its own copy of proton installs (line 292-329) for the `ProfileReviewModal`. This means:
   - The plan says `UpdateGamePanel` receives `protonInstalls` as a prop from `InstallPage`
   - `InstallPage` already loads proton installs
   - But the loading logic would need to be lifted or shared, which is not addressed in Task 3.3

3. **The section structure lists 7 sub-sections but the order and conditional rendering logic is not specified.** When should the status card show? When should the confirmation dialog appear? These conditional rendering rules are important for the component's behavior.

4. **No prop interface is defined.** The plan should specify the exact `UpdateGamePanelProps` interface since the component receives props from `InstallPage`.

5. **Elapsed time display is mentioned in the feature spec (line 368: "Elapsed time display during execution") but not mentioned anywhere in this task or the plan.**

**Recommendation:** Define the exact props interface. Specify the confirmation dialog approach (recommend an inline confirmation state pattern matching `InstallPage`'s existing `reviewConfirmation` pattern, or extract a generic `ConfirmationDialog` component). Clarify how `protonInstalls` flows from parent. Address elapsed time display.

---

### Task 3.3: Integrate UpdateGamePanel into InstallPage

**Verdict: High Quality**

| Criterion               | Pass | Notes                                                    |
| ----------------------- | ---- | -------------------------------------------------------- |
| Clear Purpose           | Yes  | Wire the new component into the page                     |
| Specific File Changes   | Yes  | Single file: `InstallPage.tsx`                           |
| Actionable Instructions | Yes  | Import, render below InstallGamePanel, pass proton props |
| Gotchas Documented      | Yes  | "Always visible, not collapsed, no tab switching"        |
| Appropriate Scope       | Yes  | One file, small change                                   |

The only dependency gap is the `protonInstalls` prop issue noted in Task 3.2 -- `InstallPage` loads them already so passing them is straightforward, but this creates a subtle shared-state concern where both `InstallGamePanel` (which loads its own) and `UpdateGamePanel` (which receives from parent) show proton installs from different load calls. This is cosmetically fine but worth noting.

---

### Task 3.4: CSS styling and end-to-end validation

**Verdict: High Quality**

| Criterion               | Pass | Notes                                                        |
| ----------------------- | ---- | ------------------------------------------------------------ |
| Clear Purpose           | Yes  | Polish and verify the complete feature                       |
| Specific File Changes   | Yes  | `theme.css` if needed                                        |
| Actionable Instructions | Yes  | 10-item validation checklist is comprehensive                |
| Gotchas Documented      | Yes  | "Profile TOML is NOT modified after update" -- key invariant |
| Appropriate Scope       | Yes  | One file plus manual testing                                 |

The checklist covers all success criteria from the feature spec. The CSS guidance ("Follow the same visual treatment: `border-radius: 20px`, subtle border, backdrop blur, dark theme") is actionable. The `cargo test` verification step is included.

---

## Cross-Cutting Issues

### 1. The `update-complete` Event Payload Is Never Formally Defined

The event is referenced in three tasks:

- Task 2.1: "emit a final `update-complete` event with the exit code as payload"
- Task 2.3: "add a `listen('update-complete', ...)` subscription"
- Task 2.4: "Subscribe to `update-complete` event to transition to `complete` or `failed`"

But no task defines the TypeScript type for the payload. Task 2.2 (types) does not include it. A developer implementing Task 2.4 will not know whether the payload is `number`, `{ exitCode: number | null }`, or something else. The Rust side (Task 2.1) emits it but the shape is unspecified.

**Recommendation:** Add to Task 2.2: `export interface UpdateCompletePayload { exit_code: number | null; }` and reference it in Tasks 2.1 and 2.4.

### 2. Streaming vs. Blocking Tension Between Feature Spec and Plan

The feature spec (line 248) says `update_game` "blocks until the updater process exits." The parallel plan's Task 1.2 says the core function returns `(UpdateGameResult, Child)` without blocking. The Advice section explains this divergence, but it creates a situation where the spec and plan disagree. A developer checking the spec for clarification would be confused.

**Recommendation:** Note in the plan that the spec's blocking language reflects the _user-perceived_ behavior (the update runs to completion), while the implementation uses streaming for live console output. The core function is non-blocking; the Tauri command layer handles process monitoring via `spawn_log_stream`.

### 3. `build_update_command` Visibility

Task 1.2 lists three public functions but Task 1.3 only re-exports: `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError`, `validate_update_request`, `update_game`. It does not re-export `build_update_command`. This is correct (the function should be `pub(crate)` or private), but Task 1.2 says "Implement three public functions" which is inconsistent. `build_update_command` should be `pub(crate)` or `fn` (private), not `pub`.

**Recommendation:** Specify `build_update_command` as `pub(crate)` in Task 1.2 to match the install pattern (where `build_install_command` is `fn` / private).

### 4. The `steam_client_install_path` Field Source

The hook (Task 2.4) `populateFromProfile` extracts `runtime.proton_path` and `runtime.prefix_path` from the loaded profile but does not mention `steam_client_install_path`. This field defaults to `""` which is fine (the Advice section confirms this), but the `UpdateGameRequest` struct has the field and it should be populated from the same source as install uses. `InstallPage` resolves it from `PreferencesContext.defaultSteamClientInstallPath || profileContext.steamClientInstallPath`. The hook should do something similar, or explicitly set it to `""`. This is not addressed in Task 2.4.

**Recommendation:** Add to Task 2.4: "Set `steam_client_install_path` to `''` (empty string) -- the backend resolves it via fallback paths. If the user has configured a Steam client path in settings, the hook can optionally read it from `PreferencesContext`, but this is not required for MVP."

---

## Recommendations (Priority Order)

1. **[High] Define the `update-complete` event payload type.** Add it to Task 2.2 and reference it in Tasks 2.1 and 2.4. Without this, the streaming completion detection will require guesswork.

2. **[High] Specify the confirmation dialog approach in Task 3.2.** This is the largest actionability gap in the plan. Recommend using an inline confirmation state pattern (like `InstallPage`'s `reviewConfirmation`) or extracting a generic `ConfirmationDialog`.

3. **[Medium] Clarify `spawn_log_stream` signature in Task 2.1.** The four-parameter call syntax conflicts with the three-parameter function that will be duplicated. Make it clear the event name is hardcoded in the copy, not passed as a parameter.

4. **[Medium] Flag the prefix validation behavioral difference in Task 1.2.** The install template does not return `PrefixPathMissing` for non-existent paths; the update module must. This is the most likely copy-paste bug.

5. **[Medium] Specify the log slug derivation in Task 2.1.** Define how `profile_name` becomes the slug for `create_log_path("update", &slug)`.

6. **[Low] Fix Task 2.1 title.** "Extract shared log utilities" implies a refactoring that does not happen. Rename to "Create Tauri update commands with streaming."

7. **[Low] Remove `update-complete` subscription from Task 2.3.** The console components do not need completion detection -- that is the hook's responsibility (Task 2.4).

8. **[Low] Clarify `build_update_command` visibility in Task 1.2.** Specify it as private or `pub(crate)` to match the install pattern.

---

## Overall Assessment

The plan is well-structured and demonstrates thorough knowledge of the codebase. The dependency graph is correct, the parallelization opportunities are real (Tasks 2.2, 2.3, 3.1 can start during Phase 1), and the Advice section catches several important gotchas. File paths and line numbers are accurate against the current codebase.

The two areas that need the most attention before implementation are (1) the `update-complete` event contract that spans three tasks without a formal definition, and (2) the confirmation dialog approach in `UpdateGamePanel` which is the feature's most complex UI interaction and is left underspecified.

Seven of eleven tasks are immediately implementable by a developer familiar with the codebase. Three tasks need minor clarifications that could be resolved in a few minutes. One task (3.2) needs more design specification before a developer can confidently implement it without making arbitrary UI decisions.
