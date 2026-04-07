# Plan: UI Standardization Phase 2 — New/Edit Profile Wizard Rework

## Summary

Rebalance the New/Edit Profile wizard so each step has comparable density and a clear goal, expose all profile-creation essentials (Steam App ID, full media art parity, launch preset selection), and eliminate field-graph drift between the wizard and the canonical profile editor by reusing the existing `profile-sections/*` components instead of a parallel inline implementation. Adds a dedicated review step with required-field validation and readiness messaging before save. Storage boundary is unchanged: this is a UI/runtime-only feature that uses the existing profile TOML and metadata models.

## User Story

As a CrossHook user creating or editing a profile, I want a wizard whose steps feel evenly weighted and expose every essential profile field — including art, Steam App ID, launch presets, and a review step — so I can save a complete, valid profile in one pass without bouncing back to the full editor afterward.

## Problem → Solution

The current wizard packs identity, game, media (cover only), runner method, and custom env vars into Step 1 while Steps 2 and 3 are sparse and runner-conditional; key fields (Steam App ID for `proton_run`, portrait/background art, launch presets, validation summary) are absent or inconsistent with the editor → introduce a 5-step wizard that reuses the canonical `profile-sections/*` components, balances density per step, and adds explicit review/validation before save, while keeping the existing dismiss/save invariants and `useOnboarding` semantics intact.

## Metadata

