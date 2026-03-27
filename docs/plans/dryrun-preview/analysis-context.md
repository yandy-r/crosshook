# Dry Run / Preview Launch Mode — Implementation Context

## Executive Summary

The preview feature adds a single read-only `preview_launch` Tauri command that assembles outputs from existing pure functions (`validate()`, `resolve_launch_directives()`, `build_steam_launch_options_command()`) into a `LaunchPreview` struct, displayed in a modal dialog. Two new files are created; seven existing files are modified. The primary backend gap is `validate()` being fail-fast — a new `validate_all()` collector is needed. Environment collection requires new pure functions in `preview.rs` that mirror `Command`-mutating helpers but return tagged `PreviewEnvVar` data instead. The frontend reuses `ProfileReviewModal` infrastructure (focus trap, portal, gamepad nav) with `CollapsibleSection` accordions. All computation is <1ms, synchronous, and side-effect-free.

---

## Architecture Context

### Data Flow

```
LaunchPanel → invoke("preview_launch", { request }) → Tauri Command (sync)
  → build_launch_preview(&request) in crosshook-core
    → request.resolved_method()         → method string
    → validate_all(&request)            → Vec<LaunchValidationIssue>
    → resolve_launch_directives()       → Ok(LaunchDirectives) | Err
    → collect_preview_environment()     → Vec<PreviewEnvVar>
    → build_effective_command_string()  → String
    → resolve_proton_setup()           → Option<ProtonSetup>
    → build_trainer_info()             → Option<PreviewTrainerInfo>
  → LaunchPreview (Serialize → JSON → TypeScript)
  → React modal with collapsible sections
```

### Key Design Decisions (Resolved)

| Decision             | Choice                                           | Rationale                                                   |
| -------------------- | ------------------------------------------------ | ----------------------------------------------------------- |
| Command structure    | Single `preview_launch` returning unified struct | Atomic snapshot, no TOCTOU, single IPC round-trip           |
| Validation           | New `validate_all()` function                    | Core value prop — show everything, not first error          |
| Core logic placement | `crosshook-core/src/launch/preview.rs`           | Unit-testable, CLI-reusable, follows workspace separation   |
| State management     | Separate `usePreviewState.ts` hook               | Decoupled from launch lifecycle; simpler than reducer       |
| Env collection       | New pure functions in `preview.rs`               | Avoids risky refactor of working `Command`-mutating code    |
| UI container         | Modal dialog                                     | Existing infrastructure, gamepad support, focused attention |
| Clipboard format     | Structured TOML via `to_display_toml()`          | Zero-dependency, matches profile format, shareable          |
| Preview scope        | Unified view (game + trainer)                    | Shows everything in one view                                |
| Auto-refresh         | Manual re-trigger only (MVP)                     | Matches Terraform `plan` metaphor                           |

### Error Semantics

- **Command errors** (`Err`): Only on malformed request (unrecognizable method)
- **Validation failures**: Data inside `LaunchPreview.validation`, NOT command errors
- **Directive failures**: `Ok(LaunchPreview)` with `environment=null`, `wrappers=null`, `directives_error="..."`
- **Partial results**: `Option<T>` fields enable sections to fail independently

---

## Critical Files Reference

### Files to Create

| File                                                               | Purpose                                                                                                                                                                                               |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` | Core module: `LaunchPreview` struct, `PreviewEnvVar`, `EnvVarSource`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation`, `build_launch_preview()`, env collection helpers, `to_display_toml()` |
| `src/crosshook-native/src/hooks/usePreviewState.ts`                | React hook: preview invocation state (loading, preview, error) using `useState` pattern                                                                                                               |

### Files to Modify

