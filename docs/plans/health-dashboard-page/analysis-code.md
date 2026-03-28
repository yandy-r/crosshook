# Health Dashboard Page — Code Pattern Analysis

## Executive Summary

The health dashboard is a pure-frontend addition: one new page component (`HealthDashboardPage.tsx`), four file edits for routing, two file edits for icons and art, and zero Rust changes. All health data is already available via `useProfileHealth` — the hook connects to Tauri IPC and a startup event listener, so the dashboard has no new async work to set up. The hardest part of implementation is the four-point route synchronization (AppRoute union → VALID_APP_ROUTES → SIDEBAR_SECTIONS → ContentArea switch); TypeScript's exhaustive `never` check enforces correctness at compile time.

---

## Existing Code Structure

### Route System (4 synchronized touch points)

| File                                    | Symbol                    | Line  | Change needed                      |
| --------------------------------------- | ------------------------- | ----- | ---------------------------------- |
| `src/components/layout/Sidebar.tsx`     | `AppRoute` type union     | 12    | add `\| 'health'`                  |
| `src/App.tsx`                           | `VALID_APP_ROUTES` record | 14–21 | add `health: true`                 |
| `src/components/layout/Sidebar.tsx`     | `SIDEBAR_SECTIONS` array  | 32–51 | add new "Dashboards" section entry |
| `src/components/layout/ContentArea.tsx` | `renderPage()` switch     | 34–51 | add `case 'health':`               |

TypeScript will surface a compile error at `ContentArea.tsx:48` if the `AppRoute` union grows but the switch is not updated (`const _exhaustive: never = route`).

### Settings route exception

`settings` is **not** in `SIDEBAR_SECTIONS` — it lives in the sidebar footer as a standalone `SidebarTrigger` (Sidebar.tsx:143–150). All other routes follow the section model. The new `'health'` route should go in a section, not the footer.

---

## Implementation Patterns

### 1. Adding a sidebar route

```tsx
// Sidebar.tsx:12 — extend the union
export type AppRoute = 'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings' | 'health';

// Sidebar.tsx:53–60 — extend ROUTE_LABELS
const ROUTE_LABELS: Record<AppRoute, string> = {
  // ...existing...
  health: 'Health',
};

// Sidebar.tsx:32–51 — add a new section
const SIDEBAR_SECTIONS: SidebarSection[] = [
  // ...existing sections...
  {
    label: 'Dashboards',
    items: [{ route: 'health', label: 'Health', icon: HealthIcon }],
  },
];
```

```tsx
// App.tsx:14–21 — extend the valid routes map
const VALID_APP_ROUTES: Record<AppRoute, true> = {
  profiles: true,
  // ...
  health: true,
};
```

```tsx
// ContentArea.tsx:34–51 — add the case before default
function renderPage() {
  switch (route) {
    // ...existing cases...
    case 'health':
      return <HealthDashboardPage onNavigate={onNavigate} />;
    default: {
      const _exhaustive: never = route;
      return _exhaustive;
    }
  }
}
```

### 2. SVG icon pattern (SidebarIcons.tsx)

All icons share identical defaults applied via spread:

```tsx
const defaults: IconProps = {
  width: 20,
  height: 20,
  viewBox: '0 0 20 20',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.5,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
};

export function HealthIcon(props: IconProps) {
  return (
    <svg {...defaults} {...props}>
      {/* e.g. pulse/heartbeat or shield-check geometry */}
    </svg>
  );
}
```

Caller passes no props; the component receives `SVGProps<SVGSVGElement>` for overrides only.

### 3. Page banner + illustration pattern (PageBanner.tsx)

```tsx
// PageBanner.tsx:27–34 — illustration SVG defaults
const SVG_DEFAULTS: SVGProps<SVGSVGElement> = {
  viewBox: '0 0 200 120',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
};

export function HealthDashboardArt() {
  return <svg {...SVG_DEFAULTS}>{/* decorative geometry */}</svg>;
}
```