- **Complexity**: Large
- **Source PRD**: N/A (GitHub issue driven)
- **PRD Phase**: `#163` Phase 2 (`#161`)
- **Estimated Files**: 8–10
- **Issue**: [#163](https://github.com/yandy-r/crosshook/issues/163), [#161](https://github.com/yandy-r/crosshook/issues/161)

---

## UX Design

### Before

```text
┌───── Step 1 of 3: Game Setup (overloaded) ──────────────────────────────┐
│ Profile Identity  •  Game (name, path, launcher meta)                   │
│ Media: Cover only + Launcher Icon                                       │
│ Runner Method                                                           │
│ Custom Environment Variables                                            │
└──────────────────────────────────────────────────────────────────────────┘
┌───── Step 2 of 3: Trainer Setup (sparse) ───────────────────────────────┐
│ Trainer Path  •  Loading Mode                                           │
│ (missing: trainer type, network isolation, version)                     │
└──────────────────────────────────────────────────────────────────────────┘
┌───── Step 3 of 3: Runtime Setup (runner-conditional, no review) ────────┐
│ steam_applaunch: App ID, Prefix, Proton Path, AutoPopulate              │
│ proton_run: Prefix, Working Dir, Proton Path  (no Steam App ID anywhere)│
│ native: Working Dir override                                            │
│ → Save Profile (no required-field summary, no preset, no readiness)     │
└──────────────────────────────────────────────────────────────────────────┘
```

### After

```text
┌───── Step 1 of 5: Identity & Game ──────────────────────────────────────┐
│ ProfileIdentitySection  +  GameSection  +  RunnerMethodSection          │
│ Profile Name • Game Name • Game Path • Runner Method                    │
└──────────────────────────────────────────────────────────────────────────┘
┌───── Step 2 of 5: Runtime (runner-conditional, mirrors editor) ─────────┐
│ RuntimeSection (Steam: App ID + Prefix + Proton + AutoPopulate)         │
│                (Proton Run: Prefix + App ID + Proton Path)              │
│                (Native: Working Dir override)                           │
└──────────────────────────────────────────────────────────────────────────┘
┌───── Step 3 of 5: Trainer (skipped on native) ──────────────────────────┐
│ TrainerSection (path, type, loading mode, network isolation, version)   │
└──────────────────────────────────────────────────────────────────────────┘
┌───── Step 4 of 5: Media ────────────────────────────────────────────────┐
│ MediaSection (Cover + Portrait + Background + Launcher Icon when ≠ native)│
└──────────────────────────────────────────────────────────────────────────┘
┌───── Step 5 of 5: Presets & Review ─────────────────────────────────────┐
│ Launch preset picker (bundled + saved) — optional pre-save              │
│ CustomEnvironmentVariablesSection (collapsible, optional)               │
│ Required-field summary  •  System checks  •  Save Profile               │
└──────────────────────────────────────────────────────────────────────────┘
```

### Interaction Changes

| Touchpoint                    | Before                                       | After                                                     | Notes                                                                              |
| ----------------------------- | -------------------------------------------- | --------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Step header                   | `Step N of 3`                                | `Step N of 5` (skip-aware)                                | Native skips Step 3 visibly (shows `4` after `2`), matching forward-skip semantics |
| Field implementation          | Inline JSX duplicating editor controls       | Reused `profile-sections/*` components                    | Eliminates wizard/editor drift                                                     |
| Steam App ID for `proton_run` | Missing                                      | Surfaced via canonical `RuntimeSection`                   | Already optional in `RuntimeSection` for `proton_run`                              |
| Game art                      | Cover only                                   | Cover, Portrait, Background, Launcher Icon                | Reuses `MediaSection`                                                              |
| Custom env vars               | Always visible on Step 1                     | Collapsible, optional, on Review step                     | Reduces Step 1 density                                                             |
| Launch preset                 | Not selectable                               | Optional bundled/saved preset picker on Review step       | Single picker, no per-option exposure                                              |
| Save gate                     | `name` + `executable_path` only              | Pre-save summary listing every required field with status | Required-field check is centralized                                                |
| Readiness checks              | Manual button only                           | Manual button + auto-render of latest result on Review    | No new IPC; reuses `runChecks`                                                     |
| Dismiss / no-write invariant  | BR-9: no profile written until explicit save | Unchanged                                                 | All step transitions remain in-memory only                                         |

---

## Mandatory Reading

| Priority       | File                                                                              | Lines                       | Why                                                                                                                                    |
| -------------- | --------------------------------------------------------------------------------- | --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/src/components/OnboardingWizard.tsx`                        | 1-764                       | Component to rework end-to-end                                                                                                         |
| P0 (critical)  | `src/crosshook-native/src/hooks/useOnboarding.ts`                                 | 1-179                       | Stage state machine + skip semantics that must extend                                                                                  |
| P0 (critical)  | `src/crosshook-native/src/types/onboarding.ts`                                    | 1-30                        | `OnboardingWizardStage` union to extend                                                                                                |
| P0 (critical)  | `src/crosshook-native/src/components/ProfileSubTabs.tsx`                          | 1-280                       | Canonical wiring of all `profile-sections/*` — copy this composition pattern                                                           |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/ProfileIdentitySection.tsx` | 1-86                        | Reusable identity section                                                                                                              |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/GameSection.tsx`            | 1-44                        | Reusable game path section                                                                                                             |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx`    | 1-53                        | Reusable runner method dropdown                                                                                                        |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`         | 1-290                       | Reusable runner-conditional runtime fields (includes Steam App ID for both Steam and Proton Run)                                       |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/TrainerSection.tsx`         | 1-220                       | Reusable trainer section (path, type, loading mode, network isolation, version)                                                        |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/MediaSection.tsx`           | 1-168                       | Reusable media section (Cover/Portrait/Background + Launcher Icon)                                                                     |
| P0 (critical)  | `src/crosshook-native/src/hooks/useProfile.ts`                                    | 388-398, 70-80              | `validateProfileForSave` (required-field gate) + preset hook surface                                                                   |
| P1 (important) | `src/crosshook-native/src/context/ProfileContext.tsx`                             | 1-40                        | `ProfileContext` exports the entire `UseProfileResult` (including bundled presets) — wizard already consumes via `useProfileContext()` |
| P1 (important) | `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`       | (whole file)                | Currently inlined in wizard Step 1; will move to Review step (collapsible)                                                             |
| P1 (important) | `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`                | 320-432                     | Existing preset picker logic (bundled + saved + handler) — pattern reference for slim wizard picker                                    |
| P1 (important) | `src/crosshook-native/src/components/ProfileFormSections.tsx`                     | 1-220, 509-572              | `FieldRow`, `OptionalSection`, `LauncherMetadataFields`, full canonical editor wiring                                                  |
| P1 (important) | `src/crosshook-native/src/styles/theme.css`                                       | 3793-3815                   | Existing `crosshook-onboarding-wizard*` styles; extend for the 5-step layout                                                           |
| P1 (important) | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                              | 5-10                        | `SCROLLABLE` selector — wizard `crosshook-modal__body` is already in the list, do not introduce new scroll containers                  |
| P2 (reference) | `src/crosshook-native/src/components/pages/ProfilesPage.tsx`                      | 120-121, 621-637, 1060-1067 | Wizard mount/edit-mode invocation surface — must keep stable                                                                           |
| P2 (reference) | `src/crosshook-native/src/App.tsx`                                                | 64-132                      | First-run wizard surface (no IPC contract changes here)                                                                                |
| P2 (reference) | `.claude/PRPs/plans/completed/ui-standardization-phase-1.plan.md`                 | (whole file)                | Phase 1 baseline that this phase builds on (banner contract, terminology, label parity)                                                |

## External Documentation

| Topic | Source | Key Takeaway                                                                                                                                                             |
| ----- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| N/A   | N/A    | No external research needed — feature uses established internal patterns and existing React/Tauri stack. All required components, hooks, and IPC commands already exist. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

// SOURCE: `src/crosshook-native/src/types/onboarding.ts:10`

```ts
export type OnboardingWizardStage = 'game_setup' | 'trainer_setup' | 'runtime_setup' | 'completed';
```

PascalCase for types/components, `snake_case` for stage tokens (matches Rust IPC convention), `crosshook-*` BEM-like CSS class names. New stages must follow `snake_case`.

### REUSED_SECTION_COMPOSITION

// SOURCE: `src/crosshook-native/src/components/ProfileSubTabs.tsx:144-214`

```tsx
<Tabs.Content value="setup" forceMount className="crosshook-subtab-content">
  <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
    <ProfileIdentitySection
      profileName={profileName}
      profile={profile}
      onProfileNameChange={onProfileNameChange}
      onUpdateProfile={onUpdateProfile}
      profileExists={profileExists}
      profiles={profiles}
    />
    <GameSection profile={profile} onUpdateProfile={onUpdateProfile} launchMethod={launchMethod} />
    <RunnerMethodSection profile={profile} onUpdateProfile={onUpdateProfile} />
  </div>
</Tabs.Content>
```

Each step body in the wizard follows this pattern: a single inner container that composes one or more `profile-sections/*` components passed `profile`, `onUpdateProfile`, and `launchMethod` from `useProfileContext()`. Do not re-implement field graphs inline.

### STAGE_SEQUENCE_WITH_SKIP

// SOURCE: `src/crosshook-native/src/hooks/useOnboarding.ts:7,106-131`

```ts
const STAGE_SEQUENCE: OnboardingWizardStage[] = ['game_setup', 'trainer_setup', 'runtime_setup', 'completed'];

const advanceOrSkip = useCallback((launchMethod: string) => {
  setStage((current) => {
    const currentIndex = STAGE_SEQUENCE.indexOf(current);
    let nextIndex = currentIndex + 1;
    if (
      nextIndex < STAGE_SEQUENCE.length &&
      STAGE_SEQUENCE[nextIndex] === 'trainer_setup' &&
      launchMethod === 'native'
    ) {
      nextIndex += 1;
    }
    return nextIndex < STAGE_SEQUENCE.length ? STAGE_SEQUENCE[nextIndex] : current;
  });
}, []);
```

Forward and backward navigation skip the trainer stage when `launchMethod === 'native'`. New stage sequence must preserve this guarantee.

### NO_WRITE_BEFORE_REVIEW

// SOURCE: `src/crosshook-native/src/hooks/useOnboarding.ts:87-95` and `src/crosshook-native/src/components/OnboardingWizard.tsx:265-280`

```ts
// BR-9 invariant: No profile is persisted until the user explicitly confirms in the review step.
// - dismiss() and skip paths only set onboarding_completed=true via dismiss_onboarding;
//   they do NOT write any profile data to TOML.

async function handleComplete() {
  const trimmedName = profileName.trim();
  if (trimmedName.length === 0) return;
  const result = await persistProfileDraft(trimmedName, profile);
  if (!result.ok) return;
  setCompletedProfileName(trimmedName);
  await dismiss();
  onComplete();
}
```

All step transitions, preset applications, and validation calls must remain in-memory (`updateProfile`/`setProfileName`) until the explicit Save Profile click on the review step.

### REQUIRED_FIELD_VALIDATION

// SOURCE: `src/crosshook-native/src/hooks/useProfile.ts:392-398`

```ts
function validateProfileForSave(profile: GameProfile): string | null {
  if (!profile.game.executable_path.trim()) {
    return 'Game executable path is required before saving a profile.';
  }
  return null;
}
```

The wizard's review step adds a UI-side checklist over the same set of fields plus the wizard-specific required set (profile name, game name, runner method, runner-method-conditional fields). It must NOT bypass `validateProfileForSave`; it complements it.

### FOCUS_TRAP_AND_PORTAL

// SOURCE: `src/crosshook-native/src/components/OnboardingWizard.tsx:155-258`

```tsx
useEffect(() => {
  // Portal host — created unconditionally on mount, NOT gated on `open`
  const host = document.createElement('div');
  host.className = 'crosshook-modal-portal';
  portalHostRef.current = host;
  document.body.appendChild(host);
  setIsMounted(true);
  return () => {
    host.remove(); /* ... */
  };
}, []);
```

Keep the existing portal/focus-trap/inert-siblings/overflow-lock contract exactly. The rework only changes step content, header eyebrow text, footer button enable/disable conditions, and the stage state machine.

### PRESET_PICKER_PATTERN

// SOURCE: `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx:382-431`

```tsx
const optimizationPresetGroups = useMemo((): SelectOptionGroup[] => {
  const groups: SelectOptionGroup[] = [];
  if (bundledOptimizationPresets.length > 0 && onApplyBundledPreset) {
    groups.push({
      label: 'Built-in',
      options: bundledOptimizationPresets.map((p) => ({
        value: bundledOptimizationTomlKey(p.preset_id),
        label: p.display_name,
        badge: 'Built-in',
      })),
    });
  }
  if (savedPresetOptionsWithOrphan.length > 0 && onSelectOptimizationPreset) {
    groups.push({ label: 'Saved', options: savedPresetOptionsWithOrphan });
  }
  return groups;
}, [...]);
```

The wizard preset picker is a slim variant of this: a single `ThemedSelect` grouped by `Built-in` and `Saved`, calling `applyBundledOptimizationPreset` or `switchLaunchOptimizationPreset` from `useProfileContext()`. No per-option toggles; this is "choose/apply preset", not the full optimizations panel.

### CSS_CLASS_HIERARCHY

// SOURCE: `src/crosshook-native/src/components/OnboardingWizard.tsx:296-303` and `src/crosshook-native/src/styles/theme.css:3793-3815`

```tsx
<div
  className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-onboarding-wizard"
  role="dialog"
  aria-modal="true"
