# UX Research: Profiles Page UI Enhancement

**Date**: 2026-03-31
**Feature**: Advanced section decluttering — Profiles page in CrossHook

---

## Executive Summary

The CrossHook Profiles page currently collapses most of its configuration into a single "Advanced" `<details>` section. This one-section-hides-everything pattern is a known UX anti-pattern: it sacrifices discoverability for simplicity without offering any meaningful structural guidance about what settings are available or which ones matter for a typical workflow.

Research across game launchers (Heroic Games Launcher, Lutris, Bottles), IDEs (VS Code, JetBrains), and system settings (macOS Ventura, KDE Plasma) consistently points to the same solution space: **task-oriented grouping with clear visual containers and at most two levels of disclosure**. The specific mechanism (sub-tabs, cards, or promoted sections) is secondary to correct grouping.

**Strongest recommendation**: Split the current "Advanced" section into 3–4 named, visually-bounded containers that are always visible (not collapsed), organized around the mental task each group serves ("Who am I?", "How do I run?", "What trainer?", "How do I appear?"). Promote health/diagnostic content to a persistent inline badge strip rather than hiding it inside the Advanced collapse.

**Updated (post-api-researcher findings)**: The codebase already contains a fully styled, production-ready sub-tab system (`crosshook-subtab-row` / `crosshook-subtab` / `crosshook-subtab--active` classes in `theme.css`) with controller-mode responsive overrides, and `@radix-ui/react-tabs` (`^1.1.13`) is already installed. This materially changes the implementation cost of the sub-tab option — it is now as low-cost as cards. The recommendation shifts to: **use the existing sub-tab infrastructure for the runner-method-dependent fields** (the section whose content changes most dramatically between steam/proton/native), while using always-visible cards for stable sections (Trainer, Environment). This hybrid gives tab-switching benefits exactly where the content diverges, without hiding stable fields.

**Confidence**: High — supported by multiple authoritative sources, direct code inspection, and confirmed library availability.

---

## 1. Current State Inventory

From reading `ProfileFormSections.tsx` and `ProfilesPage.tsx`, the Profiles page structure is:

```
[Page Banner]
[Panel]
  ├── Guided Setup (always visible, accent background)
  ├── Active Profile selector (always visible, only when profiles > 0)
  └── [CollapsibleSection: "Advanced"] (collapsed by default)
        ├── Health/stale summary row
        ├── ProfileFormSections
        │     ├── Section: Profile Identity (profile name + selector)
        │     ├── Section: Game (name, path)
        │     ├── Section: Runner Method (select)
        │     ├── CustomEnvironmentVariablesSection
        │     ├── Section: Trainer (path, type, loading mode, version) — OptionalSection
        │     ├── Section: Steam/Proton/Native Runtime
        │     │     ├── steam_applaunch: App ID, Prefix Path, Launcher Name/Icon,
        │     │     │   Proton Path, AutoPopulate, ProtonDbLookupCard
        │     │     ├── proton_run: Prefix Path, App ID, Launcher Name/Icon,
        │     │     │   Proton Path, Working Dir (OptionalSection), ProtonDbLookupCard
        │     │     └── native: Working Dir (OptionalSection)
        │     └── [nested CollapsibleSection: "Health Issues"] (when profile is broken/stale)
        └── ProfileActions (save, delete, duplicate, rename, preview, export, history)
[Panel: Launcher Export] (CollapsibleSection, when supportsLauncherExport)
```

**Identified problems:**

1. Everything except wizard + profile selector is hidden in a single "Advanced" collapse (`ProfilesPage.tsx:622`, `defaultOpen=false`).
2. Health badges and status chips are in the collapse header `meta` — they exist but users must notice them to understand something is wrong.
3. Profile identity fields (name, selector) appear inside "Advanced" even though they are the primary entry point.
4. Trainer section (`ProfileFormSections.tsx:778`) and working directory (`ProfileFormSections.tsx:1055, 1111`) are `OptionalSection` (`<details>`) — but `trainerCollapsed`/`workingDirectoryCollapsed` are `false` in the main editor when fields are non-empty. These collapses are NOT a primary problem in normal editing mode; they are a nesting violation only in `reviewMode` (InstallPage). The real nesting issue in the main editor is the Health Issues section.
5. Health Issues is a nested `CollapsibleSection` inside Advanced (`ProfilesPage.tsx:709`) — this is the concrete two-level collapse in the main editor: Advanced (level 1) → Health Issues (level 2).
6. ProtonDB lookup, environment variables, and launcher export all live at the same visual priority level with no hierarchy.

