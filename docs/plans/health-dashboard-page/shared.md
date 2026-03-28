# Health Dashboard Page

The Health Dashboard is a read-only diagnostics page added as a new top-level tab under a "Dashboards" sidebar section. It consumes existing Tauri IPC commands (`batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`) and the `useProfileHealth` hook to display aggregate profile health — summary cards, a sortable/filterable health table, issue breakdown, recent failures, launcher drift, and community import health. Implementation is pure frontend: one new page component (`HealthDashboardPage.tsx`), five existing files modified for routing, no Rust changes, no new dependencies. The feature ships in three phases: P1 (MVP — route + cards + list + re-check + fix nav), P2 (interactive table + secondary panels), P3 (polish — trends, gamepad Y, skeleton loading).

## Relevant Files

- src/crosshook-native/src/App.tsx: Route validation map (`VALID_APP_ROUTES`), AppShell with Tabs.Root — add `health: true`
- src/crosshook-native/src/components/layout/Sidebar.tsx: `AppRoute` type union (line 12), `SIDEBAR_SECTIONS` (line 32), `ROUTE_LABELS` — add `'health'` route and "Dashboards" section
- src/crosshook-native/src/components/layout/ContentArea.tsx: Route-to-page switch (lines 34-51), `onNavigate` prop pattern (line 40) — add `'health'` case
- src/crosshook-native/src/components/icons/SidebarIcons.tsx: SVG icon components (20x20 viewBox, stroke-based) — add `HealthIcon`
- src/crosshook-native/src/components/layout/PageBanner.tsx: Banner + illustration pattern (200x120 viewBox SVGs) — add `HealthDashboardArt`
- src/crosshook-native/src/hooks/useProfileHealth.ts: Primary data hook — provides `summary`, `loading`, `error`, `healthByName`, `trendByName`, `staleInfoByName`, `cachedSnapshots`, `batchValidate`, `revalidateSingle`
- src/crosshook-native/src/types/health.ts: All TypeScript health types — `EnrichedHealthSummary`, `EnrichedProfileHealthReport`, `ProfileHealthMetadata`, `HealthIssue`, `CachedHealthSnapshot`
- src/crosshook-native/src/utils/health.ts: `countProfileStatuses()` utility for aggregating status counts
- src/crosshook-native/src/components/HealthBadge.tsx: Reusable status badge with trend arrow and failure count — drop-in for table rows
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Controlled/uncontrolled collapsible panel — use for all secondary sections
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Existing health badge + issue display pattern (lines 501-563), `formatRelativeTime` (line 22) to extract in P2
- src/crosshook-native/src/components/pages/CommunityPage.tsx: Thin page wrapper pattern — follow for `HealthDashboardPage` structure
- src/crosshook-native/src/components/CompatibilityViewer.tsx: Filter/search pattern with `useDeferredValue` — follow for table search
- src/crosshook-native/src/hooks/useGamepadNav.ts: Gamepad/keyboard navigation — D-pad, A/B, L1/R1, zone model, Steam Deck detection
- src/crosshook-native/src/hooks/useProfile.ts: `useProfileContext()` — provides `selectProfile()` for fix navigation
- src/crosshook-native/src/context/ProfileContext.tsx: Pattern to follow if context lift is ever needed
- src/crosshook-native/src/styles/variables.css: CSS custom properties (`--crosshook-color-success/warning/danger/accent`, spacing, radius)
- src/crosshook-native/src/styles/theme.css: `.crosshook-panel`, `.crosshook-card`, `.crosshook-status-chip`, `.crosshook-compatibility-badge--{rating}`, `.crosshook-heading-*`, `.crosshook-muted`, `.crosshook-help-text`
- src/crosshook-native/src-tauri/src/commands/health.rs: Tauri health commands — `batch_validate_profiles` returns `EnrichedHealthSummary`, path sanitization via `sanitize_report()`
- src/crosshook-native/src-tauri/src/lib.rs: Command registration (lines 166-168 for health commands), startup health scan
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs: Core health validation — field checks, `HealthStatus`, `HealthIssue`, `HealthIssueSeverity`
- src/crosshook-native/src-tauri/tauri.conf.json: CSP configuration (line 23) — may need `style-src 'self' 'unsafe-inline'`

## Relevant Patterns

**Page Component Pattern**: Each page is a function component using `PageBanner` header + content sections. See [src/crosshook-native/src/components/pages/CommunityPage.tsx] for the thin wrapper pattern. Pages receive `onNavigate` from ContentArea for cross-page navigation.

**Hook-Driven Data**: All data fetching is via custom hooks wrapping Tauri `invoke()` calls. See [src/crosshook-native/src/hooks/useProfileHealth.ts] — the dashboard consumes this hook directly with separate instances per page (ContentArea renders one page at a time).

**Collapsible Sections**: Secondary content uses `CollapsibleSection` with `defaultOpen` prop. See [src/crosshook-native/src/components/ui/CollapsibleSection.tsx].

**Filter/Search with Deferred Value**: Text search uses `useDeferredValue` to prevent blocking on keystroke. See [src/crosshook-native/src/components/CompatibilityViewer.tsx] lines 110-140.

**Gamepad Focus Zones**: Content areas use `data-crosshook-focus-zone="content"` for D-pad navigation. See [src/crosshook-native/src/hooks/useGamepadNav.ts] for the zone model. Table rows need `tabIndex={0}` for gamepad traversal.

**Status Color Coding**: Health statuses map to CSS variables via `HealthBadge`'s `STATUS_TO_RATING` mapping — `broken` → `--crosshook-color-danger`, `stale` → `--crosshook-color-warning`, `healthy` → `--crosshook-color-success`.

**Sidebar Route Extension**: Adding a route requires 4 synchronized changes: `AppRoute` union, `VALID_APP_ROUTES`, `SIDEBAR_SECTIONS`, ContentArea switch. TypeScript exhaustive check at ContentArea line 47 enforces completeness.

## Relevant Docs

**docs/plans/health-dashboard-page/feature-spec.md**: You _must_ read this when implementing any phase — contains resolved decisions, phased user stories, business rules, data models, edge cases, and success criteria.

**docs/plans/health-dashboard-page/research-technical.md**: You _must_ read this when working on architecture or component structure — contains phase boundary contracts, component hierarchy, and file change lists per phase.

**docs/plans/health-dashboard-page/research-ux.md**: You _must_ read this when working on UI layout, accessibility, or gamepad navigation — contains ARIA patterns, loading states, dark theme color coding, and competitive analysis.

**docs/plans/health-dashboard-page/research-security.md**: You _must_ read this when rendering profile names or paths — contains XSS mitigation (use JSX interpolation only), CSP guidance, and secure coding patterns.

**docs/plans/health-dashboard-page/research-business.md**: You _must_ read this when implementing business rules or edge cases — contains phase-tagged rules (BR-01 through BR-15) and edge cases (EC-01 through EC-07).