>
```

Reuse `crosshook-onboarding-wizard*` namespace. Add new classes (`crosshook-onboarding-wizard__step-grid`, `crosshook-onboarding-wizard__review-summary`) only when an existing class does not fit.

### TEST_STRUCTURE

// SOURCE: `src/crosshook-native/Cargo.toml` workspace + `src/crosshook-native/package.json`

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cd src/crosshook-native && npm run build
```

There is no configured frontend unit-test framework in this repo. Verification relies on the Rust test suite for backend invariants and on `tsc` + Vite build + manual route checks for the wizard UI.

---

## Files to Change

| File                                                                 | Action           | Justification                                                                                                                                                                 |
| -------------------------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/types/onboarding.ts`                       | UPDATE           | Extend `OnboardingWizardStage` union with the new 5-step sequence                                                                                                             |
| `src/crosshook-native/src/hooks/useOnboarding.ts`                    | UPDATE           | Replace `STAGE_SEQUENCE`, status/hint/action label maps, and `advanceOrSkip`/`goBack` to handle the new steps and runner-method skip rules                                    |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`           | UPDATE           | Replace inline field graphs with reused `profile-sections/*` composition; add Media and Review steps; rewire footer; update `TOTAL_VISIBLE_STEPS` and visible-step calculator |
| `src/crosshook-native/src/components/wizard/WizardPresetPicker.tsx`  | CREATE           | Slim launch preset picker (bundled + saved) for the Review step, mirrors `LaunchOptimizationsPanel` preset logic but without option toggles                                   |
| `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx` | CREATE           | Required-field checklist + readiness recap rendered on the Review step                                                                                                        |
| `src/crosshook-native/src/components/wizard/wizardValidation.ts`     | CREATE           | Pure helpers that compute the wizard's required-field set per `launchMethod` (single source of truth, mirrors `validateProfileForSave`)                                       |
| `src/crosshook-native/src/styles/theme.css`                          | UPDATE           | Add `crosshook-onboarding-wizard__step-grid`, `__review-summary`, `__review-row`, `__required-badge` classes; keep existing wizard tokens                                     |
| `src/crosshook-native/src/components/pages/ProfilesPage.tsx`         | UPDATE (minimal) | Confirm `OnboardingWizard` mount and edit-mode call sites still resolve; no API change required (props unchanged)                                                             |
| `src/crosshook-native/src/App.tsx`                                   | UPDATE (minimal) | Confirm first-run wizard mount still resolves; no prop change                                                                                                                 |
| `.claude/PRPs/plans/ui-standardization-phase-2.plan.md`              | CREATE           | This plan                                                                                                                                                                     |

