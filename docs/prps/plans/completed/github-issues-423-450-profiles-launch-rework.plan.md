# Plan: GitHub Issues 423 and 450 — Route Rework: Profiles + Launch

## Summary

Implement Phase 11 of the Unified Desktop Redesign for GitHub issues #450 (tracking) and #423 (deliverable). Split three files that sit over the 500-line soft cap (`ProfileFormSections.tsx` 582, `LaunchPage.tsx` 591, `LaunchSubTabs.tsx` 508) into focused submodules **first**, then fully redesign the Profiles and Launch editor routes in the unified shell visual language (panel / pill / kv-row / field-readonly idioms). The work stays frontend-only, preserves every existing IPC call and autosave contract, and lands net-new test coverage for routes that have none today.

## User Story

As a CrossHook user editing a profile or launching a game, I want the densest editor routes to match the redesigned shell — panel-grouped sections, clear readonly values, consistent pill rows, and a mono command preview — so that the most-used surfaces feel like the rest of the native desktop app instead of a repainted form dump.

## Problem → Solution

Today `ProfilesPage` and `LaunchPage` render the new shell chrome but their interior is still the pre-redesign form-dump: free-form `crosshook-field` rows, ad-hoc inline-styled `details/summary` optional sections, inconsistent error surfaces, and no `kv-row` / `pill` idioms that every dashboard route now uses. At the same time `ProfileFormSections.tsx`, `LaunchPage.tsx`, and `LaunchSubTabs.tsx` all sit over the 500-line soft cap — each parent mixes state, handlers, JSX bodies, and helpers in one file. Phase 11 splits each parent into focused submodules (`profile-form/*`, `pages/launch/*`, `launch-subtabs/*`), extracts the duplicated ProtonDB apply orchestration shared between `ProfileFormSections` and `LaunchPage` into a single hook, then wraps the redesigned bodies around the same `DashboardPanelSection` + `crosshook-dashboard-*` class family the dashboard routes already use (with a new `crosshook-editor-field-readonly` idiom added for the editor-specific readonly value treatment). Decision lock #437 explicitly authorizes structural layout changes as long as behavior parity holds.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md`
- **PRD Phase**: Phase 11 — Route rework: Profiles + Launch (editor routes)
- **Estimated Files**: ~24 (13 CREATE, 11 UPDATE)
- **GitHub Issues**: #450 tracking, #423 deliverable (decision lock #437)
- **Persistence Classification**: runtime-only UI composition + React memoization; no new TOML settings, no SQLite schema or data migration, no IPC/backend changes.

## Persistence and Usability

1. **Storage boundary**
   - **TOML settings**: No new user-editable preferences. Route appearance remains code-driven.
   - **SQLite metadata DB**: No new cache/history tables, no migrations. Existing profile/launch data continues to come from `profile_load`, `profile_save`, `profile_save_launch_optimizations`, `profile_save_gamescope_config`, `profile_save_trainer_gamescope_config`, `profile_save_mangohud_config`, `profile_list_summaries`, and `collection_list_profiles`.
   - **Runtime-only state**: Profile draft dirty flag, selected profile, collection filter, active launch subtab, pending ProtonDB overwrite, dep-gate modal state, env-var autosave debounce timer, and autosave chip visibility all remain in React memory with unchanged semantics.

2. **Migration / backward compatibility**: No settings or schema migration is required. Older builds simply render the pre-redesign editor chrome around the same underlying profile data.

3. **Offline behavior**: Fully offline. No new network dependency is introduced. Existing offline-readiness chips, ProtonDB offline reasons, and cached trainer artifacts must keep rendering inside the redesigned chrome.

4. **Failure fallback**: If profile load, save, or any autosave IPC fails, the redesigned routes must continue to surface the same inline `crosshook-danger` banners (not toasts, not modal blocks). The `LaunchPage` dep-gate silent-catch behavior stays silent — changing it is out of scope for this plan.

5. **User visibility / editability**: Users see the redesigned editor chrome; there is no new setting to configure editor layout. Existing preferences that affect Profiles/Launch (e.g. `auto_install_prefix_deps`, `umu_preference`, `default_steam_client_install_path`) are unchanged.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks              | Depends On | Parallel Width |
| ----- | ------------------ | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3, 1.4 | —          | 4              |
| B2    | 2.1, 2.2           | B1         | 2              |
| B3    | 3.1, 3.2           | B2         | 2              |

- **Total tasks**: 8
- **Total batches**: 3
- **Max parallel width**: 4

---

## Worktree Setup

- **Parent**: `~/.claude-worktrees/crosshook-profiles-launch-rework/` (branch: `feat/profiles-launch-rework`)
- **Children** (per parallel task; merged back at end of each batch):
  - Task 1.1 → `~/.claude-worktrees/crosshook-profiles-launch-rework-1-1/` (branch: `feat/profiles-launch-rework-1-1`)
  - Task 1.2 → `~/.claude-worktrees/crosshook-profiles-launch-rework-1-2/` (branch: `feat/profiles-launch-rework-1-2`)
  - Task 1.3 → `~/.claude-worktrees/crosshook-profiles-launch-rework-1-3/` (branch: `feat/profiles-launch-rework-1-3`)
  - Task 1.4 → `~/.claude-worktrees/crosshook-profiles-launch-rework-1-4/` (branch: `feat/profiles-launch-rework-1-4`)
  - Task 2.1 → `~/.claude-worktrees/crosshook-profiles-launch-rework-2-1/` (branch: `feat/profiles-launch-rework-2-1`)
  - Task 2.2 → `~/.claude-worktrees/crosshook-profiles-launch-rework-2-2/` (branch: `feat/profiles-launch-rework-2-2`)
  - Task 3.1 → `~/.claude-worktrees/crosshook-profiles-launch-rework-3-1/` (branch: `feat/profiles-launch-rework-3-1`)
  - Task 3.2 → `~/.claude-worktrees/crosshook-profiles-launch-rework-3-2/` (branch: `feat/profiles-launch-rework-3-2`)

Each child worktree must pre-install local dev dependencies per `CLAUDE.md § Worktree setup prerequisites` so `./scripts/lint.sh` and `./scripts/format.sh` work:

```bash
npm install -D --no-save typescript@$(cat src/crosshook-native/package.json | jq -r .devDependencies.typescript) biome
cd src/crosshook-native && npm ci
```

---

## UX Design

### Before

```text
Profiles route (state-driven):
  RouteBanner
    └─ ProfilesHero (pinned + selector)
    └─ ProfileSubTabs (radix tabs: Setup / Runtime / Game Art / Trainer / Gamescope / Export)
         └─ ProfileFormSections (monolithic grid of fields, inline details/summary
                                  for optional groups, raw crosshook-field rows)

Launch route:
  RouteBanner
    └─ LaunchPanel
         ├─ profileSelectSlot: collection filter + ThemedSelect
         ├─ LaunchPanelControls  (launch + info buttons)
         ├─ LaunchPipeline       (step dots)
         └─ tabsSlot: LaunchSubTabs
              └─ 6 Tabs.Content bodies (Offline / Environment / Gamescope /
                                        MangoHud / Optimizations / Steam Options)
    └─ LaunchDepGateModal (prefix-dep install, inline subtree)
```

### After

```text
Profiles route (panel-grouped, readonly values explicit):
  RouteBanner
    └─ ProfilesHero (unchanged wiring; wrapped in crosshook-dashboard-pill-row
                     for summary chips)
    └─ ProfileSubTabs
         └─ Each tab body uses DashboardPanelSection groupings:
              • Identity panel  (name + description kv-rows)
              • Game panel      (path + readonly metadata in crosshook-editor-field-readonly)
              • Runner panel    (method pick + proton pill row)
              • Runtime panel   (env var + ProtonDB in a sibling panel,
                                 command preview in mono panel)
              • Trainer panel   (version + launcher metadata)

Launch route (panel-grouped, mono command preview):
  RouteBanner
    └─ LaunchPanel (shell unchanged; slots re-theme internally)
         ├─ profileSelectSlot: crosshook-dashboard-pill-row with
                               collection chip + ThemedSelect
         ├─ LaunchPanelControls
         ├─ LaunchPipeline (mono-styled step pill row)
         └─ tabsSlot: LaunchSubTabs
              └─ Each tab body wrapped in DashboardPanelSection with
                 unified autosave chip in the header actions slot
    └─ LaunchDepGateModal (unchanged behavior; chrome uses crosshook-panel)
```

### Interaction Changes

