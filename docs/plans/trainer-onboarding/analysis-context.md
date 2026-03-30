# Context Analysis: trainer-onboarding

## Executive Summary

Trainer-onboarding (issue #37, P0) adds a first-run modal wizard with system readiness checks, trainer acquisition guidance, and a chained auto-populate → profile → launch workflow. Implementation composes existing primitives with zero new dependencies across 8 new files and 6 modified files, organized into 5 sequential-but-partially-parallelizable phases — with **Phase 0 security hardening as a non-optional prerequisite**.

---

## Architecture Context

- **System Structure**: Tauri v2 app — `crosshook-core` (all business logic), `src-tauri` (thin IPC shell), React 18 frontend. New `onboarding/` module added to `crosshook-core` only; three commands in `commands/onboarding.rs`; modal wizard and stage-machine hook on frontend.
- **Data Flow**: `lib.rs` startup → `settings.load()` → emit `onboarding-check` (350ms delay) → `App.tsx` listener → `setShowWizard(true)` → `useOnboarding.ts` stage machine → `check_readiness` invoke → readiness results → user advances → `dismiss_onboarding` invoke → `settings.onboarding_completed = true` persisted.
- **Integration Points**: `SettingsStore` (TOML flag), `discover_steam_root_candidates()` + `discover_compat_tools()` (readiness), `attempt_auto_populate()` (wizard step 3), `ProfileFormSections.tsx` + `AutoPopulate.tsx` (composites in wizard), `HealthIssue` type (reused directly in `ReadinessCheckResult`).

---

## Critical Files Reference

| File                                                | Why Critical                                                                                                  |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/settings/mod.rs`         | Add `onboarding_completed: bool`; understand `#[serde(default)]` and load-mutate-save pattern before touching |
| `crates/crosshook-core/src/lib.rs`                  | Module registry — add `pub mod onboarding;` here                                                              |
| `crates/crosshook-core/src/profile/health.rs`       | `HealthIssue` / `HealthIssueSeverity` — reused directly by `ReadinessCheckResult`; no parallel type needed    |
| `crates/crosshook-core/src/steam/discovery.rs`      | `discover_steam_root_candidates()` — backbone of `steam_installed` check                                      |
| `crates/crosshook-core/src/steam/proton.rs`         | `discover_compat_tools()` — backbone of `proton_available` check                                              |
| `crates/crosshook-core/src/community/taps.rs`       | Phase 0: W-1 branch injection fix + W-2 URL scheme allowlist in `normalize_subscription()`                    |
| `crates/crosshook-core/src/launch/script_runner.rs` | Phase 0: A-1 symlink skip in `copy_dir_all()`                                                                 |
| `crates/crosshook-core/src/export/launcher.rs`      | Phase 0: A-4 `%` → `%%` in `escape_desktop_exec_argument()`                                                   |
| `src-tauri/src/lib.rs`                              | Register 3 new commands; emit `onboarding-check` event; startup pattern at lines 59–70                        |
| `src-tauri/src/commands/settings.rs`                | Canonical `State<'_, SettingsStore>` sync command pattern                                                     |
| `src-tauri/src/commands/steam.rs`                   | `list_proton_installs()` chains discovery — reuse for readiness                                               |
| `src-tauri/src/commands/shared.rs`                  | `sanitize_display_path()` — must apply to all path strings in readiness messages                              |
| `src/hooks/useInstallGame.ts`                       | **Canonical stage-machine hook** — `useOnboarding.ts` mirrors this exactly                                    |
| `src/components/ProfileReviewModal.tsx`             | **Canonical portal modal** — focus trap, `inert`/`aria-hidden`, `data-crosshook-focus-root="modal"`           |
| `src/App.tsx`                                       | `listen('onboarding-check')`, `showWizard` state, conditional `<OnboardingWizard>` render                     |
| `src/types/health.ts`                               | TypeScript `HealthIssue` interface — import directly in `types/onboarding.ts`                                 |

---

## Patterns to Follow

- **Stage-Machine Hook**: `useInstallGame.ts` — stage as string union, derived booleans (`isReadinessCheck`, etc.), pure `deriveStatusText()`/`deriveHintText()`, `useCallback`-wrapped async driver, `reset()`. No `useReducer`, no React Context.
- **Portal Modal**: `ProfileReviewModal.tsx` — `createPortal`, `inert`/`aria-hidden` on siblings, `data-crosshook-focus-root="modal"` on root, `data-crosshook-modal-close` on close/back buttons, `role="dialog"`, `aria-modal="true"`, `aria-labelledby` with `useId()`.
- **Tauri IPC Command**: `commands/settings.rs` — sync: `fn(State<'_, SettingsStore>) -> Result<T, String>` with `.map_err(|e| e.to_string())`. Pure no-state functions need no `State` parameter. Async blocking: `spawn_blocking`.
- **Startup Event Emission**: `lib.rs:59-70` — `tauri::async_runtime::spawn` + `sleep(Duration::from_millis(350))` + `app_handle.emit(...)`. New `onboarding-check` must follow same pattern.
- **Settings Load-Mutate-Save**: Always `store.load() → mutate field → store.save()`. Never construct a fresh default — overwrites all other fields.
- **HealthIssue Reuse**: `ReadinessCheckResult.checks: Vec<HealthIssue>` — no parallel `ReadinessCheck` type. Same Rust type, same serde, same TS interface.
- **IPC Error Routing (Frontend)**: `normalizeErrorMessage(err)` → `mapValidationErrorToField(msg)` → `setFieldError`/`setGeneralError`. Pattern in `useInstallGame.ts:90-137`.
- **Rust Module Structure**: New module = `src/<name>/mod.rs` + `pub mod <name>;` in `lib.rs`. Command module = `commands/<name>.rs` + `pub mod <name>;` in `commands/mod.rs` + register in `lib.rs` `invoke_handler!`.
- **Rust Testing**: Inline `#[cfg(test)] mod tests` with `tempfile::tempdir()` + `SettingsStore::with_base_path()`. Command signature tests: compile-time contract pattern in `commands/settings.rs:38-51`.

---

## Cross-Cutting Concerns

- **Phase 0 is a hard prerequisite** — W-1 and W-2 must land before onboarding ships because wizard actively guides users to add community taps; the security surface becomes user-facing.
- **`sanitize_display_path()` applies everywhere** — all path strings in readiness check messages must pass through `commands/shared.rs::sanitize_display_path()` before reaching the frontend (A-6).
- **Wizard placement is inside `AppShell()`** — wizard composes `ProfileFormSections.tsx` which requires `ProfileContext`; the conditional render belongs inside the `AppShell` function body (where route state and context providers live), not inside the top-level `App()` function. The `listen('onboarding-check')` handler and `showWizard` state both belong here too.
- **Existing users on upgrade** — `onboarding_completed` defaults `false` via `#[serde(default)]`. Startup logic must check `profile_store.list().len() > 0` and suppress modal (show banner only) for users with existing profiles.
- **`onboarding_completed` naming** — `research-business.md` uses `onboarding_dismissed`; canonical name is `onboarding_completed` (feature-spec). Use only `onboarding_completed`.
- **`check_readiness` must not touch `MetadataStore`** — SQLite store may be disabled (`MetadataStore::disabled()` pattern). All four readiness checks are pure filesystem operations; no SQLite dependency.
- **Settings overwrite hazard** — `SettingsStore::save()` overwrites entire TOML. Any command that touches settings must load first or it erases `auto_load_last_profile`, `last_used_profile`, `community_taps`.
- **Steam Deck file picker** — OS-native dialogs don't support gamepad. Every path input field must provide typed input alongside browse button. `ShowFloatingGamepadTextInput` for keyboard entry in controller mode.

---

## Parallelization Opportunities

| Parallel Work                                         | Notes                                                                                                                                                           |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase 0: W-1+W-2 coupled, A-1 and A-4 independent** | W-1+W-2 share `normalize_subscription()` → one task (0-AB); A-1 (`script_runner.rs`) and A-4 (`launcher.rs`) are independent → 3 parallel task groups total     |
| **Phase 1: Backend vs. Frontend types**               | `onboarding/` core module + Tauri commands vs. `types/onboarding.ts` type definitions — parallel until banner needs types                                       |
| **Phase 2 vs. Phase 3**                               | Guided workflow UI (Phase 2) and Trainer guidance content (Phase 3) are independent after `useOnboarding.ts` hook is merged                                     |
| **Phase 2 components**                                | `ReadinessChecklist.tsx`, `TrainerGuidance.tsx`, and profile creation step components can all be built in parallel after `OnboardingWizard.tsx` skeleton exists |
| **Tests**                                             | Rust unit tests for each readiness function can be written alongside implementation                                                                             |

---

## Implementation Constraints

- **Zero new Rust dependencies** — `reqwest`, `zip`, `goblin`/`pelite` all deferred. All v1 checks use existing `crosshook-core` functions.
- **Zero new npm packages** — all UI built from existing React + Tauri API packages.
- **No SQLite migration** — persistence is TOML-only. DB schema v10 unchanged for v1. Phase 4 writes to existing `health_snapshots` and `version_snapshots` tables.
- **No new Tauri capabilities** — `core:default` covers all three commands. `opener:allow-open-url` only needed if "Install Steam" button is added (post-v1 only).
- **`check_readiness` is sync** — all 4 checks combined are <200ms. No `spawn_blocking` needed. Do not make it async.
- **Compatdata check is inline** — no existing utility for `steamapps/compatdata/*/pfx` scan; implement directly in `readiness.rs` using `fs::read_dir().any(|e| e.path().join("pfx").is_dir())`.
- **Modal is not a new route** — wizard is a portal overlay, not added to `ContentArea.tsx` route switch. Exhaustive `never` check in `ContentArea.tsx` would cause compile error for a new `AppRoute`.
- **No auto-save in wizard** — profile only persists on explicit save at review step. Abandon = no cleanup needed (state in hook only).
- **Guidance content is `&'static str`** — never loaded from community taps or external sources; compiled into binary. Prevents tap-injected phishing URLs.

---

## Key Recommendations

- **Task ordering**: Phase 0 security → Phase 1 backend (core module + settings + commands) → Phase 1 frontend (types + empty-state banner) → Phase 2 hook → Phase 2 wizard components (parallel) ↔ Phase 3 guidance (parallel with Phase 2) → Phase 4 polish.
- **Start with the settings flag**: 10-line change, unlocks all downstream work, safe to merge independently.
- **Write `useOnboarding.ts` before any wizard components**: All wizard components are presentational and depend on the hook's stage type.
- **Phase 0 runs as 3 parallel task groups**: W-1+W-2 together (both in `normalize_subscription()`), A-1 symlink skip, A-4 `%` escaping — each 1–5 lines in isolated files.
- **Empty-state banner on ProfilesPage is a free quick win**: Zero backend work — render when `profiles.length === 0`. Can ship independently of wizard.
- **Integration test boundary**: Test `check_system_readiness()` unit tests with `tempfile` directories — it's a pure function with no managed state, easy to test in isolation.
- **Watch for existing users**: Startup event payload should include both `show: bool` AND whether profiles exist — lets frontend decide modal vs. banner without a second IPC call.
