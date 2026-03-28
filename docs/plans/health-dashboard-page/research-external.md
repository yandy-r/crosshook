# External API & Library Research: health-dashboard-page

## Executive Summary

The Health Dashboard Page is a **purely frontend feature** — all data already exists from Phases A, B, and D via Tauri IPC commands (`batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`). No new backend work is needed.

The key external library questions are:

1. **Sortable/filterable table**: No library needed — a hand-rolled `<table>` with `useMemo` sort/filter is sufficient for CrossHook's small profile counts. Security review confirmed no new dependencies should be introduced; the attack surface of even a small table library is unjustified here.
2. **Trend arrows and stat cards**: No library needed — the existing `HealthBadge.tsx` already renders trend arrows as Unicode characters with CSS color coding. Summary stat cards can be built with raw CSS + existing `crosshook-*` custom properties.
3. **Gamepad navigation**: No new library needed — the project already has a fully featured, production-quality `useGamepadNav` hook (D-pad, analog stick, A/B/L1/R1, zone-aware, Steam Deck detection).
4. **Charts**: No chart library needed — the feature description calls for "trend arrows" (already implemented) and optional bar charts. A bar chart can be rendered as a CSS `<progress>` or a short `<svg>` inline element at effectively zero bundle cost.
5. **Accessibility**: WAI-ARIA `role="grid"` + `aria-sort` patterns applied manually in JSX. No library needed.

**Recommendation: no new dependencies. Everything is satisfied by existing project code, native browser primitives, and a hand-rolled sort/filter with `useMemo`.**

> **Security team ruling (2026-03-28)**: Do not add any table library (`@tanstack/react-table`, `react-data-grid`, etc.). Profile counts are small (unlikely to exceed a few dozen). A hand-rolled `<table>` + `useMemo` is the correct approach. If virtualization ever becomes necessary, `react-window` or `react-virtual` are the pre-approved options. See `docs/plans/health-dashboard-page/research-security.md`.

---

## Primary APIs

### Tauri IPC (existing — no new API)

All required data is already exposed via Tauri commands. No registration, pricing, or rate limit concerns apply — these are local IPC calls.

| Command                       | Return type                                    | Already used?               |
| ----------------------------- | ---------------------------------------------- | --------------------------- |
| `batch_validate_profiles`     | `HealthCheckSummary` / `EnrichedHealthSummary` | Yes (useProfileHealth hook) |
| `get_profile_health`          | `ProfileHealthReport`                          | Yes (useProfileHealth hook) |
| `get_cached_health_snapshots` | `CachedHealthSnapshot[]`                       | Yes (useProfileHealth hook) |

The `useProfileHealth` hook (`src/crosshook-native/src/hooks/useProfileHealth.ts`) already wraps all three commands with loading, error, and caching state. The dashboard page should import `useProfileHealth` directly.

**Confidence**: High — confirmed by reading the source files.

---

## Libraries and SDKs

### 1. Sortable/Filterable Table — Hand-rolled with `useMemo`

**Verdict: No new dependency. Build with React `useMemo` + native `<table>` HTML.**

Security review ruled out all table libraries for this feature. Profile counts are small (5–30 typical, unlikely to exceed a few dozen), making a library an unjustified dependency. The hand-rolled approach is straightforward for this data size.

**Implementation pattern:**

