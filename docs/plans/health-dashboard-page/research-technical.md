# Health Dashboard Page — Technical Architecture Specification

A dedicated read-only Profile Health Dashboard page that provides aggregate diagnostics for all profiles, consuming existing Phase A+B+D health infrastructure with no new backend work. Structured for incremental delivery across three implementation phases.

## Executive Summary

The Health Dashboard Page is a frontend-only addition that composes existing Tauri IPC commands (`batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`) and the `useProfileHealth` hook into a dedicated top-level route. It introduces one new page component, one new sidebar icon, one new page illustration, and routing changes across four existing files. The architecture follows established patterns: `PageBanner` header, `CollapsibleSection` panels, `HealthBadge` chips, and gamepad-navigable focusable elements.

The feature is split into three phases that each ship a self-contained, user-facing increment:

| Phase                           | Scope                                                                              | Deliverable                                 |
| ------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------- |
| **Phase 1: Scaffold + Summary** | Route wiring, page shell, summary cards, profile list with Fix navigation          | Working page with at-a-glance triage        |
| **Phase 2: Table + Patterns**   | Sortable/filterable table, row expansion, issue breakdown, single re-check, search | Full interactive diagnostic workbench       |
| **Phase 3: Trends + Polish**    | Trend arrows, recent failures, launcher drift, community health, gamepad Y-button  | Complete dashboard with history and context |

---

## Phased Delivery Plan

### Phase 1: Scaffold + Summary (MVP)

**Goal:** A routable page that shows aggregate health summary cards, a profile list with status badges, and "Fix" navigation to the profile editor. Answers the core question: "is anything broken, and where?"

**What ships:**

- Route wiring (`health` route in Sidebar, ContentArea, App.tsx)
- Sidebar icon (`HealthIcon`) and page illustration (`HealthDashboardArt`)
- `PageBanner` header
- Summary cards row: Total / Healthy / Stale / Broken counts with color coding
- Profile list as `<table>`: name, status badge (via `HealthBadge`), issue count — default order is broken-first
- Clickable rows: "Fix" navigates to profile editor (`selectProfile()` + `onNavigate('profiles')`)
- "Re-check All" button wired to `batchValidate()`
- Loading/error/empty states (zero profiles, all healthy)
- Cached snapshot data displayed instantly on mount (no blank-screen wait)
- Null metadata handling — page renders with core health data only when MetadataStore is unavailable

**Files created:**

- `src/components/pages/HealthDashboardPage.tsx`

**Files modified:**

- `src/components/layout/Sidebar.tsx` — add `'health'` to `AppRoute` union, add sidebar entry, add route label
- `src/components/layout/ContentArea.tsx` — import + render `HealthDashboardPage`
- `src/App.tsx` — add `health: true` to `VALID_APP_ROUTES`
- `src/components/icons/SidebarIcons.tsx` — add `HealthIcon`
- `src/components/layout/PageBanner.tsx` — add `HealthDashboardArt`

**Architectural decisions locked in Phase 1 (cannot change later without refactor):**

- Route name: `health`
- Sidebar section placement
- `HealthDashboardPage` component location and props interface
- Hook instance strategy (separate `useProfileHealth()` per page)
- `<table>` layout for profile list

**What is deliberately deferred:**

- Column sorting and filtering (Phase 2)
- Row expansion with inline issue details (Phase 2)
- Issue breakdown by category (Phase 2)
- Single-profile re-check (Phase 2)
- Trend arrows on summary cards and rows (Phase 3)
- Recent failures / launcher drift / community import panels (Phase 3)
- Y-button gamepad binding (Phase 3)

**Phase 1 component structure:**

```
HealthDashboardPage
  ├─ PageBanner
  ├─ SummaryCards (inline local component)
  ├─ CollapsibleSection "Re-check"
  │    └─ Re-check All button + last-checked timestamp
  └─ CollapsibleSection "All Profiles"
       └─ <table> (columns: name, status badge, issue count; rows clickable for Fix nav)
```

**"Fix" navigation mechanism (verified):**
The health dashboard calls `selectProfile(profileName)` from `useProfileContext()` followed by `onNavigate('profiles')`. This works because:

- `ProfileContext` wraps the entire app (`App.tsx` → `ProfileProvider` → `AppShell`)
- `selectProfile` is `loadProfile`, a callback that performs an IPC load and updates context state globally
- When `onNavigate('profiles')` fires, `ContentArea` renders `ProfilesPage` which sees the now-selected profile via the same context
- No additional pre-selection mechanism is needed — the existing `selectProfile()` IS the mechanism

