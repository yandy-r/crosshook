# Dry Run / Preview Launch — Recommendations & Risk Assessment

## Executive Summary

The preview launch feature (#40) is architecturally well-positioned for implementation. All required computation functions (`validate()`, `resolve_launch_directives()`, `build_steam_launch_options_command()`, `apply_host_environment()`) already exist in `crosshook-core` and are side-effect-free. The recommended approach is a single `preview_launch` Tauri command returning a unified `LaunchPreview` struct, placed in `crosshook-core` for reuse by both the Tauri app and the CLI. The primary risk is preview/launch divergence from filesystem and PATH state changes between preview time and launch time; this is mitigated with timestamps and staleness indicators rather than caching. Estimated effort: low — the backend is ~100 lines of new Rust code assembling existing function outputs, plus a new React component/modal for display.

---

## Implementation Recommendations

### 1.1 Recommended Approach: Single Tauri Command

**Use a single `preview_launch` command** returning a unified `LaunchPreview` struct.

**Why single over multiple:**

- All computation functions are already composed in sequence inside `launch_game`/`launch_trainer` — the preview mirrors this pipeline but stops before `.spawn()`
- Multiple granular commands (`preview_env`, `preview_command`, `preview_validation`) would introduce TOCTOU risk: filesystem or PATH state could change between calls, producing an inconsistent preview
- A single IPC round-trip is simpler for the frontend to consume and display atomically
- The existing `validate_launch` command already validates in isolation — the preview subsumes this

**Why not `dry_run` flag on LaunchRequest (Option C):**

- `LaunchRequest` is a clean, Serde-serialized type used across IPC, TOML persistence, and the CLI — adding a `dry_run: bool` mixes control flow with data
- The return type differs fundamentally: launch returns `LaunchResult` (process handle + log path), preview returns `LaunchPreview` (computed state snapshot)
- A separate command makes the API explicit and self-documenting

### 1.2 Architecture: Core Library Placement

Place the `LaunchPreview` struct and `preview_launch()` function in `crosshook-core/src/launch/` (not in `src-tauri/commands/`):

```
crosshook-core/src/launch/
  mod.rs          # Add `pub mod preview;` and re-exports
  preview.rs      # NEW: LaunchPreview struct + preview_launch() function

src-tauri/src/commands/
  launch.rs       # Add #[tauri::command] fn preview_launch() that calls core

crates/crosshook-cli/src/
  args.rs          # Add Command::Preview variant
```

This ensures both the Tauri app and the CLI binary can generate previews from the same code path.

### 1.3 Suggested `LaunchPreview` Struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchPreview {
    /// Resolved launch method after auto-detection
    pub resolved_method: String,
    /// Configured (raw) method from the profile
    pub configured_method: String,
    /// All validation results (empty = valid)
    pub validation_issues: Vec<LaunchValidationIssue>,
    /// Whether validation passed (no fatal issues)
    pub is_valid: bool,
    /// Resolved launch directives (env vars + wrappers from optimizations)
    pub directives: PreviewDirectives,
    /// Full resolved environment that would be set on the process
    pub environment: Vec<PreviewEnvVar>,
    /// The command line that would be executed (program + args)
    pub command_line: Vec<String>,
    /// Working directory for the process
    pub working_directory: String,
    /// For steam_applaunch: the computed %command% Launch Options string
    pub steam_launch_options: Option<String>,
    /// Trainer staging info (what files would be copied, if CopyToPrefix mode)
    pub trainer_staging: Option<PreviewTrainerStaging>,
    /// ISO 8601 timestamp when this preview was generated
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewEnvVar {
    pub key: String,
    pub value: String,
    /// Where this var comes from: "host", "proton", "optimization", "passthrough"
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewTrainerStaging {
    pub source_path: String,
    pub staged_path: String,
    pub support_files: Vec<String>,
}
```

**Forward-compatibility notes:**

- The `generated_at` timestamp enables #36 (post-launch diagnostics) to diff "before launch" vs "after launch" snapshots
- The `source` field on `PreviewEnvVar` is useful for #49 (diagnostic bundle) — lets users see exactly why each variable is set
- The struct is Serde-serializable, so #49 can dump it as JSON into a diagnostic archive

### 1.4 Phasing: Quick Wins First

**Phase A (MVP — this PR):**

1. `preview.rs` in `crosshook-core` with `LaunchPreview` struct and `preview_launch()` function
2. `preview_launch` Tauri command in `src-tauri/commands/launch.rs`
3. "Preview" button in `LaunchPanel.tsx` that opens a modal with the result
4. Basic text rendering of validation, env vars, command line, and directives

**Phase B (polish — follow-up PR):**

1. CLI `crosshook preview` command
2. Copy-to-clipboard button for bug reports
3. Collapsible sections in the preview modal (validation, env, command, etc.)

**Phase C (cross-feature — #36/#49):**

1. Reuse `LaunchPreview` as the "before" snapshot in post-launch diagnostics
2. Include preview JSON in diagnostic bundles

### 1.5 Preview Function Implementation Strategy

The `preview_launch()` function should:

1. Call `validate()` but **collect** the error as an issue rather than short-circuiting — the preview should show what's wrong even if validation fails
2. Call `resolve_launch_directives()` to get env/wrapper directives (guard against validation errors here too)
3. Reconstruct the environment by calling the same helpers that `build_proton_game_command()` etc. use, but instead of setting them on a `Command`, collect them into `Vec<PreviewEnvVar>`
4. Build the command-line args array by reconstructing what `build_helper_command()` / `build_proton_game_command()` / `build_native_game_command()` would produce
5. For `CopyToPrefix` trainer mode, describe the staging operation without actually performing it
6. For `steam_applaunch`, call `build_steam_launch_options_command()` to produce the `%command%` string

**Key difference from launch:** The preview MUST NOT call `stage_trainer_into_prefix()` — this is the one place where the preview code path diverges from the actual launch path. Instead, it should compute and report what staging _would_ do.

---

## 2. Improvement Ideas

### 2.1 Beyond Basic Acceptance Criteria

| Idea                                                                                              | Value  | Effort  | Phase                                |
| ------------------------------------------------------------------------------------------------- | ------ | ------- | ------------------------------------ |
| **Copy as text** — one-click copy of the entire preview as formatted text for bug reports         | High   | Low     | B                                    |
| **Copy as JSON** — structured export for #49 diagnostic bundles                                   | Medium | Low     | B                                    |
| **Diff view** — show what changed since last preview (useful when tweaking optimizations)         | Medium | Medium  | C                                    |
| **Preview history** — keep last N previews in session memory for comparison                       | Low    | Medium  | C                                    |
| **CLI preview** — `crosshook preview --profile elden-ring [--json]` for headless/scripted usage   | High   | Low     | B                                    |
| **Steam Launch Options copy** — dedicated copy button for the `%command%` string                  | High   | Trivial | A                                    |
| **Validation-only preview** — show validation status without the full preview (lightweight check) | Low    | Trivial | Already exists via `validate_launch` |
| **Auto-preview on profile change** — re-run preview when profile fields change                    | Medium | Medium  | C                                    |

### 2.2 Cross-Feature Synergies

**#36 Post-Launch Diagnostics:**

- Same `LaunchPreview` struct, captured at launch time as the "before" snapshot
- After launch, diff the expected vs actual environment (if the helper script can report back)
- Staleness timestamp lets the diagnostics viewer show time-to-launch drift

**#49 Diagnostic Bundle:**

- Include `LaunchPreview` JSON as `preview.json` in the bundle
- The `source` field on env vars tells support exactly where each value came from
- Validation issues in the preview provide immediate triage context

**#38 Profile Health Check:**

- The validation subset of `LaunchPreview` is essentially a profile health check
- Could expose a `preview_launch` with a `validation_only: true` option that skips environment resolution for speed

---

## Risk Assessment

### 3.1 Technical Risks

| Risk                                                                                                 | Likelihood | Impact   | Mitigation                                                                                                             |
| ---------------------------------------------------------------------------------------------------- | ---------- | -------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Preview/launch divergence** — filesystem state changes between preview and launch                  | Medium     | Medium   | Include `generated_at` timestamp; show "stale preview" warning after 60s; re-validate at launch time (already happens) |
| **PATH-dependent wrapper availability** — mangohud/gamemoderun could appear/disappear                | Low        | Low      | Preview reflects point-in-time PATH state; actual launch re-checks independently                                       |
| **Environment variable drift** — host env vars (DISPLAY, WAYLAND_DISPLAY) change between sessions    | Low        | Low      | Preview captures current values; re-generating preview is cheap                                                        |
| **Trainer staging description mismatch** — preview describes staging differently than actual staging | Low        | High     | Keep staging description logic close to `stage_trainer_into_prefix()`; test both paths                                 |
| **Performance with many optimization toggles** — large env var sets                                  | Very Low   | Very Low | Maximum 17 optimization IDs, each producing 0-1 env var — negligible                                                   |
| **Steam launch options string divergence** — preview shows different `%command%` than helper script  | Low        | Medium   | Both call `build_steam_launch_options_command()` from `crosshook-core`; they are the same code path                    |

### 3.2 UX Risks

| Risk                                                                                        | Impact | Mitigation                                                                                                 |
| ------------------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------- |
| **Information overload** — showing 40+ env vars, all validation details, full command lines | High   | Collapsible sections with sensible defaults (validation expanded, env collapsed); progressive disclosure   |
| **False confidence** — users assume preview = guaranteed launch behavior                    | Medium | Clear disclaimer: "This preview reflects the current state. Actual launch re-validates at execution time." |
| **Gamepad navigation difficulty** — preview modal may be hard to navigate with controller   | Medium | Ensure the modal works with `useGamepadNav` hook; simple scroll + close button                             |
| **Copy/paste formatting** — raw text export may be hard to read in Discord/GitHub           | Low    | Format as Markdown-compatible text; JSON option for structured consumers                                   |

### 3.3 Maintenance Risks

| Risk                                                                                                                       | Impact | Mitigation                                                                                                                      |
| -------------------------------------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------- |
| **Preview and launch code diverge** — new launch features not reflected in preview                                         | High   | Place preview logic in `crosshook-core` next to launch logic; add tests that verify preview output matches command construction |
| **New optimization toggles not in preview** — adding toggles to `LAUNCH_OPTIMIZATION_DEFINITIONS` without updating preview | Medium | Preview calls the same `resolve_launch_directives()` function — new toggles are automatically included                          |
| **New launch methods not in preview** — adding a fourth launch method without preview support                              | Medium | Pattern-match on `resolved_method()` in preview function; compiler warns on non-exhaustive match                                |
| **Frontend type drift** — TypeScript `LaunchPreview` type diverges from Rust struct                                        | Medium | Standard Tauri IPC pattern — same risk as every other command; mitigate with integration test                                   |

---

## 4. Alternative Approaches

### Option A: Single `preview_launch` Command (RECOMMENDED)

**Description:** One Tauri command takes `LaunchRequest`, returns `LaunchPreview` with all computed data.

| Dimension          | Assessment                                                                                           |
| ------------------ | ---------------------------------------------------------------------------------------------------- |
| **Pros**           | Atomic snapshot, no TOCTOU risk; single IPC round-trip; simple frontend consumption; reusable in CLI |
| **Cons**           | Slightly larger response payload (~2-5KB JSON); preview function has more responsibilities           |
| **Effort**         | Low — ~100 lines Rust, ~150 lines TypeScript                                                         |
| **Forward-compat** | Excellent — same struct used for #36 and #49                                                         |

### Option B: Multiple Granular Commands

**Description:** Separate `preview_env`, `preview_command`, `preview_validation`, `preview_directives` commands.

| Dimension          | Assessment                                                                                                                      |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**           | Frontend can fetch only what it needs; smaller individual payloads                                                              |
| **Cons**           | TOCTOU risk between calls; 4 IPC round-trips; frontend must coordinate 4 async calls; harder to snapshot atomically for #36/#49 |
| **Effort**         | Medium — more Tauri commands, more frontend coordination                                                                        |
| **Forward-compat** | Poor — no single struct to reuse                                                                                                |

### Option C: `dry_run` Flag on LaunchRequest

**Description:** Add `dry_run: bool` to `LaunchRequest`; launch commands return `LaunchPreview` instead of `LaunchResult` when `true`.

| Dimension          | Assessment                                                                                                                                                   |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Pros**           | No new Tauri commands; conceptually simple                                                                                                                   |
| **Cons**           | Mixes control flow with data; different return types for same command; makes `LaunchRequest` messier; `dry_run` leaks into TOML serialization unless skipped |
| **Effort**         | Low — but refactoring cost to separate return types                                                                                                          |
| **Forward-compat** | Moderate — the struct itself is reusable but the API coupling is awkward                                                                                     |

### Option D: Reuse Existing `validate_launch` + Client-Side Assembly

**Description:** Call `validate_launch` from frontend, then compute env/command display purely in TypeScript using the optimization definitions already in `launch-optimizations.ts`.

| Dimension          | Assessment                                                                                                                                                 |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**           | Zero new backend code; instant                                                                                                                             |
| **Cons**           | Duplicates environment resolution logic in TypeScript; cannot accurately show runtime PATH checks or host env vars; will diverge from Rust logic over time |
| **Effort**         | Low initially — high maintenance debt                                                                                                                      |
| **Forward-compat** | Very poor — no backend struct for #36/#49                                                                                                                  |

**Recommendation:** Option A. It provides the best balance of simplicity, correctness, forward-compatibility, and maintenance characteristics.

---

## 5. Task Breakdown Preview

### Phase A: MVP (This PR)

| Task Group           | Tasks                                                                                                                           | Complexity |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| **Backend types**    | Define `LaunchPreview`, `PreviewDirectives`, `PreviewEnvVar`, `PreviewTrainerStaging` in `crosshook-core/src/launch/preview.rs` | Low        |
| **Backend function** | Implement `preview_launch(request: &LaunchRequest) -> LaunchPreview` assembling validation, directives, env, command line       | Medium     |
| **Tauri command**    | Add `preview_launch` command in `src-tauri/src/commands/launch.rs`; register in `invoke_handler`                                | Low        |
| **Frontend types**   | Add `LaunchPreview` TypeScript interface in `src/types/launch.ts`                                                               | Low        |
| **Frontend hook**    | Add `usePreviewLaunch()` hook or extend `useLaunchState` with preview action                                                    | Low        |
| **Frontend UI**      | Preview button in `LaunchPanel.tsx`; modal/panel showing preview sections                                                       | Medium     |
| **Tests**            | Unit tests for `preview_launch()` in `crosshook-core`; verify output matches expected command construction                      | Medium     |

**Estimated total: 4-6 implementation tasks, ~1.5 days**

### Phase B: Polish (Follow-up PR)

| Task Group                    | Tasks                                                                                               | Complexity |
| ----------------------------- | --------------------------------------------------------------------------------------------------- | ---------- |
| **CLI command**               | Add `Command::Preview` to `crosshook-cli/src/args.rs`; implement handler calling `preview_launch()` | Low        |
| **Copy to clipboard**         | Add copy-as-text and copy-as-JSON buttons to preview modal                                          | Low        |
| **Collapsible sections**      | Add expand/collapse to validation, environment, command, directives sections                        | Low        |
| **Steam Launch Options copy** | Dedicated copy button for the `%command%` string                                                    | Trivial    |

**Estimated total: 3-4 tasks, ~0.5 days**

### Phase C: Cross-Feature Integration (Separate PRs)

| Task Group          | Tasks                                                       | Complexity |
| ------------------- | ----------------------------------------------------------- | ---------- |
| **#36 integration** | Capture `LaunchPreview` at launch time as "before" snapshot | Low        |
| **#49 integration** | Include preview JSON in diagnostic bundle                   | Low        |
| **Diff view**       | Compare two `LaunchPreview` instances and highlight changes | Medium     |

---

## 6. Key Decisions Needed

1. **Modal vs inline panel?** The preview could be a modal dialog (blocks interaction, focused reading) or an inline collapsible panel below the launch button (always visible, non-blocking). Recommendation: modal for MVP, with option to pin/dock later.

2. **Preview button placement?** Next to the Launch Game button in `LaunchPanel.tsx` or in the profile editor? Recommendation: next to Launch Game — it answers "what will happen when I click this?"

3. **Should preview auto-refresh?** When profile fields change, should the preview automatically regenerate? Recommendation: no for MVP — explicit "Preview" button click is simpler and avoids unnecessary IPC.

4. **Should preview block on validation failure?** Recommendation: no — show validation issues as part of the preview (red section at top), but still show the rest of the computed state. This is the key value prop over just `validate_launch`.

5. **Should the preview show both game and trainer steps?** Recommendation: yes — show both as separate sections within one preview, matching the two-step launch flow.

---

## 7. Open Questions

1. **Trainer staging description accuracy** — How detailed should the staging preview be? Just source/destination paths, or also list each support file that would be copied? The `SUPPORT_DIRECTORIES` and `SHARED_DEPENDENCY_EXTENSIONS` constants in `script_runner.rs` define what gets staged.

2. **Environment variable filtering** — Should the preview show _all_ env vars (including cleared WINE vars from `WINE_ENV_VARS_TO_CLEAR`) or only the vars that are explicitly set? Showing cleared vars adds transparency but increases noise.

3. **Steam helper script args** — For `steam_applaunch`, the actual launch goes through a shell script with 15+ arguments. Should the preview show the raw script invocation or a higher-level summary? Recommendation: show the summary (what the script does) not the raw args.

4. **Preview for native launches** — Native launches are simpler (no Proton, no trainer). Should the preview modal adapt its layout to hide irrelevant sections, or show them as "N/A"? Recommendation: hide irrelevant sections for a cleaner experience.