| Touchpoint                    | Before                                            | After                                                                          | Notes                                                                            |
| ----------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| Profile form field rows       | Raw `crosshook-field` + `<label>` + `<input>`     | Same primitives wrapped in `DashboardPanelSection` groups with kv-row metadata | Behavior parity — `onUpdateProfile` pathway unchanged                            |
| Profile readonly metadata     | Inline `<span>` values                            | `crosshook-editor-field-readonly` class with explicit "value pending" copy     | Net-new idiom; introduced in Task 1.1 so all editor readonly values share a look |
| Profile optional sections     | `<details>` with ad-hoc inline-styled summary     | `DashboardPanelSection` with `eyebrow="Optional"` + action slot                | Drop 30+ lines of inline styles                                                  |
| Launch collection filter chip | Bespoke `crosshook-launch-collection-filter` span | `crosshook-dashboard-pill` inside header pill row                              | Same data, consistent chip                                                       |
| Launch command preview        | Inline `<code>` with minor styling                | `crosshook-editor-mono-panel` wrapped in `DashboardPanelSection`               | Aligns with design's "mono/panel style" directive for command preview            |
| LaunchSubTabs autosave chip   | Floating chip absolutely positioned               | Moved into tab's `DashboardPanelSection` `actions` slot (per active tab)       | Same `combinedAutoSaveStatus` logic; chip becomes a pill in the panel header     |
| LaunchDepGateModal chrome     | Inline modal overlay with raw panels              | Same modal overlay; inner content uses `crosshook-panel` + pill row            | Preserves dep-install event subscription and silent-catch dep-check semantics    |
| Pipeline step indicators      | `.crosshook-launch-pipeline__node` custom styles  | Same structure; tokens align with `crosshook-dashboard-pill` via CSS variable  | Behavior unchanged; CSS-only adjustment                                          |
| Profile save / autosave chips | Ad-hoc chip styling                               | Unified `crosshook-dashboard-pill` tone variants with same status text         | No change to IPC, debounce window, or `hasSavedSelectedProfile` gate             |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File                                                                              | Lines            | Why                                                                                                              |
| -------- | --------------------------------------------------------------------------------- | ---------------- | ---------------------------------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-redesign.prd.md`                                  | 283-288, 313-325 | Phase 11 goal/scope/signal + decision-log entries that authorize the full redesign.                              |
| P0       | `docs/prps/plans/completed/github-issues-421-448-route-rework-dashboards.plan.md` | 122-252, 286-367 | The closest sibling plan — mirror its Patterns-to-Mirror, split-then-redesign, and batch structure.              |
| P0       | `src/crosshook-native/src/components/ProfileFormSections.tsx`                     | 1-582            | Entire file — every section is either extracted in Task 1.2 or consumed by the redesign in Task 2.1.             |
| P0       | `src/crosshook-native/src/components/pages/LaunchPage.tsx`                        | 1-591            | Entire file — 591-line page; split seams and autosave parity contract live here.                                 |
| P0       | `src/crosshook-native/src/components/LaunchSubTabs.tsx`                           | 1-508            | Entire file — auto-save chip merge, offline auto-switch effect, cover-art backdrop, and 6 tab bodies.            |
| P0       | `src/crosshook-native/src/components/layout/DashboardPanelSection.tsx`            | 1-91             | Shared panel primitive introduced in Phase 9; Phase 11 reuses it verbatim.                                       |
| P0       | `src/crosshook-native/src/styles/dashboard-routes.css`                            | 1-142            | Canonical `crosshook-dashboard-pill` / `crosshook-dashboard-kv-row` class set Phase 11 reuses.                   |
| P0       | `src/crosshook-native/src/components/pages/ProfilesPage.tsx`                      | 1-318            | ProfilesPage is the outer wrapper; Phase 11 reskins its sections via `ProfileSubTabs`.                           |
| P0       | `src/crosshook-native/src/components/ProfileSubTabs.tsx`                          | 1-311            | Radix Tabs shell for Profiles — contract must preserve `forceMount` behavior, tab order, and prop surface.       |
| P0       | `src/crosshook-native/src/components/pages/profiles/useProfilesPageState.ts`      | 1-326            | Canonical thin-page-wrapper state hook; pattern to mirror for any new Launch page state extraction.              |
| P0       | `src/crosshook-native/src/components/LaunchPanel.tsx`                             | 1-203            | Reusable launch shell; slot contract (`profileSelectSlot`, `tabsSlot`, `infoSlot`, `beforeActions`) is frozen.   |
| P0       | `src/crosshook-native/src/components/launch-panel/types.ts`                       | 1-23             | `LaunchPanelProps` interface — must not change in this phase.                                                    |
| P0       | `src/crosshook-native/src/hooks/profile/useProfileCrud.ts`                        | 142-305          | `persistProfileDraft`, `updateProfile`, `updateLaunchSetting`, `saveProfile` semantics — parity-critical.        |
| P0       | `src/crosshook-native/src/hooks/useProfile.ts`                                    | 32-96, 144-191   | `ProfileContextValue` surface + `profiles-changed` event subscription — parity-critical.                         |
| P0       | `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts`       | 1-347            | Per-field autosave IPC surface (gamescope, mangohud, optimizations, trainer-gamescope).                          |
| P0       | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                              | 1-48             | `SCROLL_ENHANCE_SELECTORS` — any new `overflow-y:auto` container MUST be appended here.                          |
| P1       | `src/crosshook-native/src/components/layout/routeMetadata.ts`                     | 31-139           | `ROUTE_METADATA` entries for `profiles` and `launch`; banner copy stays unless a new summary is intentional.     |
| P1       | `src/crosshook-native/src/components/layout/RouteBanner.tsx`                      | 1-31             | Banner contract — do not change.                                                                                 |
| P1       | `src/crosshook-native/src/components/pages/CompatibilityPage.tsx`                 | 42-292           | Sibling route using `DashboardPanelSection` + pill-row + kv-row idioms; closest shape for the redesigned layout. |
| P1       | `src/crosshook-native/src/components/pages/ProtonManagerPage.tsx`                 | 13-78            | Canonical hero-kv-row pattern; Profiles/Launch redesigns reuse this structure.                                   |
| P1       | `src/crosshook-native/src/components/pages/__tests__/DashboardRoutes.test.tsx`    | 1-180            | Test harness shape Phase 11 mirrors: `vi.mock('@/lib/ipc')`, provider stack, `handlerOverrides`.                 |
| P1       | `src/crosshook-native/src/test/render.tsx`                                        | 1-80             | `renderWithMocks` + `mockCallCommand` surface.                                                                   |
| P1       | `src/crosshook-native/src/test/fixtures.ts`                                       | 1-150            | `makeProfileDraft`, `makeProfileHealthReport` factories.                                                         |
| P1       | `src/crosshook-native/tests/smoke.spec.ts`                                        | 1-290            | Route sweep, `DASHBOARD_ROUTE_HEADINGS`, pipeline + profile assertions — Phase 11 extends the headings list.     |
| P1       | `src/crosshook-native/src/utils/protondb.ts`                                      | all              | `applyProtonDbGroupToProfile`, `mergeProtonDbEnvVarGroup`, `PendingProtonDbOverwrite` — shared by the new hook.  |
| P2       | `src/crosshook-native/src/components/launch-panel/LaunchPanelControls.tsx`        | all              | Reference for the redesigned action row treatment.                                                               |
| P2       | `src/crosshook-native/src/components/LaunchPipeline.tsx`                          | 1-125            | Pipeline nav — visual-only touchups; behavior unchanged.                                                         |
| P2       | `src/crosshook-native/src/main.tsx`                                               | 1-20             | Global stylesheet registration; add `editor-routes.css` next to `dashboard-routes.css`.                          |
| P2       | `src/crosshook-native/src/hooks/__tests__/useScrollEnhance.test.ts`               | all              | Selector-registration parity test style.                                                                         |

## External Documentation

| Topic         | Source | Key Takeaway                                                                                                         |
| ------------- | ------ | -------------------------------------------------------------------------------------------------------------------- |
| External docs | none   | No external API/library research is needed; Phase 11 is bound by the PRD, decision #437, and existing repo patterns. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### DASHBOARD_PANEL_SECTION_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/DashboardPanelSection.tsx:53-59
<section className={joinClasses('crosshook-panel', 'crosshook-dashboard-panel-section', className)}>
  {(eyebrow || title || actions) && <header>{…eyebrow/title/actions}</header>}
  <div className="crosshook-dashboard-panel-section__content">{children}</div>
</section>
```

Reuse `DashboardPanelSection` verbatim for every redesigned panel grouping in Profiles and Launch. Do NOT create a parallel `EditorPanelSection`; Phase 11 proved the dashboard primitive is route-agnostic.

### DASHBOARD_PILL_ROW_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/CompatibilityPage.tsx:62-65
<div className="crosshook-dashboard-pill-row">
  <span className="crosshook-dashboard-pill">{label}</span>
  <span className="crosshook-dashboard-pill">{statusChip}</span>
</div>
```

Use this for collection filter chips, readonly status chips (network-isolation badge, trainer status, ProtonDB tone), and the Launch pipeline summary row. Canonical classes live in `dashboard-routes.css:77-96`.

### DASHBOARD_KV_ROW_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/ProtonManagerPage.tsx:48-52
<div className="crosshook-dashboard-kv-row">
  <dt className="crosshook-dashboard-kv-row__label">Effective Steam path source</dt>
  <dd className="crosshook-dashboard-kv-row__value">{source}</dd>
</div>
```

Wrap readonly metadata in `<dl>` blocks of `crosshook-dashboard-kv-row`. Use this for the Profile Game panel (resolved game path, Steam AppID, cover art source) and the Launch header (effective Steam path, umu preference, selected profile). Canonical classes live in `dashboard-routes.css:103-142`.

### ROUTE_FILL_CARD_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/LaunchPage.tsx:402-404
<div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--launch">
  <div className="crosshook-route-stack crosshook-launch-page__grid">
    <RouteBanner route="launch" />
```

Preserve the outer scroll-shell + route-stack contract on both routes. Do NOT swap to a different scroll owner; any new `overflow-y:auto` descendants must register in `SCROLL_ENHANCE_SELECTORS`.

### PROFILES_PAGE_STATE_HOOK_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/pages/profiles/useProfilesPageState.ts:21-30
const {
  profileName,
  profile,
  profiles,
  launchMethod,
  selectProfile,
  updateProfile,
  saveProfile,
  activeCollectionId,
  setActiveCollectionId,
} = useProfileContext();
const { collections } = useCollections();
const { memberNames, membersForCollectionId, loading } = useCollectionMembers(activeCollectionId);
```

Mirror this state-hook composition when Phase 11 lifts the inline state out of `LaunchPage.tsx` into `pages/launch/useLaunchPageState.ts`. Keep the page component thin (orchestration + JSX only).

### THIN_PAGE_WRAPPER_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/ProfilesPage.tsx:13-15
export function ProfilesPage() {
  const state = useProfilesPageState();
  // ... 300+ lines of JSX only
}
```

`LaunchPage` should end up under this shape after Task 1.3 — state hook aggregates, page component composes JSX.

### TRY_FINALLY_ASYNC_ACTION_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/pages/HostToolsPage.tsx:100-106
setProbingToolId(toolId);
try {
  await probeTool(toolId);
} finally {
  setProbingToolId((current) => (current === toolId ? null : current));
}
```

Apply to any new async button action introduced (e.g. when replacing the dep-gate "Install & retry" with a panel action). Existing async handlers already follow this.

