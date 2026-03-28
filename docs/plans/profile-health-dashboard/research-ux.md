# UX Research: Profile Health Dashboard (Second Pass)

**Date**: 2026-03-28 (revised from 2026-03-27)
**Feature**: Profile Health Dashboard — Phase 2 with SQLite Metadata Enrichment
**Researcher**: UX Research Specialist
**Revision note**: This document supersedes the 2026-03-27 version. Sections that remain unchanged from the first pass are marked `[unchanged]`. Sections with new or revised content are marked `[revised]` or `[new]`.

---

## Executive Summary

[revised]

The original spec established the right foundation: inline health badges, progressive disclosure via `CollapsibleSection`, a startup notification banner for broken profiles, and full gamepad navigation. Those decisions remain valid and are preserved here.

This second pass adds a layer of metadata-backed health intelligence enabled by the SQLite metadata store (PRs 89–91). The metadata layer provides three new data signals that change what the health dashboard can show:

1. **Launch history trends** — `query_failure_trends(days)` returns profiles with repeated failures over a window (e.g. last 7 or 30 days). This enables trend indicators: a profile that has failed 4 of the last 5 launches is qualitatively more urgent than one that failed once.
2. **Last-success timestamps** — `query_last_success_per_profile()` returns ISO 8601 timestamps. "Last worked 3 days ago" is now a displayable, accurate data point.
3. **Launcher drift** — `DriftState` (`aligned`, `missing`, `moved`, `stale`) on exported launchers surfaces a separate but related health signal: a launcher script that no longer matches its profile.

Additionally, the metadata layer supports collections and favorites, enabling a **filtered health summary** ("3 of 8 favorites have issues") without any new backend query.

**Critical architectural constraint confirmed**: When the metadata store is unavailable (`MetadataStore::disabled()` or mutex poison), `with_conn` returns `Ok(T::default())` — an empty collection. The UX must treat empty metadata as "no enrichment available" and fall back gracefully to path-health-only badges. This is built into the store; the frontend does not need to detect the failure mode explicitly.

**Confidence**: High — patterns corroborated by Carbon Design System, PatternFly, Cloudscape, Pencil & Paper, and direct codebase analysis.

---

## Teammate Input Synthesis

### Preserved from 2026-03-27 Pass (abridged)

The following decisions from the first pass remain authoritative:

- **3-state health roll-up**: `healthy` / `stale` / `broken` with `HealthIssueKind` at the issue level (`Missing` / `Inaccessible` / `not_configured`)
- **Notification rules**: Banner only for broken. Stale = badge only. Per-session dismiss.
- **Batch complete pattern**: Single `profile-health-batch-complete` event; all badges update atomically.
- **`sanitize_display_path()`**: All paths displayed as `~/...`, applied server-side before IPC.
- **Community-import context note**: Prepend issue list with "This profile was imported…" when `community_tap_url` set and ≥2 `missing` issues.
- **Component reuse**: `CompatibilityBadge` semantic for health badges; `CollapsibleSection` for detail panels; `crosshook-launch-panel__feedback-*` classes for issue layout.
- **Architecture**: Inline in `ProfilesPage` (Option A), not a new tab.
- **Phase 1 CTA**: Prose-only remediation + single "Open Profile" button per broken profile.
- **Phase 2 CTA**: Add `code: Option<String>` to `LaunchValidationIssue` for per-field deep links.

### New in This Pass: Metadata-Enriched Signals

**api-researcher** (expected finding): The two new queries exposed from `MetadataStore` are:

```rust
pub fn query_last_success_per_profile(&self) -> Result<Vec<(String, String)>, MetadataStoreError>
// Returns (profile_name, ISO8601 timestamp) for each profile's most recent succeeded launch

pub fn query_failure_trends(&self, days: u32) -> Result<Vec<FailureTrendRow>, MetadataStoreError>
// Returns profiles with ≥1 failure in the window, with successes/failures counts and failure_modes
```

These are `with_conn`-wrapped — they return empty `Vec` when metadata is unavailable, not errors. The frontend can safely call both unconditionally and treat empty results as "no metadata".

**Launcher drift** is surfaced via `DriftState` on `LauncherRow` entries. The existing `LauncherExport` component and `launcher_store.rs` manage drift detection. Drift state values: `unknown`, `aligned`, `missing`, `moved`, `stale`.

### Tech-Designer Input (2026-03-28): Confirmed API Shape

**tech-designer** has confirmed the enriched IPC shape. Key differences from UX assumptions that affect display:

```typescript
interface ProfileHealthReport {
  name: string;
  status: 'healthy' | 'stale' | 'broken';
  launch_method: string;
  issues: HealthIssue[];
  checked_at: string;
  metadata: ProfileHealthMetadata | null; // null when MetadataStore unavailable
}

interface ProfileHealthMetadata {
  last_success: string | null; // ISO 8601
  failure_count_30d: number; // failures in last 30 days (NOT 7 days)
  total_launches: number; // all-time launch count
  launcher_drift_state: string | null; // 'aligned'|'missing'|'moved'|'stale'|null
  is_community_import: boolean; // source was 'import'
  profile_id: string | null; // stable UUID from metadata DB
}
```

