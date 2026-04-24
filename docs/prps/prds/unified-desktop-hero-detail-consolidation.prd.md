# CrossHook Hero Detail Consolidation — Profiles, Launch, and Sidebar Rework

## Problem Statement

The Unified Desktop Redesign (`docs/prps/prds/unified-desktop-redesign.prd.md`) shipped a three-pane shell with a Hero Detail mode that replaced the old blocking `GameDetailsModal`, and Phase 11 (`#464`) redesigned the standalone `Profiles` and `Launch` routes. Those two routes now duplicate functionality that belongs in Hero Detail: configuring **how a specific game runs** is a per-game workflow, not a top-level navigation concern. The cost is structural — users edit a profile in `/profiles`, then navigate to `/launch` to preview the command, then navigate back to `/library` to launch. Three route transitions per-game, zero of them carrying value. Worse, the sidebar reads like three flavors of the same job (Library · Profiles · Launch), which contradicts the redesign's "one place to do everything per game" goal. Today's Hero Detail has placeholder `ProfilesPanel` and `launch-options` tabs that render read-only summaries (`HeroDetailPanels.tsx:310-354, 436-450`), reinforcing the perception that Hero Detail is a preview and the real editors live elsewhere.

## Evidence

- **Design-bundle v2 from the same author/session** (`/tmp/crosshook-design-v2/crosshook/chats/chat1.md`, continuation on `2026-04-22`): _"It seems like having separate profiles/launch pages is redundant. Can you figure out a way to incorporate those functions into the hero detail page? So the user has one place to do everything?"_ — explicit user ask, with the solution already prototyped in `Detail.jsx` (666 lines) and two final screenshots committed to the PRD conversation.
- **Codebase confirms the duplication**: `Sidebar.tsx:69-72` lists `library · profiles · launch` as Game-group siblings. `ContentArea.tsx:40-43` mounts `<ProfilesPage />` and `<LaunchPage />` as first-class routes. `HeroDetailPanels.tsx:310-354` has `ProfilesPanel` — a read-only kv-list summary that short-circuits with _"No active profile loaded in the editor for this game"_ when the detail card's profile doesn't match `useProfileContext().selectedProfile`.
- **Route-change tax is measurable**: the Playwright `console chrome smoke` test at `tests/smoke.spec.ts:318-346` documents the current flow — click Launch tab in sidebar → select profile → click Profiles tab in sidebar → fill Game Path → click Launch — requiring **three route changes** to configure and launch one game.
- **Reusable editor components already exist and are prop-driven**: every `profile-sections/*Section.tsx` (Runtime, Game, Trainer, Media, Identity, RunnerMethod) takes `{profile, onUpdateProfile, ...}` as props with no context coupling. `ProfileSubTabs.tsx:85-309` and `LaunchSubTabs.tsx:21-250` likewise accept props instead of reading context. **Folding them into Hero Detail is composition, not reinvention.**
- **Autosave parity is achievable**: launch-opts / gamescope / trainer-gamescope / mangohud autosave at 350ms (`hooks/profile/constants.ts:2`). Env-vars autosave at 400ms (`useLaunchEnvironmentAutosave.ts:87`). All go through `persistProfileDraft` → `profile_save` IPC. Moving the editor surface into Hero Detail only requires mounting the same hooks.
- **Favorites is 95% wired**: `LibraryFilterKey` type already includes `'favorites'` (`types/library.ts:5`), `LibraryPage.tsx:99-103` already filters on it, and `LibraryToolbar.tsx:9-13` already renders the Favorites chip. `useProfileContext().favoriteProfiles` is the single source of truth. The only missing piece is a sidebar entry that navigates to Library and sets `filterKey='favorites'`.
- **Two final screenshots delivered by the user** (`2026-04-23`) lock the exact visual contract: no ProfileSwitcher dropdown in the hero (profile switching happens via the left-list card click in the Profiles tab), and the Launch tab is a three-section stack (Launch command · Environment · Pre/post hooks) — not the intermediate multi-panel JSX mock.

## Proposed Solution

Fold the Profiles and Launch editor surfaces into Hero Detail as fully editable tabs, delete the standalone `/profiles` and `/launch` routes, and formalize Library as the single Game navigation entry. The chosen shape is:

- **Hero Detail Profiles tab** — a two-pane editor. Left: per-game profile cards list (active badge, built-in badge, proton version, description, last used, health score, `+ New` CTA). Right: full editor for the selected profile, re-using the existing prop-driven `profile-sections/*Section` family (Runtime, Environment variables with on/off checkboxes, Pre-launch hooks, Launch args). Clicking a card calls `useProfileContext().selectProfile(name)` so the singleton provider aligns with the user's mental model — the edits autosave to the card they just clicked.
- **Hero Detail Launch tab** — a three-section single-column stack matching screenshot 2. Launch command block with syntax-highlighted token spans (binaries in accent-strong, values in success, env keys in warning) plus Dry-run · Copy · .desktop · Launch action row. Environment section reusing `CustomEnvironmentVariablesSection` with on/off checkboxes and a "3 ON" count badge. Pre/post hooks section with toggle-pill rows (stage pill: `pre-launch` / `post-exit`), driven by a new schema addition (`pre_launch_hooks: []`, `post_exit_hooks: []`).
- **Hero Detail Overview deep-links** — every panel button (`Edit launch config →`, `Manage hooks →`, `Open profile →`, `Edit env vars →`) sets the active Hero Detail tab, turning Overview into a genuine dashboard for the detail view.
- **Sidebar rework** — drop `Profiles` and `Launch` from the Game group (Library becomes the only Game entry). Add a **Favorites** declared section entry in Collections (filters Library to favorites). Add a **Currently Playing** declared section entry that filters Library to games with a running session. Keep all existing Dashboards, Community, Setup, and Settings entries as-is. The `AppRoute` union loses `'profiles' | 'launch'`.
- **Route deletion** — delete `ProfilesPage.tsx`, `LaunchPage.tsx`, `pages/profiles/`, `pages/launch/`, their RTL tests, and the three Playwright smoke blocks that target them. Rewire all twelve `onNavigate('profiles'|'launch')` callers to open Hero Detail + set activeTab.

Why this approach: the user-directed design (screenshots + `Detail.jsx` + chat) explicitly asks for one per-game place. Every editor component is already prop-scopeable, which reduces the fold-in to composition work. Autosave semantics are preserved by mounting the same hooks. The alternative — keeping both UIs in parallel behind a flag — doubles visual QA, invites drift between two code paths for the same action, and contradicts the source PRD's "calm desktop" goal.

## Key Hypothesis

We believe **folding profile editing and launch configuration into Hero Detail as live editable tabs, and removing the standalone routes that duplicate them** will **eliminate route-transition tax and give users one coherent per-game workspace** for **Linux gamers who configure and launch games through CrossHook**. We'll know we're right when **the sidebar has zero entries named `Profiles` or `Launch`, clicking a library card and pressing Enter opens a Hero Detail where every editor field autosaves with parity to today's standalone routes, switching active profile via the Profiles tab left-list card click updates the whole detail view in ≤200ms, and the Playwright smoke "launch + edit + launch" flow for a single game requires zero sidebar navigations**.

## What We're NOT Building

- **Keeping the old `/profiles` and `/launch` routes behind a flag** — user chose full deletion. Parallel UIs would invite drift and doubled QA.
- **A hero ProfileSwitcher dropdown** — the final screenshots show no hero-aside profile card. Profile switching happens inside the Profiles tab via card clicks in the left list. The intermediate `Detail.jsx` mock with a dropdown is superseded.
- **New runtime execution for pre/post hooks** — we add the schema fields (`pre_launch_hooks`, `post_exit_hooks`) and the UI to declare/toggle/remove them, but runtime execution (spawning `backup-saves.sh` on game exit, etc.) is deferred to a follow-up PRD. The panel ships with a clear "Declared, not yet executed" help banner so users aren't misled.
- **A new ProfileProvider variant** — we stick with the singleton `ProfileProvider` and align it to the active card via `selectProfile(name)` on tab mount / card click. Per-tab scoped providers are a scope-creep escape hatch we explicitly reject.
- **A new syntax-highlighter dependency** — we hand-roll the colored token spans using the backend's existing structured `LaunchPreview` fields (`wrappers`, `env`, `effective_command`). No `shiki` / `prism` / `cmdk` — matches CLAUDE.md dependency hygiene.
- **Profiles-page-level features that don't appear in the screenshots** — no wizard flow inside Hero Detail, no collection-picker inside the Profiles tab (those stay on their own surfaces or move to Settings). The Hero Detail editor's scope is exactly the screenshots.
- **Backend logic changes beyond the hook schema** — `crosshook-core` persistence is extended to serialize the new optional hook arrays; launch/runtime logic, `#[tauri::command]` surfaces, and platform.rs remain untouched.
- **Alternate Collection types** — the sidebar gets `Favorites` and `Currently Playing` as new declared entries. Beyond that, collections stay exactly as they are today.

