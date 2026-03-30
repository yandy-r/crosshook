# Practices Research: trainer-onboarding

## Executive Summary

The trainer-onboarding feature maps cleanly onto patterns already established in the codebase. The `DiagnosticCollector`, install validation helpers, `useInstallGame` stage-machine hook, and `InstallField` / `CollapsibleSection` UI primitives cover the majority of what onboarding needs without new abstractions. The biggest risk is over-engineering: a single Tauri command returning a flat readiness struct, one page-level hook following the `useInstallGame` pattern, and a setting flag for first-run state are sufficient — no new React context, no new SQLite table, no wizard framework.

---

## Existing Reusable Code

| Module / Utility                  | Location                                                           | Purpose                                                                                                             | How to Reuse                                                                                                                            |
| --------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `DiagnosticCollector`             | `crates/crosshook-core/src/steam/diagnostics.rs:6`                 | Collects + deduplicates diagnostic strings and manual hints                                                         | Use directly for onboarding readiness message collection; its `add_diagnostic` / `add_hint` / `finalize` API is exactly the right shape |
| Install path validators           | `crates/crosshook-core/src/install/service.rs:157–238`             | `validate_optional_trainer_path`, `is_windows_executable`, `is_executable_file`                                     | Call these (or factor them to `crosshook-core/src/validation.rs`) for readiness file-existence checks                                   |
| `attempt_auto_populate`           | `crates/crosshook-core/src/steam/auto_populate.rs:12`              | Discovers Steam root, app ID, compatdata path, Proton path from a game executable                                   | Step 1 of the guided flow; invoke this directly rather than re-implementing Steam discovery                                             |
| `TrainerLoadingMode`              | `crates/crosshook-core/src/profile/models.rs:51`                   | 2-variant enum (`SourceDirectory` / `CopyToPrefix`) with `as_str` + `FromStr`                                       | Use as-is for explaining modes in onboarding guidance text; no extension needed                                                         |
| `InstallGameValidationError` enum | `crates/crosshook-core/src/install/models.rs:62`                   | Typed validation errors with `.message()`                                                                           | Pattern-match for readiness issue reporting; follow this enum-per-error style for a `ReadinessIssue` type                               |
| `HealthIssue` struct              | `crates/crosshook-core/src/profile/health.rs:31`                   | `field / path / message / remediation / severity` struct for surfacing issues                                       | Reuse the same struct shape for readiness issues, or literally reuse `HealthIssue` with new `field` values                              |
| `useInstallGame` hook             | `src/crosshook-native/src/hooks/useInstallGame.ts:244`             | Multi-stage workflow: idle → preparing → running → review → ready_to_save with `statusText` / `hintText` derivation | Copy this stage-machine + derived-text pattern for `useOnboardingFlow`; do not invent a new pattern                                     |
| `useLaunchState` hook             | `src/crosshook-native/src/hooks/useLaunchState.ts:137`             | `useReducer` state machine + Tauri event listener cleanup                                                           | Reference for reducer + cleanup pattern; onboarding may listen for a `first-run-complete` event                                         |
| `InstallField` component          | `src/crosshook-native/src/components/ui/InstallField.tsx:5`        | Label + text input + browse button + helpText + error message                                                       | Use directly for trainer path selection and game executable selection in onboarding                                                     |
| `CollapsibleSection` component    | `src/crosshook-native/src/components/ui/CollapsibleSection.tsx:13` | Controlled/uncontrolled collapse with title and meta slot                                                           | Use for "What is a trainer?" / "Which mode should I choose?" guidance sections                                                          |
| `ProfileContext` pattern          | `src/crosshook-native/src/context/ProfileContext.tsx:29`           | Context = hook wrapper + derived values, Tauri event listener in `useEffect`                                        | Follow this pattern exactly if onboarding state must cross page boundaries — but first check whether a page-level hook suffices         |
| `MetadataStore` + migrations      | `crates/crosshook-core/src/metadata/migrations.rs`                 | Schema-versioned SQLite migrations (currently at v10)                                                               | If onboarding completion state needs persistence beyond settings TOML, add a v11 migration; otherwise use the existing `settings.rs`    |
| App settings mod                  | `crates/crosshook-core/src/settings/mod.rs`                        | Reads/writes `~/.config/crosshook/settings.toml`                                                                    | Cheapest place to persist `onboarding_completed: bool` — no schema migration required                                                   |

---

## Modularity Design

### Recommended Rust Boundaries

```
crosshook-core/src/
  onboarding/
    mod.rs          # module root; re-exports ReadinessReport, ReadinessIssue
    readiness.rs    # check_trainer_onboarding_readiness(steam_root, proton_path, trainer_path) -> ReadinessReport
```

**Why a sub-module and not inline in a Tauri command file?**

- Health checks live in `crosshook-core/src/profile/health.rs` (not in `src-tauri`); readiness checks are analogous — they validate system state and should be independently testable without Tauri
- One exception: if readiness checks are just 3-4 `if path.exists()` calls, skip the sub-module and put them directly in `src-tauri/src/commands/install.rs` or a new `onboarding.rs` command file

