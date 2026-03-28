# Feature Spec: Health Dashboard Page

## Executive Summary

The Health Dashboard is a dedicated read-only diagnostics page added as a top-level tab in CrossHook, providing aggregate visibility into all profile health status — summary cards (total/healthy/stale/broken), an issue breakdown by category, a sortable/filterable profile health table, recent failures panel, launcher drift summary, and community import health. It enables users managing 10–50+ profiles to triage at a glance, identify systemic issues, and navigate to the profile editor for remediation. The feature is entirely frontend — all backend data exists from Phase A+B+D health implementation via three Tauri IPC commands and the `useProfileHealth` hook. Implementation introduces one new page component and touches five existing files for routing, with no new dependencies. The primary risks are a frontend type mismatch (`HealthCheckSummary` vs. `EnrichedHealthSummary` in the hook) and cross-page "Fix" navigation timing.

## External Dependencies

### APIs and Services

**None.** This is a purely frontend feature consuming existing local Tauri IPC commands. No external APIs, no network calls, no auth, no rate limits.

### Tauri IPC Commands (Existing)

| Command                       | Returns                       | Used By                                                      |
| ----------------------------- | ----------------------------- | ------------------------------------------------------------ |
| `batch_validate_profiles`     | `EnrichedHealthSummary`       | `useProfileHealth` hook — filesystem I/O + SQLite enrichment |
| `get_profile_health`          | `EnrichedProfileHealthReport` | `useProfileHealth` — single-profile re-check                 |
| `get_cached_health_snapshots` | `CachedHealthSnapshot[]`      | `useProfileHealth` — instant cached display on mount         |

### Tauri Events (Existing)

| Event                           | Payload                 | Purpose                                           |
| ------------------------------- | ----------------------- | ------------------------------------------------- |
| `profile-health-batch-complete` | `EnrichedHealthSummary` | Startup scan result emitted 500ms after app start |

### Libraries and SDKs

| Library | Version | Purpose                    | Installation |
| ------- | ------- | -------------------------- | ------------ |
| None    | —       | No new dependencies for v1 | —            |

**Rationale:** The hand-rolled table approach using `useMemo` sort/filter is sufficient for the expected 5–30 profile rows and is consistent with the existing `CompatibilityViewer` pattern. All UI components (stat cards, trend arrows, badges) are built with existing CSS classes and the `HealthBadge` component. If multi-column sort or pagination becomes a real need, `@tanstack/react-table` v8 is pre-approved as a future upgrade (0 CVEs, 2 packages, headless).

### External Documentation