## NOT Building

- Any persistence schema change (TOML, SQLite, metadata DB) — wizard remains UI/runtime only.
- A new optimization-options panel inside the wizard — only the slim preset picker (choose/apply preset) is added.
- Removing or renaming `OnboardingWizard` props (`open`, `mode`, `onComplete`, `onDismiss`) — call sites in `App.tsx` and `ProfilesPage.tsx` stay stable.
- Changing the BR-9 no-write-before-review invariant or the `dismiss_onboarding` IPC behavior.
- Reworking the Install Game flow — that is Phase 3 (`#162`).
- Changing route banner contract introduced in Phase 1.
- Adding a new readiness/check IPC. The wizard reuses `runChecks()` (`check_readiness`) exactly as today.
- Splitting `OnboardingWizard.tsx` into multiple step files — keep one component file unless it exceeds the 800-line repo cap, in which case extract step bodies into `components/wizard/steps/*.tsx`.
- Sidebar / navigation IA changes.

---

## Step-by-Step Tasks

### Task 1: Extend the wizard stage state machine

- **ACTION**: Replace the 4-stage sequence with a 6-stage sequence (`identity_game`, `runtime`, `trainer`, `media`, `review`, `completed`).
- **IMPLEMENT**:
  - In `src/crosshook-native/src/types/onboarding.ts`, update the union to:

    ```ts
    export type OnboardingWizardStage = 'identity_game' | 'runtime' | 'trainer' | 'media' | 'review' | 'completed';
    ```

  - In `src/crosshook-native/src/hooks/useOnboarding.ts`:
    - Update `STAGE_SEQUENCE` to match.
    - Update `createInitialOnboardingState()` to start at `identity_game`.
    - Update `deriveStatusText`, `deriveHintText`, `deriveActionLabel` for the new stages. Action label is `'Next'` for steps 1–4, `'Save Profile'` for `review`, `'Done'` for `completed`.
    - Update `advanceOrSkip(launchMethod)` so `'trainer'` is skipped forward when `launchMethod === 'native'`.
    - Update `goBack(launchMethod)` so `'trainer'` is skipped backward when `launchMethod === 'native'`.
    - Add `isIdentityGame`, `isRuntime`, `isTrainer`, `isMedia`, `isReview` boolean flags to `UseOnboardingResult` and remove `isGameSetup`/`isTrainerSetup`/`isRuntimeSetup`. (Update consumers in same task.)

- **MIRROR**: `STAGE_SEQUENCE_WITH_SKIP`, `NAMING_CONVENTION`.
- **IMPORTS**: none new — only types/hook self-contained changes.
- **GOTCHA**: BR-9 invariant must remain intact: every stage transition is in-memory only; do not introduce any IPC inside `advanceOrSkip` or `goBack`. The only IPC remains `check_readiness` (manual button) and `dismiss_onboarding` (on dismiss/save).
- **VALIDATE**: `cd src/crosshook-native && npm run build` compiles. `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` still passes (no Rust touched, but smoke-check).

### Task 2: Create wizard validation helpers

- **ACTION**: Create `src/crosshook-native/src/components/wizard/wizardValidation.ts` with pure helpers describing the required-field set for the wizard.
- **IMPLEMENT**:

  ```ts
  import type { GameProfile, LaunchMethod } from '../../types';

  export interface WizardRequiredField {
    id: string;
    label: string;
    isSatisfied: boolean;
  }

  export interface WizardValidationResult {
    fields: WizardRequiredField[];
    isReady: boolean;
  }

  export function evaluateWizardRequiredFields(args: {
    profileName: string;
    profile: GameProfile;
    launchMethod: LaunchMethod;
  }): WizardValidationResult {
    /* ... */
  }
  ```

  Required-field rules:
  - Always: `profileName`, `profile.game.name`, `profile.game.executable_path`, `profile.launch.method`.
  - When `launchMethod === 'steam_applaunch'`: `profile.steam.app_id`, `profile.steam.compatdata_path`, `profile.steam.proton_path`.
  - When `launchMethod === 'proton_run'`: `profile.runtime.prefix_path`, `profile.runtime.proton_path` (Steam App ID is optional here, matches `RuntimeSection`).
  - When `launchMethod === 'native'`: no extra required fields.
    Returned `isReady` is `fields.every(f => f.isSatisfied)`.

- **MIRROR**: `REQUIRED_FIELD_VALIDATION`.
- **IMPORTS**: `GameProfile`, `LaunchMethod` from `../../types`.
- **GOTCHA**: Keep this function pure and synchronous; no IPC, no React state. The Save handler still calls `persistProfileDraft`, which goes through the existing `validateProfileForSave` gate — this helper is the _UI_ layer, not a replacement.
- **VALIDATE**: Helper unit-checked manually (no test harness configured); compile passes.

### Task 3: Create the slim preset picker for the Review step

- **ACTION**: Create `src/crosshook-native/src/components/wizard/WizardPresetPicker.tsx`.
- **IMPLEMENT**:
  - Functional component with no internal IPC. Props derived from `useProfileContext()` in the wizard parent and passed in:

    ```ts
    export interface WizardPresetPickerProps {
      bundledPresets: readonly BundledOptimizationPreset[];
      savedPresetNames: readonly string[];
      activePresetKey: string;
      busy: boolean;
      onApplyBundled: (presetId: string) => Promise<void>;
      onSelectSaved: (presetName: string) => Promise<void>;
    }
    ```

  - Render a single `ThemedSelect` with grouped options (`Built-in` and `Saved`). Use `bundledOptimizationTomlKey` keys identical to `LaunchOptimizationsPanel`.
  - On change, dispatch `applyBundledOptimizationPreset` for built-in or `switchLaunchOptimizationPreset` for saved.
  - Show a help line: "Optional. Apply a built-in or saved launch optimization preset before saving the profile. You can change this later from the Launch page."
  - Render disabled when `bundledPresets.length === 0 && savedPresetNames.length === 0`.

