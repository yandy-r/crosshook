# Plan: UI Standardization Phase 3 — Install Game Flow Parity & Install-Specific UX

## Summary

Refactor the **Install Game** flow so its field taxonomy matches the canonical
profile wizard/editor (identity, runtime, trainer, media, review) while
preserving install-specific controls (installer media, executable candidate
discovery, helper log visibility, retry/reset behaviors). This phase reuses
the same `profile-sections/*` components, `WizardPresetPicker`,
`WizardReviewSummary`, and `wizardValidation` helpers introduced in Phase 2 so
the install panel and the wizard share a single source of truth for every
profile field. Storage boundary is **runtime/UI plus an additive,
backwards-compatible extension of `InstallGameRequest`** — no TOML/SQLite
schema migration is required.

## User Story

As a CrossHook user installing a Windows game through CrossHook, I want the
Install Game flow to expose every field I would set in the New Profile wizard
(runner method, Steam App ID, full art slots, launch preset) and to organize
those fields into the same `Identity → Runtime → Trainer → Media → Installer
& Review` rhythm I just learned in the wizard, so I can save a complete,
high-quality profile from a single guided session without bouncing back to
the editor afterward.

## Problem → Solution

The Install Game panel currently uses inline `InstallField` controls
duplicating the editor's field graph, exposes only a subset of profile-facing
inputs (no Steam App ID, no portrait/background art, no preset selection,
runner method hardcoded to `proton_run`), groups installer media as the
"Media" tab while game art has no home, and pads the Review tab with status
chrome around two profile-relevant fields → restructure the panel to compose
the canonical `profile-sections/*` components against an in-memory
`GameProfile` draft that lives next to a small set of install-only fields,
extend `InstallGameRequest` so the backend can populate the new profile
fields on `reviewable_profile()`, add a fifth tab for **Game Art**, fold the
launch preset picker and required-field summary into Review, and align tab
labels with Phase 1 banner terminology and Phase 2 wizard copy.

## Metadata

- **Complexity**: Large
- **Source PRD**: N/A (GitHub issue driven)
- **PRD Phase**: `#163` Phase 3 (`#162`)
- **Estimated Files**: 9–11
- **Issue**: [#163](https://github.com/yandy-r/crosshook/issues/163), [#162](https://github.com/yandy-r/crosshook/issues/162)

---

## UX Design

### Before

```text
┌───── Install Game (4 free-click tabs) ──────────────────────────────────┐
│                                                                          │
│  Tab 1: Profile identity                                                 │
│   • Profile Name • Display Name                                          │
│   • Custom Cover Art • Launcher Icon                                     │
│                                                                          │
│  Tab 2: Install media (only 2 fields)                                    │
│   • Installer EXE • Trainer EXE                                          │
│                                                                          │
│  Tab 3: Runtime (only 2 fields, runner hardcoded to proton_run)          │
│   • Proton Path • Prefix Path                                            │
│   (no Steam App ID, no working dir override, no runner method picker)    │
│                                                                          │
│  Tab 4: Review (status-heavy, light on profile data)                     │
│   • Stage label • Hint • Pills • Final Executable + candidates           │
│   • Generated profile preview (2 read-only rows)                         │
│   • Installer log path                                                   │
│                                                                          │
│  Footer: [Install Game] [Reset Form] [Review in Modal]                   │
│                                                                          │
│  ✗ No portrait/background art slots                                      │
│  ✗ No Steam App ID anywhere                                              │
│  ✗ No launch preset picker                                               │
│  ✗ No required-field readiness summary                                   │
│  ✗ Field implementation duplicates wizard/editor graph                   │
└──────────────────────────────────────────────────────────────────────────┘
```

### After

```text
┌───── Install Game (5 wizard-aligned tabs, install controls preserved) ──┐
│                                                                          │
│  Tab 1: Identity & Game (mirrors wizard step 1)                          │
│   • ProfileIdentitySection (Profile Name + Game Name)                    │
│   • Display Name (install-only — survives review)                        │
│   • RunnerMethodSection (proton_run | steam_applaunch | native)          │
│                                                                          │
│  Tab 2: Runtime (mirrors wizard step 2 + RuntimeSection)                 │
│   • RuntimeSection (runner-conditional fields incl. Steam App ID)        │
│   • Default-prefix resolver hint stays visible                           │
│                                                                          │
│  Tab 3: Trainer (skipped tab when launchMethod === native)               │
│   • TrainerSection (path, type, loading mode, network isolation, ver)    │
│                                                                          │
│  Tab 4: Media (canonical MediaSection — Cover/Portrait/Background)       │
│   • + Launcher Icon when launchMethod !== native                         │
│                                                                          │
│  Tab 5: Installer & Review (install-specific controls + readiness)      │
│   • Installer EXE (the install media) — install-only                     │
│   • Stage / candidates / final executable / helper log                   │
│   • WizardPresetPicker (Built-in + Saved)                                │
│   • CustomEnvironmentVariablesSection (collapsible, optional)            │
│   • WizardReviewSummary (required-field checklist + readiness recap)     │
│                                                                          │
│  Footer: [Install Game] [Reset] [Open Review Modal]                      │
│                                                                          │
│  ✓ Full art parity (Cover + Portrait + Background + Launcher Icon)       │
│  ✓ Steam App ID surfaced for steam_applaunch and proton_run              │
│  ✓ Launch preset picker (bundled + saved)                                │
│  ✓ Required-field summary blocks Install when incomplete                 │
│  ✓ Every field uses canonical profile-sections/*                         │
└──────────────────────────────────────────────────────────────────────────┘
```

### Interaction Changes

| Touchpoint                 | Before                                                      | After                                                                                  | Notes                                                                                                  |
| -------------------------- | ----------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Tab count                  | 4 (`identity`, `media`, `runtime`, `review`)                | 5 (`identity`, `runtime`, `trainer`, `media`, `installer_review`)                      | `trainer` tab is hidden when `launchMethod === 'native'` (mirrors wizard skip rule)                    |
| Tab label terminology      | `Profile identity` / `Install media` / `Runtime` / `Review` | `Identity & Game` / `Runtime` / `Trainer` / `Media` / `Installer & Review`             | Matches Phase 2 wizard copy and Phase 1 banner casing                                                  |
| Field implementation       | Inline `InstallField` calls                                 | Canonical `profile-sections/*` components                                              | Eliminates wizard/install drift; install-only inputs use `InstallField` only where no canonical exists |
| Runner method              | Hardcoded `proton_run`                                      | User-selectable via `RunnerMethodSection`                                              | `InstallGameRequest` extended with `runner_method`; backend uses it to set `launch.method`             |
| Steam App ID               | Missing                                                     | Surfaced via canonical `RuntimeSection` for steam_applaunch (req) and proton_run (opt) | New optional field on `InstallGameRequest`; backend sets it on the reviewable profile                  |
| Game art slots             | Cover only (in Identity tab)                                | Cover + Portrait + Background via `MediaSection`                                       | New optional fields on `InstallGameRequest`; backend writes them onto `reviewable_profile()`           |
| Launcher icon              | Identity tab                                                | Media tab (inside `MediaSection`)                                                      | Already on the request; relocated to align with wizard                                                 |
| Installer EXE              | Media tab                                                   | Installer & Review tab                                                                 | Install-only — installer media stays a CrossHook-specific input                                        |
| Trainer EXE                | Media tab                                                   | Trainer tab (inside `TrainerSection`)                                                  | Native skips trainer entirely                                                                          |
| Working directory override | Not exposed                                                 | Optional inside `RuntimeSection` (proton_run + native)                                 | Already supported by `RuntimeSection`; backend reads it from the request                               |
| Launch preset picker       | Not selectable                                              | `WizardPresetPicker` on Installer & Review tab                                         | Reuses Phase 2 component; disabled in install flow because no profile is persisted yet                 |
| Required-field readiness   | Implicit (Install button only checks installer_path)        | `WizardReviewSummary` lists every required field with status                           | Reuses Phase 2 component                                                                               |
| Save handoff               | Auto-opens `ProfileReviewModal` after install               | Same — but the modal now receives a draft populated with art/preset/AppID              | `InstallProfileReviewPayload.generatedProfile` carries the merged backend profile + local edits        |
| BR-9 invariant             | No profile persisted before explicit Save in modal          | Unchanged                                                                              | All edits remain in-memory; only `persistProfileDraft` writes TOML                                     |
| Scroll behavior            | Single owner: `crosshook-install-shell__content`            | Unchanged                                                                              | No new `overflow-y` containers; reused sections render directly                                        |

---

## Mandatory Reading

| Priority       | File                                                                              | Lines                         | Why                                                                                                                 |
| -------------- | --------------------------------------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/src/components/InstallGamePanel.tsx`                        | 1-572                         | Component to rework end-to-end                                                                                      |
| P0 (critical)  | `src/crosshook-native/src/hooks/useInstallGame.ts`                                | 1-579                         | State machine and request shape that must extend                                                                    |
| P0 (critical)  | `src/crosshook-native/src/types/install.ts`                                       | 1-119                         | TS types and validation message tables — must match Rust changes                                                    |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/install/models.rs`                | 1-279                         | `InstallGameRequest` + `reviewable_profile()` — backend source of truth                                             |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/install/service.rs`               | 26-101, 271-287               | `validate_install_request` and `build_reviewable_profile` integration points                                        |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/ProfileIdentitySection.tsx` | 1-86                          | Reusable identity section (already used by wizard)                                                                  |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx`    | 1-53                          | Reusable runner method dropdown                                                                                     |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`         | 1-289                         | Reusable runner-conditional runtime fields (Steam App ID, prefix, proton, working dir, AutoPopulate)                |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/TrainerSection.tsx`         | 1-100                         | Reusable trainer section (path, type, loading mode, network isolation, version)                                     |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/MediaSection.tsx`           | 1-168                         | Reusable media section (Cover/Portrait/Background + Launcher Icon)                                                  |
| P0 (critical)  | `src/crosshook-native/src/components/wizard/WizardPresetPicker.tsx`               | 1-122                         | Reusable preset picker — accepts arbitrary callbacks; works for both wizard and install                             |
| P0 (critical)  | `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`              | 1-106                         | Reusable required-field + readiness summary                                                                         |
| P0 (critical)  | `src/crosshook-native/src/components/wizard/wizardValidation.ts`                  | 1-117                         | `evaluateWizardRequiredFields` — strict superset of `validateProfileForSave`. Reuse directly with the install draft |
| P1 (important) | `src/crosshook-native/src/components/pages/InstallPage.tsx`                       | 1-449                         | `handleOpenProfileReview` / `handleInstallActionConfirmation` consumers — props/types stay stable                   |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/install.rs`                          | 1-38                          | Tauri command handlers — must accept the new request fields automatically (Serde) but smoke-check                   |
| P1 (important) | `src/crosshook-native/src/types/profile.ts`                                       | 18, 132-218                   | `LaunchMethod` union, `LaunchSection` shape, `active_preset` field (default profile factory)                        |
| P1 (important) | `src/crosshook-native/src/utils/launch.ts`                                        | 1-80                          | `resolveLaunchMethod(profile)` helper used to derive the runtime branch                                             |
| P1 (important) | `src/crosshook-native/src/components/wizard/checkBadges.ts`                       | 1-33                          | Shared `resolveCheckIcon` / `resolveCheckColor` helpers for the readiness recap                                     |
| P1 (important) | `src/crosshook-native/src/styles/theme.css`                                       | 550-640, 1960-2050, 2820-3000 | `crosshook-install-*` classes — extend grid for the 5-tab layout, no new scroll containers                          |
| P1 (important) | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                              | 5-10                          | `SCROLLABLE` selector — verify the install shell is already covered; do **not** add new scroll containers           |
| P2 (reference) | `src/crosshook-native/src/components/OnboardingWizard.tsx`                        | 296-560                       | Phase 2 composition pattern — copy the section composition layout exactly                                           |
| P2 (reference) | `src/crosshook-native/src/components/ProfileSubTabs.tsx`                          | 144-280                       | Canonical wiring of `profile-sections/*` in the editor                                                              |
| P2 (reference) | `.claude/PRPs/plans/completed/ui-standardization-phase-2.plan.md`                 | (whole)                       | Phase 2 baseline — same `profile-sections/*` reuse pattern this phase adopts                                        |

## External Documentation

| Topic | Source | Key Takeaway                                                                                                                                                                             |
| ----- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| N/A   | N/A    | No external research needed — feature uses established internal patterns and existing React/Tauri/Serde stack. Every required component, hook, IPC, and type already exists in the repo. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

// SOURCE: `src/crosshook-native/src/types/install.ts:5-16`

```ts
export interface InstallGameRequest {
  profile_name: string;
  display_name: string;
  installer_path: string;
  trainer_path: string;
  proton_path: string;
  prefix_path: string;
  installed_game_executable_path: string;
  launcher_icon_path: string;
  custom_cover_art_path: string;
}
```

`snake_case` for all IPC field names (matches Rust Serde defaults), PascalCase
for types/components, `crosshook-*` BEM-like CSS class names. New optional
fields added to this struct must follow `snake_case`. New TS components must
be PascalCase.

### REUSED_SECTION_COMPOSITION

// SOURCE: `src/crosshook-native/src/components/OnboardingWizard.tsx` (Phase 2 pattern) and `src/crosshook-native/src/components/ProfileSubTabs.tsx:144-214`

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

Each tab body in the install panel follows this composition: a single
`crosshook-subtab-content__inner` container holds one or more
`profile-sections/*` components passed `profile`, `onUpdateProfile`, and
`launchMethod` from the install hook's draft state. Do not re-implement field
graphs inline.

### REQUEST_VS_DRAFT_SPLIT

// SOURCE: This plan introduces the pattern; precedent is `useInstallGame.setResult` in `src/crosshook-native/src/hooks/useInstallGame.ts:326-344`

```ts
// `useInstallGame` keeps two synchronized stores:
//   1. `installerInputs` — install-only fields (installer_path)
//   2. `draftProfile` — full GameProfile mirror that the canonical sections edit
//
// `request` is *derived* on submit by `buildInstallGameRequest({ installerInputs, draftProfile, profileName })`.
// After install, `setResult(installResult)` merges `installResult.profile` into
// `draftProfile`, preserving local edits to fields the backend round-trips.
```

The current hook already mutates `request.prefix_path` and
`installed_game_executable_path` from the install result, so the precedent
for two-way sync exists. This phase formalizes it: the canonical
`profile-sections/*` operate on `draftProfile`; install-only fields stay on
`installerInputs`; `request` becomes a derived value built only at submit
time.

### NO_WRITE_BEFORE_REVIEW (BR-9)

// SOURCE: `src/crosshook-native/src/components/pages/InstallPage.tsx:253-289`

```ts
async function handleSaveProfileReview() {
  // ... validation ...
  const persistResult = await persistProfileDraft(profileName, draftProfile);
  if (!persistResult.ok) {
    /* surface error */ return;
  }
  setProfileReviewSession(null);
  onNavigate?.('profiles');
}
```

The Install Game flow only persists a profile when the user clicks **Save and
Open Profiles** inside `ProfileReviewModal`. All edits to identity/runtime/
trainer/media/preset on the install tabs are in-memory only. The retry/reset
confirmations already enforce this. New behavior must not introduce any
intermediate write.

### REQUIRED_FIELD_VALIDATION

// SOURCE: `src/crosshook-native/src/components/wizard/wizardValidation.ts` (Phase 2)

```ts
export function evaluateWizardRequiredFields(args: {
  profileName: string;
  profile: GameProfile;
  launchMethod: LaunchMethod;
}): WizardValidationResult {
  /* returns checklist */
}
```

The install panel reuses `evaluateWizardRequiredFields` directly against
its `draftProfile` plus an install-only addendum that adds `installer_path`
to the required-field set. The composed result drives both the
`WizardReviewSummary` UI and the Install button's disabled state.

### PRESET_PICKER_REUSE

// SOURCE: `src/crosshook-native/src/components/wizard/WizardPresetPicker.tsx:28-122`

```tsx
<WizardPresetPicker
  bundledPresets={bundledOptimizationPresets}
  savedPresetNames={Object.keys(draftProfile.launch.presets ?? {})}
  activePresetKey={draftProfile.launch.active_preset ?? ''}
  busy={false}
  onApplyBundled={async (presetId) => {
    /* in-memory mutate */
  }}
  onSelectSaved={async (presetName) => {
    /* in-memory mutate */
  }}
