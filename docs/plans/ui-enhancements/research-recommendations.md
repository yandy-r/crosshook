# UI Enhancements: Recommendations Report

## Executive Summary

The ProfilesPage currently funnels all profile editing into a single collapsed "Advanced" section containing 6-8 form sections, health diagnostics, action buttons, and ProtonDB lookup. This creates a poor discoverability/usability trade-off: new users miss critical fields, and experienced users must expand and scroll through unrelated content. After analyzing the codebase architecture, existing design system, component reuse constraints, conditional rendering patterns, dependency landscape, security implications, and engineering practices, the **recommended approach is a hybrid of promoting key sections (Approach C) with card-based visual separation (Approach A)**, with sub-tabs (Approach B) as a planned Phase 3 upgrade. The design system already contains sub-tab CSS tokens with controller mode overrides, providing strong evidence that sub-tab navigation was part of the original design intent. Overall security risk is **LOW** -- this is a UI-only restructuring with no new attack surface.

## Dependency Analysis

Before evaluating approaches, the API/library research established what's available without new dependencies:

| Library                     | Status                        | Verdict                                                                                                                                                                          |
| --------------------------- | ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@radix-ui/react-tabs`      | Already installed (`^1.1.13`) | Use for sub-tabs. WAI-ARIA compliant, keyboard nav built-in.                                                                                                                     |
| `@radix-ui/react-select`    | Already installed (`^2.2.6`)  | Already used by `ThemedSelect`.                                                                                                                                                  |
| `@radix-ui/react-accordion` | Not installed                 | Could replace native `<details>` in `CollapsibleSection` for animation and `type="multiple"` support. Low-risk single dependency add from same vendor. Not required for Phase 1. |
| shadcn/ui                   | Not installed                 | **Requires Tailwind CSS** -- incompatible with CrossHook's plain CSS variable system. Not recommended.                                                                           |
| Headless UI                 | Not installed                 | Duplicates Radix Tabs capability. Adds ~22kB+ unnecessarily. Not recommended.                                                                                                    |
| MUI / Ant Design / etc.     | Not installed                 | ~500kB+ bundle increase, opinionated theming conflicts with `crosshook-*` CSS. Not recommended.                                                                                  |

**Conclusion**: Zero new dependencies needed for any recommended phase. The only optional future addition is `@radix-ui/react-accordion` if animated section expand/collapse is desired.

## Design Intent Evidence for Sub-Tabs

The `variables.css` design tokens contain explicit sub-tab infrastructure that has never been used in production:

```css
/* Default (variables.css:45-46) */
--crosshook-subtab-min-height: 40px;
--crosshook-subtab-padding-inline: 16px;