**Design-for-extension notes for Phase 1:**

- The `HealthDashboardPage` props include `onNavigate?: (route: AppRoute) => void` — wired from ContentArea from day one and used immediately for Fix navigation.
- The profile list renders as `<table>` from day one (not a `<ul>` or card grid), so Phase 2 adds sort headers to the existing `<thead>` without restructuring.
- Summary cards are a local `SummaryCards` function component, not inlined JSX. Phase 3's trend arrows get added to this component without touching the rest of the page.
- The page's `useMemo` for derived data (profile list) takes the full `summary.profiles` array as input. Phase 2 adds filter/sort state that feeds into the same memo chain.

---

### Phase 2: Table + Patterns

**Goal:** The profile list becomes a fully interactive health table with sorting, filtering, search, row expansion for inline issue details, issue breakdown by category, and single-profile re-check. Answers the question: "which profiles need attention, what exactly is wrong, and are there systemic patterns?"

**What ships:**

- Sortable column headers: name, status, issue count, last successful launch, launch method, failure trend, favorites, source
- Status filter dropdown (All / Healthy / Stale / Broken)
- Text search with `useDeferredValue`
- Table toolbar row above the table
- Row expansion: clicking a row shows inline issue details (field, path, message, remediation) below the row
- Single-profile re-check button per row (calls `revalidateSingle(name)`)
- Issue Breakdown panel: aggregated counts by category (missing executables, missing Proton, inaccessible paths, etc.)
- Metadata columns populated from `ProfileHealthMetadata` (last_success, failure_count_30d, is_favorite, is_community_import, launch_method)
- Extract `formatRelativeTime` from `ProfilesPage.tsx` to `utils/format.ts` for shared use

**Files created:**

- `src/utils/format.ts` — shared `formatRelativeTime()` extracted from ProfilesPage

**Files modified:**

- `src/components/pages/HealthDashboardPage.tsx` — add sort/filter state, table toolbar, column headers, row expansion, issue breakdown panel, single re-check
- `src/components/pages/ProfilesPage.tsx` — import `formatRelativeTime` from `utils/format.ts` instead of local definition

**New page-local types added in Phase 2:**

```typescript
// Sort configuration for the health table
type SortField = 'name' | 'status' | 'issues' | 'last_success' | 'launch_method' | 'failures' | 'favorite';
type SortDirection = 'asc' | 'desc';
interface TableSort {
  field: SortField;
  direction: SortDirection;
}
type StatusFilter = 'all' | HealthStatus;

// Issue category for breakdown aggregation
type IssueCategory =
  | 'missing_executable'
  | 'missing_trainer'
  | 'missing_dll'
  | 'missing_proton'
  | 'missing_prefix'
  | 'missing_compatdata'
  | 'inaccessible_path'
  | 'optional_path'
  | 'other';

interface IssueCategoryCount {
  category: IssueCategory;
  label: string;
  count: number;
  severity: HealthIssueSeverity;
}
```

**Phase 2 component structure (additions in bold):**

```
HealthDashboardPage
  ├─ PageBanner
  ├─ SummaryCards
  ├─ CollapsibleSection "Re-check"
  ├─ **CollapsibleSection "Issue Breakdown"**
  │    └─ **IssueCategoryBreakdown (aggregated by field category)**
  └─ CollapsibleSection "All Profiles"
       ├─ **TableToolbar (search input, status filter, sort indicator)**
       └─ <table>
            ├─ **<thead> with sortable column buttons**
            └─ <tbody>
                 ├─ rows (now with all 8 columns, clickable for Fix + expandable)
                 └─ **expanded detail row (issues list + single re-check button)**
```

**Row expansion pattern:**
Expanding a row inserts an additional `<tr>` below the profile row with a `<td colSpan={columnCount}>` containing the issue details. This is toggled via local state (`expandedProfile: string | null`). The expansion shows:

- Issue list (field, path, message, remediation) — matching the existing pattern in ProfilesPage lines 548-559
- Metadata context: last success, total launches, failure count (if metadata available)
- Single re-check button for that profile

**Design-for-extension notes for Phase 2:**

- The `CollapsibleSection` wrappers can hold additional sections below them in Phase 3 without restructuring.
- Sort and filter state lives in `useState` at the page level. Phase 3's additional panels don't interact with this state — they compute their own derived data from `summary.profiles`.
- The `categorizeIssue()` function and `IssueCategoryCount` type are defined in Phase 2 for the issue breakdown panel. They do not need to be modified in Phase 3.

---

### Phase 3: Trends + Polish