/>
```

The picker is intentionally pure: it dispatches caller-supplied callbacks. In
the install flow there is no persisted profile yet, so both callbacks mutate
`draftProfile.launch.active_preset` (and seed `draftProfile.launch.presets`
for the bundled case) in memory. The user re-applies via the canonical
Launch Optimizations panel after the profile is saved if they want to
fine-tune.

### CSS_CLASS_HIERARCHY

// SOURCE: `src/crosshook-native/src/styles/theme.css:599-624`

```css
.crosshook-install-flow-tabs > .crosshook-subtab-row {
  /* ... */
}
.crosshook-install-flow-tabs > .crosshook-subtab-content {
  /* ... */
}
.crosshook-install-flow-tabs .crosshook-subtab-content__inner {
  /* ... */
}
```

Reuse the `crosshook-install-flow-tabs` namespace. The 5-tab layout requires
no new scroll containers — `crosshook-install-shell__content` is the single
scroll owner, and reused sections render directly inside the existing
`crosshook-subtab-content__inner` blocks.

### TEST_STRUCTURE

// SOURCE: `src/crosshook-native/crates/crosshook-core/src/install/models.rs:239-278` and `src/crosshook-native/Cargo.toml`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reviewable_profile_uses_install_details_without_persisting_runtime_target() {
        let temp_dir = tempdir().expect("temp dir");
        let prefix_path = temp_dir.path().join("prefix");
        let request = InstallGameRequest { /* ... */ };
        let profile = request.reviewable_profile(&prefix_path);
        assert_eq!(profile.game.name, "God of War Ragnarok");
        // ...
    }
}
```

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cd src/crosshook-native && npm run build
```

There is no configured frontend unit-test framework in this repo. Verification
relies on the Rust test suite for backend invariants (the new fields on
`reviewable_profile()` MUST get a new `#[test]`) and on `tsc` + Vite build +
manual route checks for the install UI.

---

## Files to Change

