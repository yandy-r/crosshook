# UI Enhancements — Technical Architecture Research

## Executive Summary

The Profiles page currently funnels all editable form fields through a single collapsed "Advanced" section (`CollapsibleSection` with `defaultOpen={false}`), hiding the entire editing surface by default. The monolithic `ProfileFormSections.tsx` (1144 lines) renders all profile fields inline with complex conditional logic per launch method. The proposed solution decomposes `ProfileFormSections` into focused section components and replaces the single collapsed section with a hybrid sub-tab navigation system using existing CSS primitives (`crosshook-subtab-row`, `crosshook-subtab`) already defined but unused in theme.css. Form state is safely preserved across sub-tabs because `ProfileContext` (app-root level) owns the single `GameProfile` state object.

---

## Current Architecture Analysis

### Component Tree

```
App
  ProfileProvider          <- owns profile CRUD, selection, dirty flag
    ProfileHealthProvider
      PreferencesProvider
        LaunchStateProvider
          Tabs.Root (Radix)
            Sidebar          <- route = 'profiles' | 'launch' | ... (7 routes)
            ContentArea
              ProfilesPage   <- 973 lines
                PageBanner
                CollapsibleSection("Advanced", defaultOpen=false)   <- EVERYTHING hidden
                  ProfileFormSections   <- 1144 lines, ALL form fields
                    ProfileIdentity (name, selector)
                    GameSection (name, path)
                    RunnerMethodSection (launch method select)
                    CustomEnvironmentVariablesSection
                    TrainerSection (conditional: launchMethod !== 'native')
                    RuntimeSection (conditional per launchMethod)
                      AutoPopulate
                      ProtonDbLookupCard
                  HealthIssues (conditional)
                ProfileActions (Save, Delete, Duplicate, Rename, Preview, Export, History)
                LauncherExport (CollapsibleSection, conditional)
              -- modals: delete confirm, rename, preview, history panel, onboarding wizard
```

### State Flow

```
ProfileContext (app root)
  +-- profile: GameProfile          <- single state object
  +-- updateProfile(updater)        <- immutable updater pattern
  +-- dirty: boolean                <- tracks unsaved changes
  +-- saving/loading/deleting       <- operation flags
  +-- selectProfile(name)           <- loads from disk via IPC
  +-- saveProfile()                 <- writes to disk via IPC

ProfilesPage reads from useProfileContext()
  +-- passes props to ProfileFormSections:
        profileName, profile, launchMethod, protonInstalls,
        onProfileNameChange, onUpdateProfile
```

Key: `onUpdateProfile` accepts `(current: GameProfile) => GameProfile`. Every field change produces a new immutable profile object. State lives in context, not in component local state, so sub-tab switching cannot lose data.

### CSS Patterns

- **Panels**: `crosshook-panel` — rounded dark container with border + shadow + blur
- **Cards**: `crosshook-card` — same as panel but with more padding
- **Section titles**: `crosshook-install-section-title` — uppercase eyebrow headings within forms
- **Sub-tabs (unused)**: `crosshook-subtab-row` + `crosshook-subtab` + `crosshook-subtab--active` — pill-shaped buttons in rounded container, defined in `theme.css:104-135` and `variables.css:45-46`
- **Collapsible**: `crosshook-collapsible` — `<details>` element with chevron, title, meta area
- **Controller mode**: `:root[data-crosshook-controller-mode='true']` overrides touch targets, padding, and grid columns

### Critical Pain Points in Current Layout

1. **All form fields hidden by default** — The "Advanced" section's `defaultOpen={false}` means profile name, game path, trainer config, runtime config, and environment variables are ALL invisible until the user clicks to expand.
2. **Actions bar buried** — Save, Delete, Duplicate, Rename buttons live inside the collapsed section, requiring users to expand Advanced before they can perform any profile operation.
3. **Monolithic form component** — `ProfileFormSections.tsx` at 1144 lines handles three launch methods with deeply nested conditional rendering, making it difficult to maintain.
4. **Section boundaries unclear** — Within the expanded Advanced section, sections are separated only by `crosshook-install-section-title` eyebrow headings with no visual container boundaries.

---

## Architecture Design

### Proposed Component Hierarchy