- **MIRROR**: `PRESET_PICKER_PATTERN`.
- **IMPORTS**: `ThemedSelect` from `../ui/ThemedSelect`, `BundledOptimizationPreset` from `../../types`, `bundledOptimizationTomlKey` from where `LaunchOptimizationsPanel` imports it (centralized helper — do not duplicate the constant; import the same one).
- **GOTCHA**: Preset application is one of the few operations that _does_ persist immediately (it writes to TOML through the existing IPC). This is acceptable on Step 5 because the user is on the Review step about to save, but still call `persistProfileDraft` after preset application to keep the rest of the in-memory profile in sync. Preset apply must be guarded behind `busy` to prevent reentrancy. Verify with the canonical pattern in `LaunchOptimizationsPanel.tsx:418-431`.
- **VALIDATE**: `npm run build` passes. Manual: bundled preset apply updates `active_preset` field on the profile in memory and persists.

### Task 4: Create the review summary component

- **ACTION**: Create `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`.
- **IMPLEMENT**:
  - Props:

    ```ts
    export interface WizardReviewSummaryProps {
      validation: WizardValidationResult;
      readinessResult: ReadinessCheckResult | null;
      checkError: string | null;
    }
    ```

  - Top section: "Required Fields" — render `validation.fields` as a list with check/✗ glyphs colored using existing `--crosshook-color-success` / `--crosshook-color-danger` tokens (mirror `resolveCheckColor` from `OnboardingWizard.tsx:62-71`).
  - Middle section: "System Checks" — if `readinessResult` is null, render a hint to click "Run Checks". If non-null, render the list using the same icon/color helpers already in `OnboardingWizard.tsx`.
  - Bottom section: "Tip" with a one-liner: "Save now or jump back to any step using Back."
  - All copy in plain text — no `console.*` calls.
  - Use new BEM class `crosshook-onboarding-wizard__review-summary` and `crosshook-onboarding-wizard__review-row`.

- **MIRROR**: `OnboardingWizard.tsx:51-71` (icon/color helpers — extract them to a shared module under `wizard/` so both files use the same source of truth).
- **IMPORTS**: `WizardValidationResult` from `./wizardValidation`, `ReadinessCheckResult` from `../../types/onboarding`.
- **GOTCHA**: Move `resolveCheckIcon` / `resolveCheckColor` to `src/crosshook-native/src/components/wizard/checkBadges.ts` (new file) and re-import in both `OnboardingWizard.tsx` and `WizardReviewSummary.tsx` so they cannot drift.
- **VALIDATE**: `npm run build` passes. Manual: required field flips state when input changes; readiness checks render colored bullets.

### Task 5: Refactor `OnboardingWizard.tsx` to compose canonical sections

- **ACTION**: Replace the inline Step 1/2/3 JSX with a 5-step composition that consumes `profile-sections/*`.
- **IMPLEMENT**:
  - Update `TOTAL_VISIBLE_STEPS` to `5`.
  - Replace `getVisibleStepNumber(isGameSetup, isTrainerSetup)` with `getVisibleStepNumber(stage, launchMethod)` that maps:
    - `identity_game` → 1
    - `runtime` → 2
    - `trainer` → 3 (only when `launchMethod !== 'native'`; when native, `media` becomes the visible 3 and `review` becomes 4, with header reading `Step N of 4`)
    - `media` → 3 (native) or 4 (non-native)
    - `review` → 4 (native) or 5 (non-native)
  - Replace `TOTAL_VISIBLE_STEPS` with a derived `totalVisibleSteps = launchMethod === 'native' ? 4 : 5`.
  - Replace inline Game/Trainer/Runtime sections with:

    ```tsx
    {
      isIdentityGame && (
        <section aria-label="Identity & game">
          <ProfileIdentitySection
            profileName={profileName}
            profile={profile}
            onProfileNameChange={setProfileName}
            onUpdateProfile={updateProfile}
          />
          <GameSection profile={profile} onUpdateProfile={updateProfile} launchMethod={launchMethod} />
          <RunnerMethodSection profile={profile} onUpdateProfile={updateProfile} />
        </section>
      );
    }

    {
      isRuntime && (
        <section aria-label="Runtime">
          <RuntimeSection
            profile={profile}
            onUpdateProfile={updateProfile}
            launchMethod={launchMethod}
            protonInstalls={protonInstalls}
            protonInstallsError={protonInstallsError}
          />
        </section>
      );
    }

    {
      isTrainer && (
        <section aria-label="Trainer">
          <TrainerSection
            profile={profile}
            onUpdateProfile={updateProfile}
            launchMethod={launchMethod}
            profileName={profileName}
            profileExists={mode === 'edit'}
          />
        </section>
      );
    }

    {
      isMedia && (
        <section aria-label="Media">
          <MediaSection profile={profile} onUpdateProfile={updateProfile} launchMethod={launchMethod} />
        </section>
      );
    }

    {
      isReview && (
        <section aria-label="Review and save">
          <WizardPresetPicker
            bundledPresets={bundledOptimizationPresets}
            savedPresetNames={Object.keys(profile.launch.presets ?? {})}
            activePresetKey={profile.launch.active_preset ?? ''}
            busy={optimizationPresetActionBusy}
            onApplyBundled={applyBundledOptimizationPreset}
            onSelectSaved={switchLaunchOptimizationPreset}
          />
          <CustomEnvironmentVariablesSection
            profileName={profileName}
            customEnvVars={profile.launch.custom_env_vars}
            onUpdateProfile={updateProfile}
            idPrefix="onboarding-wizard"
          />
          <WizardReviewSummary
            validation={evaluateWizardRequiredFields({ profileName, profile, launchMethod })}
            readinessResult={readinessResult}
            checkError={checkError}
          />
        </section>
      );
    }
    ```

  - Footer: replace `Save Profile` button enabled condition with `validation.isReady && !saving`.
  - Header eyebrow: `Step ${visibleStep} of ${totalVisibleSteps}`. Title resolves from a stage-keyed map (`Identity & Game`, `Runtime`, `Trainer`, `Media`, `Review & Save`, `Setup Complete`).
  - Pull `bundledOptimizationPresets`, `applyBundledOptimizationPreset`, `switchLaunchOptimizationPreset`, `optimizationPresetActionBusy` out of `useProfileContext()` (already exported via `UseProfileResult`).
  - Remove `LauncherMetadataFields`, `AutoPopulate`, `ProtonPathField`, and `CustomEnvironmentVariablesSection` from Step 1 — they are now owned by the canonical `RuntimeSection` (auto-populate / proton path) or moved to Review (custom env vars).