## Success Metrics

| Metric                                  | Target                                                                                           | How Measured                                                                          |
| --------------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| Sidebar lacks Profiles/Launch           | 0 `role="tab"` elements whose accessible name is "Profiles" or "Launch" inside the sidebar       | Playwright smoke assertion + grep of `SIDEBAR_SECTIONS`                               |
| `AppRoute` union lacks profiles/launch  | 0 source references to `'profiles'` or `'launch'` as `AppRoute` values                           | `grep -rn "'profiles'\|'launch'" src/ \| grep -v "test\|__tests__"` → 0               |
| Profiles tab editor autosave parity     | Every field that autosaves today via `/profiles` also autosaves via Hero Detail Profiles         | Extend RTL Profiles test-pack to run against `HeroDetailProfilesTab` instead          |
| Launch tab editor autosave parity       | Env vars, launch optimizations, gamescope, mangohud autosave within the same debounce            | RTL tests assert `persistProfileDraft` called within 350/400ms                        |
| Profile switch latency                  | ≤200ms from card click in Profiles tab to updated hero pills, command preview, env display       | Vitest perf test mocks `selectProfile` + asserts render cycle                         |
| Zero route changes for configure+launch | Playwright flow: click Library card → open detail → edit profile field → launch — no sidebar nav | Rewritten `console chrome smoke` test asserts `[aria-current="page"]` stays `library` |
| Syntax-highlighted command preview      | All three token classes render (`binary`, `value`, `env-key`) with distinct theme colors         | RTL test snapshots the token classnames                                               |
| Favorites sidebar entry works           | Clicking Favorites entry navigates to Library + sets `filterKey='favorites'` + chip active       | Playwright smoke: click entry, assert `[aria-pressed="true"]` on Favorites chip       |
| Currently Playing sidebar entry works   | Clicking entry navigates to Library + filterKey='currentlyRunning' + chip present                | Playwright smoke: click entry, assert new chip active                                 |
| No regressions in launched routes       | All Dashboards, Community, Install, Settings, Discover routes still pass smoke                   | Existing `ROUTE_ORDER` smoke sweep minus the deleted entries                          |
| Pre/post hooks schema round-trips       | Saving a profile with `pre_launch_hooks`/`post_exit_hooks` and reloading preserves entries       | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`        |
| Hero Detail tests cover new tabs        | `HeroDetailPanels.test.tsx` has ≥3 cases each for Profiles-edit and Launch-edit modes            | Vitest                                                                                |

## Resolved Decisions

- [x] **Route fate** — delete `/profiles` and `/launch` entirely; rewire all onNavigate callers to Hero Detail + set tab. Resolved in PRD interactive session (`2026-04-23`).
- [x] **Save model** — autosave, matching Phase 11 behavior. No Save/Cancel cliffs. Resolved in PRD interactive session.
- [x] **Favorites scope** — sidebar entry AND Library filter chip (filter chip already exists). Resolved in PRD interactive session.
- [x] **Pre/post hooks panel** — build in this PRD. Schema addition ships; runtime execution is out of scope. Resolved in PRD interactive session.
- [x] **Syntax highlighting** — token-based inline spans using backend's structured LaunchPreview. No new dependency. Resolved in PRD interactive session.
- [x] **Currently Playing sidebar entry** — add as a declared section entry with a new `filterKey='currentlyRunning'` in `LibraryFilterKey`. Resolved in PRD interactive session.
- [x] **Profile scope in Hero Detail Profiles tab** — call `selectProfile(name)` on card click; keep the singleton ProfileProvider. Resolved in PRD interactive session.
- [x] **ProfileSwitcher in hero** — omit. Screenshots do not include a hero-aside switcher card. Resolved after reviewing user-supplied screenshots in the PRD session.
- [x] **Launch tab shape** — three sections (Launch command · Environment · Pre/post hooks). Omits the earlier `Detail.jsx` mock's Launcher-stack-reordering, Arguments, Prefix & paths panels. Resolved after screenshot review.

## Open Questions

- [ ] **Pre/post hook schema** — do we expose arbitrary script paths (shell + DLL) as a single `LaunchHook` struct with an enum `stage`, or split into two distinct types? Proposal: single struct with `stage: "pre-launch" | "post-exit"` enum. Confirm before Phase 5 implementation.
- [ ] **Post-install redirect target** — `InstallPage.tsx:289` redirects to `/profiles` today. When `/profiles` is gone, redirect to `/library` with the new profile auto-selected and Hero Detail opened on the Profiles tab? Or just to `/library`? Recommend: Library + auto-open Hero Detail for the newly-installed game.
- [ ] **Collection launch-defaults editor** — `CollectionLaunchDefaultsEditor.tsx:69/83/301` has an "Open in Profiles Page" button. Since the page is deleted, does the modal retain its own editor UI (it already does) or should that button just close the modal now? Recommend: close the modal.
- [ ] **Trainer tab** — Hero Detail already has a Trainer tab with a read-only view (`HeroDetailPanels.tsx:451-484`). Does this PRD upgrade it to an editor that matches the trainer section from `TrainerSection.tsx`, or stays read-only? Recommend: upgrade to editor in Phase 4 (Profiles tab) since trainer fields live in the same TOML namespace as runtime fields; avoid two editors for one profile.
- [ ] **"Currently Playing" mechanism** — does `useGameRunning` / `LaunchSessionRegistry` expose a reactive list of running-game profile names today, or do we need new glue? Confirm before Phase 2.

---

## Users & Context

**Primary User**

- **Who**: Linux gamers using CrossHook as a primary game launcher, with multiple profiles per game (e.g. RDR2 Enhanced vs. Safe mode vs. Streaming). They want to click a game, tweak a profile, maybe switch to a different profile for this session, and launch — all without losing sight of the game's cover, health, history, or hero context.
- **Current behavior**: Click game card → Hero Detail opens → realize edits can't be made there → dismiss → sidebar → Profiles → profile selector → edit → sidebar → Launch → preview → launch. Or: skip Hero Detail entirely, use sidebar Profiles route as the "real" editor, and treat Hero Detail as a glorified preview.
- **Trigger**: Needing to toggle an env var, swap Proton version, or declare a pre-launch script for a specific game — tasks that fundamentally belong to that game, not the app as a whole.
- **Success state**: The Hero Detail is where every per-game configuration decision happens. Switching profiles is a card click. Tweaking env is an on/off checkbox. Seeing the launch command is a tab switch. Launching is a button in the hero. No sidebar round-trips.

**Job to Be Done**

When **I open a game in CrossHook**, I want to **configure and launch it — including swapping profiles, toggling env, editing hooks, and previewing the command — all in the game's own detail view**, so I can **treat each game as a self-contained workspace instead of navigating a settings app**.

**Non-Users**

- Not for users who navigate by app-level commands rather than per-game — those users have `⌘K` (the Phase 6 palette) and aren't harmed by the consolidation.
- Not for CLI-only users of `crosshook-cli` — this is a frontend-only rework.
- Not for users who expect a separate "global profile library" view — profiles in CrossHook are always per-game; the standalone `/profiles` route was a navigation detour, not a distinct mental model.

---

## Solution Detail

### Core Capabilities (MoSCoW)

| Priority | Capability                                                                                                | Rationale                                                                                        |
| -------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Must     | Hero Detail Profiles tab with two-pane editor (cards list + selected-profile editor)                      | Core of the consolidation. Without this, the rest is cosmetic.                                   |
| Must     | Hero Detail Launch tab with three sections (Launch command · Environment · Pre/post hooks)                | Replaces the standalone Launch route functionally.                                               |
| Must     | `selectProfile(name)` on Profiles tab card click                                                          | Keeps the singleton ProfileProvider aligned; no new abstraction.                                 |
| Must     | Autosave parity with Phase 11 (`profile_save`, 350/400ms debounces, status toasts)                        | Same save semantics users already have — zero surprise.                                          |
| Must     | Delete `ProfilesPage.tsx`, `LaunchPage.tsx`, their subdirs, their RTL tests                               | Completes the consolidation; removes the duplicate UI.                                           |
| Must     | Remove `'profiles'` / `'launch'` from `AppRoute`, `SIDEBAR_SECTIONS`, `ROUTE_METADATA`, `ROUTE_NAV_LABEL` | Source-of-truth cleanup; compiler catches stragglers.                                            |
| Must     | Rewire all 12 `onNavigate('profiles'\|'launch')` callers to Hero Detail + `setActiveTab`                  | Prevents dead nav; preserves collection-modal / health-dashboard / palette flows.                |
| Must     | Rewire command-palette `launch_profile` / `edit_profile` to Hero Detail                                   | `AppShell.tsx:223-239` dispatch path; otherwise palette crashes on deletion.                     |
| Must     | Favorites sidebar entry (Collections section) + wire to existing `filterKey='favorites'`                  | 95% wired; only needs the sidebar control.                                                       |
| Must     | Currently Playing sidebar entry + new `filterKey='currentlyRunning'` filter                               | Screenshot shows the entry; filter is new but small.                                             |
| Must     | Syntax-highlighted command preview block in Launch tab                                                    | Screenshot 2 shows it; plain `<pre>` doesn't match.                                              |
| Must     | Pre/post hook schema fields in profile TOML (declared, not yet executed)                                  | Screenshot 2 shows Pre/post hooks panel. Schema ships; runtime is followup.                      |
| Must     | Pre/post hook panel UI (add · toggle · stage pill · remove)                                               | Schema alone isn't useful without UI.                                                            |
| Must     | Overview-tab panel deep-link buttons that set `activeTab`                                                 | Overview becomes a dashboard for Hero Detail; matches screenshot.                                |
| Must     | Updated Playwright smoke covering: new sidebar, no /profiles /launch, edit flows, favorites, running      | Prevents regression and locks the new shape.                                                     |
| Must     | Updated `HeroDetailPanels.test.tsx` + `GameDetail.test.tsx` for new tabs                                  | Existing files already set up with the right providers; extension is cheap.                      |
| Should   | Profile card health score + last-used label in the left list                                              | Matches screenshot 1; data is already in `useProfileSummaries`.                                  |
| Should   | Profile "built-in" badge for read-only profile templates                                                  | Screenshot 1 shows it on Safe mode. If no backend flag exists, derive from name prefix or defer. |
| Should   | Profile "active" pill on the currently-active card                                                        | Screenshot 1 shows it. `profileName === activeName` check.                                       |
| Should   | Data-testid hooks on Hero Detail Profiles / Launch tab roots for smoke                                    | Makes smoke tests less CSS-class-brittle.                                                        |
| Could    | Hero Detail Trainer tab upgraded to an editor (vs. today's read-only view)                                | Eliminates the last read-only tab. May require its own phase.                                    |
| Could    | Duplicate-profile action inside the Profiles tab editor header                                            | Screenshot shows `+ Duplicate` button; `profile_duplicate` IPC exists.                           |
| Could    | "Manage all profiles" jump from Overview to Profiles tab                                                  | Convenience deep-link; thin.                                                                     |
| Won't    | Runtime execution of pre/post hooks                                                                       | Out of scope. Schema fields declared but not executed.                                           |
| Won't    | Hero ProfileSwitcher dropdown                                                                             | Screenshots show no such control. Tab-only switching.                                            |
| Won't    | Launch tab Launcher-stack reordering / Arguments / Prefix & paths panels                                  | Screenshots don't include them. Those fields live in the Profiles tab Runtime section.           |
| Won't    | New highlighter dependency (shiki/prism)                                                                  | Hand-rolled spans; zero dep cost.                                                                |
| Won't    | URL routing or back/forward history                                                                       | Source PRD ruled it out; no change here.                                                         |
| Won't    | Global profile library view (browse all profiles across games)                                            | Not in the design; not in the screenshots.                                                       |

### MVP Scope

**Minimum to validate the hypothesis**: Phases 1–4 (foundation + sidebar cleanup + Profiles editor tab + Launch editor tab). After those four phases land, the sidebar no longer has Profiles/Launch, Hero Detail has editable Profiles and Launch tabs with autosave, and the consolidation story is visible end-to-end. Phase 5 (pre/post hooks schema) is the first non-MVP must-have — it's required to honor the screenshot fully but doesn't block the hypothesis proof. Phases 6–11 amplify and polish.

### User Flow

**Critical path — configure and launch a game from cold start**:

1. User opens CrossHook. Library is the Game-group default (now the only Game entry).
2. Double-click card for `Red Dead Redemption 2` → Hero Detail mode takes over the main slot; sidebar + inspector stay mounted.
3. Profiles tab is a tap away. Left-list shows three cards: Enhanced (active), Safe mode (built-in), Streaming. User clicks Streaming → `selectProfile('rdr2-stream')` fires; hero pills update (`GE-Proton 9-15` → same or different), command preview updates, env list swaps; right editor shows Streaming's fields.
4. User toggles `MANGOHUD` env var off. Autosave fires after 400ms. Hero pill updates. Command preview re-renders without MANGOHUD.
5. User clicks Launch tab. Sees colored command preview, env list, and the Pre/post hooks section (empty for this profile).
6. User clicks back to the hero's Launch button. Game launches with the Streaming profile, MANGOHUD off.

**Total sidebar clicks**: 0. Before this PRD: 3 (Library → Profiles → Launch → back to Library).

**Favorites / Currently Playing paths**:

- **Favorites**: sidebar → Favorites (Collections section). Navigates to Library, toolbar Favorites chip is `aria-pressed="true"`, grid filtered. Click again to return to Library default.
- **Currently Playing**: sidebar → Currently Playing. Navigates to Library, new Running chip active, grid filtered to games with a running session (via `LaunchSessionRegistry` reactive state).

### Responsive contract

Hero Detail's existing responsive behavior is preserved:

- **Profiles tab two-pane layout** collapses to one-column on `deck` (left list above editor).
- **Launch tab** stays single-column at all breakpoints (matches screenshot 2).
- **Pre/post hooks panel** rows wrap on narrow viewports.
- **Command preview block** scrolls horizontally on narrow viewports; never wraps the command (preserving copy-paste fidelity).

---

## Technical Approach

**Feasibility**: MEDIUM overall. Profiles tab two-pane is MEDIUM (reuses `profile-sections/*`). Launch tab is MEDIUM (env section reuses `CustomEnvironmentVariablesSection`). Pre/post hooks schema is LOW-MEDIUM (TOML additions + serde). Command highlighting is LOW (span composition). Route deletion + rewiring is MEDIUM (12 callers, 3 smoke blocks). `selectProfile` wiring on card click is LOW (single hook call).

**Architecture Notes**

- **Hero Detail tab state extension** — `HeroDetailPanels.tsx` props grow from 15 to ~18 fields: add `profile: GameProfile | null`, `updateProfile: (draft) => Promise<void>`, and `profileList: ProfileSummary[]` (for the left-list cards). Mutation-capable tabs depend on all three. `GameDetail.tsx:117-150` extends `panelProps` accordingly.
- **Profiles tab component** — new `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx` (~350 lines). Left pane: `<ProfileCardsList profiles={...} activeId={...} onSelect={(name) => selectProfile(name)} />`. Right pane: `<ProfileEditor profile={profile} profileName={profileName} onUpdateProfile={updateProfile} ... />`, which is a thin wrapper that mounts `ProfileIdentitySection` + `RuntimeSection` + `GameSection` + `TrainerSection` + `MediaSection` stacked with dividers. Do **not** mount `ProfileSubTabs` — its tab nesting competes with Hero Detail's tab nesting. Flatten the sections into a single scrollable column.
- **Launch tab component** — new `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx` (~300 lines). Section 1: `<HighlightedCommandBlock preview={...} actions={['dry-run','copy','desktop','launch']} />`. Section 2: reuse `CustomEnvironmentVariablesSection` (imported from `launch-subtabs/EnvironmentTabContent.tsx:6`). Section 3: new `<HookListPanel hooks={pre} stage="pre-launch" />` + `<HookListPanel hooks={post} stage="post-exit" />` sharing an `onUpdateProfile` callback.
- **`HighlightedCommandBlock`** — new `src/crosshook-native/src/components/library/HighlightedCommandBlock.tsx` (~120 lines). Input: `preview: LaunchPreview`. Output: a `<pre>` composed of spans with classes `.crosshook-hero-detail__cmd-token--env-key`, `--value`, `--binary`, `--flag`, `--comment`. Composition from `preview.wrappers`, `preview.environment`, `preview.effective_command`. No string parsing — structured tokens already exist backend-side.
- **`HookListPanel`** — new `src/crosshook-native/src/components/library/HookListPanel.tsx` (~150 lines). Renders a list of `{ name, path, stage, enabled }` rows with toggle switch + stage pill + settings gear (opens a small editor popover). Autosave-fires profile_save via `onUpdateProfile` with the new `pre_launch_hooks` / `post_exit_hooks` arrays. 400ms debounce.
- **Profile schema extension** — add to `crosshook-core/src/profile/`:

  ```rust
  #[derive(Clone, Debug, Default, Serialize, Deserialize)]
  #[serde(default)]
  pub struct LaunchHook {
      pub id: String,
      pub name: String,
      pub path: String,
      pub stage: HookStage,
      pub enabled: bool,
  }

  #[derive(Clone, Debug, Default, Serialize, Deserialize)]
  #[serde(rename_all = "kebab-case")]
  pub enum HookStage { #[default] PreLaunch, PostExit }
  ```

  Add `pre_launch_hooks: Vec<LaunchHook>` and `post_exit_hooks: Vec<LaunchHook>` fields to the profile struct with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. Migration: additive fields only — old profiles load with empty vecs. **Runtime execution is NOT added in this PRD.** A `// TODO(hooks-runtime): consume in launcher` comment with a follow-up issue ref.

- **Sidebar cleanup** — `Sidebar.tsx:63-105` `SIDEBAR_SECTIONS`:
  - Game section: drop `profiles`, `launch`. Leave only `library`.
  - Collections section: append declared entries for `favorites` and `currently-playing`. These are NOT `AppRoute` values — they're sidebar-only targets that dispatch `navigateWithFilter({ route: 'library', filterKey: ... })`. Avoids bloating the `AppRoute` union.
  - Update `SidebarSection` type to allow a third variant `type: 'library-filter'` with `filterKey: LibraryFilterKey`.
- **AppRoute** — `Sidebar.tsx:20-31`: remove `'profiles'` and `'launch'` from the union. The TypeScript compiler will flag every remaining reference — that list becomes the Phase 6 rewire checklist.
- **Route deletion** — delete `pages/ProfilesPage.tsx`, `pages/LaunchPage.tsx`, `pages/profiles/`, `pages/launch/`, `routeMetadata.ts:52-65` entries for both routes, `ContentArea.tsx:40-43` cases, `routeMetadata.ts:129-130` ROUTE_NAV_LABEL entries. Clean up unused `ProfilesIcon`, `LaunchIcon` imports if not used elsewhere.
- **Nav rewiring** — every `onNavigate('profiles'|'launch')` caller becomes `onNavigate('library')` + `setSelectedGame(name) + setHeroDetailTab(...)`. The navigate callback signature grows to `onNavigate(route: AppRoute, opts?: { profileName?: string; heroDetailTab?: HeroDetailTabId; libraryFilter?: LibraryFilterKey })`. AppShell handles the opts by dispatching into Library state.
- **Favorites + Currently Playing filter chips** — `LibraryToolbar.tsx:9-13` `FILTER_OPTIONS` already has `favorites`. Append `{ key: 'currentlyRunning', label: 'Running' }`. `LibraryPage.tsx:99-103` switch adds `case 'currentlyRunning': list = list.filter((p) => runningProfileSet.has(p.name));`.
- **Running-profile state** — a new hook `src/crosshook-native/src/hooks/useRunningProfiles.ts` that subscribes to `LaunchSessionRegistry` (or polls `check_game_running` for each card) and returns `Set<string>` of active profile names. Used by LibraryPage filter + Currently Playing sidebar badge count.
- **Command palette rewiring** — `AppShell.tsx:223-239` `handleExecuteCommand`:
  - `launch_profile`: open Hero Detail for the profile's game card with `activeTab='launch-options'`, then fire `onLaunch`.
  - `edit_profile`: open Hero Detail with `activeTab='profiles'` and the card already selected.
  - `lib/commands.ts` payloads unchanged (still `{ action, profileName }`).
- **Overview deep-links** — `HeroDetailPanels.tsx` overview case (L416-433) gains deep-link buttons in each kv-section. Each button calls `onSetActiveTab(targetTab)`. Thread `onSetActiveTab` through `GameDetail → HeroDetailTabs → HeroDetailPanels`.
- **Tests**:
  - `HeroDetailPanels.test.tsx` — extend renderer with `ProfileProvider` wrapper, add cases for mode `'profiles'` (edit proton version, assert `profile_save` call after 350ms) and mode `'launch-options'` (toggle env var, assert persistProfileDraft call after 400ms; toggle pre-launch hook, assert schema field write).
  - `GameDetail.test.tsx` — add card-click-switches-profile case: render with 2 profiles in `profileList`, click second card, assert `selectProfile` called with second name and hero pills updated.
  - Delete `ProfilesRoute.test.tsx`, `LaunchRoute.test.tsx`.
  - Playwright `smoke.spec.ts`: remove `'profiles'` and `'launch'` from `ROUTE_ORDER` (L35-36); remove the `launch pipeline smoke` block (L189-203); remove `profiles + launch panel landing smoke` (L205-251); rewrite `console chrome smoke` (L307-358) to use Hero Detail flow. Add new smoke: `library → double-click card → Hero Detail Profiles tab → edit runtime → profiles tab autosave status → Launch tab → verify preview → click hero Launch`.

**Dependencies / Integration points**

- `@radix-ui/react-tabs` (already used in `HeroDetailTabs.tsx:1`) — no change.
- `react-resizable-panels` — unchanged.
- `useProfileContext`, `useProfile`, `useProfileCrud` — unchanged public surface; just called from a new caller (Hero Detail).
- `profile_save`, `profile_load`, `profile_duplicate`, `profile_list`, `profile_list_favorites`, `profile_set_favorite` — unchanged IPC surfaces.
- `check_game_running` / `LaunchSessionRegistry` — consumed by new `useRunningProfiles` hook; no IPC changes.
- No new npm dependencies. No new Tauri commands beyond existing ones used by the old routes. The `profile_save` IPC already round-trips the full profile TOML, so the new hook fields piggyback.

### Persistence & usability

Per CLAUDE.md, classify each datum introduced by this feature:

- **TOML settings (per-profile schema in profile.toml, via crosshook-core)**: `pre_launch_hooks: Vec<LaunchHook>` and `post_exit_hooks: Vec<LaunchHook>`. Each `LaunchHook` contains `{ id, name, path, stage, enabled }`. These are user-editable per profile. Additive with serde defaults — old profiles load to `[]`.
- **SQLite metadata**: none directly. Running-profile state (`currentlyRunning` filter) consumes the existing `LaunchSessionRegistry` in memory; does NOT need a new table.
- **Runtime-only**: Hero Detail active-tab (`HeroDetailTabId`) per session. Selected card in the Profiles tab left list (aligned with `useProfileContext().selectedProfile`). Launch command preview per session (already cached by `usePreviewState`). Favorites filter state stays in-memory in `LibraryPage` via existing `filterKey`. `currentlyRunning` filter state same.

**Migration/backward compatibility**: profile TOML is additive — no migration required. Old profiles deserialize with empty hook vectors. New profiles serialize hooks only when non-empty (`#[serde(skip_serializing_if = "Vec::is_empty")]`). Users who saved profiles before this PRD see the Hero Detail tabs with empty Pre/post hooks sections until they add entries. SQLite schema unchanged — `migrations.rs` is not touched.

**Offline behavior**: fully offline. The feature depends only on local profile TOML and in-memory session state.

**Degraded fallback**: if `profile_save` fails, the existing autosave error-toast path surfaces the error (precedent: `useProfileLaunchAutosaveEffects.ts` sets `setStatus({ tone: 'error', label: 'Failed to save', detail })`). If `LaunchSessionRegistry` is unavailable, `useRunningProfiles` returns an empty set — Currently Playing filter shows no matches rather than crashing. If the `HookListPanel` receives malformed hook entries (e.g. missing `id`), it renders a "Invalid hook" row with a remove button.

**User visibility/editability**: every new field is visible in Hero Detail. The pre/post hook rows are directly editable (name, path via a popover, stage toggle, enabled toggle, remove). Hook execution being deferred is called out with a single-line info banner in the Pre/post hooks section (`These hooks are saved to your profile. Runtime execution is coming in a future release.`). TOML settings like `sidebar_variant_override` (from the source PRD) are unchanged.

**Technical Risks**

| Risk                                                                                                     | Likelihood | Mitigation                                                                                                                                                                                                  |
| -------------------------------------------------------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Autosave regression when mounting editor sections in a new parent                                        | M          | Reuse the EXACT component + hooks from `/profiles`. Write RTL tests that mirror the old `ProfilesRoute.test.tsx` assertions against the Hero Detail tab before deleting the old tests.                      |
| `selectProfile()` side-effect races with autosave in flight                                              | M          | `useLaunchEnvironmentAutosave.ts:73-75` already guards `latestProfileNameRef.current !== scheduledProfileName`. Verify the same guard exists in `useProfileLaunchAutosaveEffects.ts`; if not, add it.       |
| Rewire of `onNavigate('profiles'\|'launch')` callers misses a callsite                                   | M          | Delete `'profiles'` and `'launch'` from `AppRoute` **before** the rewire PR lands — TypeScript compile errors enumerate every callsite. Don't merge the rewire PR until `tsc` is clean.                     |
| Playwright smoke tests break because of CSS class changes (`.crosshook-profiles-page__body`)             | H          | Delete the asserting blocks in the same PR as the route deletion. Smoke tests for the Hero Detail flow replace them. Run `npm run test:smoke` before PR merge.                                              |
| Hook schema addition breaks existing profile load on older CrossHook builds                              | L          | `#[serde(default)]` on struct + fields + `skip_serializing_if = "Vec::is_empty"` on serialization keeps old profiles readable by new code AND new profiles readable by old code (they ignore unknown keys). |
| Duplicate UI paths linger if we don't delete `/launch` cleanly                                           | M          | Phase 6 is route deletion, not just "hidden from sidebar". Explicit PR checklist: `ProfilesPage.tsx` + `LaunchPage.tsx` physically removed from git.                                                        |
| Pre/post hooks look runnable but aren't — users configure scripts expecting them to execute              | M          | Clear "Declared, not yet executed" banner in the Pre/post hooks section + a tooltip on the stage pill. Track the runtime execution as an open GitHub issue referenced in-product.                           |
| Command-preview highlighter misclassifies tokens (e.g. `mangohud` is env var or binary?)                 | M          | Use the structured preview from backend (`preview.wrappers`, `preview.environment`) rather than regex parsing. No ambiguity — binaries come from `wrappers`, env keys from `environment`.                   |
| Favorites sidebar entry duplicates Library/Favorites chip                                                | L          | Intentional; the chip is inside Library's toolbar and the sidebar entry is app-level. Clicking either toggles the same `filterKey` state. Test asserts both-in-sync.                                        |
| Currently Playing sidebar entry shows stale count after game exits                                       | M          | `useRunningProfiles` subscribes to `LaunchSessionRegistry` events. Playwright smoke exercises launch → exit → sidebar badge decrements.                                                                     |
| `HeroDetailProfilesTab` per-card render is expensive with many profiles                                  | L          | Virtualize only if profile count exceeds ~50 per game. Today's real-world distribution is 3–5 per game. Don't pre-optimize.                                                                                 |
| Deleting `LaunchPage.tsx` orphans `RouteBanner route="launch"` asset / icon                              | L          | Search for every `route="launch"` reference after deletion; clean up.                                                                                                                                       |
| Current `HeroDetailPanels` uses `ProfilesPanel` short-circuit — rewiring without breaking loading states | M          | Retain the `loadState === 'loading'` / `'error'` handling in the new Profiles tab. Use the same `useGameDetailsProfile` hook already in `GameDetail.tsx:43`.                                                |

---

## Implementation Phases

<!--
  STATUS: pending | in-progress | complete
  PARALLEL: phases that can run concurrently
  DEPENDS: phases that must complete first
  PRP: link to generated plan file once created
-->

| #   | Phase                                                                                | Description                                                                                                                                                                                   | Status  | Parallel | Depends | PRP Plan |
| --- | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- | -------- | ------- | -------- |
| 1   | Hero Detail panel contract expansion + test harness                                  | Extend `HeroDetailPanelsProps` with `profile`, `updateProfile`, `profileList`, `onSetActiveTab`. Thread through `GameDetail` + `HeroDetailTabs`. Add fixtures. No UI change.                  | pending | -        | -       | -        |
| 2   | Sidebar cleanup + Favorites + Currently Playing + new `currentlyRunning` filter      | Drop `profiles`/`launch` from `SIDEBAR_SECTIONS`. Add `SidebarSection` library-filter variant. Add Favorites + Currently Playing entries. Wire filter chips + `useRunningProfiles`.           | pending | with 3   | 1       | -        |
| 3   | Pre/post hook schema (crosshook-core additive fields + serde round-trip test)        | Add `LaunchHook`, `HookStage`, `pre_launch_hooks`, `post_exit_hooks` to profile struct. `cargo test` round-trip. No launcher consumption yet.                                                 | pending | with 2   | -       | -        |
| 4   | Hero Detail Profiles tab (two-pane editor)                                           | New `HeroDetailProfilesTab.tsx`. Left list + flattened editor reusing `profile-sections/*`. `selectProfile()` on card click. Replace `ProfilesPanel` in panels switch. Autosave parity tests. | pending | -        | 1       | -        |
| 5   | Hero Detail Launch tab (3-section stack) + highlighted command preview               | New `HeroDetailLaunchTab.tsx` + `HighlightedCommandBlock.tsx`. Reuse `CustomEnvironmentVariablesSection`. Hook panel stub (no hook data yet). Replace `launch-options` panel.                 | pending | with 4   | 1       | -        |
| 6   | Hero Detail Pre/post hooks panel (live UI against new schema)                        | New `HookListPanel.tsx`. Wire to `pre_launch_hooks` / `post_exit_hooks` via `onUpdateProfile`. Declared-not-executed banner. Test round-trip.                                                 | pending | -        | 3, 5    | -        |
| 7   | Overview tab deep-links                                                              | Add "Edit launch config →" / "Open profile →" / "Manage hooks →" / "Edit env vars →" buttons that call `onSetActiveTab`.                                                                      | pending | with 6   | 4, 5    | -        |
| 8   | `AppRoute` union shrink + TypeScript cleanup                                         | Remove `'profiles'` and `'launch'` from `AppRoute`. Let `tsc` enumerate every remaining reference. Prepare rewire checklist.                                                                  | pending | -        | 4, 5    | -        |
| 9   | Nav rewire — all onNavigate + palette handlers                                       | Update LibraryGrid/LibraryList/LibraryPage/HealthDashboardPage/InstallPage/CollectionViewModal/CollectionLaunchDefaultsEditor/AppShell palette to open Hero Detail + set tab.                 | pending | -        | 8       | -        |
| 10  | Route deletion — ProfilesPage, LaunchPage, subdirs, RTL tests, ContentArea cases     | Physical removal from git. Clean unused icons / banner assets. Ensure `npm test` stays green.                                                                                                 | pending | -        | 9       | -        |
| 11  | Playwright smoke rewrite — Hero Detail flows, new sidebar entries, regression guards | Remove old profiles/launch smoke blocks. Add Hero Detail launch flow, profiles edit flow, env toggle flow, Favorites + Currently Playing sidebar flows. Add `AppRoute` regression grep.       | pending | -        | 10      | -        |
| 12  | Polish + docs + release notes                                                        | Ensure `docs/internal-docs/design-tokens.md` lists new command-preview token classes. Release-notes entry. Remove dead assets (`RouteBanner route="launch"` if unused).                       | pending | -        | 11      | -        |

### Phase Details

**Phase 1: Hero Detail panel contract expansion + test harness**

- **Goal**: Every subsequent phase can mutate the profile without refactoring the prop pipeline.
- **Scope**: Extend `HeroDetailPanelsProps` (`HeroDetailPanels.tsx:18-34`) with four new optional fields: `profile: GameProfile | null`, `updateProfile: (draft: GameProfile) => Promise<void>`, `profileList: ProfileSummary[]`, `onSetActiveTab: (tab: HeroDetailTabId) => void`. Update `GameDetail.tsx:117-150` to thread them through `panelProps`. Update `HeroDetailTabs.tsx:8` prop type. Update `HeroDetailPanels.test.tsx` factory at L144-165 to accept them (with sensible defaults so existing cases don't need rewriting). Add a single "no-op default" test asserting that omitting `updateProfile` still renders the read-only panels. Add `data-testid="hero-detail-profiles-tab"` and `"hero-detail-launch-tab"` to the tab root divs.
- **Success signal**: `npm test` green. No visible UI change. `HeroDetailPanelsProps` compiles with the new fields. Existing RTL tests untouched.
- **Files touched (approx)**: `library/HeroDetailPanels.tsx`, `library/HeroDetailTabs.tsx`, `library/GameDetail.tsx`, `library/__tests__/HeroDetailPanels.test.tsx`, `library/__tests__/GameDetail.test.tsx`, `library/hero-detail-model.ts`.

**Phase 2: Sidebar cleanup + Favorites + Currently Playing**

- **Goal**: Sidebar no longer has Profiles/Launch. Favorites + Currently Playing are declared sidebar entries that navigate to Library + set a filter.
- **Scope**:
  1. `Sidebar.tsx:63-105` — drop `profiles`/`launch` items from the `'game'` section (Library stays as the only entry).
  2. Extend `SidebarSection` type with a third variant `type: 'library-filter'` carrying `{ filterKey: LibraryFilterKey, label: string, icon: ComponentType, badge?: () => ReactNode }`.
  3. Add two entries to the `'collections'` section: `{ type: 'library-filter', filterKey: 'favorites', label: 'Favorites', icon: HeartIcon, badge: FavoritesBadge }` and `{ type: 'library-filter', filterKey: 'currentlyRunning', label: 'Currently Playing', icon: PlayIcon, badge: RunningBadge }`.
  4. New hook `src/crosshook-native/src/hooks/useRunningProfiles.ts` that subscribes to `LaunchSessionRegistry` events (or falls back to periodic `check_game_running` polling if the registry isn't exposed via IPC). Returns `Set<string>` of running profile names.
  5. `types/library.ts:5` — append `'currentlyRunning'` to `LibraryFilterKey`.
  6. `LibraryToolbar.tsx:9-13` — append `{ key: 'currentlyRunning', label: 'Running' }` to `FILTER_OPTIONS`.
  7. `LibraryPage.tsx:99-103` — add `case 'currentlyRunning': list = list.filter((p) => runningSet.has(p.name));`.
  8. `AppShell.tsx` — grow the `onNavigate` signature to `(route: AppRoute, opts?: { libraryFilter?: LibraryFilterKey; heroDetailTab?: HeroDetailTabId; profileName?: string })`. Sidebar library-filter entries call `onNavigate('library', { libraryFilter: filterKey })`. AppShell sets LibraryPage's `filterKey` via new prop passthrough.
- **Success signal**: Sidebar renders without Profiles/Launch. Clicking Favorites navigates to Library with Favorites chip `aria-pressed="true"`. Clicking Currently Playing sets the new filter. `npm test` + existing Playwright smoke pass (minus soon-to-be-deleted assertions — keep them passing by deferring the `AppRoute` shrink to Phase 8).
- **Files touched**: `layout/Sidebar.tsx`, `layout/sidebarVariants.ts` (if needed), `layout/AppShell.tsx`, `hooks/useRunningProfiles.ts` (new), `pages/LibraryPage.tsx`, `library/LibraryToolbar.tsx`, `types/library.ts`.

**Phase 3: Pre/post hook schema (crosshook-core additive fields)**

- **Goal**: Profile TOML round-trips two new optional arrays (`pre_launch_hooks`, `post_exit_hooks`) with zero migration burden.
- **Scope**:
  1. Add to `src/crosshook-native/crates/crosshook-core/src/profile/` (file location per existing module layout): `LaunchHook { id, name, path, stage, enabled }` + `HookStage::PreLaunch | PostExit`.
  2. Add `pre_launch_hooks: Vec<LaunchHook>` and `post_exit_hooks: Vec<LaunchHook>` to the profile struct with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
  3. Round-trip unit test: create a profile with two hooks, serialize, deserialize, assert equality. Empty-profile round-trip asserts no `[[pre_launch_hooks]]` sections emitted.
  4. TS types regeneration: `ts-rs` (or equivalent) emits `LaunchHook` and `HookStage` in `src/crosshook-native/src/types/generated/`.
  5. Do NOT wire into launcher. Add a tracked follow-up issue: "Runtime execution of pre/post hooks".
- **Success signal**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` green. New types available to the frontend.
- **Files touched**: `crates/crosshook-core/src/profile/*.rs`, `src/types/generated/*` (auto-generated).

**Phase 4: Hero Detail Profiles tab (two-pane editor)**

- **Goal**: Clicking Profiles tab inside Hero Detail opens a two-pane editor matching screenshot 1 exactly. Autosave parity with `/profiles`.
- **Scope**:
  1. New `components/library/HeroDetailProfilesTab.tsx` (~350 lines, inside the 500-line soft cap).
  2. Left pane: profile cards list. Each card shows: active checkmark + active pill; built-in pill (derived from a `is_builtin` flag or name-prefix heuristic if no flag); profile name (bold); `{filename}.toml · {proton_version}` mono line; description (from TOML comment field or a new `description: String` — decide in implementation); `last used {duration}` + health pill. `+ New` button opens the existing profile-create wizard modal.
  3. Right pane: selected-profile editor. Flatten `ProfileIdentitySection` → `RuntimeSection` → `GameSection` → `TrainerSection` → `MediaSection` into a single scrollable column with dividers. Pass `profile`, `onUpdateProfile`, and the other props each section needs. Do NOT mount `ProfileSubTabs` — its tab nesting would fight Hero Detail's tabs.
  4. Card click → `useProfileContext().selectProfile(cardName)`. The singleton context updates; `profile` prop flows back down on next render; editor reflows.
  5. Autosave: `persistProfileDraft` is already wired by `onUpdateProfile`. Hook up `useLaunchEnvironmentAutosave` / `useProfileLaunchAutosave` via the existing chain (provider-exposed callbacks).
  6. Replace current `ProfilesPanel` (`HeroDetailPanels.tsx:310-354`) in the `profiles` case with `<HeroDetailProfilesTab {...panelProps} />`.
  7. Tests: new cases in `HeroDetailPanels.test.tsx`: render Profiles tab, edit Proton version, assert `persistProfileDraft` called within 350ms. Click second card, assert `selectProfile('card2')` and heading updates.
- **Success signal**: Visually matches screenshot 1. Autosave fires with the same debounces as `/profiles`. Switching profiles via card click updates hero pills + command preview.
- **Files touched**: `library/HeroDetailProfilesTab.tsx` (new), `library/HeroDetailPanels.tsx`, `library/__tests__/HeroDetailPanels.test.tsx`.

**Phase 5: Hero Detail Launch tab (3-section stack) + highlighted command preview**

- **Goal**: Launch tab matches screenshot 2: Launch command (colored) + Environment (checkboxes + count badge) + Pre/post hooks (placeholder section; Phase 6 fills it).
- **Scope**:
  1. New `components/library/HeroDetailLaunchTab.tsx` (~300 lines).
  2. New `components/library/HighlightedCommandBlock.tsx` (~120 lines). Input: `preview: LaunchPreview`. Output: `<pre>` with a composed child-span list:
     - Comment line: `# auto-generated from {profile.name}.toml` (span class `--comment`).
     - Env vars from `preview.environment` filtered to non-empty: `{KEY}` span class `--env-key`, `=`, `{value}` span class `--value`.
     - Line continuation (`\`) and newlines inline.
     - Each wrapper from `preview.wrappers`: span class `--binary` for the name, trailing flags split to `--flag` spans.
     - Final `proton run {executable} {args}` line with `--binary` + `--value` + `--flag` classes.
     - New CSS tokens: `.crosshook-hero-detail__cmd-token--env-key { color: var(--crosshook-color-warning); }`, `--value { color: var(--crosshook-color-success); }`, `--binary { color: var(--crosshook-color-accent-strong); font-weight: 600; }`, `--flag { color: var(--crosshook-color-text-muted); }`, `--comment { color: var(--crosshook-color-text-faint); }`.
  3. Section 1 actions row: Dry-run (calls existing `previewLaunch` IPC), Copy (copies `preview.effective_command` via `navigator.clipboard`), .desktop (reuses existing `.desktop` export), Launch (primary — calls `onLaunch`).
  4. Section 2: Environment editor reusing `CustomEnvironmentVariablesSection` (imported from `launch-subtabs/EnvironmentTabContent.tsx:6`). Render a count-pill top-right showing "{N} ON" based on enabled-count. Autosave wires through `useLaunchEnvironmentAutosave`.
  5. Section 3: placeholder empty state with "Pre/post hooks coming in Phase 6" banner (internal comment only; user-facing text says "No pre/post hooks configured yet" + "Add hook" button disabled until Phase 6).
  6. Replace `launch-options` case (`HeroDetailPanels.tsx:436-450`) with `<HeroDetailLaunchTab {...panelProps} />`.
  7. Tests: new cases in `HeroDetailPanels.test.tsx`: render Launch tab, toggle an env var, assert `persistProfileDraft` fires with updated env. Snapshot `HighlightedCommandBlock` token classes.
- **Success signal**: Visually matches screenshot 2 (modulo the Phase 6 hooks panel). Command preview is colored. Env toggle autosaves.
- **Files touched**: `library/HeroDetailLaunchTab.tsx` (new), `library/HighlightedCommandBlock.tsx` (new), `library/HeroDetailPanels.tsx`, `styles/hero-detail.css` (extend), `library/__tests__/HeroDetailPanels.test.tsx`.

**Phase 6: Pre/post hooks panel**

- **Goal**: Pre/post hooks section in Hero Detail Launch tab is a live editor.
- **Scope**:
  1. New `components/library/HookListPanel.tsx` (~150 lines).
  2. Props: `{ hooks: LaunchHook[]; stage: HookStage; onUpdate: (hooks: LaunchHook[]) => void }`.
  3. Each row: toggle (enabled), name (bold) + `.path` (mono, muted), stage pill (`pre-launch` / `post-exit`), settings gear that opens a small popover with path input + remove button. `+ Attach script or DLL` at the bottom.
  4. `HeroDetailLaunchTab.tsx` renders two `<HookListPanel>` instances, one per stage, sharing a combined `onUpdate` that merges into the two profile fields.
  5. `onUpdate` → `updateProfile({ ...profile, pre_launch_hooks, post_exit_hooks })` → `profile_save` via existing autosave path. 400ms debounce.
  6. Info banner at the top of the combined section: "These hooks are saved to your profile. Runtime execution is coming in a future release." Link to the tracked GitHub issue.
  7. Tests: add a hook, toggle it off, assert `profile_save` called with updated array. Remove hook, assert `skip_serializing_if` drops empty array (round-trip via mocked IPC).
- **Success signal**: Adding, toggling, editing, and removing a hook round-trips through TOML. The declared-not-executed banner is visible.
- **Files touched**: `library/HookListPanel.tsx` (new), `library/HeroDetailLaunchTab.tsx`, `library/__tests__/HeroDetailPanels.test.tsx`.

**Phase 7: Overview tab deep-links**

- **Goal**: Overview's panel buttons navigate to the relevant sub-tab so Overview reads as a real dashboard.
- **Scope**: In `HeroDetailPanels.tsx` `case 'overview':` (L417-433), append deep-link buttons to each panel:
  - Runtime panel: "Open runtime →" → `onSetActiveTab('profiles')` (then the Profiles tab pre-scrolls to Runtime section via anchor).
  - Active profile panel: "Open profile →" → `onSetActiveTab('profiles')`.
  - Health panel: (unchanged; already has an external link).
  - Launch command panel: "Edit launch config →" → `onSetActiveTab('launch-options')`.
  - Trainer hook panel: "Manage hooks →" → `onSetActiveTab('launch-options')` (Phase 6's hook panel is there).
- **Success signal**: Every deep-link changes the active tab. Smoke test covers one click → one tab change round-trip.
- **Files touched**: `library/HeroDetailPanels.tsx`.

**Phase 8: AppRoute union shrink**

- **Goal**: `AppRoute` drops `'profiles'` and `'launch'`. TypeScript enumerates every stale reference.
- **Scope**:
  1. `Sidebar.tsx:20-31` — remove `'profiles' | 'launch'` from `AppRoute`.
  2. `routeMetadata.ts:43-124` — remove `profiles` and `launch` entries from `ROUTE_METADATA` and `ROUTE_NAV_LABEL`.
  3. Run `npx tsc --noEmit --project src/crosshook-native/tsconfig.json`. Every error is a Phase 9 rewire target.
  4. Do NOT delete `ProfilesPage.tsx`/`LaunchPage.tsx` yet — Phase 10 does that. Phase 8 is strictly the type shrink.
- **Success signal**: TypeScript compile fails with a known list of errors (the rewire checklist for Phase 9).
- **Files touched**: `layout/Sidebar.tsx`, `layout/routeMetadata.ts`.

**Phase 9: Nav rewire**

- **Goal**: Every `onNavigate('profiles'|'launch')` caller is updated to open Hero Detail + set tab. After this phase, `tsc` is clean again.
- **Scope**: Walk the Phase 8 error list:
  - `components/library/LibraryGrid.tsx:36` / `LibraryList.tsx:37` — empty-state CTA: remove the CTA entirely (user lands in Library with no games; guidance belongs in the onboarding wizard, not a route jump).
  - `components/pages/LibraryPage.tsx:143, 155` — `onEdit(name)` and `onLaunch(name)`: update to set `selectedGame=name`, `heroMode='detail'`, `heroDetailTab='profiles'` (for edit) or `'launch-options'` (for launch). Reuse existing Hero Detail mode toggle.
  - `components/pages/HealthDashboardPage.tsx:69, 217` — "Edit profile" button: navigate to Library + open Hero Detail for that profile's game card + `heroDetailTab='profiles'`. If the profile doesn't map to a Library game card (orphan profile), fall back to a toast.
  - `components/pages/InstallPage.tsx:289` — post-install: navigate to Library + select the new game's card + `heroMode='detail'` + `heroDetailTab='profiles'`.
  - `components/collections/CollectionViewModal.tsx:35, 47, 239` — `onOpenInProfilesPage` becomes `onOpenInHeroDetail({ profileName, tab: 'profiles' })`.
  - `components/collections/CollectionLaunchDefaultsEditor.tsx:69, 83, 301` — close the modal; the editor already has its own inline UI. Delete the "Edit profile defaults" button.
  - `components/layout/AppShell.tsx:141, 149, 229, 233, 460` — `handleLaunchFromCollection` / `handleEditFromCollection` / `handleExecuteCommand` / `onOpenInProfilesPage`: all become "open Hero Detail + set tab + optionally fire onLaunch". Consolidate into a single helper `openGameInHeroDetail({ profileName, tab, launch })`.
  - `components/library/__tests__/LibraryGrid.test.tsx:48` — update to assert the new onNavigate opts payload.
- **Success signal**: `tsc` clean. Existing feature flows preserved. `npm test` green.
- **Files touched**: all the above + corresponding test files.

**Phase 10: Route deletion**

- **Goal**: `ProfilesPage.tsx`, `LaunchPage.tsx`, their subdirs, their RTL tests, and their `ContentArea` cases are physically removed.
- **Scope**:
  1. `git rm src/crosshook-native/src/components/pages/ProfilesPage.tsx`
  2. `git rm src/crosshook-native/src/components/pages/LaunchPage.tsx`
  3. `git rm -r src/crosshook-native/src/components/pages/profiles/`
  4. `git rm -r src/crosshook-native/src/components/pages/launch/`
  5. `git rm src/crosshook-native/src/components/pages/__tests__/ProfilesRoute.test.tsx`
  6. `git rm src/crosshook-native/src/components/pages/__tests__/LaunchRoute.test.tsx`
  7. `ContentArea.tsx:40-43` — remove `case 'profiles':` and `case 'launch':` (the switch falls through to a default that already exists).
  8. `ContentArea.tsx:9, 11` — remove `import { LaunchPage } from '.../LaunchPage'` and `import { ProfilesPage } from '.../ProfilesPage'`.
  9. Grep for `RouteBanner route="profiles"` / `route="launch"` and remove or rewrite if an orphan remains.
  10. Remove unused icon imports (`LaunchIcon`, `ProfilesIcon`) from `icons/SidebarIcons.tsx` exports if no callsite survives.
- **Success signal**: `tsc` green. `npm test` green. `git status` shows only deletions + import updates.
- **Files touched**: listed above + `layout/ContentArea.tsx`, `icons/SidebarIcons.tsx`.

**Phase 11: Playwright smoke rewrite**

- **Goal**: Smoke coverage matches the new shape: no `/profiles` / `/launch` routes, Hero Detail flows cover edit + launch, sidebar entries covered.
- **Scope**:
  1. `tests/smoke.spec.ts:33-45` — `ROUTE_ORDER` drops `'profiles'` and `'launch'`.
  2. Remove `test.describe('launch pipeline smoke')` at L189-203 (the pipeline UI moves to Hero Detail; replace with an equivalent Hero Detail preview assertion).
  3. Remove `test.describe('profiles + launch panel landing smoke')` at L205-251.
  4. Rewrite `console chrome smoke` at L307-358: click Library card → assert Hero Detail opens → click Profiles tab → select second profile card → assert active-profile pill updates → click Launch tab → assert command block rendered → click hero Launch button → assert launch registered.
  5. New test: `test.describe('sidebar favorites')` — click Favorites entry, assert on `/library` + `aria-pressed="true"` on Favorites chip.
  6. New test: `test.describe('sidebar currently playing')` — mock a running profile via IPC stub; click Currently Playing entry; assert filter applied.
  7. New test: `test.describe('appRoute regression guard')` — assertion: the sidebar has no `role="tab"` elements with accessible names "Profiles" or "Launch".
  8. Run `npm run test:smoke:update` to regenerate screenshots.
- **Success signal**: `npm run test:smoke` green.
- **Files touched**: `tests/smoke.spec.ts`.

**Phase 12: Polish + docs + release notes**

- **Goal**: Documentation catches up; no dead assets; release-noteable summary ready.
- **Scope**:
  1. Update `docs/internal-docs/design-tokens.md` to document the new command-preview token classes (`--cmd-token--env-key` etc.).
  2. Update `docs/architecture/adr-0001-platform-host-gateway.md` if any of the changed files touched `platform.rs` (they shouldn't, per scope).
  3. Add a release-notes bullet to the next `CHANGELOG.md` entry: "feat(ui): fold Profiles and Launch into Hero Detail; drop standalone routes". The bullet lands via Conventional Commits on PR merge.
  4. Grep for orphan `ProfilesIcon`, `LaunchIcon`, `.crosshook-profiles-page__body`, `.crosshook-launch-page__grid`, `.crosshook-launch-pipeline` CSS selectors; remove unused entries from stylesheets.
  5. Manual Steam Deck pass: open Hero Detail on a 1280×800 viewport, verify Profiles tab collapses to single-column, hooks panel rows wrap, command block scrolls horizontally.
- **Success signal**: `scripts/lint.sh` green. Manual Deck pass finds no regressions.
- **Files touched**: `docs/internal-docs/design-tokens.md`, CSS cleanup files, (no CHANGELOG.md edit — driven by cliff on release prep).

### Parallelism Notes

- Phases **2 and 3** parallelize — `Sidebar.tsx` work (TS/CSS) is disjoint from `crosshook-core/src/profile/*.rs` schema work.
- Phases **4 and 5** parallelize — `HeroDetailProfilesTab.tsx` and `HeroDetailLaunchTab.tsx` are new files with no shared surface area beyond `HeroDetailPanels.tsx` mode-switch (last-to-merge wins).
- Phase **6** depends on 3 (schema) and 5 (Launch tab shell). Must be serial.
- Phase **7** parallels 6 once 4 and 5 have landed.
- Phases **8 → 9 → 10** are strictly serial — the type shrink enumerates the rewire list, which enumerates the deletable files.
- Phase **11** blocks on 10. Phase **12** blocks on 11.

---

## Decisions Log

| Decision                 | Choice                                                      | Alternatives                                                | Rationale                                                                                     |
| ------------------------ | ----------------------------------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Route fate               | Delete `/profiles` and `/launch` entirely                   | Keep hidden from sidebar / keep as palette deep-link target | User decision; parallel UIs invite drift and doubled QA.                                      |
| Save model               | Autosave (match Phase 11 behavior)                          | Explicit Save/Cancel / read-only v1                         | Consistent with shipped behavior; no Save cliffs; design doesn't show a Save button.          |
| Favorites scope          | Sidebar entry + Library filter chip                         | Sidebar-only / skip                                         | 95% of wiring already exists; the sidebar entry closes the gap.                               |
| Currently Playing entry  | Declared sidebar entry + new `filterKey='currentlyRunning'` | Dynamic-only / badge-only                                   | Screenshot shows it; reactive via `LaunchSessionRegistry`.                                    |
| Profile switch model     | `selectProfile(name)` on card click                         | Scoped ProfileProvider / single-active-profile-only         | Keeps the singleton ProfileProvider; aligns with the card the user just clicked.              |
| Pre/post hook panel      | Build in this PRD; schema ships; runtime deferred           | Defer to follow-up / stub-only                              | Matches the screenshot exactly; runtime execution needs launcher work (separate PRD).         |
| Command preview coloring | Hand-rolled token spans using backend's structured preview  | shiki/prism dep / plain `<pre>`                             | Zero-dep; tokens already structured; matches CLAUDE.md dependency hygiene.                    |
| ProfileSwitcher in hero  | None (tab-only switching)                                   | Dropdown / card in hero aside                               | Final screenshots have no hero switcher; intermediate `Detail.jsx` mock is superseded.        |
| Launch tab shape         | 3 sections (Launch command · Environment · Pre/post hooks)  | 6-panel grid (Launcher stack, Arguments, Prefix & paths)    | Screenshots show 3 sections only. Launcher-stack reordering is deferred.                      |
| `AppRoute` union         | Remove `'profiles'` / `'launch'`                            | Keep for back-compat                                        | TypeScript enumerates every stale caller; delete stays surgical.                              |
| Overview deep-links      | Buttons call `onSetActiveTab`                               | Anchor links / `#hash` routing                              | State-driven is consistent with the rest of the shell (no URL routing anywhere).              |
| Route deletion boundary  | Physical `git rm` in Phase 10                               | Deprecation comment first                                   | Phase 8 type shrink + Phase 9 rewire are the deprecation pass; Phase 10 just executes.        |
| Trainer tab disposition  | Open question — recommend upgrade to editor in Phase 4      | Leave read-only                                             | Trainer fields live in the same TOML namespace as runtime; avoid two editors for one profile. |

---

## Research Summary

**Market Context**

Already captured in the source PRD (`docs/prps/prds/unified-desktop-redesign.prd.md` § Research Summary). This PRD inherits: per-game Hero Detail as the source of truth, calm steel-blue palette, `uw / desk / narrow / deck` breakpoints, `react-resizable-panels` three-pane shell. No net-new market research — the user's own design bundle (`/tmp/crosshook-design-v2/`) is the grounding artifact.

**Technical Context** — grounded by `ycc:prp-researcher` in the PRD session (`2026-04-23`):

- Profiles editor is fully prop-scopeable today. Every `profile-sections/*Section.tsx` takes `{profile, onUpdateProfile, ...}` without context coupling (`profile-sections/ProfileIdentitySection.tsx:5-16`, `GameSection.tsx:7-13`, `RuntimeSection.tsx:24-30`, `TrainerSection.tsx:15-24`, `MediaSection.tsx:9-13`).
- `ProfileSubTabs.tsx:85-309` and `LaunchSubTabs.tsx:21-250` are also prop-driven (no `useProfileContext()` call). Reusing their sub-components in Hero Detail is composition, not refactoring.
- `ProfileProvider` is a singleton (`context/ProfileContext.tsx:30-72`). Aligning it to the active card via `selectProfile(name)` on tab-click is the lowest-risk choice; no scope-provider variant needed.
- Autosave uses `persistProfileDraft` → `profile_save` IPC. Debounces: 350ms for launch-opts / gamescope / trainer-gamescope / mangohud (`hooks/profile/constants.ts:2`); 400ms for env-vars (`useLaunchEnvironmentAutosave.ts:87`). Moving the UI doesn't move the save pipeline.
- Favorites: `LibraryFilterKey='favorites'` filter is already wired end-to-end (`types/library.ts:5`, `LibraryPage.tsx:99-103`, `LibraryToolbar.tsx:9-13`, `useProfileContext().favoriteProfiles`). Only missing: a sidebar entry.
- Route deletion surface: 12 `onNavigate('profiles'|'launch')` callers, 3 Playwright smoke blocks (`launch pipeline smoke` at L189-203, `profiles + launch panel landing smoke` at L205-251, `console chrome smoke` at L307-358 — partial rewrite), 2 RTL files (`ProfilesRoute.test.tsx`, `LaunchRoute.test.tsx`). Palette handlers at `AppShell.tsx:223-239`.
- GAPs: (a) No dedicated pre/post-hook UI component today (`launch-optimizations/` covers optimization toggles, not pre/post scripts). (b) No syntax-highlighter in the hero path (`HeroDetailPanels.tsx` uses plain `<pre>`). (c) No "Currently Playing" collection entry in `CollectionsSidebar.tsx` today. (d) No single-profile-scoped `ProfileProvider` variant — the singleton pattern is consistent throughout.

---

_Generated: 2026-04-23_
_Status: DRAFT — ready for implementation planning via `/ycc:prp-plan docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`_