Usage in page component:

```tsx
<PageBanner
  eyebrow="Dashboards"
  title="Profile Health"
  copy="Aggregate status across all profiles — spot issues, re-validate, and navigate to fixes."
  illustration={<HealthDashboardArt />}
/>
```

`PageBanner` renders a `<header className="crosshook-page-banner">` with `.crosshook-page-banner__text` and `.crosshook-page-banner__art` sub-divs. The art div uses `aria-hidden="true"`.

### 4. Page component structure (CommunityPage thin-wrapper pattern)

```tsx
// CommunityPage.tsx — canonical thin page
export function CommunityPage() {
  const communityState = useCommunityProfiles({ profilesDirectoryPath });
  return (
    <>
      <PageBanner eyebrow="..." title="..." copy="..." illustration={<CommunityArt />} />
      <CommunityBrowser ... />
    </>
  );
}
```

`HealthDashboardPage` should follow the same fragment + banner + content body pattern. It receives `onNavigate?: (route: AppRoute) => void` from `ContentArea` (see `InstallPage` — `ContentArea.tsx:40`).

### 5. useProfileHealth hook — complete return surface

```ts
const {
  summary, // HealthCheckSummary | null
  loading, // boolean — true only during batch validate
  error, // string | null
  healthByName, // Record<string, ProfileHealthReport>
  cachedSnapshots, // Record<string, CachedHealthSnapshot>
  trendByName, // Record<string, TrendDirection>
  staleInfoByName, // Record<string, { isStale: boolean; daysAgo: number }>
  batchValidate, // (signal?: AbortSignal) => Promise<void>
  revalidateSingle, // (name: string) => Promise<void>
} = useProfileHealth();
```

Key behaviors:

- On mount, the hook listens for a `profile-health-batch-complete` Tauri event fired during startup. If the event has not arrived within 700 ms, it calls `batchValidate()` as a fallback (useProfileHealth.ts:178–183).
- `cachedSnapshots` is populated from `get_cached_health_snapshots` IPC — these are persisted snapshots from Phase D. They exist even before the live batch validate runs.
- `trendByName` is computed by comparing the live `summary.profiles[*].status` against `cachedSnapshots[*].status` via `computeTrend()` (useProfileHealth.ts:10–20).
- `staleInfoByName` marks snapshots older than 7 days (`STALE_THRESHOLD_DAYS`, useProfileHealth.ts:44).

**Each component that calls `useProfileHealth()` gets its own independent hook instance.** ProfilesPage and HealthDashboardPage will each run their own validate cycles. This is by design — ContentArea renders one tab at a time with `forceMount: true`, so both hooks stay mounted and in sync.

### 6. Health types

```ts
// health.ts — key types for the dashboard
interface HealthCheckSummary {
  profiles: ProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}

interface ProfileHealthReport {
  name: string;
  status: HealthStatus; // 'healthy' | 'stale' | 'broken'
  launch_method: string;
  issues: HealthIssue[];
  checked_at: string;
}

interface EnrichedProfileHealthReport extends ProfileHealthReport {
  metadata: ProfileHealthMetadata | null;
}

interface ProfileHealthMetadata {
  profile_id: string | null;
  last_success: string | null;
  failure_count_30d: number;
  total_launches: number;
  launcher_drift_state: string | null;
  is_community_import: boolean;
  is_favorite?: boolean;
}
```

**Enriched reports**: The backend command `batch_validate_profiles` returns `EnrichedHealthSummary` (enriched metadata). `useProfileHealth` currently types the result as `HealthCheckSummary` — the dashboard can cast via `report as EnrichedProfileHealthReport` (same pattern ProfilesPage uses at line 507).

### 7. HealthBadge component

```tsx
// Drop-in for any row — accepts status directly or a full report
<HealthBadge status="broken" trend="got_worse" metadata={metadata} tooltip="3 issues" />
<HealthBadge report={report} trend={trendByName[report.name]} />
```

