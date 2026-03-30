# Task Structure Analysis: Trainer Onboarding

## Executive Summary

The trainer-onboarding feature maps cleanly onto 22 discrete implementation tasks across 5 phases. Phase 0 (security) contains 4 fully independent tasks best parallelized 4-way. Phase 1 splits into a backend track (3 tasks) and a frontend types track (2 tasks) running concurrently. Phase 2 and Phase 3 are partially parallel: the `useOnboarding.ts` hook must land first, then 4 component tasks can run concurrently alongside Phase 3 content tasks. Phase 4 (polish) serializes only after Phase 2 completes. The critical path runs: `Phase 0 → Phase 1 backend → Phase 1 frontend types → useOnboarding.ts hook → wizard components → Phase 4`.

---

## Recommended Phase Structure

### Phase 0 — Security Hardening (4 parallel tasks)

All fixes are in separate files with no shared state. Run all 4 concurrently.

| Task                          | File(s)                   | Lines              | Type         |
| ----------------------------- | ------------------------- | ------------------ | ------------ |
| 0-A: W-1 Branch injection     | `community/taps.rs`       | ~209, 240, 362–387 | Security fix |
| 0-B: W-2 URL scheme allowlist | `community/taps.rs`       | ~362–387           | Security fix |
| 0-C: A-1 Symlink skip         | `launch/script_runner.rs` | ~310–326           | Advisory fix |
| 0-D: A-4 `%` escaping         | `export/launcher.rs`      | ~593–598           | Advisory fix |

**Why these group well**: Tasks 0-A and 0-B both touch `normalize_subscription()` in `taps.rs` and must therefore be serialized within that function, but they are independent of 0-C and 0-D. Consider merging 0-A + 0-B into one task since they are in the same function — splitting them risks a merge conflict and adds no parallelism benefit.

**Recommended merge**: Combine 0-A and 0-B into a single `taps.rs` security task (branch injection + URL allowlist together). Result: 3 truly independent Phase 0 tasks.

---

### Phase 1 — Backend Readiness + Settings Flag

Two parallel tracks. Track A (backend core) and Track B (frontend types) have no dependencies on each other.

**Track A — Backend (sequential within track)**

| Task                       | File(s)                                                                        | Depends On         |
| -------------------------- | ------------------------------------------------------------------------------ | ------------------ |
| 1-A1: Core module skeleton | `crosshook-core/src/onboarding/mod.rs` + `readiness.rs` (create)               | Phase 0 done       |
| 1-A2: Settings flag        | `crosshook-core/src/settings/mod.rs` + `crosshook-core/src/lib.rs`             | 1-A1 types defined |
| 1-A3: Tauri command layer  | `src-tauri/src/commands/onboarding.rs` (create) + `commands/mod.rs` + `lib.rs` | 1-A1, 1-A2         |

**Track B — Frontend Types (parallel with Track A)**

| Task                                  | File(s)                                                   | Depends On   |
| ------------------------------------- | --------------------------------------------------------- | ------------ |
| 1-B1: TypeScript types                | `src/types/onboarding.ts` (create) + `src/types/index.ts` | Phase 0 done |
| 1-B2: ProfilesPage empty-state banner | `src/components/pages/ProfilesPage.tsx`                   | 1-B1         |

**Key constraint**: 1-A1 and 1-B1 can start in parallel immediately after Phase 0. 1-A2 (`settings/mod.rs`) has no dependency on 1-A1 — adding `onboarding_completed: bool` to `AppSettingsData` is a standalone struct field addition that can land first or concurrently with 1-A1. 1-A3 is blocked by both (needs the module types and the settings store extension). The `lib.rs` startup event emission (`onboarding-check`) belongs in 1-A3 alongside the command registration — do not split them.

**Testing note for 1-A1**: `check_system_readiness()` takes no injected state (`SettingsStore`, `MetadataStore`, etc.) — it calls `discover_steam_root_candidates("")` and `discover_compat_tools()` directly. Tests only need `std::fs::create_dir_all(tempdir.path().join(".steam/root/steamapps"))` to produce meaningful results. No Tauri harness, no mocking. Tests belong inline in `readiness.rs`; the module re-export is `pub use readiness::check_system_readiness;` in `onboarding/mod.rs`.

---

### Phase 2 — Guided Workflow UI

One serial entry point (`useOnboarding.ts` hook), then 4 parallel component tasks.

