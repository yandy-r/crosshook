# Business Analysis: Profiles Page UI Enhancements

## Executive Summary

The Profiles page is the central configuration hub for CrossHook, but its current layout buries the entire profile editor (identity, game, runner, trainer, environment variables) inside a single collapsed `Advanced` section. The result is that first-time and returning users must expand a non-obvious section to do any actual profile editing. Health status, status badges, and the Refresh button are all pinned to the collapsed header, creating an information hierarchy that rewards power users but confuses newcomers. Separating logically distinct concerns into discrete visual containers (mirroring the existing Profile/Launcher Export split), promoting the editor out of `Advanced`, and grouping form sections by user mental model will dramatically reduce perceived clutter without hiding functionality.

---

## User Stories

**New user creating a first profile**

- As a user setting up my first game + trainer combo, I want to understand what I must fill out versus what is optional, so I can complete setup without guessing.
- As a new user, I do not want to hunt for a collapsed "Advanced" section to enter basic game paths.

**Returning user editing an existing profile**

- As a power user, I want to quickly jump to the section I need (e.g., environment variables or the ProtonDB lookup) without scrolling through unrelated fields.
- As a returning user, I want to see at a glance which profile is active and whether it is healthy, without expanding a collapsible.

**User managing multiple profiles**

- As a user with 10+ profiles, I want the profile selector to remain permanently visible so I can switch between profiles without losing my scroll position.
- As a user managing profiles, I want Save, Duplicate, Rename, Delete in a consistent place — not buried below all form fields.

**User troubleshooting a broken profile**

- As a user whose profile has health issues, I want the health status and issue list to be surfaced without having to expand a collapsed section.
- As a user who just made changes and saved, I want confirmation that the save succeeded without hunting for status indicators.

---

## Current Layout Analysis

The Profiles page is a single-column vertical layout (`display: grid; gap: 24`) with the following top-level containers:

### Top-level containers (in order, always visible)

1. **PageBanner** — Eyebrow "Profiles", title "Profile editor", copy text, illustration art.
2. **Health/rename toast area** — Temporary banners (broken count, rename confirmation).
3. **Single `crosshook-panel` div** — Contains ALL of the following in one visual card:
   - **Guided Setup subsection** — accent-colored top strip with "Profile Setup Wizard" heading and "New Profile" / "Edit in Wizard" buttons.
   - **Active Profile selector** — Only visible when `profiles.length > 0`; select + label.
   - **`CollapsibleSection` titled "Advanced" (defaultOpen=false)** — This collapses all of:
     - Status badges (HealthBadge, OfflineStatusBadge, trainer type chip, version badge)
     - Refresh button
     - Profile health summary / Re-check All
     - Stale data notice
     - **`ProfileFormSections`** — The entire profile editor (see Section Inventory below)
     - **Health Issues nested `CollapsibleSection`** (conditionally rendered, inside Advanced)
   - **ProfileActions footer** — Save, Duplicate, Rename, Preview Profile, Export as Community Profile, Mark as Verified, History, Delete, unsaved-changes indicator.
4. **`CollapsibleSection` "Launcher Export"** — Conditionally visible when `launchMethod === 'steam_applaunch' || 'proton_run'`.
5. **Modal overlays** — Delete confirm, Rename dialog, ProfilePreviewModal, ConfigHistoryPanel, OnboardingWizard.

### Key observation

The `Advanced` section wraps the **entire profile editor form plus health information**. This is the primary clutter source: users must expand "Advanced" to do any editing at all, yet the section is collapsed by default.

---

## Section Inventory

### Inside `ProfileFormSections` (the collapsed Advanced section)

#### 1. Profile Identity

- **Profile Name** — Text input. Read-only when profile exists. Required.

#### 2. Game

- **Game Name** — Text input. Display name for the game.
- **Game Path** — Text input + Browse button. Required (blocks Save).

#### 3. Runner Method

- **Runner Method** — ThemedSelect: `steam_applaunch` / `proton_run` / `native`. Required. Controls which downstream sections appear.

#### 4. Custom Environment Variables

- `CustomEnvironmentVariablesSection` — Editable key/value table of env vars passed at launch. Reserved keys blocked. ProtonDB suggestions flow into this section.

#### 5. Trainer _(only when `launchMethod !== 'native'`)_