Badge class chain: `crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--{rating}` where `STATUS_TO_RATING` maps `healthy→working`, `stale→partial`, `broken→broken`.

The badge is interactive when `onClick` is supplied — it gets `role="button"` and keyboard handling. Use this for the dashboard table rows to scroll-to / navigate-to a profile.

### 8. CollapsibleSection

```tsx
// Uncontrolled (default open)
<CollapsibleSection title="Summary Cards" className="crosshook-panel" defaultOpen>
  {/* content */}
</CollapsibleSection>

// Controlled
<CollapsibleSection title="Filters" open={filtersOpen} onToggle={setFiltersOpen} className="crosshook-panel">
  {/* content */}
</CollapsibleSection>

// With badge count in header
<CollapsibleSection title="Issues" meta={<span>{count} issues</span>} className="crosshook-panel">
  {/* content */}
</CollapsibleSection>
```

`meta` renders as `.crosshook-collapsible__meta` — suitable for counts, badges, or secondary actions. Uses native `<details>/<summary>` so it is keyboard and screen-reader accessible without extra ARIA.

### 9. Filter/search with useDeferredValue (CompatibilityViewer.tsx:110–140)

```tsx
const [searchQuery, setSearchQuery] = useState('');
const deferredQuery = useDeferredValue(searchQuery);

const filteredProfiles = useMemo(() => {
  const query = deferredQuery.trim().toLowerCase();
  if (!query) return summary?.profiles ?? [];
  return (summary?.profiles ?? []).filter((p) => p.name.toLowerCase().includes(query) || p.status.includes(query));
}, [deferredQuery, summary]);
```

`useDeferredValue` prevents the filter from blocking the keystroke — the UI shows stale results briefly, then updates when React has capacity. Do not debounce with `setTimeout` when `useDeferredValue` is available.

### 10. formatRelativeTime (ProfilesPage.tsx:22–36)

Currently a module-scoped private function in `ProfilesPage.tsx`. The feature spec calls for extracting this in P2. For P1, either inline a copy or leave it there and import later.

```ts
function formatRelativeTime(isoString: string): string {
  const then = new Date(isoString).getTime();
  const diffDays = Math.floor((Date.now() - then) / 86_400_000);
  if (diffDays <= 0) return 'today';
  if (diffDays === 1) return 'yesterday';
  if (diffDays < 7) return `${diffDays} days ago`;
  const weeks = Math.floor(diffDays / 7);
  if (diffDays < 30) return `${weeks} week${weeks !== 1 ? 's' : ''} ago`;
  const months = Math.floor(diffDays / 30);
  return `${months} month${months !== 1 ? 's' : ''} ago`;
}
```

### 11. Fix navigation — selectProfile via ProfileContext

```tsx
// In HealthDashboardPage
const { selectProfile } = useProfileContext();

// In a table row action button
<button
  onClick={() => {
    void selectProfile(profile.name);
    onNavigate?.('profiles');
  }}
>
  Fix
</button>;
```

`onNavigate` is passed from `ContentArea` via `ContentAreaProps.onNavigate` (ContentArea.tsx:14). `ProfileContext` must be consumed via `useProfileContext()` which throws if called outside `<ProfileProvider>` — that provider wraps the whole app shell (App.tsx:114), so it is always available.

### 12. Gamepad navigation

The content area already has `data-crosshook-focus-zone="content"` (ContentArea.tsx:30) applied to the `Tabs.Content` wrapper. D-pad Up/Down traverses `FOCUSABLE_SELECTOR` elements within that zone. To ensure table rows are reachable:

```tsx
<tr tabIndex={0} onKeyDown={(e) => { if (e.key === 'Enter') handleRowAction(); }}>
```

`summary` `<details>` elements are in `FOCUSABLE_SELECTOR` (useGamepadNav.ts:49). `CollapsibleSection` uses native `<details>`, so its `<summary>` is automatically focusable/navigable without any extra code.

### 13. CSS classes for the dashboard

