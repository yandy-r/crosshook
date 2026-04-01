# Feature Spec: UI Enhancements — Profiles Page Restructuring

## Executive Summary

The Profiles page buries its entire editing surface — profile identity, game path, runner method, trainer, environment variables, ProtonDB lookup, and health diagnostics — inside a single collapsed "Advanced" `<details>` section (`defaultOpen=false`), violating the Nielsen Norman Group two-level disclosure limit and forcing users to click-to-reveal before doing any work. The recommended fix is a phased approach: first remove the Advanced wrapper and separate content into visually distinct `crosshook-panel` cards grouped by user task (Phase 1), then add sub-tab navigation using the already-installed `@radix-ui/react-tabs` and the already-defined but unused `crosshook-subtab-*` CSS classes (Phase 3). Zero new dependencies are required for any phase. The primary constraint is that `ProfileFormSections.tsx` (1,144 lines) is shared with `InstallPage`'s `reviewMode` modal — tabs must live at the `ProfilesPage` level, not inside the shared form component. Security risk is LOW (UI-only restructuring, no new IPC or attack surface); the sole warning-level concern is preserving `CustomEnvironmentVariablesSection`'s buffered local state during tab switches.

## External Dependencies

### Libraries and SDKs

| Library                     | Version       | Status                | Purpose                                                                    |
| --------------------------- | ------------- | --------------------- | -------------------------------------------------------------------------- |
| `@radix-ui/react-tabs`      | ^1.1.13       | **Already installed** | Sub-tab primitives (WAI-ARIA compliant, keyboard nav built-in)             |
| `@radix-ui/react-select`    | ^2.2.6        | **Already installed** | Used by `ThemedSelect` for dropdowns                                       |
| `react-resizable-panels`    | ^4.7.6        | **Already installed** | Panel splits (not directly used but available)                             |
| `@radix-ui/react-accordion` | Not installed | **Optional future**   | Animated section expand/collapse. Same vendor, low-risk add. Not required. |

**Rejected alternatives**: shadcn/ui (requires Tailwind — incompatible), Headless UI (duplicates Radix), MUI/Ant Design (~500kB+ bundle, theming conflicts), Ark UI (overkill when Radix is installed).

**Conclusion**: Zero new dependencies needed for any recommended phase.

### Existing Design System Infrastructure

The codebase already contains unused sub-tab CSS infrastructure that was clearly designed for this use case:

- **`crosshook-subtab-row`** (`theme.css:104`): Pill-shaped flex container with border and muted background
- **`crosshook-subtab`** (`theme.css:115`): Individual tab button with hover/active transitions
- **`crosshook-subtab--active`** (`theme.css:131`): Accent gradient fill + white text
- **`--crosshook-subtab-min-height`** (`variables.css:45`): `40px` default, `48px` controller mode
- **`--crosshook-subtab-padding-inline`** (`variables.css:46`): `16px` default, `20px` controller mode
- **Controller mode responsive override** (`theme.css:3199-3205`): Full-width row, tabs expand to fill

### External Documentation