/* Controller mode override (variables.css:86-87) */
--crosshook-subtab-min-height: 48px;
--crosshook-subtab-padding-inline: 20px;
```

Additionally, `theme.css:104-135` defines complete `crosshook-subtab-row` and `crosshook-subtab` / `crosshook-subtab--active` CSS classes with pill-shaped styling, gradient active state, and transition animations. This infrastructure was built but never connected to any page component, indicating sub-tabs were always part of the design plan.

## Approach Evaluation

### Approach A: Card-Based Visual Container Separation

**Description**: Break the monolithic Advanced collapsible into distinct `crosshook-panel` / `crosshook-card` containers, one per logical group (Profile Identity, Game, Runner Method, Trainer, Runtime, Environment Variables, ProtonDB, Health).

**Pros**:

- Lowest implementation effort -- existing CSS classes (`crosshook-panel`, `crosshook-card`) already provide glassmorphism styling, borders, and shadows (`theme.css:137-152`)
- No new dependencies or architectural changes
- Preserves the linear form layout that `ProfileFormSections` expects
- No impact on any `ProfileFormSections` consumer (`ProfilesPage` full editor, `InstallPage` review modal, or `OnboardingWizard` type imports)
- Consistent with existing patterns: `LaunchPage` uses multiple `CollapsibleSection` + `crosshook-panel` containers for Gamescope, MangoHud, Launch Optimizations, etc.
- Each card can independently collapse via `CollapsibleSection`
- Practices assessment: "Low complexity, medium value -- do it as a baseline layer under any other option"
- Security: no component unmount risk -- all sections remain in DOM

**Cons**:

- Does not reduce vertical scrolling -- the page becomes visually clearer but physically longer
- No information architecture change -- all sections are still visible on one scroll
- Conditional sections (Trainer, ProtonDB) still create empty gaps depending on launch method

**Effort**: Low (1-2 days). Primarily CSS and JSX restructuring in `ProfilesPage.tsx`.

**Risk**: Low. This is purely additive visual separation.

### Approach B: Sub-Tab Navigation Within Page

**Description**: Replace the collapsed Advanced section with a sub-tab bar (e.g., "Profile | Runtime | Trainer | Tools") where each tab shows only its relevant form sections.

**Pros**:

- Dramatically reduces visible clutter -- only one section group visible at a time
- Design system already fully supports this: `crosshook-subtab-row` and `crosshook-subtab` classes exist in `theme.css:104-135` with active state styling, CSS variables in `variables.css` with controller mode overrides (`48px` min-height, `20px` padding for gamepad)
- Radix UI Tabs is already a project dependency (used in `Sidebar.tsx` and `ContentArea.tsx` via `@radix-ui/react-tabs`) -- do not build a custom tab component, do not add Headless UI or any alternative
- Zero new dependencies required
- Enables future scalability as more features are added per section
- Practices assessment: "Low-medium complexity, highest value -- best first step"
- API research assessment: "RECOMMENDED -- zero cost" given all infrastructure exists

**Cons**:

- **Conditional tab visibility is complex**: The Trainer tab only exists when `launchMethod !== 'native'`. The Runtime tab content varies dramatically by launch method (Steam shows 5+ fields; native shows 1). Empty or sparse tabs feel broken.
- **Breaks cross-section workflows**: ProtonDB recommendations (in Runtime/Tools) apply environment variables (in Runtime/Env Vars). If these are on different tabs, users must switch tabs mid-workflow. Similarly, AutoPopulate fills Steam fields that span the Runtime section.
- **ProfileFormSections reuse conflict**: This component is used in two rendering contexts beyond `ProfilesPage`: `InstallPage.tsx:441` renders it with `reviewMode` inside a `ProfileReviewModal` for compact review after game installation. Embedding tabs inside `ProfileFormSections` would force a tab-based layout into that compact review modal -- wrong UX for a confirmation step. The `OnboardingWizard` only imports the `ProtonInstallOption` type, not the component itself, so it is not directly affected. The correct approach: tabs must live at the `ProfilesPage` level, wrapping `ProfileFormSections` output -- not inside the component.
- **Action bar placement ambiguity**: Save/Delete/Duplicate must remain accessible regardless of active tab. Requires either a sticky footer (new pattern) or duplicating the bar on each tab.
- **State preservation**: Unsaved changes in one tab should be preserved when switching to another. The current single-form approach handles this naturally; tabs could accidentally lose input focus context.
- **Security (W1): Component unmount data loss**: `CustomEnvironmentVariablesSection` buffers env var row edits in local React state. If sub-tab navigation unmounts the component mid-edit, in-progress rows are silently discarded. Must use CSS show/hide (`display: none`) instead of conditional rendering for tab content, or add a `useEffect` cleanup to flush local rows to `ProfileContext` on unmount.

**Effort**: Medium-High (3-5 days). Requires refactoring `ProfileFormSections` into composable section components, adding tab state management, handling conditional tab visibility, and ensuring the action bar remains accessible.

**Risk**: Medium. The conditional visibility and cross-section workflow issues are real usability regressions if not handled carefully.

**Note**: Both practices and API research classify the tab mechanism itself as near-zero complexity since all infrastructure exists. The real effort is in the section extraction from `ProfileFormSections` and the conditional visibility logic -- not in the tab implementation.

### Approach C: Promote Key Sections from Advanced

**Description**: Remove the collapsed "Advanced" `CollapsibleSection` wrapper entirely. Promote the most important sections (Profile Identity, Game, Runner Method) to be always visible at the top level. Keep less-frequently-edited sections (Trainer details, ProtonDB, Environment Variables) in individually collapsible containers below.

**Pros**:

- Directly addresses the core problem: critical fields are hidden behind a click
- Game and Runner Method are mandatory fields that must not live behind a collapsed section (practices assessment)
- Matches how `LaunchPage` already works -- top-level sections with individual collapsibles
- Minimal code change: remove the outer `CollapsibleSection` in `ProfilesPage.tsx:622-751`, restructure the inner content into separate `CollapsibleSection` or `crosshook-panel` blocks
- The wizard, profile selector, and action bar are already outside the Advanced section -- this approach extends that pattern
- Health issues can become a persistent banner or their own card rather than being buried
- Practices assessment: "Should be done regardless"
- Security: preserves existing delete confirmation two-step flow (`confirmDelete` -> `executeDelete`) without modification

**Cons**:

- Page becomes longer (all sections rendered, even if some are collapsed)
- Doesn't introduce new navigational patterns -- power users who want quick access to a specific section still scroll
- "Where does Advanced end?" -- without clear visual grouping, the page may feel like an undifferentiated list

**Effort**: Low (1-2 days). Primarily restructuring JSX in `ProfilesPage.tsx`.

**Risk**: Low. This is the most conservative change with the highest immediate impact.

### Approach D: Creative Alternatives

#### D1: Hybrid Promote + Cards (Recommended for Phase 1)

Combine Approach C (promote from Advanced) with Approach A (card containers):

- Remove the Advanced collapsible wrapper
- Group sections into 3-4 named cards: **Core** (Profile Identity + Game + Runner Method), **Runtime** (Steam/Proton fields + Env Vars + ProtonDB + AutoPopulate), **Trainer** (path + type + version + loading mode), **Diagnostics** (Health Issues + Version Status)
- Each card is a `CollapsibleSection` with `crosshook-panel` styling
- Action bar sits outside all cards in a sticky footer or dedicated bottom card
- Cards that are empty for the current launch method simply don't render (already handled by conditional logic in `ProfileFormSections`)

**Effort**: Low-Medium (2-3 days)
**Risk**: Low

#### D2: Quick Settings Summary Bar

Add a compact summary strip above the form sections showing key profile metadata at a glance (launch method badge, game name, trainer status, health status, ProtonDB rating). Clicking any badge scrolls to or expands the relevant section. This provides discoverability without restructuring the form.

**Effort**: Low (1-2 days)
**Risk**: Low -- additive, no restructuring needed

#### D3: Section Anchor Navigation

Add a sticky section navigation bar (similar to a table-of-contents sidebar or horizontal pill strip) that shows all visible sections and scrolls to the clicked one. Uses `scrollIntoView` -- no tab content switching, just navigation aid.

**Effort**: Low-Medium (2-3 days)
**Risk**: Low -- the existing `healthIssuesRef.current?.scrollIntoView()` pattern in `ProfilesPage.tsx:517` proves this works in the codebase

#### D4: Sub-Tabs as Phase 3

After implementing the hybrid promote + cards approach, add sub-tabs as a follow-up phase. By then, sections will already be cleanly separated into composable card components, making the tab extraction straightforward. The existing Radix Tabs + `crosshook-subtab` CSS makes this zero-dependency. Both practices and API research strongly recommend this as the eventual target state.

**Not recommended**: Full page split into separate routes. High complexity, marginal gain over sub-tabs. Also not recommended: shadcn/ui (requires Tailwind), Headless UI (duplicates Radix), or any full design system (bundle bloat, style conflicts).

## Recommended Approach

### Primary: Hybrid Promote + Cards (D1) followed by Sub-Tabs (D4)

**Rationale**:

1. **Addresses the root cause immediately**: The real problem is that everything is behind a single collapsed "Advanced" toggle. Promoting sections and giving them visual boundaries directly solves this.
2. **Lowest risk first**: No new navigation patterns, no new dependencies, no component reuse conflicts in Phase 1.
3. **Consistent with existing patterns**: `LaunchPage` already uses this exact pattern (multiple `CollapsibleSection` + `crosshook-panel` blocks at the page level).
4. **Preserves `ProfileFormSections` reuse**: Both the `ProfilesPage` full editor and the `InstallPage` review modal continue to work unchanged. Tab-based navigation is layered at the `ProfilesPage` level only in Phase 3, wrapping the linear form output rather than modifying the component itself.
5. **Paves the way for sub-tabs**: Cards become natural tab content containers. The design system's existing sub-tab CSS tokens and controller mode overrides confirm this was always the intended trajectory.
6. **Zero dependency cost**: Both phases use only what's already installed and styled.
7. **Security-safe**: Phase 1 has no component unmount risks. Phase 3 (sub-tabs) must use CSS show/hide for tab content to avoid data loss in buffered components (see Security Constraints below).

### With Additions: Quick Settings Bar (D2) + Sticky Action Footer

- Add a summary bar below the profile selector showing key badges/chips
- Move `ProfileActions` to a sticky bottom bar that's always visible regardless of scroll position

### Section Grouping (Recommended)

Based on the data model (`GameProfile` type in `types/profile.ts`) and conditional rendering logic:

| Card            | Contents                                                                       | Collapsible?                         | Condition                                |
| --------------- | ------------------------------------------------------------------------------ | ------------------------------------ | ---------------------------------------- |
| **Core**        | Profile Identity, Game Name, Game Path, Runner Method                          | No (always open)                     | Always                                   |
| **Runtime**     | Steam/Proton fields, Prefix Path, Proton Path, AutoPopulate, Working Directory | Yes (default open)                   | Always (content varies by launch method) |
| **Environment** | Custom Env Vars, ProtonDB Lookup + Apply                                       | Yes (default open)                   | Always                                   |
| **Trainer**     | Trainer Path, Type, Loading Mode, Version                                      | Yes (default open)                   | `launchMethod !== 'native'`              |
| **Launcher**    | Launcher Name, Launcher Icon                                                   | Yes (default closed)                 | `supportsTrainerLaunch && !reviewMode`   |
| **Diagnostics** | Health Issues, Version Status, Stale Info                                      | Yes (default open when issues exist) | When `selectedReport` has issues         |

**Key grouping decisions**:

- Environment Variables and ProtonDB stay together in one card because ProtonDB apply-env-vars flows directly into the env vars table
- AutoPopulate stays with Runtime because it fills Steam/Proton fields
- Launcher metadata gets its own small card because it's only relevant when exporting (low-frequency action)
- Diagnostics is promoted to a visible card rather than buried inside the form

## Component Callsite Map

`ProfileFormSections` is used in three distinct contexts. Any restructuring must account for all of them:

| Callsite             | File                   | Props                                         | Context                                                                                                                                                                                                     |
| -------------------- | ---------------------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Full editor**      | `ProfilesPage.tsx:675` | No `reviewMode`, no `profileSelector`         | Primary editing UI -- the target of this refactor                                                                                                                                                           |
| **Review modal**     | `InstallPage.tsx:441`  | `reviewMode` set                              | Compact review step inside `ProfileReviewModal` after game installation. Tabs would be wrong UX here.                                                                                                       |
| **Type import only** | `OnboardingWizard.tsx` | N/A (imports `ProtonInstallOption` type only) | Wizard builds its own step-by-step form from individual components (`InstallField`, `ProtonPathField`, `CustomEnvironmentVariablesSection`, `AutoPopulate`). Not affected by `ProfileFormSections` changes. |

**Architectural constraint**: Tabs must live at the `ProfilesPage` level, wrapping `ProfileFormSections` output. They must NOT be embedded inside `ProfileFormSections` itself, because that would force a tabbed layout into the `InstallPage` review modal.

**Addendum from practices research**: Since `OnboardingWizard` already independently imports individual section components (`InstallField`, `ProtonPathField`, `CustomEnvironmentVariablesSection`, `AutoPopulate`), the Phase 3 section extraction still has value -- it gives the wizard a richer set of named, reusable building blocks without coupling it to a tab layout. But this benefit is independent of the tab decision.

## Component Deduplication (Prerequisite Cleanup)

**Critical finding from practices research**: `ProfileFormSections.tsx` contains private helper components that duplicate existing `ui/` components:

| Private component in `ProfileFormSections.tsx` | Existing `ui/` component    | Status                                                                                                                                                                                                                    |
| ---------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `FieldRow` (line 124, 10+ usages)              | `ui/InstallField.tsx`       | Functionally equivalent -- same label + input + browse + helpText + error pattern. `InstallField` has additional `browseMode`/`browseTitle`/`browseFilters` props and integrates `chooseFile`/`chooseDirectory` directly. |
| `ProtonPathField` (line 166)                   | `ui/ProtonPathField.tsx`    | Near-duplicate. The `ui/` version imports `formatProtonInstallLabel` from `ProfileFormSections` (circular-ish dependency). The private version accepts more explicit props (`label`, `onBrowse`).                         |
| `OptionalSection` (line 290)                   | `ui/CollapsibleSection.tsx` | Could be replaced with `CollapsibleSection defaultOpen={false}`. `OptionalSection` uses raw `<details>` with inconsistent inline styles.                                                                                  |

**Recommendation**: Before or during Phase 1, consolidate these:

1. Replace `FieldRow` usages with `InstallField` (or unify into a single shared component)
2. Consolidate the two `ProtonPathField` implementations -- the `ui/` version should be canonical
3. Replace `OptionalSection` with `CollapsibleSection defaultOpen={false}`

This reduces `ProfileFormSections.tsx` by ~100 lines and eliminates the inconsistent inline styles flagged by practices research. It also makes the Phase 3 section extraction cleaner since each section will use shared UI primitives rather than private copies.

## Security Constraints

**Overall security risk: LOW.** This is a UI-only restructuring of an existing native Linux desktop app with no new attack surface. No new Tauri capabilities are needed (current surface: `core:default`, `dialog:default`, `shell:open-url`).

### Must Address (Warnings)

**W1: Component unmount data loss (sub-tabs Phase 3)**

- `CustomEnvironmentVariablesSection` buffers env var row edits in local React state (`useState<CustomEnvVarRow[]>`). If sub-tab navigation unmounts the component mid-edit, in-progress rows are silently discarded.
- **Mitigation (choose one)**:
  - **Preferred**: Use CSS `display: none` instead of conditional rendering for tab panels. All sections remain mounted; only visibility changes. This is the simplest fix with zero state management overhead.
  - **Alternative**: Add a `useEffect` cleanup hook (~5 lines) to flush buffered rows to the parent `onUpdateProfile` callback on unmount.
- **Applies to**: Phase 3 only. Phases 0-2 do not unmount form sections.

**W2: sessionStorage key namespace**

- Any new `sessionStorage` keys (e.g., for sub-tab state persistence) must use the `crosshook.` prefix to avoid collisions across browser contexts.
- Existing keys follow this pattern: `crosshook.healthBannerDismissed`, `crosshook.renameToastDismissed`.
- **Recommended key**: `crosshook.profilesActiveTab`.

### Preserved Security Strengths

The following existing security properties must be preserved through all phases:

- **Profile name path traversal prevention**: enforced in `validate_name()` at `toml_store.rs:497-521`. All filesystem ops gated through this. Not affected by UI restructuring.
- **Env var key validation**: client-side checks for `=`, NUL, reserved keys (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`) -- mirrors backend constraints. Must remain in `CustomEnvironmentVariablesSection` regardless of where it's rendered.
- **Delete confirmation two-step flow**: `confirmDelete` -> `executeDelete` with modal dialog prevents accidental data loss. Must remain accessible in the restructured layout -- not hidden behind a collapsed section or non-obvious tab.
- **No `dangerouslySetInnerHTML`**: Not used anywhere in the current codebase. Must not be introduced.