**Goal:** Add historical context (trend arrows, failure history, launcher drift, community import annotation) and gamepad polish to complete the full dashboard spec.

**What ships:**

- Trend arrows on summary cards (comparing aggregate counts vs. cached snapshots)
- Trend arrows on table rows (per-profile `got_worse`/`got_better` via `trendByName`)
- Recent Failures panel: profiles with `failure_count_30d > 0`, sorted by failure count descending
- Launcher Drift Summary panel: profiles where `launcher_drift_state` is not `aligned` or `null`
- Community Import Health panel: profiles where `is_community_import === true` and status is not `healthy`, with the annotation "paths may need adjustment for your system"
- Favorites visual flag (star indicator) in table rows where `is_favorite === true`
- Y-button gamepad binding for re-check (page-local `useEffect`)
- Table CSS polish and responsive breakpoints

**Files modified:**

- `src/components/pages/HealthDashboardPage.tsx` — add three diagnostic panels, trend arrows in summary cards and table rows, favorites star, gamepad Y-button handler
- `src/styles/theme.css` — add health dashboard table styles if inline styles prove insufficient

**Phase 3 component structure (additions in bold):**

```
HealthDashboardPage
  ├─ PageBanner
  ├─ SummaryCards **(now with trend arrows)**
  ├─ CollapsibleSection "Re-check"
  ├─ CollapsibleSection "Issue Breakdown"
  ├─ CollapsibleSection "All Profiles" (table **(now with trend + favorite columns)**)
  ├─ **CollapsibleSection "Recent Failures" (defaultOpen={false})**
  │    └─ **RecentFailuresPanel**
  ├─ **CollapsibleSection "Launcher Drift" (defaultOpen={false})**
  │    └─ **LauncherDriftSummary**
  └─ **CollapsibleSection "Community Import Health" (defaultOpen={false})**
       └─ **CommunityImportHealth**
```

---

## Architecture Design

### Final Component Hierarchy (all phases)

```
App.tsx
  └─ AppShell
       └─ ContentArea (route="health")
            └─ HealthDashboardPage (props: { onNavigate? })
                 ├─ PageBanner (eyebrow="Diagnostics", illustration=HealthDashboardArt)
                 ├─ SummaryCards (counts + P3 trend arrows)                        [P1 base, P3 trends]
                 ├─ CollapsibleSection "Re-check"                                  [P1]
                 │    └─ Re-check All button + last-checked timestamp
                 ├─ CollapsibleSection "Issue Breakdown"                            [P2]
                 │    └─ IssueCategoryBreakdown
                 ├─ CollapsibleSection "All Profiles"                               [P1 base, P2 full]
                 │    ├─ TableToolbar (search, filter, sort)                        [P2]
                 │    └─ HealthTable                                                [P1 basic, P2 full, P3 trends]
                 │         ├─ <thead> (P1: 3 cols, P2: 8 cols sortable, P3: +trend/fav)
                 │         └─ <tbody>
                 │              ├─ profile rows (P1: clickable, P2: expandable)
                 │              └─ expanded detail rows (P2: issues + re-check)
                 ├─ CollapsibleSection "Recent Failures" (defaultOpen={false})      [P3]
                 │    └─ RecentFailuresPanel
                 ├─ CollapsibleSection "Launcher Drift" (defaultOpen={false})       [P3]
                 │    └─ LauncherDriftSummary
                 └─ CollapsibleSection "Community Import Health" (defaultOpen={false}) [P3]
                      └─ CommunityImportHealth
```

### New Components (all in `HealthDashboardPage.tsx`)

The page should be implemented as a single file with internal sub-components (local function components), following the pattern of `ProfilesPage.tsx` (715 lines). Each phase adds sub-components to this file. Extract to separate files only if the page exceeds ~800 lines after Phase 3.

### Integration Points

```
useProfileHealth() ─── batch_validate_profiles ───→ EnrichedHealthSummary
                   ├── get_cached_health_snapshots ──→ CachedHealthSnapshot[]
                   └── profile-health-batch-complete (Tauri event, startup)

useProfileContext() ─── selectProfile() ───→ sets active profile         [P1]
                    └── profiles[] ───→ profile name list

ContentArea ─── onNavigate('profiles') ───→ switches route to ProfilesPage  [P1]
```

---

## Data Models

### Existing Rust Types (IPC boundary)

**`EnrichedHealthSummary`** — returned by `batch_validate_profiles`

```rust
// src-tauri/src/commands/health.rs:33-41
pub struct EnrichedHealthSummary {
    pub profiles: Vec<EnrichedProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,          // ISO 8601
}
```

**`EnrichedProfileHealthReport`** — per-profile entry