| File                                                                   | Action           | Justification                                                                                                                                                                                             |
| ---------------------------------------------------------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/install/models.rs`     | UPDATE           | Add 5 optional fields to `InstallGameRequest` (`runner_method`, `steam_app_id`, `custom_portrait_art_path`, `custom_background_art_path`, `working_directory`); update `reviewable_profile()` to set them |
| `src/crosshook-native/crates/crosshook-core/src/install/service.rs`    | UPDATE           | Add lightweight `validate_optional_custom_portrait_art_path` / `validate_optional_custom_background_art_path` checks; new test asserting all fields land on `reviewable_profile()`                        |
| `src/crosshook-native/src/types/install.ts`                            | UPDATE           | Mirror Rust struct: 5 new optional fields, plus 4 new `InstallGameValidationError` variants and field-map entries                                                                                         |
| `src/crosshook-native/src/hooks/useInstallGame.ts`                     | UPDATE           | Add `draftProfile` state + `updateDraftProfile`; add `installerInputs` slim store; derive `request` at submit time; sync `result.profile` into `draftProfile` after install                               |
| `src/crosshook-native/src/components/InstallGamePanel.tsx`             | UPDATE           | Replace inline `InstallField` body with 5-tab composition over canonical `profile-sections/*` + `WizardPresetPicker` + `WizardReviewSummary`; add `installer_review` tab; honor trainer-skip rule         |
| `src/crosshook-native/src/components/install/installValidation.ts`     | CREATE           | `evaluateInstallRequiredFields(args)` — wraps `evaluateWizardRequiredFields` and adds the `installer_path` requirement; single source of truth for the Install button gate                                |
| `src/crosshook-native/src/components/install/InstallReviewSummary.tsx` | CREATE           | Tab 5 composition: `WizardReviewSummary` + install-specific status (stage, candidates, helper log); avoids growing `InstallGamePanel.tsx` past the 800-line repo cap                                      |
| `src/crosshook-native/src/styles/theme.css`                            | UPDATE           | Minor: ensure the 5-tab subtab row wraps correctly on Steam Deck width; add `crosshook-install-flow-tabs__skip-trainer` modifier (empty rule reserved if needed); no new scroll containers                |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`            | UPDATE (minimal) | Confirm `handleOpenProfileReview` payload still resolves with the richer `generatedProfile`; only touch the file if a TS error surfaces                                                                   |
| `src/crosshook-native/src-tauri/src/commands/install.rs`               | UPDATE (smoke)   | No code change required — Serde picks up new optional fields automatically via `#[serde(default)]`. Re-run `cargo test -p crosshook-core` to confirm                                                      |
| `.claude/PRPs/plans/ui-standardization-phase-3.plan.md`                | CREATE           | This plan                                                                                                                                                                                                 |

## NOT Building

- Any TOML / SQLite / metadata DB schema migration. The new
  `InstallGameRequest` fields are additive optionals with `#[serde(default)]`
  and stay confined to the install IPC payload.
- A new install runtime, helper, or installer-discovery feature. Phase 3 is
  pure UI parity + an additive request extension.
- A redesigned `ProfileReviewModal` — the existing modal continues to receive
  the merged draft. Internal layout of the modal is out of scope.
- Removing or renaming `InstallGamePanelProps` (`onOpenProfileReview`,
  `onRequestInstallAction`) — `InstallPage` consumers stay stable.
- Dropping the existing `useInstallGame` public API contract. We add new
  methods/fields and keep legacy ones during the refactor; the panel is the
  only consumer, so no other call site needs to change.
- Splitting `InstallGamePanel.tsx` into per-tab files unless it exceeds the
  800-line repo cap after the refactor. Target ≤ 600 lines (down from 572 +
  growth from new tab → expected 540-620 with helper extraction).
- Sidebar / navigation IA changes. Phase 4 (`#165`) adds the Setup sidebar
  Run EXE/MSI flow.
- Reworking `Update Game` panel. It lives next to `Install Game` but is out
  of scope for `#162`.
- Changing the route banner contract introduced in Phase 1.
- Adding new readiness/check IPCs. The install panel does not call
  `check_readiness` today and will not start in Phase 3 — readiness
  evaluation against `draftProfile` is local-only.
- Auto-applying a launch preset on install completion. The picker stays
  user-driven; the default state is "no preset selected".

---

## Step-by-Step Tasks

### Task 1: Extend `InstallGameRequest` (Rust + TS) with profile-parity fields

- **ACTION**: Add 5 optional fields to the Rust struct and its TS mirror.
- **IMPLEMENT**:
  - In `src/crosshook-native/crates/crosshook-core/src/install/models.rs`,
    extend `InstallGameRequest` with:

    ```rust
    #[serde(default)]
    pub runner_method: String,                    // "" | "proton_run" | "steam_applaunch" | "native"
    #[serde(default)]
    pub steam_app_id: String,
    #[serde(default)]
    pub custom_portrait_art_path: String,
    #[serde(default)]
    pub custom_background_art_path: String,
    #[serde(default)]
    pub working_directory: String,
    ```

    Use `#[serde(default)]` so existing IPC payloads (older clients) keep
    deserializing — additive backwards-compatible change.

  - Update `reviewable_profile(&self, prefix_path: &Path) -> GameProfile` so
    the new fields populate the generated profile:
    - `runner_method` → `profile.launch.method` (default to `"proton_run"`
      when empty, matching today's behavior).
    - `steam_app_id`:
      - When `runner_method == "steam_applaunch"` → `profile.steam.app_id`.
      - When `runner_method == "proton_run"` → `profile.runtime.steam_app_id`.
      - Otherwise: leave both empty.
    - `custom_portrait_art_path` → `profile.game.custom_portrait_art_path`.
    - `custom_background_art_path` → `profile.game.custom_background_art_path`.
    - `working_directory`:
      - When `runner_method == "proton_run"` or `runner_method == "native"` →
        `profile.runtime.working_directory`. (Steam-app-launch derives
        working dir from the executable; do not set it for that branch.)
  - In `src/crosshook-native/src/types/install.ts`, extend the
    `InstallGameRequest` interface with the same 5 fields. Keep the field
    order matching the Rust struct.

- **MIRROR**: `NAMING_CONVENTION`.
- **IMPORTS**: Rust: nothing new (everything used already imported). TS:
  nothing new (interface-only change).
- **GOTCHA**:
  - The Rust `runner_method` is a `String`, not an enum. Validate values at
    the UI layer (the runner method dropdown enforces the union); the
    backend treats unknown values as `"proton_run"` for backwards
    compatibility.
  - Empty strings remain valid for every new field — keep `#[serde(default)]`
    so old clients still work.
  - Existing test
    `reviewable_profile_uses_install_details_without_persisting_runtime_target`
    must keep passing. Keep its assertions; do not regress them.
- **VALIDATE**:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core install::models::tests`
  - `cd src/crosshook-native && npm run build` (TS compile only — UI not yet
    updated, expect no errors because the interface change is additive).

### Task 2: Add a new Rust unit test for the extended `reviewable_profile()`

- **ACTION**: Add a focused test in `install/models.rs` that asserts every
  new field lands on the generated profile.
- **IMPLEMENT**:

  ```rust
  #[test]
  fn reviewable_profile_propagates_extended_request_fields() {
      let temp_dir = tempdir().expect("temp dir");
      let prefix_path = temp_dir.path().join("prefix");

      let request = InstallGameRequest {
          profile_name: "example-game".to_string(),
          display_name: "Example Game".to_string(),
          installer_path: "/installer.exe".to_string(),
          trainer_path: String::new(),
          proton_path: "/proton".to_string(),
          prefix_path: prefix_path.to_string_lossy().into_owned(),
          installed_game_executable_path: String::new(),
          custom_cover_art_path: "/cover.png".to_string(),
          runner_method: "proton_run".to_string(),
          steam_app_id: "1245620".to_string(),
          custom_portrait_art_path: "/portrait.png".to_string(),
          custom_background_art_path: "/background.png".to_string(),
          working_directory: "/work".to_string(),
      };

      let profile = request.reviewable_profile(&prefix_path);

      assert_eq!(profile.launch.method, "proton_run");
      assert_eq!(profile.runtime.steam_app_id, "1245620");
      assert_eq!(profile.game.custom_portrait_art_path, "/portrait.png");
      assert_eq!(profile.game.custom_background_art_path, "/background.png");
      assert_eq!(profile.runtime.working_directory, "/work");
      assert!(profile.steam.app_id.is_empty()); // proton_run does not set steam.app_id
  }

  #[test]
  fn reviewable_profile_routes_steam_app_id_for_steam_applaunch() {
      let temp_dir = tempdir().expect("temp dir");
      let prefix_path = temp_dir.path().join("prefix");

      let request = InstallGameRequest {
          profile_name: "example".to_string(),
          display_name: "Example".to_string(),
          installer_path: "/installer.exe".to_string(),
          trainer_path: String::new(),
          proton_path: "/proton".to_string(),
          prefix_path: prefix_path.to_string_lossy().into_owned(),
          installed_game_executable_path: String::new(),
          custom_cover_art_path: String::new(),
          runner_method: "steam_applaunch".to_string(),
          steam_app_id: "1245620".to_string(),
          custom_portrait_art_path: String::new(),
          custom_background_art_path: String::new(),
          working_directory: String::new(),
      };

      let profile = request.reviewable_profile(&prefix_path);
      assert_eq!(profile.launch.method, "steam_applaunch");
      assert_eq!(profile.steam.app_id, "1245620");
      assert!(profile.runtime.steam_app_id.is_empty());
  }
  ```

- **MIRROR**: `TEST_STRUCTURE`.
- **IMPORTS**: `tempfile::tempdir` (already imported by the existing test).
- **GOTCHA**: Tests live alongside `models.rs` already; do not introduce a
  new integration test crate.
- **VALIDATE**:
  `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core install::models::tests`
  → expect both new tests to pass and existing test to remain green.

### Task 3: Refactor `useInstallGame` to expose a `draftProfile` + `installerInputs` model

- **ACTION**: Introduce a `draftProfile: GameProfile` state and an
  `updateDraftProfile` updater alongside the existing request store. Derive
  `request` only at submit time. Sync results back into `draftProfile`.
- **IMPLEMENT**:
  - Add a new local helper in `useInstallGame.ts`:

    ```ts
    function createEmptyDraftProfile(): GameProfile {
      // Re-use the canonical default from src/types/profile.ts (createDefaultProfile)
      // so the install draft is structurally identical to a wizard draft.
      return createDefaultProfile();
    }
    ```

    Import `createDefaultProfile` from `../types/profile` (it already exists
    — used by `useProfile.ts`; verify the symbol path during implementation).
    If it does not exist as a top-level export, lift the existing factory
    in `useProfile.ts` into `types/profile.ts` as a small refactor.

  - Add state hooks:

    ```ts
    const [draftProfile, setDraftProfileState] = useState<GameProfile>(createEmptyDraftProfile);
    ```

  - Add a typed updater:

    ```ts
    const updateDraftProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
      setDraftProfileState((current) => updater(current));
    }, []);
    ```

  - Add a reverse-sync effect: whenever `request.profile_name`,
    `request.display_name`, `request.custom_cover_art_path`,
    `request.launcher_icon_path`, `request.proton_path`, `request.prefix_path`,
    or `request.installed_game_executable_path` changes via the legacy
    `updateRequest` API, project the value onto `draftProfile`. Likewise,
    forward-sync from `draftProfile` to `request` when `updateDraftProfile`
    touches any of those fields.
    - **Simpler alternative** (preferred): drop the dual-store sync entirely
      by making the panel call `updateDraftProfile` for everything that
      maps to a profile field, and only call `updateRequest` for the
      install-only `installer_path` and `trainer_path`. The hook builds the
      `InstallGameRequest` at submit time:

      ```ts
      const buildInstallGameRequest = useCallback((): InstallGameRequest => {
        const launchMethod = (draftProfile.launch.method || 'proton_run') as LaunchMethod;
        const protonPath =
          launchMethod === 'steam_applaunch' ? draftProfile.steam.proton_path : draftProfile.runtime.proton_path;
        const prefixPath =
          launchMethod === 'steam_applaunch' ? draftProfile.steam.compatdata_path : draftProfile.runtime.prefix_path;
        const steamAppId =
          launchMethod === 'steam_applaunch' ? draftProfile.steam.app_id : (draftProfile.runtime.steam_app_id ?? '');
        return {
          profile_name: profileName,
          display_name: draftProfile.game.name,
          installer_path: installerInputs.installer_path,
          trainer_path: draftProfile.trainer.path,
          proton_path: protonPath,
          prefix_path: prefixPath,
          installed_game_executable_path: draftProfile.game.executable_path,
          launcher_icon_path: draftProfile.steam.launcher.icon_path,
          custom_cover_art_path: draftProfile.game.custom_cover_art_path ?? '',
          runner_method: launchMethod,
          steam_app_id: steamAppId,
          custom_portrait_art_path: draftProfile.game.custom_portrait_art_path ?? '',
          custom_background_art_path: draftProfile.game.custom_background_art_path ?? '',
          working_directory: draftProfile.runtime.working_directory ?? '',
        };
      }, [draftProfile, installerInputs.installer_path, profileName]);
      ```

      Adopt this approach: it removes the bidirectional sync risk and keeps
      `installerInputs` minimal (`installer_path` only — `trainer_path`
      lives on `draftProfile.trainer.path`).

  - Add a slim `installerInputs` slice:

    ```ts
    interface InstallerInputs {
      installer_path: string;
    }
    const [installerInputs, setInstallerInputs] = useState<InstallerInputs>({
      installer_path: '',
    });
    const updateInstallerInputs = useCallback(
      <Key extends keyof InstallerInputs>(key: Key, value: InstallerInputs[Key]) => {
        setInstallerInputs((current) => ({ ...current, [key]: value }));
      },
      []
    );
    ```

  - Add a separate `profileName` state hook (decoupled from the legacy
    request because the install draft profile and the install request both
    need it independently):

    ```ts
    const [profileName, setProfileNameState] = useState('');
    const setProfileName = useCallback((value: string) => {
      setProfileNameState(value);
    }, []);
    ```

  - Update `startInstall()` to call `buildInstallGameRequest()` once at the
    top, then call `validate_install_request` and `install_game` with the
    derived request — never read from the legacy `request` state inside
    `startInstall`.
  - Update `setResult()` to merge `installResult.profile` into
    `draftProfile` while preserving local-only edits the backend doesn't
    care about. Use a shallow merge:

    ```ts
    const setResult = useCallback((nextResult: InstallGameResult | null) => {
      setResultState(nextResult);
      if (nextResult === null) {
        setStageState('idle');
        setErrorState(null);
        return;
      }
      // Merge backend profile onto draft, preserving the user's existing
      // identity/media/runtime selections that the backend already echoed
      // back via the extended request → reviewable_profile path.
      setDraftProfileState((current) => ({
        ...nextResult.profile,
        // Force the candidate-derived executable so the Review tab updates.
        game: {
          ...nextResult.profile.game,
          // Custom art can come back populated from the request — keep
          // whichever side is non-empty (user wins).
          custom_cover_art_path: current.game.custom_cover_art_path?.trim()
            ? current.game.custom_cover_art_path
            : nextResult.profile.game.custom_cover_art_path,
          custom_portrait_art_path: current.game.custom_portrait_art_path?.trim()
            ? current.game.custom_portrait_art_path
            : nextResult.profile.game.custom_portrait_art_path,
          custom_background_art_path: current.game.custom_background_art_path?.trim()
            ? current.game.custom_background_art_path
            : nextResult.profile.game.custom_background_art_path,
        },
      }));
      setStageState(deriveResultStage(nextResult));
      setErrorState(nextResult.succeeded ? null : nextResult.message);
    }, []);
    ```

  - Update `setInstalledExecutablePath` to write `executable_path` and
    `working_directory` directly onto `draftProfile`. Keep the existing
    `result?.succeeded` branch that flips `stage` between `ready_to_save`
    and `review_required`.
  - Update `reset()` to clear `draftProfile`, `installerInputs`, and
    `profileName` alongside the existing fields.
  - Update the auto-resolve effect on `request.profile_name` to depend on
    the new `profileName` state instead, and to write the resolved prefix
    onto `draftProfile.runtime.prefix_path` (or `draftProfile.steam.compatdata_path`
    when `launchMethod === 'steam_applaunch'`).
  - Expand the `UseInstallGameResult` interface with:

    ```ts
    profileName: string;
    setProfileName: (value: string) => void;
    draftProfile: GameProfile;
    updateDraftProfile: (updater: (current: GameProfile) => GameProfile) => void;
    installerInputs: InstallerInputs;
    updateInstallerInputs: <Key extends keyof InstallerInputs>(key: Key, value: InstallerInputs[Key]) => void;
    ```

  - Mark the legacy `request` / `updateRequest` / `patchRequest` /
    `setRequest` / `setReviewProfile` exports as deprecated in a JSDoc
    comment but **keep them** during this refactor — the panel is the only
    consumer and will be updated in Task 4. After Task 4 lands, run a
    repo-wide grep and remove dead exports as part of Task 5.

- **MIRROR**: `REQUEST_VS_DRAFT_SPLIT`, `NO_WRITE_BEFORE_REVIEW`.
- **IMPORTS**: `import { createDefaultProfile } from '../types/profile';`
  (verify the export exists; if not, lift the factory from `useProfile.ts`).
  `import type { GameProfile, LaunchMethod } from '../types';` (already
  imported).
- **GOTCHA**:
  - Do not call `setRequestState` for fields owned by `draftProfile` after
    Task 4 lands — `draftProfile` is the only source of truth for them.
  - The auto-resolve prefix effect must depend on `profileName` (not the
    legacy `request.profile_name`) to avoid stale closures.
  - When `launchMethod` switches between `steam_applaunch` and `proton_run`,
    the prefix/proton fields live on different sub-objects in `GameProfile`
    (`steam.compatdata_path` vs `runtime.prefix_path`,
    `steam.proton_path` vs `runtime.proton_path`). The hook must NOT auto-
    copy values across — that's the user's job. Only the **default-prefix
    auto-resolve** writes a value, and it writes to whichever sub-object
    matches the current `launchMethod`.
  - Preserve the BR-9 invariant: nothing in this hook persists profile data.
    The only IPCs are `install_default_prefix_path`, `validate_install_request`,
    and `install_game` — all of which already exist.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run build` (compile passes — Task 4 is
    not yet done so the panel will still consume the legacy `request` API;
    the new `draftProfile`/`installerInputs` are additive).
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
    (sanity smoke, no Rust touched here).

### Task 4: Refactor `InstallGamePanel.tsx` to compose canonical sections in a 5-tab layout

- **ACTION**: Replace the inline `InstallField`-driven body with a 5-tab
  composition that consumes `profile-sections/*` against the new
  `draftProfile` state.
- **IMPLEMENT**:
  - Update the tab type and label map:

    ```ts
    type InstallFlowTabId = 'identity' | 'runtime' | 'trainer' | 'media' | 'installer_review';

    const INSTALL_FLOW_TAB_LABELS: Record<InstallFlowTabId, string> = {
      identity: 'Identity & Game',
      runtime: 'Runtime',
      trainer: 'Trainer',
      media: 'Media',
      installer_review: 'Installer & Review',
    };
    ```

  - Replace `useState<InstallFlowTabId>('identity')` initial value with
    `'identity'` (unchanged).
  - Compute the visible tab list with the trainer skip rule:

    ```ts
    const launchMethod = resolveLaunchMethod(draftProfile);
    const installFlowTabs = useMemo(() => {
      const all: InstallFlowTabId[] = ['identity', 'runtime', 'trainer', 'media', 'installer_review'];
      const visible = launchMethod === 'native' ? all.filter((id) => id !== 'trainer') : all;
      return visible.map((id) => ({ id, label: INSTALL_FLOW_TAB_LABELS[id] }));
    }, [launchMethod]);
    ```

  - When `activeInstallTab === 'trainer'` and the user switches
    `launchMethod` to `'native'`, snap `activeInstallTab` to `'media'` via a
    `useEffect`.
  - Pull from the hook (replace destructured `request`/`updateRequest`):

    ```ts
    const {
      profileName,
      setProfileName,
      draftProfile,
      updateDraftProfile,
      installerInputs,
      updateInstallerInputs,
      validation,
      stage,
      result,
      reviewProfile,
      error,
      defaultPrefixPath,
      defaultPrefixPathState,
      defaultPrefixPathError,
      candidateOptions,
      isRunningInstaller,
      isResolvingDefaultPrefixPath,
      setInstalledExecutablePath,
      startInstall,
      reset,
      actionLabel,
      statusText,
      hintText,
    } = useInstallGame();
    ```

  - **Tab 1 — Identity & Game**:

    ```tsx
    <Tabs.Content
      value="identity"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeInstallTab === 'identity' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
        <ProfileIdentitySection
          profileName={profileName}
          profile={draftProfile}
          onProfileNameChange={setProfileName}
          onUpdateProfile={updateDraftProfile}
        />
        {/* Display Name lives only on the install request — game.name carries it forward */}
        <RunnerMethodSection profile={draftProfile} onUpdateProfile={updateDraftProfile} />
      </div>
    </Tabs.Content>
    ```

    Note: `ProfileIdentitySection` already covers Profile Name + Game Name.
    The legacy "Display Name" field maps onto `draftProfile.game.name` —
    drop the standalone Display Name input. (The Rust struct still accepts
    `display_name`, but `buildInstallGameRequest` populates it from
    `draftProfile.game.name`.)

  - **Tab 2 — Runtime**:

    ```tsx
    <Tabs.Content
      value="runtime"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeInstallTab === 'runtime' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
        <RuntimeSection
          profile={draftProfile}
          onUpdateProfile={updateDraftProfile}
          launchMethod={launchMethod}
          protonInstalls={protonInstalls}
          protonInstallsError={protonInstallsError}
        />
        {/* Default-prefix resolver hint for the install context */}
        <p className="crosshook-help-text">
          {prefixStateLabel(defaultPrefixPathState)}
          {defaultPrefixPath.trim().length > 0 ? ` Suggested default prefix: ${defaultPrefixPath}` : null}
        </p>
      </div>
    </Tabs.Content>
    ```

    `RuntimeSection` already exposes Steam App ID (both runners), prefix
    path, proton path, working directory override, and the AutoPopulate
    helper for Steam app launch — full parity with the editor.

  - **Tab 3 — Trainer** (skipped tab when `launchMethod === 'native'`):

    ```tsx
    {
      launchMethod !== 'native' && (
        <Tabs.Content
          value="trainer"
          forceMount
          className="crosshook-subtab-content"
          style={{ display: activeInstallTab === 'trainer' ? undefined : 'none' }}
        >
          <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
            <TrainerSection
              profile={draftProfile}
              onUpdateProfile={updateDraftProfile}
              launchMethod={launchMethod}
              profileName={profileName}
              profileExists={false}
            />
          </div>
        </Tabs.Content>
      );
    }
    ```

  - **Tab 4 — Media**:

    ```tsx
    <Tabs.Content
      value="media"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeInstallTab === 'media' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
        <MediaSection profile={draftProfile} onUpdateProfile={updateDraftProfile} launchMethod={launchMethod} />
      </div>
    </Tabs.Content>
    ```

    Cover/Portrait/Background art are now wired through the canonical
    `MediaSection` (which also offers Launcher Icon when
    `launchMethod !== 'native'`).

  - **Tab 5 — Installer & Review** (delegated to `InstallReviewSummary`):

    ```tsx
    <Tabs.Content
      value="installer_review"
      forceMount
      className="crosshook-subtab-content"
      style={{ display: activeInstallTab === 'installer_review' ? undefined : 'none' }}
    >
      <div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--wide-gap">
        <div className="crosshook-install-section">
          <div className="crosshook-install-section-title">Installer Media</div>
          <InstallField
            label="Installer EXE"
            value={installerInputs.installer_path}
            onChange={(value) => updateInstallerInputs('installer_path', value)}
            placeholder="/mnt/media/setup.exe"
            browseLabel="Browse"
            browseTitle="Select Installer Executable"
            browseFilters={[{ name: 'Windows Executable', extensions: ['exe'] }]}
            helpText="Choose the installer media, not the final game executable."
            error={validation.fieldErrors.installer_path}
          />
        </div>

        <WizardPresetPicker
          bundledPresets={bundledOptimizationPresets}
          savedPresetNames={Object.keys(draftProfile.launch.presets ?? {})}
          activePresetKey={draftProfile.launch.active_preset ?? ''}
          busy={false}
          onApplyBundled={async (presetId) => {
            updateDraftProfile((current) => ({
              ...current,
              launch: {
                ...current.launch,
                active_preset: bundledOptimizationTomlKey(presetId),
              },
            }));
          }}
          onSelectSaved={async (presetName) => {
            updateDraftProfile((current) => ({
              ...current,
              launch: {
                ...current.launch,
                active_preset: presetName,
              },
            }));
          }}
        />

        <InstallReviewSummary
          installation={{
            stage,
            statusText,
            hintText,
            error,
            generalError: validation.generalError,
            candidateOptions,
            currentExecutablePath: draftProfile.game.executable_path,
            onSelectCandidate: setInstalledExecutablePath,
            helperLogPath: result?.helper_log_path ?? '',
            isRunningInstaller,
            defaultPrefixPathState,
            candidateCount: candidateOptions.length,
          }}
          validation={evaluateInstallRequiredFields({
            profileName,
            profile: draftProfile,
            launchMethod,
            installerPath: installerInputs.installer_path,
          })}
        />
      </div>
    </Tabs.Content>
    ```

  - Pull `bundledOptimizationPresets` from `useProfileContext()` (already
    exported via `UseProfileResult`). The install panel does not currently
    consume this context — add `import { useProfileContext } from