### Advisory (Non-Blocking Best Practices)

- **A1**: Path inputs (Game Path, Trainer Path) show no client-side validation feedback -- errors only surface on save from backend. Optional: add non-blocking advisory warnings for obviously malformed paths (empty, missing file extension). Backend remains authoritative.
- **A2**: Env var values have no client-side length cap. Optional: soft advisory display for values exceeding ~1000 characters. Not a blocker -- this is user self-inflicted.

## Creative Ideas

### Immediately Actionable (Phase 1)

1. **Profile type indicator**: Show a visual badge next to the profile name indicating "Steam", "Proton", or "Native" with appropriate coloring. Helps users immediately understand which sections are relevant.

2. **Section completion indicators**: Add small checkmarks or progress dots on each card header showing whether required fields are filled. Similar to the health badge pattern already in use.

3. **Sticky action footer**: Move Save/Delete/Duplicate/Rename buttons to a fixed-position footer bar. This solves the problem of scrolling to find the action buttons. The "unsaved changes" indicator becomes a persistent banner.

4. **Smart defaults on card collapse**: When a card is collapsed, show a one-line summary of its current state in the header (e.g., "Trainer: Aurora v1.2 (copy mode)" or "Runtime: Steam App 1245620, Proton Experimental"). Use the existing `CollapsibleSection` `meta` prop which already supports arbitrary ReactNode content.