```tsx
type SortKey = 'name' | 'status' | 'issue_count' | 'last_success' | 'launch_method';
type SortDir = 'asc' | 'desc';

const STATUS_RANK: Record<string, number> = { healthy: 0, stale: 1, broken: 2 };

function HealthTable({ profiles, trendByName, onOpenProfile }: HealthTableProps) {
  const [sortKey, setSortKey] = useState<SortKey>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const [filter, setFilter] = useState('');

  const sorted = useMemo(() => {
    const filtered = filter
      ? profiles.filter(
          (p) =>
            p.name.toLowerCase().includes(filter.toLowerCase()) ||
            p.launch_method.toLowerCase().includes(filter.toLowerCase())
        )
      : profiles;

    return [...filtered].sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case 'name':
          cmp = a.name.localeCompare(b.name);
          break;
        case 'status':
          cmp = (STATUS_RANK[a.status] ?? 0) - (STATUS_RANK[b.status] ?? 0);
          break;
        case 'issue_count':
          cmp = a.issues.length - b.issues.length;
          break;
        case 'last_success':
          cmp = (a.metadata?.last_success ?? '').localeCompare(b.metadata?.last_success ?? '');
          break;
        case 'launch_method':
          cmp = a.launch_method.localeCompare(b.launch_method);
          break;
      }
      return sortDir === 'asc' ? cmp : -cmp;
    });
  }, [profiles, sortKey, sortDir, filter]);

  const handleSort = (key: SortKey) => {
    if (key === sortKey) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(key);
      setSortDir('asc');
    }
  };

  const ariaSort = (key: SortKey): 'ascending' | 'descending' | 'none' =>
    sortKey === key ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none';

  return (
    <>
      <input
        type="search"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        placeholder="Filter profiles..."
        aria-label="Filter profile health table"
      />
      <table role="grid" aria-label="Profile health">
        <thead>
          <tr>
            {(['name', 'status', 'issue_count', 'last_success', 'launch_method'] as SortKey[]).map((key) => (
              <th
                key={key}
                aria-sort={ariaSort(key)}
                onClick={() => handleSort(key)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') handleSort(key);
                }}
                tabIndex={0}
                style={{ cursor: 'pointer' }}
              >
                {key.replace(/_/g, ' ')}
                {sortKey === key ? (sortDir === 'asc' ? ' ↑' : ' ↓') : ''}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((profile) => (
            <tr
              key={profile.name}
              tabIndex={0}
              role="row"
              onClick={() => onOpenProfile(profile.name)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') onOpenProfile(profile.name);
              }}
            >
              <td>{profile.name}</td>
              <td>
                <HealthBadge report={profile} trend={trendByName[profile.name]} />
              </td>
              <td>{profile.issues.length}</td>
              <td>{profile.metadata?.last_success ?? '—'}</td>
              <td>{profile.launch_method}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}
```

This covers all dashboard requirements (sort by any column, global text filter, ARIA sort indicators, gamepad-compatible row tabIndex) with zero new dependencies.

**Confidence**: High — appropriate for the data size, confirmed by security review.

---

### 2. Gamepad Navigation

**Verdict: No new library needed — use the existing `useGamepadNav` hook.**

The project already has a production-quality gamepad navigation implementation at:
`src/crosshook-native/src/hooks/useGamepadNav.ts`

It covers:

- `requestAnimationFrame` polling of `navigator.getGamepads()`
- D-pad up/down/left/right — focus navigation
- Analog stick axes with threshold activation
- A (button 0) = confirm/click, B (button 1) = back
- L1/R1 (buttons 4/5) = cycle sidebar tabs
- Keyboard fallback: Arrow keys, Tab, Enter, Space, Escape (all in capture phase)
- Steam Deck detection via UA + media queries + env vars
- Two-zone focus model (sidebar / content) with `data-crosshook-focus-zone` attributes
- Modal overlay priority via `data-crosshook-focus-root="modal"`

For the HealthDashboardPage, the table rows must be `tabIndex={0}` and the content area must use `data-crosshook-focus-zone="content"`. The "Y button to re-check" feature maps to button index 3 (Y on Xbox layout) — a `useEffect` listening to button 3 edge in the gamepad poll, or a top-level button with `tabIndex={0}` in the content zone is the simpler path.

**Researched alternatives (for reference):**

- `react-ts-gamepads` (npm: `react-ts-gamepads`): TypeScript fork of react-gamepads, actively maintained. Would duplicate the existing hook. **Not needed.**
- `react-gamepads`: Unmaintained (5 years, no updates). **Do not use.**
- `react-gamepad`: Unmaintained (8 years). **Do not use.**
- Raw `Navigator.getGamepads()` API: Already what `useGamepadNav` uses. Supported in all modern Chromium-based WebViews (which Tauri v2 uses). **No browser compat concern.**

**Confidence**: High — hook source confirmed by reading the file.

---

### 3. Trend Arrows and Status Indicators

**Verdict: No new library needed — use existing `HealthBadge.tsx` + pure CSS/Unicode.**

The existing `HealthBadge` component (`src/crosshook-native/src/components/HealthBadge.tsx`) already renders:

- Status chip with `crosshook-status-chip crosshook-compatibility-badge--{rating}` classes
- Trend arrows via Unicode `↑` / `↓` characters with `var(--crosshook-color-warning)` / `var(--crosshook-color-success)` colors
- Failure count badges
- ARIA labels

For summary stat cards (total/healthy/stale/broken counts), a simple `<div>` with existing CSS custom properties is sufficient:

```tsx
function StatCard({ label, count, colorVar }: { label: string; count: number; colorVar: string }) {
  return (
    <div className="crosshook-panel crosshook-stat-card" style={{ borderTop: `3px solid var(${colorVar})` }}>
      <span className="crosshook-stat-card__count" aria-label={`${count} ${label}`}>
        {count}
      </span>
      <span className="crosshook-stat-card__label">{label}</span>
    </div>
  );
}
```

No chart library (Recharts, Chart.js, D3) is needed for trend arrows — these are already Unicode arrows. If a bar chart for "issue breakdown by category" is wanted later, an inline `<svg>` or CSS `width` on a `<div>` bar is sufficient and adds zero bundle weight.

**Confidence**: High — HealthBadge source confirmed, CSS variables confirmed.

---

### 4. Accessibility

**Verdict: Implement WAI-ARIA patterns manually. No library needed.**

TanStack Table is headless — it does not add ARIA roles automatically. The grid pattern must be applied by the developer. Key rules:

- `<table role="grid">` with `aria-label`
- `<th aria-sort="ascending|descending|none">` for sortable columns
- `<tr tabIndex={0}>` on data rows for keyboard/gamepad navigation
- Roving tabindex within the table body for arrow key cell navigation (optional for this page — row-level focus is sufficient)
- `aria-live="polite"` region for the loading/re-check state announcements