```rust
// src-tauri/src/commands/health.rs:26-31
pub struct EnrichedProfileHealthReport {
    #[serde(flatten)]
    pub core: ProfileHealthReport,     // name, status, launch_method, issues[], checked_at
    pub metadata: Option<ProfileHealthMetadata>,
}
```

**`ProfileHealthMetadata`** — enrichment from MetadataStore (SQLite)

```rust
// src-tauri/src/commands/health.rs:15-24
pub struct ProfileHealthMetadata {
    pub profile_id: Option<String>,
    pub last_success: Option<String>,          // ISO 8601 or null
    pub failure_count_30d: i64,                // defaults to 0
    pub total_launches: i64,                   // defaults to 0
    pub launcher_drift_state: Option<DriftState>, // aligned|missing|moved|stale|unknown or null
    pub is_community_import: bool,
    pub is_favorite: bool,
}
```

**`DriftState`** — launcher drift enum

```rust
// crates/crosshook-core/src/metadata/models.rs:124-130
pub enum DriftState { Unknown, Aligned, Missing, Moved, Stale }
// Serializes as snake_case strings
```

**`HealthIssue`** — individual path validation issue

```rust
// crates/crosshook-core/src/profile/health.rs:30-37
pub struct HealthIssue {
    pub field: String,       // e.g. "game.executable_path", "steam.compatdata_path"
    pub path: String,        // sanitized (~ replaces home dir)
    pub message: String,     // NOTE: contains unsanitized paths — see A-02 in security research
    pub remediation: String,
    pub severity: HealthIssueSeverity, // error | warning | info
}
```

### Existing TypeScript Interfaces

All defined in `src/types/health.ts` — no changes needed in any phase:

```typescript
type HealthStatus = 'healthy' | 'stale' | 'broken';
type HealthIssueSeverity = 'error' | 'warning' | 'info';

interface ProfileHealthReport {
  name: string;
  status: HealthStatus;
  launch_method: string;
  issues: HealthIssue[];
  checked_at: string;
}

interface ProfileHealthMetadata {
  profile_id: string | null;
  last_success: string | null;
  failure_count_30d: number;
  total_launches: number;
  launcher_drift_state: string | null; // "aligned"|"missing"|"moved"|"stale"|"unknown"
  is_community_import: boolean;
  is_favorite?: boolean;
}

interface EnrichedProfileHealthReport extends ProfileHealthReport {
  metadata: ProfileHealthMetadata | null;
}

interface EnrichedHealthSummary {
  profiles: EnrichedProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}

interface CachedHealthSnapshot {
  profile_id: string;
  profile_name: string;
  status: HealthStatus;
  issue_count: number;
  checked_at: string;
}
```

### New TypeScript Types by Phase (all page-local, not exported)

**Phase 2:**

```typescript
// Sort configuration
type SortField = 'name' | 'status' | 'issues' | 'last_success' | 'launch_method' | 'failures' | 'favorite';
type SortDirection = 'asc' | 'desc';
interface TableSort {
  field: SortField;
  direction: SortDirection;
}
type StatusFilter = 'all' | HealthStatus;

// Issue category for breakdown aggregation
type IssueCategory =
  | 'missing_executable'
  | 'missing_trainer'
  | 'missing_dll'
  | 'missing_proton'
  | 'missing_prefix'
  | 'missing_compatdata'
  | 'inaccessible_path'
  | 'optional_path'
  | 'other';

interface IssueCategoryCount {
  category: IssueCategory;
  label: string;
  count: number;
  severity: HealthIssueSeverity;
}
```

### Derived State Shape (from useProfileHealth)

```typescript
// Hook return type — already exists at hooks/useProfileHealth.ts:236-247
{
  summary: HealthCheckSummary | null; // see type mismatch note below
  loading: boolean;
  error: string | null;
  healthByName: Record<string, ProfileHealthReport>;
  cachedSnapshots: Record<string, CachedHealthSnapshot>;
  trendByName: Record<string, TrendDirection>;
  staleInfoByName: Record<string, { isStale: boolean; daysAgo: number }>;
  batchValidate: (signal?: AbortSignal) => Promise<void>;
  revalidateSingle: (name: string) => Promise<void>;
}
```

**Important type mismatch note:** The `useProfileHealth` hook currently types `summary` as `HealthCheckSummary | null` and invokes `batch_validate_profiles` expecting `HealthCheckSummary` (line 127) and `get_profile_health` expecting `ProfileHealthReport` (line 143). However, the Rust IPC actually returns `EnrichedHealthSummary` and `EnrichedProfileHealthReport` respectively — these include the `metadata` field via `#[serde(flatten)]`. At runtime the metadata is present on each profile object, but TypeScript doesn't know about it.