### Future Consideration (Phase 2+)

5. **Profile templates**: Pre-fill common configurations (e.g., "Steam + Aurora trainer", "Proton + WeMod", "Native Linux"). The `BundledOptimizationPreset` pattern in `types/profile.ts:76-83` already demonstrates this concept for launch optimizations -- extend it to full profiles.

6. **Comparison view**: Side-by-side diff of two profiles (the `ConfigHistoryPanel` and `fetchConfigDiff` already support TOML diff rendering -- reuse for profile-vs-profile comparison).

7. **Conditional section auto-expand**: When the user changes the Runner Method, automatically expand sections that become relevant and collapse those that become irrelevant. This provides contextual guidance without complex smart-settings infrastructure.

8. **Sub-tab state persistence**: If sub-tabs are added in Phase 3, the active tab should persist across page navigation. The `sessionStorage` pattern already exists for banners/toasts (`HEALTH_BANNER_DISMISSED_SESSION_KEY`, `RENAME_TOAST_DISMISSED_SESSION_KEY` in `ProfilesPage.tsx`). Use key `crosshook.profilesActiveTab` per W2.

9. **Radix Accordion upgrade** (optional): Replace the native `<details>` in `CollapsibleSection` with `@radix-ui/react-accordion` for animation support and `type="multiple"` (all sections open simultaneously). Same vendor as existing Radix dependencies, single-package add. Not required -- only if animated expand/collapse is desired.