| Task                                    | File(s)                                          | Depends On | Parallelizable?                                                            |
| --------------------------------------- | ------------------------------------------------ | ---------- | -------------------------------------------------------------------------- |
| 2-0: `useOnboarding.ts` hook            | `src/hooks/useOnboarding.ts` (create)            | 1-A3, 1-B1 | — (serial entry point)                                                     |
| 2-A: `ReadinessChecklist.tsx`           | `src/components/ReadinessChecklist.tsx` (create) | 2-0        | Yes, with 2-B/C/D                                                          |
| 2-B: `OnboardingWizard.tsx` modal shell | `src/components/OnboardingWizard.tsx` (create)   | 2-0        | Yes, with 2-A/C/D                                                          |
| 2-C: `App.tsx` wiring                   | `src/App.tsx`                                    | 2-B        | After 2-B skeleton; render goes inside `AppShell()`, not top-level `App()` |
| 2-D: `ControllerPrompts` extension      | `src/components/layout/ControllerPrompts.tsx`    | none       | Immediately                                                                |

**Why `useOnboarding.ts` is the serial gate**: Every wizard component imports from this hook. The `useInstallGame.ts` pattern is well-understood (580 lines), and `useOnboarding.ts` will follow the same structure. It defines the `OnboardingWizardStage` union, the `useOnboarding()` return shape, and `deriveStatusText`/`deriveHintText`/`deriveActionLabel` — all consumed by every wizard component. Attempting to parallelize component work before the hook API is stable causes interface churn.

**2-C (`App.tsx`) constraint**: The wizard modal render belongs inside `AppShell()`, not the top-level `App()` function — `AppShell` is where route state and `ProfileProvider` context live, making it the correct layer for a conditional modal overlay. The change is lightweight: add `listen<T>('onboarding-check')` in a `useEffect`, add `onboardingOpen` state, conditionally render `<OnboardingWizard />`. It can start once 2-B has defined the `OnboardingWizard` component signature, even before internal steps are complete.

**2-D can start immediately**: `ControllerPrompts.tsx` extension (adding `confirmLabel`/`backLabel`/`showBumpers` override props) is purely additive, has no dependency on the hook, and should be done early since other Phase 2 components reference it.

---

### Phase 3 — Trainer Guidance Content (parallel with Phase 2 components)

Phase 3 can start once `useOnboarding.ts` (2-0) exists and `commands/onboarding.rs` (1-A3) exists.

| Task                                 | File(s)                                         | Depends On | Notes                                 |
| ------------------------------------ | ----------------------------------------------- | ---------- | ------------------------------------- |
| 3-A: `get_trainer_guidance` backend  | `src-tauri/src/commands/onboarding.rs` (modify) | 1-A3       | Add static content to existing file   |
| 3-B: `TrainerGuidance.tsx` component | `src/components/TrainerGuidance.tsx` (create)   | 2-0, 3-A   | Loading mode cards + WeMod disclaimer |

**Note**: 3-A is a modification to the already-created `commands/onboarding.rs` (from 1-A3), not a new file. The `get_trainer_guidance` command returns `TrainerGuidanceContent` with static `&'static str` constants — zero IPC round-trips needed. 3-A and 3-B can run in parallel since 3-B only needs the TypeScript return type (defined in `types/onboarding.ts` from 1-B1) and the component shell.

---

### Phase 4 — Polish and Integration (sequential, after Phase 2)

| Task                                      | File(s)                                                                       | Depends On          |
| ----------------------------------------- | ----------------------------------------------------------------------------- | ------------------- |
| 4-A: Post-onboarding health check         | `src-tauri/src/commands/health.rs` (modify) or trigger from wizard completion | Phase 2 complete    |
| 4-B: Version snapshot on profile creation | `src-tauri/src/commands/version.rs` (modify)                                  | Phase 2 complete    |
| 4-C: Interrupt recovery validation        | Testing/validation only                                                       | Phase 2, 3 complete |
| 4-D: Steam Deck end-to-end test           | Manual QA                                                                     | All phases complete |

---

## Task Granularity Recommendations

### Right-sized tasks (1–3 files each)

The feature-spec's Phase 0–4 structure is sound but needs granularity splits in two areas:

1. **Phase 1 Core Module**: Split `onboarding/mod.rs` (types + re-exports) from `readiness.rs` (business logic). The types need to stabilize first; the readiness logic references existing `steam/discovery.rs` and `steam/proton.rs` functions that already work. Keep these in one task since they're in the same new directory.

2. **Phase 1 Tauri Commands**: `commands/onboarding.rs` creation + `commands/mod.rs` + `lib.rs` (startup event + handler registration) is correctly 3 files in one task. Do not split further — they must be applied atomically or the compiler will reject module references.