| File                                                                     | Change                                                                                                                          |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs`           | Add `pub mod preview;` + re-export `build_launch_preview`, `LaunchPreview`                                                      |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`       | Add `validate_all()` with method-specific collectors (`collect_steam_issues`, `collect_proton_issues`, `collect_native_issues`) |
| `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` | Add `Serialize, Deserialize` to `LaunchDirectives` derive (line 17, one-line change)                                            |
| `src/crosshook-native/src-tauri/src/commands/launch.rs`                  | Add sync `preview_launch` Tauri command wrapping `build_launch_preview()`                                                       |
| `src/crosshook-native/src-tauri/src/lib.rs`                              | Register `commands::launch::preview_launch` in `invoke_handler` (after line 90)                                                 |
| `src/crosshook-native/src/types/launch.ts`                               | Add `LaunchPreview`, `PreviewEnvVar`, `EnvVarSource`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation` interfaces       |
| `src/crosshook-native/src/components/LaunchPanel.tsx`                    | Add "Preview Launch" ghost button in `__actions` div + modal trigger/display                                                    |

### Key Reference Files (Read-Only)

| File                     | What to Reference                                                                                                                                                                                | Key Lines                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------- |
| `request.rs`             | `LaunchRequest` struct, `validate()`, `ValidationError` enum, `LaunchValidationIssue`, helper validators                                                                                         | L16-37, L151-199, L442-454 |
| `optimizations.rs`       | `LaunchDirectives`, `resolve_launch_directives()`, `build_steam_launch_options_command()`, `LAUNCH_OPTIMIZATION_DEFINITIONS`                                                                     | L17-21, L267-283, L288-300 |
| `env.rs`                 | `WINE_ENV_VARS_TO_CLEAR` (31 vars), `REQUIRED_PROTON_VARS` (3), `LAUNCH_OPTIMIZATION_ENV_VARS` (14), `PASSTHROUGH_DISPLAY_VARS` (4)                                                              | Constants                  |
| `runtime_helpers.rs`     | `resolve_wine_prefix_path()`, `resolve_steam_client_install_path()`, `apply_host_environment()`, `apply_runtime_proton_environment()`                                                            | L94, L157, L46, L62        |
| `script_runner.rs`       | `stage_trainer_into_prefix()` (side-effecting — preview replicates path computation only), `STAGED_TRAINER_ROOT`                                                                                 | L227-266, L22              |
| `commands/launch.rs`     | `validate_launch` (sync thin-wrapper pattern), `LaunchResult` struct                                                                                                                             | L25-28, L18-23             |
| `useLaunchState.ts`      | Reducer-based state machine — reference but preview uses simpler `useState`                                                                                                                      | —                          |
| `CollapsibleSection.tsx` | `<details>`/`<summary>` wrapper, BEM classes `crosshook-collapsible__*`                                                                                                                          | —                          |
| `ProfileReviewModal.tsx` | Focus trap, portal, `data-crosshook-focus-root="modal"`, backdrop, inert siblings, `statusTone`                                                                                                  | —                          |
| `LaunchPage.tsx`         | `buildLaunchRequest()` — constructs `LaunchRequest` from profile, returns null if game path empty                                                                                                | L10-43                     |
| `variables.css`          | Color tokens: `--crosshook-color-success` (#28c76f), `--crosshook-color-warning` (#f5c542), `--crosshook-color-danger` (#ff758f), `--crosshook-font-mono`, `--crosshook-touch-target-min` (48px) | —                          |

---

## Patterns to Follow

### Tauri Command (Sync Thin Wrapper)

Follow `validate_launch` pattern: sync function, takes `LaunchRequest`, returns `Result<T, String>`, calls core with `.map_err(|e| e.to_string())`. See `commands/launch.rs:25-28`.

### Serde Conventions

- **Enums**: `#[serde(rename_all = "snake_case")]` → TypeScript string unions
- **Structs**: No `rename_all` — Rust `snake_case` matches TypeScript directly
- **Output-only types**: Derive `Serialize` only. Input types derive `Serialize + Deserialize + Default` with `#[serde(default)]`

### React Hook (Simple useState)

Follow `useProfile.ts` pattern: individual `useState` hooks, `invoke()` in async function, error normalization. NOT the reducer pattern from `useLaunchState.ts`.

### Module Registration

New modules: `pub mod preview;` in `mod.rs`, key types re-exported with `pub use`.

### BEM CSS + Data Attributes

Classes: `crosshook-{component}__{element}--{modifier}`. State-driven styling via `data-severity`, `data-phase`. See `LaunchPanel.tsx:120-137`.

### Test Fixtures

Use `tempfile::TempDir`, factory functions like `steam_request()` / `proton_request()`, `ScopedCommandSearchPath` for PATH isolation. See `request.rs:655+`.

---

## Cross-Cutting Concerns

### 1. Validation Dispatch Duplication

`validate_all()` mirrors `validate()` dispatch logic but collects instead of short-circuiting. Individual checks should be extracted as shared helpers where feasible to minimize duplication, but `validate()` must remain unchanged to avoid regressions.

### 2. Environment Collection Duplication

New pure functions in `preview.rs` mirror `apply_*` functions in `runtime_helpers.rs`. Intentional duplication — refactoring the `Command`-mutating helpers to be generic would risk the critical launch path. Preview helpers are ~30 lines total.

### 3. Trainer Staging Path Computation

`stage_trainer_into_prefix()` has side effects (file copies). Preview computes staged path via string manipulation only: `C:\CrossHook\StagedTrainers\{stem}\{filename}`. Must stay close to `script_runner.rs` logic to avoid divergence.

### 4. Method-Conditional UI Sections

- `native`: Hide Proton/WINE sections entirely (BR-8, EC-4)
- `steam_applaunch`: Show Steam Launch Options section
- `proton_run`: Show full Proton setup + optimization details

### 5. Frontend Guard Alignment

Preview button must use the same guard as Launch button: disabled when `buildLaunchRequest()` returns null (empty game path) OR when `phase !== Idle` (launch in progress).

### 6. Gamepad Navigation