```
ProfilesPage
  PageBanner
  ProfileSelectorBar (always visible)
    +-- ThemedSelect (profile dropdown)
    +-- HealthBadge, OfflineStatusBadge, VersionStatusBadge
    +-- Refresh button
  ProfileSubTabRow (always visible)
    +-- SubTab "General" (default)
    +-- SubTab "Runtime"
    +-- SubTab "Environment"
    +-- SubTab "Health" (conditional: only when issues exist)
  SubTabContent (renders active tab)
    +-- General: ProfileIdentitySection + GameSection + RunnerMethodSection
    +-- Runtime: TrainerSection + RuntimeSection (Steam/Proton/Native)
    +-- Environment: CustomEnvironmentVariablesSection + ProtonDbLookupCard
    +-- Health: HealthSummary + HealthIssuesList
  ProfileActionsBar (always visible, outside sub-tabs)
    +-- Save, Duplicate, Rename, Preview, Export, History, Delete
    +-- Dirty indicator
  LauncherExportPanel (CollapsibleSection, conditional)
  -- modals: delete confirm, rename, preview, history panel, onboarding wizard
```

### New/Modified Components

| Component                | Status        | File Path                                                | Responsibility                            |
| ------------------------ | ------------- | -------------------------------------------------------- | ----------------------------------------- |
| `ProfileSubTabs`         | **New**       | `components/ProfileSubTabs.tsx`                          | Sub-tab row + content routing             |
| `ProfileIdentitySection` | **New**       | `components/profile-sections/ProfileIdentitySection.tsx` | Profile name field                        |
| `GameSection`            | **New**       | `components/profile-sections/GameSection.tsx`            | Game name + executable path               |
| `RunnerMethodSection`    | **New**       | `components/profile-sections/RunnerMethodSection.tsx`    | Launch method selector                    |
| `TrainerSection`         | **New**       | `components/profile-sections/TrainerSection.tsx`         | Trainer path, type, loading mode, version |
| `RuntimeSection`         | **New**       | `components/profile-sections/RuntimeSection.tsx`         | Steam/Proton/Native runtime fields        |
| `FieldRow`               | **Extract**   | `components/ui/FieldRow.tsx`                             | Generic labeled input + browse button     |
| `ProfileFormSections`    | **Modify**    | Keep as thin re-export or remove                         | Backward-compat for OnboardingWizard      |
| `ProfilesPage`           | **Modify**    | Existing file                                            | Add sub-tab state, restructure layout     |
| `ProfileActions`         | **No change** | Existing file                                            | Moved outside sub-tab content area        |

---

## Data Flow Design

### Form State Across Sub-Tabs

```
ProfileContext (persists across ALL tabs)
  |
  +-- General Tab
  |     ProfileIdentitySection <- reads: profileName, profileExists
  |     GameSection            <- reads: profile.game
  |     RunnerMethodSection    <- reads: profile.launch.method
  |     All call: onUpdateProfile((current) => ({ ...current, ... }))
  |
  +-- Runtime Tab
  |     TrainerSection         <- reads: profile.trainer, launchMethod
  |     RuntimeSection         <- reads: profile.steam | profile.runtime
  |     All call: onUpdateProfile((current) => ({ ...current, ... }))
  |
  +-- Environment Tab
  |     CustomEnvVarsSection   <- reads: profile.launch.custom_env_vars
  |     ProtonDbLookupCard     <- reads: profile.steam.app_id
  |     ProtonDB merge logic   <- calls: onUpdateProfile to merge env vars
  |
  +-- Health Tab
        HealthSummary          <- reads from useProfileHealthContext()
        HealthIssuesList       <- reads from useProfileHealthContext()
```

### Validation Across Sections

- **Cross-section validation**: The `canSave` check (`profileName.trim().length > 0 && profile.game.executable_path.trim().length > 0`) spans General tab fields. This check stays in ProfilesPage.
- **Per-field validation**: Custom env var key validation stays in `CustomEnvironmentVariablesSection`. Reserved key checks, duplicate detection unchanged.
- **Health validation**: Profile health is computed by the Rust backend and accessed via `useProfileHealthContext()`. No frontend cross-field validation needed.

### ProtonDB State Management

Currently `pendingProtonDbOverwrite`, `applyingProtonDbGroupId`, and `protonDbStatusMessage` are local state in `ProfileFormSections`. Two options:

**Option A (Recommended)**: Move these states to `RuntimeSection` or the Environment tab component. They are only relevant when the ProtonDB card is visible.

**Option B**: Lift to ProfilesPage. Overkill since these states are only consumed by ProtonDB-related UI.

---

## Navigation Design

### Sub-Tab Routing Approach: Local State (Recommended)

```tsx
// In ProfilesPage
type ProfileSubTab = 'general' | 'runtime' | 'environment' | 'health';
const [activeSubTab, setActiveSubTab] = useState<ProfileSubTab>('general');
```

**Rationale**:

- The app uses Radix Tabs for top-level routing (sidebar). No URL router exists.
- Sub-tabs are purely visual navigation within a single page context.
- Profile form state persists in ProfileContext regardless of which tab is rendered.
- Tab resets to 'general' when navigating away and back — acceptable behavior, matches user expectation.

**Rejected alternatives**:

- URL hash routing: No URL router in the app; adding one is a disproportionate change.
- Nested Radix Tabs: Possible but constrains layout flexibility. Plain buttons with conditional rendering are simpler and equally accessible with proper ARIA.

### Accessibility

```tsx
<div className="crosshook-subtab-row" role="tablist" aria-label="Profile sections">
  <button
    role="tab"
    aria-selected={activeSubTab === 'general'}
    aria-controls="profile-tab-general"
    className={`crosshook-subtab ${activeSubTab === 'general' ? 'crosshook-subtab--active' : ''}`}
    onClick={() => setActiveSubTab('general')}
  >
    General
  </button>
  {/* ... more tabs */}
</div>

<div id="profile-tab-general" role="tabpanel" aria-labelledby="...">
  {activeSubTab === 'general' && <GeneralTabContent />}
</div>
```

### Integration with Sidebar Navigation

No changes to Sidebar.tsx or ContentArea.tsx. The sub-tabs are entirely within ProfilesPage. The sidebar route remains `'profiles'` — sub-tab state is local to the page component.

### Controller Mode / Gamepad

The existing `crosshook-subtab` CSS already respects controller mode variables (`--crosshook-subtab-min-height`, `--crosshook-subtab-padding-inline`). Gamepad focus navigation (`useGamepadNav`) uses `data-crosshook-focus-zone` attributes — the sub-tab row should be annotated with `data-crosshook-focus-zone="subtabs"`.

---

## Component Specifications

### ProfileSubTabs

```tsx
interface ProfileSubTabsProps {
  activeTab: ProfileSubTab;
  onTabChange: (tab: ProfileSubTab) => void;
  showHealthTab: boolean; // only show when health issues exist
}
```

Renders the `crosshook-subtab-row` with tab buttons. Uses existing CSS classes.

### ProfileIdentitySection

```tsx
interface ProfileIdentitySectionProps {
  profileName: string;
  profileExists: boolean;
  onProfileNameChange: (value: string) => void;
  // Optional: profile selector (used by OnboardingWizard)
  profileSelector?: ProfileFormSectionsProfileSelector;
}
```

Extracted from `ProfileFormSections.tsx` lines 665-702.

### GameSection