**Recommended resolution (pre-Phase 1):** Update the hook's `invoke<>` generics and state types to use `EnrichedHealthSummary` / `EnrichedProfileHealthReport`. This is a type-only change with no runtime impact — the data already flows correctly. It unblocks the dashboard from needing casts. The existing `ProfilesPage` consumer already casts to `EnrichedProfileHealthReport` at line 507, confirming the runtime shape matches the enriched type.

---

## API Design

### Tauri IPC Commands Consumed (no new commands, all phases)

| Command                       | Params             | Returns                       | Phase Used           |
| ----------------------------- | ------------------ | ----------------------------- | -------------------- |
| `batch_validate_profiles`     | none               | `EnrichedHealthSummary`       | P1                   |
| `get_profile_health`          | `{ name: string }` | `EnrichedProfileHealthReport` | P2 (single re-check) |
| `get_cached_health_snapshots` | none               | `CachedHealthSnapshot[]`      | P1 (via hook)        |

### Tauri Events Consumed

| Event                           | Payload                 | Phase Used    |
| ------------------------------- | ----------------------- | ------------- |
| `profile-health-batch-complete` | `EnrichedHealthSummary` | P1 (via hook) |

### Component Props/Interfaces

**HealthDashboardPage** (top-level page, established in Phase 1)

```typescript
interface HealthDashboardPageProps {
  onNavigate?: (route: AppRoute) => void; // used from P1 for Fix navigation
}
```

---

## System Constraints

### Performance (50+ profiles)

1. **Batch IPC call**: `batch_validate_profiles` does filesystem I/O for every path in every profile. With 50 profiles averaging 4-5 path checks each, this is ~200-250 `fs::metadata` calls. On SSD this completes in <100ms; on Steam Deck's eMMC storage it may take ~500ms.

2. **Client-side aggregation** (Phase 2): Issue category breakdown, filtering, and sorting should use `useMemo` with dependency on `summary.profiles`. The `useDeferredValue` pattern from `CompatibilityViewer` should be used for search filter input.

3. **Table rendering**: With 50-200 profiles, a plain HTML table with CSS `max-height` + `overflow-y: auto` is sufficient. Virtual scrolling (react-window) is unnecessary at this scale and would complicate gamepad navigation.

4. **Re-check rate limiting**: The "Re-check All" button should be disabled during `loading` state (already provided by the hook). No additional debouncing needed since the button is disabled while a check is in-flight.

### Gamepad Navigation (Steam Deck)

1. **Table rows** (Phase 1): Each table row should be a `<tr>` with `tabIndex={0}` so D-pad up/down navigates rows. Enter/A-button on a row triggers Fix navigation (Phase 1).

2. **Sort headers** (Phase 2): `<th>` elements should contain `<button>` elements for column sort toggling.

3. **Re-check All button** (Phase 1): Standard `<button>` — naturally focusable.

4. **Y-button re-check** (Phase 3): The `useGamepadNav` hook currently maps button 0 (A) to confirm and button 1 (B) to back. Button 3 (Y) is not mapped. Phase 3 adds a page-local `useEffect` with `navigator.getGamepads()` polling for the Y-button, scoped to this page only (avoids modifying the shared hook).

5. **Focus zones**: The page content is within `data-crosshook-focus-zone="content"` inherited from `ContentArea`. No additional focus zone setup needed.

### Tab Routing (Phase 1)

Changes required in four files to add the `health` route:

1. **`Sidebar.tsx:12`** — Extend `AppRoute` union:

   ```typescript
   export type AppRoute = 'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'health' | 'settings';
   ```

2. **`Sidebar.tsx:32-51`** — Add sidebar section:

   ```typescript
   {
     label: 'Diagnostics',
     items: [{ route: 'health', label: 'Health', icon: HealthIcon }],
   },
   ```

   And add to `ROUTE_LABELS`:

   ```typescript
   health: 'Health',
   ```

3. **`App.tsx:14-21`** — Add to `VALID_APP_ROUTES`:

   ```typescript
   health: true,
   ```

4. **`ContentArea.tsx:34-51`** — Add case:

   ```typescript
   case 'health':
     return <HealthDashboardPage onNavigate={onNavigate} />;
   ```

---

## Codebase Changes by Phase

### Phase 1