### INLINE_ALERT_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/ProfilesPage.tsx:262-266
{
  state.previewError ? (
    <p className="crosshook-danger" role="alert" style={{ marginTop: 12 }}>
      Preview failed: {state.previewError}
    </p>
  ) : null;
}
```

All error surfaces stay inline. Do NOT introduce a toast system; there is no `ToastContext` in the repo today and Phase 11 is not the place to add one.

### CALL_COMMAND_TRY_CATCH_LOG_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/pages/profiles/useProfilesPageState.ts:195-200
} catch (err) {
  const message = err instanceof Error ? err.message : String(err);
  console.error('Profile preview failed:', err);
  setPreviewError(message);
}
```

If any new async handler is added in Task 1.2/1.3 (e.g. extracted ProtonDB apply hook), mirror this exact shape: `console.error('<verb> failed:', err)` + setter. There is no wrapped logger; use `console.error` directly.

### LAUNCH_AUTOSAVE_DEBOUNCE_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/pages/LaunchPage.tsx:370-399
environmentAutosaveTimerRef.current = setTimeout(() => {
  void persistProfileDraftRef.current(latestProfileName, {
    ...latestProfile,
    launch: { ...latestProfile.launch, custom_env_vars: { ...latestNextEnvVarsRef.current } },
  });
}, 400);
```

When Task 1.3 extracts env-var autosave into a hook, keep the 400ms window, the `hasSavedSelectedProfile` gate, the `trigger === 'value' && row.key.trim().length === 0` skip, and the ref-based latest-value capture. Parity is mandatory.

### RADIX_TABS_FORCEMOUNT_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/LaunchSubTabs.tsx:300-305
<Tabs.Content value="offline" forceMount
  className="crosshook-subtab-content"
  style={{ display: activeTab === 'offline' ? undefined : 'none' }}>
```

Extracted tab body components in Task 1.4 must NOT drop `forceMount` or the inline display toggle. Removing either breaks autosave status continuity and the offline auto-switch effect.

### OFFLINE_AUTO_SWITCH_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/LaunchSubTabs.tsx:213-227
const hasOfflineConcern = Boolean(offlineReadinessError) || offlineWarning || launchPathWarnings.length > 0;
useEffect(() => {
  if (hasOfflineConcern && !autoSwitchedRef.current) {
    autoSwitchedRef.current = true;
    setActiveTab('offline');
  }
}, [hasOfflineConcern]);
```

This effect is the reason the Offline tab takes over when trainer-hash warnings surface. Must survive the LaunchSubTabs split; keep it in the parent wrapper, not in the extracted `OfflineTabContent`.

### AUTOSAVE_CHIP_MERGE_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/LaunchSubTabs.tsx:163-193
const TONE_PRIORITY = { idle: 0, success: 1, warning: 2, saving: 3, error: 4 };
const combinedAutoSaveStatus = allStatuses.reduce(
  (best, s) => ((TONE_PRIORITY[s.tone] ?? 0) > (TONE_PRIORITY[best.tone] ?? 0) ? s : best),
  { tone: 'idle', label: '' }
);
```

The combined-status reducer + 3s fade timer must survive the split. Extract to `launch-subtabs/useAutoSaveChip.ts` in Task 1.4; do not change priorities or timings.

### IPC_MOCK_TEST_HARNESS_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/pages/__tests__/DashboardRoutes.test.tsx:16-19
vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});
```

Every new test file added in Task 3.1 must top-level-mock `@/lib/ipc` this way, then provide per-test `handlerOverrides` via `renderWithMocks` to fix `profile_load`, `profile_list_summaries`, `collection_list_profiles`, and the autosave commands.

### SELECTOR_REGISTRATION_TEST_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/__tests__/useScrollEnhance.test.ts:4-8
it('registers the context rail body scroll target exactly once', () => {
  const matches = SCROLL_ENHANCE_SELECTORS.match(/\.crosshook-context-rail__body\b/g);
  expect(matches?.length).toBe(1);
});
```

Every new scroll container added by Task 2.1 / 2.2 gets a matching registration-count test in the same suite.

---

## Files to Change