```tsx
interface GameSectionProps {
  profile: GameProfile;
  launchMethod: LaunchMethod;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 704-737. Includes game name + executable path with browse.

### RunnerMethodSection

```tsx
interface RunnerMethodSectionProps {
  launchMethod: LaunchMethod;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 739-766. Launch method dropdown (steam_applaunch / proton_run / native).

### TrainerSection

```tsx
interface TrainerSectionProps {
  profile: GameProfile;
  launchMethod: LaunchMethod;
  profileName: string;
  profileExists: boolean;
  reviewMode: boolean;
  trainerVersion: string | null;
  onVersionSet?: () => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 775-897. Only rendered when `launchMethod !== 'native'`. Includes trainer path, type selector, loading mode, version display, and manual version set.

### RuntimeSection

```tsx
interface RuntimeSectionProps {
  profile: GameProfile;
  launchMethod: LaunchMethod;
  protonInstalls: ProtonInstallOption[];
  protonInstallsError: string | null;
  reviewMode: boolean;
  profileExists: boolean;
  profileName: string;
  trainerVersion: string | null;
  versionStatus: VersionCorrelationStatus | null;
  onVersionSet?: () => void;
  onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
}
```

Extracted from lines 899-1138. Handles all three launch method variants (steam_applaunch, proton_run, native). Includes ProtonPathField, LauncherMetadataFields, AutoPopulate, and ProtonDbLookupCard.

### FieldRow (extracted to ui/)

```tsx
interface FieldRowProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  helperText?: string;
  error?: string | null;
  browseLabel?: string;
  onBrowse?: () => Promise<void>;
}
```

Currently a local function component inside ProfileFormSections (lines 124-163). Used 10+ times across sections. Should be extracted to `components/ui/FieldRow.tsx` for reuse.

---

## CSS / Styling Strategy

### Existing Classes to Use

| Class                             | Source          | Purpose                              |
| --------------------------------- | --------------- | ------------------------------------ |
| `crosshook-subtab-row`            | `theme.css:104` | Container for sub-tab pills          |
| `crosshook-subtab`                | `theme.css:115` | Individual sub-tab button            |
| `crosshook-subtab--active`        | `theme.css:131` | Active sub-tab with accent gradient  |
| `crosshook-panel`                 | `theme.css:137` | Dark panel container                 |
| `crosshook-install-section-title` | `theme.css`     | Section eyebrow headings within form |
| `crosshook-install-grid`          | `theme.css`     | Grid layout for form fields          |

### New CSS Needed

Minimal — the existing sub-tab classes cover the tab row. May need:

```css
/* Container for sub-tab content to provide consistent padding */
.crosshook-subtab-content {
  padding: var(--crosshook-card-padding);
  /* No border-top since sub-tab-row provides visual separation */
}

