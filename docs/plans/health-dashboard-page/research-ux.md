# UX Research: Profile Health Dashboard Page

## Executive Summary

The Profile Health Dashboard is a read-only aggregate diagnostics view for all CrossHook profiles. It must serve three primary user goals simultaneously: rapid triage of broken/stale profiles, pattern identification across the full library, and trend tracking over time. The 1280x800 Steam Deck constraint and full gamepad navigation requirement heavily influence layout density and interaction model choices.

Key findings:

- A four-card summary row (Total / Healthy / Stale / Broken) at the top is the established industry pattern for aggregate status dashboards; limit to this row only — more than 5–6 cards on initial view hurts scanability.
- Dark theme status color systems require strict semantic assignments: green = operational, amber/yellow = degraded, red = failure. These map directly to the existing `--crosshook-color-success`, `--crosshook-color-warning`, and `--crosshook-color-danger` CSS variables.
- Skeleton screens during batch validation outperform spinners for 10+ item sets. For 50+ profiles, pair a global indeterminate progress bar with skeleton rows.
- The existing two-zone gamepad model (sidebar / content) in `useGamepadNav.ts` must be extended with a third layer: the data table needs its own row-level D-pad navigation, separate from the content zone's sequential focus traversal.
- WCAG 4.1.3 requires status messages to be announced without focus theft — use `role="status"` + `aria-live="polite"` for validation completion, `role="alert"` only for hard failures.

**Confidence**: High — findings derived from multiple authoritative sources (MDN, WCAG documentation, Carbon Design System, Smashing Magazine, industry-standard design systems) plus direct analysis of the existing CrossHook codebase.

---

## User Workflows

### Primary Flow: Rapid Triage

1. User navigates to Health page (sidebar nav, or L1/R1 bumper cycle on Steam Deck).
2. Summary cards render immediately from cached snapshots (Phase D data already available in `useProfileHealth`). Non-zero broken count is visually prominent.
3. User scans the profile table, sorted by status severity (broken first) by default.
4. User identifies the broken profile row. Status badge and issue count column give enough context without expanding.
5. User presses A (gamepad) or Enter (keyboard) on a broken row — navigates to Profiles page with that profile pre-selected. The health dashboard never edits directly.

**Decision point mapped to business logic**: The navigation-to-editor action is the single exit point from this page. The dashboard must pass the profile name to the Profiles page context, not attempt any in-place mutation.

### Secondary Flow: Pattern Identification

1. After initial triage, user wants to know if missing-executable issues are widespread.
2. User uses the category filter (or column sort by issue count) to surface profiles with the same issue type.
3. Issue Breakdown section (below summary cards) groups issue categories: missing_executable, missing_proton, inaccessible_directory, launcher_drift, other.
4. User can see at a glance whether a single issue type is systemic (e.g., all proton_run profiles missing Proton).

**Decision point**: Issue category grouping should be a summary, not a deep drill-down. Each category row shows count, not individual profiles — users click through to the filtered table.

### Alternative Flow: Re-check All

1. User notices "Last validated 8 days ago" note (stale cache indicator).
2. User activates "Re-check All" button (Y button on gamepad).
3. Progress feedback appears. Table rows show per-row skeleton or spinner during validation.
4. When complete, summary cards update, trend arrows appear comparing new results to cached snapshots.
5. `aria-live="polite"` region announces "Validation complete — 3 broken, 2 stale" to screen readers without interruption.

### Alternative Flow: Launcher Drift Review

1. User opens Health page after exporting several launchers.
2. Launcher Drift Summary section shows count of profiles with drift states: missing, moved, stale.
3. User sorts table by "launcher drift" column to cluster drifted profiles together.
4. User activates each row to open the profile editor for re-export.

### Alternative Flow: Community Import Audit

1. User imported profiles from community tap.
2. Community Import Health section shows how many community-sourced profiles have path-related issues.
3. These are expected to be stale on first import (paths need adjustment), so the section contextualizes this without alarming the user.