3. **Phase 2 Wizard steps within `OnboardingWizard.tsx`**: The trainer setup step (profile creation composing `AutoPopulate.tsx` + game path picker), the trainer path step (composing `InstallField` + loading mode selector), and the profile review step (composing `ProfileFormSections.tsx`) are all sub-sections of `OnboardingWizard.tsx`. Keep them in the same task rather than splitting into separate files — they share step state from the hook.

### Tasks that should NOT be split

- `taps.rs` W-1 + W-2 fixes: both modify `normalize_subscription()`. Merge into one task.
- `commands/onboarding.rs` initial creation + `lib.rs` registration: atomic pair.
- `types/onboarding.ts` + `types/index.ts`: 2-line change to index, trivially bundled.

### Tasks that could be further parallelized

- Phase 0: 0-C (symlink skip, 5 lines) and 0-D (`%` escaping, 1 line) are micro-fixes that could ship immediately, even before the rest of Phase 0.
- Phase 2: `ControllerPrompts.tsx` extension (2-D) has zero dependencies; start it in Sprint 1 alongside Phase 0.

---

## Dependency Analysis

```
Phase 0 (all parallel)
  ├── 0-AB: taps.rs security
  ├── 0-C: script_runner.rs symlink
  └── 0-D: launcher.rs % escaping
        │
Phase 1 (two parallel tracks)
  ├── Track A:
  │     1-A1 → 1-A2 → 1-A3
  └── Track B:
        1-B1 → 1-B2
              │
        (merge point: 1-A3 + 1-B1 both complete)
              │
Phase 2 (hook gates components)
  2-D ──────────────────────────────── (immediate, independent)
  2-0 (useOnboarding.ts) ──────────── (after 1-A3 + 1-B1)
    ├── 2-A (ReadinessChecklist.tsx)   (parallel)
    ├── 2-B (OnboardingWizard.tsx)     (parallel)
    │     └── 2-C (App.tsx wiring)    (after 2-B skeleton)
    │
Phase 3 (parallel with Phase 2 components, after 2-0 + 1-A3)
  ├── 3-A: backend guidance command
  └── 3-B: TrainerGuidance.tsx
        │
Phase 4 (after Phase 2 complete)
  ├── 4-A: health check trigger
  ├── 4-B: version snapshot
  ├── 4-C: interrupt recovery
  └── 4-D: Steam Deck QA
```

**Critical path** (minimum sequential depth): `0-AB → 1-A1 → 1-A2 → 1-A3 → 2-0 → 2-B → 2-C → 4-A`

That's 8 sequential steps. Everything else is parallelizable off this spine.

---

## File-to-Task Mapping

### New Files (8)

| File                                                | Task | Phase |
| --------------------------------------------------- | ---- | ----- |
| `crates/crosshook-core/src/onboarding/mod.rs`       | 1-A1 | 1     |
| `crates/crosshook-core/src/onboarding/readiness.rs` | 1-A1 | 1     |
| `src-tauri/src/commands/onboarding.rs`              | 1-A3 | 1     |
| `src/types/onboarding.ts`                           | 1-B1 | 1     |
| `src/hooks/useOnboarding.ts`                        | 2-0  | 2     |
| `src/components/ReadinessChecklist.tsx`             | 2-A  | 2     |
| `src/components/OnboardingWizard.tsx`               | 2-B  | 2     |
| `src/components/TrainerGuidance.tsx`                | 3-B  | 3     |

### Modified Files (6 from spec + 2 advisory)

| File                                          | Task | Change                                           |
| --------------------------------------------- | ---- | ------------------------------------------------ |
| `crates/crosshook-core/src/lib.rs`            | 1-A1 | `pub mod onboarding;`                            |
| `crates/crosshook-core/src/settings/mod.rs`   | 1-A2 | `onboarding_completed: bool` field               |
| `src-tauri/src/commands/mod.rs`               | 1-A3 | `pub mod onboarding;`                            |
| `src-tauri/src/lib.rs`                        | 1-A3 | 3 command registrations + startup event emission |
| `src/types/index.ts`                          | 1-B1 | `export * from './onboarding';`                  |
| `src/App.tsx`                                 | 2-C  | Event listener + conditional wizard render       |
| `src/components/layout/ControllerPrompts.tsx` | 2-D  | `confirmLabel`/`backLabel`/`showBumpers` props   |
| `src/components/pages/ProfilesPage.tsx`       | 1-B2 | Empty-state "Get Started" banner                 |

