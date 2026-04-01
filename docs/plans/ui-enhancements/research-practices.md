# UI Enhancements — Engineering Practices Research

## Executive Summary

The Profiles page is dominated by a single `CollapsibleSection` (titled "Advanced") that contains `ProfileFormSections.tsx` — a 1,144-line monolith that renders every profile field inline. The architecture has a working primitive library (CollapsibleSection, ThemedSelect), a ready Radix UI Tabs dependency, and clean CSS variable infrastructure. However, `ProfileFormSections` is used at three callsites with different layout needs, so the tab layer must live at `ProfilesPage` level only — not inside `ProfileFormSections` itself.

## Existing Reusable Code

| File                                                                        | Description                                                                                                                                                                                                                                                                        |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/pages/ProfilesPage.tsx`                | Top-level page: Wizard area, profile selector, single "Advanced" CollapsibleSection, Actions bar, Launcher Export section, all modals                                                                                                                                              |
| `src/crosshook-native/src/components/ProfileFormSections.tsx`               | 1,144-line monolith: inline sub-components (FieldRow, ProtonPathField, LauncherMetadataFields, OptionalSection, ProfileSelectorField, TrainerVersionSetField) + main `ProfileFormSections` export; used by ProfilesPage, InstallPage (reviewMode), and imports by OnboardingWizard |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`                 | Uses `ProfileFormSections` with `reviewMode` prop inside a compact `ProfileReviewModal` — a tab-based layout would be wrong UX here                                                                                                                                                |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`                  | Imports only the `ProtonInstallOption` type from `ProfileFormSections`; builds its own step-by-step form from individual components directly                                                                                                                                       |
| `src/crosshook-native/src/components/ui/CollapsibleSection.tsx`             | Controlled/uncontrolled `<details>` wrapper; accepts `meta` slot for inline badges                                                                                                                                                                                                 |
| `src/crosshook-native/src/components/ui/ThemedSelect.tsx`                   | Radix `@radix-ui/react-select` wrapper; supports groups and pinned values                                                                                                                                                                                                          |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                | Top-level router using `@radix-ui/react-tabs` — the existing tabs pattern for the app shell                                                                                                                                                                                        |
| `src/crosshook-native/src/components/ProfileActions.tsx`                    | Action bar (Save/Duplicate/Rename/Preview/Export/History/Delete buttons)                                                                                                                                                                                                           |
| `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx` | Already an independent component for env vars; contains security-critical `RESERVED_CUSTOM_ENV_KEYS` constant                                                                                                                                                                      |
| `src/crosshook-native/src/components/GamescopeConfigPanel.tsx`              | Already extracted; uses CollapsibleSection + ThemedSelect                                                                                                                                                                                                                          |
| `src/crosshook-native/src/components/MangoHudConfigPanel.tsx`               | Already extracted; uses CollapsibleSection + ThemedSelect + hook                                                                                                                                                                                                                   |
| `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`          | Already extracted; complex multi-section panel                                                                                                                                                                                                                                     |
| `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`           | Already extracted; IPC-backed preview panel                                                                                                                                                                                                                                        |
| `src/crosshook-native/src/styles/variables.css`                             | All design tokens; `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` tokens already exist with controller-mode overrides                                                                                                                                     |
| `src/crosshook-native/src/styles/collapsible-section.css`                   | CSS for CollapsibleSection; includes stripping rules for nested panels                                                                                                                                                                                                             |
| `src/crosshook-native/src/styles/theme.css`                                 | Main theme stylesheet                                                                                                                                                                                                                                                              |
| `src/crosshook-native/package.json`                                         | Dependencies: `@radix-ui/react-tabs` v1.1.13 and `@radix-ui/react-select` already installed                                                                                                                                                                                        |

## Architectural Patterns

- **Radix UI as the UI primitive layer**: Both `@radix-ui/react-tabs` (already used in `ContentArea.tsx`) and `@radix-ui/react-select` (wrapped by `ThemedSelect`) are already installed and in use. No new dependency needed.
- **CSS variable token system**: `variables.css` already defines `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` with controller-mode overrides — the design system anticipates sub-tabs and has pre-allocated tokens for them. Also `--crosshook-panel-padding` and `--crosshook-card-padding` cover card/section spacing.
- **CollapsibleSection as the disclosure primitive**: Controlled/uncontrolled, accepts a `meta` slot for badges, already styled. The stripping rules in `collapsible-section.css` let nested panels appear unstyled inside a CollapsibleSection wrapper.
- **BEM-like `crosshook-*` class convention**: All components use `crosshook-<block>__<element>` patterns. For sub-tabs specifically, `crosshook-subtab-row` / `crosshook-subtab` / `crosshook-subtab--active` are already the canonical classes in `theme.css` — use these, do not invent new BEM names.
- **Component extraction pattern**: `GamescopeConfigPanel`, `MangoHudConfigPanel`, `LaunchOptimizationsPanel`, `SteamLaunchOptionsPanel`, and `CustomEnvironmentVariablesSection` demonstrate the target shape: self-contained props interface, internal state only, `CollapsibleSection`/`ThemedSelect` for structure, no context imports.
- **Context at page level**: `useProfileContext`, `useProfileHealthContext`, and `usePreferencesContext` are consumed at page level in `ProfilesPage.tsx`. Extracted sub-panels should receive their slice of data as props — they must not call these hooks directly.
- **`OptionalSection` is a private `<details>` wrapper inside `ProfileFormSections.tsx`**: Not exported; uses hardcoded inline style objects (`optionalSectionStyle`, `optionalSectionSummaryStyle`) instead of CSS classes. Inconsistent with the CSS variable pattern; replace with `CollapsibleSection defaultOpen={false}` when refactoring.

## KISS Assessment

| Approach                                            | Complexity                                                                                                         | Value                                                                                         | Verdict                                                                                      |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| **Visual containers / clear boundaries**            | Low — CSS-only, no new components                                                                                  | Medium — reduces visual noise, does not address discoverability                               | Good baseline step; do it as part of any other option                                        |
| **Promote critical sections out of Advanced**       | Low — move JSX up in `ProfilesPage.tsx`, delete or reduce the outer CollapsibleSection                             | High — mandatory fields (Game Path, Runner Method) are always visible                         | Should be done regardless; lowest risk, meaningful gain                                      |
| **Sub-tabs at `ProfilesPage` level**                | Low-Medium — `@radix-ui/react-tabs` already installed; one new CSS block + tab wrappers in `ProfilesPage.tsx` only | High — surfaces all sections as named, accessible tabs without touching `ProfileFormSections` | Correct approach: tabs live in `ProfilesPage`, `ProfileFormSections` stays a linear renderer |
| **Tabs embedded inside `ProfileFormSections`**      | Medium — forces dual rendering paths for `reviewMode` (InstallPage) and normal mode                                | Negative — wrong UX for compact review modal in `InstallPage`                                 | Do not do this; breaks the existing `reviewMode` contract                                    |
| **Full page split (separate routes per section)**   | High — new routes, navigation state, breadcrumbs                                                                   | Low marginal gain over sub-tabs                                                               | Overkill                                                                                     |
| **Contextual/smart settings (game-type detection)** | High — requires metadata that does not exist                                                                       | Low — metadata infrastructure absent                                                          | Scope creep; do not include                                                                  |
| **Drag-and-drop section reordering**                | High — needs DnD library, persistence layer for order preferences                                                  | Low — does not solve clutter                                                                  | Poor effort-to-impact ratio                                                                  |
| **Search/filter for ~15–20 fields**                 | Medium                                                                                                             | Near-zero — search is justified for 50+ settings                                              | Not applicable at this form size                                                             |

## Security Constraints

These patterns must be preserved as shared utilities during any restructuring. Do not inline or duplicate them.

- **`RESERVED_CUSTOM_ENV_KEYS`** (`CustomEnvironmentVariablesSection.tsx:6-10`): Client-side constant that mirrors `RESERVED_CUSTOM_ENV_KEYS` in `crosshook-core/src/launch/request.rs`. This is a **defense-in-depth guard for manually-entered env vars** — the ProtonDB suggestion path is already sanitized at the backend (`aggregation.rs::safe_env_var_suggestions()` applies key regex, value character filtering, and reserved-key stripping before IPC). The frontend constant remains the authoritative guard for user-typed input and a second line of defense for any future code paths. Keep it in `CustomEnvironmentVariablesSection` or extract to `utils/envVars.ts` if the component is split; never inline or remove it.
- **`customEnvKeyFieldError` / `customEnvRowError`** (`CustomEnvironmentVariablesSection.tsx:38–89`): Pure validation functions for env var keys and values. Already well-isolated. If the env var section is split, extract to `utils/envVars.ts` rather than duplicating.
- **`validate_name()` / `profile_path()` gate** (`crosshook-core/src/profile/toml_store.rs:468–521`): All filesystem operations for profile names go through this Rust-side gate. Backend-only; no change needed from UI restructuring. Do not add any new direct `fs::` calls in profile commands that bypass this gate.
- **Path fields are intentionally free-form**: Game path, executable path, and working directory fields have no shared client-side validator — this is deliberate. Do not add one.
- **Tab/navigation state**: Use `sessionStorage` locally in `ProfilesPage.tsx` if persistence is needed. No shared utility required.

## Modularity Design

### Recommended module boundaries for splitting `ProfileFormSections.tsx`

The monolith should be split into section components that `ProfilesPage.tsx` composes inside tab panels, while `ProfileFormSections` continues to render them linearly for `InstallPage`'s `reviewMode`:

1. **`ProfileIdentitySection`** — Profile name, profile selector dropdown, profile load/pin control. Can be hoisted as a permanent-visible header above the tabs in `ProfilesPage`.

2. **`GameSection`** — Game name, game path browse. Purely controlled fields, no internal state.

3. **`RunnerMethodSection`** — Runner method select + helper text. Pure field; switching the method controls what other sections render.

4. **`RuntimeSection`** (method-conditional) — Steam App ID, Prefix Path, Proton Path, Working Directory override, AutoPopulate, ProtonDB lookup.

5. **`TrainerSection`** — Trainer path, trainer type select, trainer loading mode, trainer version display + manual set. `TrainerVersionSetField` and its IPC call stay file-local here.

6. **`EnvVarsSection`** — Already extracted as `CustomEnvironmentVariablesSection`. No further split needed; security-critical validation logic must stay inside this component (see Security Constraints above).

7. **`LauncherMetadataSection`** — Launcher name and icon fields. Consolidate the currently split `LauncherMetadataFields` (private) and method-conditional blocks.

### Shared vs. feature-specific

- **Shared (promote to `ui/`)**: `ProtonPathField` — currently private to `ProfileFormSections.tsx`; check against `ui/ProtonPathField.tsx` before extracting (may be same or diverged). `FieldRow` is resolved: merge into `ui/InstallField.tsx` with `id` prop addition (see Abstraction vs. Repetition).
- **Feature-specific (keep local or delete)**: `OptionalSection` — replace with `CollapsibleSection defaultOpen={false}`.
- **Shared context for sub-tab state**: A single `useState<TabId>` in `ProfilesPage.tsx` is sufficient. Do not create a context for tab selection.

## Abstraction vs. Repetition

- **`FieldRow` vs. `InstallField`**: `FieldRow` (private, 10+ usages in `ProfileFormSections.tsx`) and `ui/InstallField.tsx` (exported) are the same pattern at slightly different API surfaces — `InstallField` has `browseMode`, `browseFilters`, `browseTitle`, and `className` but lacks `id` (uses no `useId`); `FieldRow` has `id` via `useId` but no browse-mode/filter props. Resolution: add `id` support to `InstallField`, migrate all `FieldRow` usages to it, and delete the private copy. No new component needed.
- **`ProtonPathField`** appears twice with near-identical props. A version already exists at `src/crosshook-native/src/components/ui/ProtonPathField.tsx` — verify it is the same component before promoting the private one.
- **`OptionalSection`** — one-off inline `<details>` with hardcoded inline styles. Replace all usages with `CollapsibleSection defaultOpen={false}` and delete.
- **The ProtonDB overwrite confirmation dialog** (lines 549–652 in `ProfileFormSections.tsx`) is 100 lines of inline JSX. Extract to a named `ProtonDbConflictDialog` component.
- **Do not abstract the tab definitions** into a config array — four to five tabs with conditional rendering differences do not warrant a generic tab-registry pattern.

## Interface Design

The correct architecture keeps `ProfileFormSections` as a linear renderer and adds the tab layer only at `ProfilesPage` level:

```
ProfilesPage
  ├── PageBanner (always visible)
  ├── Health/Rename toasts (always visible)
  ├── Panel
  │   ├── Guided Setup header (always visible)
  │   ├── Profile selector bar (always visible when profiles exist)
  │   └── Tabs.Root  ← NEW: replaces the "Advanced" CollapsibleSection
  │       ├── Tabs.List
  │       │   ├── Tabs.Trigger "Setup"       (Profile Identity + Game + Runner Method)
  │       │   ├── Tabs.Trigger "Runtime"     (Proton/Steam paths, AutoPopulate, ProtonDB)
  │       │   ├── Tabs.Trigger "Trainer"     (hidden/disabled when method = 'native')
  │       │   ├── Tabs.Trigger "Environment" (custom env vars)
  │       │   └── Tabs.Trigger "Launcher"    (disabled/empty for native profiles)
  │       ├── Tabs.Content "setup"    → <ProfileIdentitySection> + <GameSection> + <RunnerMethodSection>
  │       ├── Tabs.Content "runtime"  → <RuntimeSection>
  │       ├── Tabs.Content "trainer"  → <TrainerSection>
  │       ├── Tabs.Content "env"      → <CustomEnvironmentVariablesSection>
  │       └── Tabs.Content "launcher" → <LauncherMetadataSection> + <LauncherExport>
  └── ProfileActions bar (always visible, below the panel)