| Class                                     | Use                                                     |
| ----------------------------------------- | ------------------------------------------------------- |
| `.crosshook-panel`                        | Secondary card with 20px padding, dark glass background |
| `.crosshook-card`                         | Primary card with 28px padding (use for summary cards)  |
| `.crosshook-heading-eyebrow`              | Uppercase accent label above title                      |
| `.crosshook-heading-title`                | Large page/section heading                              |
| `.crosshook-heading-copy`                 | Subtitle / description paragraph (muted)                |
| `.crosshook-help-text`                    | Small hint text (subtle, 0.92rem)                       |
| `.crosshook-muted`                        | Inline muted text (`--crosshook-color-text-subtle`)     |
| `.crosshook-danger`                       | Inline danger text (`--crosshook-color-danger`)         |
| `.crosshook-status-chip`                  | Pill badge (min 48px touch target, pill border-radius)  |
| `.crosshook-compatibility-badge--working` | Green badge                                             |
| `.crosshook-compatibility-badge--partial` | Yellow/amber badge                                      |
| `.crosshook-compatibility-badge--broken`  | Red badge                                               |
| `.crosshook-button`                       | Primary action button                                   |
| `.crosshook-button--secondary`            | Secondary action button                                 |
| `.crosshook-button--ghost`                | Ghost/text button                                       |
| `.crosshook-input`                        | Text input field                                        |
| `.crosshook-collapsible`                  | CollapsibleSection root                                 |

---

## Integration Points

### Files to modify

**`src/crosshook-native/src/components/layout/Sidebar.tsx`**

- Line 12: Add `| 'health'` to `AppRoute` union
- Line 53–60: Add `health: 'Health'` to `ROUTE_LABELS`
- Line 32–51 (after line 51): Insert a new "Dashboards" section with the `health` route
- Line 1–10 (imports): Add `HealthIcon` import from `../icons/SidebarIcons`

**`src/crosshook-native/src/App.tsx`**

- Line 14–21: Add `health: true` to `VALID_APP_ROUTES`

**`src/crosshook-native/src/components/layout/ContentArea.tsx`**

- Line 1–9 (imports): Add `HealthDashboardPage` import from `../pages/HealthDashboardPage`
- Line 34–51: Add `case 'health': return <HealthDashboardPage onNavigate={onNavigate} />;` before `default`

**`src/crosshook-native/src/components/icons/SidebarIcons.tsx`**

- Append `HealthIcon` export (20x20, stroke-based SVG)

**`src/crosshook-native/src/components/layout/PageBanner.tsx`**

- Append `HealthDashboardArt` export (200x120 viewBox SVG)

### Files to create

**`src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`**

- New page component — follows `CommunityPage` thin-wrapper structure
- Accepts `onNavigate?: (route: AppRoute) => void` from ContentArea

### Files to leave unmodified

- `useProfileHealth.ts` — already provides all needed state
- `types/health.ts` — already has all needed types
- `HealthBadge.tsx` — drop-in ready
- `CollapsibleSection.tsx` — drop-in ready
- All Rust/Tauri files — no backend changes required

---

## Code Conventions

- Component files: `PascalCase` (`HealthDashboardPage.tsx`)
- Function components: named export + default export both (see `CommunityPage.tsx`)
- Hooks called at component top level, no conditional hook calls
- All Tauri IPC through custom hooks — never call `invoke()` directly from a page component
- Inline styles used only for one-off overrides — structural layout uses CSS classes
- Boolean loading states come from the hook, not local `useState`
- `aria-live="polite"` for status updates (see rename toast pattern in ProfilesPage.tsx:393)
- `role="alert"` for error messages (see `crosshook-danger` usage in ProfilesPage.tsx:652)
- JSX string interpolation only for user-controlled values — never `dangerouslySetInnerHTML`

---

## Gotchas and Warnings

**The `settings` route is excluded from `SIDEBAR_SECTIONS`** — it renders in the sidebar footer only (Sidebar.tsx:143–150). Do not try to add `health` there; add it as a proper section entry.

