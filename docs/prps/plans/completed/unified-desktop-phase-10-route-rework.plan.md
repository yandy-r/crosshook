# Plan: Unified Desktop Phase 10 — Route Rework (Install · Settings · Community · Discover)

## Summary

Implement Phase 10 of the Unified Desktop Redesign (GitHub issues #422 deliverable / #449 tracker) by re-skinning the four non-editor routes — **Install**, **Settings**, **Community**, **Discover** — to match the steel-blue visual language Phase 9 established for the dashboard routes. The work reuses the existing `<DashboardPanelSection>` primitive + `dashboard-routes.css`, splits `OnboardingWizard.tsx` (606 lines) and `CommunityBrowser.tsx` (561 lines) under the 500-line soft cap _before_ restyling, harmonizes inline error banners on the Phase 9 canonical pattern, and extends Playwright smoke + adds focused RTL coverage so the four routes never regress silently again. No Tauri IPC, no TOML/SQLite persistence, no route-order changes.

## User Story

As a CrossHook user, I want Install, Settings, Community, and Discover to feel like they belong in the same app, so that every non-editor route reads as one coherent desktop tool — steel-blue palette, unified panel/section chrome, consistent inline alerts — instead of four bespoke older surfaces next to the redesigned dashboards.

## Problem → Solution

Today each of the four target routes has its own chrome contract: Install uses a two-level Radix Tabs wrapper with `crosshook-install-shell` / `crosshook-install-page-tabs` classes; Settings uses an 11-section `CollapsibleSection` stacked 2-column grid with per-section `crosshook-settings-*` idioms; Community wraps its browser in `CollapsibleSection` panels with bespoke `crosshook-community-tap` / `crosshook-community-browser__*` classes; Discover uses a `crosshook-card crosshook-discovery-panel` wrapper with `crosshook-discovery-card` result chrome. Inline error banners are inconsistent (`<p className="crosshook-danger">` vs `<div className="crosshook-error-banner">` vs banner-with/without `role="alert"`). Two target files already violate the 500-line soft cap (`OnboardingWizard.tsx` 606, `CommunityBrowser.tsx` 561). Phase 10 rewraps each route around Phase 9's `<DashboardPanelSection>` / `crosshook-dashboard-route-body` idioms, retargets the existing unused `crosshook-page-scroll-shell--install|--settings|--community|--discover` CSS hook on a per-route stylesheet, harmonizes error banners on `crosshook-error-banner--section` + `role="alert"`, and splits the two oversized files first so the rework is modular — all while preserving IPC boundaries, route state, hook contracts, and the wizard/portal focus-management model exactly.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md`
- **PRD Phase**: Phase 10 — Route rework — Install + Settings + Community + Discover
- **Estimated Files**: ~22 (5 new CSS files + 4 new component splits + 10 UPDATE + 3 test files)
- **GitHub Issues**: #449 tracking, #422 deliverable
- **Persistence Classification**: runtime-only UI composition; no new TOML settings; no new SQLite tables, migrations, or cache entries.

## Persistence and Usability

1. **Storage boundary**
   - **TOML settings**: No new user-editable preferences. Route appearance is code-driven only.
   - **SQLite metadata DB**: No new cache, catalog, or history tables. All existing Install / Community / Discover data continues to come from current hooks and IPC surfaces (`install_game`, `community_sync`, `discovery_search_trainers`, `settings_load`, `settings_save`).
   - **Runtime-only state**: Tab selection, filter query, rating chip filter, wizard step index, review-session dirty state, and consent-gate state remain in React memory.

2. **Migration / backward compatibility**: No settings or schema migration is required. Older builds keep the prior route chrome. The unused `crosshook-page-scroll-shell--install|--settings|--community|--discover` modifier classes already ship on the JSX today, so adding CSS for them is purely additive.

3. **Offline behavior**: No new online requirement introduced. Community cached-fallback banner, Discover consent-gate fallback, and Install prefix-path-resolver degraded states must all continue to surface inline within the redesigned shell.

4. **Failure fallback**: If a route data source fails (`community_list_profiles` error, `discovery_search_trainers` error, `install_default_prefix_path` timeout, `settings_load` error), the restyled route must continue to show the same inline error/empty/cached banners instead of collapsing the shell. Error-banner harmonization must not change which errors surface.

5. **User visibility / editability**: Users see the redesigned route chrome but cannot configure or persist route-specific layout preferences. Consent toggles for Discover (`discovery_enabled`) and all existing Settings TOML flags remain editable through the same handlers.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks                   | Depends On | Parallel Width |
| ----- | ----------------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3           | —          | 3              |
| B2    | 2.1, 2.2, 2.3, 2.4, 2.5 | B1         | 5              |
| B3    | 3.1, 3.2                | B2         | 2              |

- **Total tasks**: 10
- **Total batches**: 3
- **Max parallel width**: 5

---

## Worktree Setup

- **Parent**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework/ (branch: feat/unified-desktop-phase-10-route-rework)
- **Children** (per parallel task; merged back at end of each batch):
  - Task 1.1 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-1-1/ (branch: feat/unified-desktop-phase-10-route-rework-1-1)
  - Task 1.2 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-1-2/ (branch: feat/unified-desktop-phase-10-route-rework-1-2)
  - Task 1.3 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-1-3/ (branch: feat/unified-desktop-phase-10-route-rework-1-3)
  - Task 2.1 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-1/ (branch: feat/unified-desktop-phase-10-route-rework-2-1)
  - Task 2.2 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-2/ (branch: feat/unified-desktop-phase-10-route-rework-2-2)
  - Task 2.3 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-3/ (branch: feat/unified-desktop-phase-10-route-rework-2-3)
  - Task 2.4 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-4/ (branch: feat/unified-desktop-phase-10-route-rework-2-4)
  - Task 2.5 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-5/ (branch: feat/unified-desktop-phase-10-route-rework-2-5)
  - Task 3.1 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-3-1/ (branch: feat/unified-desktop-phase-10-route-rework-3-1)
  - Task 3.2 → ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-3-2/ (branch: feat/unified-desktop-phase-10-route-rework-3-2)

Per-worktree prerequisites (required for `./scripts/lint.sh` / `./scripts/format.sh`): `npm install -D --no-save typescript biome` at worktree root, then `cd src/crosshook-native && npm ci`.

---

## UX Design

### Before

```text
Install       -> outer Radix Tabs (install|update|run_executable) wrapping
                 inner InstallGamePanel with 5 flow sub-tabs; bespoke
                 crosshook-install-shell chrome; OnboardingWizard is a
                 606-line portal modal with its own crosshook-modal chrome.

Settings      -> 11-section stacked 2-column grid (CollapsibleSection
                 wrappers); per-section error spans without role="alert"
                 noise; RecentFilesColumn on the right.

Community     -> CollapsibleSection panels with crosshook-community-tap
                 cards; cached-fallback aria-live banner; diagnostics list;
                 eager useMemo filter; 561-line CommunityBrowser.

Discover      -> crosshook-card crosshook-discovery-panel wrapper with
                 consent gate, search bar, result grid using
                 crosshook-discovery-card (flat bg-elevated, no halo).
```

### After

```text
All four non-editor routes share the Phase 9 visual language:

RouteBanner (unchanged)
  -> <DashboardPanelSection> body sections with eyebrow/title/summary/actions
  -> crosshook-dashboard-pill / crosshook-status-chip pill rows
  -> crosshook-error-banner crosshook-error-banner--section role="alert"
  -> steel-blue halo on cards (matches route-banner gradient)
  -> per-route CSS hooks via existing crosshook-page-scroll-shell--<route>

Install       -> InstallPage banner + DashboardPanelSection wrappers around
                 the two Radix Tabs levels; InstallGamePanel flow tabs get
                 eyebrow/title chrome; OnboardingWizard keeps portal modal
                 but adopts Phase 9 eyebrow + card idioms.

Settings      -> SettingsPanel becomes a DashboardPanelSection-wrapped grid;
                 per-section CollapsibleSection shells inherit the panel
                 chrome; harmonized error banner on the page shell.

Community     -> CommunityBrowser panels wrapped in <DashboardPanelSection>;
                 TapChip gets steel-blue card treatment; cache banner reuses
                 the same chrome; diagnostics list in a dashboard section.

Discover      -> TrainerDiscoveryPanel uses <DashboardPanelSection> for
                 gate / search / results; discovery cards get steel-blue
                 halo + hover; consent modal reuses the inline-alert chrome.
```

### Interaction Changes

| Touchpoint            | Before                                                     | After                                                                | Notes                                                                          |
| --------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Install page          | Two-level Radix Tabs + `crosshook-install-shell` chrome    | Same tabs; outer frame wrapped in `<DashboardPanelSection>`          | Tab identity and `forceMount` behavior preserved exactly                       |
| Install review        | Inline `<p className="crosshook-danger">` errors           | `<div className="crosshook-error-banner ...--section" role="alert">` | Error copy unchanged; role/class only                                          |
| OnboardingWizard      | 606-line modal with inline step driver                     | <500-line modal with extracted stage components                      | Keeps portal + focus-trap + inert; stage machine unchanged                     |
| Settings page         | Page-shell only error banner + stacked sections            | Same sections re-wrapped in dashboard panel chrome                   | `onPersistSettings(patch)` contract preserved per section                      |
| Settings section save | Per-section `<p className="crosshook-danger">`             | Harmonized `crosshook-error-banner--section` + `role="alert"`        | Where the section currently shows a warning-style notice, keep `role="status"` |
| Community browser     | CollapsibleSection + bespoke card chrome                   | `<DashboardPanelSection>` + steel-blue cards                         | `useCommunityProfiles` hook contract unchanged                                 |
| Community import      | `CommunityImportWizardModal` chrome untouched              | Footer buttons align with Phase 9 button tokens                      | WizardStepper untouched functionally                                           |
| Discover panel        | Flat `crosshook-discovery-card` background                 | Halo + hover; `<DashboardPanelSection>` wrapper                      | Debounce + request-id race guard preserved                                     |
| Consent gate          | Inline `<div className="crosshook-discovery-panel__gate">` | Reuses inline-alert + button-token chrome                            | `discovery_enabled` persistence semantics unchanged                            |
| Smoke coverage        | 4 routes in `ROUTE_ORDER` but no structural assertion      | Adds heading + dashboard-body visibility assertions                  | `attachConsoleCapture` zero-error contract unchanged                           |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File                                                                              | Lines   | Why                                                                                           |
| -------- | --------------------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-redesign.prd.md`                                  | 277-282 | Phase 10 goal, scope, success signal                                                          |
| P0       | `docs/prps/plans/completed/github-issues-421-448-route-rework-dashboards.plan.md` | 1-473   | Sibling Phase 9 plan; mirror structure and the DashboardPanelSection/error-banner conventions |
| P0       | `src/crosshook-native/src/components/layout/DashboardPanelSection.tsx`            | 1-92    | Primitive contract that Phase 10 wraps around every target route's body                       |
| P0       | `src/crosshook-native/src/components/layout/routeMetadata.ts`                     | 1-123   | Route metadata entries for install/settings/community/discover                                |
| P0       | `src/crosshook-native/src/components/layout/RouteBanner.tsx`                      | 1-30    | Banner contract (read-only — not edited in Phase 10)                                          |
| P0       | `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`               | 90-220  | Phase 9 canonical route composition with `<DashboardPanelSection>` stack                      |
| P0       | `src/crosshook-native/src/components/pages/HostToolsPage.tsx`                     | 120-220 | Phase 9 hero/section pattern with pill rows                                                   |
| P0       | `src/crosshook-native/src/components/pages/InstallPage.tsx`                       | 1-463   | Outer Install route — two-level tab contract, review session, save path                       |
| P0       | `src/crosshook-native/src/components/InstallGamePanel.tsx`                        | 1-460   | Inner Install flow tabs; profile-section composition; force-rewrite to `proton_run`           |
| P0       | `src/crosshook-native/src/components/OnboardingWizard.tsx`                        | 1-606   | 606-line file to split; stage machine driven by `useOnboarding`                               |
| P0       | `src/crosshook-native/src/components/CommunityBrowser.tsx`                        | 1-561   | 561-line file to split; TapChip + CompatibilityBadge + filters + import                       |
| P0       | `src/crosshook-native/src/components/SettingsPanel.tsx`                           | 1-93    | Settings 2-column grid + CollapsibleSection composition                                       |
| P0       | `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`                   | 1-337   | Discover consent gate + debounced search + result cards                                       |
| P0       | `src/crosshook-native/src/components/pages/SettingsPage.tsx`                      | 1-81    | Canonical page-level error banner pattern already uses `crosshook-error-banner--section`      |
| P0       | `src/crosshook-native/src/styles/dashboard-routes.css`                            | 1-143   | Phase 9 reusable chrome classes (dashboard-route-body, panel-section, pill, kv-row)           |
| P0       | `src/crosshook-native/src/styles/layout.css`                                      | 140-219 | Shared route-shell contract (`--fill`, `route-card-host`, `route-card-scroll`)                |
| P0       | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                              | 1-40    | SCROLLABLE selector allowlist — WebKitGTK scroll contract                                     |
| P1       | `src/crosshook-native/src/main.tsx`                                               | 1-30    | Stylesheet registration site for per-route CSS                                                |
| P1       | `src/crosshook-native/src/styles/variables.css`                                   | 80-200  | Steel-blue tokens (surface-1/2/3, accent-strong, accent-glow, scrim)                          |
| P1       | `src/crosshook-native/src/components/install/InstallReviewSummary.tsx`            | 1-146   | Install review card to align with new card chrome                                             |
| P1       | `src/crosshook-native/src/components/settings/SteamGridDbSection.tsx`             | 1-150   | Representative settings section: input-row + error + status note                              |
| P1       | `src/crosshook-native/src/components/settings/DiagnosticExportSection.tsx`        | 1-70    | Try/catch/finally error-capture idiom used across Settings                                    |
| P1       | `src/crosshook-native/src/components/settings/ProfilesSection.tsx`                | 40-80   | Uncontrolled input + onBlur-persist + `role="status"` notice pattern                          |
| P1       | `src/crosshook-native/src/components/CommunityImportWizardModal.tsx`              | 1-366   | Import wizard footer + step renderer — reused from Community reskin                           |
| P1       | `src/crosshook-native/src/components/community-import/WizardStepper.tsx`          | 1-23    | Stepper primitive (namespaced to community-import)                                            |
| P1       | `src/crosshook-native/src/hooks/useInstallGame.ts`                                | 160-360 | Install state machine — do not change; only read to confirm preserved contract                |
| P1       | `src/crosshook-native/src/hooks/useOnboarding.ts`                                 | 1-220   | Wizard stage sequence and readiness-check flow                                                |
| P1       | `src/crosshook-native/src/hooks/useCommunityProfiles.ts`                          | 220-400 | Community sync/import pipeline                                                                |
| P1       | `src/crosshook-native/src/hooks/useTrainerDiscovery.ts`                           | 1-120   | 300 ms debounce + request-id race guard                                                       |
| P1       | `src/crosshook-native/src/context/PreferencesContext.tsx`                         | 40-120  | `settings_load` / `settings_save` IPC wiring; `discovery_enabled` persistence                 |
| P1       | `src/crosshook-native/src/components/layout/AppShell.tsx`                         | 80-215  | Wizard is rendered at AppShell root (not inside InstallPage); inspector gating logic          |
| P1       | `src/crosshook-native/src/components/__tests__/OnboardingWizard.test.tsx`         | 1-161   | Existing Vitest harness pattern for Phase 10 RTL tests                                        |
| P1       | `src/crosshook-native/src/components/pages/__tests__/DashboardRoutes.test.tsx`    | 1-200   | Phase 9 sibling dashboard-route test file — mirror structure for Phase 10                     |
| P1       | `src/crosshook-native/tests/smoke.spec.ts`                                        | 1-120   | Route sweep + console-error gate + heading assertions                                         |
| P2       | `src/crosshook-native/src/components/pages/CommunityPage.tsx`                     | 1-28    | Thin page wrapper pattern                                                                     |
| P2       | `src/crosshook-native/src/components/pages/DiscoverPage.tsx`                      | 1-21    | Thin page wrapper pattern                                                                     |
| P2       | `src/crosshook-native/src/components/ui/ThemedSelect.tsx`                         | all     | Radix select wrapper reused by filters                                                        |

## External Documentation

| Topic         | Source | Key Takeaway                                                                                                 |
| ------------- | ------ | ------------------------------------------------------------------------------------------------------------ |
| External docs | none   | No external API/library research needed. Phase 10 is fully constrained by existing Phase 9 idioms + the PRD. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### DASHBOARD_PANEL_SECTION_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/DashboardPanelSection.tsx:53-84
<section className={joinClasses('crosshook-panel', 'crosshook-dashboard-panel-section', className)}>
  <div className="crosshook-dashboard-panel-section__header">
    {eyebrow ? <p className="crosshook-dashboard-panel-section__eyebrow crosshook-heading-eyebrow">{eyebrow}</p> : null}
    <HeadingTag className="crosshook-heading-title crosshook-heading-title--card ...__title">{title}</HeadingTag>
```

Wrap every Phase 10 route body section in this primitive. Accepts `eyebrow`, `title`, `summary`, `description`, `actions`, `headingAfter`, plus `className` / `headerClassName` / `contentClassName` / `bodyClassName` pass-throughs.

### DASHBOARD_ROUTE_BODY_STACK_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/HealthDashboardPage.tsx:138-161
<div className="crosshook-route-stack">
  <div className="crosshook-dashboard-route-body crosshook-dashboard-route-section-stack">
    <DashboardPanelSection eyebrow="Health overview" title="…" description="…">
```

Use `crosshook-dashboard-route-body crosshook-dashboard-route-section-stack` as the vertical stack container for Install/Settings/Community/Discover bodies.

### CANONICAL_INLINE_ALERT_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/SettingsPage.tsx:41-45
{
  settingsError ? (
    <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
      {settingsError}
    </div>
  ) : null;
}
```

Harmonize every Phase 10 error surface on this shape: `<div>` (not `<p>`), `crosshook-error-banner crosshook-error-banner--section`, `role="alert"`. Non-blocking status notices keep `crosshook-warning-banner` + `role="status"`.

### ROUTE_BANNER_USAGE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/CommunityPage.tsx:13-22
<div className="crosshook-page-scroll-shell--community">
  <div className="crosshook-route-stack crosshook-community-page">
    <div className="crosshook-route-stack__body--fill crosshook-community-page__body">
      <RouteBanner route="community" />
```

Keep the existing per-route `crosshook-page-scroll-shell--<route>` hook class. Phase 10 **adds the CSS** that targets these hooks — it does not rename the classes.

### ROUTE_CARD_SHELL_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/CommunityPage.tsx:13-19 + layout.css:179-218
<div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill …">
  <div className="crosshook-route-stack">
    <div className="crosshook-route-card-host">
      <div className="crosshook-route-card-scroll">
```

Community/Discover retain this bounded card-scroll shell. Install/Settings continue to use `--fill` + inner scroll containers registered in `useScrollEnhance`.

### DASHBOARD_PILL_ROW_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/HealthDashboardPage.tsx:197-212
<div className="crosshook-dashboard-pill-row" aria-live="polite">
  {loading && <span className="crosshook-dashboard-pill">Checking profile health…</span>}
  <span className="crosshook-dashboard-pill">…</span>
</div>
```

Use for Install progress summaries, Settings status rows, Community cache-status chips, Discover search-state chips. `aria-live="polite"` only when content updates dynamically.

### RADIX_TABS_FORCEMOUNT_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/InstallGamePanel.tsx:56-64
<Tabs.Content
  value={value}
  forceMount
  className="crosshook-subtab-content"
  style={{ display: activeTab === value ? undefined : 'none' }}
  aria-label={tabLabel}
>
  {children}
</Tabs.Content>
```

Preserve both Install tab levels (page-level `install|update|run_executable` and inner flow `identity|runtime|trainer|media|installer_review`) with `forceMount` + display-none swap — no remounting between tabs.

### WIZARD_PORTAL_MODAL_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/OnboardingWizard.tsx:381-414
return createPortal(
  <div className="crosshook-modal" role="presentation">
    <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
    <div ref={surfaceRef} className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-onboarding-wizard"
      role="dialog" aria-modal="true" aria-labelledby={titleId} data-crosshook-focus-root="modal" onKeyDown={handleKeyDown}>
```

Keep the portal + focus-trap + inert-siblings contract exactly. The modal surface already reads `crosshook-panel`; Phase 10 adds/rebalances chrome via `crosshook-onboarding-wizard` + `.crosshook-modal__header`.

### WIZARD_STAGE_MACHINE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/hooks/useOnboarding.ts:12,142-151
const STAGE_SEQUENCE: OnboardingWizardStage[] = ['identity_game','runtime','trainer','media','review','completed'];
const advanceOrSkip = useCallback((launchMethod: string) => {
  setStage((current) => {
    const currentIndex = STAGE_SEQUENCE.indexOf(current); let nextIndex = currentIndex + 1;
    if (STAGE_SEQUENCE[nextIndex] === 'trainer' && launchMethod === 'native') nextIndex += 1; …
```

Do NOT move the stage machine into step components. `OnboardingWizard.tsx` stays as the driver; extracted step components receive `profile`, `updateProfile`, validation, readiness props — no local stage mutation.

### INSTALL_DRAFT_MERGE_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useInstallGame.ts:165-178
const setResult = useCallback((nextResult) => {
  setResultState(nextResult);
  if (nextResult === null) { setStageState('idle'); setErrorState(null); return; }
  setDraftProfileState((c) => mergeInstallGameResultIntoDraft(c, nextResult.profile));
  setStageState(deriveResultStage(nextResult));
```

Install draft merge preserves user-edited `launch`, `injection`, `local_override`, identity/art. Phase 10 re-skin must not replace this merge call with naive overwrite.

### TRY_FINALLY_ASYNC_ACTION_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx:224-241
setImportingId(result.id);
setImportError(null);
try { … } catch (err) { setImportError(err instanceof Error ? err.message : String(err)); }
finally { setImportingId((current) => (current === result.id ? null : current)); }
```

All async button handlers across Install/Settings/Community/Discover follow this shape: set-busy → clear-error → try → catch-setError → finally-clear-busy.

### SECTION_ONBLUR_PERSIST_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/settings/ProfilesSection.tsx:40-48
<input
  defaultValue={settings.profiles_directory}
  onBlur={(event) => {
    const v = event.target.value.trim();
    if (v !== settings.profiles_directory.trim()) void onPersistSettings({ profiles_directory: v });
  }}
/>
```

Settings sections use uncontrolled inputs + `onBlur`-persist with dirty-check. Phase 10 re-skin must NOT change the controlled/uncontrolled state of any input.

### WARNING_NOTICE_ROLE_STATUS_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/settings/ProfilesSection.tsx:76-80
{
  settings.profiles_directory_requires_restart ? (
    <p className="crosshook-warning-banner crosshook-settings-help" role="status">
      Restart CrossHook …
    </p>
  ) : null;
}
```

Non-blocking restart/info notices keep `role="status"` + `crosshook-warning-banner`; do NOT upgrade these to `role="alert"`.

### COMMUNITY_CACHE_BANNER_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/CommunityBrowser.tsx:263-282
<div className="crosshook-community-browser__cache-banner" role="status" aria-live="polite">
  <span className="crosshook-status-chip crosshook-community-browser__cache-chip">Cached data</span>
```

Cached-fallback banners stay in `role="status"` + `aria-live="polite"` — they're not errors.

### SCROLL_ENHANCE_REGISTRATION_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-11
export const SCROLL_ENHANCE_SELECTORS =
  '.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-palette__list, .crosshook-prefix-deps__log-output, .crosshook-discovery-results, …';
```

Any new `overflow-y: auto` container introduced by Phase 10 MUST be appended to this selector list in the same task. This is explicit CLAUDE.md policy — missing registrations cause WebKitGTK dual-scroll jank.

### VITEST_PORTAL_WIZARD_TEST_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/__tests__/OnboardingWizard.test.tsx:124-140
it('mounts in a portal and focuses the heading when opened', async () => {
  render(<OnboardingWizard open onComplete={vi.fn()} onDismiss={vi.fn()} />);
  const heading = await screen.findByRole('heading', { name: 'Identity & Game' });
  await waitFor(() => {
    expect(heading).toHaveFocus();
  });
});
```

Mirror this harness for new RTL tests: plain `render`, stub heavy child sections with `vi.mock(...)` placeholders, assert heading text + focus behavior + step indicator + Save-disabled.

### SMOKE_ROUTE_SWEEP_PATTERN

```ts
// SOURCE: src/crosshook-native/tests/smoke.spec.ts:47-52,86-92
const DASHBOARD_ROUTE_HEADINGS: Partial<Record<AppRoute, string>> = { health: 'Monitor…', 'host-tools': 'Check runtime…', … };
await expect(page.locator('.crosshook-dashboard-route-body, .crosshook-host-tool-dashboard').first()).toBeVisible();
```

Phase 10 adds analogous heading-text + `.crosshook-dashboard-route-body` assertions for `install` / `settings` / `community` / `discover` to lock in the re-skin.

### FILE_SPLIT_EXTRACTION_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/community-import/ProfileDetailsStep.tsx:12
export function ProfileDetailsStep({ draft, profile, profileName, launchMethod, onProfileNameChange }: ProfileDetailsStepProps) {
```

Wizard step extractions live under a sibling directory (`community-import/`); each step is a `PascalCaseStep.tsx` taking explicit props. Phase 10 OnboardingWizard split follows this convention under `src/crosshook-native/src/components/onboarding/`.

---

## Files to Change

| File                                                                              | Action | Justification                                                                                                                |
| --------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/onboarding/OnboardingIdentityStageBody.tsx`  | CREATE | Extract identity_game stage body from OnboardingWizard to drive it under the 500-line soft cap.                              |
| `src/crosshook-native/src/components/onboarding/OnboardingRuntimeStageBody.tsx`   | CREATE | Extract runtime stage body; receives profile + updateProfile + readiness props; no local stage mutation.                     |
| `src/crosshook-native/src/components/onboarding/OnboardingTrainerStageBody.tsx`   | CREATE | Extract trainer stage body; skipped on native launch method via parent driver.                                               |
| `src/crosshook-native/src/components/onboarding/OnboardingMediaStageBody.tsx`     | CREATE | Extract media stage body.                                                                                                    |
| `src/crosshook-native/src/components/onboarding/OnboardingReviewStageBody.tsx`    | CREATE | Extract review stage body; reuses `WizardReviewSummary`.                                                                     |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`                        | UPDATE | Keep stage machine + portal + focus-trap; compose extracted stage bodies; target <500 lines.                                 |
| `src/crosshook-native/src/components/community/TapChip.tsx`                       | CREATE | Extract the `TapChip` inline component (currently inside CommunityBrowser.tsx:90-127) to drop the file under 500 lines.      |
| `src/crosshook-native/src/components/community/CompatibilityBadge.tsx`            | CREATE | Extract the `CompatibilityBadge` inline component and `ratingLabel` table.                                                   |
| `src/crosshook-native/src/components/community/CommunityTapManagementSection.tsx` | CREATE | Extract the Tap Management panel block so the reskin fits inside the soft cap.                                               |
| `src/crosshook-native/src/components/community/CommunityProfilesSection.tsx`      | CREATE | Extract the Profiles list + filters block; the section becomes a `<DashboardPanelSection>` consumer.                         |
| `src/crosshook-native/src/components/CommunityBrowser.tsx`                        | UPDATE | Compose extracted sections around `<DashboardPanelSection>`; harmonize error/success banners; target <500 lines.             |
| `src/crosshook-native/src/styles/install-routes.css`                              | CREATE | Per-route chrome targeting `crosshook-page-scroll-shell--install` + install-specific card idioms — kept out of `theme.css`.  |
| `src/crosshook-native/src/styles/settings-routes.css`                             | CREATE | Per-route chrome targeting `crosshook-page-scroll-shell--settings` + per-section panel-inner idioms.                         |
| `src/crosshook-native/src/styles/community-routes.css`                            | CREATE | Per-route chrome for `crosshook-page-scroll-shell--community` + steel-blue card treatment for `crosshook-community-tap`.     |
| `src/crosshook-native/src/styles/discover-routes.css`                             | CREATE | Per-route chrome for `crosshook-page-scroll-shell--discover` + halo/hover for `crosshook-discovery-card`.                    |
| `src/crosshook-native/src/styles/onboarding-wizard.css`                           | CREATE | Portal-modal chrome extracted from `theme.css` scope so Phase 10 can tune it without touching `theme.css`.                   |
| `src/crosshook-native/src/main.tsx`                                               | UPDATE | Register the five new per-route stylesheets at the app entrypoint.                                                           |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                              | UPDATE | Append any new scroll-container selectors introduced by the reskin (e.g. `.crosshook-install-page-tabs__panel-inner`).       |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`                       | UPDATE | Wrap Install outer tabs in `<DashboardPanelSection>`; harmonize error banners (`--section` + `role="alert"`).                |
| `src/crosshook-native/src/components/InstallGamePanel.tsx`                        | UPDATE | Wrap install flow sections in `<DashboardPanelSection>`; preserve Radix Tabs + `forceMount` + native/proton rewrite effect.  |
| `src/crosshook-native/src/components/install/InstallReviewSummary.tsx`            | UPDATE | Harmonize error surface to `crosshook-error-banner--section` + `role="alert"`; align card chrome with dashboard idioms.      |
| `src/crosshook-native/src/components/pages/SettingsPage.tsx`                      | UPDATE | Rewrap Settings body in `<DashboardPanelSection>`-style header; keep existing error-banner shape (already canonical).        |
| `src/crosshook-native/src/components/SettingsPanel.tsx`                           | UPDATE | Wrap the 2-column grid header in the dashboard panel chrome; keep CollapsibleSection sub-sections.                           |
| `src/crosshook-native/src/components/pages/CommunityPage.tsx`                     | UPDATE | Keep thin wrapper; apply new `crosshook-community-page__body` restyle hook as needed.                                        |
| `src/crosshook-native/src/components/pages/DiscoverPage.tsx`                      | UPDATE | Keep thin wrapper; apply new `crosshook-discover-page__body` restyle hook as needed.                                         |
| `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`                   | UPDATE | Wrap gate / search / results in `<DashboardPanelSection>`; harmonize error + notice surfaces.                                |
| `src/crosshook-native/src/components/__tests__/InstallGamePanel.test.tsx`         | CREATE | RTL coverage for Install flow shell (chrome + error-banner regression); stubs heavy child sections like the Onboarding test. |
| `src/crosshook-native/src/components/__tests__/SettingsPanel.test.tsx`            | CREATE | RTL coverage for Settings shell render + `role="alert"` banner + 11-section stacked render.                                  |
| `src/crosshook-native/src/components/__tests__/CommunityBrowser.test.tsx`         | CREATE | RTL coverage for Community shell + cached-fallback banner + import button disabled state.                                    |
| `src/crosshook-native/src/components/__tests__/TrainerDiscoveryPanel.test.tsx`    | CREATE | RTL coverage for Discover consent gate + search debounce + result card chrome.                                               |
| `src/crosshook-native/tests/smoke.spec.ts`                                        | UPDATE | Add heading-text + `.crosshook-dashboard-route-body` visibility assertions for install/settings/community/discover.          |

## NOT Building

- **No new Tauri `#[tauri::command]` handlers** — `install_game`, `install_default_prefix_path`, `validate_install_request`, `check_generalized_readiness`, `dismiss_onboarding`, `settings_load`, `settings_save`, `community_list_profiles`, `community_sync`, `community_add_tap`, `community_prepare_import`, `community_import_profile`, `discovery_search_trainers`, `discovery_search_external`, `profile_save` all remain unchanged.
- **No Settings sub-tab introduction.** The PRD wording "Settings keeps its sub-tabs" is interpreted here as "preserve Settings' existing 11 stacked `CollapsibleSection` sub-sections exactly" — there is no Radix Tabs layer in Settings today and Phase 10 does not add one. Re-skin only.
- **No changes to the Install two-level Radix Tabs structure** (`install|update|run_executable` outer, `identity|runtime|trainer|media|installer_review` inner). Both levels keep `forceMount` + `display:none` semantics.
- **No changes to the OnboardingWizard portal / focus-trap / inert-siblings model.** The split extracts stage bodies only; the portal mount, focus-scope, `aria-modal`, and `data-crosshook-focus-root` contracts are untouched.
- **No new npm dependencies.** All idioms reuse Radix Tabs/Select/Tooltip + `react-resizable-panels` + `@tauri-apps/api`. No `cmdk`, no Radix Dialog, no `@tanstack/react-query`.
- **No inspector-component introduction for these routes.** Install/Settings/Community/Discover continue to return `undefined` from `ROUTE_METADATA[route].inspectorComponent`; the Inspector panel stays collapsed. Inspector wiring for non-Library routes is explicitly deferred.
- **No persistence / TOML schema / SQLite schema changes.** No migration, no cache eviction, no history tables.
- **No LibraryCard / GameInspector quick-action to Community or Discover.** Not in scope.
- **No refactor of `theme.css`.** New CSS goes into the five new per-route stylesheets; `theme.css` edits are limited to targeted literal/class touch-ups if any remain after Phase 2's sweep.
- **No `useBreakpoint` / responsive restructure** beyond what Phase 1 already ships. The reskin preserves the current breakpoint behavior.
- **No `CommunityImportWizardModal` internal rewrite.** Only footer-button token alignment; stepper + steps + data flow untouched.
- **No scope creep into Phase 11** (Profiles + Launch editor rework, which owns `ProfileFormSections`, `LaunchPage`, `LaunchSubTabs` splits).

---

## Step-by-Step Tasks

### Task 1.1: Split `OnboardingWizard` under the 500-line soft cap — Depends on [none]

- **BATCH**: B1
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-1-1/ (branch: feat/unified-desktop-phase-10-route-rework-1-1)
- **ACTION**: Create `src/crosshook-native/src/components/onboarding/` directory with five stage-body components (`OnboardingIdentityStageBody.tsx`, `OnboardingRuntimeStageBody.tsx`, `OnboardingTrainerStageBody.tsx`, `OnboardingMediaStageBody.tsx`, `OnboardingReviewStageBody.tsx`), then update `OnboardingWizard.tsx` to import and compose them.
- **IMPLEMENT**: Keep the stage machine (`useOnboarding`), `ProfileContext` consumers (`profile`, `profileName`, `updateProfile`, `persistProfileDraft`, `selectProfile`, `setProfileName`, `profileError`), portal + focus-trap + inert-siblings lifecycle, and the `handleBack`/`handleNext`/`handleComplete`/`handleSkip` handlers in `OnboardingWizard.tsx`. Extract each stage's JSX body into its stage-body component; pass only the props each stage needs. No state lifts, no new hooks.
- **MIRROR**: `WIZARD_PORTAL_MODAL_PATTERN`, `WIZARD_STAGE_MACHINE_PATTERN`, `FILE_SPLIT_EXTRACTION_PATTERN`.
- **IMPORTS**: `useOnboarding`, `useProfileContext`, `ProfileIdentitySection`, `ProfileLaunchSection`, `MediaSection`, `WizardPresetPicker`, `WizardReviewSummary`, `evaluateWizardRequiredFields` — all already imported by today's `OnboardingWizard.tsx`.
- **GOTCHA**: The `useLayoutEffect` that force-rewrites `native` → `proton_run` lives in `InstallGamePanel`, NOT the onboarding wizard — do not copy it. Conversely, preserve onboarding's `useEffect(() => { if (open && mode === 'create') void selectProfile(''); }, ...)` draft-reset in the parent file, not in a step component.
- **VALIDATE**: `wc -l src/crosshook-native/src/components/OnboardingWizard.tsx` returns <500. `npm test -- OnboardingWizard` green. Existing tests (`OnboardingWizard.test.tsx`) pass unchanged — the stage-body extraction should be transparent to the portal/focus test assertions because the stubs already replace child sections.

### Task 1.2: Split `CommunityBrowser` under the 500-line soft cap — Depends on [none]

- **BATCH**: B1
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-1-2/ (branch: feat/unified-desktop-phase-10-route-rework-1-2)
- **ACTION**: Create `src/crosshook-native/src/components/community/` directory with `TapChip.tsx`, `CompatibilityBadge.tsx`, `CommunityTapManagementSection.tsx`, and `CommunityProfilesSection.tsx`; update `CommunityBrowser.tsx` to import and compose them.
- **IMPLEMENT**: Move the inline `TapChip` (`CommunityBrowser.tsx:90-127`) and `CompatibilityBadge` (plus the `ratingLabel` / `ratingOrder` tables) into their own files with explicit props. Extract the Tap Management panel block (add-tap form + existing-tap list + Refresh/Sync buttons) into `CommunityTapManagementSection.tsx`. Extract the Profiles panel block (filter chips + search + result list + import buttons) into `CommunityProfilesSection.tsx`. The parent `CommunityBrowser.tsx` now composes two sibling sections plus the cached-fallback banner + diagnostics list + import modal.
- **MIRROR**: `FILE_SPLIT_EXTRACTION_PATTERN`, `COMMUNITY_CACHE_BANNER_PATTERN`, `TRY_FINALLY_ASYNC_ACTION_PATTERN`.
- **IMPORTS**: `useCommunityProfiles`, `ThemedSelect`, `CommunityImportWizardModal`, `useImportCommunityProfile` — no new imports beyond today's.
- **GOTCHA**: The `useMemo(visibleEntries)` filter is eager (no `useDeferredValue`, no `startTransition`). Do not add debounce or deferred-value state in this split — behavior parity is mandatory. Also: the seven `.catch(err => setError(...))` wrappers on async buttons (add-tap, refresh, sync, remove, pin, unpin, import) MUST stay in the parent so the single local `error` slot remains the source of truth — do not move error state into the extracted children.
- **VALIDATE**: `wc -l src/crosshook-native/src/components/CommunityBrowser.tsx` returns <500. `npm test` green. Smoke test still passes without any new console errors on the `community` route.

### Task 1.3: Add per-route stylesheet scaffolding and register in `main.tsx` — Depends on [none]

- **BATCH**: B1
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-1-3/ (branch: feat/unified-desktop-phase-10-route-rework-1-3)
- **ACTION**: Create five empty per-route stylesheets — `install-routes.css`, `settings-routes.css`, `community-routes.css`, `discover-routes.css`, `onboarding-wizard.css` — under `src/crosshook-native/src/styles/`. Register each with an `import './styles/<file>.css';` line in `src/crosshook-native/src/main.tsx` after the existing `dashboard-routes.css` import. Add the route-card scroll container selectors that Phase 10 will introduce (if any) to `SCROLL_ENHANCE_SELECTORS` in `src/crosshook-native/src/hooks/useScrollEnhance.ts`.
- **IMPLEMENT**: The five CSS files start empty (one-line header comment); Phase 10 batch 2 tasks populate each one. For `useScrollEnhance.ts`, append `.crosshook-install-page-tabs__panel-inner` to the SCROLL_ENHANCE_SELECTORS string — it's already referenced in `theme.css:571-617` as a scroll owner but was never registered. Do NOT pre-populate the per-route CSS files in this task — that happens in the corresponding batch-2 task to avoid cross-file coupling.
- **MIRROR**: `SCROLL_ENHANCE_REGISTRATION_PATTERN`, stylesheet-registration order from the existing `main.tsx:5-17` block.
- **IMPORTS**: none (pure CSS + import statements).
- **GOTCHA**: `variables.css` is NOT imported from `main.tsx` today; steel-blue tokens are actually loaded via `theme.css`. Do not "fix" this — Phase 2 intentionally left the token definitions duplicated. Verify with `grep 'variables.css' src/crosshook-native/src/main.tsx` — zero matches expected.
- **VALIDATE**: `npm run typecheck` passes. `./scripts/lint.sh` passes. `npm run test:smoke` still green — the five empty stylesheets must not regress any existing chrome.

### Task 2.1: Re-skin the Install route — Depends on [1.3]

- **BATCH**: B2
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-1/ (branch: feat/unified-desktop-phase-10-route-rework-2-1)
- **ACTION**: Update `src/crosshook-native/src/components/pages/InstallPage.tsx`, `src/crosshook-native/src/components/InstallGamePanel.tsx`, `src/crosshook-native/src/components/install/InstallReviewSummary.tsx`, and populate `src/crosshook-native/src/styles/install-routes.css`.
- **IMPLEMENT**: Wrap the outer InstallPage body in `<DashboardPanelSection eyebrow="Setup" title="Install & Run" summary="…">` around the two-level Radix Tabs block. Wrap each `InstallGamePanel` flow section (identity/runtime/trainer/media/installer_review) in a `<DashboardPanelSection>` with a per-section eyebrow/title. Harmonize every `<p className="crosshook-danger">` in `InstallReviewSummary` and `InstallPage` into `<div className="crosshook-error-banner crosshook-error-banner--section" role="alert">`. Populate `install-routes.css` with selectors targeting `.crosshook-page-scroll-shell--install`, `.crosshook-install-shell` (remap spacing/colors to dashboard tokens), and `.crosshook-install-flow-tabs` chrome. Preserve Radix Tabs + `forceMount` + `display:none` tab-swap exactly.
- **MIRROR**: `DASHBOARD_PANEL_SECTION_PATTERN`, `DASHBOARD_ROUTE_BODY_STACK_PATTERN`, `RADIX_TABS_FORCEMOUNT_PATTERN`, `CANONICAL_INLINE_ALERT_PATTERN`, `INSTALL_DRAFT_MERGE_PATTERN` (as a non-regression constraint).
- **IMPORTS**: `DashboardPanelSection` from `'../layout/DashboardPanelSection'` in InstallPage + InstallGamePanel.
- **GOTCHA**: The `useLayoutEffect` force-rewrite of `native` → `proton_run` at `InstallGamePanel.tsx:103-112` MUST stay untouched. The `activeInstallTab` auto-skip effect at `:121-135` MUST also stay untouched. The `profileReviewSession` dirty-state and `reviewConfirmationResolverRef` in InstallPage drive the promise-based confirm flow — visual rework must not refactor them. InstallGamePanel targets 460 lines today and must stay under 500 after the wrapping.
- **VALIDATE**: `npm run typecheck` green. `npm test` green. `npm run test:smoke` passes on the `install` route with no new console errors. Manual: run `./scripts/dev-native.sh --browser` and verify install wizard flows end-to-end at 1920×1080 (identity → runtime → review) without behavioral regressions.

### Task 2.2: Re-skin the Settings route — Depends on [1.3]

- **BATCH**: B2
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-2/ (branch: feat/unified-desktop-phase-10-route-rework-2-2)
- **ACTION**: Update `src/crosshook-native/src/components/SettingsPanel.tsx`, touch `src/crosshook-native/src/components/pages/SettingsPage.tsx` minimally, and populate `src/crosshook-native/src/styles/settings-routes.css`.
- **IMPLEMENT**: Wrap the SettingsPanel header + 2-column grid inside a single `<DashboardPanelSection eyebrow="App" title="App preferences and storage" summary="…">`. Preserve the 11 stacked `CollapsibleSection` sub-sections exactly as-is — DO NOT introduce Radix Tabs or lift any section into a tab. Populate `settings-routes.css` with selectors targeting `.crosshook-page-scroll-shell--settings`, `.crosshook-settings-panel`, `.crosshook-settings-grid`, `.crosshook-settings-column`, `.crosshook-settings-field-row` to align spacing, borders, and section-header typography with the dashboard chrome. Keep existing `SettingsPage.tsx` error-banner — it already uses the canonical `crosshook-error-banner crosshook-error-banner--section` + `role="alert"` shape.
- **MIRROR**: `DASHBOARD_PANEL_SECTION_PATTERN`, `DASHBOARD_ROUTE_BODY_STACK_PATTERN`, `SECTION_ONBLUR_PERSIST_PATTERN`, `WARNING_NOTICE_ROLE_STATUS_PATTERN`, `CANONICAL_INLINE_ALERT_PATTERN`.
- **IMPORTS**: `DashboardPanelSection` from `'./layout/DashboardPanelSection'` in `SettingsPanel.tsx`.
- **GOTCHA**: Do not touch the individual `*Section.tsx` files in `src/crosshook-native/src/components/settings/` — their per-section uncontrolled-input + `onBlur`-persist pattern is the Settings contract and Phase 10 explicitly preserves per-section save semantics. Per-section `<p className="crosshook-danger crosshook-settings-error">` spans stay as-is (they're handled inside each section file and are separately scoped); Phase 10 only harmonizes the page-level banner.
- **VALIDATE**: `npm run typecheck` green. `npm test` green. All 11 sections render, each able to load/save its slice of settings. Manual: at 1920×1080 confirm the 2-column grid still shows 11 sections + RecentFilesColumn unchanged.

### Task 2.3: Re-skin the Community route — Depends on [1.2, 1.3]

- **BATCH**: B2
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-3/ (branch: feat/unified-desktop-phase-10-route-rework-2-3)
- **ACTION**: Update `src/crosshook-native/src/components/CommunityBrowser.tsx` (now post-split from 1.2) and `src/crosshook-native/src/components/pages/CommunityPage.tsx`, and populate `src/crosshook-native/src/styles/community-routes.css`.
- **IMPLEMENT**: Replace the two `<CollapsibleSection>` wrappers (`Tap Management`, `Community Profiles`) with `<DashboardPanelSection>` instances that include eyebrow + title + summary + actions. Move the Tap Management "Refresh Index" / "Sync Taps" buttons into the section's `actions` slot. Keep the cached-fallback banner exactly as-is — it already uses `role="status" aria-live="polite"` + `crosshook-status-chip`. Harmonize the `.crosshook-community-browser__error` `<p>` into `<div className="crosshook-error-banner crosshook-error-banner--section" role="alert">`. Populate `community-routes.css` with steel-blue treatment for `.crosshook-community-tap` (background, border, hover halo that mirrors the route-banner gradient), `.crosshook-community-browser__cache-banner`, and `.crosshook-page-scroll-shell--community`. Import button semantics unchanged.
- **MIRROR**: `DASHBOARD_PANEL_SECTION_PATTERN`, `DASHBOARD_ROUTE_BODY_STACK_PATTERN`, `COMMUNITY_CACHE_BANNER_PATTERN`, `CANONICAL_INLINE_ALERT_PATTERN`, `TRY_FINALLY_ASYNC_ACTION_PATTERN`, `ROUTE_CARD_SHELL_PATTERN`.
- **IMPORTS**: `DashboardPanelSection` plus the four new community-local split components from task 1.2.
- **GOTCHA**: Do not touch the `useMemo(visibleEntries)` filter or replace it with `useDeferredValue` — behavior parity is mandatory. The import flow goes `prepareCommunityImport(path) → importDraft state → CommunityImportWizardModal` — preserve this 3-step chain exactly. The `ValidationStep` inside `CommunityImportWizardModal` reuses the class `crosshook-community-browser__error` (cross-component coupling) — leave the class name alone so the wizard's inline error doesn't silently break.
- **VALIDATE**: `npm run typecheck` green. `npm test -- Community` green. `npm run test:smoke` on the `community` route green with no new console errors. Manual: Add-Tap, Remove-Tap, Pin, Unpin, Refresh, Sync, and Import-from-file flows still work and surface inline errors in the new banner chrome.

### Task 2.4: Re-skin the Discover route — Depends on [1.3]

- **BATCH**: B2
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-4/ (branch: feat/unified-desktop-phase-10-route-rework-2-4)
- **ACTION**: Update `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` and `src/crosshook-native/src/components/pages/DiscoverPage.tsx`, and populate `src/crosshook-native/src/styles/discover-routes.css`.
- **IMPLEMENT**: Wrap the consent-gate block, the search bar, and the results list in three `<DashboardPanelSection>` instances with distinct eyebrows ("Community", "Search", "Results"). Replace the `.crosshook-discovery-panel__error` `<p>` with `<div className="crosshook-error-banner crosshook-error-banner--section" role="alert">`; replace `.crosshook-discovery-panel__notice` `<p>` with `<div className="crosshook-success-banner ...--section" role="status">` (or keep `crosshook-warning-banner`-style notice class if a success variant does not exist — check `theme.css` first). Populate `discover-routes.css` with steel-blue halo + hover for `.crosshook-discovery-card` (mirrors the route-banner gradient), plus `.crosshook-page-scroll-shell--discover` spacing. Keep the `pendingConsent` state + `handleConsentAccept` path intact.
- **MIRROR**: `DASHBOARD_PANEL_SECTION_PATTERN`, `DASHBOARD_ROUTE_BODY_STACK_PATTERN`, `DASHBOARD_PILL_ROW_PATTERN`, `CANONICAL_INLINE_ALERT_PATTERN`, `TRY_FINALLY_ASYNC_ACTION_PATTERN`.
- **IMPORTS**: `DashboardPanelSection` from `'./layout/DashboardPanelSection'`.
- **GOTCHA**: The 300 ms debounce in `useTrainerDiscovery` + the request-id race guard MUST NOT be replicated or weakened in the component. `settings.discovery_enabled` is the persistence gate — the component only toggles it through `persistSettings({ discovery_enabled: true })` → `settings_save` IPC. Keep the `aria-live="polite" aria-atomic="true"` region around the results-meta status — the live region is load-bearing for screen-reader feedback.
- **VALIDATE**: `npm run typecheck` green. `npm test` green. `npm run test:smoke` on the `discover` route green. Manual: consent accept path, search with > 3 chars, import (single result), and the disabled→enabled transition all behave identically.

### Task 2.5: Re-skin the OnboardingWizard modal chrome — Depends on [1.1, 1.3]

- **BATCH**: B2
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-2-5/ (branch: feat/unified-desktop-phase-10-route-rework-2-5)
- **ACTION**: Update `src/crosshook-native/src/components/OnboardingWizard.tsx` (now post-split from 1.1), and populate `src/crosshook-native/src/styles/onboarding-wizard.css`.
- **IMPLEMENT**: Align the `crosshook-modal__header` to use the Phase 9 `crosshook-heading-eyebrow` + `crosshook-heading-title--card` combination the dashboards use (the eyebrow text "Step X of Y" stays identical; class alignment only). Harmonize the wizard's `profileError` surface from `<p className="crosshook-danger" role="alert" style={{ marginBottom: 12 }}>` to `<div className="crosshook-error-banner crosshook-error-banner--section" role="alert">` (drop inline style). Populate `onboarding-wizard.css` with selectors targeting `.crosshook-onboarding-wizard`, `.crosshook-onboarding-wizard .crosshook-modal__header`, and the stage-body shared spacing. Do NOT modify portal mount, focus-trap, `inert`-siblings, or stage-advance handlers.
- **MIRROR**: `WIZARD_PORTAL_MODAL_PATTERN`, `WIZARD_STAGE_MACHINE_PATTERN`, `CANONICAL_INLINE_ALERT_PATTERN`.
- **IMPORTS**: no new imports.
- **GOTCHA**: The wizard mount lifecycle creates `document.createElement('div')` + `body.classList.add('crosshook-modal-open')` + `(element as HTMLElement).inert = true` on every sibling outside `#crosshook-root`. This path is fragile — a stray `className` edit that removes `crosshook-modal__backdrop` breaks `handleBackdropMouseDown`. Read `OnboardingWizard.tsx:166-233` before touching any class. The `handleKeyDown` listener relies on `data-crosshook-focus-root="modal"` — do not rename.
- **VALIDATE**: `npm run typecheck` green. `npm test -- OnboardingWizard` green with no new failures. Manual: open the wizard (trigger `onboarding-check` event via dev mocks or cold-start profile-less state), verify heading focus, Tab trap, Esc dismiss, Skip Setup, and Back/Next flow.

### Task 3.1: Add focused RTL route-shell coverage — Depends on [2.1, 2.2, 2.3, 2.4, 2.5]

- **BATCH**: B3
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-3-1/ (branch: feat/unified-desktop-phase-10-route-rework-3-1)
- **ACTION**: Create four new RTL test files under `src/crosshook-native/src/components/__tests__/`: `InstallGamePanel.test.tsx`, `SettingsPanel.test.tsx`, `CommunityBrowser.test.tsx`, `TrainerDiscoveryPanel.test.tsx`.
- **IMPLEMENT**: For each file, mirror the `OnboardingWizard.test.tsx` harness — plain `render`, `vi.mock(...)` stubs for heavy child sections, assertions on: (a) root `<section>` renders with the new dashboard panel chrome (`expect(screen.getByRole('region', { name: /…/ })).toBeInTheDocument()`), (b) error banner `role="alert"` regression (`expect(screen.queryByRole('alert')).not.toBeInTheDocument()` in the happy path, present when an error is injected), (c) for Install: tab-trigger visibility and `forceMount` content persistence across tab switches, (d) for Settings: all 11 sections render, (e) for Community: the cached-fallback `role="status"` region appears when `cachedTapNotices.length > 0`, (f) for Discover: consent gate shows when `settings.discovery_enabled === false`.
- **MIRROR**: `VITEST_PORTAL_WIZARD_TEST_PATTERN`, existing harness at `src/crosshook-native/src/components/__tests__/OnboardingWizard.test.tsx:1-161`.
- **IMPORTS**: `{ render, screen, waitFor } from '@testing-library/react'`, `userEvent`, `{ beforeEach, describe, expect, it, vi } from 'vitest'`, fixture factories from `'@/test/fixtures'` (if missing fixtures, extend `@/test/fixtures` with the minimum scaffolding needed).
- **GOTCHA**: These tests use plain `render`, NOT `renderWithMocks` — integration-scale route shell testing is out of scope. Stub `useCommunityProfiles`, `useTrainerDiscovery`, `useInstallGame`, `useOnboarding`, and `usePreferencesContext` per file so the suite does not touch IPC. Keep tests scoped to chrome/shell regression; do NOT duplicate wizard-step or search-pipeline behavioral tests.
- **VALIDATE**: `npm test` green with four new test files passing. `npm run test:coverage` reports coverage on each of the four target components. No added `console.error` noise during the run.

### Task 3.2: Expand Playwright smoke to lock in Phase 10 chrome — Depends on [2.1, 2.2, 2.3, 2.4, 2.5]

- **BATCH**: B3
- **Worktree**: ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework-3-2/ (branch: feat/unified-desktop-phase-10-route-rework-3-2)
- **ACTION**: Update `src/crosshook-native/tests/smoke.spec.ts` to assert the Phase 10 chrome on each of the four redesigned routes.
- **IMPLEMENT**: Extend `DASHBOARD_ROUTE_HEADINGS` with entries for `install` / `settings` / `community` / `discover` matching the new `<DashboardPanelSection title>` copy. Extend the per-route visibility assertion to include the new dashboard-route-body locator: `await expect(page.locator('.crosshook-dashboard-route-body, .crosshook-host-tool-dashboard, .crosshook-install-page-tabs, .crosshook-settings-panel, .crosshook-community-browser, .crosshook-discovery-panel').first()).toBeVisible();`. Keep the existing `ROUTE_ORDER` list unchanged — all four routes already appear. Keep the `attachConsoleCapture` assertion at the end of each route iteration (`expect(capture.errors).toEqual([])`). Add a focused assertion that the page-level error banner uses `role="alert"` when surfaced (harder to reach organically — skip unless mocked data provides the error state).
- **MIRROR**: `SMOKE_ROUTE_SWEEP_PATTERN`, existing `attachConsoleCapture` contract.
- **IMPORTS**: none new (reuse `ROUTE_NAV_LABEL`, `ROUTE_ORDER`, helpers from `tests/helpers.ts`).
- **GOTCHA**: If browser mocks don't have enough data to exercise Community (tap list) or Discover (result list) bodies, extend mocks in `src/crosshook-native/src/lib/mocks/` before loosening the assertion. Do NOT weaken the zero-console-error gate to paper over missing fixtures.
- **VALIDATE**: `npm run test:smoke` green on all 11 routes, with screenshot captures for `install`, `settings`, `community`, `discover` showing the new chrome. `npm run test:smoke` exits 0 with `capture.errors.length === 0` on every route.

---

## Testing Strategy

### Unit / Integration Tests

| Test                                  | Input                                                              | Expected Output                                                                                     | Edge Case? |
| ------------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------- | ---------- |
| OnboardingWizard post-split render    | Render `OnboardingWizard` with `mode='create'`, stubbed hooks      | Portal mounts, heading gets focus, "Step 1 of N" eyebrow renders, existing tests pass unmodified    | No         |
| InstallGamePanel shell render         | Plain render with `vi.mock` stubs for heavy child sections         | `<DashboardPanelSection>` chrome present; tab triggers visible; flow-tab content uses `forceMount`  | No         |
| InstallGamePanel error banner parity  | Inject install `stage='failed'` with error message                 | Error surface renders as `<div role="alert">` with `crosshook-error-banner--section` class          | Yes        |
| SettingsPanel 11-section render       | Plain render with stubbed context                                  | All 11 `CollapsibleSection` sub-sections appear; no sub-tabs introduced                             | No         |
| SettingsPage error banner             | Render with `settingsError='…'` in context                         | `role="alert"` banner visible above the panel; happy-path render has no `role="alert"`              | Yes        |
| CommunityBrowser panels render        | Plain render with tap + profile fixtures                           | Two `<DashboardPanelSection>` regions render; cached-fallback banner hidden when no cached notices  | No         |
| CommunityBrowser cached fallback      | Inject `cachedTapNotices=[…]`                                      | `role="status" aria-live="polite"` banner with `crosshook-status-chip` visible                      | Yes        |
| CommunityBrowser async error          | Reject `refreshProfiles` in test                                   | `<div role="alert">` banner surfaces the error message                                              | Yes        |
| TrainerDiscoveryPanel consent gate    | `settings.discovery_enabled=false`, `pendingConsent=false`         | Gate block visible with Enable button; search bar not rendered                                      | No         |
| TrainerDiscoveryPanel search + import | Enable discovery, search for "cheat", click import on first result | `importCommunityProfile` is called with the expected profile path; notice surfaces with role=status | Yes        |
| Playwright smoke `install`            | Navigate via tab to Install                                        | Heading matches Phase 10 title; `.crosshook-install-page-tabs` visible; zero console errors         | Yes        |
| Playwright smoke `settings`           | Navigate to Settings                                               | Heading matches; `.crosshook-settings-panel` visible; zero console errors                           | Yes        |
| Playwright smoke `community`          | Navigate to Community                                              | Heading matches; `.crosshook-community-browser` visible; zero console errors                        | Yes        |
| Playwright smoke `discover`           | Navigate to Discover                                               | Heading matches; `.crosshook-discovery-panel` visible; zero console errors                          | Yes        |

### Edge Cases Checklist

- [ ] Install review-session dirty-state confirmation still prompts before discarding edits.
- [ ] Install `native` launch method auto-rewrites to `proton_run` and the trainer tab is auto-skipped.
- [ ] Settings per-section `onBlur`-persist round-trip still calls `settings_save` on change and skips it on unchanged input.
- [ ] Community cached-fallback banner remains visible when `lastTapSyncResults` contains cache-hit entries.
- [ ] Community import of a file with `required_prefix_deps.length > 0` still shows the security-warning `role="alert"` block at the ProfileDetails step.
- [ ] Discover consent-enable path persists `discovery_enabled: true` and re-renders with search enabled.
- [ ] Discover with `discovery_enabled=true` but `query.length < 3` shows neither loading spinner nor error.
- [ ] OnboardingWizard dismiss (backdrop + Esc + Skip) each call `onDismiss` exactly once.
- [ ] OnboardingWizard `useEffect(selectProfile(''))` still runs on open when `mode='create'`.
- [ ] All five new CSS files register without introducing unregistered scroll containers (grep `SCROLL_ENHANCE_SELECTORS` covers any `overflow-y: auto` declaration touching `crosshook-*` classes).
- [ ] No task adds a second scroll owner that is missing from `SCROLL_ENHANCE_SELECTORS`.
- [ ] Playwright mocks for Community (tap list) and Discover (result list) contain enough data for the new chrome assertions to fire.

---

## Validation Commands

### Static Analysis

```bash
npm run typecheck
```

EXPECT: Zero TypeScript errors across InstallPage, InstallGamePanel, InstallReviewSummary, OnboardingWizard + onboarding stage bodies, SettingsPanel, SettingsPage, CommunityBrowser + community sub-components, CommunityPage, DiscoverPage, TrainerDiscoveryPanel, and the five new CSS imports in main.tsx.

### Unit / Integration Tests

```bash
npm test
```

EXPECT: Vitest green, including the four new RTL test files from Task 3.1 and the unchanged `OnboardingWizard.test.tsx`.

### Full Lint Pass

```bash
./scripts/lint.sh
```

EXPECT: Biome, ESLint-replacement checks, and Rust workspace lints all pass after the CSS + component churn. Fixable issues can be resolved with `./scripts/lint.sh --fix`.

### Browser Smoke

```bash
npm run test:smoke
```

EXPECT: Browser-dev smoke captures `install`, `settings`, `community`, `discover` (plus the existing 7 routes) with zero `pageerror` / `console.error` per `attachConsoleCapture`.

### Full Rust Suite (for parity — no Rust changes expected)

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: Green. Phase 10 is frontend-only; this command is a parity guard to confirm no unintended Rust churn.

### Host Gateway Guard (no-op expected)

```bash
./scripts/check-host-gateway.sh
```

EXPECT: Green. No Rust changes → no gateway changes.

### Manual Validation

- [ ] Run `./scripts/dev-native.sh --browser` and visually inspect Install, Settings, Community, Discover at `1920×1080`.
- [ ] Repeat visual inspection at `3440×1440` to confirm the reskin still fills the ultrawide shell and the new panel chrome stays coherent next to the Phase 9 dashboard routes.
- [ ] Repeat at `1280×800` (Deck) to confirm no layout regression on the smaller viewport.
- [ ] Verify Install: identity → runtime → review flow completes; native launch method auto-rewrites to proton_run; review confirmation prompt still blocks unsaved dismissal.
- [ ] Verify Settings: toggle at least one section (Startup, SteamGridDB, Profiles Directory) and confirm persistence via `settings_save` round-trip.
- [ ] Verify Community: Add-Tap, Refresh, Sync, Pin, Unpin, Remove, and Import-from-File all work; cached-fallback banner visible when offline sync returns cached entries.
- [ ] Verify Discover: consent gate → enable → search (≥3 chars) → import result; `discovery_enabled` persists across reloads.
- [ ] Verify OnboardingWizard: triggered on first run with no profiles; heading focus, Tab trap, Esc dismiss, Skip Setup, Back/Next all behave identically.

---

## Acceptance Criteria

- [ ] All ten tasks completed across the three batches.
- [ ] The four Phase 10 routes (`install`, `settings`, `community`, `discover`) share the Phase 9 dashboard visual language — `<DashboardPanelSection>` chrome, `crosshook-dashboard-route-body` stack, harmonized inline alerts.
- [ ] `OnboardingWizard.tsx` and `CommunityBrowser.tsx` each drop below the 500-line soft cap.
- [ ] All existing Install / Settings / Community / Discover / Onboarding behavior is preserved — no IPC changes, no persistence changes, no route-order changes, no inspector-wiring changes.
- [ ] Five new per-route stylesheets are registered in `main.tsx`; any new scroll container is registered in `SCROLL_ENHANCE_SELECTORS`.
- [ ] Playwright smoke asserts Phase 10 chrome on all four redesigned routes with zero `pageerror` / `console.error`.
- [ ] Four new RTL test files cover the redesigned route shells + error-banner regression.
- [ ] `npm run typecheck`, `npm test`, `./scripts/lint.sh`, `npm run test:smoke`, and `cargo test -p crosshook-core` all pass.

## Completion Checklist

- [ ] `<DashboardPanelSection>` is reused — no new panel primitive is introduced.
- [ ] Inline alerts are harmonized on `crosshook-error-banner--section` + `role="alert"` where the surface is an error, and `crosshook-warning-banner` + `role="status"` where the surface is a non-blocking notice.
- [ ] Radix Tabs + `forceMount` + `display:none` tab swap is preserved at both Install tab levels.
- [ ] Portal + focus-trap + inert-siblings lifecycle is untouched in `OnboardingWizard.tsx`.
- [ ] No section file under `src/crosshook-native/src/components/settings/` is functionally rewritten — only CSS-level reskin.
- [ ] No IPC command is added, removed, or renamed.
- [ ] No new npm dependency is introduced.
- [ ] No new TOML field or SQLite table is introduced.
- [ ] No `theme.css` structural refactor occurs; Phase 10 CSS lives in the five new per-route files.
- [ ] `SCROLL_ENHANCE_SELECTORS` is audited — every new `overflow-y: auto` declaration has a matching entry.
- [ ] Per-worktree prerequisites (`npm install -D --no-save typescript biome` + `npm ci` inside `src/crosshook-native/`) are documented in the worktree setup notes for each parallel task.

## Risks

| Risk                                                                                                    | Likelihood | Impact | Mitigation                                                                                                                                                                      |
| ------------------------------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OnboardingWizard split lifts stage state into a child and breaks stage advance                          | M          | H      | Keep the `stage` value + `advanceOrSkip/goBack/dismiss` calls in `OnboardingWizard.tsx`; stage-body components receive data, not control. Preserve `OnboardingWizard.test.tsx`. |
| CommunityBrowser split moves `error` state into a child and the seven `.catch` wrappers lose their sink | M          | H      | Explicitly keep `setError` + the seven `.catch(err => setError(...))` wrappers in the parent. Children consume `error` via props, never own it.                                 |
| Install `useLayoutEffect` that rewrites `native` → `proton_run` is refactored accidentally              | L          | H      | Call it out as a do-not-touch invariant in Task 2.1 GOTCHA; add a test in Task 3.1 that asserts mount-time launch method rewrites on native.                                    |
| New per-route CSS files leak selectors that override Phase 9 dashboard chrome                           | M          | M      | Scope every new selector under `.crosshook-page-scroll-shell--<route>` or a route-specific parent class. CI smoke captures dashboard-route screenshots and catches regressions. |
| Settings "sub-tabs" interpretation — PRD says sub-tabs, code has none; stakeholder may expect new tabs  | M          | M      | Plan explicitly freezes Settings as stacked `CollapsibleSection`s (see NOT Building). Flag to the user during implementation kickoff that this is the chosen interpretation.    |
| New `overflow-y: auto` containers miss `SCROLL_ENHANCE_SELECTORS` registration → WebKitGTK scroll jank  | M          | M      | Task 1.3 pre-registers the known new selectors; Task 2.\* tasks audit each file before merge. A review checklist item enforces scroll-selector coverage per PR.                 |
| Error-banner harmonization changes `role` where a screen reader already depended on the prior role      | L          | M      | Task 2.\* sections each explicitly note which surfaces stay `role="status"` (cached fallbacks, restart notices) vs which move to `role="alert"` (actionable failures).          |
| Playwright smoke mocks lack data for Community/Discover bodies, forcing assertion weakening             | H          | M      | Task 3.2 requires fixture updates before loosening assertions. Zero-console-error gate is non-negotiable.                                                                       |
| Worktree fan-in merge conflict on `main.tsx` between 1.3 and 2.\* (all import CSS)                      | L          | L      | 1.3 registers all five CSS files up front; 2.\* tasks populate their CSS file contents but never edit `main.tsx`.                                                               |
| `useScrollEnhance.ts` SCROLLABLE selector becomes too long / unreadable                                 | L          | L      | Phase 10 appends at most 1-2 selectors. If the constant drifts past readability, file a chore issue to chunk it; do not split it mid-phase.                                     |
| The PRD Phase 10 Note "Community/Discover cards updated" is interpreted as LibraryCard-style idioms     | M          | L      | Plan explicitly scopes Community/Discover card chrome to `.crosshook-community-tap` + `.crosshook-discovery-card` steel-blue treatment — NOT a LibraryCard hover-gradient port. |

## Notes

- Research dispatch used the requested `--parallel` mode: three standalone sub-agent `ycc:prp-researcher` instances (patterns-research, quality-research, infra-research) ran concurrently in a single message. Findings were merged into the `Patterns to Mirror` table with de-duplication and gap annotations.
- `variables.css` is NOT imported from `main.tsx` today (confirmed by the infra researcher). Phase 2 steel-blue tokens are actually loaded via `theme.css`. Phase 10 does not change this — any "fix" is Phase 13 scope.
- `crosshook-page-scroll-shell--install|--settings|--community|--discover` modifier classes are already on the JSX but have zero matching CSS. Phase 10 fills them in via the five new per-route stylesheets, which means the reskin is purely additive and the old chrome continues to work if a worktree is abandoned mid-flight.
- Settings has no Radix Tabs layer today; the PRD's "keeps its sub-tabs" wording is interpreted here as "preserve the existing 11 `CollapsibleSection` sub-sections". This is called out in NOT Building and in the Risks table so the stakeholder can redirect early if their intent differs.
- Both 500-line splits (OnboardingWizard, CommunityBrowser) are done in Batch 1 so the reskin in Batch 2 can target smaller, modular surfaces. Neither split changes any hook contract or IPC call.
- `InstallGamePanel.tsx` is 460 lines today — close to the cap. After wrapping its five flow sections in `<DashboardPanelSection>` in Task 2.1, re-run `wc -l` and split further if the file breaches 500. If so, file a small follow-up task (not a blocker for Phase 10).
- Phase 10 runs in parallel with Phase 9 per the PRD, but Phase 9 is already complete (see `docs/prps/plans/completed/github-issues-421-448-route-rework-dashboards.plan.md`). Phase 10's dependencies (Phases 2, 3) are both pending per the PRD phase table — however, the `DashboardPanelSection` primitive and `dashboard-routes.css` Phase 10 reuses landed with Phase 9 work on `main`. If Phase 2/3 state regresses, the reskin will surface the pre-steel-blue palette; that is a Phase 2/3 problem, not Phase 10's.
- All merged research output is retained in the conversation transcript; each discovery-table finding in `Patterns to Mirror` cites `file:line` so an implementor can re-read the source without reopening the research phase.
