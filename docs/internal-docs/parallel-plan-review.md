# Parallel Plan Review: Platform-Native UI

Evaluation of the 34-task parallel implementation plan (`docs/plans/platform-native-ui/parallel-plan.md`) for the Tauri v2 native UI. Each task was cross-referenced against the actual codebase to verify line numbers, file paths, technical claims, and completeness.

---

## Task Quality Summary

| Metric                       | Count |
| ---------------------------- | ----- |
| **Total Tasks**              | 34    |
| **High Quality**             | 22    |
| **Needs Minor Improvements** | 8     |
| **Needs Significant Work**   | 4     |

---

## Detailed Findings

Tasks not listed below passed all four criteria (clear purpose, specific file paths, actionable instructions, appropriate scope) and are implementation-ready. These include: 1.1, 1.3, 1.4, 1.5, 1.6, 1.8, 1.9, 1.10, 1.15, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.10, 2.12, 2.13, 2.14, 3.1, 3.2.

---

### Phase 1

#### Task 1.2: React/Vite/Tauri scaffolding -- Needs Minor Improvement

- **Issue: Ambiguous scaffolding method.** Instruction says "Run `cargo create-tauri-app` or manually scaffold" -- a developer must decide which approach to use and the two produce different file structures. The manual option requires knowing exactly what boilerplate files Tauri v2 needs (e.g., `index.html`, `capabilities/`, `icons/`).
- **Fix:** Pick one approach and specify it. If manual, list every file including `index.html`, `src-tauri/capabilities/default.json`, `src-tauri/icons/`. If using the CLI tool, specify the exact command with flags (e.g., `cargo create-tauri-app crosshook-native --template react-ts --manager npm`).

#### Task 1.7: Shell script invocation wrapper -- Needs Minor Improvement

- **Issue: Environment variable list is hardcoded in prose.** The instructions list ~10 specific env vars to pass through, but this duplicates what Task 1.9 defines as named constants. There is no cross-reference telling the developer to import from `launch::env`.
- **Fix:** Add "Import `PASSTHROUGH_DISPLAY_VARS` and `REQUIRED_PROTON_VARS` from `launch::env` (Task 1.9) for the env setup. If 1.9 is not yet complete, use inline constants temporarily." This also surfaces a dependency ordering issue -- 1.7 depends on [1.1] but references 1.9's output without declaring that dependency.

#### Task 1.11: Tauri IPC commands (launch) -- Needs Minor Improvement