---

## 2. UI/UX Best Practices

### Dashboard Layout for 1280x800

The sidebar in CrossHook occupies ~200px (collapsed: 56px), leaving ~1024–1080px for content at the 1280px target width. The existing `--crosshook-page-padding: 32px` leaves ~960px of usable content width at full sidebar.

**Layout grid recommendation**: Single-column scroll layout within the content area. Do not use multi-column unless all columns are visible simultaneously on 1280x800 without horizontal scroll.

Structure (top to bottom):

```
[ Page Banner: "Health" eyebrow, "Profile Health Dashboard" title ]
[ Re-check All button row (right-aligned) + "Last validated: N min ago" ]
[ Summary Cards Row: 4 cards equal width ]
[ Issue Breakdown Section (collapsible) ]
[ Profile Health Table (main content, largest area) ]
[ Recent Failures Panel (collapsible) ]
[ Launcher Drift Summary (collapsible) ]
[ Community Import Health (collapsible) ]
```

The existing `CollapsibleSection` component already handles the collapsible pattern consistently. Reuse it for all secondary sections.

**Grid for summary cards**: Use `grid-template-columns: repeat(4, 1fr)` with `gap: var(--crosshook-grid-gap)`. At 900px breakpoint, collapse to 2x2. At 640px (unlikely for this app but safe), stack to 1-column.

### Stat Card Design

Each summary card contains:

- Large count (2.4–3rem, bold, color-coded by status)
- Label below count (1rem, `--crosshook-color-text-muted`)
- Optional trend arrow (top-right corner, small, color-coded)
- Card border: left-side accent stripe (4px) in the status color, matching the existing `HealthBadge` severity conventions

Color assignments (matching existing CSS variables):

