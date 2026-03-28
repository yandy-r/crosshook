# Context Analysis: health-dashboard-page

## Executive Summary

The Health Dashboard is a **frontend-only** read-only diagnostics page added as a new top-level route (`'health'`). All backend infrastructure exists from prior health phases — no Rust changes required. The feature creates one new page component (`HealthDashboardPage.tsx`), modifies five routing files, and ships in three independently releasable phases. No new context file or hook refactor is needed — `forceMount: true` on all tabs means independent `useProfileHealth()` instances per page are safe by design.

**Metadata access pattern**: The hook types `summary` as `HealthCheckSummary` (base type), but the Tauri backend returns `EnrichedHealthSummary`. Access metadata via type cast: `const enriched = report as EnrichedProfileHealthReport; const metadata = enriched.metadata ?? null;` — this is the same pattern `ProfilesPage.tsx:507` uses today.

---

## Architecture Context

### System Structure

```
App.tsx (Radix UI Tabs.Root, NOT React Router)
  └─ AppShell
       └─ ContentArea (Tabs.Content, forceMount: true — all pages stay mounted)
            └─ HealthDashboardPage (route="health")
                 ├─ PageBanner (eyebrow="Dashboards")
                 ├─ SummaryCards (4 cards: total/healthy/stale/broken)    [P1, P3 +trends]
                 ├─ CollapsibleSection "Re-check"                          [P1]
                 ├─ CollapsibleSection "Issue Breakdown"                   [P2]
                 ├─ CollapsibleSection "All Profiles" (HealthTable)        [P1 base, P2 full]
                 ├─ CollapsibleSection "Recent Failures"                   [P2, defaultOpen=false]
                 ├─ CollapsibleSection "Launcher Drift"                    [P2, defaultOpen=false]
                 └─ CollapsibleSection "Community Import Health"           [P2, defaultOpen=false]
```

### Data Flow

```
HealthDashboardPage
  └─ useProfileHealth()   ← independent instance, safe alongside ProfilesPage's instance
       ├─ listen("profile-health-batch-complete") → EnrichedHealthSummary (startup event)
       ├─ invoke("batch_validate_profiles") → EnrichedHealthSummary (700ms fallback)
       └─ invoke("get_cached_health_snapshots") → CachedHealthSnapshot[] (on mount)

"Fix" action → void selectProfile(name) + onNavigate?.('profiles')
```

### Routing Integration (4 atomic changes, TypeScript-enforced)

1. `Sidebar.tsx:12` — `AppRoute` union: add `| 'health'`
2. `Sidebar.tsx:53` — `ROUTE_LABELS` record: add `health: 'Health'`
3. `Sidebar.tsx:32` — `SIDEBAR_SECTIONS`: add new "Dashboards" section with health item
4. `App.tsx:14` — `VALID_APP_ROUTES`: add `health: true` (**not** TypeScript-enforced — must not be forgotten)
5. `ContentArea.tsx:34` — exhaustive switch: add `case 'health':` (compile error at line 48 until wired)

**Note**: `settings` is NOT in `SIDEBAR_SECTIONS` — it's in the sidebar footer only. `'health'` goes in a section, not the footer.

---