| Action | File                                           | Change                                                     |
| ------ | ---------------------------------------------- | ---------------------------------------------------------- |
| Create | `src/components/pages/HealthDashboardPage.tsx` | Page shell with summary cards, basic table, Fix navigation |
| Modify | `src/components/layout/Sidebar.tsx`            | Add `'health'` to `AppRoute`, sidebar entry, route label   |
| Modify | `src/components/layout/ContentArea.tsx`        | Import + render `HealthDashboardPage`                      |
| Modify | `src/App.tsx`                                  | Add `health: true` to `VALID_APP_ROUTES`                   |
| Modify | `src/components/icons/SidebarIcons.tsx`        | Add `HealthIcon`                                           |
| Modify | `src/components/layout/PageBanner.tsx`         | Add `HealthDashboardArt`                                   |

### Phase 2

| Action | File                                           | Change                                                                                                     |
| ------ | ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Modify | `src/components/pages/HealthDashboardPage.tsx` | Add sort/filter state, table toolbar, all 8 columns, row expansion, issue breakdown panel, single re-check |
| Create | `src/utils/format.ts`                          | Extract `formatRelativeTime` from ProfilesPage                                                             |
| Modify | `src/components/pages/ProfilesPage.tsx`        | Import `formatRelativeTime` from `utils/format.ts`                                                         |

### Phase 3

| Action | File                                           | Change                                                                      |
| ------ | ---------------------------------------------- | --------------------------------------------------------------------------- |
| Modify | `src/components/pages/HealthDashboardPage.tsx` | Add three diagnostic panels, trend arrows, favorites star, Y-button handler |
| Modify | `src/styles/theme.css`                         | Add health dashboard table styles (if inline styles prove insufficient)     |

### Pre-Phase 1 (Optional, Recommended)

| Action | File                            | Change                                                                                    |
| ------ | ------------------------------- | ----------------------------------------------------------------------------------------- |
| Modify | `src/hooks/useProfileHealth.ts` | Update `invoke<>` generics to use `EnrichedHealthSummary` / `EnrichedProfileHealthReport` |

### No Changes Needed (any phase)

- **No Rust changes** — all IPC commands exist
- **No new hooks** — `useProfileHealth` provides everything needed
- **No new TypeScript type files** — page-local types stay in the page component
- **No new CSS files** (P1/P2) — reuse existing `crosshook-panel`, `crosshook-status-chip`, `crosshook-compatibility-badge--{rating}`, `crosshook-collapsible`, `crosshook-page-banner` classes

---

## Technical Decisions

### Decision 1: Hook Instance Strategy

| Option                                        | Pros                                           | Cons                                                               |
| --------------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------ |
| **A: Separate `useProfileHealth()` per page** | Simple, no refactoring, phases are independent | Duplicate IPC calls if both pages were mounted simultaneously      |
| **B: Lift to context provider**               | Single data source                             | Requires refactoring ProfilesPage, adds complexity, couples phases |

**Recommendation: Option A.** `ContentArea` renders only the active route via a `switch` statement (lines 34-51). Even though `Tabs.Content` has `forceMount`, the `renderPage()` function only returns one page component at a time. The non-active page is not rendered, so its hooks don't run. Separate instances are safe and keep phases independent — Phase 1 doesn't touch ProfilesPage at all.

### Decision 2: Table vs. Card Layout

| Option            | Pros                                                                    | Cons                                          |
| ----------------- | ----------------------------------------------------------------------- | --------------------------------------------- |
| **A: HTML table** | Data-dense, supports sorting, familiar, progressive enhancement natural | Requires table-specific CSS                   |
| **B: Card grid**  | Matches CompatibilityViewer pattern                                     | Too spread out for 8 columns and 50+ profiles |

**Recommendation: Option A.** The spec requires specific sortable columns (name, status, issue count, last success, launch method, failure trend, favorites, source). A table is the natural fit. Phase 1 starts with 3 columns in a `<table>`, Phase 2 adds the remaining columns and sort headers to the same `<thead>` — no layout restructuring needed.

### Decision 3: Sidebar Section Placement

| Option                             | Pros                                                  | Cons                             |
| ---------------------------------- | ----------------------------------------------------- | -------------------------------- |
| **A: New "Diagnostics" section**   | Clean separation, discoverable, room for future tools | One more sidebar group           |
| **B: Under "Game" section**        | Fewer sections, directly related to profiles          | Crowds the Game section          |
| **C: Footer (alongside Settings)** | Parallel to Settings                                  | Health is not a settings concern |

**Recommendation: Option A.** Place between "Setup" and "Community" sections. The "Diagnostics" label clearly communicates the page's purpose and leaves room for future diagnostic tools.

### Decision 4: Issue Category Aggregation (Phase 2)