## Risk Assessment

### Low Risk

- **Card separation (A/C/D1)**: Pure visual restructuring. No logic changes. Easy to revert. No component unmount risk.
- **Sticky action footer**: CSS-only change with potential z-index edge cases.
- **Quick settings bar (D2)**: Additive component. No existing code modified.
- **Component deduplication**: Replacing private helpers with existing `ui/` components. Low risk if done incrementally.

### Medium Risk

- **Sub-tabs (B/D4)**: Conditional tab visibility, cross-section workflow breaks, `InstallPage` review modal reuse conflict. All solvable but require careful design. Risk is lower than initially assessed because all tab infrastructure already exists (Radix Tabs, CSS tokens, controller mode overrides). **Security constraint**: must use CSS show/hide for tab panels to prevent data loss in buffered components (W1). **Architectural constraint**: tabs must live at `ProfilesPage` level, not inside `ProfileFormSections`.
- **Refactoring ProfileFormSections into composable sections**: This component is 1,144 lines with shared internal state (ProtonDB overwrite flow, trainer info modal). Splitting it requires careful state hoisting. Practices research recommends 7 named section components: `ProfileIdentitySection`, `GameSection`, `RunnerMethodSection`, `RuntimeSection`, `TrainerSection`, `(EnvVars already done)`, `LauncherMetadataSection`.