## Critical Files Reference

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfileHealth.ts` — primary data hook; call directly from `HealthDashboardPage`; no context wrapper needed; startup event fallback at line 178–183
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/health.ts` — all health TypeScript interfaces; `EnrichedHealthSummary`, `EnrichedProfileHealthReport`, `ProfileHealthMetadata`, `HealthIssue`, `CachedHealthSnapshot`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/Sidebar.tsx` — `AppRoute` union (line 12), `ROUTE_LABELS` (line 53), `SIDEBAR_SECTIONS` (line 32)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/ContentArea.tsx` — exhaustive route switch (line 34), `never` guard at line 48, `data-crosshook-focus-zone="content"` already on wrapper (line 30)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx` — `VALID_APP_ROUTES` (line 14)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/HealthBadge.tsx` — drop-in status chip; accepts `report`, `trend`, `metadata`, `tooltip`; maps `healthy→working`, `stale→partial`, `broken→broken` for CSS classes
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ui/CollapsibleSection.tsx` — use for all secondary panels; has `meta` prop for header counts; uses native `<details>/<summary>` (NOT valid inside `<table>`)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/ProfilesPage.tsx` — source of `formatRelativeTime` (line 22–36); metadata cast pattern at line 507; health badge + issue display pattern at lines 501–563
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CompatibilityViewer.tsx` — exact filter + `useDeferredValue` pattern to replicate (lines 110–140)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/context/ProfileContext.tsx` — `useProfileContext()` for `selectProfile()` in Fix navigation; throws if called outside `<ProfileProvider>` (safe — provider wraps all of App.tsx:114)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/health.ts` — `countProfileStatuses()` already aggregates status counts
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/PageBanner.tsx` — add `HealthDashboardArt` SVG (200x120 viewBox, `fill: none`, opacity 0.1–0.5 geometry)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/icons/SidebarIcons.tsx` — add `HealthIcon` SVG (20x20 viewBox, `stroke: currentColor`, `strokeWidth: 1.5`)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css` — add `crosshook-health-dashboard*` CSS classes; do NOT create a new CSS file
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src-tauri/src/commands/health.rs` — `sanitize_issues()` sanitizes `issue.path` but NOT `issue.message`

---

## Patterns to Follow

- **Page component structure**: Fragment + `PageBanner` + content. Receives `onNavigate?: (route: AppRoute) => void` from `ContentArea`. See `CommunityPage.tsx` (thin wrapper) and `InstallPage` (ContentArea.tsx:40 for prop pass-through)
- **Single-file page with inline sub-components**: Follow `ProfilesPage.tsx` pattern (~715 lines); extract to separate files only if >800 lines after Phase 2
- **Metadata access**: `const enriched = report as EnrichedProfileHealthReport; const metadata = enriched.metadata ?? null;` — null-guard all metadata fields throughout. Pattern from `ProfilesPage.tsx:507`
- **Filter + deferred search**: `useDeferredValue` on search input. `String.includes()` for matching — never `new RegExp(userInput)`. See `CompatibilityViewer.tsx:110-140`
- **Sortable table**: Hand-rolled `useMemo` sort/filter. `STATUS_RANK = { healthy:0, stale:1, broken:2 }`. Default: broken-first, alpha within groups
- **Gamepad D-pad in table**: `tabIndex={0}` on each `<tr>`. `data-crosshook-focus-zone="content"` already on `ContentArea` wrapper — no additional zone attribute needed. `<CollapsibleSection>` `<summary>` elements are automatically in `FOCUSABLE_SELECTOR`
- **Y button (P3)**: Page-local `useEffect` polling `navigator.getGamepads()[0].buttons[3]` via `requestAnimationFrame`; edge-detect (track prev state); call `batchValidate()` on leading edge; do NOT modify shared `useGamepadNav`
- **Fix navigation**: `void selectProfile(profileName); onNavigate?.('profiles');` — `selectProfile` is async but navigation can fire immediately; `ProfileContext` wraps entire app
- **CSS naming**: `crosshook-health-dashboard*` namespace in existing `theme.css`; use `--crosshook-color-success/warning/danger/accent` variables; use `.crosshook-card` for summary cards, `.crosshook-panel` for secondary panels
- **Status announcements**: `role="status"` + `aria-live="polite"` for validation complete; `role="alert"` for errors only
- **Error display**: `console.error(...)` + generic "Health scan failed" in UI — never surface raw IPC error strings (may contain unsanitized paths before sanitization)

---

## Cross-Cutting Concerns

- **`forceMount: true` on all tabs**: Both `ProfilesPage` and `HealthDashboardPage` are simultaneously mounted. Each has an independent `useProfileHealth()` instance with its own state and AbortController. Both receive the `profile-health-batch-complete` Tauri event independently — this is by design, not a conflict
- **`summary` starts null**: Guard every access — `summary?.profiles ?? []`, `summary?.broken_count ?? 0`. Hook fires 700ms fallback if startup event not received
- **Null metadata**: `ProfileHealthMetadata` is `null` when SQLite `MetadataStore` is unavailable. Every metadata access must null-guard. Page must never crash on null metadata
- **XSS**: All profile name rendering via JSX interpolation only (`{profile.name}`). Never `dangerouslySetInnerHTML`
- **CSP gap**: Add `style-src 'self' 'unsafe-inline'` to `tauri.conf.json` — existing codebase uses inline styles extensively; this acknowledges an existing pattern, not a new risk
- **`issue.message` contains unsanitized home paths**: `sanitize_issues()` sanitizes `issue.path` but not `issue.message`. Display `issue.path` (pre-sanitized with `~`) as the primary path reference; show `issue.message` as supplementary text only
- **`<unknown>` sentinel**: When `ProfileStore.list()` fails, batch check returns a single `name:"<unknown>"` entry. Detect and render a system-level error banner instead of a broken profile row
- **`VALID_APP_ROUTES` is NOT TypeScript-enforced**: The compiler catches the missing `ContentArea` switch case via `never`, but does NOT catch a missing entry in `VALID_APP_ROUTES` (it's a `Record`, not a switch). Must update manually
- **`CollapsibleSection` controlled vs. uncontrolled**: Do NOT mix `defaultOpen` and `open` props — pick one control model per section. Controlled mode syncs state via `useEffect` that mutates `element.open` directly

---

## Parallelization Opportunities

```
P1 (sequential start, then parallel):
  1.1 Routing (5 files, atomic — must all land together) → 1.2 Page Shell + Cards
  then 1.3 (Profile List) and 1.4 (Re-check All) are INDEPENDENT of each other

