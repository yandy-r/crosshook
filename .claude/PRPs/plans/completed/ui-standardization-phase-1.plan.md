# Plan: UI Standardization Phase 1 — Route Banner Baseline

## Summary

Implement a shared, top-level route banner contract across all primary routes so page identity, spacing, and visual hierarchy are consistent. The work introduces one reusable banner component, wires it into every top-level route, and removes conflicting route-intro treatments where they duplicate the same purpose. This phase is scoped to UI runtime behavior only (no persistence changes) and preserves the existing route scroll-shell contract.

## User Story

As a CrossHook user, I want every route to present the same top-of-page banner structure, so that navigation feels consistent and each page’s purpose is immediately clear.

## Problem → Solution

Routes currently mix multiple intro styles (hero strip, panel header, no route intro) → define and apply one shared route banner component and normalize route-level spacing/typography/icon treatment across Library, Profiles, Launch, Install, Community, Discover, Compatibility, Settings, and Health.

## Metadata

- **Complexity**: Large
- **Source PRD**: N/A (GitHub issue driven)
- **PRD Phase**: `#163` Phase 1 (`#160`)
- **Estimated Files**: 12-16
- **Issue**: [#163](https://github.com/yandy-r/crosshook/issues/163), [#160](https://github.com/yandy-r/crosshook/issues/160)

---

## UX Design

### Before

```text
┌───────────────────────────── Route Content ─────────────────────────────┐
│ Profiles: custom hero strip                                             │
│ Launch: launch-panel title strip                                        │
│ Settings/Community/Compatibility: in-card header only                   │
│ Library/Discover: no standardized route identity banner                 │
└──────────────────────────────────────────────────────────────────────────┘
```

### After

```text
┌────────────────────────────── Shared Route Banner ──────────────────────┐
│ [Route art/icon]  ROUTE EYEBROW                                         │
│                    Route Title                                           │
│                    Route summary/intent                                  │
└──────────────────────────────────────────────────────────────────────────┘
┌────────────────────────────── Route Body (existing) ─────────────────────┐
│ Existing cards/panels/subtabs with normalized spacing beneath banner     │
└──────────────────────────────────────────────────────────────────────────┘
```

### Interaction Changes

| Touchpoint            | Before                                       | After                                                                 | Notes                                               |
| --------------------- | -------------------------------------------- | --------------------------------------------------------------------- | --------------------------------------------------- |
| Route identity at top | Mixed/duplicated patterns                    | Single `RouteBanner` contract                                         | Applied to all top-level routes                     |
| Banner visual height  | Varies by route                              | Normalized to sidebar brand rhythm                                    | Mirrors sidebar brand card proportions              |
| Intro duplication     | Route + panel headers overlap on some routes | Route-level intro is unified; panel headers become section-level only | Avoid “double hero” perception                      |
| Scroll behavior       | Route-specific risk when adding top content  | Scroll-shell contract explicitly preserved                            | Must keep `crosshook-route-stack` invariants intact |

---

## Mandatory Reading

| Priority       | File                                                         | Lines                   | Why                                                                      |
| -------------- | ------------------------------------------------------------ | ----------------------- | ------------------------------------------------------------------------ |
| P0 (critical)  | `src/crosshook-native/src/components/layout/ContentArea.tsx` | 20-70                   | Route entrypoint and top-level page composition                          |
| P0 (critical)  | `src/crosshook-native/src/components/layout/Sidebar.tsx`     | 15-72, 113-143          | Canonical route list + brand banner content                              |
| P0 (critical)  | `src/crosshook-native/src/styles/layout.css`                 | 119-182                 | Route shell/stack/body/card scroll contract that cannot regress          |
| P0 (critical)  | `src/crosshook-native/src/hooks/useScrollEnhance.ts`         | 5-10, 78-93             | Scroll owner selector contract; new scroll containers must be compatible |
| P1 (important) | `src/crosshook-native/src/styles/sidebar.css`                | 15-31, 21-23, 251-254   | Sidebar brand card visual baseline (height/padding/radius)               |
| P1 (important) | `src/crosshook-native/src/styles/theme.css`                  | 387-421, 553-586        | Existing heading/hero styling and typography tokens                      |
| P1 (important) | `src/crosshook-native/src/components/pages/ProfilesPage.tsx` | 573-690                 | Most complex current route-intro implementation                          |
| P1 (important) | `src/crosshook-native/src/components/LaunchPanel.tsx`        | 727-739                 | Launch route intro strip pattern to reconcile                            |
| P1 (important) | `src/crosshook-native/src/components/SettingsPanel.tsx`      | 897-906                 | In-card heading stack used in route body cards                           |
| P2 (reference) | `src/crosshook-native/src/components/pages/*.tsx`            | all route return blocks | Apply banner uniformly across all top-level pages                        |
| P2 (reference) | `src/crosshook-native/src/components/layout/PageBanner.tsx`  | 1-162                   | Existing per-route artwork exports for reuse                             |

## External Documentation

| Topic | Source | Key Takeaway                                                                                          |
| ----- | ------ | ----------------------------------------------------------------------------------------------------- |
| N/A   | N/A    | No external research needed — feature uses established internal patterns and existing React/CSS stack |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

// SOURCE: `src/crosshook-native/src/components/layout/Sidebar.tsx:15-33`

```tsx
export type AppRoute =
  | 'library'
  | 'profiles'
  | 'launch'
  | 'install'
  | 'community'
  | 'discover'
  | 'compatibility'
  | 'settings'
  | 'health';

interface SidebarSection {
  label: string;
  items: SidebarSectionItem[];
}
```

Use `PascalCase` for components/types, `camelCase` for functions/constants in local scope, and `crosshook-*` BEM-like class names.

### ERROR_HANDLING

// SOURCE: `src/crosshook-native/src/components/pages/SettingsPage.tsx:40-44`

```tsx
{
  settingsError ? (
    <div className="crosshook-error-banner crosshook-error-banner--section" role="alert">
      {settingsError}
    </div>
  ) : null;
}
```

Render UI errors as alert banners in-place; avoid throwing from render paths.

### LOGGING_PATTERN

// SOURCE: `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx:1071-1076`

```tsx
function handleRetry() {
  if (error) {
    console.error('Health scan error (retrying):', error);
  }
  void batchValidate();
}
```

For UI recoverable faults, log with `console.error` and continue with non-blocking retry behavior.

### ROUTE_LAYOUT_CONTRACT

// SOURCE: `src/crosshook-native/src/styles/layout.css:128-162`

```css
.crosshook-page-scroll-shell--fill {
  height: 100%;
  max-height: 100%;
  min-height: 0;
  overflow: hidden;
}

.crosshook-route-stack {
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-height: 0;
}
```

Any new route banner must fit this contract and not create nested uncontrolled scroll.

### ROUTE_DECOR_PATTERN

// SOURCE: `src/crosshook-native/src/components/layout/PanelRouteDecor.tsx:8-14`

```tsx
export function PanelRouteDecor({ illustration }: PanelRouteDecorProps) {
  return (
    <div className="crosshook-panel-route-decor" aria-hidden="true">
      <div className="crosshook-panel-route-decor__glow" />
      <div className="crosshook-panel-route-decor__art">{illustration}</div>
    </div>
  );
}
```

Decorative artwork is non-interactive and `aria-hidden`.

### TEST_STRUCTURE

// SOURCE: `src/crosshook-native/package.json:6-10`

```json
"scripts": {
  "dev": "vite",
  "build": "tsc && vite build",
  "preview": "vite preview"
}
```

No configured frontend unit test framework in this repo; verification relies on compile/build plus manual route checks.

---

## Files to Change

| File                                                                | Action | Justification                                                                |
| ------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/RouteBanner.tsx`        | CREATE | New shared top-level route banner component                                  |
| `src/crosshook-native/src/components/layout/routeMetadata.ts`       | CREATE | Single source-of-truth for route labels/summaries/art mapping                |
| `src/crosshook-native/src/components/layout/PageBanner.tsx`         | UPDATE | Reuse existing art exports and optionally add route banner metadata mapping  |
| `src/crosshook-native/src/styles/theme.css`                         | UPDATE | Add shared route-banner styling and route-specific spacing harmonization     |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`         | UPDATE | Inject route banner at top-level route content                               |
| `src/crosshook-native/src/components/pages/ProfilesPage.tsx`        | UPDATE | Replace/reshape current hero strip to align with shared route banner         |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx`          | UPDATE | Add route banner above launch stack; remove duplicated route-intro treatment |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`         | UPDATE | Add route banner above install tabs shell                                    |
| `src/crosshook-native/src/components/pages/CommunityPage.tsx`       | UPDATE | Add route banner and preserve existing card scrolling                        |
| `src/crosshook-native/src/components/pages/DiscoverPage.tsx`        | UPDATE | Add route banner for current route lacking consistent identity               |
| `src/crosshook-native/src/components/pages/CompatibilityPage.tsx`   | UPDATE | Add route banner while preserving subtab structure                           |
| `src/crosshook-native/src/components/pages/SettingsPage.tsx`        | UPDATE | Add route banner while retaining settings error handling                     |
| `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx` | UPDATE | Add route banner and adjust health-specific top padding handling             |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`            | UPDATE | Consume shared route metadata for label parity with banner                   |
| `src/crosshook-native/src/components/LaunchPanel.tsx`               | UPDATE | Demote route-level title strip to section-level content as needed            |
| `src/crosshook-native/src/components/SettingsPanel.tsx`             | UPDATE | Ensure heading hierarchy still makes sense under new route banner            |
| `src/crosshook-native/src/components/CommunityBrowser.tsx`          | UPDATE | Same heading hierarchy alignment under route banner                          |
| `src/crosshook-native/src/components/CompatibilityViewer.tsx`       | UPDATE | Same heading hierarchy alignment under route banner                          |

## NOT Building

- Wizard rework and profile field parity changes (Phase 2 / issue `#161`)
- Install flow parity redesign beyond route-level banner baseline (Phase 3 / issue `#162`)
- Any persistence/data model changes (TOML, SQLite, metadata schema)
- Sidebar navigation IA or route taxonomy changes
- New feature flags for this rollout
- Full visual redesign of inner cards; only route-level standardization and spacing consistency

---

## Step-by-Step Tasks

### Task 1: Define route banner contract component

- **ACTION**: Create `RouteBanner` in `components/layout` with props for route id, title, summary, and illustration.
- **IMPLEMENT**: Build a single reusable component using existing panel/card/decor visual language; keep illustration `aria-hidden`; expose a compact API so pages only pass route key or metadata object.
- **MIRROR**: `ROUTE_DECOR_PATTERN`, `NAMING_CONVENTION`.
- **IMPORTS**: `PanelRouteDecor`, route art exports from `PageBanner`, `AppRoute` type if needed.
- **GOTCHA**: Do not introduce a new scroll container inside the banner; support icon-only fallback for any route without dedicated art (or add missing art export).
- **VALIDATE**: TypeScript build passes and component renders in at least one page without style regressions.

### Task 1.5: Centralize route metadata for sidebar/banner parity

- **ACTION**: Create `routeMetadata.ts` and wire both `Sidebar` and `RouteBanner` to consume it.
- **IMPLEMENT**: Move/derive route labels from one shared map (`library`, `profiles`, `launch`, `install`, `community`, `discover`, `compatibility`, `settings`, `health`) including banner title, summary, and art key.
- **MIRROR**: `AppRoute` union from `Sidebar.tsx`.
- **IMPORTS**: `AppRoute`, art exports from `PageBanner`.
- **GOTCHA**: Preserve current product wording differences intentionally (e.g., `community` route label shown as “Browse” in navigation if desired) but make this explicit in metadata.
- **VALIDATE**: Sidebar labels and route banner titles resolve from same source data.

### Task 2: Establish shared banner CSS + token-driven dimensions

- **ACTION**: Add route-banner styles in `theme.css` (or dedicated style file if project convention prefers central theme).
- **IMPLEMENT**: Define height/padding/radius/typography so banner aligns to sidebar brand rhythm (using existing CSS variables and sidebar measurements as baseline).
- **MIRROR**: `sidebar.css` brand style (`min-height`, gradients, radius), `theme.css` heading tokens.
- **IMPORTS**: Existing CSS vars only (`--crosshook-*`), no hardcoded one-off values unless justified.
- **GOTCHA**: Keep desktop + Steam Deck responsive behavior and avoid duplicate “hero” vertical space.
- **VALIDATE**: Verify banner height and spacing on standard viewport and `max-height: 820px` media scenario.

### Task 3: Wire route banner into all top-level page shells

- **ACTION**: Insert `RouteBanner` in each top-level page component before primary route body content.
- **IMPLEMENT**: Update all nine routes (`library`, `profiles`, `launch`, `install`, `community`, `discover`, `compatibility`, `settings`, `health`) so route-level identity is consistent and route metadata copy is explicit.
- **MIRROR**: Existing page shell pattern:
  - `crosshook-page-scroll-shell`
  - `crosshook-route-stack`
  - `crosshook-route-stack__body--fill` or `--scroll`
- **IMPORTS**: `RouteBanner` + per-route art mapping.
- **GOTCHA**: Preserve each route’s existing content ownership for scroll (`route-card-scroll`, subtab panels). `HealthDashboardPage` is a special-case route and must keep its current shell strategy (do not force it into `route-stack__body--fill` refactors during this phase).
- **VALIDATE**: Manually switch through all routes and confirm banner appears once per route.

### Task 4: Remove/normalize duplicated route-intro wrappers

- **ACTION**: Refactor pages/components where route and panel intros overlap (notably Profiles + Launch + card-heavy routes).
- **IMPLEMENT**: Keep section-level headings and all functional controls/status/actions. Only remove or demote duplicate route-identity title strips/copy that become redundant once `RouteBanner` is present.
- **MIRROR**: `crosshook-heading-eyebrow`, `crosshook-heading-title--card` for in-card section headings.
- **IMPORTS**: Existing heading classes and helper components.
- **GOTCHA**: In `LaunchPanel` and `ProfilesPage`, do not remove active profile selection rows, status chips, launch phase indicators, or CTA controls; these are functional, not decorative.
- **VALIDATE**: Check that each route has one clear top-level identity banner and card-level headings remain contextual.

### Task 5: Preserve scroll-shell and overflow behavior

- **ACTION**: Re-check route layout + scroll enhancements after banner insertion.
- **IMPLEMENT**: Ensure no new `overflow-y: auto` container is introduced without selector updates; keep inner containers `overscroll-behavior: contain` when scrollable.
- **MIRROR**: `ROUTE_LAYOUT_CONTRACT`, `useScrollEnhance` `SCROLLABLE` rules.
- **IMPORTS**: none.
- **GOTCHA**: Dual-scroll jank can appear if scroll ownership shifts from existing route containers. Health route keeps its custom top-padding behavior (`crosshook-page-scroll-shell--health`) and needs explicit spacing validation.
- **VALIDATE**: Mouse wheel + keyboard arrow scroll tested on Launch/Profiles/Community/Settings pages.

### Task 6: Final pass for terminology and accessibility consistency

- **ACTION**: Standardize banner copy and aria semantics across all routes.
- **IMPLEMENT**: Ensure route labels match sidebar naming conventions (e.g., `Browse` vs `Community` treatment choices stay deliberate and consistent).
- **MIRROR**: `ROUTE_LABELS` and sidebar section naming.
- **IMPORTS**: `Sidebar` route labels or a shared route metadata map to avoid drift.
- **GOTCHA**: Inconsistent naming between sidebar and banner can reintroduce UX drift.
- **VALIDATE**: Quick copy audit and screen-reader pass for decorative vs semantic elements.

---

## Testing Strategy

### Unit Tests

| Test                         | Input                         | Expected Output                               | Edge Case? |
| ---------------------------- | ----------------------------- | --------------------------------------------- | ---------- |
| Route banner metadata render | Route id + metadata           | Correct title/summary/art per route           | No         |
| Route composition smoke      | Each top-level page           | Exactly one route-level banner appears        | Yes        |
| Scroll ownership sanity      | Wheel/keyboard on route pages | Existing scroll containers still handle input | Yes        |

> Note: no frontend test framework is configured; validate with build + manual verification.

### Edge Cases Checklist

- [ ] Empty/missing summary text for a route still renders without layout break
- [ ] Long route titles or copy wrap correctly without overflow
- [ ] Steam Deck / controller-mode spacing remains usable
- [ ] `max-height: 820px` responsive mode preserves header + body usability
- [ ] Route with existing panel decor doesn’t visually conflict with new route banner
- [ ] Health route custom top padding does not produce double top-offset

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run build
```

EXPECT: TypeScript + Vite build succeeds with zero errors.

### Unit Tests

```bash
# No frontend unit test framework configured in this repo
echo "N/A"
```

EXPECT: N/A (manual + compile verification only).

### Full Test Suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: No regressions from UI-only changes (sanity guard for workspace health).

### Database Validation (if applicable)

```bash
echo "N/A - no persistence/schema changes"
```

EXPECT: N/A.

### Browser Validation (if applicable)

```bash
cd src/crosshook-native && npm run dev
```

EXPECT: All top-level routes show standardized route banner and preserved scroll behavior.

### Manual Validation

- [ ] Navigate every sidebar route and verify banner appears once, at top, with correct route identity.
- [ ] Verify Launch + Profiles no longer feel like duplicate hero stacks.
- [ ] Confirm Settings/Community/Compatibility still keep section-level card headers appropriately.
- [ ] Confirm Discover and Library now have the same route identity affordance as other pages.
- [ ] Verify keyboard navigation/focus order starts from route banner then content.
- [ ] Verify no dual-scroll behavior introduced in route body.
- [ ] Verify sidebar/banners keep label parity for known edge labels (`community`/Browse, `install`/Install Game).

---

## Acceptance Criteria

- [ ] Every top-level route renders the standardized route banner component.
- [ ] Banner height, padding, typography, and icon/art treatment are consistent.
- [ ] Existing per-route intro blocks duplicating route identity are removed or merged.
- [ ] Route scroll-shell contract remains intact (no overflow regressions).
- [ ] Accessibility semantics preserved (`aria-hidden` for purely decorative art, meaningful text structure for headings).

## Completion Checklist

- [ ] Code follows discovered route/page naming patterns.
- [ ] Error handling stays banner-based and non-blocking in page shells.
- [ ] Logging behavior remains consistent (`console.error` only for recoverable UI faults).
- [ ] Layout/scroll contract unchanged (`crosshook-route-stack` and scroll owners).
- [ ] No hardcoded visual values where existing CSS variables exist.
- [ ] No persistence boundary changes.
- [ ] No unnecessary scope additions into profile wizard/install redesign phases.
- [ ] Self-contained implementation steps with no extra codebase discovery needed.

## Risks

| Risk                                                              | Likelihood | Impact | Mitigation                                                                            |
| ----------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------- |
| Banner insertion introduces double-scroll in one or more routes   | Medium     | High   | Validate against `layout.css` contract + `useScrollEnhance` behavior for each route   |
| Duplicate heading hierarchy remains confusing after migration     | Medium     | Medium | Explicitly demote in-card route-level headings to section-level where needed          |
| Health route custom top padding conflicts with new banner spacing | Medium     | Medium | Reconcile `crosshook-page-scroll-shell--health` padding with new banner spacing rules |
| Copy/naming drift between sidebar labels and banners              | Low        | Medium | Centralize route metadata map reused by sidebar/banner where practical                |

## Notes

- Storage boundary classification: **runtime/UI-only** (no TOML, no SQLite, no migration).
- Persistence/usability impact: no migration/back-compat concerns; app remains fully offline-capable with unchanged persistence behavior.
- This phase should leave downstream Phase 2/3 with a stable route-level visual contract and reduce repeated header implementation effort.