- **MIRROR**: `REUSED_SECTION_COMPOSITION`, `NO_WRITE_BEFORE_REVIEW`, `FOCUS_TRAP_AND_PORTAL`, `CSS_CLASS_HIERARCHY`.
- **IMPORTS**:

  ```ts
  import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
  import { GameSection } from './profile-sections/GameSection';
  import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
  import { RuntimeSection } from './profile-sections/RuntimeSection';
  import { TrainerSection } from './profile-sections/TrainerSection';
  import { MediaSection } from './profile-sections/MediaSection';
  import { WizardPresetPicker } from './wizard/WizardPresetPicker';
  import { WizardReviewSummary } from './wizard/WizardReviewSummary';
  import { evaluateWizardRequiredFields } from './wizard/wizardValidation';
  import { resolveCheckIcon, resolveCheckColor } from './wizard/checkBadges';
  ```

  Drop these imports (no longer used inside the wizard): `LauncherMetadataFields`, `InstallField`, `ThemedSelect`, `ProtonPathField`, `AutoPopulate`. (`CustomEnvironmentVariablesSection` stays — moved to Review.)

- **GOTCHA**:
  - Keep the existing portal/focus-trap/inert/dismiss/handleSkip/handleComplete blocks intact. Only the body and the visible-step header change.
  - The `mode === 'create'` branch must keep `void selectProfile('')` so the wizard opens with a clean draft.
  - Do NOT introduce a new `overflow-y` container inside any step body — `crosshook-modal__body` is already in the `SCROLLABLE` selector and owns scrolling. Reused sections must render directly inside it.
  - When `launchMethod` flips at runtime (e.g. user changes runner method on Step 1, then advances), the `goBack`/`advanceOrSkip` callers must re-pass the _current_ `launchMethod` so the trainer skip stays correct. Pass `launchMethod` from `resolveLaunchMethod(profile)` at every navigation call.
  - The wizard already injects scroll into `.crosshook-modal__body`. Reused sections like `MediaSection` and `RuntimeSection` use grids but no nested scrollers — they are safe to drop in.
  - File budget: the rework should reduce, not grow, `OnboardingWizard.tsx`. Target ≤ 600 lines after refactor (currently 764). If still > 600 lines, extract per-stage step bodies into `src/crosshook-native/src/components/wizard/steps/*.tsx`.
- **VALIDATE**: `npm run build` succeeds. Manual: navigate every step in create mode for each launch method and confirm fields render via the canonical components, no field appears twice, and the trainer step is skipped correctly when native.

### Task 6: Wire validation gate to footer Save button + readiness recap

- **ACTION**: Compute `validation` once per render in the wizard body and gate the Save button on `validation.isReady`.
- **IMPLEMENT**:
  - Compute `const validation = useMemo(() => evaluateWizardRequiredFields({ profileName, profile, launchMethod }), [profileName, profile, launchMethod]);`
  - Pass `validation` into `WizardReviewSummary` and use it for the Save button:

    ```tsx
    {
      isReview && (
        <button
          type="button"
          className="crosshook-button"
          disabled={saving || !validation.isReady}
          onClick={() => void handleComplete()}
        >
          {saving ? 'Saving...' : 'Save Profile'}
        </button>
      );
    }
    ```

  - Add a small inline tooltip / `aria-describedby` referencing the first missing field id when the button is disabled.

- **MIRROR**: `REQUIRED_FIELD_VALIDATION`, existing button render in `OnboardingWizard.tsx:723-736`.
- **IMPORTS**: `useMemo` (already imported via React).
- **GOTCHA**: Do not remove the existing fallback to `validateProfileForSave` inside `persistProfileDraft` — both gates must remain. Wizard surface checks `isReady` to _prevent_ the click; the hook still validates server-side at save time.
- **VALIDATE**: Manual: leave a required field blank on Review and verify Save button is disabled with descriptive `aria-describedby`. Fill the field and verify Save enables.

### Task 7: Style the new wizard step layout and review summary

- **ACTION**: Add scoped CSS for the new wizard surfaces in `src/crosshook-native/src/styles/theme.css`.
- **IMPLEMENT**:
  - Add classes (use existing tokens, no hardcoded colors):
    - `.crosshook-onboarding-wizard__step-grid` — outer grid for step bodies (gap variable from existing `--crosshook-spacing-*`).
    - `.crosshook-onboarding-wizard__review-summary` — card-style container with the same radius/border treatment as `.crosshook-auto-populate` (see `theme.css:3829-3837`).
    - `.crosshook-onboarding-wizard__review-row` — flex row for label + status icon.
    - `.crosshook-onboarding-wizard__required-badge--missing` — danger color tint.
    - `.crosshook-onboarding-wizard__required-badge--ok` — success color tint.
  - Update `.crosshook-onboarding-wizard__nav` only if the 5-step layout requires more horizontal room. Otherwise leave existing styles untouched.