---

## 1b. Existing Infrastructure (Post-Research Update)

Confirmed via direct inspection after receiving findings from api-researcher:

**`@radix-ui/react-tabs` is already installed** (`package.json`: `^1.1.13`). Full WAI-ARIA Tabs pattern compliance out of the box, including:

- `orientation="vertical"` for sidebar-style sub-nav
- `activationMode="manual"` option (require Enter/Space instead of auto-activate on arrow — useful for complex panels)
- Data attributes for styling: `data-state="active"`, `data-disabled`, `data-orientation`

**Sub-tab CSS is already defined** in `src/styles/theme.css:104–135` and `src/styles/variables.css:45–47, 86–87`:

- `.crosshook-subtab-row`: pill-shaped container (`border-radius: 999px`, subtle border + background tint)
- `.crosshook-subtab`: individual tab button with hover/active transitions
- `.crosshook-subtab--active`: blue gradient fill (`--crosshook-color-accent` to `--crosshook-color-accent-strong`)
- Controller/responsive override (`theme.css:3199–3205`): full-width row, tabs expand to fill (`flex: 1 1 0`)
- CSS variables: `--crosshook-subtab-min-height: 40px` (standard) / `48px` (controller mode), `--crosshook-subtab-padding-inline: 16px` / `20px` (controller mode)

**Implication**: The sub-tab option is not a new design — it is an already-designed component waiting to be applied to the Profiles page. Implementation cost is equivalent to using named section cards. The runner-method selector (Steam app launch / Proton runtime / Native) is a natural match for sub-tabs because it already changes which fields are visible — a tab strip makes that branching explicit and navigable rather than implicit and collapsed.

---

## User Workflows

### 2.1 Primary Flow: Edit an Existing Profile

1. User opens Profiles page.
2. User selects a profile from the dropdown (Active Profile — currently always visible, good).
3. User wants to change a setting — must expand "Advanced" to reach any field.
4. User edits fields, saves.
5. User may then go to Launcher Export (already a separate panel, good).

**Pain point**: Step 3 is a mandatory detour through a collapsed section that has no visible preview of what's inside.

### 2.2 Primary Flow: Create a New Profile

1. User sees the wizard (always visible), clicks "New Profile".
2. Wizard guides through fields step by step.
3. After wizard completes, user lands back on the manual form inside "Advanced" to make tweaks.

**Pain point**: After wizard exit, the detailed form is collapsed. The user must remember to expand "Advanced" to verify fields.

### 2.3 Advanced User Flow: Tune Environment Variables + ProtonDB

1. User selects profile.
2. User opens "Advanced".
3. User scrolls past Profile Identity, Game, Runner Method sections to find env vars.
4. User opens ProtonDB card inside the Steam Runtime section — may not find it easily.

**Pain point**: Multiple levels of nesting before reaching the relevant feature.

### 2.4 Health Monitoring Flow

1. Health badges appear in the "Advanced" collapse header — visible without expanding.
2. User must expand "Advanced" then scroll to see Health Issues panel.
3. Health issues trigger a top-of-page banner for broken profiles — good, but separate from the inline context.

**Pain point**: Health information is split between the banner (top), the collapse header (meta badges), and the nested CollapsibleSection ("Health Issues") inside the form.

---

## 3. UI/UX Best Practices

### 3.1 Progressive Disclosure — What the Research Says