**Reference**: WAI-ARIA Authoring Practices Guide (APG) — [Grid Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/grid/) and [Sortable Table example](https://www.w3.org/WAI/ARIA/apg/patterns/table/examples/sortable-table/).

**React Aria** (`@react-aria/table`): Provides accessible table primitives and integrates with TanStack Table. However, it adds ~80 kB gzipped and requires restructuring the rendering model. Given the project already has accessibility-aware components and keyboard navigation via `useGamepadNav`, the manual ARIA approach is lower cost with the same outcome.

**Confidence**: High — WAI-ARIA APG patterns are stable and official.

---

## Integration Patterns

### Pattern 1: Health Dashboard as read-only consumer

The dashboard page imports `useProfileHealth` directly and uses the returned `summary`, `loading`, `error`, `trendByName`, `staleInfoByName`, and `batchValidate`. It does **not** call `revalidateSingle` (that is for the profile editor on save). The "Re-check All" button calls `batchValidate()`.

```tsx
// src/crosshook-native/src/components/pages/HealthDashboardPage.tsx
import { useProfileHealth } from '../../hooks/useProfileHealth';
import { HealthBadge } from '../HealthBadge';
import { CollapsibleSection } from '../ui/CollapsibleSection';

export function HealthDashboardPage() {
  const { summary, loading, error, trendByName, batchValidate } = useProfileHealth();
  // ... sortKey, sortDir, filter state — all local React state + useMemo
}
```

### Pattern 2: Table row navigation for gamepad

Each data row must be a focusable element (`tabIndex={0}`) within `data-crosshook-focus-zone="content"`. The existing `useGamepadNav` will then automatically include rows in its D-pad traversal. The "A" button (confirm) triggers `row.click()` which can navigate to the ProfileEditor.

```tsx
<tbody>
  {table.getRowModel().rows.map((row) => (
    <tr
      key={row.id}
      tabIndex={0}
      role="row"
      onClick={() => onOpenProfile(row.original.name)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') onOpenProfile(row.original.name);
      }}
    >
      ...
    </tr>
  ))}
</tbody>
```

### Pattern 3: Derived stats from existing summary

No new API calls for stat cards — derived from `HealthCheckSummary` fields that already exist:

```tsx
const stats = {
  total: summary.total_count,
  healthy: summary.healthy_count,
  stale: summary.stale_count,
  broken: summary.broken_count,
};
```

The "recent failures" panel (profiles with failures in last 30 days) is derived from `EnrichedProfileHealthReport.metadata.failure_count_30d > 0`.

The "launcher drift" summary derives from `metadata.launcher_drift_state !== null`.

The "community import health" panel filters on `metadata.is_community_import === true`.

---

## Constraints and Gotchas

### Hand-rolled sort/filter table

- **ARIA must be explicit**: Add `role="grid"`, `aria-sort` on sortable `<th>`, `aria-label` on the `<table>`. Without these the table passes visual review but fails accessibility audits.
- **`useMemo` dependencies**: The `sorted` memo must include `profiles`, `sortKey`, `sortDir`, and `filter` in the dependency array. Missing any causes stale renders on profile updates (e.g. after re-check).
- **Reference stability for `profiles` prop**: If the parent re-renders and passes a new array reference with the same content, `useMemo` will recompute needlessly. This is harmless for the expected data sizes.
- **Virtualization if needed**: If profile counts ever grow large, `react-window` and `react-virtual` are the security-approved options. Do not add any other virtualization library. Not a concern for v1.
- **Column resize**: Not needed for Steam Deck (fixed 1280x800 viewport).

### Gamepad Y button mapping

The "Y to re-check" requirement (from the feature spec) maps to button index 3 in the standard gamepad mapping (Xbox layout: A=0, B=1, X=2, Y=3). The existing `useGamepadNav` hook does not expose a callback for button 3. Options:

1. Add a `onY?: () => void` callback to `GamepadNavOptions` in `useGamepadNav.ts`
2. Add a separate `useEffect` in `HealthDashboardPage` that polls `navigator.getGamepads()` for button 3 (simpler, avoids modifying shared hook)

Option 2 is lower-risk for v1 since it doesn't change the shared hook's API.

### Hook type mismatch — pre-implementation blocker

The existing `useProfileHealth` hook has incorrect TypeScript generic types on its IPC calls. The backend returns `EnrichedHealthSummary` / `EnrichedProfileHealthReport` but the hook is typed as the narrower base types. `metadata` is present at runtime but invisible to the TypeScript compiler, so the dashboard page cannot access it without a type error.

**This must be fixed in `useProfileHealth.ts` before the dashboard page is built.** Required changes:

| Call site                                           | Current type                          | Correct type                                  |
| --------------------------------------------------- | ------------------------------------- | --------------------------------------------- |
| `invoke<...>("batch_validate_profiles")` (L127)     | `HealthCheckSummary`                  | `EnrichedHealthSummary`                       |
| `invoke<...>("get_profile_health", ...)` (L143)     | `ProfileHealthReport`                 | `EnrichedProfileHealthReport`                 |
| `listen<...>("profile-health-batch-complete", ...)` | `HealthCheckSummary`                  | `EnrichedHealthSummary`                       |
| `ProfileHealthState.summary`                        | `HealthCheckSummary \| null`          | `EnrichedHealthSummary \| null`               |
| `healthByName` return type                          | `Record<string, ProfileHealthReport>` | `Record<string, EnrichedProfileHealthReport>` |

No new IPC command needed. No backend change. Frontend-only type correction. The `EnrichedProfileHealthReport.metadata` fields remain nullable (`| null`) after this fix — null guards are still required throughout.

### CollapsibleSection inside table

The `CollapsibleSection` component uses `<details>/<summary>` — it must **not** be placed inside a `<table>` element (invalid HTML). Use it for the panels surrounding the table (Recent Failures panel, Launcher Drift Summary, Community Import Health), not inside table rows.

---

## Confirmed Backend Details (source-verified 2026-03-28)

The following were confirmed by reading `src-tauri/src/commands/health.rs` and `src-tauri/src/lib.rs` directly — these close the previously listed open questions.

### `batch_validate_profiles` returns `EnrichedHealthSummary`

**Confirmed.** The command signature is:

```rust
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<EnrichedHealthSummary, String>
```

It calls `build_enriched_health_summary` which runs `batch_check_health` (filesystem I/O per profile) then enriches each report with metadata (SQLite queries). The frontend type in `health.ts` correctly matches the backend struct via `#[serde(flatten)]` on the `core: ProfileHealthReport` field.

**Important serialization note**: `EnrichedProfileHealthReport` uses `#[serde(flatten)]` on the `core` field. This means all `ProfileHealthReport` fields (`name`, `status`, `launch_method`, `issues`, `checked_at`) are serialized at the top level alongside `metadata`. The TypeScript type `EnrichedProfileHealthReport extends ProfileHealthReport` already reflects this correctly.

### `profile-health-batch-complete` event emits `EnrichedHealthSummary`

**Confirmed.** In `lib.rs`, the startup health scan (500 ms after app start) calls `build_enriched_health_summary` and emits the result as `profile-health-batch-complete`. The `useProfileHealth` hook listens for this event correctly. The event payload type is the same `EnrichedHealthSummary` as the IPC command — no type mismatch.

### `batch_validate_profiles` I/O cost and re-check rate limiting

**Confirmed by source inspection.** `batch_check_health` performs `fs::metadata()` for every configured path in every profile — roughly 3–6 syscalls per profile depending on launch method:

- `game.executable_path` (always) — 1 syscall
- `trainer.path` (if set) — 1 syscall
- Each `injection.dll_paths` entry — 1 syscall each
- `steam.compatdata_path` + `steam.proton_path` (steam_applaunch) OR `runtime.prefix_path` + `runtime.proton_path` (proton_run) — 2 syscalls
- `steam.launcher.icon_path`, `runtime.working_directory` (if set) — up to 2 more

For a typical 20-profile collection this is ~120 synchronous `fs::metadata` calls on the Tauri async runtime thread, plus the SQLite queries in `prefetch_batch_metadata` (6 bulk queries, not per-profile).

**Implication for the dashboard UI**: `batchValidate()` blocks the IPC thread during the filesystem scan. The existing 700 ms startup fallback in `useProfileHealth` is appropriate. For the dashboard's "Re-check All" button, debounce the call to prevent accidental double-trigger (300–500 ms debounce is sufficient). The existing `loading: state.status === "loading"` guard in the hook already prevents concurrent invocations.

**No rate-limit mechanism exists in the backend** — repeated rapid calls will each do the full filesystem scan. The frontend must prevent this via the loading guard and button debounce.

### Error handling pattern: `Result<T, String>`

**Confirmed.** All three health IPC commands return `Result<T, String>` on the Rust side. The `useProfileHealth` hook already catches these with try/catch and dispatches `{ type: "error", message: normalizeError(error) }`. The dashboard page should surface `error` state from the hook rather than adding its own error handling.

### `MetadataStore` fail-soft behavior

**Confirmed.** When SQLite is unavailable (`MetadataStore::disabled()`), `batch_validate_profiles` still succeeds but returns `metadata: None` for every profile. The `prefetch_batch_metadata` function returns `BatchMetadataPrefetch::default()` (all empty HashMaps) and sets `metadata_available: false`, causing `enrich_profile` to set `metadata: None`. The dashboard must handle `metadata === null` throughout — do not assume metadata is always present.

---

## Open Questions

1. **Y button (button 3) implementation strategy**: Add a `onY?: () => void` callback to `GamepadNavOptions` in `useGamepadNav.ts`, or handle locally in `HealthDashboardPage` with a separate `useEffect` polling button 3. The local approach is lower-risk for v1 since it avoids changing the shared hook API.

2. **Should the health table support multi-select + bulk re-check?** The feature spec says "Re-check All" triggers `batchValidate()`. If per-profile re-check from the table is wanted, `revalidateSingle` is available but the spec says fix actions navigate to the profile editor.

3. **Pagination or virtual scroll?** Unlikely to be needed — typical CrossHook users have 5–30 profiles. Default to no pagination unless profile count consistently exceeds 50.

---

## Search Queries Executed

1. `TanStack Table v8 React sortable filterable table 2024 2025`
2. `React gamepad navigation library Steam Deck controller input 2024`
3. `React dashboard stat card component library lightweight 2024`
4. `TanStack Table v8 npm package size bundle weight headless`
5. `React accessibility data table ARIA keyboard navigation WAI-ARIA pattern 2024`
6. `Recharts trend sparkline arrow indicator React TypeScript lightweight`
7. `AG Grid community vs TanStack Table React comparison bundle size 2024`
8. `Web Gamepad API browser support navigator.getGamepads keyboard fallback React hook 2024`
9. `CSS only trend arrow indicator up down neutral no library pure React TypeScript`
10. `react-gamepads useGamepad npm version maintenance status 2024 2025`
11. `react-ts-gamepads npm TypeScript Gamepad API hook active 2024`
12. `@tanstack/react-table v8 current version npm 2025`
13. `HTML details summary native collapsible sortable table no library keyboard accessible`
14. `TanStack Table v8 keyboard navigation ARIA grid pattern implementation example`

---

## Sources

- [MDN — Gamepad API](https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API)
- [MDN — Navigator.getGamepads()](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/getGamepads)
- [react-ts-gamepads — GitHub](https://github.com/nogiszd/react-ts-gamepads) (evaluated, not adopted)
- [react-gamepads — GitHub](https://github.com/whoisryosuke/react-gamepads) (evaluated, inactive, 5 years)
- [WAI-ARIA APG — Grid Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/grid/)
- [WAI-ARIA APG — Sortable Table Example](https://www.w3.org/WAI/ARIA/apg/patterns/table/examples/sortable-table/)
- [W3Schools — CSS Arrows](https://www.w3schools.com/howto/howto_css_arrows.asp)
- [Simple Table — React Data Grid Bundle Size Comparison 2025](https://www.simple-table.com/blog/react-data-grid-bundle-size-comparison) (background research)
- [TanStack Table — Introduction](https://tanstack.com/table/v8/docs/introduction) (evaluated, not adopted per security review)