- **Trainer Path** — Text input + Browse.
- **Trainer type (offline scoring)** — ThemedSelect from catalog + optional "Offline help" button.
- **Trainer Loading Mode** — ThemedSelect: `source_directory` / `copy_to_prefix`.
- **Trainer Version** (read-only, conditionally rendered when version recorded).
- **Set Trainer Version** (only when profileExists and not reviewMode) — Manual version override field.

#### 6. Steam Runtime _(only when `launchMethod === 'steam_applaunch'`)_

- **Steam App ID** — Text input. Required for ProtonDB lookup.
- **Prefix Path** — Text input + Browse (compatdata_path).
- **Launcher Name** + **Launcher Icon** — LauncherMetadataFields; display name and icon for the exported .desktop entry.
- **Proton Path** — ProtonPathField: ThemedSelect of detected installs + manual text input + Browse.
- **AutoPopulate** component — Automatically fills App ID, compatdata path, proton path from game path.
- **ProtonDbLookupCard** — Fetches ProtonDB recommendations for the App ID. Shows env var suggestion groups with Apply buttons.
- **ProtonDB conflict resolution UI** — Inline conflict per-key resolution when applying ProtonDB env vars.

#### 7. Proton Runtime _(only when `launchMethod === 'proton_run'`)_

- **Prefix Path** — Text input + Browse (runtime.prefix_path).
- **Steam App ID** — Optional, for ProtonDB lookup.
- **Launcher Name** + **Launcher Icon** — LauncherMetadataFields.
- **Working Directory** — Optional override; collapsed in reviewMode when empty.
- **Proton Path** — ProtonPathField.
- **ProtonDbLookupCard** + conflict resolution UI.

#### 8. Native Runtime _(only when `launchMethod === 'native'`)_

- **Working Directory** — Optional override; collapsed in reviewMode when empty.

### Outside `ProfileFormSections` (still inside `Advanced`)

- **Profile health summary chip + Re-check All button** — Shows stale/broken count across all profiles.
- **Stale info notice** — "Last checked N days ago — consider re-checking".
- **Health Issues `CollapsibleSection`** — Per-issue list (field, path, message, remediation), last success time, total launches, failure count, drift warnings, community import note.

### Outside `Advanced`

- **Guided Setup** — Wizard buttons (always visible at top of panel, above Advanced).
- **Active Profile selector** — ThemedSelect + label (always visible when profiles exist).
- **ProfileActions** — All action buttons + save status indicator (always visible at panel bottom).
- **Launcher Export `CollapsibleSection`** — LauncherExport component (conditionally visible, always defaultOpen=false).

---

## Business Rules

The following groupings encode the business logic for how profile settings should be organized, based on user mental models, workflow frequency, and domain relationships.

## Proposed Section Groupings

Based on user mental models and the field inventory, natural groupings emerge:

### Group 1: Profile Management (always visible)

Fields that identify the profile and control profile-level actions.

- Profile Name
- Active Profile selector (switch profile)
- ProfileActions (Save, Duplicate, Rename, Delete, Preview, History, Export, Mark Verified)
- Dirty/save status indicator

### Group 2: Setup Assistance (promote from buried position)

Entry points for assisted setup; should be discoverable but not dominating.

- Profile Setup Wizard (New Profile / Edit in Wizard)

### Group 3: Game Configuration (core, always visible/expanded)

The minimum fields required to configure a launch. Required for Save.

- Game Name
- Game Path
- Runner Method
- Trainer Path (when non-native)

### Group 4: Runtime / Proton Settings (expanded by default for proton users)

Runner-specific paths, conditionally shown based on Runner Method.

- Steam App ID (when steam_applaunch or proton_run)
- Prefix Path
- Proton Path
- AutoPopulate assistance
- Working Directory (proton_run / native, optional)

### Group 5: Launcher Export Settings

Only relevant when building a Steam launcher entry.

- Launcher Name
- Launcher Icon
- LauncherExport component

### Group 6: Environment Variables (toggleable, medium priority)

Power-user configuration. Important for compatibility but rarely touched per-session.

- Custom Environment Variables table
- ProtonDB Lookup + env var suggestion/conflict UI

### Group 7: Optimization & Hardware

Launch flags and hardware tuning (currently entirely on the Launch page).

- Trainer type (offline scoring)
- Trainer Loading Mode
- Trainer Version / Set Trainer Version

### Group 8: Profile Health & Diagnostics (promoted from collapsed Advanced header)