- **Healthy**: `--crosshook-color-success` (#28c76f)
- **Stale**: `--crosshook-color-warning` (#f5c542)
- **Broken**: `--crosshook-color-danger` (#ff758f)
- **Total**: `--crosshook-color-accent` (#0078d4)

**Do not use background fills for entire cards** — on dark backgrounds, colored card backgrounds create excessive visual noise. Use left-border accents and colored count text only. This matches research showing that dark-mode status systems rely on color temperature scaling from grey (off/neutral) to red (critical) without full-area fills.

**Trend arrows on cards**: Up arrow = more issues (worse), down arrow = fewer issues (better). The total count arrow is neutral (no trend color). Reuse the trend logic already in `useProfileHealth.ts` — `computeTrend()` returns `got_worse | got_better | unchanged`.

### Data Table UX

The profile health table is the core interaction surface. Columns (recommended order, left to right):

1. **Status badge** (40px fixed, no label, sortable)
2. **Profile name** (flex grow, sortable, primary identifier)
3. **Issue count** (80px, sortable by number)
4. **Last successful launch** (120px, relative time, sortable)
5. **Launch method** (100px, filterable)
6. **Failure trend** (80px, trend arrow + count, sortable)
7. **Favorites** (40px, star icon, filterable)
8. **Source** (80px: local / community, filterable)
9. **Action** (60px, "Open" button, fixed right — not sortable)

**Column header clicks toggle sort**: ascending → descending → default (by severity). Show sort arrow indicator in column header. Use `aria-sort="ascending|descending|none"` on `<th>` elements.

**Row height**: Minimum 48px (`--crosshook-touch-target-min`). Gamepad users need adequate touch targets.

**Zebra striping**: Use subtle alternating row backgrounds (`rgba(255,255,255,0.02)` alternating with transparent). Do not use strong contrast — maintain focus ring visibility on both row types.

**Filter controls**: Place above the table, not inline. A filter bar with: status multi-select (All / Healthy / Stale / Broken), launch method dropdown, source dropdown (Local / Community), and a search input for profile name. All filters are additive (AND). Reset button clears all filters. The name search input must have `maxLength={200}` to prevent unbounded input; placeholder text "Filter by name..." is appropriate.

**Path display in issue details**: `HealthIssue.path` values arrive from the backend already sanitized — home directory is replaced with `~` (e.g., `~/games/MyTrainer.exe`). Display these as-is inside a `<code>` element for monospace readability. No frontend path transformation is needed or appropriate.

**Profile name display**: Profile names are user-defined strings. They render safely via React JSX interpolation. Do not use HTML `innerHTML` or `dangerouslySetInnerHTML`. For tooltips showing issue summaries, use plain `title` attributes or custom tooltip components with text-only content — never render profile names as markup.

**Sort default**: broken first, then stale, then healthy; within each group sort alphabetically by name. This puts the most actionable items at the top without requiring user interaction.

**Virtualization**: For 50+ profiles, use windowed rendering (e.g., `react-window` or a lightweight equivalent). The table should not render all rows if the list exceeds ~30 items. However, keyboard and gamepad navigation must still traverse all rows logically even if they are virtualized — this requires careful focus management (see Gamepad Navigation section).

### Dark Theme Status Color Coding

Research finding: inconsistent color systems leave users confused. Apply status colors only to semantic status elements (badges, stat card numbers, left-border accents, trend arrows). Do not reuse status colors for decorative purposes elsewhere on this page.

Avoid pure red/green for accessibility — the existing danger (#ff758f) and success (#28c76f) are already offset from pure red/green, which helps deuteranopia users. No change needed.

For hover/focus states on status-colored elements, lighten the color by applying `opacity: 0.8` on hover rather than shifting hue. This preserves semantic meaning while providing interaction feedback.

---

## 3. Error Handling UX

### Empty States

**No profiles exist**: Show illustration + message: "No profiles yet" + action button "Create your first profile" (navigates to Profiles page). Do not show the table at all.

**All profiles healthy**: Show the summary cards (4 zeros except healthy = total), then a positive empty state inside the table area: checkmark illustration + "All profiles are healthy" + "Last checked: [relative time]" + soft "Re-check" text link. Avoid over-celebrating — this is a utility tool, not a game.

**Validation in progress (first load)**: Show skeleton cards (4 grey placeholder boxes at the same dimensions as real cards) + skeleton table rows. Do not show "0 broken" while validation is running — show `—` or a pulsing placeholder. This prevents the false-positive of showing "healthy" counts that are actually just incomplete.

**Validation error (batch validate failed)**: Use `role="alert"` banner at top of content area. Message: "Health scan failed. Check the app logs for details." with a "Retry" button. Do **not** surface the raw IPC error string — Rust backend errors may contain raw filesystem paths before sanitization. Do not clear existing cached data — show the stale cached state with a banner warning. Users should not lose the last-known state because of a transient error.

**Individual profile health fetch failed**: Show an error state inline in that row only. Do not fail the whole table. Mark the row with a "?" status badge and tooltip: "Could not load health for this profile."

### Loading States

- **Initial load (no cache)**: Skeleton table with 5-8 skeleton rows + 4 skeleton stat cards.
- **Cached data available**: Show cached state immediately (cards and table from `cachedSnapshots`), overlay a subtle "Checking..." status badge in the top-right of the page (not a full-page overlay). This is the optimistic display pattern.
- **Re-check All in progress**: Show global indeterminate progress bar at the top of the content area (below the banner). Individual rows that have been re-validated update in-place as results come in — do not wait for all profiles to complete before updating the UI. Per-row spinner icon in the "status" column during active validation of that row.

### Validation Progress (50+ Profiles)

Research finding: for long waits (10+ seconds), show progress bar, percentage, and status updates. For validation of 50+ profiles:

1. Indeterminate progress bar at content area top (since total count may not be pre-known).
2. "Checking N profiles..." text label that updates as validation proceeds.
3. Per-row status: rows that complete validation update their status badge immediately (no full re-render).
4. Accessibility: `aria-live="polite"` region announces at 25%, 50%, 75%, and completion milestones.
5. "Re-check All" button becomes disabled and shows "Checking..." text during validation. Must be re-enabled immediately on completion or error.

---

## Performance UX

### Optimistic Display

The `useProfileHealth` hook already fetches `get_cached_health_snapshots` on mount before the startup batch validation event arrives. Use this data immediately:

- Render stat cards from cached counts.
- Render table rows from cached snapshots with a "cached" visual indicator (small clock icon or "N days ago" in the last-checked column).
- When live validation completes, transition smoothly from cached to live data without a flash.

### Transition Animations

Use CSS transitions for count changes in stat cards (the number should animate up/down using a counting animation capped at 300ms). For row status badge transitions, use `transition: background-color 140ms ease` (the existing `--crosshook-transition-fast`). Do not animate row reorder — an abrupt resort after Re-check All is fine and predictable.

### Table Rendering Strategy

- Render up to 30 rows without virtualization.
- Above 30 rows, use windowed rendering with overscan of 5 rows to reduce visible pop-in.
- Height of the table container should be fixed (fill available viewport height) to enable stable scroll behavior. The existing resizable panel system in `App.tsx` handles the outer container — the table should fill its parent.

---

## 5. Competitive Analysis

### Steam Library

Steam's library does not have a dedicated health dashboard but shows per-game icons and contextual warnings. Key observation: Steam places actionable items (install, update, play) at row level, not in a separate detail panel. CrossHook should follow this — the "Open" action per row is the right pattern.

**Confidence**: Medium — based on general knowledge of Steam UI patterns.

### Lutris

Lutris shows game status as a per-game badge (installed / not installed / broken runner). The library view is primarily a grid or list with no aggregate health view. Its diagnostic depth is in individual game configuration dialogs, not a dashboard. Key observation: Lutris users are accustomed to per-game diagnostics, not aggregate views. The CrossHook health dashboard is more ambitious than anything Lutris provides.

**Confidence**: Medium — supported by search results showing Lutris favors "diagnostic depth" over aggregate views.

### Heroic Launcher

Heroic focuses on streamlined recovery over diagnostic depth — it simplifies the "something is wrong" state by automatically re-linking or re-downloading rather than exposing detailed path diagnostics. Key observation: Heroic's audience (Steam Deck, casual users) prefers actionable guidance over raw diagnostic data. CrossHook should follow Heroic's lead in making the "Fix" path (navigating to editor) as obvious and low-friction as possible from the dashboard.

**Confidence**: Medium — supported by search results characterizing Heroic's design philosophy.

### System Monitoring Tools (Prometheus/Grafana, Datadog, Checkmk)

These tools establish several patterns directly applicable to CrossHook:

- **Alert fatigue mitigation**: Group similar issues. Don't show 12 individual "missing executable" errors — show "12 profiles missing executable" in the issue breakdown.
- **Trend-over-time**: Trend arrows and failure-count-30d columns are the right granularity for CrossHook (daily frequency data, not millisecond metrics).
- **Auto-retry on failure**: The existing fallback timer in `useProfileHealth` (700ms before triggering batch validate if startup event not received) follows this pattern correctly.

**Confidence**: High — these are established patterns from authoritative monitoring tool documentation.

### Comparison Matrix

| Feature                     | CrossHook Health Dashboard | Steam Library | Lutris  | Heroic |
| --------------------------- | -------------------------- | ------------- | ------- | ------ |
| Aggregate status view       | Planned                    | No            | No      | No     |
| Per-item health badge       | Existing (HealthBadge)     | Partial       | Partial | No     |
| Trend over time             | Planned (trendByName)      | No            | No      | No     |
| Issue categorization        | Planned                    | No            | Partial | No     |
| Gamepad navigation          | Existing + extending       | Yes           | Partial | Yes    |
| Read-only / navigate-to-fix | Planned                    | N/A           | No      | N/A    |

---

## 6. Gamepad Navigation

### Two-Zone Model Extension

The existing two-zone model (`sidebar` = L/R D-pad to switch, `content` = sequential up/down traversal) works for forms and lists. The health dashboard introduces a **data table**, which requires 2D navigation within the content zone.

**Recommended extension**: The health table should be a **sub-zone** within the content zone. When focus is inside the table:

- D-pad Up/Down = previous/next row (not previous/next focusable element)
- D-pad Left/Right = switch columns within a row (only for interactive columns: status, action)
- A button = activate the row's "Open" action (navigate to profile editor)
- Y button = trigger Re-check All (always available, regardless of focused row)
- B button = back to content zone top (focus the first element above the table: filter bar)
- L1/R1 = still cycle sidebar views (existing behavior)

Implement this by wrapping the table in a `data-crosshook-focus-zone="health-table"` container and adding dedicated row-navigation handlers to the table component. The `useGamepadNav` hook's `switchZone` function can be extended to support custom sub-zones.

### D-pad Table Navigation Implementation Pattern

```tsx
// In HealthTable component
const rowsRef = useRef<HTMLTableRowElement[]>([]);
const [focusedRowIndex, setFocusedRowIndex] = useState(-1);

// Handle D-pad up/down when focus is inside the table
function handleTableKeyDown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    const next = Math.min(focusedRowIndex + 1, rows.length - 1);
    rowsRef.current[next]?.focus();
    setFocusedRowIndex(next);
  } else if (event.key === 'ArrowUp') {
    event.preventDefault();
    const prev = Math.max(focusedRowIndex - 1, 0);
    rowsRef.current[prev]?.focus();
    setFocusedRowIndex(prev);
  } else if (event.key === 'Enter' || event.key === ' ') {
    // Activate "Open" for focused row
    activateRow(rows[focusedRowIndex]);
  }
}
```

Each table row should have `tabIndex={focusedRowIndex === index ? 0 : -1}` (roving tabindex pattern). This ensures only one row is in the tab order at a time, which is the standard ARIA grid navigation pattern.

### Focus Management on Sort/Filter

When the user applies a filter or sort, the focused row may move or disappear. Rules:

1. If the previously focused profile still exists in the filtered/sorted result, restore focus to its new row position.
2. If the previously focused profile is filtered out, move focus to the first visible row.
3. Do not return focus to the filter controls — users expect to stay in the table after a sort operation.

### Controller Prompts

The existing `ControllerPrompts` component should be extended to include health-dashboard-specific prompts when the Health page is active:

- A = Open Profile
- Y = Re-check All
- B = Back to filters

Show these prompts only when `controllerMode === true` and the active route is 'health'.

### Steam Deck-Specific Notes

- Minimum 48px row height ensures tappable targets on the touchscreen.
- The Steam Deck's 7-inch display at 1280x800 means the font size for table row content should be at minimum 14px (body) and 12px for secondary/muted text.
- The existing `isSteamDeckRuntime()` detection in `useGamepadNav.ts` correctly identifies the Steam Deck via `(pointer: coarse)` + viewport size + user agent heuristic.

---

## 7. Accessibility

### ARIA Patterns for Data Tables

The profile health table should use `role="grid"` (not `role="table"`) because rows are interactive (activatable to navigate to editor). Grid supports keyboard navigation of cells.

```html
<table role="grid" aria-label="Profile health status" aria-rowcount="N">
  <thead>
    <tr role="row">
      <th role="columnheader" aria-sort="ascending">Profile Name</th>
      <th role="columnheader" aria-sort="none">Status</th>
      ...
    </tr>
  </thead>
  <tbody>
    <tr role="row" tabindex="{0}" aria-label="GameTitle — Broken, 3 issues">
      ...
    </tr>
  </tbody>
</table>
```

Use `aria-rowindex` on each row to maintain correct row context when virtualized (virtualized tables remove rows from DOM, so `aria-rowcount` on `<table>` + `aria-rowindex` on each `<tr>` is required for screen reader context).

### Status Announcements

Use a dedicated `aria-live` region for validation state changes:

```tsx
<div role="status" aria-live="polite" aria-atomic="true" className="sr-only">
  {validationAnnouncement}
</div>
```

Set `validationAnnouncement` to:

- "" (empty) when idle
- "Checking all profiles..." when batch validation starts
- "Validation complete. N broken, N stale, N healthy." when batch validation ends
- "Validation failed: [error message]" on error (use `role="alert"` for failures)

Do **not** set `aria-live="assertive"` for routine status updates — only for failures.

### Screen Reader Table Navigation

Each row's `aria-label` should read the full status summary: `"[ProfileName] — [Status], [IssueCount] issue[s], last launched [relative time]"`. This allows screen reader users to quickly scan rows without entering each cell.

Column headers must have visible text, not just icons. The status badge column header should read "Status", not show a badge icon.

### Keyboard Navigation (Non-Gamepad)

- Tab = move between interactive controls (filter bar, Re-check All button, table)
- When focus enters the table, the first row receives focus automatically
- Arrow keys within the table: per the ARIA grid pattern, Up/Down move between rows, Left/Right move between cells
- Enter / Space = activate focused row (navigate to editor)
- Escape from table = return focus to filter bar
- Home = first row in table
- End = last row in table
- Page Up / Page Down = scroll visible rows (10-row jump recommended)

### Focus Trap Avoidance

The dashboard is a non-modal page — there is no focus trap. Users can freely tab out of the table to the sidebar, filter bar, or other content. Do not implement focus trapping on the table.

### Reduced Motion

Respect `prefers-reduced-motion`. The counting animation on stat cards and the skeleton pulse animation should be disabled:

```css
@media (prefers-reduced-motion: reduce) {
  .health-stat-count {
    transition: none;
  }
  .health-skeleton {
    animation: none;
  }
}
```

---

## 8. Recommendations

### Must Have

1. **Default sort by severity**: Broken first, then stale, then healthy, then alphabetical within each group. Users should see the most actionable items without any interaction.
2. **Immediate cached data display**: Render from `cachedSnapshots` on mount. Never show "0 broken" while validation is running.
3. **Re-check All button + Y button gamepad shortcut**: Prominently placed, clearly disabled during validation, with "Checking..." label during progress.
4. **Roving tabindex for table rows**: Required for both keyboard and gamepad navigation correctness in the data table.
5. **`aria-live="polite"` announcement on validation complete**: Required for WCAG 4.1.3 compliance.
6. **Per-row "Open" action**: Navigates to Profiles page with that profile pre-selected. This is the only exit action from the dashboard.
7. **Status-color left-border accent on stat cards**: Not full background fills. Matches existing HealthBadge pattern.
8. **Skeleton loading state**: Not a full-page spinner. Table rows should show skeleton placeholders during initial validation.

### Should Have

9. **Issue breakdown section**: Aggregate count by issue category (missing_executable, missing_proton, inaccessible_directory, launcher_drift). Collapsible, reuse `CollapsibleSection`.
10. **Failure trend column**: Display `failure_count_30d` from `ProfileHealthMetadata` with the existing `HealthBadge` failure trend indicator.
11. **Filter bar**: Status filter + launch method filter + source filter. All additive. Clear button.
12. **Trend arrows on stat cards**: Comparing current run to cached snapshot, using existing `computeTrend()` logic.
13. **"Last validated: N ago" timestamp**: Below or next to Re-check All button. Use `validated_at` from `HealthCheckSummary`.
14. **Launcher drift summary section**: Collapsible. Shows count of missing / moved / stale launcher states.
15. **Community import health section**: Collapsible. Contextualizes that community imports often need path adjustment.
16. **Row aria-label with full status summary**: Enables efficient screen reader scanning.

### Nice to Have

17. **Virtualization for 50+ profiles**: Only needed if user testing reveals performance issues. Skip for initial implementation; add if profiling shows jank.
18. **Counting animation on stat card numbers**: 300ms ease-in-out count-up when values change. Disable with `prefers-reduced-motion`.
19. **Per-row re-check button**: Allows re-validating a single profile without running batch validation. Low priority — the existing `revalidateSingle()` hook supports it.
20. **Export health report**: "Copy to clipboard" of a plain-text health summary for pasting into bug reports. Lowest priority.

---

## 9. Open Questions

1. **Navigation to editor**: When the user activates a row to open a profile, should the app navigate immediately to the Profiles page with the profile pre-selected, or open a detail drawer/panel within the health dashboard? The research strongly favors navigation-away for simplicity (matches Heroic's recovery-oriented UX), but a drawer would allow quick context switching. Recommend navigation-away as the default; evaluate drawer after user testing.

2. **Validation frequency**: Should the dashboard auto-re-validate on a timer (e.g., every 5 minutes) when it is the active page? The current `useProfileHealth` hook only validates on mount and on explicit trigger. Auto-refresh may be useful for long sessions but risks user distraction. Leave as manual for now; this is a configuration option for later.

3. **Y button conflict**: If Y button is assigned to Re-check All on the Health page, does this conflict with any existing global Y button assignment in `ControllerPrompts`? Verify with the tech designer that per-page button remapping is supported.

4. **Virtualization threshold**: What is the expected maximum profile count for typical users? If most users have 5–20 profiles, virtualization is unnecessary complexity. If power users may have 100+, virtualization is required. Recommend instrumenting profile count in telemetry before deciding.

5. **Issue breakdown granularity**: Should the issue breakdown section link to a filtered table view (e.g., click "12 missing executable" → table filters to show only those profiles)? This requires the filter state to be driven from outside the table component. Consider whether this is worth the wiring complexity for v1.

6. **Trend direction semantics for summary cards**: A broken count going up is "got worse". But for the Healthy count card, a count going up is "got better". The trend arrow on the Healthy card should point up with success color when healthy count increases. This inversion of the trend arrow semantic needs explicit handling separate from the per-row `computeTrend()` logic, which only compares individual profile status rank.

---

## Sources

- [Dashboard Design UX Patterns Best Practices](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards) — Pencil & Paper
- [Designing Better Loading and Progress UX](https://smart-interface-design-patterns.com/articles/designing-better-loading-progress-ux/) — Smart Interface Design Patterns
- [Empty States Pattern](https://carbondesignsystem.com/patterns/empty-states-pattern/) — Carbon Design System
- [ARIA live regions](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Guides/Live_regions) — MDN Web Docs
- [ARIA Grids & Data Tables — Making Complex Interactive Data Accessible](https://www.accesify.io/blog/aria-grids-data-tables-accessibility/) — Accesify
- [WCAG SC 4.1.3: Status Messages](https://www.includia.com/guides/posts/sc-413/) — Includia
- [Status System](https://www.astrouxds.com/patterns/status-system/) — Astro UX Design System
- [React Table Keyboard Navigation & Accessibility](https://www.simple-table.com/blog/mit-licensed-react-tables-accessibility-keyboard-navigation) — Simple Table
- [Gamepad and remote control interactions](https://learn.microsoft.com/en-us/windows/apps/design/input/gamepad-and-remote-interactions) — Microsoft Learn
- [UX Strategies For Real-Time Dashboards](https://www.smashingmagazine.com/2025/09/ux-strategies-real-time-dashboards/) — Smashing Magazine
- [Heroic Games Launcher](https://heroicgameslauncher.com/) — Heroic
- [Lutris vs Heroic Games Launcher comparison](https://theserverhost.com/blog/post/heroic-games-launcher-vs-lutris) — TheServerHost
- [Loading Pattern](https://carbondesignsystem.com/patterns/loading-pattern/) — Carbon Design System
- [Best Practices For Animated Progress Indicators](https://www.smashingmagazine.com/2016/12/best-practices-for-animated-progress-indicators/) — Smashing Magazine
- [Error Message UX, Handling & Feedback](https://www.pencilandpaper.io/articles/ux-pattern-analysis-error-feedback) — Pencil & Paper