Modal must set `data-crosshook-focus-root="modal"`. Controller prompts: A=expand, B=close, X=copy, Y=launch. 48px min touch targets. Focus trap with Tab cycling through sections to footer.

---

## Parallelization Opportunities

### Independent Streams

| Stream                 | Tasks                                                                                                        | Dependencies                                  |
| ---------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------- |
| **Backend Types**      | Define `LaunchPreview` + supporting types in `preview.rs`, add `Serialize/Deserialize` to `LaunchDirectives` | None                                          |
| **Backend Validation** | Implement `validate_all()` + method-specific collectors in `request.rs`                                      | None                                          |
| **Frontend Types**     | Add TypeScript interfaces in `launch.ts`                                                                     | None                                          |
| **Backend Function**   | Implement `build_launch_preview()` in `preview.rs`                                                           | Depends on Backend Types + Backend Validation |
| **Tauri Wiring**       | Add `preview_launch` command + register in handler                                                           | Depends on Backend Function                   |
| **Frontend Hook**      | Create `usePreviewState.ts`                                                                                  | Depends on Frontend Types                     |
| **Frontend UI**        | Add button + modal in `LaunchPanel.tsx`                                                                      | Depends on Frontend Hook + Frontend Types     |
| **Tests**              | Unit tests for `build_launch_preview()` and `validate_all()`                                                 | Depends on Backend Function                   |

### Batch Execution

- **Batch 1** (fully parallel): Backend Types, Backend Validation, Frontend Types
- **Batch 2** (after Batch 1): Backend Function, Frontend Hook
- **Batch 3** (after Batch 2): Tauri Wiring, Frontend UI
- **Batch 4** (after Batch 3): Tests

---

## Implementation Constraints

### Gotchas

1. **`chrono` dependency**: Feature spec uses `chrono::Utc::now().to_rfc3339()` but `chrono` may not be in `Cargo.toml`. Verify before using — may need `chrono = "0.4"` added, or use `std::time::SystemTime` with manual formatting.

2. **`validate_proton_run()` calls `resolve_launch_directives()`**: At `request.rs:505`, proton validation internally calls directive resolution. For `validate_all()`, directive errors must be collected alongside path errors, not treated separately.

3. **Steam vs Proton env differ**: `apply_steam_proton_environment()` (script_runner.rs) hardcodes `compatdata_path + "/pfx"` while `apply_runtime_proton_environment()` (runtime_helpers.rs) uses `resolve_wine_prefix_path()` heuristic. Preview must use correct resolution per method.

4. **`stage_trainer_into_prefix()` has side effects**: File copies at script_runner.rs:227-266. Preview MUST NOT call this — compute staged path via string manipulation only.

5. **`LAUNCH_OPTIMIZATION_DEFINITIONS` is private**: Cannot iterate from `preview.rs`, but `resolve_launch_directives()` is public and returns the resolved output — use that.

6. **All validation severities are Fatal**: `ValidationError::severity()` always returns `Fatal`. Consider introducing `Warning` for non-blocking issues in follow-up, but for MVP just collect existing Fatal issues.

7. **`LaunchDirectives` needs Serialize/Deserialize**: One-line change at `optimizations.rs:17` — unblocks preview struct serialization.

### Performance

- All computation: <1ms (pure functions + minor fs stat calls)
- JSON payload: ~2-5KB
- Tauri command: synchronous (no `async fn` needed)
- No new external dependencies required (except possibly `chrono`)

### Phasing

- **Phase A (MVP/this PR)**: `preview.rs`, `validate_all()`, Tauri command, TS types, hook, button + modal, unit tests
- **Phase B (follow-up)**: Copy-to-clipboard, collapsible section defaults, CLI `crosshook preview`, toast confirmations
- **Phase C (cross-feature)**: Reuse `LaunchPreview` for #36 diagnostics, #49 bundle

---

## Key Recommendations

1. **Start with Backend Types + Frontend Types in parallel** — they have zero dependencies and unblock everything else.

2. **`validate_all()` is the highest-risk backend task** — it mirrors `validate()` dispatch logic and must handle the `validate_proton_run() → resolve_launch_directives()` nesting correctly. Allocate careful attention here.

3. **Env collection helpers should be compact** — ~30 lines total mirroring `runtime_helpers.rs`. Don't over-engineer; these change infrequently.

4. **Modal UI should reuse `ProfileReviewModal` patterns exactly** — portal, focus trap, inert siblings, `data-crosshook-focus-root="modal"`. Don't reinvent.

5. **Section defaults matter for UX**: Summary (always visible), Validation (expanded), Command Chain (expanded), Env Vars (collapsed), Proton Setup (collapsed, hidden for native).

6. **Guard alignment is critical** — Preview button disabled state must match Launch button exactly: `!request || phase !== 'idle'`.

7. **Test with all three launch methods** — `steam_applaunch`, `proton_run`, `native` each produce different preview shapes (different sections visible/hidden, different env vars).