Status that tells users whether their profile is ready to launch.

- HealthBadge, OfflineStatusBadge, version badge, trainer type chip
- Refresh button
- Health summary (stale/broken count, Re-check All)
- Stale info notice
- Health Issues detail list

---

## Workflows

### Primary Workflow: First-time profile creation

1. User opens Profiles page — sees PageBanner and the main panel.
2. User sees "New Profile" wizard button and the profile name input.
3. User types a profile name.
4. User fills in Game Path.
5. User selects Runner Method.
6. (For non-native) User fills Trainer Path and Proton/prefix paths.
7. User clicks Save.
8. Health check runs automatically; badge appears.

**Current pain point**: Steps 2–7 require the user to expand "Advanced" first. The wizard is the only above-the-fold guided path, which users may overlook if they want direct editing.

### Alternative Workflow: Edit existing profile

1. User selects existing profile from the selector.
2. User expands "Advanced" to see form.
3. User edits specific field(s).
4. User clicks Save.

**Current pain point**: The user must know to expand "Advanced" to see any fields. Health and status badges are in the Advanced header, creating split attention — the badge is partially visible but the content is hidden.

### Alternative Workflow: Diagnose and fix a broken profile

1. User sees health banner ("N profiles have issues").
2. User selects the flagged profile.
3. User expands "Advanced".
4. User scrolls past profile identity, game, runner, trainer, env vars to reach the nested "Health Issues" collapsible.
5. User reads issue, fixes field, saves, re-checks.

**Current pain point**: Health Issues are nested inside a nested collapsible. Finding them requires multi-step expand + scroll.

### Alternative Workflow: Apply ProtonDB recommendations

1. User selects a steam_applaunch or proton_run profile.
2. User expands "Advanced".
3. User scrolls to Steam/Proton Runtime section.
4. User waits for ProtonDB lookup card to load.
5. User clicks "Apply" on a recommendation group.
6. If conflicts: user resolves each key in the inline conflict UI.
7. User scrolls back up to Save.

**Current pain point**: ProtonDB lookup is buried after multiple form sections and requires significant scroll to reach on dense profiles.

---

## Domain Model

### Profile entity (`GameProfile`)

A profile is the complete configuration for launching one game + trainer combination. It is stored as TOML (one file per profile). Key sub-objects:

| Sub-object               | Purpose                                                     | When visible                                                 |
| ------------------------ | ----------------------------------------------------------- | ------------------------------------------------------------ |
| `game`                   | Game name + executable path                                 | Always                                                       |
| `trainer`                | Trainer path, type, loading mode, version                   | When `launchMethod !== 'native'`                             |
| `injection`              | DLL injection paths + flags                                 | (Not in current form; legacy fields)                         |
| `steam`                  | App ID, compatdata path, proton path, launcher display/icon | When `launchMethod === 'steam_applaunch'`                    |
| `runtime`                | Prefix path, proton path, working directory                 | When `launchMethod === 'proton_run'` or working dir override |
| `launch.method`          | The enum Runner Method                                      | Always                                                       |
| `launch.custom_env_vars` | User-set key/value env vars                                 | Always                                                       |
| `launch.optimizations`   | Launch optimization toggle flags                            | LaunchPage only                                              |
| `launch.gamescope`       | Gamescope display config                                    | LaunchPage only                                              |
| `launch.mangohud`        | MangoHud overlay config                                     | LaunchPage only                                              |
| `local_override`         | Per-machine path overrides (not yet in form UI)             | —                                                            |

### Launch methods

Three mutually-exclusive runners — `steam_applaunch`, `proton_run`, `native` — gate which runtime fields appear. This is the primary configuration axis: selecting it should be the first meaningful choice after naming the profile.

### Health system

Profile health is computed asynchronously by `ProfileHealthContext`. A health badge (broken/stale/ok) appears in the Advanced section meta. Health Issues is a nested collapsible that lists per-field validation issues. This is diagnostic information — it does not block saving but informs the user whether the profile will launch.

---

## Success Criteria

1. A user with no prior CrossHook experience can create a working profile without expanding a collapsible section.
2. A user editing an existing profile can see the profile form without any extra interaction (no expand needed).
3. Profile health status is visible at a glance when a profile is selected, without requiring the user to scroll or expand.
4. Users who rarely touch advanced settings (ProtonDB, trainer type, env vars) are not visually overwhelmed by those sections.
5. The page retains all existing functionality — nothing is removed, only reorganized.
6. The new layout is consistent with existing CrossHook design patterns (panel/collapsible composition, `crosshook-*` CSS classes, CSS variable theming).