**Shared vs. feature-specific:**

- `DiagnosticCollector` — shared; don't copy
- `validate_optional_trainer_path` — promote to `crosshook-core/src/validation.rs` only if onboarding is the _second_ caller; until then, inline a copy (rule of three)
- `TrainerLoadingMode` — shared; use from `profile::models`

### Recommended Frontend Boundaries

```
src/crosshook-native/src/
  hooks/
    useOnboardingFlow.ts     # stage machine + readiness state + step advancement
  components/pages/
    OnboardingPage.tsx       # OR a panel inside InstallPage.tsx (UX decision)
  types/
    onboarding.ts            # OnboardingStage, ReadinessReport, ReadinessIssue types
```

Do **not** create an `OnboardingContext` unless onboarding state is needed by more than one page simultaneously — `ProfileContext` already wraps a hook, so the pattern exists but adding contexts has a cost (indirection, harder to trace).

---

## KISS Assessment

| Area                          | Current Proposal Risk                                     | Simpler Alternative                                                                         | Trade-off                                                                                                                                   |
| ----------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Readiness check persistence   | New SQLite table `onboarding_state`                       | Single boolean `onboarding_completed` in `settings.toml` via `AppSettings`                  | Settings TOML is already read on startup; SQLite adds schema migration and a `MetadataStore` dependency with no benefit for a single flag   |
| Onboarding state sharing      | New `OnboardingContext`                                   | Page-level `useOnboardingFlow` hook                                                         | Context is necessary when multiple disconnected components consume the same state; a single page flow doesn't need it                       |
| Readiness "report" type       | New `OnboardingReadiness` struct with nested step structs | Flat struct mirroring `SteamAutoPopulateResult` or just `Vec<ReadinessIssue> + Vec<String>` | Nested step structs are readable but add serde surface area; flat is sufficient for the first version                                       |
| Wizard navigation abstraction | Generic `WizardStep` / `WizardManager`                    | Inline step enum in `useOnboardingFlow` (same as `InstallGameStage`)                        | The codebase has zero prior uses of a wizard abstraction; inventing one for a single feature is premature                                   |
| Trainer source guidance       | Inline hint strings in Rust command                       | Separate `trainer_sources` catalog in TOML/JSON                                             | Hint strings are static, few in number, and UI-copy that changes rarely; externalizing them adds a read + parse path for no runtime benefit |
| Step 3 "first-run readiness"  | Dedicated Rust service                                    | `check_onboarding_readiness` free function with Path args                                   | The checks are 3-5 `fs::metadata` calls; a service struct adds `new()`, state, and lifetime complexity for no gain                          |

---

## Abstraction vs. Repetition

- **`DiagnosticCollector`** — already used in `auto_populate.rs` (Steam) and `steam/discovery.rs`. Onboarding would be a third consumer. Use it directly; it crossed the rule-of-three threshold.

- **Path existence validators** (`validate_optional_trainer_path`, `is_windows_executable`) — currently private to `install/service.rs`. Onboarding needs the same checks. This is the second consumer — promote to a shared location only if a third appears, or if onboarding and install share a test fixture. Until then, inline. **Do not** create a `validation` module prematurely.

- **Stage-machine + `statusText`/`hintText` pattern** — already in `useInstallGame` and `useLaunchState`. Onboarding is the third consumer. Pattern is established — follow it exactly: pure derivation functions outside the hook body (like `deriveStatusText`, `deriveHintText` in `useInstallGame.ts:186`), `useReducer` for complex state or `useState` for simple linear stages.

- **`HealthIssue` struct** — has the right shape (`field / path / message / remediation / severity`). Onboarding readiness issues share the same anatomy. Reuse the struct directly rather than defining `ReadinessIssue` with identical fields. If divergence is needed later, add a newtype.

---

## Interface Design

### Rust IPC Command

```rust
// src-tauri/src/commands/onboarding.rs

#[tauri::command]
pub fn check_trainer_onboarding_readiness(
    steam_client_install_path: String,
    proton_path: String,
    trainer_path: String,
) -> Result<TrainerReadinessReport, String>
```

**`TrainerReadinessReport`** (mirrors `SteamAutoPopulateResult` field-state pattern):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct TrainerReadinessReport {
    pub steam_found: bool,
    pub proton_found: bool,
    pub game_launched_once: bool,   // compatdata dir exists
    pub trainer_present: bool,
    pub issues: Vec<HealthIssue>,   // reuse existing type
    pub hints: Vec<String>,
}
```

- Accepts explicit paths rather than reading from global state — same as `validate_optional_trainer_path(path: &str)`
- Returns `HealthIssue` (existing type) for issues — no new serialization surface
- Does not take `ProfileStore` state; reads filesystem only

### React Hook Interface

```typescript
// useOnboardingFlow.ts

export type OnboardingStep =
  | 'idle'
  | 'checking_readiness'
  | 'readiness_result'
  | 'trainer_selection'
  | 'mode_selection'
  | 'complete';

