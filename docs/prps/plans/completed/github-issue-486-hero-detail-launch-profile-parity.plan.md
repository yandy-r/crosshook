# Plan: Hero Detail Launch/Profile Parity Before Route Removal (Phase 5b)

**GitHub issue**: [#486](https://github.com/yandy-r/crosshook/issues/486) — Phase 5b: Hero Detail Launch/Profile parity before route removal. Part of #478 (Hero Detail Consolidation tracker).

| Metadata                | Value                                                                                                  |
| ----------------------- | ------------------------------------------------------------------------------------------------------ |
| Complexity              | High                                                                                                   |
| Confidence              | 8/10                                                                                                   |
| Estimated files changed | ~30                                                                                                    |
| Research mode           | Enhanced (7 parallel researchers: api / business / tech / ux / security / practices / recommendations) |
| Execution               | Parallel-capable (`/ycc:prp-implement --parallel`) — see Batches section                               |

## Goal

Bring the Hero Detail `launch-options` and `profiles` tabs to like-for-like functional parity with the legacy `/launch` (`LaunchPage` + `LaunchSubTabs`) and `/profiles` (`ProfilesPage` + `ProfileSubTabs`) routes, so Phases 8/9/10 (#473/#474/#475) can shrink, rewire, and delete those routes without functional regression. Deliver the two parity inventories required by the acceptance criteria (embedded in this plan, finalized against the shipped UI in the last task).

## Why

- Phase 5 (#470) delivered only 3 launch sections (command / environment / hooks placeholder); the legacy Launch route exposes 6 method-sensitive sub-tabs plus dep gate, ProtonDB guidance, optimization presets, and launch feedback — none of which exist in Hero Detail yet.
- Phase 4 (#469) delivered a 4-section profile editor (Identity/Runtime/Game/Media); legacy Profiles additionally exposes RunnerMethod, Trainer, trainer-Gamescope, LauncherExport, lifecycle actions (duplicate/rename/delete/preview/export/history), health affordances, and ProtonUp runtime suggestions.
- #473 is explicitly only the `AppRoute` union shrink; parity is NOT implicitly handled downstream — that is exactly why #486 was inserted as a gate.

## Scope decisions (user-confirmed)

1. **Trainer editing**: Port the legacy `TrainerSection` editor (path/type/loading mode/network isolation/version) + trainer-gamescope `GamescopeConfigPanel` into the Hero Detail Profiles editor. #479 retains ownership of the separate read-only Trainer _top-level tab_ upgrade (injection log, loaded hooks).
2. **Profile lifecycle actions**: Port ALL of duplicate, rename (+undo toast/F2), delete (confirm overlay), TOML preview, community export, config history/rollback, and mark-as-verified, via a shared `useProfileActions` hook extracted from `useProfilesPageState`.
3. **In-place launch**: Wire `useLaunchStateContext().launchGame`/`launchTrainer` + `useLaunchDepGate` + `LaunchDepGateModal` + launch feedback directly into the Hero Detail Launch tab (selectProfile-first). Global navigation entries are NOT touched (Phase 9's domain).

## What we're NOT building (deferred ownership — do not double-plan)

| Capability                                                         | Owner           | 5b action                                                                                  |
| ------------------------------------------------------------------ | --------------- | ------------------------------------------------------------------------------------------ |
| Live pre/post hook editing (`HookListPanel`)                       | #471 (Phase 6)  | Keep the disabled placeholder in the Launch tab                                            |
| Hook runtime execution                                             | #482            | Nothing                                                                                    |
| Create-profile flow hardening (prefill, post-create selection)     | #487 (Phase 5c) | Keep the existing `+ New` → `OnboardingWizard` CTA as-is                                   |
| Trainer top-level tab editor upgrade (injection log, loaded hooks) | #479            | Trainer _tab_ stays read-only; trainer editing lands in the _Profiles_ editor (decision 1) |
| Overview deep-link buttons (`onSetActiveTab`)                      | #472 (Phase 7)  | Leave `onSetActiveTab` `undefined` (`GameDetail.tsx:195` TODO)                             |
| `AppRoute` shrink / nav rewire / page deletion                     | #473/#474/#475  | Touch none of those files; both legacy routes must keep working                            |
| New persisted schema, new npm dependencies                         | —               | Forbidden by issue AC                                                                      |

---

## Launch Parity Inventory (acceptance-criteria artifact)

Classification: **Ported** (in Hero Detail; rows marked `5b — Task N` landed with this plan) / **Superseded** (redundant in game-scoped Hero Detail — documented, not built) / **Deferred(#N)** (owned elsewhere). Finalized against the shipped UI in Task 5.2 — every former **Port** row was implemented and flipped to **Ported**; no row required reclassification.

| Surface/Control                                                                          | Legacy source                                                          | Hero Detail status today                                             | 5b classification                                                                              |
| ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Launch command preview (highlighted)                                                     | `LaunchPanel.tsx:191-200`                                              | `HighlightedCommandBlock` (`HeroDetailLaunchTab.tsx:246`)            | **Ported**                                                                                     |
| Dry-run / preview trigger                                                                | `LaunchPanel.tsx:160` via `requestPreview`                             | Dry-run button (`HeroDetailLaunchTab.tsx:194-205`)                   | **Ported**                                                                                     |
| Copy command (+2.5s status)                                                              | `LaunchPanel` preview path                                             | `HeroDetailLaunchTab.tsx:165-184,206-213`                            | **Ported**                                                                                     |
| Launch action (game) — in place                                                          | `LaunchPanelControls` via `launchGame` (`LaunchPanel.tsx:158`)         | `onLaunch` navigates to legacy `/launch` (`LibraryPage.tsx:170-175`) | **Ported** (5b — Task 3.1, in-place via `LaunchStateContext`)                                  |
| Launch trainer action                                                                    | `LaunchPanel.tsx:96,150,159`                                           | Missing                                                              | **Ported** (5b — Task 3.1)                                                                     |
| `onBeforeLaunch` dep-gate interception                                                   | `LaunchPage.tsx:215`; `LaunchPanel.tsx:92-103`                         | Missing — `onLaunch` fires directly                                  | **Ported** (5b — Task 3.1)                                                                     |
| Dependency gate modal (prefix deps)                                                      | `LaunchDepGateModal` + `useLaunchDepGate` (`LaunchPage.tsx:76-80,219`) | Missing                                                              | **Ported** (5b — Task 3.1)                                                                     |
| Launch feedback / diagnostic report + copy JSON                                          | `LaunchPanelFeedback` (`LaunchPanel.tsx:107-143`)                      | Missing                                                              | **Ported** (5b — Task 3.1)                                                                     |
| Version-status acknowledge ("Mark as Verified")                                          | `LaunchPanelVersionStatus` (`LaunchPanel.tsx:176-180`)                 | Missing                                                              | **Ported** (5b — Task 3.2 — via `useProfileActions` in Profiles tab)                           |
| Launch pipeline visualization                                                            | `LaunchPipeline` (`LaunchPanel.tsx:165`)                               | Missing                                                              | **Ported** (5b — Task 3.1, render in Launch tab feedback area)                                 |
| Helper log path indicator                                                                | `LaunchPanel.tsx:166-168`                                              | Missing                                                              | **Ported** (5b — Task 3.1)                                                                     |
| Launch guidance / hint text                                                              | `LaunchPanel.tsx:169-173`                                              | Missing                                                              | **Ported** (5b — Task 3.1)                                                                     |
| Preview modal (launch-from-preview)                                                      | `PreviewModal` (`LaunchPanel.tsx:206-213`)                             | Inline command only                                                  | **Superseded** — inline `HighlightedCommandBlock` + in-place Launch covers the flow            |
| Custom env vars editor (400ms blur autosave)                                             | `EnvironmentTabContent.tsx:74-80`                                      | `HeroDetailLaunchTab.tsx:267-273`                                    | **Ported**                                                                                     |
| ProtonDB lookup + env guidance                                                           | `ProtonDbLookupCard` (`EnvironmentTabContent.tsx:82-99`)               | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| ProtonDB overwrite confirmation                                                          | `ProtonDbOverwriteConfirmation` (`EnvironmentTabContent.tsx:101-108`)  | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| ProtonDB suggestion accept/dismiss                                                       | `LaunchPage.tsx:55-74,208-210`                                         | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| Launch optimizations toggles                                                             | `OptimizationsTabContent.tsx:50-63` (`LaunchOptimizationsPanel`)       | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| Optimization preset selector (named presets)                                             | `OptimizationsTabContent.tsx:54-57`                                    | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| Bundled optimization presets (one-click apply)                                           | `OptimizationsTabContent.tsx:58-59`                                    | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| Save manual preset                                                                       | `OptimizationsTabContent.tsx:61`                                       | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| Gamescope config (game) + method gating                                                  | `GamescopeTabContent.tsx:33-37` (`GamescopeConfigPanel`)               | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| "Inside gamescope session" guard                                                         | `LaunchPage.tsx:156` via `depGate.isGamescopeRunning`                  | Missing                                                              | **Ported** (5b — Tasks 2.1 + 3.1)                                                              |
| MangoHud config + gating                                                                 | `MangoHudTabContent.tsx:36-41` (`MangoHudConfigPanel`)                 | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| Steam Launch Options preview                                                             | `SteamOptionsTabContent.tsx:34-38`                                     | Missing                                                              | **Ported** (5b — Task 2.1)                                                                     |
| Offline readiness panel + launch-path warnings + trainer-hash actions                    | `OfflineTabContent.tsx:50-101`                                         | Overview shows passive `offlineReport` only                          | **Ported** (5b — Task 2.1 — Offline sub-tab with auto-switch)                                  |
| Offline auto-switch on concern                                                           | `LaunchSubTabs.tsx:82-98` (`OFFLINE_AUTO_SWITCH_PATTERN`)              | Missing                                                              | **Ported** (5b — Task 2.1 — comes free with `LaunchSubTabs`)                                   |
| Merged autosave chip (opts/gamescope/mangohud)                                           | `useAutoSaveChip` (`LaunchSubTabs.tsx:69-124`)                         | Env-only feedback                                                    | **Ported** (5b — Task 2.1 — comes with `LaunchSubTabs`)                                        |
| Full `LauncherExport` panel (status, preview script/desktop, delete, re-export)          | `ProfileSubTabs.tsx:287-306`                                           | Inline `.desktop` export button only                                 | **Ported** (5b — Task 3.2 — full panel in Profiles editor; Launch tab keeps the inline button) |
| Profile selector (`LaunchProfileSelector`: pick/pin, collection filter, isolation badge) | `LaunchPage.tsx:127-143`                                               | Profiles-tab card list owns selection                                | **Superseded** — Hero Detail is game-scoped; selection lives on the Profiles tab card list     |
| Info slot (selected profile / Steam path / umu pref KV)                                  | `LaunchPage.tsx:98-126`                                                | Missing                                                              | **Superseded** — duplicates game-scoped context                                                |
| `RouteBanner` / `Breadcrumb`                                                             | `LaunchPage.tsx:92`                                                    | n/a                                                                  | **Superseded** — route-only chrome                                                             |
| Pre/post hooks editing                                                                   | n/a (new surface)                                                      | Disabled placeholder                                                 | **Deferred(#471)**                                                                             |

## Profiles Parity Inventory (acceptance-criteria artifact)

| Surface/Control                                                            | Legacy source                                                              | Hero Detail status today                                             | 5b classification                                                                  |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Profile identity (name + game name)                                        | `ProfileSubTabs.tsx:192-199` (`ProfileIdentitySection`)                    | `HeroDetailProfilesTab.tsx:255-262`                                  | **Ported**                                                                         |
| Game path / Steam App ID / cover source                                    | `ProfileSubTabs.tsx:200` (`GameSection`)                                   | `HeroDetailProfilesTab.tsx:270`                                      | **Ported**                                                                         |
| Runtime fields (prefix/proton/umu/app id/working dir/AutoPopulate)         | `ProfileSubTabs.tsx:213-219` (`RuntimeSection`)                            | `HeroDetailProfilesTab.tsx:263-269`                                  | **Ported**                                                                         |
| Media / game art + launcher icon                                           | `ProfileSubTabs.tsx:231` (`MediaSection`)                                  | `HeroDetailProfilesTab.tsx:271`                                      | **Ported**                                                                         |
| Runner method select (steam_applaunch / proton_run / native)               | `ProfileSubTabs.tsx:201` (`RunnerMethodSection`)                           | Missing — method is derived/read-only                                | **Ported** (5b — Task 2.2)                                                         |
| Trainer editor (path/type/loading mode/network isolation/version set)      | `ProfileSubTabs.tsx:244-252` (`TrainerSection` + `TrainerVersionSetField`) | Missing — Trainer top-level tab is read-only                         | **Ported** (5b — Task 2.2, decision 1)                                             |
| Trainer-gamescope config (+derived-from-game notice)                       | `ProfileSubTabs.tsx:258-284` (`GamescopeConfigPanel`)                      | Missing                                                              | **Ported** (5b — Task 2.2)                                                         |
| Game metadata bar                                                          | `ProfileSubTabs.tsx` (`GameMetadataBar`)                                   | Missing                                                              | **Ported** (5b — Task 2.2)                                                         |
| Launcher export (full panel incl. `pendingReExport`)                       | `ProfileSubTabs.tsx:287-306` (`LauncherExport`)                            | Missing                                                              | **Ported** (5b — Task 3.2)                                                         |
| Profile selection                                                          | `ProfilesHero` + `filteredProfiles` (`ProfilesPage.tsx:107-156`)           | Card list (`HeroDetailProfilesTab.tsx:180-227`)                      | **Ported** (different UX, same function)                                           |
| New profile wizard                                                         | `ProfilesPage.tsx:118`                                                     | `+ New` CTA (`HeroDetailProfilesTab.tsx:228-236`)                    | **Ported** (hardening owned by #487)                                               |
| Edit profile wizard                                                        | `ProfilesPage.tsx:117`                                                     | Inline editor                                                        | **Superseded** — inline two-pane editor replaces the edit wizard                   |
| Explicit Save button + dirty text                                          | `ProfileActions.tsx:116-118,177-179`                                       | 350ms autosave + chip (`HeroDetailProfilesTab.tsx:115-176`)          | **Superseded** — autosave chip replaces explicit save                              |
| Duplicate profile                                                          | `ProfileActions.tsx:119-126`                                               | Missing                                                              | **Ported** (5b — Tasks 1.2 + 3.2)                                                  |
| Rename profile (+modal, undo toast, F2)                                    | `ProfileActions.tsx:127-134`; `useProfilesPageNotifications.ts:95-154`     | Missing (rename-pause autosave guard exists)                         | **Ported** (5b — Tasks 1.2 + 3.2)                                                  |
| Delete profile (+confirm overlay, collection-aware)                        | `ProfileActions.tsx:169-176`; `ProfilesPage.tsx:296-318`                   | Missing                                                              | **Ported** (5b — Tasks 1.2 + 3.2)                                                  |
| TOML preview                                                               | `ProfileActions.tsx:135-142`; `ProfilesPage.tsx:276-280`                   | Missing                                                              | **Ported** (5b — Tasks 1.2 + 3.2)                                                  |
| Community export                                                           | `ProfileActions.tsx:143-150`; `ProfilesPage.tsx:281-290`                   | Missing                                                              | **Ported** (5b — Tasks 1.2 + 3.2)                                                  |
| Config history / rollback / mark known good                                | `ProfileActions.tsx:161-168`; `useProfileHistory.ts`                       | Missing (Hero History tab = _launch_ history, distinct)              | **Ported** (5b — Tasks 1.2 + 3.2)                                                  |
| Mark as Verified (version drift ack)                                       | `ProfileActions.tsx:151-160`                                               | Missing                                                              | **Ported** (5b — Tasks 1.2 + 3.2)                                                  |
| Prefix dependencies panel                                                  | `PrefixDepsPanel` (`ProfilesPage.tsx:163-171`)                             | Missing                                                              | **Ported** (5b — Task 2.2, inside `CollapsibleSection`)                            |
| Runtime suggestion banner (community-recommended Proton, ProtonUp install) | `ProfilesPage.tsx:173-217`; `useProfilesPageProton.ts:87-131`              | Bare `useProtonInstalls` only                                        | **Ported** (5b — Task 2.2)                                                         |
| Profile health badge (click→issues) + health issues list                   | `ProfilesPage.tsx:55-99,158-161`                                           | Per-card `HealthBadge` only; Overview has `GameDetailsHealthSection` | **Ported** (5b — Task 2.2 — issues list in editor; badge click scrolls to it)      |
| Offline status badge                                                       | `ProfilesPage.tsx:48-53,132`                                               | Overview has it                                                      | **Superseded** — Overview tab `GameDetailsHealthSection` covers it                 |
| Trainer type chip / version status badge / network isolation badge         | `ProfilesPage.tsx:28-46,129-142`                                           | Missing                                                              | **Ported** (5b — Task 2.2 — render with trainer/health affordances)                |
| Profiles-with-issues summary chip ("N of M")                               | `ProfilesPage.tsx:143-148`                                                 | n/a                                                                  | **Superseded** — library-wide affordance; Hero Detail is single-game scoped        |
| Collection filter / clear                                                  | `ProfilesPage.tsx:115`                                                     | n/a                                                                  | **Superseded** — game-scoped card list                                             |
| Health banner dismiss                                                      | `ProfilesPage.tsx:111,116`                                                 | Missing                                                              | **Superseded** — banner itself is library-wide; per-game issues list covers parity |
| Stale-check note ("Checked Nd ago")                                        | `ProfilesPage.tsx:149-153`                                                 | Missing                                                              | **Ported** (5b — Task 2.2 — alongside health issues list)                          |

---

## Architecture (all patterns verified by research, with citations)

### Provider topology — the load-bearing fact

`GameDetail` (rendered at `LibraryPage.tsx:394` inside `ContentArea`) already lives inside **all** required providers: `ProfileProvider` (`App.tsx:37`), `PreferencesProvider` (`AppShell.tsx:381`), `LaunchStateProvider` (`AppShell.tsx:383`), plus `ProfileHealthProvider`/`HostReadinessProvider`/`CollectionsProvider` (`App.tsx:38-41`). Every missing capability is reachable by **consuming existing contexts inside the tabs** — no new providers, no new prop-drilling beyond targeted `panelProps` additions.

**CRITICAL scoping gotcha**: `LaunchStateContext` builds its `LaunchRequest` from `ProfileContext`'s _selected_ profile (`LaunchStateContext.tsx:25-31`). Hero Detail may display a _fallback_ profile (`useGameDetailsProfile` when `!singletonOwnsGame`, `GameDetail.tsx:74-79`). Any in-place launch / dep-gate / offline wiring MUST `selectProfile` into `ProfileContext` first (legacy does this: `LibraryPage.tsx:170`). Gate the in-place launch/dep-gate/offline surfaces on the selected-profile-matches-displayed-profile condition; render disabled state otherwise.

### Single persistence path (issue hard requirement)

There is exactly one write surface: `persistProfileDraft` → `profile_save` (`useProfileCrud.ts:266-301`) plus granular section autosaves `profile_save_launch_optimizations` / `profile_save_gamescope_config` / `profile_save_trainer_gamescope_config` / `profile_save_mangohud_config` — all owned by `useProfile` itself via `useProfileLaunchAutosaveEffects.ts:145,217,289,361` (350ms = `launchOptimizationsAutosaveDelayMs`, `hooks/profile/constants.ts:2`), firing whenever `ProfileContext.profile` changes. **They are not route-owned** — mutating `profile.launch.gamescope` from Hero Detail through context mutators already triggers the same path.

Rules for every new surface:

- Call context mutators only: `updateProfile`, `updateLaunchSetting`, `toggleLaunchOptimization`, `switchLaunchOptimizationPreset`, `persistProfileDraft`. NEVER `invoke('profile_save*')` directly from a tab component.
- Env autosave stays on `useLaunchEnvironmentAutosave` (400ms blur, `useLaunchEnvironmentAutosave.ts:53-90`) — already shared by both surfaces.
- Granular writes serialize via `enqueueLaunchProfileWrite`; the Profiles-tab full-draft 350ms autosave (`HeroDetailProfilesTab.tsx:115-145`) does NOT share that queue. Keep a single autosave owner per profile name: the Profiles-tab draft effect must remain the only full-draft writer in Hero Detail; launch-tab surfaces use only granular context mutators. Doubled idempotent writes are documented-acceptable (#469 plan risk table) but full-draft-vs-granular interleave on the same name must be avoided.

### Component reuse map (no re-implementation — DRY mandate)

| Reuse as-is                                                         | Source                                                                 | Notes                                                                                                                                                                             |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `LaunchSubTabs` + all `launch-subtabs/*TabContent`                  | `components/LaunchSubTabs.tsx:21-251`, `launch-subtabs/types.ts:20-78` | 40+ prop payload; fully prop-driven except `OfflineTabContent`/`SteamOptionsTabContent` read `useLaunchStateContext` (available). Owns offline auto-switch + merged autosave chip |
| `GamescopeConfigPanel`                                              | `components/GamescopeConfigPanel.tsx`                                  | Prop-driven; used for game AND trainer gamescope                                                                                                                                  |
| `LauncherExport` (full panel)                                       | `components/LauncherExport.tsx`                                        | Request builders already shared via `utils/launcherExport.ts`                                                                                                                     |
| `RunnerMethodSection`, `TrainerSection`, `GameMetadataBar`          | `components/profile-sections/`                                         | Prop-driven; `TrainerSection` self-fetches `useTrainerTypeCatalog`                                                                                                                |
| `PrefixDepsPanel`, `LaunchDepGateModal`, `useLaunchDepGate`         | `components/PrefixDepsPanel.tsx`, `components/pages/launch/`           | Hook reusable; modal presentational                                                                                                                                               |
| `useProtonDbApply`, `useProtonDbSuggestions`                        | `hooks/profile/useProtonDbApply.ts`, `hooks/useProtonDbSuggestions.ts` | Needs `resolvedSteamAppId` (derive via `resolveArtAppId`)                                                                                                                         |
| `LaunchPanelFeedback`, `LaunchPipeline`, `LaunchPanelVersionStatus` | `components/` (via `LaunchPanel.tsx:135-180`)                          | Verify prop contracts; extract from `LaunchPanel` only if coupled                                                                                                                 |
| `useProfileHistory`, `RollbackPanel`                                | `hooks/profile/useProfileHistory.ts`                                   | Config history ≠ launch history                                                                                                                                                   |
| `CollapsibleSection`                                                | `ui/CollapsibleSection`                                                | For conditional sections (prefix deps)                                                                                                                                            |

### New shared modules (extract-first, both call sites keep working)

1. **`useLaunchSubTabsProps`** (new, e.g. `src/hooks/launch/useLaunchSubTabsProps.ts`) — extract the 40-prop assembly currently inline in `LaunchPage.tsx:93-213` (consuming `ProfileContext` + ProtonDB hooks + bundled presets + `useLaunchDepGate.isGamescopeRunning`). `LaunchPage` AND the Hero Detail launch tab both consume it. **Blocks** embedding `LaunchSubTabs`.
2. **`useProfileActions`** (new, e.g. `src/hooks/profile/useProfileActions.ts`) — extract duplicate/rename/delete/TOML-preview/community-export/history/mark-verified handlers + busy/error state from `useProfilesPageState.ts:21-326` (and rename toast/undo/F2 from `useProfilesPageNotifications.ts`). `ProfilesPage` AND Hero Detail Profiles both consume it.
3. **`HeroProfileEditorSections`** (new, `components/library/profiles/`) — ordered section renderer mirroring `ProfileSubTabs.tsx:184-306` order without Radix tabs.

### File decomposition (~500-line soft cap)

- `HeroDetailLaunchTab.tsx` (294 now) → split into `components/library/launch/`: tab shell, `HeroLaunchSubTabsHost.tsx` (LaunchSubTabs + bridge), `HeroLaunchCommandSection.tsx` (command/copy/export/launch + `ExportDesktopButton`), `HeroLaunchGate.tsx` (dep gate + feedback wiring).
- `HeroDetailProfilesTab.tsx` (288 now) → split into `components/library/profiles/`: tab shell, `HeroProfileEditorSections.tsx`, `HeroProfileCardList.tsx`, `HeroProfileActionsBar.tsx`, `useHeroProfilesAutosave.ts` (move the `:115-164` effect/selectCard).
- Component conventions: `export interface <Component>Props` + named export + trailing default export; BEM `crosshook-hero-detail__<element>`; `DashboardPanelSection` with `title`/`titleAs="h3"`/`actions` for sections; styles in `styles/hero-detail.css`.

### Prop bridge conventions

`GameDetail` builds memoized `panelProps` (`GameDetail.tsx:173-220`) → `HeroDetailTabs` → `HeroDetailPanels` switch (`HeroDetailPanels.tsx:133-221`). The launch branch deliberately reads `updateProfile` from `useProfileContext()`, not props (`HeroDetailPanels.tsx:160-163`). New parity data flows the same way: consume contexts in the tab; add `panelProps` entries only for GameDetail-derived data (preview, launchRequest). **Every new prop added to the memo must also be added to its dependency array** (`GameDetail.tsx:195` TODO precedent; lessons.md: `key` does not survive inside a spread props object — pass `key` directly in JSX).

### UI / responsive / scroll rules (issue hard requirements)

- **Navigation pattern**: reuse the existing nested Radix `Tabs` styled as `crosshook-subtab-row` pill bar (`theme.css:125-157`) — `LaunchSubTabs` brings its own. Pill row is `flex-wrap: wrap` (`theme.css:127`) so it wraps, never overflows. `CollapsibleSection` for conditional sections.
- **Nested chrome check**: `LaunchSubTabs` renders its own `crosshook-subtabs-shell` panel/backdrop (cover art). Nested inside Hero Detail's tab content this may produce double panel chrome / nested `Tabs.Root` — add CSS to flatten the shell when hosted in `crosshook-hero-detail__launch-tab` (assumption flagged by research; verify visually in browser dev mode).
- **Scroll containers (HARD RULE)**: any new `overflow-y: auto` container MUST be added to `SCROLL_ENHANCE_SELECTORS` in `hooks/useScrollEnhance.ts:8-9` and get `overscroll-behavior: contain`. Currently registered & relevant: `.crosshook-subtab-content__inner--scroll`, `.crosshook-hero-detail__body`, `.crosshook-hero-detail__profiles-editor`. The profiles card list (`__profiles-cards`) has no own scroll today — if it gains one, register it. Horizontal-only scrollers (command block pattern, `hero-detail.css:253-270`) don't need registration but need `overscroll-behavior: contain`.
- **Overflow mitigations to replicate**: `min-width: 0` cascades (`hero-detail.css:316-323`), env grid collapse `<720px` (`hero-detail.css:716-718`), two-pane collapse at `max-width:720px` (`hero-detail.css:702-704`), `flex-wrap` on action rows (`hero-detail.css:229-234`). Overflow vectors are fixed multi-column grids and non-wrapping button rows — the new `HeroProfileActionsBar` (7 actions) must wrap.
- **Status feedback**: reuse `crosshook-launch-autosave-chip--{idle,saving,success,error}` (`theme.css:2446-2481`) with `aria-live="polite" aria-atomic="true"`; `role="status"`/`role="alert"` for success/error lines; missing data renders **disabled controls, never removed** (pattern: `HeroDetailLaunchTab.tsx:214-225`).

### Security MUST NOTs (verified contracts)

1. **MUST NOT** generate `.desktop`/script content in React. Sole generators: `build_desktop_entry_content` (`crates/crosshook-core/src/export/launcher/content.rs:294`) and `build_trainer_script_content` (`content.rs:9`). Frontend builds the typed request (`utils/launcherExport.ts:48`) and calls `validate_launcher_export` → `export_launchers` (preserve ordering, `useLauncherExport.ts:123-124`).
2. **MUST NOT** persist invalid env rows. `profile_save` does NOT sanitize `custom_env_vars` server-side (`lifecycle.rs:99-200`) — the `applyRows` gate (`CustomEnvironmentVariablesSection.tsx:149-161`) is the only guard on the TOML write path. Keep the `RESERVED_CUSTOM_ENV_KEYS`/`BLOCKED_ENV_KEY_PREFIXES` frontend mirror (`:5-22`) in sync with `protondb/aggregation.rs:10-24`.
3. **MUST NOT** introduce `dangerouslySetInnerHTML` (codebase has zero). Render commands/ProtonDB text as React text children (`HighlightedCommandBlock.tsx:111-115`).
4. **MUST NOT** compute launcher/home paths client-side (`resolve_target_home_path`, `paths.rs:62`; `check_launcher_exists` derives server-side).
5. **MUST NOT** add `any`-typed IPC bridges. Use `callCommand<T>('snake_case_name', …)` with Serde-mirrored interfaces (`useLauncherExport.ts:12-36`). No Rust changes expected; if any, run `./scripts/check-host-gateway.sh`.

### Test patterns (cookbook)

- **Focused tab tests (Strategy A)**: hand-rolled `vi.mock` of contexts/IPC — `HeroDetailLaunchTab.test.tsx:9-30`, `HeroDetailProfilesTab.test.tsx:27-49` (child sections stubbed to `<div>`; `callCommandMock` switch on command name).
- **Integration/a11y (Strategy B)**: `vi.mock('@/lib/ipc')` → `mockCallCommand` from `@/test/render` (`render.tsx:44-48`); seed every command or it throws `[test-mock] Unhandled command`. `LaunchSubTabs.test.tsx:55-107` shows the full provider stack the legacy launch surface needs (`TooltipProvider > ProfileProvider > PreferencesProvider > ProfileHealthProvider > HostReadinessProvider > CollectionsProvider > LaunchStateProvider`) — the Hero Detail harness inherits this when `LaunchSubTabs` is embedded.
- **a11y harness**: `components.a11y.test.tsx:238-268` — launch-options case needs BOTH `ProfileProvider` + `PreferencesProvider` (issue-470 lesson); new context consumers must be added to the harness or the suite fails.
- **Debounced autosave**: positive case → real timers + `waitFor(..., { timeout: 1000 })` (`HeroDetailLaunchTab.test.tsx:268-289`); negative case → fake timers + `vi.advanceTimersByTimeAsync` + `vi.useRealTimers()` in `finally` (`:291-309`). Import `launchOptimizationsAutosaveDelayMs`, never hardcode 350.
- **Panel tests mock child tabs** (`HeroDetailPanels.test.tsx:12-31`); **GameDetail tests assert `panelProps` shape** via `expect.objectContaining` (`GameDetail.test.tsx:80-98`).
- **Console-error guard**: spy `console.error`, assert not called (`HeroDetailProfilesTab.test.tsx:165-171`) — React act/key/ARIA warnings become hard failures.
- **Biome gotchas**: explicit `type="button"` everywhere; `<button>` inside `<li>` for clickable cards; label wrapping `figure` not `pre`; stable generated keys (no array index); real `<label>`+control pairs.
- **Fixture dedup opportunity**: `makeLaunchRequest()`/`makePreview()` are copy-pasted across 3 test files — move to `@/test/fixtures` (Task 4.2).
- **Guard nested profile reads** (lessons.md 2026-04-04): sparse TOML profiles can deserialize without sections — guard `profile.runtime`/`profile.steam`/`profile.trainer` access in all ported sections.

## Storage Boundary & Persistence

| Datum                                                                                                                                    | Classification                                                                                      |
| ---------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| All per-profile config edited by parity surfaces (runner method, trainer, gamescope, mangohud, optimizations, env vars, launcher fields) | **Existing TOML** via `GameProfile` + `profile_save` / granular `profile_save_*` — no schema change |
| Launcher export results, config history, ProtonDB cache                                                                                  | **Existing SQLite metadata** paths already used by backend commands — unchanged                     |
| Selection, expanded sections, copy/export status, preview loading, autosave chip state, dep-gate modal state                             | **Runtime-only React state**                                                                        |

- **Backward compatibility**: existing profile TOML files load/save without migration (no schema change; sparse-section guards per lessons.md).
- **Offline**: all local profile editing works offline; ProtonDB/community guidance degrades exactly as the legacy route does (inline error/disabled states).
- **Degraded behavior**: missing preview/export/dependency data renders inline disabled/error states — controls are never silently removed.
- **User visibility/editability**: every control classified **Port** above is visible and editable from Hero Detail after this plan; **Superseded** rows are documented here as the user-facing record.

---

## Tasks

### Batch 1 — shared extraction (composition-only; legacy routes stay green)

**Task 1.1 — Extract `useLaunchSubTabsProps` bridge hook**
**Depends on**: []
Create `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts` extracting the `LaunchSubTabs` prop-assembly from `LaunchPage.tsx:93-213` (ProtonDB hooks wiring, optimization preset names/catalog, gamescope/mangohud/optimizations payloads, env autosave handler, `isInsideGamescopeSession`). Refactor `LaunchPage.tsx` to consume it — behavior-neutral, asserted by the existing legacy route tests. Input: the profile-state surface (`useProfileContext()`) + dep-gate `isGamescopeRunning` + `resolvedSteamAppId`/`effectiveSteamClientInstallPath` params so both call sites can supply their derivations.
Validation: `npm exec vitest run src/components/__tests__/LaunchSubTabs.test.tsx` + legacy launch route tests pass unchanged; typecheck.

**Task 1.2 — Extract `useProfileActions` shared hook**
**Depends on**: []
Create `src/crosshook-native/src/hooks/profile/useProfileActions.ts` extracting duplicate/rename/delete/TOML-preview/community-export/config-history/mark-verified handlers, busy flags, and error state from `useProfilesPageState.ts` (and the rename modal/toast/undo/F2 wiring from `useProfilesPageNotifications.ts` where separable). Refactor `ProfilesPage.tsx`/`useProfilesPageState.ts` to consume it — behavior-neutral. Keep `canSave/canDelete/canDuplicate/canRename` derivations with the hook.
Validation: legacy profiles route tests pass unchanged; typecheck.

**Task 1.3 — Profiles editor section renderer + tab decomposition scaffold**
**Depends on**: []
Create `src/crosshook-native/src/components/library/profiles/` with `HeroProfileEditorSections.tsx` (ordered prop-driven section list mirroring `ProfileSubTabs.tsx:184-306`: Identity → RunnerMethod → Runtime → Game → GameMetadataBar → Media → Trainer → trainer-Gamescope → LauncherExport slot, each behind feature props so Task 2.2 can wire incrementally), `HeroProfileCardList.tsx` (move card list from `HeroDetailProfilesTab.tsx:180-227`), and `useHeroProfilesAutosave.ts` (move the 350ms draft autosave + flush-before-switch from `:115-164`). `HeroDetailProfilesTab.tsx` becomes a shell composing them with identical current behavior (4 sections).
Validation: `npm exec vitest run src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` passes unchanged; typecheck.

**Task 1.4 — CSS groundwork + scroll registration**
**Depends on**: []
In `styles/hero-detail.css`: rules to flatten the nested `crosshook-subtabs-shell` chrome when hosted inside `crosshook-hero-detail__launch-tab` (no double panel/backdrop); wrap rules for a profiles actions bar (`flex-wrap`); `min-width: 0` cascades for new editor sections; narrow-width (`max-width:720px`) behavior for new surfaces. Register any new `overflow-y: auto` container in `SCROLL_ENHANCE_SELECTORS` (`hooks/useScrollEnhance.ts:8`) with `overscroll-behavior: contain`. No component changes — selectors target classes Tasks 2.x will introduce (document each selector with a comment).
Validation: typecheck + full vitest unaffected; lint.

### Batch 2 — embed parity surfaces (disjoint files, parallel)

**Task 2.1 — Hero Launch tab: embed `LaunchSubTabs` + ProtonDB**
**Depends on**: [1.1, 1.4]
Split `HeroDetailLaunchTab.tsx` into `components/library/launch/` (shell + `HeroLaunchCommandSection.tsx` + `HeroLaunchSubTabsHost.tsx`). `HeroLaunchSubTabsHost` consumes `useLaunchSubTabsProps` (1.1) and mounts `LaunchSubTabs` — bringing Environment (replaces the current bare env section to avoid duplication), Gamescope, MangoHud, Optimizations (toggles + named/bundled/manual presets), Steam Options, Offline (readiness panel, launch-path warnings, trainer-hash actions, auto-switch), merged autosave chip, and ProtonDB lookup/overwrite/suggestions. All writes through context mutators only (single persistence path). Keep: command block, Dry-run/Copy/`.desktop`/Launch action row, hooks placeholder (disabled, #471). Guard sparse profile sections (`profile.launch?.…`). Surfaces gated on `singletonOwnsGame`/selected-profile-match render disabled with inline hint, never removed.
Validation: `npm exec vitest run src/components/library/__tests__/HeroDetailLaunchTab.test.tsx src/components/__tests__/LaunchSubTabs.test.tsx`; typecheck.

**Task 2.2 — Hero Profiles tab: full editor parity sections**
**Depends on**: [1.3, 1.4]
Wire into `HeroProfileEditorSections` (1.3): `RunnerMethodSection`, `TrainerSection` (+`TrainerVersionSetField`), trainer-gamescope `GamescopeConfigPanel` (+derived-from-game notice per `ProfileSubTabs.tsx:258-284`), `GameMetadataBar`, prefix-deps `PrefixDepsPanel` inside `CollapsibleSection`, runtime suggestion banner (ProtonUp suggestion/install via the `useProfilesPageProton` surface or an extracted equivalent), health issues list + badge click-to-scroll + stale-check note + trainer-type/version-status/network-isolation chips. All edits flow through the existing draft autosave (1.3 hook) — no granular writes from this tab. Guard sparse sections.
Validation: `npm exec vitest run src/components/library/__tests__/HeroDetailProfilesTab.test.tsx`; typecheck.

### Batch 3 — actions and in-place launch (build on Batch 2 files)

**Task 3.1 — In-place launch + dependency gate + feedback**
**Depends on**: [2.1]
Add `HeroLaunchGate.tsx` to `components/library/launch/`: wire `useLaunchStateContext().launchGame`/`launchTrainer`/`validateLaunch`, `useLaunchDepGate` + `LaunchDepGateModal`, `onBeforeLaunch` interception, `LaunchPanelFeedback` (success/error + expandable diagnostic + copy JSON report), `LaunchPipeline`, helper log path, and guidance/hint text. **selectProfile-first**: before launch/gate actions, ensure the displayed profile is selected into `ProfileContext` (mirror `LibraryPage.tsx:170`); disable in-place launch with inline hint when that's not possible. Replace the Launch button's navigate-to-`/launch` behavior _within Hero Detail only_ (`GameDetail`/tab `onLaunch` path) — do NOT touch global navigation, `AppRoute`, or palette dispatch (Phases 8/9). Add trainer launch affordance gated by `canLaunchTrainer` semantics (`LaunchPanel.tsx:150`).
Validation: focused launch tab tests; typecheck.

**Task 3.2 — Profile lifecycle actions UI + full LauncherExport**
**Depends on**: [1.2, 2.2]
Add `HeroProfileActionsBar.tsx` consuming `useProfileActions` (1.2): Duplicate, Rename (modal + undo toast + F2), Delete (collection-aware confirm overlay), TOML Preview (modal), Community Export, Config History/Rollback (via `useProfileHistory` + `RollbackPanel`), Mark as Verified. Per-action busy labels and `role="alert"` error surface (mirror `ProfileActions.tsx:116-182` semantics). Mount the full `LauncherExport` panel (with `pendingReExport` handling per `ProfileSubTabs.tsx:287-306`) in the editor's export slot. Rename must pause the draft autosave (existing rename-pause guard pattern, `HeroDetailProfilesTab` tests). Actions bar wraps at narrow widths (1.4 CSS).
Validation: focused profiles tab tests; typecheck.

### Batch 4 — wiring + tests

**Task 4.1 — `GameDetail` / `HeroDetailPanels` bridge + integration/a11y test updates**
**Depends on**: [3.1, 3.2]
Update the `panelProps` memo (`GameDetail.tsx:173-220`, including dependency array) and the `HeroDetailPanels` branches for any new GameDetail-derived props; update `GameDetail.test.tsx` `objectContaining` assertions, `HeroDetailPanels.test.tsx` (child tabs stay mocked), and `components.a11y.test.tsx` harness — the launch-options case now needs the full provider stack from `LaunchSubTabs.test.tsx:91-107`; seed all newly-fired IPC commands in `mockCallCommand` handler maps.
Validation: `npm exec vitest run src/components/library/__tests__/GameDetail.test.tsx src/components/library/__tests__/HeroDetailPanels.test.tsx src/__tests__/a11y/components.a11y.test.tsx`.

**Task 4.2 — Launch parity focused tests + shared fixtures**
**Depends on**: [3.1]
Move duplicated `makeLaunchRequest`/`makePreview` builders into `@/test/fixtures`; update the 3 consuming test files. Extend `HeroDetailLaunchTab.test.tsx` (or a new `HeroLaunchSubTabsHost.test.tsx`): sub-tab rendering and method gating, optimization toggle → context mutator (NOT direct IPC), bundled/manual preset actions, gamescope/mangohud panels render + session guard, ProtonDB lookup/accept/dismiss/overwrite, offline auto-switch, dep-gate modal flow (gated launch), in-place launch via `LaunchStateContext` with selectProfile-first, launch feedback render, merged autosave chip, disabled-not-removed states. Console-error guard on every test.
Validation: new + existing focused tests pass.

**Task 4.3 — Profiles parity focused tests**
**Depends on**: [3.2]
Extend `HeroDetailProfilesTab.test.tsx` (+ new focused files for the actions bar/editor sections as needed): runner-method change → draft autosave, trainer section edit + version set, trainer-gamescope derived notice, each lifecycle action (duplicate/rename+undo/delete confirm/preview/export/history/mark-verified) with busy + error states, rename-pause autosave, prefix-deps panel, runtime suggestion banner, health issues list + badge scroll, full LauncherExport mount, flush-before-switch still ordered (`invocationCallOrder`). Console-error guard.
Validation: new + existing focused tests pass.

### Batch 5 — responsive verification + finalization

**Task 5.1 — Responsive no-horizontal-overflow checks**
**Depends on**: [4.1]
Add Playwright assertions to `tests/smoke.spec.ts` (browser dev mode, `?fixture=populated`): open Hero Detail, switch to `launch-options` and `profiles` tabs, and for each viewport in `SWEEP_VIEWPORTS` (`smoke.spec.ts:440-445`) plus the 1024×800 deck case, assert `el.scrollWidth <= el.clientWidth` for `document.documentElement`, `.crosshook-hero-detail__launch-tab`, and `.crosshook-hero-detail__profiles-editor` via `page.evaluate` (style: `smoke.spec.ts:506-527`). Keep zero-console-error assertions. (No such assertion exists today — this is new coverage required by the issue AC.)
Validation: `npm run test:smoke` green (note: requires dev server; if 5173 is busy, verify the running server with `curl` and run Playwright against it — issue-469 lesson).

**Task 5.2 — Finalize parity inventories + full validation**
**Depends on**: [4.2, 4.3, 5.1]
Reconcile the two inventory tables in this plan against the shipped UI: flip each **Port** row to **Ported** (or correct to Superseded/Deferred with justification); confirm every legacy control is classified. Verify legacy `/launch` and `/profiles` routes still fully work (their test packs + a browser pass). Run the complete validation suite (below) and the dependency guard.
Validation: full suite below, all green.

## Batches

Ready for `/ycc:prp-implement --parallel`:

| Batch | Tasks (parallel within batch) | Gate                                                            |
| ----- | ----------------------------- | --------------------------------------------------------------- |
| 1     | 1.1, 1.2, 1.3, 1.4            | All independent extractions; legacy + hero tests unchanged      |
| 2     | 2.1, 2.2                      | Disjoint files (`library/launch/` vs `library/profiles/`)       |
| 3     | 3.1, 3.2                      | 3.1 extends launch files, 3.2 extends profiles files — disjoint |
| 4     | 4.1, 4.2, 4.3                 | 4.1 = bridge/integration files; 4.2/4.3 = focused test files    |
| 5     | 5.1, then 5.2                 | 5.2 depends on 5.1 — run sequentially within the batch          |

Merge-conflict note: `HeroDetailPanels.tsx` is touched only in 4.1 (single owner). `hero-detail.css` is touched in 1.4 (selectors pre-declared) and possibly 2.x — if both, last-to-merge rebases (same convention as #469/#470).

## Validation Commands

All from `src/crosshook-native/` unless noted; `lint.sh` from repo root.

```bash
# Static analysis
npm run typecheck
# EXPECT: zero errors (checks app + test tsconfigs)

# Focused tests (extend with new files as created)
npm exec vitest run \
  src/components/library/__tests__/HeroDetailLaunchTab.test.tsx \
  src/components/library/__tests__/HeroDetailProfilesTab.test.tsx \
  src/components/library/__tests__/HeroDetailPanels.test.tsx \
  src/components/library/__tests__/GameDetail.test.tsx \
  src/components/__tests__/LaunchSubTabs.test.tsx \
  src/__tests__/a11y/components.a11y.test.tsx
# EXPECT: all pass, zero console.error

# Full frontend suite
npm test
# EXPECT: all pass (baseline 247+ tests; count grows with new files)

# Lint (repo root)
./scripts/lint.sh --modified
# EXPECT: exit 0; pre-existing warnings in unrelated files are non-blocking — touched files clean

# Build
npm run build
# EXPECT: tsc + vite build succeed

# Responsive smoke (requires browser dev server; see port-5173 lesson)
npm run test:smoke
# EXPECT: pass incl. new no-horizontal-overflow assertions

# Dependency guard (repo root)
git diff -- package.json src/crosshook-native/package.json package-lock.json src/crosshook-native/package-lock.json
# EXPECT: empty — no new dependency for parity UI
```

## Acceptance Criteria (from #486)

- [x] Launch parity inventory documents every legacy Launch route sub-tab/option as ported / intentionally omitted (superseded) / deferred — finalized in Task 5.2.
- [x] Profiles parity inventory ditto.
- [x] Hero Detail `launch-options` exposes all legacy Launch route user-facing controls that would otherwise be lost when `/launch` is removed.
- [x] Hero Detail `profiles` exposes all legacy Profiles route user-facing controls that would otherwise be lost when `/profiles` is removed.
- [x] Existing Launch/Profile route behavior remains working until Phase 10 deletion (legacy test packs green; behavior-neutral extractions).
- [x] No new package dependency added for parity UI.
- [x] `npm run typecheck`, focused Launch/Profile tests, and full frontend tests pass.
- [x] Responsive checks verify no horizontal overflow in Hero Detail Launch/Profile parity surfaces (new Playwright assertions, Task 5.1).

## Risks & Mitigations

| Risk                                                                                                             | L/I | Mitigation                                                                                                                                                                                  |
| ---------------------------------------------------------------------------------------------------------------- | --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Autosave write race: profiles-tab full-draft 350ms vs granular `enqueueLaunchProfileWrite` queue on same profile | M/H | Single autosave owner per surface: launch tab uses granular context mutators only; profiles tab is the only full-draft writer. Existing `latestProfileNameRef` + dirty-gate guards retained |
| `LaunchStateContext` scoped to _selected_ profile, not displayed fallback                                        | M/H | selectProfile-first wiring (Task 3.1); disabled-with-hint when fallback profile displayed                                                                                                   |
| Legacy route breakage from extractions (1.1/1.2)                                                                 | M/H | Composition-only refactors; both call sites compile; legacy test packs run in the same task's validation                                                                                    |
| Nested `LaunchSubTabs` chrome (double panel/backdrop, nested Radix `Tabs.Root`)                                  | M/M | Task 1.4 CSS flattening, verified visually in browser dev mode; nested Radix Tabs roots are independent by design                                                                           |
| Horizontal overflow in narrow pane (new grids/action rows)                                                       | M/M | `min-width:0` cascades, `flex-wrap`, 720px collapse (1.4); new Playwright overflow assertions (5.1)                                                                                         |
| Offline auto-switch tab behavior change in nested context                                                        | M/L | Comes with `LaunchSubTabs` unchanged; covered by focused test                                                                                                                               |
| a11y suite failure from new context consumers                                                                    | M/M | Task 4.1 updates the harness provider stack up front (issue-470 lesson)                                                                                                                     |
| File-size cap breach in tabs                                                                                     | M/L | Pre-planned decomposition into `library/launch/` and `library/profiles/` subdirectories                                                                                                     |
| Doubled idempotent writes (env 400ms + section 350ms)                                                            | M/L | Documented-acceptable (#469 precedent); no new write paths added                                                                                                                            |

## References

- Issue: #486 · Tracker: #478 · Gated phases: #473, #474, #475 · Sibling gate: #487 · Deferred owners: #471, #472, #479, #482
- PRD: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- Prior plans/reports: `docs/prps/plans/completed/github-issue-469-hero-detail-profiles-tab.plan.md`, `…-470-hero-detail-launch-tab.plan.md`, `docs/prps/reports/github-issue-469-…-report.md`, `…-470-…-report.md`
- Lessons: `tasks/lessons.md` (sparse-profile guards, key-in-spread, audit-every-mounted-surface)
- Commit convention: implementation PR = `feat(ui): …` with `Part of #478` + `Closes #486`; this plan file = `docs(internal): …`