'../context/ProfileContext';` and destructure
    `const { bundledOptimizationPresets } = useProfileContext();`.
  - Replace the existing `<button type="button" className="crosshook-button">{actionLabel}</button>`
    Install Game button with a disabled-state-aware version:

    ```tsx
    const installValidation = useMemo(
      () =>
        evaluateInstallRequiredFields({
          profileName,
          profile: draftProfile,
          launchMethod,
          installerPath: installerInputs.installer_path,
        }),
      [profileName, draftProfile, launchMethod, installerInputs.installer_path]
    );

    // ... in the footer ...
    <button
      type="button"
      className="crosshook-button"
      disabled={isRunningInstaller || isResolvingDefaultPrefixPath || !installValidation.isReady}
      aria-describedby={!installValidation.isReady ? installRequiredHintId : undefined}
      onClick={async () => {
        const shouldProceed = await Promise.resolve(onRequestInstallAction?.('retry') ?? true);
        if (!shouldProceed) return;
        await startInstall();
      }}
    >
      {actionLabel}
    </button>;
    ```

  - Drop the legacy "Generated profile preview" rows and the "Custom Cover
    Art" / "Launcher Icon" duplications from the old Identity tab — they
    are now owned by `MediaSection` (Tab 4).
  - Drop the dominant-color hero (`useGameCoverArt` /
    `useImageDominantColor`) **from the install panel itself** if it
    duplicates the new `RouteBanner`. Verify `RouteBanner route="install"`
    is rendering above the panel via `InstallPage.tsx:328` — if so, the
    panel-local hero is redundant and should go.
    - **Conditional**: keep the panel-local hero only if removing it
      regresses the gradient/cover-art preview that informs install context.
      Manual smoke test should confirm. If kept, ensure it reads from
      `draftProfile.game.custom_cover_art_path` instead of
      `request.custom_cover_art_path`.

- **MIRROR**: `REUSED_SECTION_COMPOSITION`, `NO_WRITE_BEFORE_REVIEW`,
  `PRESET_PICKER_REUSE`, `CSS_CLASS_HIERARCHY`.
- **IMPORTS**:

  ```ts
  import { ProfileIdentitySection } from './profile-sections/ProfileIdentitySection';
  import { RunnerMethodSection } from './profile-sections/RunnerMethodSection';
  import { RuntimeSection } from './profile-sections/RuntimeSection';
  import { TrainerSection } from './profile-sections/TrainerSection';
  import { MediaSection } from './profile-sections/MediaSection';
  import { WizardPresetPicker } from './wizard/WizardPresetPicker';
  import { bundledOptimizationTomlKey } from '../utils/launchOptimizationPresets';
  import { useProfileContext } from '../context/ProfileContext';
  import { resolveLaunchMethod } from '../utils/launch';
  import { evaluateInstallRequiredFields } from './install/installValidation';
  import { InstallReviewSummary } from './install/InstallReviewSummary';
  ```

  Drop the now-unused `ProtonPathField` import — `RuntimeSection` owns
  proton path. Drop `useGameCoverArt` / `useImageDominantColor` if removing
  the panel-local hero.

- **GOTCHA**:
  - `useProfileContext()` must be called only inside the function body
    (already enforced by React rules). The install panel's parent
    (`InstallPage`) already runs inside a `ProfileProvider` — verify by
    grep'ing for `<ProfileProvider>` in `App.tsx`. If not, this will throw.
  - The trainer-skip rule MUST be applied symmetrically: when the user
    sets `launchMethod` to `'native'` while `activeInstallTab === 'trainer'`,
    snap to the next visible tab (`'media'`) via a `useEffect`.
  - The Install button's disabled state depends on `installValidation.isReady`
    — make sure the validation helper considers the correct fields for
    each `launchMethod` (Task 5).
  - The auto-open `useEffect` that opens the review modal after a successful
    install (lines 201-219) must continue to use `reviewableInstallResult`
    - `reviewProfile`. Update its dependency array to consume `draftProfile`
      instead of `request.launcher_icon_path` / `request.profile_name`.
  - File budget: target ≤ 600 lines after the refactor. Extracting
    `InstallReviewSummary` (Task 6) and `installValidation` (Task 5) keeps
    `InstallGamePanel.tsx` slim. If the file still exceeds the cap, extract
    per-tab bodies into `src/crosshook-native/src/components/install/tabs/*.tsx`.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run build` (TypeScript + Vite) — expect
    zero errors.
  - Manual: open the panel from the Install Game route, click each tab in
    order, confirm fields render via the canonical components.

### Task 5: Create the install validation helper

- **ACTION**: Create
  `src/crosshook-native/src/components/install/installValidation.ts` that
  wraps `evaluateWizardRequiredFields` and adds the install-only
  `installer_path` requirement.
- **IMPLEMENT**:

  ```ts
  import type { GameProfile, LaunchMethod } from '../../types';
  import { evaluateWizardRequiredFields, type WizardValidationResult } from '../wizard/wizardValidation';

  export interface EvaluateInstallRequiredFieldsArgs {
    profileName: string;
    profile: GameProfile;
    launchMethod: LaunchMethod;
    installerPath: string;
  }

  export function evaluateInstallRequiredFields(args: EvaluateInstallRequiredFieldsArgs): WizardValidationResult {
    const wizardResult = evaluateWizardRequiredFields({
      profileName: args.profileName,
      profile: args.profile,
      launchMethod: args.launchMethod,
    });
    const installerSatisfied = args.installerPath.trim().length > 0;
    const fields = [
      ...wizardResult.fields,
      {
        id: 'installer_path',
        label: 'Installer EXE',
        isSatisfied: installerSatisfied,
      },
    ];
    return {
      fields,
      isReady: fields.every((field) => field.isSatisfied),
    };
  }
  ```

- **MIRROR**: `REQUIRED_FIELD_VALIDATION`.
- **IMPORTS**: `GameProfile`, `LaunchMethod` from `../../types`;
  `evaluateWizardRequiredFields`, `WizardValidationResult` from
  `../wizard/wizardValidation`.
- **GOTCHA**:
  - Keep this helper pure — no React hooks, no IPC.
  - The wizard's required-field set already covers the
    runner-method-conditional logic (Steam App ID + prefix + proton for
    `steam_applaunch`, prefix + proton for `proton_run`, no extras for
    `native`). Do not duplicate that logic here.
  - The install-only `installed_game_executable_path` is **not** required
    pre-install — it gets populated after the installer completes. Do
    not add it to the required-field set; the existing
    `setInstalledExecutablePath` flow gates the post-install Save instead.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run build` — compile passes.
  - Spot-check by reading the file and confirming the field IDs match
    `draftProfile` paths.

### Task 6: Create the `InstallReviewSummary` component

- **ACTION**: Create
  `src/crosshook-native/src/components/install/InstallReviewSummary.tsx`
  to host the install-specific status, candidate list, helper log, and the
  reused `WizardReviewSummary` block.
- **IMPLEMENT**:

  ```tsx
  import { WizardReviewSummary } from '../wizard/WizardReviewSummary';
  import type { WizardValidationResult } from '../wizard/wizardValidation';
  import type {
    InstallGameExecutableCandidate,
    InstallGamePrefixPathState,
    InstallGameStage,
  } from '../../types/install';

  interface InstallationStatus {
    stage: InstallGameStage;
    statusText: string;
    hintText: string;
    error: string | null;
    generalError: string | null;
    candidateOptions: readonly InstallGameExecutableCandidate[];
    currentExecutablePath: string;
    onSelectCandidate: (path: string) => void;
    helperLogPath: string;
    isRunningInstaller: boolean;
    defaultPrefixPathState: InstallGamePrefixPathState;
    candidateCount: number;
  }

  export interface InstallReviewSummaryProps {
    installation: InstallationStatus;
    validation: WizardValidationResult;
  }

  function stageLabel(stage: InstallGameStage): string {
    switch (stage) {
      case 'preparing':
        return 'Preparing';
      case 'running_installer':
        return 'Running installer';
      case 'review_required':
        return 'Review required';
      case 'ready_to_save':
        return 'Ready to save';
      case 'failed':
        return 'Failed';
      case 'idle':
      default:
        return 'Idle';
    }
  }

  function prefixStateLabel(state: InstallGamePrefixPathState): string {
    switch (state) {
      case 'loading':
        return 'Resolving default prefix...';
      case 'ready':
        return 'Default prefix resolved';
      case 'failed':
        return 'Default prefix unavailable';
      case 'idle':
      default:
        return 'Awaiting profile name';
    }
  }

  export function InstallReviewSummary({ installation, validation }: InstallReviewSummaryProps) {
    const {
      stage,
      statusText,
      hintText,
      error,
      generalError,
      candidateOptions,
      currentExecutablePath,
      onSelectCandidate,
      helperLogPath,
      isRunningInstaller,
      defaultPrefixPathState,
      candidateCount,
    } = installation;

    return (
      <div className="crosshook-install-card">
        <div className="crosshook-install-status">
          <div>
            <div className="crosshook-install-stage">{stageLabel(stage)}</div>
            <p className="crosshook-heading-copy" style={{ marginTop: 8 }}>
              {statusText}
            </p>
          </div>
          <div style={{ display: 'grid', gap: 10, justifyItems: 'end' }}>
            <div className="crosshook-install-pill">{prefixStateLabel(defaultPrefixPathState)}</div>
            <div className="crosshook-install-pill">Candidates: {candidateCount}</div>
          </div>
        </div>

        <div className="crosshook-install-review">
          {error ? <p className="crosshook-danger">{error}</p> : null}
          {generalError ? <p className="crosshook-danger">{generalError}</p> : null}
          <p className="crosshook-help-text">{hintText}</p>

          {candidateOptions.length > 0 ? (
            <div className="crosshook-install-candidate-list">
              {candidateOptions.map((candidate) => (
                <button
                  key={`${candidate.index}:${candidate.path}`}
                  type="button"
                  className="crosshook-install-candidate"
                  onClick={() => onSelectCandidate(candidate.path)}
                  style={{
                    borderColor:
                      candidate.path === currentExecutablePath
                        ? 'rgba(0, 120, 212, 0.45)'
                        : 'rgba(255, 255, 255, 0.06)',
                  }}
                >
                  <span>{candidate.is_recommended ? `${candidate.path} (recommended)` : candidate.path}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className="crosshook-help-text">
              {isRunningInstaller
                ? 'Candidate discovery will appear after the installer exits.'
                : 'Run the installer to discover candidate executables.'}
            </p>
          )}

          {helperLogPath ? (
            <div className="crosshook-install-candidate" style={{ cursor: 'default', flexDirection: 'column' }}>
              <span>Installer log path</span>
              <span style={{ wordBreak: 'break-all', color: 'var(--crosshook-color-text)' }}>{helperLogPath}</span>
            </div>
          ) : null}
        </div>

        <WizardReviewSummary validation={validation} readinessResult={null} checkError={null} />
      </div>
    );
  }

  export default InstallReviewSummary;
  ```

- **MIRROR**: existing review-tab JSX in `InstallGamePanel.tsx:415-515`,
  `WizardReviewSummary` props contract.
- **IMPORTS**: see snippet above.
- **GOTCHA**:
  - `WizardReviewSummary` accepts `readinessResult` and `checkError`. The
    install panel does not run system readiness checks (different scope
    from the wizard's `runChecks`). Pass `null` for both — the summary
    component already handles the "not run yet" empty state.
  - Do NOT add `console.*` calls.
  - The candidate `<button>` styling intentionally inlines two color
    properties (matches the legacy `CandidateRow`); they're driven by
    selection state, not arbitrary values, so a small inline-style is
    acceptable. If a future pass converts this to BEM, do it then — out
    of scope for Phase 3.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run build` — compile passes.
  - Manual: render the install flow, drive `stage` through `idle →
preparing → running_installer → review_required → ready_to_save` and
    confirm each label/hint renders.

### Task 7: Update CSS for the 5-tab subtab row and minor wrapping

- **ACTION**: Verify `crosshook-install-flow-tabs > .crosshook-subtab-row`
  in `src/crosshook-native/src/styles/theme.css` accommodates 5 tabs on
  Steam Deck width without overflow.
- **IMPLEMENT**:
  - Inspect the existing rule at `theme.css:599-624`. If the row uses
    `flex-wrap: wrap` already (it does — verified during exploration),
    no change is needed.
  - If the row gap is too tight for 5 tabs at 1280×800 (Steam Deck), add a
    `flex-wrap: wrap` + `row-gap: 6px` modifier on
    `.crosshook-install-flow-tabs > .crosshook-subtab-row`. Reuse existing
    `--crosshook-spacing-*` tokens — no hardcoded pixels.
  - Add a focused empty rule (or minimal padding tweak) for
    `.crosshook-install-flow-tabs--skip-trainer` if the trainer-skip path
    causes a visual jump on tab snap. Mark the rule with a comment so
    future passes know it exists for the skip case.
- **MIRROR**: `CSS_CLASS_HIERARCHY`. The Phase 1 rule `.crosshook-install-flow-tabs > .crosshook-subtab-row` already exists and uses tokens.
- **IMPORTS**: none (CSS only).
- **GOTCHA**:
  - Do NOT introduce new `overflow-y: auto` containers anywhere in the
    panel. The single scroll owner is `crosshook-install-shell__content`.
  - Do NOT add new media queries that conflict with existing
    `max-height: 820px` Steam Deck rules elsewhere in the file.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run build` — confirms the CSS file
    still parses.
  - Visual smoke test on a standard 1080p viewport and a `max-height: 820px`
    simulator size (browser dev tools) — confirm the 5 tabs wrap or fit
    without overflow.

### Task 8: Verify mount sites and `InstallPage.tsx` consumer compatibility

- **ACTION**: Confirm `InstallPage.tsx` still consumes `InstallGamePanel`
  with the same props after Tasks 3-6.
- **IMPLEMENT**:
  - `InstallPage.tsx:354-358` — `<InstallGamePanel
onOpenProfileReview={handleOpenProfileReview}
onRequestInstallAction={handleInstallActionConfirmation} />`.
    Props are unchanged. No edit required unless a TS error surfaces.
  - The auto-open `useEffect` in `InstallGamePanel.tsx:201-219` still
    builds an `InstallProfileReviewPayload` with `generatedProfile:
patchedProfile`. The new `patchedProfile` should now reference
    `draftProfile` (which already carries the user-set art, App ID,
    runtime working dir, and active preset). Drop the manual
    `request.launcher_icon_path` patch — `draftProfile.steam.launcher.icon_path`
    already has the correct value.
  - Double-check that `handleSaveProfileReview` (`InstallPage.tsx:253-289`)
    still calls `persistProfileDraft(profileName, draftProfile)` against
    the modal's session — no change required because the modal already
    receives the merged profile.
- **MIRROR**: P1 references above.
- **IMPORTS**: none new.
- **GOTCHA**:
  - The modal's `ProfileFormSections` consumer (`InstallPage.tsx:427-437`)
    receives `profileReviewSession.draftProfile` and edits it through
    `handleProfileReviewUpdate`. Those edits stay in modal-local state and
    do NOT need to feed back into `useInstallGame.draftProfile`. (The
    modal's draft is a snapshot at the moment the user opens it.)
  - When the user clicks **Reset Form**, `useInstallGame.reset()` clears
    `draftProfile`. Confirm the existing `handleInstallActionConfirmation`
    confirmation prompt still appears when there's an unsaved review draft.
- **VALIDATE**:
  - `cd src/crosshook-native && npm run build` — zero errors.
  - Manual: open the install flow, run an install (or simulate via the
    Reset → Retry path), confirm the modal opens with the populated
    `draftProfile` and the user's previously-selected preset/art carries
    through.

### Task 9: Final terminology, accessibility, and Phase 2 parity audit

- **ACTION**: Audit all install-panel copy and aria semantics to match
  Phase 1 banner standardization and Phase 2 wizard labels.
- **IMPLEMENT**:
  - Tab labels: `Identity & Game`, `Runtime`, `Trainer`, `Media`, `Installer & Review`.
  - Section titles inside each tab use the same casing as the wizard:
    `Profile Identity`, `Steam Runtime` / `Proton Runtime` / `Native Runtime`
    (already provided by `RuntimeSection`), `Trainer`, `Game Art` (from
    `MediaSection`), `Launch Preset` (from `WizardPresetPicker`), `Required Fields`
    (from `WizardReviewSummary`).
  - `aria-label` on each `<Tabs.Content>`: match the visible tab label.
  - Confirm focus order: tab list → tab body controls → footer Install /
    Reset / Open Review Modal.
  - Confirm `aria-describedby` on the Install button references a hint id
    listing the first missing required field when disabled.
  - Confirm the panel's heading id (`install-game-heading` at line 222)
    is still bound via `aria-labelledby` on the section.
  - Update the panel's `<p className="crosshook-heading-copy">` body copy
    to mention the new field parity:

    > "This guided flow runs the installer through Proton, surfaces a
    > reviewable profile with full art, runtime, and preset support, and
    > only persists the profile when you confirm Save."

- **MIRROR**: Phase 2 wizard copy (`OnboardingWizard.tsx` step titles) and
  Phase 1 banner terminology in `routeMetadata.ts`.
- **IMPORTS**: none new.
- **GOTCHA**:
  - Do not regress the auto-open review modal flow — the install-complete
    handoff stays as-is.
  - The Install button label comes from `actionLabel` (e.g.,
    `Install Game` / `Installing...` / `Retry Install`). Do not hardcode
    a different label.
- **VALIDATE**:
  - Manual keyboard sweep through every tab; confirm focus trap is sane
    and `Tab` / `Shift+Tab` reach all controls.
  - `gh issue view 162` acceptance criteria checklist verified manually.

---

## Testing Strategy

### Unit Tests

| Test                                                                                 | Input                                                         | Expected Output                                                                  | Edge Case? |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------- | -------------------------------------------------------------------------------- | ---------- |
| `reviewable_profile_propagates_extended_request_fields` (Rust)                       | `runner_method=proton_run`, `steam_app_id=123`, art paths set | `profile.runtime.steam_app_id=123`, art paths populated, `working_directory` set | No         |
| `reviewable_profile_routes_steam_app_id_for_steam_applaunch` (Rust)                  | `runner_method=steam_applaunch`, `steam_app_id=123`           | `profile.steam.app_id=123`, `profile.runtime.steam_app_id=""`                    | Yes        |
| `evaluateInstallRequiredFields` ready when all wizard fields + installer set         | profile name, full proton_run profile, installer path set     | `isReady=true`, all fields satisfied                                             | No         |
| `evaluateInstallRequiredFields` blocks when installer empty                          | wizard fields satisfied, `installerPath=""`                   | `isReady=false`, `installer_path` field unsatisfied                              | No         |
| `evaluateInstallRequiredFields` blocks when Steam App ID missing for steam_applaunch | `launchMethod=steam_applaunch`, `steam.app_id=""`             | `isReady=false`, `steam.app_id` field unsatisfied                                | Yes        |
| `evaluateInstallRequiredFields` does not require Steam App ID for proton_run         | `launchMethod=proton_run`, `runtime.steam_app_id=""`          | `isReady=true` (assuming installer path set)                                     | Yes        |
| `useInstallGame.buildInstallGameRequest` snaps proton/prefix from steam fields       | `draftProfile.launch.method=steam_applaunch`                  | request.proton_path === draftProfile.steam.proton_path                           | Yes        |
| Trainer-tab snap on launchMethod flip                                                | `activeInstallTab=trainer`, then user sets method to native   | `activeInstallTab` snaps to `media`                                              | Yes        |
| `setResult` merges backend profile while preserving local custom_cover_art_path      | local cover set, backend returns empty cover                  | merged profile keeps local cover                                                 | Yes        |

> Note: no frontend test framework is configured. The TS-side cases are
> documented as manual verification steps; if pytest/vitest is later added,
> they map directly to test cases. The Rust cases ARE wired into
> `cargo test -p crosshook-core`.

### Edge Cases Checklist

- [ ] Empty profile name + empty installer path keeps Install disabled.
- [ ] Switching `runner_method` from `proton_run` to `steam_applaunch`
      surfaces Steam App ID as required and disables Install until set.
- [ ] Switching `runner_method` to `native` hides the Trainer tab and
      removes Steam App ID and proton/prefix requirements.
- [ ] Going from Media tab back to Trainer when `launchMethod === native`
      is impossible (tab not rendered) and snapping the active tab to
      `media` works without flicker.
- [ ] Default-prefix auto-resolve writes to
      `draftProfile.runtime.prefix_path` when `launchMethod === proton_run`.
- [ ] Default-prefix auto-resolve writes to
      `draftProfile.steam.compatdata_path` when
      `launchMethod === steam_applaunch`.
- [ ] Custom cover/portrait/background art set on Media tab survives the
      install round-trip and shows up in the modal review.
- [ ] Bundled launch preset selected on Installer & Review tab persists
      onto the saved profile after the modal Save.
- [ ] Saved launch preset (from a prior profile) selected on the
      Installer & Review tab persists onto the saved profile.
- [ ] Steam Deck `max-height: 820px` viewport: no clipped content, all
      tabs reachable, install footer remains pinned.
- [ ] Screen reader announces tab transitions and Install button
      `aria-describedby` hint when disabled.
- [ ] **BR-9** invariant: `Reset Form` and tab navigation never persist
      profile data.

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run build
```

EXPECT: TypeScript + Vite build succeeds with zero errors.

### Unit Tests (Rust)

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core install
```

EXPECT: All `crosshook-core::install::*` tests pass, including the two new
`reviewable_profile_*` tests added in Task 2.

### Full Test Suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: 720+ tests pass (existing 718+3 from Phase 2 + 2 new). No
regressions from UI-only changes.

### Database Validation (if applicable)

```bash
echo "N/A - no persistence/schema changes"
```

EXPECT: N/A. Phase 3 is purely UI + additive IPC payload extension. No TOML,
SQLite, or metadata DB schema migration.

### Browser Validation (if applicable)

```bash
./scripts/dev-native.sh
```

EXPECT: Install Game route renders the new 5-tab panel, all tabs render
canonical sections, Install button respects required-field gating, install
result populates the modal review with the merged draft.

### Manual Validation

- [ ] Open `Install Game` route. Confirm tab list reads
      `Identity & Game / Runtime / Trainer / Media / Installer & Review`.
- [ ] Set `Runner method = native` on Identity tab. Confirm Trainer tab
      disappears.
- [ ] Set `Runner method = steam_applaunch`. Confirm `Steam App ID` is
      required on the Runtime tab and Install button is disabled until
      set.
- [ ] Set `Runner method = proton_run`. Confirm `Steam App ID` is
      optional on the Runtime tab.
- [ ] Type a profile name and confirm the default-prefix auto-resolve
      populates `prefix_path` in `RuntimeSection`.
- [ ] Add cover, portrait, background art on the Media tab. Confirm
      previews render via `MediaSection`.
- [ ] Add a launcher icon. Confirm it persists.
- [ ] Open Installer & Review tab. Pick a bundled launch preset.
      Confirm `draftProfile.launch.active_preset` updates.
- [ ] Run the installer end-to-end with valid paths. After the installer
      exits, confirm the auto-open `ProfileReviewModal` shows the merged
      draft with all the user-set fields populated (cover, portrait,
      background, App ID, preset).
- [ ] Click `Save and Open Profiles` in the modal. Confirm the saved
      profile under `~/.local/share/crosshook/profiles/<name>.toml`
      contains all the new fields.
- [ ] Click `Reset Form`. Confirm the panel returns to a fresh state and
      no profile is persisted.
- [ ] Click `Reset Form` while the review modal is open with unsaved
      edits. Confirm the existing confirmation prompt fires.
- [ ] Verify Steam Deck viewport: `max-height: 820px` simulator does not
      clip the install panel.
- [ ] Confirm the route banner (`RouteBanner route="install"`) renders
      above the panel and tab navigation does not affect it.
- [ ] Confirm focus trap and Escape dismiss still work in the review
      modal.

---

## Acceptance Criteria

- [ ] Install panel renders 5 tabs (4 when `launchMethod === 'native'`)
      with balanced density and wizard-aligned terminology.
- [ ] Every tab body is composed from canonical `profile-sections/*`
      components — no inline duplicate field graphs (the only inline
      `InstallField` left is `Installer EXE` on the Installer & Review
      tab, which has no canonical equivalent).
- [ ] `InstallGameRequest` is extended (Rust + TS) with `runner_method`,
      `steam_app_id`, `custom_portrait_art_path`,
      `custom_background_art_path`, and `working_directory`. All are
      `#[serde(default)]` so older clients keep working.
- [ ] `reviewable_profile()` populates the new fields onto the generated
      `GameProfile` for both `proton_run` and `steam_applaunch` runners.
- [ ] Steam App ID is surfaced for `steam_applaunch` (required) and
      `proton_run` (optional) inside the canonical `RuntimeSection`.
- [ ] Media tab exposes Cover, Portrait, Background art, plus Launcher
      Icon when `launchMethod !== 'native'`.
- [ ] Installer & Review tab exposes the `WizardPresetPicker` (bundled +
      saved) and the `WizardReviewSummary` required-field checklist.
- [ ] Install button respects `evaluateInstallRequiredFields(...).isReady`
      and surfaces a missing-field hint via `aria-describedby` when
      disabled.
- [ ] BR-9 invariant preserved: install flow only persists a profile via
      `persistProfileDraft` inside `ProfileReviewModal`. No tab navigation,
      preset apply, or art import writes to disk.
- [ ] Install-complete auto-open modal carries the merged
      `draftProfile` (art, App ID, runtime override, active preset) into
      the review modal without requiring user re-entry.
- [ ] No regression in route-level scroll behavior — the single scroll
      owner is still `crosshook-install-shell__content`.
- [ ] `InstallGamePanel.tsx` ≤ 600 lines after extracting
      `InstallReviewSummary` and `installValidation` (currently 572).
- [ ] All Rust install tests pass; new tests cover the extended request
      fields and runner-method routing.
- [ ] Tab navigation honors the trainer-skip rule when the user picks
      `native` runner method.

## Completion Checklist

- [ ] Code follows discovered `profile-sections/*` composition pattern.
- [ ] Error handling stays banner-based and non-blocking
      (`crosshook-danger` paragraphs for `error` / `generalError`).
- [ ] Logging follows existing `console.error`-only-on-recoverable-fault
      convention; no new `console.log` statements anywhere.
- [ ] Tests N/A on the TS side (no test framework configured); Rust
      tests added for the new fields.
- [ ] No hardcoded colors/sizes in any new CSS (uses existing
      `--crosshook-*` tokens).
- [ ] Conventional commit prefix:
      `feat(ui): align Install Game flow with profile wizard parity (#162)`.
- [ ] No unnecessary scope additions into Phase 4 (Setup Run EXE/MSI flow)
      or persistence layer.
- [ ] Self-contained — no questions needed during implementation.

## Risks

| Risk                                                                                         | Likelihood | Impact | Mitigation                                                                                                                                                                                                      |
| -------------------------------------------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `useInstallGame` dual-store sync causes ghost edits (request and draftProfile drift apart)   | Medium     | High   | Adopt the **submit-time derive** approach in Task 3 — `request` is built from `draftProfile` + `installerInputs` only at install start. No bidirectional sync. Single source of truth for each field.           |
| `RuntimeSection` width-sensitive grid behaves differently in the install panel vs the editor | Low        | Medium | The wizard already proved Phase 2 reuse works at modal width. The install panel renders inside `crosshook-subtab-content__inner` which is wider than the wizard modal — should be safer, not riskier.           |
| Backend `serde(default)` does not pick up the new fields from old TS clients                 | Low        | Low    | The TS client is part of the same workspace and ships in lockstep. Add the Rust unit test in Task 2 to assert default values resolve cleanly.                                                                   |
| Trainer-skip math breaks when `launchMethod` flips between tabs                              | Medium     | High   | Add a `useEffect` snap from `trainer` → `media` whenever `launchMethod` becomes `'native'`. Mirror the wizard's symmetric forward/back skip logic.                                                              |
| `InstallGamePanel.tsx` exceeds the 800-line repo cap after refactor                          | Low        | Medium | Target ≤ 600 lines. Extracted `InstallReviewSummary` (Task 6) and `installValidation` (Task 5) reclaim ~150 lines from the existing review tab JSX. If still too large, extract per-tab bodies.                 |
| Removing the panel-local `useGameCoverArt` hero regresses install context cues               | Medium     | Low    | Conditional removal in Task 4. Manual smoke test confirms whether `RouteBanner` provides equivalent context. Keep the hero if it aids clarity; otherwise drop it for cleaner alignment with other routes.       |
| Default-prefix auto-resolve writes to the wrong sub-object on launchMethod switch            | Medium     | Medium | The auto-resolve effect explicitly branches on `launchMethod` and writes to `draftProfile.runtime.prefix_path` or `draftProfile.steam.compatdata_path`. Manual edge-case check on flip during typing.           |
| Launch preset application without a persisted profile races with backend assumptions         | Low        | Low    | The install flow's preset picker is **in-memory only** — it sets `draftProfile.launch.active_preset` and never invokes the persisting `applyBundledOptimizationPreset` IPC. The modal Save handles persistence. |
| Custom art merge in `setResult` overwrites user-set art with backend-empty values            | Medium     | Medium | The merge in Task 3 explicitly preserves user-set art when the backend returns empty: `current.game.custom_cover_art_path?.trim() ? current : nextResult`.                                                      |
| Steam Deck (`max-height: 820px`) viewport clips the new 5-tab row                            | Low        | Medium | `crosshook-install-flow-tabs > .crosshook-subtab-row` already wraps; verify with Chrome DevTools simulator during manual validation. Add a `flex-wrap: wrap` modifier in Task 7 if needed.                      |

## Notes

- **Storage boundary classification**: **runtime/UI** plus an additive
  **IPC payload extension** on `InstallGameRequest`. The 5 new fields are
  optional (`#[serde(default)]`) and stay confined to the install IPC
  payload — they do NOT reach any TOML or SQLite schema. The generated
  `GameProfile` already supports these fields (`runtime.steam_app_id`,
  `game.custom_portrait_art_path`, etc.) — Phase 3 just routes them
  through the install path.
- **Persistence/usability impact**:
  - No migration required.
  - Backwards-compatible: old IPC clients keep working, new fields default
    to empty strings and route to no-op branches.
  - Offline behavior unchanged.
  - Degraded fallback: if `evaluateInstallRequiredFields` mismatches the
    backend's `validate_install_request`, the backend's validation message
    surfaces via the existing `mapValidationErrorToField` path. Both gates
    coexist intentionally.
  - User visibility/editability: every new field is visible and editable
    via canonical sections; the user can edit or clear each one before or
    after install.
- This phase deliberately leaves Phase 4 (`#165` Setup Run EXE/MSI) untouched.
  The shared `profile-sections/*` reuse pattern established here is the
  same one Phase 4 will use for the standalone EXE/MSI run flow.
- The `WizardPresetPicker`, `WizardReviewSummary`, `wizardValidation`, and
  `checkBadges` modules from Phase 2 are reused **as-is**. No new wizard-side
  changes are needed for Phase 3.
- Conventional commit prefix for the implementation work:
  `feat(ui): align Install Game flow with profile wizard parity (#162)`.
- After implementation, file the report at
  `.claude/PRPs/reports/ui-standardization-phase-3-report.md` mirroring the
  Phase 2 report structure.