**UX adjustments from tech-designer's shape:**

1. **Window is 30 days, not 7**: All UX copy changes from "last 7 days" to "last 30 days". Threshold for `↑N` chip stays at `failures >= 2` but denominator is 30 days.

2. **`total_launches` is available**: Enable launch count in trend line: "Launched 8 times • 3 failures in last 30 days". Show when `total_launches > 0` and `metadata !== null`.

3. **`metadata: null` wrapper** (not flat optional fields): Check `report.metadata !== null` once. A non-null block with `last_success: null` means "never succeeded." `metadata: null` means "MetadataStore unavailable."

4. **`is_community_import` is in metadata struct**: Apply community-import context note when `metadata !== null && metadata.is_community_import === true`.

5. **"Unconfigured" profiles need a softer badge** (new constraint): When `status === 'broken'` but all issues have `kind === 'not_configured'`, render with `unknown` badge class (muted, not red). Copy: "Profile not configured — no paths set." Does not trigger startup banner and does not count toward the "broken" tally in the summary chip.

   ```typescript
   const isFullyUnconfigured = (report: ProfileHealthReport) =>
     report.status === 'broken' && report.issues.every((i) => i.kind === 'not_configured');
   ```

6. **`trainer.path` and `steam_client_install_path` skipped by health check**: UX must not surface issues for these fields. Any `not_configured` issue for an unvalidated field is advisory-only.

---

## User Workflows

### Primary Flow 1: Startup Health Notification [unchanged]

```
App launches
  └─ Background validation runs for all profiles (on Profiles page mount)
       ├─ [all healthy] → No notification; badges render green
       ├─ [≥1 broken] → Startup banner: "N profiles have broken paths" [Review]
       └─ [stale/unconfigured only] → Badges render, no banner (expected lifecycle noise)
```

Banner only for broken. Dismiss is per-session; re-shows next launch if issues persist.

### Primary Flow 2: Metadata-Enriched Profile List [revised]

```
User opens Profiles page
  └─ invoke('validate_all_profiles') + invoke('get_enrichment_data')
       Profile list renders with composite health badge per entry:
         [Broken ↑3x] My Cyberpunk Trainer   [Edit] [Recheck]
         [Healthy]    Elden Ring + FLiNG      [Edit]
         [Stale]      Witcher 3 Trainer       [Edit] [Recheck]
         [Broken ✦]   Dark Souls Trainer      [Edit] [Recheck]   ← launcher drift
           ↓
       User selects a broken profile (D-pad Down + Confirm)
         └─ Health detail section expands inline (CollapsibleSection)
              Path issues + trend context + last-success timestamp
```

The `↑3x` is a failure-count indicator: "3 failures in the last 30 days." The `✦` is a launcher drift badge overlay. Both are optional overlays on the existing health badge — they appear only when metadata is available and indicates enrichment.

### Primary Flow 3: Drill-Down with Metadata Context [revised]

```
User selects broken profile card
  └─ Detail panel opens (CollapsibleSection)
       Header:   [Broken] profile-name — 2 issues
                 "Last worked: 3 days ago  •  3 failures in last 30 days"
       Issue 1:  ✗ Game executable not found.
                 Re-browse to the current executable or use Auto-Populate.
       Issue 2:  ✗ Trainer path not found.
                 Set a trainer path or remove it.
       Launcher: ⚠ Exported launcher is out of sync — regenerate to update.
                 [Open Profile]   ← single navigation CTA
```

The metadata context ("Last worked: 3 days ago • 3 failures in last 30 days") appears as a secondary line in the detail header, below the issue count. It is omitted entirely when `metadata === null`.

### Primary Flow 4: Manual Re-check [unchanged]

```
User triggers re-check:
  ├─ [Recheck All] button in dashboard header
  │    → All badges reset to spinner → batch-complete fires → badges update atomically
  └─ [Recheck] on individual profile
       → Spinner on that profile row → badge updates in-place
```

### Alternative Flow 1: Startup Notification with No Interaction [unchanged]

User ignores the banner → goes to Launch page → launch validation catches same broken paths in `LaunchPanel` → familiar fatal feedback shown.

### Alternative Flow 2: Filtered Health View by Collection [new]

```
User opens Profiles page → selects "Favorites" or a named collection from filter chips
  └─ Profile list filters to collection members
       Summary chip updates: "3 of 8 favorites have issues"
       Only filtered profiles shown; health badges unchanged
       [Recheck All] rechecks only visible profiles
```

The filtered summary chip ("3 of 8 favorites have issues") is a derived count using existing list state — no new backend query. It counts profiles in the filtered view whose health status is `broken` or `stale`.

### Alternative Flow 3: Fail-Soft Degradation [new]