### Higher Risk

- **Drag-and-drop section reordering**: Requires new persistence layer, DnD library, complex state management. Over-engineering for ~15 form fields.
- **Contextual/smart settings**: Requires game metadata infrastructure that doesn't exist. Scope creep risk.
- **Search/filter for settings**: Inappropriate for a form with fewer than 20 fields. Adds cognitive overhead without proportional benefit.
- **shadcn/ui or Tailwind adoption**: Incompatible with existing CSS variable system. Would require complete restyling.
- **Full design system (MUI, Ant Design)**: ~500kB+ bundle bloat, opinionated theming conflicts with `crosshook-*` CSS.

### Cross-Cutting Risks

- **ProfileFormSections multi-consumer compatibility**: Changes to `ProfileFormSections` props or rendering affect both `ProfilesPage` (full editor) and `InstallPage` (review modal with `reviewMode`). The `OnboardingWizard` only imports the `ProtonInstallOption` type, not the component -- but it independently imports shared UI components (`InstallField`, `ProtonPathField`, `CustomEnvironmentVariablesSection`, `AutoPopulate`) that may be affected by deduplication work. Test all three code paths after changes.
- **Keyboard navigation**: The existing `F2` rename shortcut, tab focus management, and `data-crosshook-focus-root` / `data-crosshook-focus-zone` attributes must be preserved. Sub-tabs add another layer of keyboard navigation complexity -- but Radix Tabs handles this natively with arrow key navigation.
- **Controller mode**: The `ControllerPrompts` component suggests gamepad support. The sub-tab CSS tokens already have controller mode overrides (`variables.css:86-87`), which means D-pad navigation was considered in the design. Implementation should verify actual gamepad interaction works.
- **Circular dependency risk**: `ui/ProtonPathField.tsx` imports `formatProtonInstallLabel` from `ProfileFormSections.tsx`. If `ProfileFormSections` is split into section components, this import path must be updated. Consider extracting `formatProtonInstallLabel` to a utility module.
- **Delete confirmation accessibility**: The two-step delete flow (`confirmDelete` -> `executeDelete`) with modal dialog must remain easily accessible in the restructured layout. If sub-tabs are used, the Delete button must be on a persistent action bar, not hidden inside a tab.

## Alternative Approaches

### Alt 1: Settings-Style Two-Column Layout