- **MIRROR**: existing `.crosshook-auto-populate` block in `theme.css:3829-3850` for card border/background tokens.
- **IMPORTS**: none (CSS only).
- **GOTCHA**: Do not introduce new media queries that conflict with the existing Steam Deck `max-height: 820px` rules. Verify the wizard still fits inside the Steam Deck viewport with the new step density.
- **VALIDATE**: Visual smoke test on a standard viewport and `max-height: 820px`.

### Task 8: Verify mount sites and edit-mode call sites

- **ACTION**: Confirm `App.tsx` and `ProfilesPage.tsx` still render the wizard correctly.
- **IMPLEMENT**:
  - `App.tsx:128-133` — no change needed; props (`open`, `onComplete`, `onDismiss`) unchanged.
  - `ProfilesPage.tsx:1060-1067` — no change needed; props unchanged. Confirm `wizardMode === 'edit'` initializes from the currently selected profile (existing behavior — verify the new identity-game step shows the existing values).
  - Touch each file only if a TypeScript error surfaces from removed exports/types.
- **MIRROR**: P2 references above.
- **IMPORTS**: none new.
- **GOTCHA**: When invoking the wizard from `Edit in Wizard`, the existing `selectProfile('')` reset only fires for `mode === 'create'` (`OnboardingWizard.tsx:117-121`). Edit mode preserves the loaded profile — keep that branch as is so users do not lose their current profile when entering the wizard.
- **VALIDATE**: Manual: open the wizard from `Profiles → New Profile` (create) and from `Profiles → Edit in Wizard` (edit) and confirm both mount cleanly.

### Task 9: Final terminology, accessibility, and Phase-1 parity audit

- **ACTION**: Audit all wizard copy and aria semantics to match Phase 1 banner standardization terminology.
- **IMPLEMENT**:
  - Step titles: `Identity & Game`, `Runtime`, `Trainer`, `Media`, `Review & Save`, `Setup Complete`.
  - Make sure step `aria-label` values match the human title.
  - Confirm focus order: step heading → step controls → footer Back → Run Checks → Next/Save.
  - Confirm `aria-modal="true"` and `aria-labelledby` still bind to the heading id.
  - Confirm `controllerPrompts` `confirmLabel` is updated for the new stages: `'Next'` for steps 1–4, `'Save Profile'` for review, `'Done'` for completed.
- **MIRROR**: Phase 1 banner terminology in `routeMetadata.ts` and Sidebar labels.
- **IMPORTS**: none new.
- **GOTCHA**: Do not regress the `Skip Setup` affordance — it remains visible for all in-progress stages and uses `dismiss()` (no profile written).
- **VALIDATE**: Manual: keyboard tab through every stage; confirm focus trap and skip/save semantics. Verify each title matches Phase 1 sidebar/banner casing.

---

## Testing Strategy

### Unit Tests

| Test                                                                          | Input                                             | Expected Output                                      | Edge Case? |
| ----------------------------------------------------------------------------- | ------------------------------------------------- | ---------------------------------------------------- | ---------- |
| `evaluateWizardRequiredFields` returns ready when all required filled (steam) | `profileName='X'`, full Steam profile             | `isReady=true`, all `fields[i].isSatisfied=true`     | No         |
| `evaluateWizardRequiredFields` flags missing executable path                  | empty `executable_path`                           | `isReady=false`, `executable_path` field unsatisfied | No         |
| `evaluateWizardRequiredFields` does not require Steam App ID for `proton_run` | `proton_run` profile without `steam.app_id`       | `isReady=true`                                       | Yes        |
| Stage transition (`advanceOrSkip`) skips trainer when native                  | current=`runtime`, launchMethod=`native`          | next=`media`                                         | Yes        |
| Stage transition (`advanceOrSkip`) keeps trainer when steam                   | current=`runtime`, launchMethod=`steam_applaunch` | next=`trainer`                                       | No         |
| `goBack` from `media` skips trainer when native                               | current=`media`, launchMethod=`native`            | prev=`runtime`                                       | Yes        |
| `WizardPresetPicker` empty when no presets                                    | `bundledPresets=[]`, `savedPresetNames=[]`        | renders disabled select with help text               | Yes        |

> Note: no frontend test framework is configured. These cases are documented as manual verification steps; if pytest/vitest is later added, they map directly to test cases.

### Edge Cases Checklist