Source: [Nielsen Norman Group — Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/), [LogRocket — Progressive Disclosure Types](https://blog.logrocket.com/ux-design/progressive-disclosure-ux-types-use-cases/)

**Key findings:**

- Progressive disclosure improves learnability, expert efficiency, and error reduction — but only when the split is done correctly.
- The most important requirement: **everything users frequently need must appear upfront**. If the split is wrong, progressive disclosure becomes a hindrance.
- **Never exceed two disclosure levels.** Designs with three or more levels of nesting show poor usability as users lose context.
- Use task analysis or frequency-of-use data to determine which features belong at each level. Intuition alone is unreliable.
- "Staged disclosure" (wizards) is a separate pattern — it works for sequential onboarding, not for ongoing configuration.

**Implication for CrossHook**: The current design violates the "two levels" rule (Advanced > OptionalSection > nested CollapsibleSection). Profile Identity fields are not advanced — they should always be visible or at minimum in the first-visible layer.

### 3.2 Tabs — When and How

Source: [Eleken — Tabs UX](https://www.eleken.co/blog-posts/tabs-ux), [LogRocket — Tabs UX Best Practices](https://blog.logrocket.com/ux-design/tabs-ux-best-practices/)

**When tabs are appropriate:**

- Grouping genuinely categorical (not sequential) content.
- Users do not need to compare content across tabs simultaneously.
- 3–5 tabs maximum; more indicates poor information architecture.
- Content is self-contained within each tab.

**Tab design specifics:**

- Labels: short, descriptive nouns. Avoid "Info", "More", "Advanced".
- Active state: bold font + underline or color change, never color alone (accessibility).
- Keyboard: Tab moves focus to tablist; Arrow keys navigate between tabs; Space/Enter activates.
- ARIA: `role="tablist"`, `role="tab"`, `role="tabpanel"`, `aria-selected="true"` on active tab.
- Fixed tabs (all visible) for 4 or fewer options; scrollable only when necessary.

**Sub-tabs (horizontal within a main page):**

- Acceptable for a single additional depth level; do not nest sub-tabs within sub-tabs.
- Alternative to sub-tabs: card-based grouping with section headers (often cleaner for settings pages).

**When to avoid tabs and use alternatives:**

- Accordions: better for hierarchical content where headings provide structure clues.
- Sidebars/vertical tabs: better when there are 6+ categories.
- Segmented controls: better for minimalist 2–3 state switches where categories are closely related.

### 3.3 Card-Based Layouts for Settings Grouping

Source: [Design Shack — Card Layouts Modern UX](https://designshack.net/articles/ux-design/card-layouts-modern-ux/), [Mockplus — Card UI Design Best Practices](https://www.mockplus.com/blog/post/card-ui-design)

Cards create visual containment that:

- Communicates that fields inside are related.
- Allows users to quickly scan the page structure.
- Provides clear whitespace separations between sections.

**In the context of settings pages**, cards (visual panels with a section heading, optional border, background tint) are often more appropriate than tabs because:

- Users can see multiple sections simultaneously.
- Scrolling preserves spatial memory better than tab switching.
- No "what tab was that field on?" disorientation.

This is exactly the existing pattern CrossHook uses for the Profile/Launcher Export split — two distinct panels on the page. The recommendation is to bring this same pattern inside the current monolithic "Advanced" section.

### 3.4 Basic vs. Advanced — Categorization Heuristics

Source: [NN/G — Mental Models](https://www.nngroup.com/articles/mental-models/), [LogRocket — Information Architecture](https://blog.logrocket.com/ux-design/organizing-categorizing-content-information-architecture/)

The decision of what is "basic" versus "advanced" should be driven by:

1. **Frequency of use**: Fields changed on most profiles should always be visible.
2. **Consequence of wrong values**: High-consequence fields (game path, runner method) should be visible and clearly labeled.
3. **Task-dependency**: Fields only relevant after a certain condition (e.g., Launcher Name only relevant when exporting a launcher) can be conditionally disclosed.

**The existing code already implements conditional disclosure** via `showLauncherMetadata`, `supportsTrainerLaunch`, `showProtonDbLookup` — these fields only render when the runner method warrants them. This is correct.

**What is wrong**: Wrapping all of this in a single "Advanced" section label with no structure signals to users that they never need to look inside unless they are "advanced".

---

## 4. Competitive Analysis

### 4.1 Heroic Games Launcher

Source: [Heroic Games Launcher Settings Interface](https://deepwiki.com/Heroic-Games-Launcher/HeroicGamesLauncher/4.4-settings-interface)

**Structure:**

- Global Settings vs. per-game settings (two levels of hierarchy, clearly labelled).
- Per-game settings categories: Wine/Proton, Performance, Launch Options, Advanced.
- Fallback mechanism: only changed values are stored per-game; defaults inherit from global.
- Advanced section contains experimental features and environment variable tables.

**What CrossHook can learn:**

- The category split (Wine/Proton + Performance + Launch Options + Advanced) maps well onto CrossHook's fields.
- "Advanced" in Heroic is genuinely a small residual category for experimental/power-user flags — not the entire feature set.
- Per-game vs. global inheritance is a useful mental model; CrossHook profiles are already per-game.

### 4.2 Lutris

- Per-game configuration accessible via right-click > Configure.
- Settings are organized into tabs: Game, Runner, System, and sometimes Wine.
- Advanced runner settings exist as a separate tab within Runner — not a collapse of the main screen.

**What CrossHook can learn:**

- Runner-specific settings (equivalent to CrossHook's Steam Runtime / Proton Runtime section) should be a dedicated visual area, not a conditional block inside a larger form.

### 4.3 Bottles (GTK4/Libadwaita)

Source: [Bottles — Welcome](https://docs.usebottles.com/), [Linux IAC — Bottles 60.0](https://linuxiac.com/bottles-60-0-launches-with-native-wayland-support/)

- Bottle-level settings use sidebar navigation with categories.
- Recent update (v60.0) introduced a "native Wayland option directly in the bottle settings" as a prominently-placed toggle, not hidden in advanced.
- Registry rules system for reusable policies shows a pattern of abstracting power-user flows rather than hiding them.

**What CrossHook can learn:**

- New or important features should get a card or a prominent toggle, not be buried in an expanded-by-default "Advanced" detail.

### 4.4 VS Code Settings

Source: [VS Code UX Guidelines](https://code.visualstudio.com/api/ux-guidelines/overview)

- Settings use a sidebar tree (categories) with a search bar and full-text search as the primary navigation.
- Two views: GUI form and JSON editor — users can use either.
- Settings are tagged `(modified)` when changed from default.
- No "Advanced" collapse — every setting is visible by scrolling, with categories providing structure.
- Settings have scope indicators (User, Workspace, Folder).

**What CrossHook can learn:**

- A search-in-form capability would allow removing the "Advanced" hide entirely without creating cognitive overload — users can find what they need.
- Marking modified fields visually helps users understand what is changed from defaults.

### 4.5 JetBrains IDEs

- Settings use a deep sidebar tree with many categories.
- Each settings pane shows fields in a standard form layout with no additional collapses.
- Keyboard shortcut `⌘,` opens settings directly; keyboard navigation within the tree is complete.

**What CrossHook can learn:**

- Deep hierarchies work in settings precisely because users are searching for something specific; there is no need to hide a settings pane from within its own pane.

### 4.6 macOS Ventura System Settings

Source: [9to5Mac — Ventura System Settings](https://9to5mac.com/2022/06/06/macos-13-ventura-system-settings-first-look/), [Macworld — Ventura System Settings Problems](https://www.macworld.com/article/836295/macos-ventura-system-settings-preferences-problems.html)

- Moved from icon grid (System Preferences) to sidebar list (System Settings) to mirror iOS.
- 31 preference sections consolidated and reorganized.
- Mixed reception: the sidebar approach works well for navigation, but some items were moved to unintuitive locations.

**Lesson**: Sidebar + content area is a proven pattern for complex settings, but the label taxonomy must match user mental models. "Advanced" fails this because it's not a task-category, it's a skill-level judgment.

### 4.7 KDE Plasma System Settings

Source: [KDE UserBase — System Settings](<https://userbase.kde.org/System_Settings/GNOME_Application_Style_(GTK)>)

- Sidebar tree + main panel; categories map to user tasks (e.g., "Workspace Behavior", "Input Devices").
- "Simple by default, powerful when needed" — initial view shows common settings; "More options" or dedicated subcategories reveal advanced settings.
- Complexity management challenge explicitly acknowledged: support common cases well while avoiding complexity explosion.

**Lesson**: KDE's philosophy — simple defaults, power accessible via clear secondary navigation rather than hidden collapses — is the right model.

### 4.8 Browser Profile Settings (Firefox / Chrome)

- Browser settings pages are long-scrolling forms with section headers, not tabs.
- Section headers (e.g., "General", "Home", "Privacy") serve as visual landmarks.
- Search functionality within settings is a primary navigation path.
- This layout works because browsers have stable, well-known setting categories.

**What CrossHook can learn:**

- CrossHook's field count is small enough that a long-scroll card layout (no tabs) may be more appropriate than sub-tabs — less interaction cost, better spatial memory.

---

## 5. Error Handling UX

### 5.1 Validation Across Tabs / Sections

Source: [Cloudscape Design System — Unsaved Changes](https://cloudscape.design/patterns/general/unsaved-changes/), [Smashing Magazine — Inline Validation](https://www.smashingmagazine.com/2022/09/inline-validation-web-forms-ux/)

- If moving to a tab/section pattern, validation errors in non-visible sections must be surfaced at the section level, not just field level.
- Common pattern: **tab indicator mutation** — append `•` or `*` to tab label when that section has unsaved changes or validation errors.
- Field-level inline validation: show error state immediately after focus-out, not only on save attempt.
- Cross-section save: a confirmation modal is appropriate when changes span multiple sections. "Are you sure? You have changes in: Game, Trainer, Runtime."

### 5.2 Unsaved Changes Warning

Source: [Cloudscape — Unsaved Changes](https://cloudscape.design/patterns/general/unsaved-changes/), [GitHub — Unsaved Changes Alerts in React](https://medium.com/@ignatovich.dm/how-to-create-a-custom-hook-for-unsaved-changes-alerts-in-react-b1441f0ae712)

- Warn when user tries to navigate away (profile switch, page navigation) with unsaved changes.
- Use in-page modal for in-app navigation; use `beforeunload` for window close.
- Track `hasChanges` boolean state; the existing `dirty` state in `ProfileContext` already does this.
- Confirmation message: "You have unsaved changes. Leave anyway?" with Cancel and Leave buttons.
- Do not warn if no changes were made (even if user opened a section).

**Scope clarification (from security-researcher)**: Sub-tab switching within the same profile (Steam → Proton → Native runner tabs) does NOT require a dirty-check prompt — form state persists across tab switches in React state. The `dirty` guard applies only to: (1) selecting a different profile from the dropdown, (2) page-level navigation away from Profiles. This simplifies tab implementation — no inter-tab confirmation dialogs needed.

**Discard action constraint**: If a "Discard changes" or "Reset" button is added anywhere, it must reload from the backend (`invoke('load_profile', ...)`) and clear `dirty`. Resetting to an in-memory snapshot is not acceptable — backend is the source of truth.

**Note**: CrossHook already has a `dirty` state from `useProfileContext()`. The mechanism for warning exists; it is a matter of hooking it to profile-switch and navigation events only.

### 5.3 Tab/Section State Persistence

- When a user switches sections (sub-tabs or collapses), their changes within that section must persist in React state until explicit save or discard.
- CrossHook already manages this via the profile form state in context — switching sections or expanding/collapsing should not lose field values.
- For tab-based layout: mount all tab content eagerly (or use CSS `display: none` to hide, not conditional rendering) to avoid losing form state on tab switch.

---

## Performance UX

Source: [MDN — Lazy Loading](https://developer.mozilla.org/en-US/docs/Web/Performance/Guides/Lazy_loading), [618media — Lazy Loading in JS 2024](https://618media.com/en/blog/user-experience-with-lazy-loading-in-js/)

### 6.1 For CrossHook's Scale

The Profiles page has a bounded, predictable field count. Performance concerns around lazy-loading tab content are not significant at this scale. The ProtonDB lookup card is the heaviest component (network requests to the ProtonDB API) and already deferred by the app_id dependency.

### 6.2 Recommendations

- Mount all section/tab content eagerly and control visibility via CSS `display: none` or `visibility: hidden`, not conditional rendering (`{condition && <Component />`). This prevents form state loss on tab switch.
- Skeleton screens or loading spinners are appropriate only for the ProtonDB lookup card, which makes external API calls. All other fields render immediately from local state.
- Tab switching should be instantaneous — no artificial delays or lazy mounting.

### 6.3 Perceived Performance

- When expanding a section, animate the height transition with CSS `transition: height` for perceived smoothness (already used in `CollapsibleSection`).
- Keep initial visible content minimal but complete — show all section headers immediately so users understand what's available.

---

## 7. Accessibility Requirements

Source: [W3C WAI ARIA Authoring Practices — Tab Pattern](https://www.w3.org/WAI/ARIA/apg/), [Deque — ARIA Tab Panel Accessibility](https://www.deque.com/blog/a11y-support-series-part-1-aria-tab-panel-accessibility/), [Level Access — Accessible Navigation Menus](https://www.levelaccess.com/blog/accessible-navigation-menus-pitfalls-and-best-practices/)

### 7.1 Tab Pattern (if adopted)

Required ARIA roles:

- Container: `role="tablist"`
- Individual tabs: `role="tab"`, `aria-selected="true|false"`, `aria-controls="panel-id"`
- Content panels: `role="tabpanel"`, `aria-labelledby="tab-id"`

Keyboard navigation:

- `Tab` key: moves focus INTO the tablist (not between tabs)
- Arrow keys `←/→` (horizontal tabs) or `↑/↓` (vertical): navigate BETWEEN tabs and activate them
- `Home`/`End`: jump to first/last tab
- `Space`/`Enter`: activates focused tab (should also work if not using automatic activation)

### 7.2 Card/Accordion Pattern (if adopted instead of tabs)

- Section headings use proper heading level hierarchy (`h2` for section titles within the page).
- Collapsible accordions: use `<button>` for trigger, `aria-expanded="true|false"`, `aria-controls` pointing to content panel.
- Never use `<details>/<summary>` for interactive disclosure that modifies application state — use button+panel pattern for full ARIA support. **Note**: `OptionalSection` in `ProfileFormSections.tsx` uses `<details>/<summary>` — this is acceptable for static optional content but not for stateful or frequently-accessed sections.

### 7.3 Visual Accessibility

- Active section indicator: do not rely on color alone; use underline, bold, or border change.
- Focus indicators: visible on all interactive elements; maintain 3:1 contrast ratio minimum.
- Minimum touch targets: 44×44 CSS pixels for tab/section buttons.
- High contrast mode: ensure section borders use `currentColor` or CSS variables that adapt.

---

## 7b. Component Inventory (Post-Practices-Researcher Update)

Grounded assessment of what exists vs. what needs to be built:

| Component                      | Status                                                                                                                                                                                                                                           | Action                                                                                                                                                                               |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `FieldWithBrowse` / `FieldRow` | **Duplicated** — `InstallField` at `src/components/ui/InstallField.tsx` (exported, has `browseMode`/`browseFilters`/`browseTitle`/`className`) and private `FieldRow` inside `ProfileFormSections.tsx` (thinner, no browse filters, has `useId`) | Consolidate: add `id` prop to `InstallField`, delete private `FieldRow`. `InstallField` becomes canonical.                                                                           |
| `SectionCard`                  | **Not needed as new component** — `crosshook-panel` CSS class already provides the visual surface (border, background gradient, radius, shadow in `theme.css:137–151`). `CollapsibleSection` already accepts `className="crosshook-panel"`.      | A thin React wrapper (`title`, `meta?: ReactNode`, `children` props) is a one-day addition only if always-visible titled headers are needed in 2+ places. Do not build preemptively. |
| `SectionErrorIndicator`        | **Not needed** — `CollapsibleSection`'s existing `meta` slot accepts arbitrary `ReactNode`. An error dot is a two-line addition at the call site.                                                                                                | Extract only if the same indicator appears in 3+ call sites.                                                                                                                         |
| `UnsavedChangesGuard`          | **Valid but out of scope for initial cleanup.** `dirty` state exists in `ProfileContext`; tab switching within the page does not cause data loss (all tabs share the same profile state). Inter-page navigation guard is additive scope.         | Track as a follow-on issue, not a prerequisite for the layout change.                                                                                                                |
| `@radix-ui/react-tabs`         | **Already installed** (`^1.1.13`). Use primitives directly in `ProfilesPage.tsx`; no wrapper component until the pattern appears on a second page.                                                                                               | Use directly.                                                                                                                                                                        |

---

## 8. Recommendations

### 8.1 Must Have

**M1 — Eliminate the single "Advanced" collapse around the entire form.**
Replace it with named, always-visible section cards and/or sub-tabs. The sections should reflect user task mental models:

Hybrid recommended layout (post-infrastructure discovery):

| Section                          | Pattern                                                                     | Contents                                                                           |
| -------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Profile Identity                 | Always-visible strip (already partial)                                      | Profile name, selector dropdown                                                    |
| Health Status                    | Always-visible badge strip                                                  | Health badge, offline status, version badge                                        |
| Runner (Steam / Proton / Native) | **Sub-tabs** using existing `crosshook-subtab-row` + `@radix-ui/react-tabs` | Runner-specific fields: App ID, prefix, proton path, AutoPopulate, ProtonDB lookup |
| Trainer                          | Always-visible card                                                         | Trainer path, type, loading mode, version                                          |
| Environment & Launch             | Always-visible card                                                         | Custom env vars, working directory override                                        |
| Launcher Export                  | Separate panel (already exists — keep)                                      | Launcher name/icon, export action                                                  |

Using sub-tabs for the runner section is now the preferred approach because:

1. The existing `crosshook-subtab-*` CSS is purpose-built for exactly this use case.
2. The three runner methods (Steam / Proton / Native) have distinct field sets — sub-tabs make that branching explicit.
3. `@radix-ui/react-tabs` provides ARIA compliance without additional library cost.
4. The runner method dropdown (current `ThemedSelect`) becomes the tab selector — removing a dropdown and replacing it with visible, labeled tabs reduces cognitive load.

**M2 — Promote health status to always-visible position.**
The health badge and offline status badge belong in a persistent strip below the profile selector, not inside the collapsed section. Health information is critical-path — users should see it without having to expand anything.

**M3 — Keep launcher export as a separate panel.**
The existing split between "Profile" and "Launcher Export" panels is correct and should be preserved. It represents a task boundary: configure → export.

**M4 — Cap disclosure at one level.**
Within any card, a single optional sub-section (working directory override, launcher metadata) is acceptable using the existing `OptionalSection` pattern. But no card should contain a collapse that contains another collapse.

### 8.2 Should Have

**S1 — Dirty state indicator per section.**
When using section cards, append a visual indicator (e.g., a colored left border or `•` in the section title) when that section has been modified since last save. This prevents lost changes when users scroll past edited fields.

**S2 — ProtonDB lookup card in a prominent but contextual position.**
Currently embedded deep inside the Steam Runtime section. Should be immediately after the runner-specific required fields (App ID + Proton path), clearly labeled as "ProtonDB Compatibility" with a short description. Not hidden in a collapse.

**S3 — Unsaved changes dialog on profile switch.**
When `dirty === true` and the user selects a different profile in the top dropdown, show a confirmation dialog: "You have unsaved changes to [current profile]. Switch anyway?" This uses the existing `dirty` state from context. Note: tab switching within the same profile does NOT need this guard — all runner tabs share the same `profile` state in `ProfileContext`. This guard applies only to profile dropdown selection and sidebar page navigation. Implementation is additive and can ship as a follow-on to the layout cleanup.

**S4 — AutoPopulate result as a contextual hint, not a separate section.**
The `AutoPopulate` component currently renders as a separate sub-block inside the Steam Runtime section. Consider integrating its results as inline suggestions beneath the fields they populate (App ID, Prefix Path, Proton Path), similar to the ProtonDB suggestion pattern.

### 8.3 Nice to Have

**N1 — Inline field search.**
A search box within the profile form that scrolls to and highlights matching fields. Eliminates the discoverability problem entirely for power users. VS Code and JetBrains settings use this to great effect.

**N2 — Contextual help tooltips on hover.**
Short (1–2 sentence) explanations for non-obvious fields (Trainer Loading Mode, Prefix Path, STEAM_COMPAT_DATA_PATH derivation). Currently this information is in `crosshook-help-text` paragraphs; converting to tooltip-on-demand reduces vertical space while preserving the information.

**N3 — Section collapse as a user preference.**
Allow users to personally collapse sections they never use (e.g., a native Linux user who never uses trainer fields). Persist this preference in localStorage or TOML user settings. This is a "power user" feature and should not be the primary decluttering mechanism.

**N4 — "Compact / Expanded" view toggle.**
A single toggle that collapses all optional/infrequently-needed fields (working directory override, launcher metadata) while keeping essential fields visible. This is a simpler version of N3 that requires no per-section persistence.

---

## 9. Creative Ideas

### 9.1 Task-Oriented Wizard as the Default Path; Manual Form as Power-User Path

The existing wizard is excellent for first-time setup. Consider extending this model: the default view shows a compact "summary card" of the current profile (name, runner method, trainer path, health badge — read-only) with an "Edit" button that expands into the full form. This makes the page clean for users who just want to verify their setup without editing.

### 9.2 Split-Pane: Profile List Left, Editor Right

Replace the page-banner layout with a two-column layout: a profile list on the left (small, ~250px), and the editor on the right (large). This is the pattern used by VS Code's multi-file editor, JetBrains settings, and most terminal emulator profile editors (Alacritty, WezTerm). Benefits:

- Profile list always visible — no dropdown needed.
- Editor area larger and easier to scroll.
- Direct visual feedback when switching profiles.

This is a significant layout change but would address root-cause layout problems better than any single section reorganization.

### 9.3 Inline Health Issue Annotations

Rather than a separate "Health Issues" collapse, annotate specific fields with the health issue inline. If `game.executable_path` is broken, show a red border and inline error on that specific field. This eliminates the need for a separate health section entirely.

### 9.4 "Required for Launch" vs. "Optional" Visual Treatment

A subtle label system:

- Red asterisk or "Required" chip: fields that will prevent launch if empty.
- Gray "Optional" label: fields that are safe to leave empty.
- Blue "Recommended" chip: ProtonDB suggestions.

This replaces the "Advanced" concept with a task-relevance concept — users know which fields they must fill, not which ones require "advanced" knowledge.

---

## 10. Open Questions

1. **Target user profile**: Is the primary user a first-time configurator (wizard path) or a returning power user making small tweaks? This determines whether compact-by-default or expanded-by-default is more appropriate.

2. **Profile count at scale**: What happens to the profile selector UX when a user has 20+ profiles? The dropdown already supports pinning — is that sufficient, or does a profile list panel (split-pane idea in §9.2) become necessary at that scale?

3. **Runner-method-driven field conditionality**: Fields change significantly based on runner method. Would sub-tabs per runner method (Steam | Proton | Native) be cleaner than showing/hiding fields within a single form? This would eliminate the `showLauncherMetadata`, `supportsTrainerLaunch`, `showProtonDbLookup` conditional logic in favor of separate tab content.

4. **Wizard-vs-form parity**: The onboarding wizard and the manual form cover the same fields. If the form is reorganized into named cards, the wizard should step through the same card groups (Profile Identity → Game & Runtime → Trainer → Finalize) to create consistent mental models.

5. **When to introduce tabs**: Originally deferred pending cost analysis. **Updated**: The existing `crosshook-subtab-*` CSS and `@radix-ui/react-tabs` library make tabs zero additional design or dependency cost. Use tabs for the runner-method section (where content diverges most). Use always-visible cards for Trainer and Environment sections (whose content is stable regardless of runner method).

---

## Sources

- [NN/G — Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/)
- [LogRocket — Progressive Disclosure UX Types](https://blog.logrocket.com/ux-design/progressive-disclosure-ux-types-use-cases/)
- [Eleken — Tabs UX: Best Practices](https://www.eleken.co/blog-posts/tabs-ux)
- [LogRocket — Tabbed Navigation UX Best Practices](https://blog.logrocket.com/ux-design/tabs-ux-best-practices/)
- [Design Shack — Card Layouts Modern UX](https://designshack.net/articles/ux-design/card-layouts-modern-ux/)
- [Cloudscape Design System — Unsaved Changes](https://cloudscape.design/patterns/general/unsaved-changes/)
- [W3C WAI ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/)
- [Deque — ARIA Tab Panel Accessibility](https://www.deque.com/blog/a11y-support-series-part-1-aria-tab-panel-accessibility/)
- [Heroic Games Launcher — Settings Interface](https://deepwiki.com/Heroic-Games-Launcher/HeroicGamesLauncher/4.4-settings-interface)
- [9to5Mac — macOS Ventura System Settings](https://9to5mac.com/2022/06/06/macos-13-ventura-system-settings-first-look/)
- [Macworld — Ventura System Settings Problems](https://www.macworld.com/article/836295/macos-ventura-system-settings-preferences-problems.html)
- [VS Code UX Guidelines](https://code.visualstudio.com/api/ux-guidelines/overview)
- [Smashing Magazine — Inline Validation UX](https://www.smashingmagazine.com/2022/09/inline-validation-web-forms-ux/)
- [MDN — Lazy Loading](https://developer.mozilla.org/en-US/docs/Web/Performance/Guides/Lazy_loading)
- [618media — Lazy Loading in JS 2024](https://618media.com/en/blog/user-experience-with-lazy-loading-in-js/)
- [Level Access — Accessible Navigation Menus](https://www.levelaccess.com/blog/accessible-navigation-menus-pitfalls-and-best-practices/)
- [NN/G — Mental Models](https://www.nngroup.com/articles/mental-models/)