- **Issue: Log streaming implementation underspecified.** The 500ms poll interval file-reading approach is described but lacks detail on: when to stop polling (the C# version uses a 2-minute deadline at MainForm.cs line 2910), what to do if the log file never appears, and error handling for file locks.
- **Fix:** Add: "Implement a 2-minute deadline matching the C# behavior. If the log file does not appear within 10 seconds after launch, emit a warning event. Stop streaming when the child process exits OR the deadline is reached."

#### Task 1.12: React profile editor form -- Needs Minor Improvement

- **Issue: "Lines 1-200 for UI field layout" is a vague reference.** MainForm.cs lines 1-200 are field declarations and constructor setup, not a UI layout specification. A developer would need to read hundreds of lines of Designer.cs to understand actual layout.
- **Fix:** Replace the reference with a bulleted field list directly in the instructions: "Form fields in order: Game Path (browse), Trainer Path (browse), DLL 1 Path (browse), DLL 2 Path (browse), Launch Inject 1 (checkbox), Launch Inject 2 (checkbox), Launch Method (dropdown), Steam Mode (toggle), Steam App ID (text), Compatdata Path (browse), Proton Path (browse), Launcher Icon Path (browse)."

#### Task 1.13: React two-step launch UI -- Needs Minor Improvement

- **Issue: Does not mention error state handling.** The `LaunchPhase` enum includes happy-path states but no `Error` state. If `launch_game` fails, the UI behavior is undefined.
- **Fix:** Add `Error` to the `LaunchPhase` enum and specify: "On launch failure, transition to `Error` state with error message display. Show a 'Retry' button that resets to `Idle`."

#### Task 1.16: Shell script bundling -- Needs Minor Improvement

- **Issue: Resource path in `tauri.conf.json` assumes scripts stay at the C# project location.** The path `../../src/CrossHookEngine.App/runtime-helpers/*.sh` is fragile and couples the native app build to the legacy project directory structure.
- **Fix:** Add a build step (or a note in the instructions) to copy the 3 scripts into `src/crosshook-native/src-tauri/runtime-helpers/` during development, so the resource path becomes `runtime-helpers/*.sh`. The `build-native.sh` script (Task 3.8) can handle the copy.

---

### Phase 2

#### Task 2.1: VDF key-value parser -- Needs Minor Improvement

- **Issue: "First evaluate the `steam-vdf-parser` crate" is a research step embedded in an implementation task.** If the crate fails evaluation, the developer must pivot to writing ~125 lines of parser code -- a fundamentally different task. This makes time estimation unpredictable.
- **Fix:** Split into two sequential micro-tasks: (A) Evaluate `steam-vdf-parser` crate against real files, document result. (B) Based on result, either wrap the crate or port the parser. Alternatively, just commit to the port -- the C# parser is well-documented (lines 739-863 of SteamAutoPopulateService.cs, verified) and the crate evaluation adds uncertainty.

#### Task 2.9: Diagnostic collector -- Needs Minor Improvement

- **Issue: This task is trivially small (~15 lines of Rust) and could be folded into Task 2.4 (Steam data models) since `DiagnosticCollector` is already defined in Task 2.4's struct list.** Having it as a separate task creates overhead. Task 2.4 already specifies the struct; Task 2.9 only adds 3 methods.
- **Fix:** Merge into Task 2.4, or keep as-is but flag it as a 15-minute task for sequencing purposes.

---

### Phase 3

#### Task 3.5: React settings panel -- Needs Significant Work

- **Issue: No design specifics.** "Settings UI: auto-load toggle, recent files display, profiles directory configuration. Simple form layout." gives almost no implementation guidance. What does "recent files display" look like? Is "profiles directory configuration" a text input or a browse dialog? How is this panel accessed (modal, sidebar, route)?
- **Fix:** Specify: panel access method (e.g., gear icon in header opens a slide-out drawer or modal), each field's input type, and the recent files display format (e.g., "collapsible list grouped by type: Game Paths, Trainer Paths, DLL Paths, with click-to-fill behavior").

#### Task 3.6: Dark gaming theme -- Needs Significant Work

- **Issue: Two CSS files are listed but no guidance on CSS architecture.** Does the app use CSS modules, Tailwind, styled-components, or plain CSS? The `variables.css` file implies CSS custom properties, but this conflicts with whatever CSS approach Task 1.2 scaffolds. No mention of how components consume these styles.
- **Fix:** Decide and document the CSS strategy in Task 1.2, then reference it here. Specify: "Use CSS custom properties in `variables.css` for the design tokens. Import `theme.css` in `main.tsx`. Components use the variables via `var(--color-bg-primary)` etc." This is a cross-cutting concern that affects all Phase 1 React components.

#### Task 3.7: Controller/gamepad navigation -- Needs Significant Work

- **Issue: "Detect `SteamDeck=1` env var" -- this is a Tauri/Rust concern, not a React hook concern.** Environment variables are not accessible from the browser/WebView context. This task needs a Tauri command to expose the detection, or the hook needs to use the Gamepad API directly.
- **Fix:** Add: "Create a Tauri command `is_steam_deck() -> bool` that checks the `SteamDeck` env var from the Rust side. The React hook calls this on mount. Also detect the Gamepad API as a fallback for non-Steam-Deck controllers." Alternatively, move env var detection to `startup.rs` (Task 3.3) and emit it as a frontend event.

#### Task 3.9: AUR PKGBUILD -- Needs Significant Work

- **Issue: Extremely vague.** "Build with `cargo tauri build`" is insufficient for a PKGBUILD. Missing: `pkgname`, `pkgver` strategy, `makedepends` (rust, nodejs, npm, webkit2gtk, etc.), the `build()` function body, `package()` function body with `install -Dm755` commands, `license`, where the binary and shell scripts are installed, `source` array format for GitHub releases.
- **Fix:** Provide a skeleton PKGBUILD with at least: `pkgname=crosshook-native`, `makedepends` list, `build()` body, `package()` body with `install` commands, and the `source` array format. An Arch developer would currently have to guess every field.

---

### Phase 4

Phase 4 tasks (4.1-4.6) are intentionally high-level, which is acceptable for their planning horizon. Two minor observations:

- **Task 4.3 (Git-based profile sharing)** is a multi-day feature compressed into one paragraph. It should be flagged as requiring its own sub-plan before implementation. The scope (git2 crate or shelling out, index format, conflict resolution, network errors, background sync) is at least 3-5 subtasks.
- **Task 4.6 (Compatibility database viewer)** does not specify where the compatibility data comes from (bundled, remote API, derived from community profiles). This data source question must be answered before implementation.

---

## Cross-Cutting Issues

### 1. Implicit dependency between Tasks 1.7 and 1.9

Task 1.7 (script runner) hardcodes environment variable names in its instructions. Task 1.9 (env cleanup constants) defines them as named constants. Both depend on [1.1] and can run in parallel -- but a developer doing 1.7 first will write inline constants that 1.9 then duplicates. Either add 1.9 as a dependency of 1.7, or note that 1.7 should import from 1.9 when available.

### 2. No integration test task

There is no task for end-to-end integration testing of the Rust core (e.g., "load a legacy profile, build a launch command, verify the command args match expected values"). Individual tasks mention `#[cfg(test)]` unit tests, but no task validates the full pipeline works together. A Task 1.17 or 2.15 for integration tests would catch cross-module wiring issues early.

### 3. CSS strategy undefined until Phase 3

Task 3.6 (Dark gaming theme) establishes the CSS architecture, but Phase 1 React components (Tasks 1.12, 1.13, 1.14) will already need styling decisions. Either move the CSS strategy to Task 1.2, or add a Phase 1 task for base styles. Without this, three developers could independently choose different styling approaches.

### 4. Missing module registration notes

Several tasks create new Rust files (e.g., `launch/script_runner.rs`, `profile/legacy.rs`, `steam/vdf.rs`) but do not mention updating the corresponding `mod.rs` to declare the submodule with `pub mod`. This is a trivial step but can trip up developers less familiar with Rust's module system. Consider adding a standard note: "Register the new module in the parent `mod.rs`."

### 5. `directories` crate missing from Task 1.1 dependency list

Task 1.6 uses the `directories` crate for `~/.config/crosshook/` path resolution, but Task 1.1 (workspace scaffolding) does not include it in the `crosshook-core/Cargo.toml` dependency list. Same applies to `unicase` (mentioned in Task 2.1 for case-insensitive keys). These should be added to Task 1.1, or each consuming task should note "add `directories` to `Cargo.toml` if not already present."

---

## Line Number Verification

All C# line references in the plan were spot-checked against the actual source. Results:

| Reference                                               | Claimed   | Actual    | Status   |
| ------------------------------------------------------- | --------- | --------- | -------- |
| ProfileService.cs ProfileData class                     | line 199  | line 199  | Exact    |
| SteamAutoPopulateService.cs ParseKeyValueObject         | line 739  | line 739  | Exact    |
| SteamLaunchService.cs SteamLaunchRequest                | line 10   | line 10   | Exact    |
| SteamLaunchService.cs GetEnvironmentVariablesToClear    | line 233  | line 233  | Exact    |
| SteamAutoPopulateService.cs DiscoverSteamRootCandidates | line 164  | line 164  | Exact    |
| MainForm.cs BuildSteamLaunchRequest                     | line 2648 | line 2648 | Exact    |
| MainForm.cs UpdateSteamModeUiState                      | line 2119 | line 2119 | Exact    |
| MainForm.cs StreamSteamHelperLogAsync                   | line 2908 | line 2908 | Exact    |
| SteamAutoPopulateService.cs line count                  | ~1286     | 1285      | Off by 1 |
| SteamLaunchService.cs line count                        | ~737      | 736       | Off by 1 |
| ProfileService.cs line count                            | ~225      | 224       | Off by 1 |

Line references are highly accurate. The ~1-line discrepancies in total line counts are negligible and likely due to trailing newline differences.

---

## Priority Improvements (Ordered)

1. **Add CSS strategy to Task 1.2** -- blocks all Phase 1 React components (1.12, 1.13, 1.14) from having a consistent approach
2. **Fix Task 1.7 / 1.9 dependency** -- prevents constant duplication during parallel development
3. **Flesh out Task 3.9 (AUR PKGBUILD)** -- currently unimplementable without guessing every field
4. **Add `directories` crate to Task 1.1** -- blocks Task 1.6 at build time
5. **Clarify Task 1.2 scaffolding approach** -- pick CLI or manual, list all files
6. **Add error state to Task 1.13 launch UI** -- prevents undefined behavior on failure
7. **Specify Task 3.5 settings panel layout** -- currently too vague to implement
8. **Fix Task 3.7 env var detection** -- `SteamDeck=1` env var is not accessible from browser context
9. **Add integration test task after Phase 1** -- validates the pipeline before Phase 2
10. **Add log streaming termination conditions to Task 1.11** -- missing deadline/timeout behavior from C# reference

---

## Overall Assessment

**Plan Quality: Strong -- Ready for implementation with minor pre-work.**

The 22 high-quality tasks (65%) can be picked up and implemented immediately without guessing. The 8 tasks needing minor improvements can proceed with a developer who exercises reasonable judgment -- the gaps are small and well-bounded. The 4 tasks needing significant work (3.5, 3.6, 3.7, 3.9) are all in Phase 3, which provides time to refine them before they block progress.

**Key Strengths:**

- Every task has a "READ THESE BEFORE TASK" section with specific source files and line numbers -- all verified accurate
- Line number references are precise across every spot-checked location (8/8 exact matches)
- The dependency graph is a valid DAG with no cycles and reasonable parallelism
- The Advice section at the bottom contains genuinely critical implementation wisdom (script selection for Phase 2 launch, path conversion code NOT needed, profile format cross-app contract)
- File paths for all 34 tasks specify concrete locations in the `src/crosshook-native/` tree -- no placeholders

**Key Weaknesses:**

- CSS/styling strategy is deferred to Phase 3 but needed in Phase 1
- One implicit dependency (1.7 <-> 1.9) could cause wasted parallel work
- Four Phase 3 tasks (3.5, 3.6, 3.7, 3.9) need significant specification work
- No integration test task anywhere in the plan

**Implementation Readiness:** Phase 1 and Phase 2 are ready to begin immediately. The recommended pre-work is: (1) add CSS strategy to Task 1.2, (2) resolve the 1.7/1.9 dependency, and (3) add `directories` to Task 1.1's dependency list. Phase 3 should be refined after Phase 1 delivery. Phase 4 is appropriately sketched and should get its own detailed sub-plan when the time comes.