**`AppRoute` is imported from `Sidebar.tsx` by `App.tsx` and `ContentArea.tsx`** — changes to the union cascade automatically. But both `VALID_APP_ROUTES` (App.tsx) and the switch (ContentArea.tsx) require manual updates; the TypeScript compiler will catch the switch omission (`never` guard at line 48) but not the `VALID_APP_ROUTES` omission (it's a record, not a switch).

**`useProfileHealth` fires a batch validate 700 ms after mount if no startup event arrives.** On the dashboard, `loading` will briefly be `true`. Render a loading skeleton or spinner, not an empty table.

**`summary` starts as `null` before the first validate completes.** Guard every access: `summary?.profiles ?? []`, `summary?.broken_count ?? 0`.

**Enriched metadata is available via type cast, not the type system.** The hook's `ProfileHealthReport` type does not include `metadata`, but the Tauri command actually returns `EnrichedProfileHealthReport`. Follow ProfilesPage.tsx:507: `const enriched = report as EnrichedProfileHealthReport; const metadata = enriched.metadata ?? null;` and guard all metadata fields with null checks.

**`formatRelativeTime` is private to `ProfilesPage.tsx`** — extract to `src/utils/time.ts` (or `src/utils/health.ts`) in P2 when the dashboard needs it. For P1, inline it or omit timestamps.

**`CollapsibleSection` in controlled mode syncs state via a `useEffect` that directly mutates `element.open`** (CollapsibleSection.tsx:39–43). Do not set `defaultOpen` and `open` simultaneously — pick one control model per section.

**The `FOCUSABLE_SELECTOR` includes `summary`** so `<CollapsibleSection>` elements are reachable via D-pad without any extra code. Table rows need explicit `tabIndex={0}` to enter the traversal list.

**No test framework exists for the frontend** — only Rust `cargo test` targets in `crosshook-core`. Frontend changes are manually verified.

---

## Task-Specific Guidance

### P1 — Route + Summary Cards + Profile List + Re-check + Fix nav

1. Add `HealthIcon` to SidebarIcons.tsx — keep the SVG minimal (20x20, stroke).
2. Add `HealthDashboardArt` to PageBanner.tsx — 200x120 decorative geometry, opacity 0.1–0.5.
3. Edit the 4 routing touch points atomically — the TypeScript exhaustive check will catch a missed ContentArea case immediately on `npm run dev`.
4. Create `HealthDashboardPage.tsx` with:
   - `<PageBanner>` header
   - `useProfileHealth()` at top level
   - Summary cards block: healthy/stale/broken counts + total (use `crosshook-card` grid)
   - Profile list in `<CollapsibleSection>` using `HealthBadge` per row
   - "Re-check All" button wired to `batchValidate()`
   - "Fix" button per broken/stale row: `selectProfile(name)` then `onNavigate?.('profiles')`
5. Guard `summary` null and `loading` states before rendering counts.

### P2 — Interactive table + secondary panels

- Extract `formatRelativeTime` to `src/utils/time.ts` and import from both pages.
- Add `useDeferredValue` filter following `CompatibilityViewer.tsx:110–140`.
- Add sort state (`useState<'name' | 'status' | 'issues'>`) with ascending/descending toggle.
- Render issue breakdown in a `<CollapsibleSection defaultOpen={false}>`.
- Use enriched `metadata` (cast pattern from ProfilesPage.tsx:507) for drift warnings and community import flags.

### P3 — Trends + gamepad Y + skeleton loading

- Trend arrows: use `trendByName[profile.name]` — already computed by the hook, render via `HealthBadge`'s built-in `trend` prop.
- Skeleton: replace null/loading states with placeholder `<div>` elements styled with animation (no external dependency needed — inline CSS `animation: pulse 1.5s ease infinite`).
- Gamepad Y (batchValidate on controller button): the hook's `batchValidate` is already a stable callback; wire it to a button that is reachable via D-pad traversal.