The `SettingsPanel` uses a two-column grid (`crosshook-settings-grid` with `crosshook-settings-grid-columns: minmax(0, 1fr) minmax(0, 1.1fr)`). The Profiles page could adopt this: left column for Core + Runtime fields, right column for Trainer + Environment + Diagnostics.

**Trade-off**: Better space utilization on wide screens, but responsive design becomes more complex. The existing `--crosshook-layout-main-columns` CSS variable (`minmax(0, 1.3fr) minmax(320px, 0.9fr)`) already defines a two-column layout at the app level -- nesting another two-column grid inside could feel cramped.

### Alt 2: Accordion-Only (No Cards)

Replace the single Advanced collapsible with multiple independent accordions (one per section). No visual card styling -- just expand/collapse toggles. Similar to the current `OptionalSection` pattern in `ProfileFormSections` (using `<details>` elements). Could optionally use `@radix-ui/react-accordion` for animation and `type="multiple"` support.

**Trade-off**: Simplest possible change. But without visual container boundaries, sections blur together when multiple are expanded.

### Alt 3: Modal-Based Editing for Secondary Sections

Keep Core fields always visible. Move Trainer, Environment, and Diagnostics into modal dialogs accessible via buttons. The codebase already has multiple modal patterns (`ProfilePreviewModal`, `ProfileReviewModal`, `OfflineTrainerInfoModal`, `CommunityImportWizardModal`).

**Trade-off**: Reduces page clutter dramatically but introduces modal fatigue. Editing environment variables in a modal (with the ProtonDB apply flow) would be awkward.

## Task Breakdown Preview

### Phase 0: Component Cleanup (Prerequisite) -- Estimated 1 day

1. **Deduplicate FieldRow / InstallField**: Replace `FieldRow` usages in `ProfileFormSections.tsx` with the existing `ui/InstallField.tsx` component (or unify the API if minor prop differences exist).
2. **Consolidate ProtonPathField**: Make `ui/ProtonPathField.tsx` the single canonical implementation. Extract `formatProtonInstallLabel` to a shared utility to break the circular import.
3. **Replace OptionalSection**: Swap `OptionalSection` for `CollapsibleSection defaultOpen={false}` to eliminate inconsistent inline styles.
4. **Verify all consumers**: Test `ProfilesPage` (full editor), `InstallPage` (review modal with `reviewMode`), and `OnboardingWizard` (type imports + independent component imports). Confirm all three code paths still work after deduplication.

### Phase 1: Promote + Cards (D1 Hybrid) -- Estimated 2-3 days

5. **Remove Advanced collapsible wrapper**: Unwrap the `CollapsibleSection` at `ProfilesPage.tsx:622-751`. Move its children to the page level.
6. **Create section cards**: Wrap each logical group in its own `CollapsibleSection` + `crosshook-panel`. Follow the grouping table above.
7. **Extract action bar**: Move `ProfileActions` from inside the former Advanced section to a dedicated bottom area. Consider making it sticky. **Security note**: ensure the Delete button and its two-step confirmation flow remain easily accessible.
8. **Promote Health Issues**: Move the health issues IIFE block (`ProfilesPage.tsx:689-750`) to its own top-level card that renders conditionally.
9. **Test all ProfileFormSections consumers**: Verify `InstallPage` review modal's `reviewMode` rendering and `OnboardingWizard` type/component imports are unaffected.
10. **Verify keyboard/controller navigation**: Ensure F2 rename, tab order, and focus zones work with the new layout.

### Phase 2: Polish + Summary Bar -- Estimated 1-2 days

11. **Add quick settings summary**: Create a compact metadata strip below the profile selector.
12. **Sticky action footer**: CSS positioning for the action bar.
13. **Card header summaries**: Show collapsed-state summaries in card headers using `CollapsibleSection` `meta` prop.
14. **Launch method badges**: Visual indicator for profile type.

### Phase 3: Sub-Tabs (Planned) -- Estimated 3-4 days