Issue `field` values from the Rust backend map to display categories:

| Field Pattern                                           | Category             | Display Label         |
| ------------------------------------------------------- | -------------------- | --------------------- |
| `game.executable_path`                                  | `missing_executable` | Missing executables   |
| `trainer.path`                                          | `missing_trainer`    | Missing trainers      |
| `injection.dll_paths[*]`                                | `missing_dll`        | Missing DLLs          |
| `steam.proton_path`, `runtime.proton_path`              | `missing_proton`     | Missing Proton paths  |
| `runtime.prefix_path`                                   | `missing_prefix`     | Missing Proton prefix |
| `steam.compatdata_path`                                 | `missing_compatdata` | Missing compatdata    |
| Any field with "permission denied" message              | `inaccessible_path`  | Inaccessible paths    |
| `steam.launcher.icon_path`, `runtime.working_directory` | `optional_path`      | Optional path issues  |
| Anything else                                           | `other`              | Other issues          |

This mapping is done client-side via a `categorizeIssue(issue: HealthIssue): IssueCategory` function using the `field` string and `severity` level.

### Decision 5: "Fix" Navigation (Phase 1)

When a user clicks a profile row in the health table:

1. Call `selectProfile(profileName)` from `useProfileContext()` to load the profile
2. Call `onNavigate('profiles')` to switch to the Profiles page

This follows the `InstallPage` pattern where `onNavigate` is received from `ContentArea` (line 40). `selectProfile` triggers an async IPC load that updates `ProfileContext` state globally. Since `ProfileContext` wraps the entire app tree and persists across route changes, the profile is loaded by the time `ProfilesPage` renders.

### Decision 6: Single File vs. Multi-File Component

| Option                                         | Pros                                         | Cons                                |
| ---------------------------------------------- | -------------------------------------------- | ----------------------------------- |
| **A: Single file, local sub-components**       | Matches ProfilesPage pattern, simple imports | May exceed 800 lines after Phase 3  |
| **B: Directory with extracted sub-components** | Better separation for large component        | Over-engineering for initial phases |

**Recommendation: Start with Option A.** Phase 1 will be ~200-300 lines. Phase 2 adds ~200 lines. Phase 3 adds ~200 lines. Total should be ~600-700 lines, within the single-file comfort zone. If the file exceeds ~800 lines, extract Phase 3 panel components into `src/components/health/` as a follow-up.

---

## Phase Boundary Contracts

These are the stable interfaces that Phase 1 establishes for later phases to build on:

### Page Props (stable from Phase 1)

```typescript
interface HealthDashboardPageProps {
  onNavigate?: (route: AppRoute) => void;
}
```

### Hook Data Shape (stable, already exists)

```typescript
// From useProfileHealth — consumed by phase:
// P1: summary, loading, error, cachedSnapshots, batchValidate
// P2: healthByName, revalidateSingle
// P3: trendByName, staleInfoByName
```

### DOM Structure Contract

Phase 1 establishes the page's DOM skeleton. Later phases add `CollapsibleSection` elements into the existing `div style={{ display: 'grid', gap: 24 }}` container (matching the layout pattern in `LaunchPage.tsx` and `ProfilesPage.tsx`). The table's `<thead>` gains columns in Phase 2. Row expansion adds `<tr>` elements in Phase 2. No DOM restructuring occurs between phases.

### CSS Class Reuse

All phases reuse existing CSS classes. No phase introduces custom CSS that later phases depend on. If Phase 3 needs table-specific styles, they go in `theme.css` as `.crosshook-health-table` and descendants — isolated from existing classes.

---

## Cross-Reference: Business Rules Alignment

This technical spec aligns with the business research at `docs/plans/health-dashboard-page/research-business.md`. Key correspondences:

| Business Rule                                      | Phase | Technical Implementation                                                                       |
| -------------------------------------------------- | ----- | ---------------------------------------------------------------------------------------------- |
| BR-01: Read-only surface                           | All   | No mutation IPC calls; Fix is navigation only                                                  |
| BR-03: Status hierarchy (Broken > Stale > Healthy) | P1    | Default sort order in profile list; color coding via `crosshook-compatibility-badge--{rating}` |
| BR-04: Issue severity classification               | P2    | Row expansion shows severity via `HealthIssueSeverity`; issue breakdown groups by severity     |
| BR-05: Failure trend window (30 days)              | P3    | `failure_count_30d` from metadata; Recent Failures panel threshold: `> 0`                      |
| BR-06: Trend direction                             | P3    | `trendByName` from hook; arrows render only for `got_worse`/`got_better`                       |
| BR-08: Launch method scoping                       | P2    | Launch method column; issue categories are field-aware                                         |
| BR-09: Community import annotation                 | P3    | Community Import Health panel with "paths may need adjustment" note                            |
| BR-10: Launcher drift states                       | P3    | Launcher Drift panel shows `missing`/`moved`/`stale` profiles                                  |
| BR-12: Metadata may be absent                      | P1    | Null-safe rendering from day one; metadata-dependent columns show "N/A"                        |
| BR-13: Re-Check All                                | P1    | Button wired to `batchValidate()`, disabled during loading                                     |
| BR-14: Single profile re-check                     | P2    | Per-row button calls `revalidateSingle(name)`                                                  |
| EC-01: Zero profiles                               | P1    | Empty state message                                                                            |
| EC-02: All healthy                                 | P1    | Positive confirmation state                                                                    |
| EC-04: Profile load failure                        | P1    | Renders as Broken row with error issue                                                         |
| EC-05: Enumeration failure                         | P1    | Detect `<unknown>` sentinel, show system error state                                           |
| EC-06: Stale snapshot                              | P1    | Cached data renders immediately with "checking..." indicator                                   |

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx` — Route validation map (line 14), AppShell with Tabs.Root
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/ContentArea.tsx` — Route-to-page mapping (lines 34-51), onNavigate prop pattern (line 40)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/Sidebar.tsx` — AppRoute type (line 12), sidebar sections (lines 32-51), route labels (lines 53-60)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/icons/SidebarIcons.tsx` — SVG icon pattern (20x20 viewBox, stroke-based)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/PageBanner.tsx` — Banner + illustration pattern (200x120 viewBox SVGs)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfileHealth.ts` — Hook providing all health data and actions
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/health.ts` — All TypeScript health types
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/health.ts` — `countProfileStatuses` utility
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/HealthBadge.tsx` — Reusable health status badge with trend/failure indicators
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ui/CollapsibleSection.tsx` — Collapsible details/summary component
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/health.rs` — All Tauri health commands and enrichment logic
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/health.rs` — Core health validation (field checks, status classification)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — Command registration (lines 166-168 for health commands)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/ProfilesPage.tsx` — Existing health badge + issue display pattern (lines 501-563), `formatRelativeTime` at line 22 (to extract in Phase 2)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/CompatibilityPage.tsx` — Thin page wrapper pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CompatibilityViewer.tsx` — Filter/search pattern with useDeferredValue
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGamepadNav.ts` — Gamepad/keyboard navigation system
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/context/ProfileContext.tsx` — `selectProfile()` mechanism for Fix navigation (wraps entire app)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css` — CSS custom properties
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css` — Panel, badge, status-chip CSS classes

---

## Open Questions

1. **Hook type accuracy (Pre-Phase 1):** `useProfileHealth` types its `summary` as `HealthCheckSummary` (non-enriched), but the IPC actually returns `EnrichedHealthSummary`. Should the hook's types be updated before Phase 1 (cleaner, small blast radius — ProfilesPage already casts at line 507), or should Phase 1 cast at the consumption point? Updating the hook types is recommended as a pre-Phase 1 prep task.

2. **Sidebar section naming:** "Diagnostics" is proposed. Alternatives: "Health", "Status", "Monitor". Business research suggests "Game" section (alongside Profiles/Launch) as an alternative. The choice should align with the app's vocabulary — "Health" as the section label is most intuitive but "Diagnostics" leaves room for future tools.

3. **Empty state:** What should the dashboard show when there are 0 profiles? The batch validation returns an empty summary. Phase 1 should show a helpful message directing the user to create their first profile (or navigate to the Profiles page).

4. **Table row ARIA semantics (Phase 2):** For row expansion + gamepad navigation, the table needs careful ARIA. Options: `role="grid"` with `role="row"` + `role="gridcell"`, or standard `<table>` semantics with `tabIndex={0}` on `<tr>`. Standard table semantics with `tabIndex` is simpler and sufficient for the gamepad system's focusable-element scanning.

5. **Phase 3 — Y-button scope:** Should the Y-button binding be page-local (duplicates some gamepad polling logic) or added to the shared `useGamepadNav` hook (more elegant but affects all pages)? Recommendation: page-local for Phase 3, with a follow-up to generalize if other pages want custom button bindings.

6. **Security note (A-02):** `HealthIssue.message` contains unsanitized home directory paths. The security research recommends extending `sanitize_issues()` in `commands/health.rs` to also sanitize `issue.message`. This is a one-line backend fix that could be done as a pre-Phase 1 task alongside the hook type update.