---

## Open Questions

1. **Should the Wizard be a top-level CTA or a secondary option?** The wizard is currently the dominant UI element. Promoting the direct-edit form may reduce wizard discoverability for new users.
2. **Where does "Profile Identity" (profile name + selector) live?** Currently split between the always-visible "Active Profile" selector and the "Profile Name" field inside Advanced. Should these merge into one always-visible identity card?
3. **Should Health Issues be promoted to a dedicated panel?** Currently nested inside Advanced. A dedicated card beneath the Profile panel (similar to Launcher Export) would eliminate multi-step expand-to-diagnose.
4. **Should environment variables be collapsed by default?** They are rarely edited per-session but can grow large. An opt-in expand with a count badge ("3 env vars set") would reduce visual weight.
5. **Sub-tabs vs. section containers**: Sub-tabs within the Profiles page (e.g., "Setup | Environment | Health") would reduce scroll but increase navigation complexity and may break the wizard's sequential flow mental model.
6. **Form completeness indicator**: Should a progress or readiness indicator (e.g., 3/5 required fields filled) appear inline on the panel header to guide first-time setup?

---

## Implementation Constraints

These constraints were identified through cross-team analysis and must govern any structural redesign:

### 1. `ProfileFormSections` is shared across two pages

`ProfileFormSections` is used in:

- `ProfilesPage.tsx` — main profile editor (no `reviewMode`)
- `InstallPage.tsx` — post-install profile review step (`reviewMode={true}`, inside `ProfileReviewModal`)

The `OnboardingWizard` only imports the `ProtonInstallOption` type from this file — it does not render the component. Any restructuring of `ProfileFormSections` (e.g., splitting into sub-components, adding sub-tabs) must be compatible with its `reviewMode` usage in `InstallPage`. Sub-tabs embedded inside `ProfileFormSections` would conflict with the modal context in `InstallPage` where there is no need for tabs — `reviewMode` is a modal review step, not a full editor. **Preferred approach**: keep tabs/panels at the `ProfilesPage` level (the page orchestrator), not inside `ProfileFormSections`.

### 2. Launch method gates section visibility — sub-tabs labeled by runner method would confuse users

The `launchMethod` field controls which sections render:

- `steam_applaunch`: AppID, Prefix Path, Proton Path, Launcher metadata, AutoPopulate, ProtonDB lookup
- `proton_run`: Prefix Path, Proton Path, Launcher metadata, Working Dir, ProtonDB lookup
- `native`: Working Dir only
- Trainer section: all methods except `native`

A sub-tab labeled "Steam Runtime" would be empty/hidden for `native` profiles. A sub-tab labeled "Trainer" would be empty for `native` profiles. Any tab-based approach must either: (a) hide tabs for irrelevant methods, (b) use generic labels ("Runtime Settings", "Trainer & Tools") that work across all methods, or (c) avoid tabs entirely in favor of collapsible panels that naturally collapse to zero height when empty.

### 3. ProtonDB and env vars must stay in the same visual zone

The `ProtonDbLookupCard` applies recommendations directly into `launch.custom_env_vars`, which is managed by `CustomEnvironmentVariablesSection`. If these are separated across tabs or distant panels, users must tab-switch to verify what was applied. They belong in the same panel or in adjacent sections within the same panel.

### 4. Action bar must remain persistent regardless of layout choice

`ProfileActions` (Save, Duplicate, Rename, Preview, Export, History, Delete) must remain visible at all times. Currently it is below the Advanced collapsible in the same card — this means it is visible even when Advanced is collapsed, but it operates on content the user cannot see. In any new layout, the action bar should be either:

- Anchored to the top-level panel (above or below the form content)
- Or sticky/fixed at the bottom of the viewport

It must never be inside a tab panel that becomes hidden.

### 5. Health Issues are diagnostic, not a form section

`CollapsibleSection title="Health Issues"` renders conditionally only when a profile has `broken` or `stale` status. It contains read-only metadata (last success time, launch count, failure count, drift warnings, per-field issue list). It does not contain any editable inputs. It should be treated as a status surface — positioned near the action bar or as a distinct diagnostic panel — not embedded inside the editable form flow.