- [Radix UI Tabs](https://www.radix-ui.com/primitives/docs/components/tabs): Tab primitive API reference
- [Radix UI Accordion](https://www.radix-ui.com/primitives/docs/components/accordion): Optional future accordion upgrade
- [NN/G Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/): Two-level disclosure limit research
- [W3C WAI ARIA Tab Pattern](https://www.w3.org/WAI/ARIA/apg/): Accessibility requirements for tabs

## Business Requirements

### User Stories

**New user creating a first profile**

- As a new user, I want to see the profile editor fields immediately — without hunting for a collapsed "Advanced" section — so I can complete setup without guessing.
- As a user, I want to understand which fields are required vs optional so I can fill only what's needed.

**Returning user editing an existing profile**

- As a power user, I want to jump directly to the section I need (env vars, ProtonDB, trainer) without scrolling through unrelated fields.
- As a returning user, I want to see at a glance which profile is active and whether it is healthy.

**User managing multiple profiles**

- As a user with 10+ profiles, I want Save, Duplicate, Rename, Delete in a consistent, always-visible location.

**User troubleshooting a broken profile**

- As a user whose profile has health issues, I want health status and the issue list surfaced without expanding nested collapsibles.

### Business Rules

1. **Profile Identity and Game Path are always visible**: These are required fields that must not live behind any collapsed section.
2. **Runner Method gates section visibility**: `steam_applaunch` shows Steam fields, `proton_run` shows Proton fields, `native` shows only Working Directory. Sub-tabs labeled by runner method would confuse users — use generic labels.
3. **ProtonDB and Environment Variables stay co-located**: ProtonDB's "Apply" action writes to `launch.custom_env_vars`. Separating them across tabs forces unnecessary tab switching.
4. **Action bar is always visible**: Save, Delete, Duplicate, Rename must never be inside a tab panel that becomes hidden.
5. **Health Issues are diagnostic, not a form section**: Read-only metadata that should be a separate status surface, not embedded in the editable form flow.
6. **`injection.*` fields must not be surfaced**: Present in `GameProfile` but intentionally absent from all form components. Must not be exposed during restructuring.
7. **`ProfileFormSections` is shared**: Used by `ProfilesPage` (full editor) and `InstallPage` (`reviewMode` modal). Tabs must live at `ProfilesPage` level only.
8. **Disclosure capped at one level**: Within any card, at most one nested `CollapsibleSection`. No card should contain a collapse that contains another collapse.

### Edge Cases

| Scenario                                              | Expected Behavior                                         | Notes                                       |
| ----------------------------------------------------- | --------------------------------------------------------- | ------------------------------------------- |
| Native launch method selected                         | Trainer card hidden, Runtime shows only Working Directory | Conditional rendering already exists        |
| New profile (no data)                                 | All cards default open for quick setup                    | Helps first-time flow                       |
| Existing profile selected                             | Cards default open; user can collapse individually        | Preserves editability                       |
| Profile switch with dirty state                       | Confirmation dialog: "You have unsaved changes"           | Existing `dirty` flag in `ProfileContext`   |
| Sub-tab switch within same profile                    | No confirmation needed — state persists in context        | Tab switching is purely visual              |
| `CustomEnvironmentVariablesSection` unmount (Phase 3) | Must use CSS show/hide, not conditional rendering         | Local `rows` state would be lost on unmount |

### Success Criteria

- [ ] A user with no prior CrossHook experience can create a working profile without expanding a collapsible section
- [ ] A user editing an existing profile can see the form without any extra interaction
- [ ] Profile health status is visible at a glance when a profile is selected
- [ ] Users who rarely touch advanced settings are not visually overwhelmed
- [ ] All existing functionality preserved — nothing removed, only reorganized
- [ ] Layout consistent with existing CrossHook design patterns (`crosshook-panel`, `crosshook-*` CSS classes)
- [ ] Keyboard and controller navigation preserved (F2 rename, focus zones, gamepad D-pad)

## Technical Specifications

### Architecture Overview

```
Current:
  ProfilesPage
    └── CollapsibleSection("Advanced", defaultOpen=false)  ← EVERYTHING hidden
          ├── ProfileFormSections (1,144 lines, ALL fields)
          ├── HealthIssues (nested collapsible)
          └── ProfileActions

Proposed (Phase 1 — Cards):
  ProfilesPage
    ├── ProfileSelectorBar (always visible)
    │     ├── ThemedSelect (profile dropdown)
    │     ├── HealthBadge, OfflineStatusBadge, VersionBadge
    │     └── Refresh button
    ├── Panel: Core (always open)
    │     └── ProfileIdentity + Game + RunnerMethod
    ├── Panel: Runtime (collapsible, default open)
    │     └── Steam/Proton/Native fields + AutoPopulate + ProtonDB
    ├── Panel: Environment (collapsible, default open)
    │     └── CustomEnvVars + ProtonDB lookup
    ├── Panel: Trainer (collapsible, default open, conditional)
    │     └── Trainer path/type/loading mode/version
    ├── Panel: Diagnostics (conditional, when issues exist)
    │     └── Health Issues + stale info
    ├── ProfileActionsBar (always visible)
    └── Panel: Launcher Export (existing separate section)

Proposed (Phase 3 — Sub-tabs):
  ProfilesPage
    ├── ProfileSelectorBar (always visible)
    ├── Tabs.Root (nested, horizontal)
    │     ├── Tabs.List: General | Runtime | Environment | Health
    │     ├── Tabs.Content "general": Identity + Game + RunnerMethod
    │     ├── Tabs.Content "runtime": Trainer + RuntimeSection
    │     ├── Tabs.Content "environment": EnvVars + ProtonDB
    │     └── Tabs.Content "health": HealthIssues (conditional)
    ├── ProfileActionsBar (always visible, outside tabs)
    └── Panel: Launcher Export
```

### Data Models

This feature does not change the `GameProfile` data model. It restructures how the same fields are visually presented. The profile state remains a single `GameProfile` object managed by `ProfileContext` at the app root. Sub-tab switching is purely a UI concern with no data model impact.

### State Management

```
ProfileContext (app root, persists across ALL tabs)
  ├── profile: GameProfile          ← single state object
  ├── updateProfile(updater)        ← immutable updater: (current) => GameProfile
  ├── dirty: boolean                ← tracks unsaved changes
  ├── saving/loading/deleting       ← operation flags
  ├── selectProfile(name)           ← loads from disk via IPC
  └── saveProfile()                 ← writes to disk via IPC
```

Key: `onUpdateProfile` accepts `(current: GameProfile) => GameProfile`. State lives in context, not component local state, so sub-tab switching cannot lose data (except for `CustomEnvironmentVariablesSection` local buffer — see W1).

### System Integration

#### Files to Create

- `components/profile-sections/ProfileIdentitySection.tsx`: Profile name field (extracted from ProfileFormSections lines 665-702)
- `components/profile-sections/GameSection.tsx`: Game name + executable path (lines 704-737)
- `components/profile-sections/RunnerMethodSection.tsx`: Launch method selector (lines 739-766)
- `components/profile-sections/TrainerSection.tsx`: Trainer path, type, loading mode, version (lines 775-897)
- `components/profile-sections/RuntimeSection.tsx`: Steam/Proton/Native runtime fields (lines 899-1138)
- `components/profile-sections/LauncherMetadataSection.tsx`: Launcher name + icon (extracted from LauncherMetadataFields)
- `components/ProfileSubTabs.tsx`: Sub-tab row + content routing (Phase 3)

#### Files to Modify

- `components/pages/ProfilesPage.tsx`: Remove Advanced wrapper, add card layout (Phase 1), add sub-tab state (Phase 3)
- `components/ProfileFormSections.tsx`: Reduce to thin composition of extracted section components; keep as export for InstallPage reviewMode compatibility
- `components/ui/InstallField.tsx`: Add `id` prop to replace private `FieldRow`

#### Files to Delete (After Migration)

- Private `FieldRow` in ProfileFormSections (replaced by `InstallField`)
- Private `OptionalSection` in ProfileFormSections (replaced by `CollapsibleSection defaultOpen={false}`)
- Private `ProtonPathField` in ProfileFormSections (consolidated with `ui/ProtonPathField.tsx`)

#### Prerequisite: Circular Dependency Fix

`ui/ProtonPathField.tsx` imports `formatProtonInstallLabel` from `ProfileFormSections.tsx`. Extract `formatProtonInstallLabel` to a shared utility (e.g., `utils/proton.ts`) before splitting ProfileFormSections.

## UX Considerations

### User Workflows

#### Primary Workflow: Create a New Profile

1. **Open Profiles page** — user sees PageBanner, profile selector, and all section cards (no collapse needed)
2. **Enter profile name** — in Core card (always visible)
3. **Set Game Path** — browse button in Core card
4. **Select Runner Method** — dropdown in Core card; Runtime card content updates accordingly
5. **Fill runtime fields** — Steam App ID, Proton path, etc. in Runtime card
6. **Optionally configure Trainer** — in Trainer card (collapsible, default open for non-native)
7. **Click Save** — in always-visible Actions bar

#### Primary Workflow: Edit Existing Profile

1. **Select profile** from dropdown (always visible)
2. **Health status visible** immediately in selector bar badges
3. **Navigate to relevant section** — scroll to card or click sub-tab (Phase 3)
4. **Edit fields, Save** — Actions bar always visible

#### Error Recovery: ProtonDB Apply Conflict

1. **Open ProtonDB card** in Environment section
2. **Click Apply** on recommendation group
3. **Resolve conflicts inline** — per-key conflict resolution UI
4. **Verify in env vars table** — co-located in same section/tab
5. **Save**

### UI Patterns

| Component          | Pattern                                           | Notes                                    |
| ------------------ | ------------------------------------------------- | ---------------------------------------- |
| Section cards      | `CollapsibleSection` + `crosshook-panel`          | Matches existing LaunchPage pattern      |
| Sub-tabs (Phase 3) | `@radix-ui/react-tabs` + `crosshook-subtab-*` CSS | Nested Tabs.Root, horizontal orientation |
| Actions bar        | Fixed footer below tab content                    | Always visible, sticky optional          |
| Health badges      | Inline in profile selector bar                    | Promoted from Advanced header meta       |
| Dirty indicator    | Per-section visual cue (colored border or dot)    | Prevents missed unsaved changes          |

### Accessibility Requirements

- **Tab pattern**: `role="tablist"`, `role="tab"` with `aria-selected`, `role="tabpanel"` with `aria-labelledby`
- **Keyboard**: Arrow keys navigate tabs, Tab moves into panel, Home/End jump to first/last tab
- **Focus indicators**: Visible on all interactive elements, 3:1 contrast minimum
- **Touch targets**: 44x44px minimum (controller mode: 48px via existing CSS variables)
- **Controller mode**: `data-crosshook-focus-zone="subtabs"` for gamepad navigation

### Performance UX

- **Loading States**: Only ProtonDB lookup card needs a loading spinner (external API). All other fields render instantly from local state.
- **Tab Switching**: Instantaneous — no lazy loading needed. Form has bounded, predictable field count.
- **Rendering Strategy (Phase 3)**: CSS `display: none` for inactive tab panels (not conditional rendering) to preserve `CustomEnvironmentVariablesSection` local state.

## Recommendations

### Implementation Approach

**Recommended Strategy**: Hybrid Promote + Cards (Phase 1) followed by Sub-Tabs (Phase 3), with prerequisite component cleanup (Phase 0).

**Rationale**:

1. Addresses root cause immediately — everything is behind a single collapsed toggle
2. Lowest risk first — no new navigation patterns or dependencies in Phase 1
3. Consistent with existing patterns — `LaunchPage` already uses this exact pattern
4. Preserves `ProfileFormSections` reuse — wizard continues to work unchanged
5. Paves the way for sub-tabs — cards become natural tab content containers
6. Zero dependency cost — uses only what's already installed and styled

**Phasing**:

1. **Phase 0 - Component Cleanup** (~1 day): Deduplicate FieldRow/InstallField, consolidate ProtonPathField, replace OptionalSection, extract formatProtonInstallLabel to shared utility
2. **Phase 1 - Promote + Cards** (~2-3 days): Remove Advanced wrapper, create section cards, extract action bar, promote Health Issues
3. **Phase 2 - Polish** (~1-2 days): Quick settings summary bar, sticky action footer, card header summaries, launch method badges
4. **Phase 3 - Sub-Tabs** (~3-4 days): Split ProfileFormSections into composable section components, add sub-tab navigation with existing Radix Tabs + CSS

### Technology Decisions

| Decision              | Recommendation                                     | Rationale                                                                   |
| --------------------- | -------------------------------------------------- | --------------------------------------------------------------------------- |
| Tab library           | `@radix-ui/react-tabs` (already installed)         | Zero dependency cost, WAI-ARIA compliant, matches existing codebase pattern |
| Tab routing           | Local `useState<ProfileSubTab>`                    | No URL router exists; sub-tabs are purely visual navigation                 |
| Content rendering     | CSS `display: none` for inactive panels            | Preserves component local state (W1 mitigation)                             |
| Section containers    | `crosshook-panel` CSS class                        | Matches existing LaunchPage pattern — glassmorphism borders/shadows         |
| Sub-tab styling       | Existing `crosshook-subtab-*` CSS classes          | Already defined with controller mode overrides — zero CSS work              |
| Action bar            | Fixed below sub-tab content area                   | Always visible regardless of active tab                                     |
| Tab state persistence | `sessionStorage` key `crosshook.profilesActiveTab` | Follows existing pattern for banner/toast dismissal state                   |

### Quick Wins

- **Remove the Advanced wrapper**: Single biggest impact change — promotes all content to always-visible
- **Move ProfileActions outside any collapsible**: Save/Delete always accessible
- **Promote health badges to profile selector bar**: Health status visible at a glance

### Future Enhancements

- **Profile templates**: Pre-fill common configurations (extends existing `BundledOptimizationPreset` pattern)
- **Inline field search**: VS Code-style search within settings for power users
- **Profile comparison view**: Side-by-side diff (reuses existing `ConfigHistoryPanel` TOML diff rendering)
- **Conditional section auto-expand**: Auto-expand/collapse sections when Runner Method changes
- **`@radix-ui/react-accordion` upgrade**: Replace native `<details>` for animated expand/collapse (same vendor, low-risk)

### Creative Ideas

1. **Smart defaults on card collapse**: Show one-line summary in header when collapsed (e.g., "Trainer: Aurora v1.2 (copy mode)") using existing `CollapsibleSection` `meta` prop
2. **"Required for Launch" visual treatment**: Red asterisk for required fields, gray "Optional" label, blue "Recommended" chip for ProtonDB suggestions
3. **Inline health annotations**: Annotate specific fields with health issues inline (red border + error message) instead of separate Health Issues section
4. **Split-pane layout**: Profile list on left (~250px), editor on right — mirrors VS Code/JetBrains settings pattern (significant change, consider for v2)

## Risk Assessment

### Technical Risks

| Risk                                               | Likelihood | Impact | Mitigation                                                                               |
| -------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------- |
| `ProfileFormSections` reuse breakage (InstallPage) | Medium     | High   | Phase 1 changes only ProfilesPage; Phase 3 keeps ProfileFormSections as thin composition |
| Component state loss on tab switch (W1)            | Medium     | Medium | Use CSS `display: none` instead of conditional rendering                                 |
| `ProtonInstallOption` type import breaks           | Low        | Medium | Re-export from `src/types/index.ts` when ProfileFormSections is split                    |
| Circular dependency (`formatProtonInstallLabel`)   | High       | Low    | Extract to shared utility in Phase 0                                                     |
| Controller mode regression                         | Low        | Medium | Sub-tab CSS already has controller mode overrides; verify with gamepad testing           |
| OnboardingWizard compatibility                     | Low        | Medium | Wizard imports only `ProtonInstallOption` type; test both code paths                     |

### Integration Challenges

- **ProtonDB apply-to-env-vars cross-section flow**: Must keep ProtonDB and env vars co-located (same card in Phase 1, same tab in Phase 3)
- **Conditional tab visibility**: Trainer tab hidden for native profiles; Runtime tab content varies by launch method. Use generic labels, hide empty tabs.
- **`FieldRow`/`InstallField` consolidation**: Minor API differences to reconcile (browseMode vs onBrowse, id prop)

### Security Considerations

#### Critical -- Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings -- Must Address

| Finding                                                             | Risk                                                           | Mitigation                                                                                           | Alternatives                                 |
| ------------------------------------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| W1: `CustomEnvironmentVariablesSection` local state loss on unmount | In-progress env var edits silently discarded during tab switch | CSS `display: none` for tab panels                                                                   | `useEffect` cleanup to flush rows on unmount |
| W3: `injection.*` fields must not be surfaced                       | Exposes removed DLL injection capability                       | Explicitly exclude `injection.*` from any new form sections; no generic "render all fields" patterns | —                                            |

#### Advisories -- Best Practices

- **A1**: Path inputs show no client-side traversal feedback — optional inline advisory for malformed paths (deferral: backend is authoritative)
- **A2**: Env var values have no length limit — optional soft character limit advisory (deferral: self-inflicted user action)

## Task Breakdown Preview

### Phase 0: Component Cleanup (Prerequisite)

**Focus**: Reduce ProfileFormSections line count and eliminate inconsistencies before restructuring
**Tasks**:

- Deduplicate `FieldRow` → `InstallField` (add `id` prop to InstallField, migrate 10+ usages)
- Consolidate `ProtonPathField` implementations (make `ui/ProtonPathField.tsx` canonical)
- Extract `formatProtonInstallLabel` to `utils/proton.ts` (fix circular import)
- Replace `OptionalSection` with `CollapsibleSection defaultOpen={false}` (eliminate inline styles)
- Verify OnboardingWizard and InstallPage imports still resolve

**Parallelization**: FieldRow migration and ProtonPathField consolidation can run in parallel

### Phase 1: Promote + Cards

**Focus**: Remove Advanced wrapper, create visually distinct section cards
**Dependencies**: Phase 0 complete
**Tasks**:

- Remove `CollapsibleSection("Advanced")` wrapper in `ProfilesPage.tsx:622-751`
- Wrap each logical group in `CollapsibleSection` + `crosshook-panel`
- Move `ProfileActions` to dedicated bottom area (outside any card)
- Promote health badges to profile selector bar
- Move Health Issues to dedicated diagnostic card
- Test OnboardingWizard reviewMode, keyboard nav, controller mode

### Phase 2: Polish + Summary Bar

**Focus**: UX refinements and discoverability improvements
**Dependencies**: Phase 1 complete
**Tasks**:

- Add quick settings summary strip below profile selector
- Implement sticky action footer CSS
- Add card header collapse summaries via `CollapsibleSection` `meta` prop
- Add launch method type badges

### Phase 3: Sub-Tabs

**Focus**: Split monolith into composable sections, add tab navigation
**Dependencies**: Phase 1 complete (Phase 2 optional)
**Tasks**:

- Create `components/profile-sections/` directory with 6 section components
- Reduce `ProfileFormSections` to thin composition of sections
- Add `ProfileSubTabs` component using `@radix-ui/react-tabs` + `crosshook-subtab-*` CSS
- Implement tab state (`useState<ProfileSubTab>`) in ProfilesPage
- Handle conditional tab visibility (Trainer hidden for native)
- Use CSS `display: none` for inactive tab panels (W1 mitigation)
- Persist active tab in `sessionStorage` (key: `crosshook.profilesActiveTab`)
- Verify wizard and InstallPage compatibility

**Parallelization**: Section component extraction can be parallelized across 3-4 agents

## Decisions Needed

1. **Sticky action footer vs. inline actions**
   - Options: Fixed footer (always visible) vs. bottom of scrollable area
   - Impact: Sticky consumes permanent screen real estate; inline requires scrolling to Save
   - Recommendation: Sticky footer — Save/Delete discoverability is more important

2. **Default collapse state for cards**
   - Options: All open (immediate visibility) vs. smart defaults (open for new, collapsed for existing)
   - Impact: All-open may overwhelm; smart defaults add complexity
   - Recommendation: All default open — the whole point is removing hidden content

3. **Sub-tabs timeline**
   - Options: Schedule immediately after Phase 2 vs. wait for user feedback on cards
   - Impact: Cards alone may be sufficient; sub-tabs add complexity but reduce scrolling
   - Recommendation: Plan Phase 3 but gate on user feedback after Phase 1 ships

4. **Launcher tab behavior for native profiles**
   - Options: Hidden entirely vs. disabled state
   - Impact: Hidden is simpler; disabled adds accessibility complexity
   - Recommendation: Hidden — Radix Tabs handles dynamic tab removal cleanly

5. **Tab panel rendering strategy**
   - Options: CSS `display: none` (preserves state) vs. conditional rendering (lighter DOM)
   - Impact: CSS approach keeps all panels mounted; conditional unmounts inactive panels
   - Recommendation: CSS `display: none` — required by W1 to prevent data loss

6. **ProtonDB card placement in sub-tabs**
   - Options: Environment tab (co-located with env vars) vs. Runtime tab (co-located with App ID)
   - Impact: ProtonDB reads `steam.app_id` (Runtime) but writes to `custom_env_vars` (Environment)
   - Recommendation: Environment tab — the write target is more important than the read source

## Persistence & Usability

- **Tab state**: Runtime-only (`useState` in ProfilesPage). Optional `sessionStorage` persistence per session.
- **Card collapse state**: Runtime-only. No persistence needed — cards default open on page load.
- **Profile data**: No changes to TOML storage. All existing persistence mechanisms unchanged.
- **Offline expectations**: No change — UI restructuring has no network dependencies beyond existing ProtonDB lookup.
- **Migration/backward compatibility**: No data migration needed. UI-only change. `ProfileFormSections` retained as thin wrapper for InstallPage reviewMode.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): UI library evaluation, Radix Tabs API, code examples
- [research-business.md](./research-business.md): Section inventory, user workflows, grouping proposals, implementation constraints
- [research-technical.md](./research-technical.md): Component hierarchy, state flow, migration path, component specifications
- [research-ux.md](./research-ux.md): Progressive disclosure research, competitive analysis, accessibility requirements
- [research-security.md](./research-security.md): Severity-leveled findings, state management security, input validation
- [research-practices.md](./research-practices.md): Modularity design, KISS assessment, build-vs-depend, existing reusable code
- [research-recommendations.md](./research-recommendations.md): Approach evaluation, creative ideas, phased task breakdown, risk assessment