- [ ] Empty `profileName` keeps Save disabled and shows the missing-field row in summary.
- [ ] Switching launch method on Step 1 then advancing triggers correct trainer skip.
- [ ] Going Back from Media on a native profile lands on Runtime (not Trainer).
- [ ] Editing an existing profile loads current values into the new step layout (no draft reset).
- [ ] Custom env vars added on Review step persist into the saved profile.
- [ ] Bundled preset apply on Review step persists via the existing IPC.
- [ ] Saved preset switch on Review step persists via the existing IPC.
- [ ] Steam Deck `max-height: 820px` viewport: no clipped content, all rows reachable by keyboard scroll.
- [ ] Screen reader announces stage transitions (heading focus on stage change).
- [ ] Closing the wizard via `Skip Setup` does not write a profile (BR-9 invariant).

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run build
```

EXPECT: TypeScript + Vite build succeeds with zero errors.

### Unit Tests

```bash
# No frontend unit test framework configured in this repo
echo "N/A"
```

EXPECT: N/A (manual + compile verification only).

### Full Test Suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: No regressions from UI-only changes (sanity guard for workspace health).

### Database Validation (if applicable)

```bash
echo "N/A - no persistence/schema changes"
```

EXPECT: N/A.

### Browser Validation (if applicable)

```bash
./scripts/dev-native.sh
```

EXPECT: Wizard opens (first-run + Profiles route), all 5 steps render with reused sections, validation gates Save, native flow correctly skips Trainer.

### Manual Validation

- [ ] Open wizard from `Profiles → New Profile` and walk all 5 steps with `steam_applaunch`.
- [ ] Open wizard from `Profiles → New Profile` and walk all 4 steps with `native` (Trainer skipped).
- [ ] Open wizard from `Profiles → New Profile` and walk all 5 steps with `proton_run`, confirm Steam App ID surfaces in Runtime step (optional).
- [ ] Open wizard from `Profiles → Edit in Wizard` against an existing profile and confirm fields populate.
- [ ] Set/clear required fields and confirm Save button enable/disable + summary feedback.
- [ ] Apply a bundled preset on Review step and confirm `active_preset` updates.
- [ ] Apply a saved preset on Review step and confirm `active_preset` switches.
- [ ] Click `Skip Setup` mid-flow and confirm no new profile appears in the Profiles list.
- [ ] Switch launch method on Step 1, advance, and confirm trainer skip + back skip work.
- [ ] Confirm wizard scroll behavior is unchanged (single scroll inside `crosshook-modal__body`).
- [ ] Confirm focus trap, Escape dismiss, and inert siblings still work.
- [ ] Confirm `gh issue view 161` acceptance criteria are met.

---

## Acceptance Criteria

- [ ] Wizard renders 5 visible steps (4 when `launchMethod === 'native'`) with balanced density.
- [ ] Every step body is composed from canonical `profile-sections/*` components — no inline duplicate field graphs.
- [ ] Required fields are explicit per launch method and gate Save via `evaluateWizardRequiredFields`.
- [ ] Steam App ID is surfaced for `proton_run` (optional) and `steam_applaunch` (required) inside the canonical Runtime step.
- [ ] Media step exposes Cover, Portrait, Background art, plus Launcher Icon when `launchMethod !== 'native'`.
- [ ] Review step exposes a launch preset picker (bundled + saved) and the optional CustomEnvironmentVariables section.
- [ ] Review step shows a required-field summary and the latest readiness check result.
- [ ] BR-9 invariant preserved: dismiss/skip never writes a profile; only Save persists.
- [ ] `Skip Setup`, `Run Checks`, `Back`, `Next`, `Save Profile`, and `Done` actions all reachable by keyboard with the existing focus trap.
- [ ] No regression in route-level scroll behavior or wizard portal/focus contract from Phase 1.

## Completion Checklist

- [ ] Code follows discovered `profile-sections/*` composition pattern.
- [ ] Error handling stays banner-based and non-blocking (`profileError` rendered as `crosshook-danger`).
- [ ] Logging follows existing `console.error` only-on-recoverable-fault convention; no new `console.log` statements.
- [ ] Tests N/A — validation is `tsc` + Vite build + manual checks.
- [ ] No hardcoded colors/sizes in new CSS (uses existing `--crosshook-*` tokens).
- [ ] Documentation updated in commit message; `docs(internal):` prefix not required because user-visible (`feat(ui):` is correct).
- [ ] No unnecessary scope additions into Phase 3 (Install Game flow) or persistence layer.
- [ ] Self-contained — no questions needed during implementation.

## Risks

| Risk                                                                                                         | Likelihood | Impact | Mitigation                                                                                                                                                 |
| ------------------------------------------------------------------------------------------------------------ | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reused sections behave differently in modal vs. full editor (e.g. width-sensitive grids)                     | Medium     | Medium | Smoke-test each step at the wizard's modal width; reuse `crosshook-install-grid` and existing classes as `profile-sections/*` already do.                  |
| New required-field rules diverge from `validateProfileForSave` and confuse users                             | Medium     | Medium | Keep `evaluateWizardRequiredFields` as a strict superset of `validateProfileForSave`. Document the union in a code comment.                                |
| Preset application IPC inside the wizard breaks BR-9 expectations for users who skip after applying a preset | Low        | Medium | Document explicitly in Step 3 GOTCHA. Preset apply on Review step is intentional; the user is on the final step, and Skip Setup remains a separate action. |
| Trainer-step skip math breaks when `launchMethod` flips between steps                                        | Medium     | High   | Always pass the live `launchMethod` to `advanceOrSkip` / `goBack`; rely on the same skip rule on both navigations.                                         |
| Wizard exceeds 800-line file cap after refactor                                                              | Low        | Medium | Target ≤ 600 lines after extracting Review-step helpers; if exceeded, extract per-stage step bodies into `components/wizard/steps/*.tsx`.                  |
| First-run wizard from `App.tsx` regresses because of skipped/missing stage flag                              | Medium     | High   | Update `useOnboarding` consumers (booleans, `controllerPrompts` labels) in the same task as the type change. Compile catches drift.                        |
| Steam Deck (`max-height: 820px`) viewport clips the new Review summary                                       | Medium     | Medium | Verify with media query inspection during manual validation; ensure the modal body remains the only scroll owner.                                          |

## Notes

- Storage boundary classification: **runtime/UI-only** (no TOML, no SQLite, no migration). Launch preset application reuses existing IPC and is unchanged.
- Persistence/usability impact: no migration/back-compat concerns; app remains fully offline-capable; preset selection is optional.
- This phase deliberately leaves Phase 3 (`#162` Install Game flow parity) untouched. The shared `profile-sections/*` reuse pattern established here is the same one Phase 3 will adopt.
- `OnboardingWizard.tsx` already mounts portal/focus contract that matches `ProfileReviewModal.tsx`. Keep that contract unchanged when refactoring step bodies.
- Conventional commit prefix for the implementation work: `feat(ui): rebalance profile wizard with full field parity (#161)`.