P2 (after P1):
  2.1 Sortable Table + extract formatRelativeTime → then all independent:
  2.2 Filter/Search | 2.3 Fix Nav | 2.4 Issue Breakdown | 2.5 Recent Failures |
  2.6 Launcher Drift | 2.7 Community Import Health

P3 (after P1, all INDEPENDENT of each other and of P2):
  3.1 Trend Arrows | 3.2 Skeleton Loading | 3.3 Y Button | 3.4 Responsive Layout
  NOTE: P3 and remaining P2 panels can proceed in parallel
```

---

## Implementation Constraints

- **No new npm dependencies** — hand-roll everything; `@tanstack/react-table` pre-approved as future upgrade only if sort/filter complexity grows
- **No new Tauri IPC commands** — consume only existing three commands via `useProfileHealth`
- **No `ProfileHealthContext` needed** — `forceMount: true` makes independent instances safe; skip this refactor
- **No `useProfileHealth` type changes needed** — use type cast pattern (`as EnrichedProfileHealthReport`) following `ProfilesPage.tsx:507`
- **No virtualization in v1** — plain `<table>` sufficient for ≤50 profiles; `react-window` available if needed later
- **`formatRelativeTime`**: Copy from `ProfilesPage.tsx:22–36` for P1 use in the page; extract to `src/utils/time.ts` in P2 (two consumers justifies extraction)
- **Search `maxLength={200}`**: Required hygiene on the filter input
- **Routing changes are atomic**: All 5 routing edits (Sidebar ×3, App, ContentArea + new page import) must land in one commit — `never` guard in `ContentArea.tsx:48` causes compile error on partial state

---

## Key Recommendations

1. **Start with routing wiring** (task 1.1, atomic) — TypeScript exhaustive check makes this the natural first step; compile error on missing switch case confirms completeness
2. **Remember `VALID_APP_ROUTES`** — it is NOT TypeScript-enforced; easy to forget after the `never` check passes
3. **Use the cast pattern for metadata** — `report as EnrichedProfileHealthReport` then null-guard all fields; do not attempt to change the hook's generics
4. **Sidebar section is "Dashboards"** (new section) — confirmed from code analysis; not under "Game" section
5. **`onNavigate` prop on the page**: `HealthDashboardPage` receives `onNavigate?: (route: AppRoute) => void` from `ContentArea`; include from P1 since Fix navigation uses it immediately
6. **Render from `cachedSnapshots` on mount** — never show "0 broken" while validation runs; show `—` placeholders until `summary` is non-null
7. **`CollapsibleSection` `meta` prop** — use for showing counts in section headers (e.g., `meta={<span>{brokenCount} issues</span>}`)
8. **P3 trend arrows are trivial** — `trendByName[profile.name]` is already computed by the hook; pass directly to `HealthBadge`'s `trend` prop — zero new logic needed

---

## Sources

- `docs/plans/health-dashboard-page/shared.md` — file list and relevant patterns
- `docs/plans/health-dashboard-page/feature-spec.md` — resolved decisions, phased user stories, business rules
- `docs/plans/health-dashboard-page/research-technical.md` — architecture spec, phase boundary contracts
- `docs/plans/health-dashboard-page/research-business.md` — BR-01 through BR-15, edge cases EC-01 through EC-07
- `docs/plans/health-dashboard-page/research-ux.md` — layout, gamepad nav, accessibility, loading states
- `docs/plans/health-dashboard-page/research-security.md` — XSS mitigation, CSP, path sanitization gaps
- `docs/plans/health-dashboard-page/research-practices.md` — reusable code inventory, KISS assessment
- `docs/plans/health-dashboard-page/research-recommendations.md` — phased plan, risk assessment
- `docs/plans/health-dashboard-page/research-external.md` — library evaluation (all "no new dep")
- `docs/plans/health-dashboard-page/analysis-code.md` — code pattern analysis (routing system, forceMount behavior, type cast pattern, CSS classes)