15. **Refactor ProfileFormSections into composable section components**: Split the 1,144-line component into `ProfileIdentitySection`, `GameSection`, `RunnerMethodSection`, `TrainerSection`, `RuntimeSection`, `LauncherMetadataSection`. Use the already-extracted `CustomEnvironmentVariablesSection`, `GamescopeConfigPanel`, `MangoHudConfigPanel`, and `LaunchOptimizationsPanel` as the template for how section components should be structured (pure controlled props components).
16. **Add sub-tab navigation at ProfilesPage level**: Use `@radix-ui/react-tabs` (already installed) + existing `crosshook-subtab-row` / `crosshook-subtab` classes. Persist active tab in sessionStorage using key `crosshook.profilesActiveTab`. **Security constraint (W1)**: use CSS `display: none` for inactive tab panels instead of conditional rendering, to prevent data loss in `CustomEnvironmentVariablesSection`'s buffered local state. **Architectural constraint**: tabs wrap `ProfileFormSections` output at the `ProfilesPage` level -- they must NOT be embedded inside `ProfileFormSections` itself, to avoid breaking the `InstallPage` review modal.
17. **Handle conditional tabs**: Tab visibility based on launch method. Decide: disabled vs. hidden for Trainer/Launcher tabs when `launchMethod === 'native'`.
18. **Preserve all consumer reuse**: Ensure composable sections work in tabbed (`ProfilesPage`), linear review (`InstallPage` review modal), and independent import (`OnboardingWizard`) modes.

## Key Decisions Needed

1. **Sticky action footer vs. inline actions**: Should Save/Delete/Rename be in a fixed footer or at the bottom of a scrollable area? Sticky footers are more discoverable but consume permanent screen real estate. Security note: Delete must remain accessible without extra navigation.

2. **Default collapse state for promoted cards**: Should Runtime, Trainer, and Environment cards default to open or closed? Recommendation: all default open for new profiles (no data yet = quick setup), default closed for existing profiles (user is likely editing one specific thing).

3. **Card ordering**: The recommended order (Core > Runtime > Environment > Trainer > Launcher > Diagnostics) follows the typical setup workflow. Should this be configurable? Recommendation: no, not in Phase 1.

4. **Sub-tabs timeline**: Phase 3 should be planned given the design system's existing sub-tab infrastructure. The question is whether to schedule it immediately after Phase 2 or wait for user feedback on the card-based approach.

5. **Scope of ProfileFormSections refactor**: Phase 0 does the component deduplication cleanup in `ProfileFormSections`. Phase 1 changes only `ProfilesPage.tsx` -- the form component continues to render sections linearly, and the page-level code wraps them in cards. Phase 3 splits `ProfileFormSections` into composable section components but layers tabs at `ProfilesPage` only -- the `InstallPage` review modal and wizard continue using the linear form or individual components.

6. **Launcher tab behavior for native profiles**: Should the Launcher tab show a disabled state, or be hidden entirely? Hiding is simpler; disabled state adds accessibility complexity but is natively handled by Radix Tabs.

7. **Sub-tab state persistence**: Should the active sub-tab persist across page navigation via sessionStorage? Recommended yes, following the existing pattern for banner/toast dismissal state. Use namespaced key `crosshook.profilesActiveTab` per W2.

8. **Tab panel rendering strategy**: CSS show/hide (preferred for data safety per W1) vs. conditional rendering (lighter DOM but risks data loss). Recommendation: CSS show/hide.

## Open Questions

1. Is the `OnboardingWizard` staying long-term, or will the card-based layout make it redundant for editing? (It still has value for guided creation, and it independently imports individual components rather than using `ProfileFormSections`.)

2. Are there plans for additional profile sections (e.g., DLL injection config is in the data model at `GameProfile.injection` but not rendered in the current form)? If so, the card-based layout accommodates this better than tabs.

3. Should the Launcher Export section (`ProfilesPage.tsx:801-813`) be absorbed into the profile editor cards, or remain a separate top-level section? It currently lives outside the former Advanced area.

4. The `ProtonDbLookupCard` was recently added (untracked files in git status). Should its placement be finalized before or during this UI refactor?

5. Controller mode implications: The sub-tab CSS tokens already have controller mode overrides. Does the gamepad navigation system need explicit updates beyond what Radix Tabs provides for keyboard/focus management?

6. Is `ui/InstallField.tsx` already the intended replacement for the private `FieldRow`? If yes, why wasn't it used in `ProfileFormSections` -- was this an intentional split for the wizard context, or an oversight?

7. Should `@radix-ui/react-accordion` be added for animated section expand/collapse, or is the native `<details>` element in `CollapsibleSection` sufficient?

## Cross-References

- Practices research: `docs/plans/ui-enhancements/research-practices.md`
- External APIs/libraries: `docs/plans/ui-enhancements/research-external.md`
- Security research: `docs/plans/ui-enhancements/research-security.md`