### Security Fix Files (Phase 0)

| File                                                | Task | Location                                                         |
| --------------------------------------------------- | ---- | ---------------------------------------------------------------- |
| `crates/crosshook-core/src/community/taps.rs`       | 0-AB | `normalize_subscription()` lines 362–387, git args lines 209/240 |
| `crates/crosshook-core/src/launch/script_runner.rs` | 0-C  | `copy_dir_all()` lines 310–326                                   |
| `crates/crosshook-core/src/export/launcher.rs`      | 0-D  | `escape_desktop_exec_argument()` lines 593–598                   |

---

## Optimization Opportunities

### Quick wins (ship before Phase 1 completes)

1. **`ProfilesPage` empty-state banner** (1-B2): Pure React, no backend needed. Can ship as soon as `types/onboarding.ts` exists. High user-facing value with minimal effort.
2. **`ControllerPrompts.tsx` props extension** (2-D): Purely additive prop interface change; no hook dependency. Ship in Phase 0 sprint.
3. **Phase 0 micro-fixes** (0-C, 0-D): Both are under 5 lines each. Ship as a single fast PR before the larger taps.rs work.

### Parallelization ceiling

Maximum parallelism per phase:

- **Phase 0**: 3 workers (0-AB, 0-C, 0-D)
- **Phase 1**: 2 workers (Track A, Track B)
- **Phase 2 + Phase 3 combined**: 5 workers (2-A, 2-B, 2-D, 3-A, 3-B) after 2-0 lands
- **Phase 4**: 3 workers (4-A, 4-B, 4-C)

### Avoid over-splitting

The `onboarding/mod.rs` + `readiness.rs` pair (1-A1) should not be split across implementors — `mod.rs` defines types consumed by `readiness.rs`, and the Rust module system requires both to exist before either compiles. Similarly, `commands/onboarding.rs` initial creation + `lib.rs` registration must land in the same PR or the build will fail.

---

## Implementation Strategy Recommendations

### 1. Phase 0 as a standalone PR

Ship Phase 0 security fixes as a dedicated PR before any onboarding work. Rationale:

- Low risk, reviewable in isolation
- Unblocks Phase 1 with no rework cost
- The `taps.rs` branch/URL fixes are self-contained and have clear test vectors

### 2. Phase 1 backend lands as a draft PR first

The Rust compiler will catch any IPC contract mistakes early. Land `onboarding/` module + settings flag + Tauri command layer before writing any frontend. This lets frontend developers `invoke('check_readiness')` against a real backend immediately.

### 3. `useOnboarding.ts` as a reviewed checkpoint

The hook defines the entire wizard's state API. Before starting 2-A/2-B/3-B, require a code review of the hook's stage union type and return shape. Preventing API drift here avoids component rewrites downstream. The `useInstallGame.ts` pattern is the template — deviation should be intentional.

### 4. `capabilities/default.json` update belongs in 1-A3

The Tauri capability file must register new command names before they can be invoked from the frontend. It belongs atomically with `commands/onboarding.rs` creation and `lib.rs` registration. Never treat this as a separate task — it's part of the command layer plumbing.

### 5. Tests belong inside the task, not as a follow-up

Unit tests for each Phase 0 fix and Phase 1 readiness functions should be written inline, not deferred to a "testing task." The existing pattern in `src-tauri/src/commands/settings.rs` (compile-time signature tests) and `export/launcher.rs` (doc tests) demonstrates this. Each implementor owns their tests.

### 6. Phase 3 content can drive Phase 2 component placeholders

`TrainerGuidance.tsx` (3-B) can land as a skeleton component with hardcoded placeholder text while 3-A (`get_trainer_guidance` backend) is in-flight. The component invokes `get_trainer_guidance` in a `useEffect` — stub the data shape from `types/onboarding.ts` and hydrate later. This decouples the content work from the UI work.

### 7. WeMod disclaimer is data, not a separate component

The WeMod disclaimer is a conditional render inside `TrainerGuidance.tsx` driven by `TrainerGuidanceEntry.id === 'wemod'` — not a standalone component. Avoid creating a `WeMod Disclaimer.tsx` file; this is a one-time conditional within the guidance content card.

### 8. Startup event timing

The `onboarding-check` event should be emitted at the same delay as `auto-load-profile` (350ms), not the health check delay (500ms). Frontend `useEffect` listeners in `App.tsx` must be registered before both events fire. The current `lib.rs` pattern (spawn + sleep + emit) is correct; the wizard should check `onboarding_completed` from the settings already loaded at startup, not make a separate IPC call.