- [WAI-ARIA APG — Grid Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/grid/): ARIA table navigation patterns
- [WAI-ARIA APG — Sortable Table](https://www.w3.org/WAI/ARIA/apg/patterns/table/examples/sortable-table/): Sort indicator patterns

## Business Requirements

### User Stories

**Primary User: CrossHook profile manager (5–50 profiles)**

#### Phase 1 — MVP: Triage at a Glance

- **US-1.1** As any user, I want to open a Health tab and immediately see color-coded summary counts (total/healthy/stale/broken) so I can assess fleet health in one glance
- **US-1.2** As any user, I want to see a flat list of all profiles with their health status badge so I know which have problems
- **US-1.3** As any user, I want a "Re-check All" button that rescans all profiles and updates the view
- **US-1.4** As a Steam Deck user, I want to reach the Health tab via L1/R1 and navigate the list with D-pad
- **US-1.5** As any user, I want a "Fix" action that navigates directly to that profile in the editor
- **US-1.6** As any user, I want meaningful empty states (no profiles, all healthy) clearly communicated

#### Phase 2 — Power-User: Filtering and Pattern Identification

- **US-2.1** As a user with many profiles, I want to sort by status/name/issue count to prioritize fixes
- **US-2.2** As a user, I want to filter the table to show only broken or stale profiles
- **US-2.3** As a user, I want to expand a row to see specific issues (field, path, message, remediation)
- **US-2.4** As a user, I want single-profile re-check without triggering a full re-scan
- **US-2.5** As a user, I want an issue breakdown panel categorizing problems by type across all profiles

#### Phase 3 — Polish: Trends, History, and Contextual Signals

- **US-3.1** As a returning user, I want trend arrows (got worse/got better) based on cached snapshot comparison
- **US-3.2** As a user, I want a "Recent Failures" panel showing profiles with launch failures in the last 30 days
- **US-3.3** As a user, I want launcher drift indicators per profile (missing/moved/stale)
- **US-3.4** As a user, I want broken community imports annotated with "paths may need adjustment"
- **US-3.5** As a user, I want favorited profiles visually flagged with a star
- **US-3.6** As a Steam Deck user, I want Y button mapped to Re-check All

### Business Rules

**BR-01: Read-Only Surface** (P1) — The dashboard never modifies profiles. "Fix" actions navigate to the Profile Editor.

**BR-02: Status Hierarchy** (P1) — `Broken` (2) > `Stale` (1) > `Healthy` (0). Drives color coding, default sort order, and summary card prominence.

**BR-03: Metadata May Be Absent** (P1) — `ProfileHealthMetadata` is `null` when `MetadataStore` is unavailable. All metadata-dependent columns must degrade to "N/A" or hide. The page must never crash on null metadata.

**BR-04: Re-Check All** (P1) — Triggers `batchValidate()`. Button disabled during scan. Hook manages loading state.

**BR-05: Issue Category Aggregation is Client-Side** (P2) — Backend provides no pre-aggregated counts. Group `issues` by `field` prefix: `game.executable_path` → "Missing executable", `trainer.path` → "Missing trainer", `steam.proton_path`/`runtime.proton_path` → "Missing Proton path", etc.

**BR-06: Failure Trend Window** (P3) — 30-day rolling window. Panel inclusion: `failure_count_30d > 0`. Badge threshold: `failure_count_30d >= 2`.

**BR-07: Trend Direction** (P3) — Derived from comparing live `HealthStatus` against `CachedHealthSnapshot.status`. Only `got_worse` and `got_better` render arrows. `unchanged` and `null` (no baseline) are silent.

**BR-08: Launcher Drift States** (P3) — `missing`, `moved`, `stale` show re-export warning. `aligned`, `unknown`, `null` are silent.

### Edge Cases

| Scenario                      | Expected Behavior                                           | Phase |
| ----------------------------- | ----------------------------------------------------------- | ----- |
| Zero profiles                 | Empty state: "No profiles yet" + link to create             | P1    |
| All profiles healthy          | Positive state: summary cards + "All healthy" message       | P1    |
| MetadataStore unavailable     | Core health data works; metadata columns show "N/A"         | P1    |
| Malformed profile TOML        | Appears as Broken with parse error; Fix navigates to editor | P1    |
| ProfileStore.list() failure   | Sentinel `<unknown>` entry → system error banner            | P1    |
| Stale cache, no live data yet | Show cached data with "cached / checking…" indicator        | P1    |
| No cached snapshot baseline   | Trend arrows don't render (null trend)                      | P3    |

### Success Criteria

#### Phase 1

- [ ] Health tab reachable from sidebar
- [ ] Summary cards show correct color-coded counts
- [ ] All profiles listed with status badge, default broken-first order
- [ ] Re-check All triggers batch validate, disables during scan
- [ ] Fix navigates to Profile Editor with profile pre-selected
- [ ] Empty state and all-healthy state render correctly
- [ ] Page handles null metadata without crashing
- [ ] Cached data displays instantly on load
- [ ] D-pad navigates profile list; A activates Fix

#### Phase 2

- [ ] Table sortable by status, name, issue count, last success, launch method
- [ ] Filter by status (broken/stale/all), text search by name
- [ ] Expandable row shows issue details
- [ ] Single-profile re-check updates one row
- [ ] Issue breakdown panel categorizes problems by type

#### Phase 3

- [ ] Trend arrows on rows where status changed since last snapshot
- [ ] Recent Failures panel lists profiles with failures in last 30 days
- [ ] Launcher drift indicator per row
- [ ] Community import annotation on broken/stale imports
- [ ] Favorites flagged with star
- [ ] Y gamepad button triggers Re-check All

## Technical Specifications

### Architecture Overview

```
App.tsx
  └─ AppShell
       └─ ContentArea (route="health")
            └─ HealthDashboardPage
                 ├─ PageBanner (eyebrow="Dashboards")
                 ├─ SummaryCards (4 cards: total/healthy/stale/broken)     [P1, P3 trends]
                 ├─ CollapsibleSection "Re-check"                          [P1]
                 ├─ CollapsibleSection "Issue Breakdown"                    [P2]
                 ├─ CollapsibleSection "All Profiles"                       [P1 base, P2 full]
                 │    ├─ TableToolbar (search, filter)                      [P2]
                 │    └─ HealthTable (sortable, clickable rows)             [P1 base, P2 full]
                 ├─ CollapsibleSection "Recent Failures"                    [P3]
                 ├─ CollapsibleSection "Launcher Drift"                     [P3]
                 └─ CollapsibleSection "Community Import Health"            [P3]

Data Flow:
  useProfileHealth() ─── batch_validate_profiles ──→ EnrichedHealthSummary
                     ├── get_cached_health_snapshots ─→ CachedHealthSnapshot[]
                     └── profile-health-batch-complete (startup event)

  "Fix" action ─── selectProfile(name) + onNavigate('profiles') ──→ ProfilesPage
```

### Data Models

#### Existing Types (No Changes)

**`EnrichedHealthSummary`** — root IPC response:

```typescript
interface EnrichedHealthSummary {
  profiles: EnrichedProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string; // ISO 8601
}
```

**`EnrichedProfileHealthReport`** — per-profile entry (serde `#[flatten]` on Rust side):

```typescript
interface EnrichedProfileHealthReport extends ProfileHealthReport {
  metadata: ProfileHealthMetadata | null;
}
```

**`ProfileHealthMetadata`** — enrichment from MetadataStore:

```typescript
interface ProfileHealthMetadata {
  profile_id: string | null;
  last_success: string | null; // ISO 8601
  failure_count_30d: number;
  total_launches: number;
  launcher_drift_state: string | null; // "aligned"|"missing"|"moved"|"stale"|"unknown"
  is_community_import: boolean;
  is_favorite?: boolean;
}
```

**`HealthIssue`** — individual validation issue:

```typescript
interface HealthIssue {
  field: string; // e.g. "game.executable_path"
  path: string; // sanitized with ~ for home dir
  message: string;
  remediation: string;
  severity: HealthIssueSeverity; // "error"|"warning"|"info"
}
```

#### New Page-Local Types (Not Exported)

**Phase 2:**

```typescript
type SortField = 'name' | 'status' | 'issues' | 'last_success' | 'launch_method' | 'failures' | 'favorite';
type SortDirection = 'asc' | 'desc';
type StatusFilter = 'all' | HealthStatus;
```

**Phase 3:**

```typescript
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
```

### API Design

No new Tauri IPC commands. The page consumes the `useProfileHealth` hook which wraps all three existing commands.

#### Component Interface

```typescript
interface HealthDashboardPageProps {
  onNavigate?: (route: AppRoute) => void; // included from P1, used from P2
}
```

### System Integration

#### Files to Create

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx` — The page component with inline sub-components (SummaryCard, HealthTable, panel components)

#### Files to Modify

| File                                    | Change                                                            | Phase  |
| --------------------------------------- | ----------------------------------------------------------------- | ------ |
| `src/components/layout/Sidebar.tsx`     | Add `'health'` to `AppRoute` union, sidebar entry, route label    | P1     |
| `src/components/layout/ContentArea.tsx` | Import + render `HealthDashboardPage`                             | P1     |
| `src/App.tsx`                           | Add `health: true` to `VALID_APP_ROUTES`                          | P1     |
| `src/components/icons/SidebarIcons.tsx` | Add `HealthIcon` SVG component                                    | P1     |
| `src/components/layout/PageBanner.tsx`  | Add `HealthDashboardArt` illustration                             | P1     |
| `src/hooks/useProfileHealth.ts`         | Update `invoke<>` generics to `EnrichedHealthSummary` (type-only) | Pre-P1 |
| `src/components/pages/ProfilesPage.tsx` | Import `formatRelativeTime` from extracted utility                | P2     |
| `src/styles/theme.css`                  | Add health dashboard table styles if needed                       | P3     |

#### Files to Create (Phase 2)

- `src/crosshook-native/src/utils/format.ts` — Extract `formatRelativeTime` from ProfilesPage for shared use

## UX Considerations

### User Workflows

#### Primary Workflow: Rapid Triage

1. User navigates to Health tab (sidebar click or L1/R1 bumper cycle)
2. Summary cards render immediately from cached snapshots — non-zero broken count is prominent
3. User scans the profile list sorted by status severity (broken first)
4. User identifies broken profile row — status badge and issue count provide context
5. User activates "Fix" (A button / click) → navigates to Profile Editor with profile pre-selected

#### Error Recovery Workflow

1. If batch validation fails, `role="alert"` banner shows generic error with Retry button
2. Existing cached data remains visible — user never sees a blank state
3. Individual profile fetch failures show inline error on that row only

### UI Patterns

| Component          | Pattern                                                                                | Notes                                                                      |
| ------------------ | -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Summary cards      | 4-card row, `grid-template-columns: repeat(4, 1fr)`                                    | Left-border accent stripe (4px) in status color, not full background fills |
| Table              | HTML `<table>` with `role="grid"`                                                      | Row-level focus, `aria-sort` on column headers                             |
| Filter bar         | Above table, not inline                                                                | Status multi-select + text search with `maxLength={200}`                   |
| Collapsible panels | Existing `CollapsibleSection`                                                          | `defaultOpen={false}` for secondary panels                                 |
| Status colors      | `--crosshook-color-success` / `--crosshook-color-warning` / `--crosshook-color-danger` | Existing CSS variables, no new color definitions                           |
| Loading            | Skeleton cards + skeleton rows                                                         | Not a full-page spinner; cached data shown with "checking…" indicator      |
| Empty state        | Illustration + message + action link                                                   | "No profiles yet" → link to Profiles page                                  |

### Accessibility Requirements

- **ARIA grid pattern**: `role="grid"` on table, `aria-sort` on sortable headers, `aria-rowcount` + `aria-rowindex` for screen reader context
- **Live region**: `role="status"` + `aria-live="polite"` for validation completion announcements; `role="alert"` only for failures
- **Row labels**: `aria-label="[Name] — [Status], [N] issues, last launched [time]"` for screen reader scanning
- **Roving tabindex**: Only one table row in tab order at a time (`tabIndex={0}` on focused row, `-1` on others)
- **Reduced motion**: Respect `prefers-reduced-motion` — disable counting animations and skeleton pulses

### Performance UX

- **Optimistic display**: Render from `cachedSnapshots` on mount before live scan completes
- **Skeleton loading**: 4 skeleton cards + 5–8 skeleton rows during initial validation
- **Re-check feedback**: Indeterminate progress indicator, button disabled with "Checking…" text
- **No virtualization for v1**: Plain table is sufficient for ≤50 profiles; `react-window` available if needed

### Gamepad Navigation

- **D-pad up/down**: Navigate table rows
- **A button**: Activate "Fix" on focused row
- **Y button** (P3): Trigger Re-check All from any position
- **B button**: Back to filter controls above table
- **L1/R1**: Cycle sidebar tabs (existing behavior)
- **Minimum 48px row height** for touchscreen targets on Steam Deck

## Recommendations

### Implementation Approach

**Recommended Strategy**: Single-file page component with inline sub-components, consuming `useProfileHealth` directly, hand-rolled sort/filter table. Three independently shippable phases.

**Phasing:**

1. **Phase 1 — Core Dashboard (MVP)**: Route wiring + page shell + summary cards + basic profile list + Re-check All + Fix navigation + loading/error/empty states + basic gamepad nav. Delivers 80% of user value.
2. **Phase 2 — Secondary Panels**: Sortable/filterable table upgrade + issue breakdown + recent failures + launcher drift + community import health. Filtered views of existing data.
3. **Phase 3 — Polish**: Skeleton loading states + Y button gamepad + trend arrows on cards + responsive layout. Visual refinement.

### Technology Decisions

| Decision                   | Recommendation                                                | Rationale                                                                                                                             |
| -------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Table library              | None (hand-rolled `useMemo`)                                  | ≤50 rows, consistent with `CompatibilityViewer` pattern, zero bundle cost                                                             |
| Hook instance              | Separate `useProfileHealth()` per page                        | ContentArea renders one page at a time; dual instances don't conflict. Lift to context only if profiling shows duplicate batch calls. |
| Sidebar placement          | New "Dashboards" section with "Health" item                   | Generic section name, future-proof for additional dashboards                                                                          |
| Single file vs. multi-file | Single file with local sub-components                         | Follow `ProfilesPage` pattern; extract after Phase 3 if >800 lines                                                                    |
| Fix navigation             | `selectProfile(name)` + `onNavigate('profiles')` sequentially | Test timing first; add `pendingNavProfile` to ProfileContext only if unreliable                                                       |
| Y button                   | Page-local `useEffect` polling button index 3                 | Avoids modifying shared `useGamepadNav` hook                                                                                          |
| Filtering                  | `String.includes()`, not `RegExp`                             | Avoids ReDoS from user input                                                                                                          |

### Quick Wins

- Reuse `HealthBadge` directly in table status column — zero new rendering code
- Derive all secondary panel data from `summary.profiles` array with simple filter predicates
- Use existing CSS variables (`--crosshook-color-success/warning/danger/accent`) for all color coding
- `countProfileStatuses()` utility already exists in `utils/health.ts`

### Future Enhancements

- **Sidebar notification badge**: Red dot on Health tab when `broken_count > 0`
- **Health-aware launch gating**: Warning on Launch page when selected profile is broken
- **Export health report**: Copy summary to clipboard for support channels
- **Health history sparkline**: 7-day status history per profile from existing SQLite snapshots
- **Batch fix for common issues**: Update Proton path for all affected profiles at once

## Risk Assessment

### Technical Risks

| Risk                                                                      | Likelihood | Impact | Mitigation                                                                                     |
| ------------------------------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------- |
| Frontend type mismatch (`HealthCheckSummary` vs. `EnrichedHealthSummary`) | High       | Medium | Update hook's `invoke<>` generics pre-Phase 1 (type-only change, no runtime impact)            |
| Fix navigation timing race (`selectProfile` + `onNavigate` in same tick)  | Medium     | Low    | Test sequential calls first; add `pendingNavProfile` to ProfileContext if unreliable           |
| Dual `useProfileHealth` causing redundant batch validation                | Medium     | Low    | Startup event deduplication prevents on mount. Monitor backend logs; lift to context if needed |
| Table performance with 100+ profiles                                      | Low        | Medium | Unlikely scenario. Add `useDeferredValue` for search; `react-window` available if needed       |
| Gamepad Y button polling conflicts with main hook                         | Low        | Low    | Both read gamepad state independently; no shared mutable state                                 |

### Integration Challenges

- **TypeScript exhaustive switch**: Adding `'health'` to `AppRoute` causes compile error in `ContentArea.tsx` until switch case is added (by design)
- **`controllerMode` access**: Currently lives in `AppShell`, not passed to `ContentArea`/pages. Options: thread through props, or detect locally via gamepad API
- **CSS namespace**: New classes follow `crosshook-health-dashboard*` pattern, added to existing `theme.css`

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| Finding                          | Risk                                                             | Mitigation                                                                  | Alternatives                                      |
| -------------------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------- |
| XSS via profile name rendering   | If `innerHTML`/`dangerouslySetInnerHTML` used with profile names | Use JSX interpolation only (`{profile.name}`) — React escapes automatically | Audit any new component that renders user strings |
| CSP missing explicit `style-src` | Inline styles may break under strict CSP in production AppImage  | Add `style-src 'self' 'unsafe-inline'` to `tauri.conf.json` CSP             | Migrate inline styles to CSS classes (long-term)  |

#### Advisories — Best Practices

- **Search input length cap**: Add `maxLength={200}` to filter input (deferral: low-impact hygiene item)
- **Error message path leakage**: Display generic "scan failed" message, log full error to console (deferral: local app, user sees own paths)
- **CSP `img-src` restriction**: Add `img-src 'self' data:` for defense-in-depth (deferral: requires XSS to be exploitable first)

## Task Breakdown Preview

### Phase 1: Core Dashboard (MVP)

**Focus**: Route wiring, page shell, summary cards, basic profile list, Re-check All, Fix navigation

**Tasks**:

- **1.1 Routing Integration** — Add `'health'` to `AppRoute`, sidebar entry, `VALID_APP_ROUTES`, ContentArea switch, `HealthIcon`, `HealthDashboardArt`
- **1.2 Page Shell + Summary Cards** — Create `HealthDashboardPage.tsx`, `PageBanner` header, 4 color-coded summary cards, wire `useProfileHealth`, loading/error/empty states
- **1.3 Profile Health Table** — Basic table with columns (name, status via `HealthBadge`, issue count), default broken-first sort, `role="grid"` + `aria-sort`, row `tabIndex={0}`
- **1.4 Fix Navigation** — "Fix" button per row, `selectProfile()` + `onNavigate('profiles')`, test timing
- **1.5 Re-check All** — Button wired to `batchValidate()`, disabled during scan, `aria-live` status region

**Pre-requisite**: Update `useProfileHealth` invoke generics to `EnrichedHealthSummary` (type-only change)

**Parallelization**: 1.4 and 1.5 are independent after 1.3 completes

### Phase 2: Table Upgrade + Secondary Panels

**Focus**: Sortable/filterable table, issue breakdown, recent failures, launcher drift, community import health

**Dependencies**: Phase 1 complete

**Tasks**:

- **2.1 Sortable/Filterable Table** — Add sort headers (status/name/issues/last success/launch method/favorite), status filter dropdown, text search with `useDeferredValue`, all 8 columns
- **2.2 Issue Breakdown Panel** — Aggregate issues by field category, collapsible section, CSS bar charts showing affected profile counts
- **2.3 Recent Failures Panel** — Filter `failure_count_30d > 0`, collapsible (defaultOpen=false)
- **2.4 Launcher Drift Summary** — Filter `launcher_drift_state` not null/aligned, collapsible
- **2.5 Community Import Health** — Filter `is_community_import && status !== healthy`, contextual note

**Parallelization**: 2.2–2.5 are all independent of each other; all depend on 2.1

### Phase 3: Polish

**Focus**: Visual refinement, gamepad Y button, trend arrows, responsive layout

**Dependencies**: Phases 1 and 2 complete

**Tasks**:

- **3.1 Skeleton Loading States** — Skeleton cards + rows during initial validation, smooth cached→live transition
- **3.2 Gamepad Y Button** — Page-local `useEffect` for button index 3, trigger `batchValidate()`, controller prompt
- **3.3 Trend Arrows on Summary Cards** — Compare current vs. cached aggregate counts, arrow + color per card
- **3.4 Responsive Card Layout** — CSS: 4 cards at >1100px, 2x2 at 900px, stacked at <640px
- **3.5 Extract `formatRelativeTime`** — Move from `ProfilesPage.tsx` to `utils/format.ts`, update imports

**Parallelization**: All Phase 3 tasks are independent of each other

## Decisions (Resolved)

All decisions have been resolved:

1. **Hook instance strategy** → **Separate `useProfileHealth()` per page.** ContentArea renders only one page at a time via its switch statement — both pages never mount simultaneously. No context lift needed, no refactoring of ProfilesPage.

2. **Sidebar section placement** → **New "Dashboards" section with "Health" item.** Generic section name allows future dashboard pages (e.g., "Launch History", "Compatibility") to be grouped under the same section without renaming.

3. **Fix navigation mechanism** → **Sequential `selectProfile()` + `onNavigate('profiles')`.** ProfileContext wraps the entire app, so `selectProfile()` updates global state that ProfilesPage reads on mount. No `pendingNavProfile` needed.

4. **`formatRelativeTime` extraction** → **Extract to `utils/format.ts` in Phase 2** when the second consumer (dashboard) needs it. Two callers justifies a shared utility.

5. **Table library** → **Hand-rolled with `useMemo` sort/filter.** Zero new dependencies. Sufficient for ≤50 profile rows. Consistent with existing `CompatibilityViewer` pattern.

6. **`controllerMode` access** → **Detect locally via gamepad API.** The Phase 3 Y-button `useEffect` already polls `navigator.getGamepads()` directly, which inherently detects controller presence. No need to thread props through ContentArea.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Library evaluation (table, gamepad, charts — all "no new dep")
- [research-business.md](./research-business.md): User stories by phase, business rules, domain model, edge cases
- [research-technical.md](./research-technical.md): Architecture spec, data models, routing changes, phase boundary contracts
- [research-ux.md](./research-ux.md): Dashboard UX patterns, gamepad nav, accessibility, competitive analysis
- [research-security.md](./research-security.md): 0 critical, 2 warnings (XSS, CSP), 3 advisories
- [research-practices.md](./research-practices.md): Reusable code inventory, KISS assessment, build vs. depend
- [research-recommendations.md](./research-recommendations.md): Phased implementation plan, risk assessment, alternatives