```
Metadata store unavailable (disabled, path error, mutex poisoned)
  └─ metadata queries return empty Vec (no error propagated)
       Profile list renders with path-health-only badges (no trend indicators)
       "Last worked" timestamps absent — section omitted
       Launcher drift badges absent — section omitted
       No error message to user — the absence of enrichment is silent
```

The fail-soft path is entirely server-side. The frontend receives the same `ProfileHealthResult` shape but without trend/timestamp enrichment fields populated. The UI simply does not render those sections when the data is absent (null/undefined guards).

---

## UI/UX Best Practices

### Enhanced Health Badge UX: Composite Signal Display [new]

The challenge with composite health (path + launch-trend + drift) is information density. Research confirms ([PatternFly Status and Severity](https://www.patternfly.org/patterns/status-and-severity/)) that the correct pattern is **aggregated severity with counts**, not stacked badges.

**Recommended approach: Primary badge + secondary overlay chips**

The primary health badge (`healthy` / `stale` / `broken`) remains the dominant signal and maps directly to the existing `crosshook-compatibility-badge--{rating}` semantic. Secondary signals are compact overlays attached to the badge:

```
[Broken ↑3]   ← primary badge + failure-count chip (only when failures > 1 in window)
[Broken ✦]    ← primary badge + drift chip (only when drift_state != aligned)
[Broken ↑3 ✦] ← all three signals combined
[Healthy]     ← no enrichment overlays (clean)
```

**Why not stack multiple full badges?**
PatternFly recommends against showing multiple severity indicators side-by-side without count labels — it creates ambiguity about whether they represent the same issue or different domains. Keeping one primary badge with compact suffix chips preserves the primary signal while hinting at enrichment detail.

**Failure count indicator** (`↑3`):

- Show only when `failure_count_30d >= 2` (a single failure is noise)
- Threshold is configurable constant `FAILURE_TREND_MIN_COUNT = 2`
- Color follows the parent badge (no additional color; avoids confusion with severity spectrum)
- Text: `↑N` where N is `failure_count_30d`. Up arrow indicates "repeated upward failure trend"
- Tooltip / gamepad detail: "N failures in the last 30 days"
- When `failure_count_30d >= 5`: use `↑5+` to cap (avoids single-digit overflow)

**Launcher drift indicator** (`✦`):

- Show only when at least one exported launcher for this profile has `drift_state IN ('missing', 'moved', 'stale')`
- `unknown` drift state = no indicator (data not yet collected)
- Symbol: `✦` (a distinct non-directional shape to avoid confusion with the trend arrow)
- Color: `--crosshook-color-warning` (#f5c542), regardless of parent badge color
- Tooltip / gamepad detail: "Exported launcher is out of sync"

**Carbon DS rule** ([Carbon Design System](https://carbondesignsystem.com/patterns/status-indicator-pattern/)): Never rely on color alone. Each overlay chip must be readable by screen readers and distinct by shape, not only by color. The `↑` arrow and `✦` diamond are shape-distinct.

### Trend Visualization: Text-Based for Steam Deck [new]

Research surveyed sparkline micro-charts (React Sparklines, MUI X Sparkline, FluentUI SparklineChart). The verdict for Steam Deck: **do not use graphical sparklines in Phase 1**.

Reasons:

1. 1280×800 @ typical viewing distance requires minimum 44px touch targets — a sparkline in a profile row would need to be at least 60×24px, which crowds the row
2. Sparklines are cursor-hover interactive by convention; D-pad navigation cannot efficiently interact with a mini-chart embedded in a list row
3. SVG rendering adds a dependency (or heavy canvas ops) for what is effectively a 3-number summary

**Recommended pattern: Text-based trend summary in the detail panel**

In the profile row: the `↑N` count chip (described above) is the only trend signal in the row.

In the detail panel (on expand):

```
Last worked: 3 days ago  •  Launched 8 times  •  3 failures in last 30 days
```

This is a single text line using `crosshook-launch-panel__feedback-help` muted styling — the same style used for path detail. It communicates the trend without a chart.

**When to show trend context in detail panel** (using `ProfileHealthMetadata` fields):

- "Last worked" — show when `metadata.last_success !== null`
- "Launched N times" — show when `metadata.total_launches > 0` (field now confirmed available)
- "N failures in last 30 days" — show when `metadata.failure_count_30d >= 1`

**Text formulations for trend line:**

| Condition                                     | Display                                                                   |
| --------------------------------------------- | ------------------------------------------------------------------------- |
| `last_success` set, no failures               | "Last worked: 3 days ago • Launched 8 times"                              |
| `last_success` set, failures present          | "Last worked: 3 days ago • Launched 8 times • 3 failures in last 30 days" |
| `last_success` null, failures present         | "Never successfully launched • 3 failures in last 30 days"                |
| `last_success` null, `total_launches > 0`     | "Launched N times, never succeeded"                                       |
| `metadata === null`                           | (omit line entirely)                                                      |
| `metadata !== null` but all fields empty/zero | (omit line entirely)                                                      |

### Last-Success Timestamps: Relative Display [new]

Research confirms ([Cloudscape Timestamps](https://cloudscape.design/patterns/general/timestamps/), [UX Movement](https://uxmovement.com/content/absolute-vs-relative-timestamps-when-to-use-which/)) that relative timestamps are superior for "last worked" type display in list contexts.

**Recommended pattern:**

| Age          | Displayed text                                                                      |
| ------------ | ----------------------------------------------------------------------------------- |
| < 60 seconds | "Just now"                                                                          |
| 1–59 minutes | "N minutes ago"                                                                     |
| 1–23 hours   | "N hours ago"                                                                       |
| 1–6 days     | "N days ago"                                                                        |
| 7–30 days    | "N weeks ago"                                                                       |
| > 30 days    | "Over a month ago" (avoid exact month count — too much precision for "last worked") |
| Never        | "Never successfully launched"                                                       |

**Accessibility rule** (Cloudscape): Wrap in `<time>` element. Set `datetime` to the ISO 8601 string from `query_last_success_per_profile`. Set `title` to the human-readable absolute timestamp for hover/focus access:

```tsx
<time dateTime={lastSuccess} title={new Date(lastSuccess).toLocaleString()}>
  3 days ago
</time>
```

**Implementation note**: The `lastSuccess` ISO 8601 string from the backend is already UTC (`Utc::now().to_rfc3339()`). Use `Date.parse()` for age calculation. No additional library required.

**Do not persist "last checked" timestamps** — the original spec constraint stands. `checked_at` on `ProfileHealthResult` is display-only after a manual recheck. The last-success timestamp from `query_last_success_per_profile` is a launch record from the DB, not a health check record.

### Launcher Drift Indicators: Integrated, Not Separate [new]

The question in the brief: separate badge or integrated? **Integrated** is the right choice.

Rationale:

- Launcher drift is a health signal, not a separate feature. A drifted launcher is a "things are not working correctly" indicator.
- A separate badge column next to the health badge doubles the cognitive load for users who do not export launchers (which may be many Steam Deck users who prefer the native integration).
- The `✦` chip overlaid on the health badge conveys "there is also a launcher issue" without requiring a separate conceptual domain.

**When a launcher is drifted but the profile itself is healthy:**
The health badge stays `healthy`. The drift chip `✦` is appended. This communicates "the profile configuration is fine, but your exported launcher needs regenerating."

```
[Healthy ✦]  My Game  [Edit]
             Detail panel: "Exported launcher is out of sync — regenerate to update."
                           [Open Profile]
```

The drift detail in the panel uses the existing `crosshook-launch-panel__feedback-*` layout with `warning` severity styling.

**Drift state display mapping:**

| `drift_state` | Chip shown    | Detail message                                                                     |
| ------------- | ------------- | ---------------------------------------------------------------------------------- |
| `aligned`     | None          | —                                                                                  |
| `unknown`     | None          | —                                                                                  |
| `missing`     | `✦` (warning) | "Exported launcher file was not found — it may have been deleted"                  |
| `moved`       | `✦` (warning) | "Exported launcher file has moved — regenerate to update"                          |
| `stale`       | `✦` (warning) | "Exported launcher is out of sync with the current profile — regenerate to update" |

### Fail-Soft UX: Silent Absence [new]

The metadata store's `with_conn` design already handles degradation server-side: if the store is disabled or the connection is poisoned, every query returns `Ok(Vec::default())` — an empty collection, no error.

**Frontend rule**: Never display "metadata unavailable" to the user. The absence of enrichment signals is silent. This is the correct pattern because:

1. The user cannot do anything about metadata unavailability
2. An error message about the metadata DB is alarming and confusing for a non-technical user
3. The path-health badge is still accurate — it is the primary product

**Implementation guidance** (using confirmed `ProfileHealthReport` shape):

```typescript
// metadata: null → MetadataStore unavailable → render no enrichment, no error
// metadata !== null → store available; individual fields may still be null

function renderTrendLine(metadata: ProfileHealthMetadata | null): string | null {
  if (!metadata) return null;
  const parts: string[] = [];
  if (metadata.last_success) parts.push(`Last worked: ${formatRelativeTime(metadata.last_success)}`);
  if (metadata.total_launches > 0) parts.push(`Launched ${metadata.total_launches} times`);
  if (metadata.failure_count_30d >= 1) parts.push(`${metadata.failure_count_30d} failures in last 30 days`);
  return parts.length > 0 ? parts.join('  •  ') : null;
}

function shouldShowDriftChip(metadata: ProfileHealthMetadata | null): boolean {
  if (!metadata) return false;
  return (
    metadata.launcher_drift_state !== null &&
    metadata.launcher_drift_state !== 'aligned' &&
    metadata.launcher_drift_state !== 'unknown'
  );
}

function shouldShowTrendChip(metadata: ProfileHealthMetadata | null): boolean {
  if (!metadata) return false;
  return metadata.failure_count_30d >= FAILURE_TREND_MIN_COUNT; // 2
}
// Never render "No metadata available" or similar — absence is always silent
```

### Filtered Health Views: Collection and Favorites Chips [new]

Research ([PatternFly Filters](https://www.patternfly.org/patterns/filters/design-guidelines/), [Pencil & Paper Dashboards](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards)) confirms that filter chips at the top of a list are the correct pattern for segmented views — no separate tab needed.

**Recommended implementation:**

Above the profile list, a filter row:

```
[All]  [Favorites ★]  [My Collection]  [Issues Only]
```

- `All` — default; shows all profiles
- `Favorites ★` — filters to profiles where `is_favorite = true` (from `ProfileRow.is_favorite`)
- Collection names — one chip per user-created collection from `list_collections()`
- `Issues Only` — filters to profiles where `status != 'healthy'`

The filtered summary chip updates dynamically:

- When "Favorites" active: "3 of 8 favorites have issues" (count of broken/stale within favorites)
- When "My Collection" active: "1 of 4 in My Collection has issues"
- When "All" active: existing "N issues" summary

**Gamepad navigation**: Filter chips are horizontally navigable with D-pad Left/Right when the chip row is focused. Standard `useGamepadNav` two-zone model applies — D-pad Down from the chip row moves to the profile list.

**Implementation note**: The filter state lives entirely in React component state. No new Tauri commands needed. `list_favorite_profiles()` and `list_profiles_in_collection(collection_id)` both exist in `MetadataStore`. The favorites list can be fetched alongside the health batch, merged client-side.

### Historical Health UX: Trend Direction Arrows [new]

The brief asks: can we show "health improved/degraded since last check"?

**Answer**: Yes, but only within a session (no cross-session persistence of health snapshots).

**Session-scoped trend arrows** (viable in Phase 1):

- Store the health result of the last batch check in React state
- On the next manual "Recheck All", compare new status to stored status
- Show a micro-indicator in the badge when status changed:
  - `broken` → `healthy`: add `↓` improvement arrow (green) for one render cycle, then revert to clean `healthy` badge
  - `healthy` → `broken`: the badge changing to red is sufficient; no additional arrow needed
  - No change: no arrow

This gives users satisfying feedback ("your recheck fixed 2 profiles") without requiring persistence.

**Cross-session health trends** (defer to later phase):
The metadata layer does not currently persist health snapshots — only launch outcomes. To show "health degraded since last week" would require a new `health_snapshots` table. Defer. The `failures_in_window` count from `query_failure_trends(7)` is a strong enough proxy for now: "3 failures in last 7 days" communicates degradation implicitly.

**Trend direction in the Pencil & Paper pattern** ([source](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards)): Use a delta indicator (arrow + color + count) for unambiguous trend communication. The arrow must have a text label or be accompanied by a count — never arrow-only (accessibility rule: shape+color+text).

---

## Status Indicator Design [revised]

Map existing compatibility-badge vocabulary to health states (4-state model, extended with enrichment overlays):

| Backend State    | Badge Class Suffix | Color Token                 | Icon | Display Label | Enrichment Overlay           |
| ---------------- | ------------------ | --------------------------- | ---- | ------------- | ---------------------------- |
| `healthy`        | `working`          | `--crosshook-color-success` | ✓    | Healthy       | None (or `✦` if drift)       |
| `stale`          | `partial`          | `--crosshook-color-warning` | !    | Stale         | None (or trend chips)        |
| `broken`         | `broken`           | `--crosshook-color-danger`  | ✗    | Broken        | `↑N` if trends, `✦` if drift |
| `not_configured` | `unknown`          | muted text                  | –    | Not set       | None                         |
| `unchecked`      | `unknown`          | muted (spinner)             | …    | Checking…     | None                         |

The enrichment chips (`↑N`, `✦`) are additional `<span>` elements inside the badge `<button>`. They do not replace the primary badge class — they append to it:

```tsx
<span className={`crosshook-compatibility-badge crosshook-compatibility-badge--${badgeClass}`}>
  {icon} {label}
  {failureCount >= 2 && (
    <span className="crosshook-health-badge__trend" aria-label={`${failureCount} failures in last 30 days`}>
      ↑{failureCount > 5 ? '5+' : failureCount}
    </span>
  )}
  {launcherDrift && (
    <span className="crosshook-health-badge__drift" aria-label="exported launcher out of sync">
      ✦
    </span>
  )}
</span>
```

The `aria-label` on each chip gives screen reader users (and the gamepad announce layer) the full meaning without seeing the symbol.

---

## Error Handling UX [unchanged from first pass, with drift row added]

### Error States Table

| Backend State    | Field              | Display label  | User-facing message                                                                     | Remediation action          |
| ---------------- | ------------------ | -------------- | --------------------------------------------------------------------------------------- | --------------------------- |
| `missing`        | game executable    | Missing        | "Game executable not found — file may have moved"                                       | [Open Profile]              |
| `inaccessible`   | game executable    | Inaccessible   | "Game executable exists but cannot be read — check file permissions"                    | [Open Profile]              |
| `not_configured` | trainer            | Not set        | "No trainer configured — profile will launch game only"                                 | [Open Profile] _(advisory)_ |
| `missing`        | trainer            | Missing        | "Trainer executable not found — file may have moved"                                    | [Open Profile]              |
| `missing`        | proton version     | Missing        | "Proton runtime not found — version may have been uninstalled"                          | [Open Profile]              |
| `stale`          | any path           | Stale          | "Path no longer found — game or runtime may have been removed"                          | [Open Profile] [Recheck]    |
| `missing`        | (community import) | Missing + note | "Paths not found — this profile was imported and may need path updates for your system" | [Open Profile]              |
| drift `missing`  | launcher           | Drift          | "Exported launcher file was not found — it may have been deleted"                       | [Open Profile]              |
| drift `stale`    | launcher           | Drift          | "Exported launcher is out of sync — regenerate to update"                               | [Open Profile]              |

All paths: `~/...` notation via `sanitize_display_path()`, applied server-side.

### Message Design Principles [unchanged]

- State what happened, not just that an error occurred.
- Tell the user what to do next — include a specific action.
- Distinguish `missing` from `inaccessible` — different root causes, different fixes.
- For community-imported profiles with many missing paths, prepend the context note.
- Avoid blame language — prefer system-oriented framing.

---

## Performance UX [unchanged]

### Loading Indicators During Batch Validation

1. All profiles render immediately with `[Checking…]` spinner badge.
2. Enrichment fetch (metadata queries) runs in parallel with path validation.
3. When `profile-health-batch-complete` fires, update all badges atomically.
4. Enrichment data merges into the same atomic update — no separate render cycle for metadata.

**api-researcher confirms**: validation is fast (<50ms typical). The enrichment queries (`query_failure_trends`, `query_last_success_per_profile`) are SQLite reads — similarly fast. Both can be fetched in the same `invoke` call or in parallel, merged before the single render.

### Optimistic UI Patterns [unchanged]

When a user manually triggers "Recheck" on a single profile:

1. Immediately set badge to spinner.
2. On result: animate badge to new state.
3. Do NOT reset badge to "unchecked" — users trust "was healthy 3 minutes ago" over "unknown".

---

## Competitive Analysis [revised]

### Steam Library: Verify Integrity of Game Files [unchanged]

- Inline status: No. Batch check: No. Gamepad-accessible: No. Remediation: No (auto-repairs).

### Lutris: Runner/Wine Prefix Checks [unchanged]

- Inline status: Partial. Batch check: No. Gamepad: No. Error messages: technical (no remediation).

### Heroic Games Launcher [unchanged]

- Always-visible badge-on-card validates CrossHook's inline badge approach.
- Context menus not gamepad-navigable — CrossHook's D-pad-first design must keep actions as buttons, never context menus.

### Grafana / Datadog [revised]

- **Trend indicators**: Grafana uses sparklines and delta arrows. For CrossHook's 1280×800 Steam Deck constraint, adopt the **text-based delta pattern** ("+2 failures" vs "-1 from last week") not the sparkline chart.
- **Filtered health views**: Grafana's folder-based filtering (by team, namespace) maps to CrossHook's collections. The "filter chip updates summary count" pattern is directly applicable.
- **Fail-soft**: Grafana shows "No data" panels when a datasource is unavailable — each panel degrades independently. CrossHook should follow this: each profile badge degrades independently. If metadata is unavailable for profile X but available for profile Y, Y gets enrichment and X does not.

### Cloudscape / AWS Console [new]

- **Timestamp patterns** ([Cloudscape Timestamps](https://cloudscape.design/patterns/general/timestamps/)): Relative timestamps in list views ("3 days ago"), absolute in detail tooltips. Pair with `<time datetime="...">` for accessibility.
- **Status indicators**: Inline status in resource tables with aggregated severity counts in the header ("3 critical, 1 warning"). Maps directly to CrossHook's "3 of 8 favorites have issues" summary chip.

### PatternFly (Red Hat Design System) [new]

- **Aggregate status cards** ([PatternFly Aggregate Status](https://pf3.patternfly.org/v3/pattern-library/cards/aggregate-status-card/)): Shows count of resources per severity tier. Directly applicable to CrossHook's batch summary: "2 broken, 1 stale, 5 healthy".
- **Multi-signal aggregation**: PatternFly recommends "when you use multiple severity icons, include a count for each icon." CrossHook should show counts in the summary chip ("2 broken (3 issues)") not just "N profiles have issues."

### Summary Matrix [revised]

| Tool                 | Inline badges | Batch check | Gamepad | Trend indicators | Last-success | Filtered views  | Fail-soft        |
| -------------------- | ------------- | ----------- | ------- | ---------------- | ------------ | --------------- | ---------------- |
| Steam                | No            | No          | No      | No               | No           | No              | N/A              |
| Lutris               | Partial       | No          | No      | No               | No           | No              | No               |
| Heroic               | Yes           | No          | Partial | No               | No           | No              | N/A              |
| Grafana              | Yes           | Yes         | No      | Yes (sparklines) | No           | Yes (folders)   | Yes (per-panel)  |
| Cloudscape           | Yes           | Yes         | No      | No               | Yes          | Yes (filters)   | Partial          |
| **CrossHook target** | **Yes**       | **Yes**     | **Yes** | **Yes (text)**   | **Yes**      | **Yes (chips)** | **Yes (silent)** |

---

## Recommendations

### Must Have [revised]

0. **4-state health model with issue-level distinction** — `healthy` / `stale` / `broken` roll-up; `Missing` / `Inaccessible` / `not_configured` at issue level. Unchanged from first pass.

1. **Inline health badge on every profile row** — `crosshook-compatibility-badge` semantic. Unchanged.

2. **Startup background validation** — batch complete; notification banner only for broken. Unchanged.

3. **Per-issue prose remediation + single "Open Profile" CTA** — Phase 1. Unchanged.

4. **Manual Recheck button** — 48px min height, no hover. Unchanged.

4a. **Community-import context note** — prepend for community-imported profiles with ≥2 missing issues. Unchanged.

5. **Gamepad-accessible health detail** — inline CollapsibleSection, D-pad + A confirm. Unchanged.

**New must-have additions:**

6. **Fail-soft enrichment** — when `metadata === null`, render path-health-only badges silently. No error state surfaced to user. Frontend guards all enrichment renders on `metadata !== null`.

7. **`ProfileHealthReport` confirmed shape** — use the `ProfileHealthMetadata` wrapper from tech-designer. See "Tech-Designer Input" section for authoritative shape. Frontend reads `report.metadata?.last_success`, `report.metadata?.failure_count_30d`, `report.metadata?.launcher_drift_state`.

8. **"Unconfigured" profile softer badge** — detect via `isFullyUnconfigured()` check (all issues `kind === 'not_configured'`). Render `unknown` badge class, not `broken`. Does not trigger startup banner or count toward broken tally. Copy: "Profile not configured — no paths set."

### Should Have [revised]

9. **Last-success relative timestamp in detail panel** — "Last worked: 3 days ago" using `<time datetime="...">` element with absolute title. Show when `metadata?.last_success !== null`.

10. **Failure trend chip on badge** — `↑N` chip using `metadata.failure_count_30d`. Show when `failure_count_30d >= 2`. Text-based, no sparkline.

11. **Launcher drift chip on badge** — `✦` chip (warning color) when `metadata.launcher_drift_state` is `missing`, `moved`, or `stale`. Integrated into health badge, not a separate badge column.

12. **Launcher drift detail in panel** — drift-specific message in the expanded detail. Uses `crosshook-launch-panel__feedback-*` layout with warning severity.

13. **Filter chips for collections/favorites** — `[All]` `[Favorites ★]` `[Collection name]` `[Issues Only]` above the profile list. Summary chip updates to "N of M in [context] have issues." Fully unconfigured profiles count as neither broken nor healthy in the tally.

14. **Session-scoped health improvement indicator** — compare current recheck to previous in-session result. Show `↓` (green) improvement when status changed from broken → healthy for one render cycle.

15. **Batch summary with severity count** — `CollapsibleSection` header meta: "2 broken, 1 stale" (not just "3 issues"). Auto-opens when broken profiles exist.

16. **Progressive badge updates** — spinners → atomic update. Unchanged.

17. **Controller hint overlay** — `Y Re-check` / `X Fix` when broken profile focused. Unchanged.

18. **Silent success** — no notification when all profiles healthy. Unchanged.

### Nice to Have [revised]

18. **"Fix All" quick action** — auto-repair for issues CrossHook can resolve (re-detect Proton, update Steam paths). Unchanged.

19. **Cross-session health snapshots** — "health degraded since last week." Requires new `health_snapshots` table. Defer.

20. **Per-collection health summary panel** — a "Collection Health" view showing badge distribution per collection. Nice if collections feature usage grows.

21. **Failure mode breakdown in trend** — `failure_modes` from `FailureTrendRow` contains comma-separated failure mode strings. Could show "3 failures (2× proton_crash, 1× exit_timeout)" in detail. Low priority; adds complexity.

---

## Open Questions

0. **Auto-revalidate on profile save** — Closed (confirmed by business-analyzer): call `validate_profile(name)` after save resolves. Still pending implementation.

1. **Validation depth** — Closed (shallow: existence + type + permissions). Unchanged.

2. **Stale threshold** — Closed (binary, not time-based). Unchanged.

3. **Auto-repair scope** — Which issues CrossHook can auto-fix without user confirmation. Open; flagged for business-analyzer.

4. ~~**`failures_in_window` window size**~~ — **Closed.** Tech-designer confirmed 30 days (`failure_count_30d`). All UX copy uses "last 30 days."

5. **Notification persistence** — Per-session dismiss. Re-shows next launch if issues persist. Unchanged.

6. **Profile card vs. selector UX** — Current `ProfilesPage` uses `<select>`. Health dashboard assumes a focusable card list. This remains a UI architecture decision for tech-designer.

7. **Drift chip threshold** — Currently: show `✦` for any non-aligned, non-unknown drift state. Should `unknown` drift state (no launcher exported, never tracked) suppress the chip? Yes — `unknown` means "no launcher exported" which is not a problem. Confirmed in implementation guidance above.

8. ~~**Trend window in IPC**~~ — **Closed.** Tech-designer confirmed 30-day window, exposed as `failure_count_30d` in the `ProfileHealthMetadata` struct.

---

## Codebase Observations [revised]

| Existing pattern                                                 | File                        | Health dashboard application                            |
| ---------------------------------------------------------------- | --------------------------- | ------------------------------------------------------- |
| `CompatibilityBadge` + `crosshook-compatibility-badge--{rating}` | `CompatibilityViewer.tsx`   | Health badge primary class (unchanged)                  |
| `severityIcon()` + `data-severity` attributes                    | `LaunchPanel.tsx`           | Health issue icons (extract to `src/utils/severity.ts`) |
| `CollapsibleSection`                                             | `ui/CollapsibleSection.tsx` | Per-profile detail + batch summary panels               |
| `crosshook-launch-panel__feedback-*` classes                     | `LaunchPanel.tsx`           | Remediation message + CTA layout                        |
| `crosshook-rename-toast` + `role="status"` + `aria-live`         | `ProfilesPage.tsx`          | Startup notification banner                             |
| `useGamepadNav` two-zone model                                   | `hooks/useGamepadNav.ts`    | Health content zone; profile cards as focusable units   |
| `--crosshook-touch-target-min: 48px`                             | `variables.css`             | All Recheck and Fix buttons                             |
| `--crosshook-color-success/warning/danger`                       | `variables.css`             | Badge and chip color tokens                             |
| `query_last_success_per_profile()`                               | `metadata/mod.rs:401`       | Last-success timestamp source                           |
| `query_failure_trends(days)`                                     | `metadata/mod.rs:437`       | Failure trend count source (`FailureTrendRow`)          |
| `DriftState` enum (`aligned/missing/moved/stale/unknown`)        | `metadata/models.rs:121`    | Launcher drift chip source                              |
| `MetadataStore::disabled()` + `with_conn` fail-soft              | `metadata/mod.rs:48,67`     | Fail-soft: empty Vec on unavailability                  |
| `list_favorite_profiles()`                                       | `metadata/mod.rs:323`       | Favorites filter chip data                              |
| `list_collections()`                                             | `metadata/mod.rs:262`       | Collection filter chips data                            |

**New CSS classes to add:**

- `crosshook-health-badge__trend` — for `↑N` failure chip inside the health badge
- `crosshook-health-badge__drift` — for `✦` drift chip inside the health badge
- `crosshook-health-filter-chips` — for the filter chip row above the profile list
- `crosshook-health-trend-line` — for the "Last worked: N days ago • M failures" line in detail

---

## Sources

- [Carbon Design System — Status Indicator Pattern](https://carbondesignsystem.com/patterns/status-indicator-pattern/)
- [Carbon Design System — Loading Pattern](https://carbondesignsystem.com/patterns/loading-pattern/)
- [PatternFly — Status and Severity](https://www.patternfly.org/patterns/status-and-severity/)
- [PatternFly — Filters](https://www.patternfly.org/patterns/filters/design-guidelines/)
- [PatternFly — Aggregate Status Card](https://pf3.patternfly.org/v3/pattern-library/cards/aggregate-status-card/)
- [Cloudscape Design System — Timestamps](https://cloudscape.design/patterns/general/timestamps/)
- [Pencil & Paper — Dashboard UX Patterns](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards)
- [Pencil & Paper — Error UX Patterns](https://www.pencilandpaper.io/articles/ux-pattern-analysis-error-feedback)
- [UX Movement — Absolute vs. Relative Timestamps](https://uxmovement.com/content/absolute-vs-relative-timestamps-when-to-use-which/)
- [Heroic Games Launcher](https://heroicgameslauncher.com/)
- [Steam Support — Verify Integrity of Game Files](https://help.steampowered.com/en/faqs/view/0C48-FCBD-DA71-93EB)
- [MDN — :focus-visible](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Selectors/:focus-visible)
- [NN/g — Indicators, Validations, and Notifications](https://www.nngroup.com/articles/indicators-validations-notifications/)
- [Medium — Dashboard Design Principles 2025](https://medium.com/@allclonescript/20-best-dashboard-ui-ux-design-principles-you-need-in-2025-30b661f2f795)
- [Unreal Engine — Gamepad UI Navigation](https://medium.com/@Jamesroha/dev-guide-gamepad-ui-navigation-in-unreal-engine-5-with-enhanced-input-3ab5403f8ab5)
- [UX Patterns for Devs — Progressive Loading](https://uxpatterns.dev/glossary/progressive-loading)
- [Infragistics — Sparkline Charts](https://www.infragistics.com/blogs/html5-sparkline-chart/)