export interface UseOnboardingFlowResult {
  step: OnboardingStep;
  readinessReport: TrainerReadinessReport | null;
  trainerPath: string;
  loadingMode: TrainerLoadingMode;
  isComplete: boolean;
  statusText: string;
  hintText: string;
  setTrainerPath: (path: string) => void;
  setLoadingMode: (mode: TrainerLoadingMode) => void;
  checkReadiness: (steamPath: string, protonPath: string, trainerPath: string) => Promise<void>;
  advance: () => void;
  reset: () => void;
}
```

Extension point: the hook accepts `onComplete?: (trainerPath: string, mode: TrainerLoadingMode) => void` callback so `InstallPage` or `ProfilesPage` can hydrate the profile form directly when the guided flow finishes — same handoff pattern as `useInstallGame` → `InstallPage` → profile review.

---

## Testability Patterns

### Recommended Patterns

**Rust:**

- Readiness check functions take `&Path` / `&str` arguments, never call `BaseDirs` or read global config — same as `validate_proton_path(path: &str)` in `install/service.rs:192`. This makes tests use `tempfile::tempdir()` without mocking.
- Use `DiagnosticCollector::default()` and call `finalize()` in tests to assert emitted hints — same pattern as `diagnostics.rs:46`.
- Write unit tests in the same file following the existing `#[cfg(test)] mod tests` pattern.

**Frontend:**

- Pure derivation functions (`deriveStatusText`, `deriveHintText`) are testable without React or Tauri — extract them to module scope like `useInstallGame.ts:186`.
- Hook integration tests: if added, use `@testing-library/react-hooks` — but note the CLAUDE.md: _no test framework is configured for the frontend_. Don't add one just for onboarding.

### Anti-Patterns to Avoid

- **Mocking `MetadataStore` or `ProfileStore`** — the install service tests demonstrate real filesystem + real Tokio runtime are affordable; keep integration tests real
- **Onboarding hook that reads from `ProfileContext`** — creates hidden coupling; pass data explicitly via props or hook arguments
- **`useEffect` chain** — `useLaunchState.ts` uses exactly one `useEffect` per concern (reset on profile change, event listener); don't combine them into one big effect

---

## Build vs. Depend

| Need                         | Build Custom                                             | Use Library                                    | Recommendation             | Rationale                                                                                                               |
| ---------------------------- | -------------------------------------------------------- | ---------------------------------------------- | -------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Step-by-step UI flow         | Inline stage enum + hook                                 | React wizard library (react-step-wizard, etc.) | **Build**                  | `useInstallGame` already proves the pattern works with 30 lines; adding a library for a 5-step flow is over-engineering |
| Readiness checks             | Thin wrapper over `fs::metadata` + `DiagnosticCollector` | —                                              | **Build** (trivial)        | No library exists for this domain-specific check; it's 5-10 `if` branches                                               |
| Trainer file browsing        | Tauri `dialog` plugin (already used)                     | —                                              | **Existing**               | `InstallField` already calls `chooseFile` / `chooseDirectory` via `src/utils/dialog.ts`; reuse that                     |
| Onboarding state persistence | `settings.rs` append `onboarding_completed`              | SQLite via `MetadataStore`                     | **Existing settings TOML** | Single boolean; no query, no migration overhead                                                                         |
| Trainer source guidance text | Inline hint strings in Rust command                      | External TOML/JSON catalog                     | **Inline**                 | Static copy; externalizing is premature until there are >10 sources that change frequently                              |
| Loading mode explanation UI  | `CollapsibleSection` + inline text                       | Tooltip library                                | **Existing UI primitive**  | `CollapsibleSection` already renders `children` — a `<p>` tag inside it is sufficient                                   |

---

## Open Questions

1. **Where does the onboarding flow surface in the UI?** — A new sidebar page ("Onboarding") vs. a collapsible entry section on InstallPage vs. a first-run modal. This determines whether a page component or a modal + state is needed, and whether `ProfileContext` must be aware of onboarding completion.

2. **Does "game launched once" mean compatdata directory exists, or a `launch_operations` row in SQLite?** — The filesystem check (compatdata dir presence) works without MetadataStore; the SQLite check is more precise but depends on `MetadataStore` availability. Clarify before implementing.

3. **Should the guided flow auto-populate after trainer selection?** — `attempt_auto_populate` already discovers the game; if onboarding chains into auto-populate it becomes a 4-step wizard (readiness → trainer select → auto-populate → profile review). This has UX implications the tech design should clarify.

4. **Is the `trainer_type` field (FLiNG, WeMod, standalone) needed for readiness, or is it purely informational?** — `TrainerSection.kind` exists in the profile but is a free-form string. If onboarding is expected to validate trainer type compatibility, a known set of type strings needs to be defined.

5. **Onboarding on every fresh profile or only on first app launch?** — The `settings.rs` approach persists a single boolean; per-profile onboarding would need SQLite or a profile-level TOML field.