/* Sticky actions bar at bottom of profile panel */
.crosshook-profile-actions-bar {
  padding: var(--crosshook-card-padding);
  border-top: 1px solid var(--crosshook-color-border);
  position: sticky;
  bottom: 0;
  background: inherit; /* match panel background for scroll cover */
}
```

### Controller Mode

All existing `crosshook-subtab-*` variables already have controller-mode overrides in `variables.css:87-88`:

```css
:root[data-crosshook-controller-mode='true'] {
  --crosshook-subtab-min-height: 48px;
  --crosshook-subtab-padding-inline: 20px;
}
```

No additional controller-mode CSS changes needed.

---

## Migration Path

### Phase 1 — Extract Shared UI Components (Low Risk)

**Goal**: Reduce `ProfileFormSections.tsx` line count without changing behavior.

1. Extract `FieldRow` to `components/ui/FieldRow.tsx`
2. Reconcile `ProtonPathField` — the local version in ProfileFormSections (lines 166-231) vs the existing `components/ui/ProtonPathField.tsx` file. Consolidate into one.
3. Extract `OptionalSection` to `components/ui/OptionalSection.tsx` (small, ~15 lines)
4. Extract `TrainerVersionSetField` to `components/profile-sections/TrainerVersionSetField.tsx`
5. Extract `LauncherMetadataFields` to `components/profile-sections/LauncherMetadataFields.tsx`

**Verification**: All form fields still render and update profile correctly. `cargo test -p crosshook-core` passes (no backend change). Visual diff: none.

### Phase 2 — Split ProfileFormSections into Section Components (Medium Risk)

**Goal**: Each logical form section becomes its own component file.

1. Create `components/profile-sections/` directory
2. Create `ProfileIdentitySection.tsx`, `GameSection.tsx`, `RunnerMethodSection.tsx`
3. Create `TrainerSection.tsx` (conditional on launch method)
4. Create `RuntimeSection.tsx` (conditional per launch method variant)
5. Reduce `ProfileFormSections.tsx` to a thin composition:

   ```tsx
   export function ProfileFormSections(props) {
     return (
       <div className="crosshook-profile-shell">
         <ProfileIdentitySection {...} />
         <GameSection {...} />
         <RunnerMethodSection {...} />
         <CustomEnvironmentVariablesSection {...} />
         {supportsTrainer && <TrainerSection {...} />}
         <RuntimeSection {...} />
       </div>
     );
   }
   ```

6. Keep `ProfileFormSections` export for backward compatibility (OnboardingWizard uses it in review mode).

**Verification**: Same as Phase 1. No visible change to users.

### Phase 3 — Add Sub-Tab Navigation (Medium Risk)

**Goal**: Replace collapsed "Advanced" section with sub-tab layout.

1. Add `ProfileSubTabs.tsx` component
2. In `ProfilesPage.tsx`:
   - Remove the outer `CollapsibleSection("Advanced")` wrapper
   - Add `useState<ProfileSubTab>('general')` for active tab
   - Render `ProfileSubTabs` row below the profile selector
   - Conditionally render section components based on active tab
   - Move `ProfileActions` OUTSIDE the sub-tab content area (always visible)
3. Profile selector bar, health badges, and actions bar remain always-visible
4. Launcher Export panel stays as a separate `CollapsibleSection` below the main profile panel

**Verification**: All form fields accessible via sub-tabs. Profile save/load/delete still works. Dirty indicator reflects changes from any tab. Health issues visible in Health tab.

### Phase 4 — CSS and Layout Polish (Low Risk)

1. Apply `crosshook-subtab-row` / `crosshook-subtab` / `crosshook-subtab--active` classes
2. Add any needed spacing/container CSS
3. Test controller mode (larger touch targets)
4. Test responsive breakpoints (max-width: 900px, max-height: 820px)

---

## Technical Decisions

### Decision 0: Overall Layout Strategy

Four approaches were evaluated for decluttering the Profiles page Advanced section:

| Approach                                                                                  | Scroll Reduction                | Visual Clarity                                 | Random-Access Editing    | Effort | Recommendation  |
| ----------------------------------------------------------------------------------------- | ------------------------------- | ---------------------------------------------- | ------------------------ | ------ | --------------- |
| **A. Sub-tabs** (replace collapsed section with tabbed sections)                          | High — only active tab rendered | High — each tab is focused                     | Full — click any tab     | Medium | Good            |
| **B. Card-based containers** (each logical group gets its own `crosshook-panel`)          | None — all content visible      | Medium — visual boundaries help but still long | Full — scroll to section | Low    | Complement only |
| **C. Hybrid: promote + sub-tabs** (always-visible essentials + sub-tabs for form content) | High                            | High                                           | Full                     | Medium | **Recommended** |
| **D. Progressive disclosure stepper** (inline step-by-step flow like OnboardingWizard)    | High                            | Medium                                         | Poor — sequential only   | Medium | Rejected        |

**Recommendation**: **Approach C (Hybrid promote + sub-tabs)**. Promote the profile selector, wizard access, health badges, and actions bar to always-visible positions outside the sub-tab content area. Use sub-tabs for the form sections (General, Runtime, Environment, Health). Within each tab, use `CollapsibleSection` for optional/advanced subsections (matching the existing LaunchPage pattern where each concern — Gamescope, MangoHud, Optimizations, Steam Launch Options — is its own `CollapsibleSection` panel).

**Why not pure cards (B)?** Cards alone do not reduce the vertical scroll length — the Advanced section content is too long when all fields are expanded for Steam or Proton launch methods. Cards are a good complement within sub-tabs (visual grouping inside a tab) but insufficient as the sole strategy.

**Why not stepper (D)?** Power users need random access to any field at any time. The `OnboardingWizard` already provides a guided linear flow for first-time setup as a modal overlay — duplicating that pattern inline would confuse the two use cases and block experienced users who want to jump directly to, say, environment variables.

**Why hybrid (C) over pure sub-tabs (A)?** The key insight is that the profile selector, wizard buttons, health badges, and save/delete actions should never be hidden behind a tab boundary. By promoting these to always-visible positions, the sub-tabs only organize the form fields themselves — which is where the clutter actually lives. This matches existing patterns: the profile selector and wizard area already sit above the Advanced section, and the LaunchPage places its profile selector and launch controls outside its collapsible panels.

### Decision 1: Sub-Tab Implementation

| Option                   | Complexity | Accessibility      | Layout Flexibility          | Recommendation       |
| ------------------------ | ---------- | ------------------ | --------------------------- | -------------------- |
| Local useState + buttons | Low        | Manual ARIA needed | High                        | **Recommended**      |
| Nested Radix Tabs        | Medium     | Built-in           | Constrained by Tabs.Content | Viable alternative   |
| URL hash routing         | High       | N/A                | N/A                         | Rejected (no router) |

**Recommendation**: Local `useState` with plain buttons. The app has no URL router; Radix Tabs constrains layout. Manual ARIA attributes (`role="tablist"`, `role="tab"`, `role="tabpanel"`, `aria-selected`, `aria-controls`) are straightforward.

**Note on Radix Tabs**: `@radix-ui/react-tabs` is already a dependency (used for sidebar routing in App.tsx). A nested `Tabs.Root` inside the profile panel would provide built-in keyboard navigation (arrow keys between tabs) and ARIA roles for free. The trade-off is that Radix Tabs enforces a `Tabs.List` + `Tabs.Content` structure that may constrain layout flexibility if the actions bar or health badges need to sit between the tab list and tab content. If accessibility is prioritized over layout flexibility, Radix sub-tabs are a strong alternative.

### Decision 2: Content Rendering Strategy

| Option                                | DOM Weight | State Preservation              | Re-fetch Behavior     |
| ------------------------------------- | ---------- | ------------------------------- | --------------------- |
| Conditional render (unmount inactive) | Light      | Via ProfileContext              | Hooks re-run on mount |
| Hidden render (CSS display:none)      | Heavy      | Component-local state preserved | No re-fetch           |

**Recommendation**: **Conditional render** (unmount inactive tabs). ProfileContext preserves all form state regardless. ProtonDB lookup will re-fetch when the Environment tab mounts, but the hook handles caching. This keeps the DOM minimal.

### Decision 3: Where to Place Actions Bar

| Option                                | Discoverability              | UX              |
| ------------------------------------- | ---------------------------- | --------------- |
| Inside sub-tab content (current)      | Poor — hidden when collapsed | Bad             |
| Fixed below sub-tabs (always visible) | Excellent                    | **Recommended** |
| Floating/sticky at bottom of scroll   | Good                         | Complex CSS     |

**Recommendation**: Actions bar fixed below the sub-tab content area, inside the main panel but outside the sub-tab switching logic. Always visible regardless of which tab is selected.

### Decision 4: OnboardingWizard Compatibility

The `OnboardingWizard` imports and uses `ProfileFormSections` with `reviewMode={true}` and a `profileSelector` prop. Two paths:

| Option                                   | Effort | Risk        |
| ---------------------------------------- | ------ | ----------- |
| Keep ProfileFormSections as thin wrapper | Low    | None        |
| Create separate ReviewFormSections       | Medium | Duplication |

**Recommendation**: Keep `ProfileFormSections` as a thin composition of the new section components. The wizard passes `reviewMode` which collapses optional sections — this behavior transfers naturally to the section components. The wizard's modal overlay operates independently of the sub-tab layout; it renders its own copy of `ProfileFormSections` within the modal, not within the page's sub-tab content area.

---

## Open Questions

1. **Tab naming**: Should the "Runtime" tab be named differently based on launch method (e.g., "Steam Runtime" vs "Proton Runtime" vs "Native Runtime")? The current section title in ProfileFormSections already does this.

2. **Health tab visibility**: Should the Health sub-tab always appear, or only when issues are detected? Showing it always provides consistency but may confuse users with healthy profiles.

3. **Launcher Export placement**: Currently a separate `CollapsibleSection` below the profile panel. Should it become a sub-tab, or stay as a separate panel? It's conditionally shown only for steam_applaunch and proton_run methods.

4. **ProtonDB card placement**: Currently in the Runtime section. Moving it to the Environment tab (alongside custom env vars) makes logical sense since its primary action is merging env vars. But it also reads `steam.app_id` which is a Runtime concept. Which tab grouping is more intuitive?

5. **Wizard setup flow**: The OnboardingWizard guides users through fields linearly. If fields are now in sub-tabs, should the wizard auto-navigate between tabs, or keep its own linear flow (modal overlay)?

6. **Alternative sub-tab groupings**: An alternative grouping of "Profile" / "Trainer" / "Runtime" / "Export" was considered, which collapses launcher metadata and community export into an "Export" tab. This may be worth evaluating against the "General" / "Runtime" / "Environment" / "Health" grouping proposed here — particularly whether export-related fields (launcher name, icon) belong with their associated runtime fields or in a dedicated export tab.