### 6. The `reviewMode` prop controls launcher metadata visibility

`showLauncherMetadata = supportsTrainerLaunch && !reviewMode`. In `reviewMode`, Launcher Name and Launcher Icon fields are hidden. This is intentional — launcher metadata is not relevant during the install-flow review. Any refactor that splits launcher metadata into its own panel must preserve this conditional: launcher metadata panels should not render during `reviewMode`.

---

## Library & CSS Infrastructure Notes

These findings from API research confirm zero new dependencies are required for either layout approach:

### Radix Tabs already in use at the app level

`@radix-ui/react-tabs` v1.1.13 is installed and actively used in:

- `App.tsx` — `Tabs.Root orientation="vertical"` wraps the entire app shell
- `Sidebar.tsx` — `Tabs.List` + `Tabs.Trigger` drive page navigation
- `ContentArea.tsx` — `Tabs.Content` renders each page

The app's page routing IS the Radix Tabs primitive. Any within-page sub-tabs would be a nested `Tabs.Root` — Radix supports nested tab roots, but care is needed since the outer root uses `orientation="vertical"` and the inner would be `orientation="horizontal"`. The inner root must have a distinct `value`/`onValueChange` scope.

### Sub-tab CSS is already defined

`theme.css` already defines `.crosshook-subtab-row`, `.crosshook-subtab`, and `.crosshook-subtab--active` classes with full styling (pill shape, active gradient, transitions). `variables.css` already defines `--crosshook-subtab-min-height: 40px` and `--crosshook-subtab-padding-inline: 16px`. These classes are currently unused — they were designed in anticipation of within-page sub-tab navigation.

### Implication for layout decision

The existence of pre-built subtab CSS means the sub-tab approach (option 2 from the feature description) has lower implementation cost than previously assumed. However, the constraint from Implementation Constraints §1 still applies: sub-tabs must be composed at the `ProfilesPage` level, not inside `ProfileFormSections`, to avoid breaking the `InstallPage` modal reuse.

A hybrid approach is viable: discrete panels (option 1) for the primary layout restructure, with optional sub-tabs inside a single "Configuration" panel for the runner-specific sections (Steam Runtime / Proton Runtime / Native), using the pre-existing `.crosshook-subtab` classes with Radix `Tabs.Root`.

---

## UX Research Synthesis

Findings from UX research (`docs/plans/ui-enhancements/research-ux.md`) confirmed and integrated:

### Three-level collapse hierarchy violates NN/G two-level limit

The current nesting depth is:

1. `CollapsibleSection "Advanced"` — `defaultOpen=false`, wraps the entire editor (`ProfilesPage.tsx:622`)
2. `OptionalSection "Trainer details"` / `"Working directory override"` — native `<details>` inside `ProfileFormSections.tsx:778,1055,1111`; only collapsed in `reviewMode`
3. `CollapsibleSection "Health Issues"` — nested `CollapsibleSection` inside "Advanced" (`ProfilesPage.tsx:709`), `defaultOpen=true` but only rendered when profile is broken/stale

Nielsen Norman Group's guidance on progressive disclosure limits nesting to two levels before cognitive overhead outweighs the benefit. The current design reaches three levels in the worst case (Advanced > Trainer details > and separately Advanced > Health Issues when both are collapsed). **This is a concrete UX violation, not just a preference.**

### Competitive app patterns confirm task-oriented flat groupings

| App                    | Pattern                                                                | Lesson                                                         |
| ---------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------- |
| Heroic Games Launcher  | Wine/Proton + Performance + Launch Options + small "Advanced" residual | "Advanced" should be a small true residual, not the whole form |
| Lutris                 | Tabs per runner (Game, Runner, System, Wine)                           | Runner-specific settings deserve a dedicated visual area       |
| Bottles (GTK4)         | Sidebar categories per bottle; new features get prominent cards        | New/important features should not default-hide                 |
| macOS Ventura Settings | Sidebar list + content area grouped by user task                       | Group by task, not by skill level                              |
| VS Code Settings       | Flat list with section headers + search; no collapses within panes     | Flat expandable sections beat nested collapses                 |

### UX-recommended card groupings (task-oriented)

These align closely with the business analysis Proposed Section Groupings but use more task-centric naming:

| Card                 | Contents                                               | Notes                                                |
| -------------------- | ------------------------------------------------------ | ---------------------------------------------------- |
| Profile Identity     | Profile name, selector                                 | Selector already partially visible; consolidate here |
| Game & Runtime       | Game name/path, Runner Method, runtime-specific fields | Core of every profile; must be always-visible        |
| Trainer              | Trainer path, type, loading mode, version              | Keep co-located; only shown for non-native           |
| Environment & Launch | Custom env vars, ProtonDB lookup, working directory    | Power-user panel; collapsible with count badge       |
| Launcher Export      | Already separate — keep as-is                          | Existing separate panel pattern works                |
| Health & Diagnostics | Health issues, stale info, diagnostics                 | Promote from nested collapse to own panel            |

### Health badge disconnect is a known UX anti-pattern

Status badges (HealthBadge, OfflineStatusBadge, version badge) are currently in the `meta` slot of the Advanced `CollapsibleSection` header. They are visible without expanding, but the content they describe (Health Issues) is hidden. This is a "status indicator without context" anti-pattern — users see a red badge but cannot act on it without first expanding a different section, then scrolling, then expanding again. The fix is to co-locate the badge with the actionable content, either in a dedicated Health panel or directly adjacent to the profile selector where the user first looks.

---

## Security Constraints

From security research (`docs/plans/ui-enhancements/research-security.md`):

### Injection fields must remain UI-absent (W3)

`GameProfile.injection` (`dll_paths: string[]`, `inject_on_launch: boolean[]`) exists in the type and default state but is intentionally never rendered in any user-facing form component. These fields are populated exclusively by the install/migration pipeline. `exchange.rs:259` explicitly clears `dll_paths` during community export sanitization — this is active, intentional exclusion.

**Hard constraint**: any component reorganization that iterates over `GameProfile` keys or auto-renders fields from the profile type must explicitly exclude `injection.*`. This rules out any generic "render all profile fields" pattern as a shortcut during refactor.

### Path fields: free-form, backend-enforced, Browse affordances must be preserved

All path fields (`game.executable_path`, `trainer.path`, `steam.compatdata_path`, `steam.proton_path`, `runtime.prefix_path`, `runtime.proton_path`, `runtime.working_directory`, `steam.launcher.icon_path`) are free-form strings with no client-side path validation. This is correct for a launcher. Validation is backend-only. The Browse button pattern (Tauri dialog APIs, not string execution) is the correct UX affordance and must be preserved wherever path fields land in the restructured layout.

---

## Technical Design Constraints

From technical design research (`tech-designer`):

### `launch.*` is split across two pages — do not consolidate

`GameProfile.launch` contains fields served by two different pages:

- **ProfilesPage** renders: `launch.method` (Runner Method), `launch.custom_env_vars` (Custom Env Vars)
- **LaunchPage** renders: `launch.optimizations`, `launch.presets`, `launch.gamescope`, `launch.mangohud`

`ProfileFormSections` contains zero references to `gamescope`, `mangohud`, or `optimizations` (confirmed). The restructure must not move these to `ProfilesPage` — the LaunchPage multi-panel pattern (CollapsibleSection per feature: Gamescope, MangoHud, Launch Optimizations, Steam Launch Options) is the right model for those fields and already works well.

### Form state is a single `GameProfile` — tab-switching is safe

`ProfileContext` holds a single `profile: GameProfile` and `updateProfile: (updater) => void` and wraps the entire app. Switching between sub-tabs or panels within `ProfilesPage` carries zero risk of state loss — context stays mounted regardless of which panel is visible. The `onUpdateProfile` updater pattern `(current: GameProfile) => GameProfile` passes cleanly to any sub-component regardless of how sections are reorganized.

### `LauncherMetadataFields` and `TrainerVersionSetField` are local sub-components

Both are defined inside `ProfileFormSections.tsx` and not exported. If launcher metadata or trainer version fields move to a separate panel at the `ProfilesPage` level, these sub-components must either be extracted and exported, or the logic inlined at the call site. Neither is complex — extraction is straightforward.

### LaunchPage multi-panel pattern is the existing precedent for the restructure

`LaunchPage` already demonstrates the target architecture: discrete `CollapsibleSection className="crosshook-panel"` blocks for Gamescope, MangoHud, LaunchOptimizations, and SteamLaunchOptions, each receiving `profile` props and an `onUpdateProfile`-equivalent callback. `ProfilesPage` should adopt the same pattern, replacing the monolithic Advanced section with equivalent discrete panels.