| File                                                                              | Action | Justification                                                                                                                                                                                                                                                                                                                                                                                      |
| --------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/styles/editor-routes.css`                               | CREATE | New stylesheet paralleling `dashboard-routes.css`; houses the `crosshook-editor-field-readonly` + `crosshook-editor-mono-panel` idioms.                                                                                                                                                                                                                                                            |
| `src/crosshook-native/src/main.tsx`                                               | UPDATE | Register `editor-routes.css` immediately after `dashboard-routes.css`.                                                                                                                                                                                                                                                                                                                             |
| `src/crosshook-native/src/components/profile-form/FormFieldRow.tsx`               | CREATE | Extracts the exported `FieldRow` from `ProfileFormSections.tsx:107-152` into a focused reusable component.                                                                                                                                                                                                                                                                                         |
| `src/crosshook-native/src/components/profile-form/OptionalSection.tsx`            | CREATE | Extracts `OptionalSection` + inline styles from `ProfileFormSections.tsx:57-72, 245-256` into its own file with CSS class replacements.                                                                                                                                                                                                                                                            |
| `src/crosshook-native/src/components/profile-form/ProtonPathField.tsx`            | CREATE | Extracts the exported `ProtonPathField` from `ProfileFormSections.tsx:154-218`.                                                                                                                                                                                                                                                                                                                    |
| `src/crosshook-native/src/components/profile-form/LauncherMetadataFields.tsx`     | CREATE | Extracts `LauncherMetadataFields` from `ProfileFormSections.tsx:220-243`.                                                                                                                                                                                                                                                                                                                          |
| `src/crosshook-native/src/components/profile-form/ProfileSelectorField.tsx`       | CREATE | Extracts the internal `ProfileSelectorField` from `ProfileFormSections.tsx:258-310`; no behavior change.                                                                                                                                                                                                                                                                                           |
| `src/crosshook-native/src/components/profile-form/TrainerVersionSetField.tsx`     | CREATE | Extracts the exported `TrainerVersionSetField` from `ProfileFormSections.tsx:312-368`.                                                                                                                                                                                                                                                                                                             |
| `src/crosshook-native/src/components/profile-form/helpers.ts`                     | CREATE | Hosts `parentDirectory`, `updateGameExecutablePath`, and the shared inline-style-to-class migration helpers.                                                                                                                                                                                                                                                                                       |
| `src/crosshook-native/src/hooks/profile/useProtonDbApply.ts`                      | CREATE | Extracts the duplicated ProtonDB apply orchestration (`applyProtonDbGroup`, `handleApplyProtonDbEnvVars`, `handleAcceptSuggestion`, `pendingProtonDbOverwrite`, `applyingProtonDbGroupId`, `protonDbStatusMessage`) into a single hook shared by `ProfileFormSections` and `LaunchPage`.                                                                                                           |
| `src/crosshook-native/src/components/ProfileFormSections.tsx`                     | UPDATE | Shrink to <500 lines by consuming the `profile-form/*` extracts and `useProtonDbApply`; retain external re-exports (`PendingProtonDbOverwrite`, `ProtonInstallOption`, `FieldRow`, `ProtonPathField`, `TrainerVersionSetField`, `LauncherMetadataFields`) via barrel re-export to keep downstream imports (`UpdateGamePanel`, `RunExecutablePanel`, `RuntimeSection`, `ui/ProtonPathField`) green. |
| `src/crosshook-native/src/components/pages/launch/useLaunchPageState.ts`          | CREATE | Hoists LaunchPage's context pulls, memos, effects, and handlers out of the page component per `THIN_PAGE_WRAPPER_PATTERN`.                                                                                                                                                                                                                                                                         |
| `src/crosshook-native/src/components/pages/launch/useLaunchDepGate.ts`            | CREATE | Extracts prefix-dep gate state + `prefix-dep-complete` subscription from LaunchPage.                                                                                                                                                                                                                                                                                                               |
| `src/crosshook-native/src/components/pages/launch/LaunchDepGateModal.tsx`         | CREATE | Extracts the dep-gate modal JSX (`LaunchPage.tsx:519-586`) into its own component.                                                                                                                                                                                                                                                                                                                 |
| `src/crosshook-native/src/components/pages/launch/LaunchProfileSelector.tsx`      | CREATE | Extracts the `profileSelectSlot` JSX (`LaunchPage.tsx:410-448`) into a focused component; reuses `crosshook-dashboard-pill-row`.                                                                                                                                                                                                                                                                   |
| `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`          | CREATE | Extracts the 400ms env-var autosave timer + refs (`LaunchPage.tsx:193-212, 370-399`) into a hook that preserves the `hasSavedSelectedProfile` gate and trigger guard.                                                                                                                                                                                                                              |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx`                        | UPDATE | Shrink to <500 lines by consuming the `pages/launch/*` extracts and the new hooks; body becomes a thin wrapper around `LaunchPanel`.                                                                                                                                                                                                                                                               |
| `src/crosshook-native/src/components/launch-subtabs/types.ts`                     | CREATE | Hosts `LaunchSubTabId`, `TAB_LABELS`, and the extracted `LaunchSubTabsProps` interface; paralleling `launch-panel/types.ts`.                                                                                                                                                                                                                                                                       |
| `src/crosshook-native/src/components/launch-subtabs/useAutoSaveChip.ts`           | CREATE | Extracts the chip-merge reducer + 3s fade timer from `LaunchSubTabs.tsx:163-193`.                                                                                                                                                                                                                                                                                                                  |
| `src/crosshook-native/src/components/launch-subtabs/useTabVisibility.ts`          | CREATE | Extracts the `showsGamescopeTab` / `showsMangoHudTab` / `showsOptimizationsTab` / `showsSteamOptionsTab` + `tabs[]` builder.                                                                                                                                                                                                                                                                       |
| `src/crosshook-native/src/components/launch-subtabs/OfflineTabContent.tsx`        | CREATE | Extracts the ~70-line offline tab body (status badge + readiness panel + launch-path warnings).                                                                                                                                                                                                                                                                                                    |
| `src/crosshook-native/src/components/launch-subtabs/EnvironmentTabContent.tsx`    | CREATE | Extracts the ~45-line environment tab body (env vars + conditional ProtonDB panel via shared `useProtonDbApply` hook).                                                                                                                                                                                                                                                                             |
| `src/crosshook-native/src/components/launch-subtabs/GamescopeTabContent.tsx`      | CREATE | Thin wrapper around `GamescopeConfigPanel` + Tabs.Content contract.                                                                                                                                                                                                                                                                                                                                |
| `src/crosshook-native/src/components/launch-subtabs/MangoHudTabContent.tsx`       | CREATE | Thin wrapper around `MangoHudConfigPanel` + Tabs.Content contract.                                                                                                                                                                                                                                                                                                                                 |
| `src/crosshook-native/src/components/launch-subtabs/OptimizationsTabContent.tsx`  | CREATE | Thin wrapper around `LaunchOptimizationsPanel` + Tabs.Content contract.                                                                                                                                                                                                                                                                                                                            |
| `src/crosshook-native/src/components/launch-subtabs/SteamOptionsTabContent.tsx`   | CREATE | Thin wrapper around `SteamLaunchOptionsPanel` + Tabs.Content contract.                                                                                                                                                                                                                                                                                                                             |
| `src/crosshook-native/src/components/LaunchSubTabs.tsx`                           | UPDATE | Shrink to <500 lines by consuming the `launch-subtabs/*` extracts; keep the `OFFLINE_AUTO_SWITCH_PATTERN` effect and cover-art backdrop here.                                                                                                                                                                                                                                                      |
| `src/crosshook-native/src/components/pages/ProfilesPage.tsx`                      | UPDATE | Swap inline form layout for `DashboardPanelSection`-wrapped groupings; add pill-row + kv-row idioms; no state changes.                                                                                                                                                                                                                                                                             |
| `src/crosshook-native/src/components/ProfileSubTabs.tsx`                          | UPDATE | Wrap each tab body in `DashboardPanelSection`; preserve Radix `forceMount` per tab; no prop surface change.                                                                                                                                                                                                                                                                                        |
| `src/crosshook-native/src/components/profile-sections/GameSection.tsx`            | UPDATE | Replace raw field layout with `DashboardPanelSection` + `crosshook-dashboard-kv-row` for readonly metadata.                                                                                                                                                                                                                                                                                        |
| `src/crosshook-native/src/components/profile-sections/ProfileIdentitySection.tsx` | UPDATE | Wrap in a `DashboardPanelSection`; convert identity readonly row to `crosshook-editor-field-readonly`.                                                                                                                                                                                                                                                                                             |
| `src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx`    | UPDATE | Wrap in a panel; convert method pills to `crosshook-dashboard-pill-row`.                                                                                                                                                                                                                                                                                                                           |
| `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`         | UPDATE | Wrap env-var subpanel in its own `DashboardPanelSection`; render command preview inside `crosshook-editor-mono-panel`.                                                                                                                                                                                                                                                                             |
| `src/crosshook-native/src/components/profile-sections/TrainerSection.tsx`         | UPDATE | Wrap trainer fields in a panel; consume the extracted `TrainerVersionSetField`.                                                                                                                                                                                                                                                                                                                    |
| `src/crosshook-native/src/components/LaunchPanel.tsx`                             | UPDATE | Theme the action row + slot chrome using `crosshook-dashboard-pill-row`; the public props surface (`LaunchPanelProps`) must not change.                                                                                                                                                                                                                                                            |
| `src/crosshook-native/src/components/LaunchPipeline.tsx`                          | UPDATE | Style steps consistent with `crosshook-dashboard-pill`; no structural change; preserve 6-node assertion from smoke.                                                                                                                                                                                                                                                                                |
| `src/crosshook-native/src/styles/launch-pipeline.css`                             | UPDATE | Align pipeline step tokens with the pill idiom; do not introduce legacy palette literals.                                                                                                                                                                                                                                                                                                          |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                              | UPDATE | Append any new `overflow-y:auto` selectors introduced by Tasks 2.1/2.2 (candidates: `.crosshook-launch-page__command-preview`, env-var list scroll).                                                                                                                                                                                                                                               |
| `src/crosshook-native/src/hooks/__tests__/useScrollEnhance.test.ts`               | UPDATE | Add registration-count parity tests for each new selector added in this phase.                                                                                                                                                                                                                                                                                                                     |
| `src/crosshook-native/src/components/pages/__tests__/ProfilesRoute.test.tsx`      | CREATE | Net-new provider-backed RTL suite: panel rendering, save flow, ProtonDB apply, health-issues banner parity.                                                                                                                                                                                                                                                                                        |
| `src/crosshook-native/src/components/pages/__tests__/LaunchRoute.test.tsx`        | CREATE | Net-new suite: collection filter, dep-gate silent-catch, env-var 400ms autosave window, offline auto-switch, `hasSavedSelectedProfile` gate.                                                                                                                                                                                                                                                       |
| `src/crosshook-native/src/components/__tests__/LaunchSubTabs.test.tsx`            | CREATE | Net-new suite: auto-save chip merge priority, tab visibility matrix, `forceMount` mount persistence across tab switches.                                                                                                                                                                                                                                                                           |
| `src/crosshook-native/tests/smoke.spec.ts`                                        | UPDATE | Add `profiles` + `launch` to `DASHBOARD_ROUTE_HEADINGS` (if banners are asserted); add a smoke assertion for the redesigned panel landing markers on each route.                                                                                                                                                                                                                                   |

## NOT Building

- **No `crosshook-core` / `src-tauri` / IPC changes.** Phase 11 is frontend-only; every existing `#[tauri::command]` stays verbatim.
- **No new TOML settings** — no `inspector_collapsed_override`, no `console_drawer_mode`, no editor layout preference, no sidebar override. The PRD explicitly defers these to a later PRD.
- **No new SQLite tables, migrations, or metadata-DB changes.**
- **No `ToastContext` introduction.** Error surfaces remain inline `crosshook-danger` banners; the decision lock does not authorize adding a toast system.
- **No wrapped logger module.** Existing `console.error` / `console.warn` sites keep their current form; do not gate-keep logging behind a new abstraction.
- **No prop-surface changes to `LaunchPanel`, `LaunchPipeline`, `LaunchStateContext`, `ProfileContext`, or `ROUTE_METADATA`.** Public contracts are frozen for this phase.
- **No replacement of `ProfileFormSections`' external re-exports** (`PendingProtonDbOverwrite`, `ProtonInstallOption`, `FieldRow`, `ProtonPathField`, `TrainerVersionSetField`, `LauncherMetadataFields`). Downstream imports (`UpdateGamePanel`, `RunExecutablePanel`, `ui/ProtonPathField`, `RuntimeSection`) must remain green without modification.
- **No Radix Tabs `forceMount` removal** on ProfileSubTabs or LaunchSubTabs. Dropping `forceMount` silently regresses autosave continuity and the offline auto-switch.
- **No new fuzzy search, recency ranking, or command-palette surfaces.** Those belong to Phase 6 (already done) and a later PRD.
- **No LaunchPage dep-gate silent-catch rewrite.** Parity extends to preserving the current bare `catch {}` behavior; introducing a new error surface is out of scope unless decision #437 is reopened.
- **No responsive-sweep expansion.** Viewport-sweep smoke at 1280/1920/2560/3440 is Phase 12's scope; this phase only extends `DASHBOARD_ROUTE_HEADINGS`.
- **No `gamepad-nav` focus-zone changes.** Any new panel chrome must reuse existing zone primitives via `useFocusTrap` patterns — no n-zone refactor.
- **No token drift.** New CSS MUST use existing `--crosshook-*` tokens; `scripts/check-legacy-palette.sh` stays green.

---

## Step-by-Step Tasks

### Task 1.1: Add shared editor-route chrome idioms — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-1-1/` (branch: `feat/profiles-launch-rework-1-1`)
- **ACTION**: Create `src/crosshook-native/src/styles/editor-routes.css` and register it in `src/crosshook-native/src/main.tsx`.
- **IMPLEMENT**: Define two net-new CSS idioms used by Tasks 2.1 and 2.2: `.crosshook-editor-field-readonly` (used where the dashboard routes would use `kv-row`, but the value sits inline next to an editable field) and `.crosshook-editor-mono-panel` (monospace panel background for command previews and resolved-launch debug output). Both rules MUST reference existing `--crosshook-*` tokens — no literal `#` colors. Keep selectors route-agnostic. Register the stylesheet in `main.tsx` immediately after the `dashboard-routes.css` import so cascade ordering matches Phase 9.
- **MIRROR**: `DASHBOARD_PANEL_SECTION_PATTERN`, `DASHBOARD_PILL_ROW_PATTERN`, `DASHBOARD_KV_ROW_PATTERN` — the new classes must compose with these, not replace them.
- **IMPORTS**: CSS registered via `main.tsx:5-17` import block.
- **GOTCHA**: `scripts/check-legacy-palette.sh` scans the whole `src/crosshook-native/src/` tree on every run; any literal `#0078d4`, `#2da3ff`, `#1a1a2e`, `#20243d`, `#12172a`, `rgba(0,120,212,...)`, or `rgba(45,163,255,...)` fails CI. Use only tokens.
- **VALIDATE**: `./scripts/lint.sh` passes (Biome + legacy-palette + shellcheck + host-gateway). `npm run typecheck` passes. Dev server boots and a smoke check shows the new classes load (visually verify in DevTools once a call site lands in B2).

### Task 1.2: Split `ProfileFormSections.tsx` into `profile-form/` submodules — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-1-2/` (branch: `feat/profiles-launch-rework-1-2`)
- **ACTION**: Create `src/crosshook-native/src/components/profile-form/` with `FormFieldRow.tsx`, `OptionalSection.tsx`, `ProtonPathField.tsx`, `LauncherMetadataFields.tsx`, `ProfileSelectorField.tsx`, `TrainerVersionSetField.tsx`, `helpers.ts`. Update `src/crosshook-native/src/components/ProfileFormSections.tsx` to import from these, keep its external re-exports intact, and consume the new `useProtonDbApply` hook (landed in Task 1.3's tree but merged in the same batch — this task creates its own hook under `hooks/profile/useProtonDbApply.ts` concurrently; both tasks MUST agree on the hook signature in their child worktrees before the B2 fan-in so the merge is clean).
- **IMPLEMENT**: Move `FieldRow` (lines 107-152 of today's file), `ProtonPathField` (154-218), `LauncherMetadataFields` (220-243), `OptionalSection` (57-72, 245-256 — replace inline `optionalSectionStyle`/`optionalSectionSummaryStyle` with class names defined in `editor-routes.css` from Task 1.1, adding a fallback using token-based rgba), `ProfileSelectorField` (258-310), and `TrainerVersionSetField` (312-368) to their own files. Move `parentDirectory`, `updateGameExecutablePath`, and any other pure helpers to `profile-form/helpers.ts`. The parent file keeps `ProfileFormSectionsProfileSelector`/`ProfileFormSectionsProps` type unions and the final compose render (lines 520-578). Preserve the `export type { PendingProtonDbOverwrite }` and `export type { ProtonInstallOption }` re-exports so downstream imports in `UpdateGamePanel.tsx`, `RunExecutablePanel.tsx`, `ui/ProtonPathField.tsx`, `profile-sections/RuntimeSection.tsx`, and `profile-sections/GameSection.tsx` remain green. Extract the ~110-line ProtonDB orchestration block (388-518) into `src/crosshook-native/src/hooks/profile/useProtonDbApply.ts` — the hook returns `{ pendingOverwrite, applyingGroupId, statusMessage, applyGroup, applyEnvVars, acceptSuggestion, clearOverwrite }`. Shape the hook API so both `ProfileFormSections` and `LaunchPage` can swap their local orchestration for a single hook call.
- **MIRROR**: `CALL_COMMAND_TRY_CATCH_LOG_PATTERN` inside `useProtonDbApply`, `THIN_PAGE_WRAPPER_PATTERN` for the parent file's final shape.
- **IMPORTS**: `applyProtonDbGroupToProfile`, `mergeProtonDbEnvVarGroup`, `PendingProtonDbOverwrite` from `utils/protondb`; existing `AcceptSuggestionRequest`/`ProtonDbRecommendationGroup` from `types/protondb`.
- **GOTCHA**: `ProfileFormSections.tsx` has named-export _and_ a default-export. Some downstream imports use `import ProfileFormSections from '../ProfileFormSections'` (InstallPage.tsx:13); keep the default export AND all named re-exports. Any downstream that imports `FieldRow`, `ProtonPathField`, `TrainerVersionSetField`, `LauncherMetadataFields`, or `parentDirectory` directly MUST continue to work via a barrel re-export in `ProfileFormSections.tsx` — grep before merging: `rg "from '[^']*ProfileFormSections'" src/crosshook-native/src`.
- **VALIDATE**: `wc -l src/crosshook-native/src/components/ProfileFormSections.tsx` returns <500. `npm run typecheck` passes. `npm test` passes (existing `OnboardingWizard.test.tsx` exercises this path). Downstream callers in `InstallPage`, `ProfileReviewModal`, `UpdateGamePanel`, `RunExecutablePanel`, `ui/ProtonPathField`, `RuntimeSection`, `GameSection` all resolve their imports without editing.

### Task 1.3: Split `LaunchPage.tsx` into `pages/launch/` submodules — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-1-3/` (branch: `feat/profiles-launch-rework-1-3`)
- **ACTION**: Create `src/crosshook-native/src/components/pages/launch/` with `useLaunchPageState.ts`, `useLaunchDepGate.ts`, `LaunchDepGateModal.tsx`, `LaunchProfileSelector.tsx`, plus `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`. Update `src/crosshook-native/src/components/pages/LaunchPage.tsx` to consume them. Coordinate with Task 1.2 on the `useProtonDbApply` hook shape — the two tasks agree on the signature and one of them lands the file (per-batch merge: if both add the file, `merge-children.sh` will fail; pre-agree that Task 1.2 creates the hook and Task 1.3 consumes it after fan-in).
- **IMPLEMENT**:
  - `useLaunchPageState.ts`: pull context aggregation (lines 33-108 of today's file), collection filter memoization (44-56), profile-list-summaries effect (86-108), launch-request builder (120-136), `hasSavedSelectedProfile` derivation (151-160), `effectiveSteamClientInstallPath` resolution (137-150). Return a `{...}` object consumed by the thin page wrapper.
  - `useLaunchDepGate.ts`: extract the dep-gate state (`depGatePackages`, `depGateInstalling`, `depGateError`, lines 188-212) + the `prefix-dep-complete` subscription effect (227-253) + `handleBeforeLaunch` (255-300). Return `{ depGatePackages, depGateInstalling, depGateError, handleBeforeLaunch, closeDepGate }`.
  - `LaunchDepGateModal.tsx`: render the modal subtree (lines 519-586). Accept the dep-gate hook's return values as props.
  - `LaunchProfileSelector.tsx`: render the `profileSelectSlot` JSX (lines 410-448). Use `crosshook-dashboard-pill-row` for the collection chip (visible net result matches Task 2.2's redesign). Props: `activeCollection`, `filteredProfiles`, `favoriteProfiles`, `profileNetworkIsolation`, `onSelectProfile`.
  - `useLaunchEnvironmentAutosave.ts`: extract the 400ms debounce timer, latest-value refs, and `handleEnvironmentBlurAutoSave` callback (lines 193-212, 370-399). Preserve `LAUNCH_AUTOSAVE_DEBOUNCE_PATTERN` exactly — same 400ms window, same `hasSavedSelectedProfile` gate, same `trigger === 'value' && row.key.trim().length === 0` skip, same `persistProfileDraftRef` pattern.
  - `LaunchPage.tsx`: shrink to <500 lines by composing these hooks + the extracted components. JSX body keeps the outer `crosshook-page-scroll-shell--launch` wrapper and threads the extracted `profileSelectSlot` and `tabsSlot` into `<LaunchPanel>`.
- **MIRROR**: `PROFILES_PAGE_STATE_HOOK_PATTERN`, `THIN_PAGE_WRAPPER_PATTERN`, `LAUNCH_AUTOSAVE_DEBOUNCE_PATTERN`, `CALL_COMMAND_TRY_CATCH_LOG_PATTERN`.
- **IMPORTS**: `useProfileContext`, `useLaunchStateContext`, `usePreferencesContext`, `useProfileHealthContext`, `useLaunchPrefixDependencyGate`, `useCollections`, `useCollectionMembers`, `useProtonDbSuggestions`, `subscribeEvent`, `callCommand`, `buildProfileLaunchRequest`, `LaunchSubTabs` (consumed in page body).
- **GOTCHA**: (1) The `catch {}` silent-catch on `installPrefixDependency` / `getDependencyStatus` must stay silent — do not add any `console.error` or user surface. Decision #437 did not authorize changing dep-gate behavior. (2) `environmentAutosaveTimerRef.current` MUST be cleared on unmount (`useEffect` cleanup) — preserve the ref-capture pattern in `useLaunchEnvironmentAutosave`. (3) The effect that re-fetches `profile_list_summaries` on `activeCollectionId` change must keep its `active` capture flag to prevent late-resolution races. (4) The ProtonDB status message 4s auto-clear timer (`setTimeout` inside `handleApplyProtonDbEnvVars`) moves into `useProtonDbApply` — do not drop it.
- **VALIDATE**: `wc -l src/crosshook-native/src/components/pages/LaunchPage.tsx` returns <500. `npm run typecheck` passes. `npm test` passes. Smoke `pipeline renders on launch page` still passes (6 `.crosshook-launch-pipeline__node` elements on the Launch tab). The launch button still resolves `launchRequest` correctly across a `method` change (exercise via dev-native browser mode).

### Task 1.4: Split `LaunchSubTabs.tsx` into `launch-subtabs/` submodules — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-1-4/` (branch: `feat/profiles-launch-rework-1-4`)
- **ACTION**: Create `src/crosshook-native/src/components/launch-subtabs/` with `types.ts`, `useAutoSaveChip.ts`, `useTabVisibility.ts`, `OfflineTabContent.tsx`, `EnvironmentTabContent.tsx`, `GamescopeTabContent.tsx`, `MangoHudTabContent.tsx`, `OptimizationsTabContent.tsx`, `SteamOptionsTabContent.tsx`. Update `src/crosshook-native/src/components/LaunchSubTabs.tsx` to consume them.
- **IMPLEMENT**:
  - `types.ts`: move `LaunchSubTabId`, `TAB_LABELS`, and `LaunchSubTabsProps` from lines 25-94. Parallel to `launch-panel/types.ts`.
  - `useAutoSaveChip.ts`: the `TONE_PRIORITY` table + combined-status reducer + 3s fade timer (lines 163-208). Return `{ combinedStatus, chipVisible }`.
  - `useTabVisibility.ts`: the `showsGamescopeTab` / `showsMangoHudTab` / `showsOptimizationsTab` / `showsSteamOptionsTab` predicates + the `tabs[]` builder (lines 136-162). Return an ordered `LaunchSubTabId[]`.
  - `OfflineTabContent.tsx`: the 70-line offline tab body (lines 299-372) wrapped in `<Tabs.Content value="offline" forceMount style={...}>`. Props: `offlineReadiness`, `offlineReadinessError`, `offlineWarning`, `launchPathWarnings`, `profileName`, `onUpdateProfile`.
  - `EnvironmentTabContent.tsx`: the env var + ProtonDB panel body (lines 455-501). Props include the `useProtonDbApply` hook's return values (shared with LaunchPage's ProtonDB orchestration).
  - `GamescopeTabContent.tsx` / `MangoHudTabContent.tsx` / `OptimizationsTabContent.tsx` / `SteamOptionsTabContent.tsx`: thin wrappers (~20 lines each) that render `<Tabs.Content value="..." forceMount style={...}>` + the respective existing panel component.
  - `LaunchSubTabs.tsx`: shrink to <500 lines by composing these. KEEP the `OFFLINE_AUTO_SWITCH_PATTERN` effect (lines 213-227), the cover-art backdrop (229-268), and the outer Tabs.Root + Tabs.List in this file. Do NOT move them into a child component — the auto-switch effect needs the parent's `setActiveTab`.
- **MIRROR**: `RADIX_TABS_FORCEMOUNT_PATTERN`, `AUTOSAVE_CHIP_MERGE_PATTERN`, `OFFLINE_AUTO_SWITCH_PATTERN`.
- **IMPORTS**: existing `GamescopeConfigPanel`, `MangoHudConfigPanel`, `LaunchOptimizationsPanel`, `SteamLaunchOptionsPanel`, `CustomEnvironmentVariablesSection`, `OfflineStatusBadge`, `OfflineReadinessPanel`, `ProtonDbLookupCard`, `ProtonDbOverwriteConfirmation`, `GameMetadataBar`; types from the new `launch-subtabs/types.ts`.
- **GOTCHA**: (1) Every `<Tabs.Content>` in the extracted children MUST keep `forceMount` + `style={{ display: activeTab === '...' ? undefined : 'none' }}`. Dropping either silently regresses autosave continuity (panels unmount, autosave state resets). (2) The `autoSwitchedRef` stays in the parent — do NOT lift it into `useAutoSaveChip` or duplicate it per tab. (3) `combinedAutoSaveStatus` is used by the existing autosave chip JSX; its shape cannot change even though its location in the DOM does (Task 2.2 moves it into the panel header). (4) `useGameCoverArt` + `useImageDominantColor` fire on every render; keep them in the parent `LaunchSubTabs` so cover art reloads don't cascade into children.
- **VALIDATE**: `wc -l src/crosshook-native/src/components/LaunchSubTabs.tsx` returns <500. All 6 `crosshook-subtab-content` elements still render (`forceMount` intact). `npm run typecheck` + `npm test` pass. Smoke `pipeline renders on launch page` still finds the launch pipeline nodes.

### Task 2.1: Redesign the Profiles route in the unified visual language — Depends on [1.1, 1.2]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-2-1/` (branch: `feat/profiles-launch-rework-2-1`)
- **ACTION**: Update `ProfilesPage.tsx`, `ProfileSubTabs.tsx`, and the five `profile-sections/*Section.tsx` components + the new `profile-form/*` extracts where needed. Append any new scroll selectors to `useScrollEnhance.ts` and extend `useScrollEnhance.test.ts` with registration-count assertions.
- **IMPLEMENT**: Wrap each logical profile group in `DashboardPanelSection`: Identity, Game, Runner, Runtime (with env-var + ProtonDB sub-sections each in their own panel), Trainer. Convert the Game panel's read-only metadata rows (resolved path, Steam AppID, cover art source) to a `<dl>` of `crosshook-dashboard-kv-row`. Convert the Runner panel's method indicators to `crosshook-dashboard-pill-row`. Render command preview inside `crosshook-editor-mono-panel` (new idiom from Task 1.1). Replace `ProfileFormSections`' `OptionalSection` inline styling with its new CSS class. Use `crosshook-editor-field-readonly` wherever a readonly value sits adjacent to an editable field (e.g. "Launcher name" computed preview, ProtonDB recommendation pending label). Do not change section order or prop surfaces. Do not alter autosave/save semantics. If adding a new overflow-y scroll container, append the selector in `SCROLL_ENHANCE_SELECTORS` (line 9) and add a matching `.match(/\.selector\b/g).length === 1` test.
- **MIRROR**: `DASHBOARD_PANEL_SECTION_PATTERN`, `DASHBOARD_PILL_ROW_PATTERN`, `DASHBOARD_KV_ROW_PATTERN`, `ROUTE_FILL_CARD_PATTERN`, `INLINE_ALERT_PATTERN`, `SELECTOR_REGISTRATION_TEST_PATTERN`.
- **IMPORTS**: `DashboardPanelSection` from `components/layout/DashboardPanelSection`; new classes are CSS-only so no TS import for them.
- **GOTCHA**: (1) `ProfileFormSections`' default export AND its named re-exports (`PendingProtonDbOverwrite`, `ProtonInstallOption`, `FieldRow`, `ProtonPathField`, `TrainerVersionSetField`, `LauncherMetadataFields`) must remain green for `InstallPage`, `ProfileReviewModal`, `UpdateGamePanel`, `RunExecutablePanel`, `ui/ProtonPathField`. (2) `ProfileSubTabs`' `forceMount` per tab content stays — dropping it breaks cross-tab state continuity. (3) `ProfilesHero` (thumbnail + pinned profiles + selector) keeps its existing behavior; only chrome changes. (4) Token drift is a CI failure — do not paste literal hex colors; use the tokens already wired up in `dashboard-routes.css`. (5) `useScrollEnhance` already covers `.crosshook-subtab-content__inner--scroll`; wherever possible reuse that selector class rather than adding a new one.
- **VALIDATE**: `npm run typecheck`, `npm test`, `./scripts/lint.sh`, `npm run test:smoke`. Open the app in `./scripts/dev-native.sh --browser` and verify: (a) identity/game/runner/runtime/trainer render as panels; (b) save flow works end-to-end; (c) ProtonDB apply still pops the overwrite confirmation; (d) health-issues banner parity with pre-phase layout.

### Task 2.2: Redesign the Launch route in the unified visual language — Depends on [1.1, 1.3, 1.4]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-2-2/` (branch: `feat/profiles-launch-rework-2-2`)
- **ACTION**: Update `LaunchPage.tsx`, `LaunchPanel.tsx`, `LaunchPipeline.tsx`, `launch-pipeline.css`, `LaunchSubTabs.tsx`, and the 6 extracted `launch-subtabs/*TabContent.tsx` files. Also update the extracted `pages/launch/LaunchProfileSelector.tsx` (from Task 1.3) to use the pill-row idiom. Append any new scroll selectors to `useScrollEnhance.ts` with matching registration-count tests.
- **IMPLEMENT**: Wrap each launch tab body in `DashboardPanelSection` (eyebrow="Environment" / "Gamescope" / etc., with the unified autosave chip in the panel header `actions` slot for the active tab). Convert the `profileSelectSlot` (in `LaunchProfileSelector`) to a `crosshook-dashboard-pill-row` with collection chip + select. Render the launch command preview inside `crosshook-editor-mono-panel`. Move the floating autosave chip from its absolute-positioned slot into the active panel's header actions. Restyle `LaunchPipeline` step nodes to use `crosshook-dashboard-pill` tokens (behavior + count unchanged). Apply `crosshook-editor-field-readonly` for readonly chips in the Launch hero (effective Steam path, umu preference, selected profile) rendered as a `<dl>` of kv-rows. Keep `LaunchDepGateModal` rendering intact; apply `crosshook-panel` to the inner content for consistency but DO NOT change the modal overlay, focus trap, or prefix-dep-complete subscription behavior.
- **MIRROR**: `DASHBOARD_PANEL_SECTION_PATTERN`, `DASHBOARD_PILL_ROW_PATTERN`, `DASHBOARD_KV_ROW_PATTERN`, `AUTOSAVE_CHIP_MERGE_PATTERN`, `RADIX_TABS_FORCEMOUNT_PATTERN`, `OFFLINE_AUTO_SWITCH_PATTERN`, `SELECTOR_REGISTRATION_TEST_PATTERN`.
- **IMPORTS**: `DashboardPanelSection`; existing `LaunchPanel`/`LaunchPipeline` types (unchanged); `useProtonDbApply` (consumed by `EnvironmentTabContent`).
- **GOTCHA**: (1) `LaunchPanelProps` is frozen for this phase — do NOT add or remove fields; only change what's rendered inside the slots. (2) Smoke asserts exactly 6 `.crosshook-launch-pipeline__node` elements — don't break this count. (3) The unified autosave chip lives per-tab in the redesigned layout, but the combined-status reducer in `useAutoSaveChip` (from Task 1.4) remains a single instance in the `LaunchSubTabs` parent — the chip is positioned into the active panel's header via a stable DOM slot, not cloned per tab. (4) Moving the autosave chip MUST NOT change the 3s fade timer semantics. (5) Env-var autosave 400ms debounce is parity-critical; its hook (from Task 1.3) is consumed unchanged by `EnvironmentTabContent`. (6) Token drift = CI failure; use tokens only. (7) Keep `crosshook-launch-subtabs .crosshook-subtabs-foreground > .crosshook-subtab-content` scroll contract — if the new panel adds a nested scroll owner, register it in `SCROLL_ENHANCE_SELECTORS` and add the registration-count test.
- **VALIDATE**: `npm run typecheck`, `npm test`, `./scripts/lint.sh`, `npm run test:smoke` all pass. In `./scripts/dev-native.sh --browser`: (a) launch game + trainer still work, (b) dep-gate still triggers and silently catches, (c) offline auto-switch still fires when `launchPathWarnings` is non-empty, (d) combined autosave chip still merges statuses with the same priority, (e) pipeline renders 6 nodes at 1920×1080 and 1280×800.

### Task 3.1: Add focused Profiles + Launch RTL coverage — Depends on [2.1, 2.2]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-3-1/` (branch: `feat/profiles-launch-rework-3-1`)
- **ACTION**: Create `src/crosshook-native/src/components/pages/__tests__/ProfilesRoute.test.tsx`, `src/crosshook-native/src/components/pages/__tests__/LaunchRoute.test.tsx`, and `src/crosshook-native/src/components/__tests__/LaunchSubTabs.test.tsx`.
- **IMPLEMENT**:
  - `ProfilesRoute.test.tsx`: mirror `DashboardRoutes.test.tsx` harness. Cover: (a) `DashboardPanelSection`-wrapped groups render with their eyebrow/title, (b) `handleSave` calls `callCommand('profile_save', ...)` exactly once, (c) inline `crosshook-danger` banner shows on `profile_save` error handler rejection, (d) ProtonDB apply surfaces the overwrite confirmation and then applies on confirm.
  - `LaunchRoute.test.tsx`: cover (a) collection chip reflects `useCollectionMembers` result, (b) env-var autosave fires `profile_save` after 400ms for `value` triggers when the key is non-empty, but NOT for `key` triggers alone and NOT for empty-key `value` triggers; use `vi.useFakeTimers` to advance 399ms → no call, 400ms → one call, (c) `hasSavedSelectedProfile` gate blocks autosave when `profileName !== selectedProfile`, (d) `handleBeforeLaunch` silent-catches `getDependencyStatus` rejection and still allows launch, (e) `prefix-dep-complete` event handler closes the dep-gate modal.
  - `LaunchSubTabs.test.tsx`: cover (a) combined autosave chip merges three statuses by `TONE_PRIORITY`, (b) tab visibility matrix for `native` / `proton_run` / `steam_applaunch` launch methods, (c) switching tabs preserves state across all 6 `forceMount` panels (DOM nodes persist; e.g. a mangohud config input value stays after switching to environment and back), (d) offline auto-switch fires when `launchPathWarnings.length > 0` transitions from 0, and NOT again on subsequent changes (`autoSwitchedRef` guard).
  - Wire each test with `vi.mock('@/lib/ipc', ...)` + `renderWithMocks({ handlerOverrides: { profile_load: ..., collection_list_profiles: ..., profile_list_summaries: ..., profile_save: ..., profile_save_launch_optimizations: ..., profile_save_gamescope_config: ..., profile_save_mangohud_config: ... } })`.
  - Assert `expect(consoleErrorSpy).not.toHaveBeenCalled()` on every test (mirroring `DashboardRoutes.test.tsx:81`).
- **MIRROR**: `IPC_MOCK_TEST_HARNESS_PATTERN`, test harness shape from `DashboardRoutes.test.tsx:29-80`, fixture patterns from `test/fixtures.ts`.
- **IMPORTS**: `renderWithMocks`, `mockCallCommand`, `makeProfileDraft`, `makeProfileHealthReport`; all context providers; the route components.
- **GOTCHA**: (1) `useFakeTimers` + React 19 concurrent rendering may require `vi.runAllTimersAsync()` inside `act()` — prefer `await vi.advanceTimersByTimeAsync(400)` over `advanceTimersByTime` to keep render + effect flushes deterministic. (2) The mock IPC layer throws on unhandled commands — seed every command the route touches or the tests throw `[test-mock] Unhandled command: ...`. (3) Do not assert on the specific DOM text of headings rendered by `DashboardPanelSection` — instead use role/testid queries, because copy is a design iteration surface. (4) When asserting `forceMount` persistence, use `getAllByRole('tabpanel')` with `hidden: true` to count mounted panels regardless of visibility.
- **VALIDATE**: `npm test` green; new suites add ≥6 tests with no added `console.error` noise.

### Task 3.2: Extend smoke route banner headings + DOM landing markers — Depends on [2.1, 2.2]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-profiles-launch-rework-3-2/` (branch: `feat/profiles-launch-rework-3-2`)
- **ACTION**: Update `src/crosshook-native/tests/smoke.spec.ts`.
- **IMPLEMENT**: Add `profiles: 'Profiles'` and `launch: 'Launch'` to `DASHBOARD_ROUTE_HEADINGS` so the route sweep asserts the `RouteBanner` H1 renders on Phase-11 routes too. If `DASHBOARD_ROUTE_HEADINGS` is iterated over by an existing test, keep the existing tests passing; otherwise add a simple `test.each` over the two new entries that navigates to the route and asserts `page.getByRole('heading', { level: 1, name: 'Profiles' })` / `{ name: 'Launch' }` is visible. Add one `panel-landing` smoke assertion per route: for Profiles, assert at least one `section.crosshook-dashboard-panel-section` is present after navigating to the Profiles tab; for Launch, assert the pipeline's 6 nodes still render AND that `section.crosshook-dashboard-panel-section` is present in the active Launch subtab. Keep `attachConsoleCapture` empty-error assertions intact. If Task 2.2 introduces a new scroll selector, also smoke-assert it exists and no dual-scroll indicator appears.
- **MIRROR**: existing route-loop structure and `attachConsoleCapture` contract in `smoke.spec.ts:7-85, 175-189`.
- **IMPORTS**: existing `ROUTE_NAV_LABEL`, smoke helpers only — no new dependencies.
- **GOTCHA**: (1) `smoke.spec.ts` iterates `ROUTE_ORDER` (line 33-45) and both `'profiles'` and `'launch'` are already present — don't add duplicates. (2) `DASHBOARD_ROUTE_HEADINGS` (line 47-52) is the gating record for banner-heading assertions. (3) Browser dev mode provides only the mocks in `src/crosshook-native/src/browser/`; if a test requires richer mock data to render the redesigned panels, fix the mock first — do not relax the no-error contract.
- **VALIDATE**: `npm run test:smoke` passes with Profiles + Launch banner assertions and panel-landing checks. `npm run test:smoke:update` refreshes snapshots only if the visual changes from Task 2.1/2.2 intentionally drift existing screenshots.

---

## Testing Strategy

### Unit / Integration Tests

| Test                                            | Input                                                         | Expected Output                                                                                           | Edge Case? |
| ----------------------------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ---------- |
| Profiles panel chrome                           | Render `ProfilesPage` with seeded profile                     | Identity / Game / Runner / Runtime / Trainer panels render with `crosshook-dashboard-panel-section` class | No         |
| Profile save flow                               | User clicks Save                                              | `profile_save` called once with normalized data; success cleared, dirty flag reset                        | No         |
| Profile save error                              | `profile_save` rejects                                        | Inline `crosshook-danger` alert renders with error message; no toast                                      | Yes        |
| ProtonDB apply overwrite                        | User clicks Apply on a ProtonDB recommendation                | `ProtonDbOverwriteConfirmation` shows; on confirm, profile state merges recommendation                    | Yes        |
| Launch collection chip                          | `useCollectionMembers` resolves with 3 members                | Collection pill renders with name; profile list filters to member set                                     | No         |
| Launch env-var autosave debounce (value, key!=) | Type value after key, advance 399ms                           | No `profile_save` call                                                                                    | Yes        |
| Launch env-var autosave debounce (value, key!=) | Same, advance 400ms                                           | Exactly one `profile_save` call with merged `custom_env_vars`                                             | Yes        |
| Launch env-var autosave skip (key empty)        | Blur value for a row whose key is empty                       | No `profile_save` call even after 400ms                                                                   | Yes        |
| Launch env-var autosave gate                    | `profileName !== selectedProfile`                             | No `profile_save` call even after 400ms                                                                   | Yes        |
| Dep-gate silent-catch                           | `getDependencyStatus` rejects                                 | Launch proceeds (returns `true`), no console error, no alert                                              | Yes        |
| Dep-gate modal close on event                   | `prefix-dep-complete` event fires with `succeeded: true`      | Modal closes; `depGatePackages` reset to `null`                                                           | Yes        |
| Autosave chip priority merge                    | gamescope `success`, mangohud `error`, optimizations `saving` | Combined chip tone = `error` (highest priority)                                                           | Yes        |
| Autosave chip 3s fade                           | Status transitions idle → success                             | Chip visible for 3s then fades (`chipVisible === false`)                                                  | Yes        |
| Tab visibility (native)                         | `launchMethod === 'native'`                                   | Only `['environment', 'offline']` tabs rendered                                                           | Yes        |
| Tab visibility (proton_run)                     | `launchMethod === 'proton_run'`                               | All 6 tabs rendered                                                                                       | No         |
| Tab visibility (steam_applaunch)                | `launchMethod === 'steam_applaunch'`                          | All 6 tabs except optimizations rule depends on applicable methods                                        | Yes        |
| forceMount persistence                          | Type into MangoHud input, switch to Environment, switch back  | Input value persists (DOM node was not unmounted)                                                         | Yes        |
| Offline auto-switch fires once                  | `launchPathWarnings` transitions [] → ['w1']                  | `activeTab === 'offline'`; on second transition, does NOT auto-switch again                               | Yes        |
| Scroll selector registration                    | New selectors appended to `SCROLL_ENHANCE_SELECTORS`          | Each new selector matches exactly once via regex                                                          | No         |
| Smoke — Profiles banner + panel landing         | Navigate to Profiles                                          | H1 "Profiles" visible; ≥1 `section.crosshook-dashboard-panel-section` present                             | No         |
| Smoke — Launch banner + pipeline + panel        | Navigate to Launch                                            | H1 "Launch" visible; 6 pipeline nodes present; ≥1 panel section in active subtab                          | No         |

### Edge Cases Checklist

- [ ] New profile (dirty, unsaved) → save works; `hasSavedSelectedProfile` blocks autosave until first save completes.
- [ ] Profile with empty `custom_env_vars` → Environment tab renders empty state, autosave not fired.
- [ ] Profile with collection_id=null → collection pill hidden, full profile list shown.
- [ ] Collection with zero members → Launch profile list renders empty; selected profile cleared.
- [ ] `profile_list_summaries` rejects → network-isolation badges absent, no error surface (silent by design).
- [ ] Radix Tabs switch while autosave in flight → status chip retains the saving tone through the switch.
- [ ] Panel-section DOM reflow at `1280×800` (Deck) does not introduce horizontal scrollbars.
- [ ] `scripts/check-legacy-palette.sh` finds zero matches in new CSS (Task 1.1 + Task 2.1/2.2 stylesheet edits).
- [ ] `SCROLL_ENHANCE_SELECTORS` registration count === 1 for every selector (no accidental duplicates).

---

## Validation Commands

### Static Analysis

```bash
npm --prefix src/crosshook-native run typecheck
```

EXPECT: Zero TypeScript errors across the split files, extracted hooks, redesigned components, and new test suites.

### Unit / Integration Tests

```bash
npm --prefix src/crosshook-native test
```

EXPECT: Vitest green including the new `ProfilesRoute.test.tsx`, `LaunchRoute.test.tsx`, and `LaunchSubTabs.test.tsx` suites; no added `console.error` noise.

### Full Lint Pass

```bash
./scripts/lint.sh
```

EXPECT: Biome + tsc + shellcheck + host-gateway + legacy-palette sentinel all pass. No legacy hex/rgba literals in the new CSS.

### Format (Pre-merge hygiene)

```bash
./scripts/format.sh
```

EXPECT: Zero drift; Biome + Prettier find nothing to change post-implementation.

### Browser Smoke

```bash
npm --prefix src/crosshook-native run test:smoke
```

EXPECT: Profiles + Launch route banner heading assertions and panel-landing assertions pass; pipeline still renders 6 nodes; zero `pageerror` / `console.error`.

### Soft-cap audit

```bash
wc -l \
  src/crosshook-native/src/components/ProfileFormSections.tsx \
  src/crosshook-native/src/components/pages/LaunchPage.tsx \
  src/crosshook-native/src/components/LaunchSubTabs.tsx
```

EXPECT: Each file under 500 lines.

### Manual Validation

- [ ] `./scripts/dev-native.sh --browser` at 1920×1080: Profiles + Launch render with panel sections; save works; env-var autosave fires after 400ms.
- [ ] Same at 1280×800 (Deck): layout does not clip; pipeline still shows 6 nodes; offline auto-switch fires when a launch-path warning surfaces.
- [ ] Same at 3440×1440: ultrawide shell (shell-level rework from earlier phases) does not letterbox Profiles/Launch; inspector + context rail coexist.
- [ ] Verify `LaunchDepGateModal` still opens when a profile with missing protontricks packages is selected and launched; silent-catch on failure still allows the launch.
- [ ] Verify ProtonDB apply still triggers the overwrite confirmation and merges on confirm (both from Profiles RuntimeSection and Launch Environment tab — one hook, two call sites).
- [ ] Verify `LaunchPanel` slot layout is unchanged: `profileSelectSlot` + action row + `tabsSlot` + optional `infoSlot` + `beforeActions`.

---

## Acceptance Criteria

- [ ] All 8 tasks complete; all 3 batches validated and fan-in merged without conflict.
- [ ] `ProfileFormSections.tsx`, `LaunchPage.tsx`, `LaunchSubTabs.tsx` each under the 500-line soft cap.
- [ ] Profiles + Launch routes share the unified panel / pill / kv-row / field-readonly / mono-panel visual language with the dashboard routes.
- [ ] Profile save, profile load, env-var autosave (400ms, gated), per-field autosave (gamescope/mangohud/optimizations), ProtonDB apply, dep-gate silent-catch, offline auto-switch, and `prefix-dep-complete` event handling all preserved verbatim.
- [ ] Public surfaces unchanged: `LaunchPanelProps`, `ProfileContextValue`, `LaunchStateContext`, `ROUTE_METADATA` entries for `profiles` / `launch`, `RouteBanner` contract.
- [ ] New `crosshook-editor-field-readonly` + `crosshook-editor-mono-panel` classes defined in `editor-routes.css` using `--crosshook-*` tokens only.
- [ ] New scroll containers (if any) registered in `SCROLL_ENHANCE_SELECTORS` with matching registration-count tests.
- [ ] `npm run typecheck`, `npm test`, `./scripts/lint.sh`, `npm run test:smoke` all pass.
- [ ] `scripts/check-legacy-palette.sh` finds zero matches in changed files.
- [ ] No new TOML settings, SQLite tables, or `#[tauri::command]` surfaces.

## Completion Checklist

- [ ] Shared panel primitive (`DashboardPanelSection`) reused — no parallel `EditorPanelSection` created.
- [ ] Error/empty/loading states still render as inline `crosshook-danger` banners — no toast introduction.
- [ ] `forceMount` retained on every `Tabs.Content` across `ProfileSubTabs` and `LaunchSubTabs`.
- [ ] Env-var autosave: 400ms window, `hasSavedSelectedProfile` gate, `trigger === 'value' && row.key.trim().length === 0` skip all intact.
- [ ] `LaunchPage`'s silent `catch {}` on dep-check handlers remains silent.
- [ ] `useProtonDbApply` hook consumed by both `ProfileFormSections` (RuntimeSection) and `LaunchSubTabs` (EnvironmentTabContent) — duplication removed.
- [ ] `ProfileFormSections` external re-exports (`PendingProtonDbOverwrite`, `ProtonInstallOption`, `FieldRow`, `ProtonPathField`, `TrainerVersionSetField`, `LauncherMetadataFields`) preserved; downstream callers (`InstallPage`, `ProfileReviewModal`, `UpdateGamePanel`, `RunExecutablePanel`, `ui/ProtonPathField`) compile without edits.
- [ ] Pipeline smoke still finds exactly 6 `.crosshook-launch-pipeline__node` elements on the Launch tab.
- [ ] Test harness mirrors `DashboardRoutes.test.tsx`: `vi.mock('@/lib/ipc')`, `renderWithMocks` with `handlerOverrides`, no added `console.error` noise.
- [ ] Self-contained — no open questions remain at implementation time.

## Risks

| Risk                                                                                 | Likelihood | Impact | Mitigation                                                                                                                                                                    |
| ------------------------------------------------------------------------------------ | ---------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Two child worktrees (1.2 and 1.3) both create `useProtonDbApply.ts` → merge conflict | High       | Medium | Pre-agree in each child's prompt: Task 1.2 owns the hook file creation; Task 1.3 consumes via import. Document the agreed signature in the plan's Task 1.2 IMPLEMENT section. |
| Env-var autosave debounce regression                                                 | Medium     | High   | Parity-critical test asserts exact 400ms window with fake timers; `LaunchRoute.test.tsx` covers the three skip conditions (key-empty, trigger=key, gate=false).               |
| Dropped `forceMount` in extracted tab content                                        | Medium     | High   | Explicit gotcha in Task 1.4; test suite asserts DOM persistence across tab switches; smoke still finds expected pipeline node count.                                          |
| Legacy palette literal sneaking into new CSS                                         | Low        | Medium | CI runs `scripts/check-legacy-palette.sh` full-tree on every run; use only tokens; suppression comment discouraged.                                                           |
| Scroll selector not registered → dual-scroll jank on WebKitGTK                       | Medium     | Medium | Every new `overflow-y:auto` selector adds to `SCROLL_ENHANCE_SELECTORS` + gets a registration-count test mirroring `useScrollEnhance.test.ts:4-8`.                            |
| `ProfileFormSections` downstream imports break after split                           | Medium     | High   | Preserve default + named re-exports; grep downstream imports before merge; CI typecheck is the gate.                                                                          |
| Offline auto-switch effect dropped when extracting subtab bodies                     | Medium     | High   | Explicit gotcha in Task 1.4; dedicated `LaunchSubTabs.test.tsx` case asserts switch fires once on transition and does not re-fire on subsequent transitions.                  |
| Design drift between Profiles and Launch redesigns (B2 parallel)                     | Medium     | Medium | Both tasks share `DashboardPanelSection` + `crosshook-editor-*` idioms from Task 1.1; share `useProtonDbApply`; use the same `dashboard-routes.css` pill / kv-row classes.    |
| Launch dep-gate silent-catch unintentionally rewritten                               | Low        | High   | NOT Building clause explicit; `LaunchRoute.test.tsx` asserts silent-catch parity; decision lock #437 scope noted in Task 1.3 GOTCHA.                                          |
| Hierarchical file path collisions inside `launch-subtabs/` naming                    | Low        | Low    | Files all end in `TabContent.tsx` suffix; per-file singular responsibility; follows `launch-panel/` precedent.                                                                |
| 500-line soft cap regression elsewhere (e.g. ProfilesPage grows post-redesign)       | Medium     | Medium | Task 2.1 VALIDATE runs `wc -l` post-change; if a page crosses the cap, extract a section into `pages/profiles/` following Phase 4's precedent.                                |
| Smoke fixtures lack data to render redesigned panels                                 | Medium     | Medium | Task 3.2 GOTCHA explicitly directs fixing mocks first; never weaken the no-error contract.                                                                                    |

## Notes

- Research dispatch used the requested `--parallel` mode (3 standalone `ycc:prp-researcher` sub-agents covering patterns / quality / infra). Findings were merged and cross-checked; no significant category went uncovered.
- Worktree annotations are included per the `--parallel` default. Each parallel task (1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 3.1, 3.2) maps to its own child worktree under `~/.claude-worktrees/crosshook-profiles-launch-rework-<task-id>/`. There are no sequential tasks in this plan; every task carries a `**Worktree**:` annotation.
- **Coordination hot spot**: `useProtonDbApply` is owned by Task 1.2 (it creates the file); Task 1.3 imports from it. Because the two tasks run in parallel child worktrees, the implementer must (a) land the hook file in 1.2's child, (b) stub the import in 1.3's child so its LaunchPage shape is complete, (c) resolve the merge in B2 once both children fan back into the parent branch. `merge-children.sh` handles the fan-in automatically; if it conflicts, the conflict will be on the hook file and must be resolved by taking the 1.2 version.
- **Scope discipline**: Phase 11 is the editor-route redesign. It is NOT the inspector-content phase for Profiles/Launch (those routes ship `inspectorComponent: undefined`); it is NOT the responsive-sweep phase (Phase 12); it is NOT the polish phase (Phase 13). Resist the urge to add an inspector panel or sweep viewport smoke here.
- **ProtonDB duplication elimination**: the pre-Phase-11 codebase carried ~110 lines of ProtonDB orchestration in `ProfileFormSections.tsx` AND a near-identical block in `LaunchPage.tsx`. Extracting `useProtonDbApply` resolves this duplication during the split, so the redesign can call one hook from both routes. The behavior parity test (Task 3.1) validates both call sites against the same hook.
- **No `EditorPanelSection` fork**: Phase 9 already proved `DashboardPanelSection` is route-agnostic — Phase 11 reuses it rather than forking, and validates the primitive's reusability. If a genuinely editor-specific need emerges during implementation (unlikely), the fallback is to introduce a thin wrapper that composes `DashboardPanelSection`, not duplicate it.
- **DASHBOARD_ROUTE_HEADINGS extension**: adding `profiles` and `launch` to `smoke.spec.ts:47-52` is the minimum banner-heading coverage expansion; Phase 12 will later add the full 1280/1920/2560/3440 viewport sweep across all routes.
- **File-count budget**: 13 CREATE + 11 UPDATE ≈ 24 files. This sits at "Large" complexity. Per CLAUDE.md, avoid accidental scope creep — any file that starts drifting past 500 lines during redesign must be split within the same task, not bundled into a follow-up.