InstallPage (unchanged)
  └── ProfileReviewModal
      └── ProfileFormSections reviewMode={true}  ← linear render, no tabs
```

`ProfileFormSections` stays unchanged as a linear renderer. The extracted section components are composed inside both the tab panels (ProfilesPage) and `ProfileFormSections` (for reviewMode compatibility).

### CSS needed

No new CSS file required. The subtab classes are **fully implemented** in `theme.css:104-135`:

- `.crosshook-subtab-row` — pill-style flex container with border and muted background (`theme.css:104`)
- `.crosshook-subtab` — individual tab button using `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` tokens (`theme.css:115`)
- `.crosshook-subtab--active` — accent gradient + white text for the selected tab (`theme.css:131`)
- Responsive override at narrow widths: `.crosshook-subtab` gets `flex: 1 1 0` so tabs fill the row (`theme.css:3214`)

The Radix `Tabs.Trigger` elements should receive `className="crosshook-subtab"` and the active state applied via `data-state="active"` selector or the `--active` class. No new styles needed.

## Testability Patterns

- **No test framework is currently configured** (noted in `CLAUDE.md`). Extraction produces individually testable components but testing is not a blocker.
- **The section components are pure**: `GameSection`, `TrainerSection`, etc. take controlled props and fire `onUpdateProfile` callbacks — straightforward to test when a framework is added.
- **Tab state is trivially testable**: A single `activeTab` string in `ProfilesPage`; no async logic.
- **IPC-backed components** (`TrainerVersionSetField`, `SteamLaunchOptionsPanel`, `AutoPopulate`) only call `invoke()` — the existing pattern of passing callbacks from the hook layer keeps them boundary-testable.

## Build vs. Depend

| Decision                             | Recommendation                                                                                                                               |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **Tab primitives**                   | `@radix-ui/react-tabs` v1.1.13 — already installed; `orientation="vertical"` available if sidebar-style is ever needed. Do not build custom. |
| **Collapsible/disclosure animation** | `@radix-ui/react-accordion` is the natural upgrade path if `<details>` animation is needed, but is not required for this refactor.           |
| **Select component**                 | `ThemedSelect` wrapping `@radix-ui/react-select` — already done, keep it.                                                                    |
| **Collapsible**                      | `CollapsibleSection` (existing) — already done.                                                                                              |
| **New dependencies**                 | None needed. `@headlessui/react`, `@ark-ui/react`, and shadcn/ui are all redundant or incompatible.                                          |
| **Animation**                        | Not needed. `--crosshook-transition-fast` and `--crosshook-transition-standard` CSS variables are sufficient.                                |

## Gotchas and Edge Cases

- **`ProfileFormSections` is used at three callsites with incompatible layout needs**: `ProfilesPage.tsx` (full editing, tab layout appropriate), `InstallPage.tsx` (compact `reviewMode` modal — tabs would be wrong UX here), and `OnboardingWizard.tsx` (type import only; wizard builds its own step form). Embedding tabs inside `ProfileFormSections` would break `InstallPage`. Tabs must live at `ProfilesPage` level only.
- **`OptionalSection` uses hardcoded inline style objects**: `optionalSectionStyle` and `optionalSectionSummaryStyle` in `ProfileFormSections.tsx` lines 60–75. Inconsistent with the CSS variable pattern; replace during extraction.
- **`ProfileFormSections` exports `deriveSteamClientInstallPath` as a re-export** (line 113): `export { deriveSteamClientInstallPath } from '../utils/steam'`. Other files may import this utility via `ProfileFormSections` — check before splitting the file.
- **Method-conditional rendering inside a single component**: `RuntimeSection` renders completely different fields for `steam_applaunch` vs. `proton_run`. The conditional blocks are large and must be preserved exactly when extracting.
- **`ProtonInstallOption` type is imported by `OnboardingWizard`, `InstallGamePanel`, `UpdateGamePanel`, and `ui/ProtonPathField.tsx`** from `ProfileFormSections` directly. If the file is split, this type must be re-exported from a stable location (e.g., `src/types/index.ts` or `ui/ProtonPathField.tsx`) to avoid breaking all four importers.
- **`RESERVED_CUSTOM_ENV_KEYS` must not be duplicated**: The constant in `CustomEnvironmentVariablesSection.tsx` mirrors a Rust-side set in `crosshook-core`. Any refactor that touches env var handling must keep this in a single location — either in the component or extracted to `utils/envVars.ts`. See Security Constraints.

## Open Questions

1. **Should the active sub-tab be persisted across page navigation?** `sessionStorage` is already used for banner/toast dismissal in `ProfilesPage.tsx` — the same pattern could persist the active tab key.
2. **Does the "Launcher" tab show a disabled state or hide for native profiles?** Currently `supportsLauncherExport` hides `LauncherExport` entirely; an always-visible but conditionally disabled tab may be more discoverable.
3. **`FieldRow` / `InstallField` merger**: Resolved — `InstallField` is the canonical component; add `id` prop support and migrate `FieldRow` usages to it. See Abstraction vs. Repetition.
4. **ProtonDB conflict resolution dialog**: The 100-line inline dialog (lines 549–652) should become a named `ProtonDbConflictDialog` component before or during the `RuntimeSection` extraction.

## Out-of-Scope Follow-ons

These were surfaced during research but are not required for the initial UI cleanup:

- **`UnsavedChangesGuard`**: A hook wrapping `dirty` state (already in `ProfileContext`) to show a confirmation dialog on inter-page navigation. Tab switching within `ProfilesPage` does not trigger data loss — `ContentArea` uses Radix `Tabs.Content forceMount` and `ProfileContext` persists across route switches. The guard would protect against sidebar route changes while dirty. Valid UX improvement; implement as a separate issue after the layout cleanup.

## Other Docs

- `docs/plans/ui-enhancements/research-security.md` -- security constraints detail (reserved env key contract, profile name path traversal gate, verified ProtonDB env var sanitization chain in `aggregation.rs`)
- `docs/plans/ui-enhancements/research-external.md` — dependency analysis (Radix UI version inventory, build-vs-depend table)
- `docs/plans/ui-enhancements/research-ux.md` — UX patterns, competitive analysis, proposed reusable components assessment
