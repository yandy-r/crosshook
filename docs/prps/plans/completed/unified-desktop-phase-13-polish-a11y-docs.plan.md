# Phase 13 — Polish, Accessibility, and Documentation

**Issue**: Closes #425, Part of #452  
**PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md` § Phase 13  
**Depends on**: Phases 4–12  
**Success signal**: axe unit tests pass on every route page; reduced-motion Playwright test passes; focus-visible rings present on every interactive element; CI runs `check-legacy-palette.sh`; `docs/internal-docs/design-tokens.md` covers all token categories; Steam Deck QA checklist documented

---

## Context

Phase 13 is the final quality-gate before release polish. Three parallel researchers surveyed
the codebase post-Phases 1–12 and produced this summary.

### Already complete — do NOT redo

| Item                                             | Evidence                                                                                                                                                                         |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Legacy palette sweep                             | `check-legacy-palette.sh --selftest` → 0 matches in `src/`                                                                                                                       |
| `scripts/check-legacy-palette.sh`                | Exists, wired into `scripts/lint.sh:197-203`                                                                                                                                     |
| All 16 scroll containers in `useScrollEnhance`   | `useScrollEnhance.ts:8-9` SCROLLABLE constant                                                                                                                                    |
| Smoke: all 11 routes at 4 viewports              | `smoke.spec.ts` — Phase 12 adds multi-viewport sweep                                                                                                                             |
| ContextRail smoke (uw 3440 / sub-uw 2560)        | Phase 12 Task 1 — uses `data-testid="context-rail"`                                                                                                                              |
| Deck console status-bar smoke                    | Phase 12 Task 2 — `data-testid="console-status-bar"`                                                                                                                             |
| Focus-ring infrastructure                        | `focus.css` — `.crosshook-focus-ring`, `.crosshook-focus-target`                                                                                                                 |
| Library card hover-reveal reduced-motion guard   | `library.css:297-301`                                                                                                                                                            |
| Library card favorite-heart reduced-motion guard | `library.css:340-344`                                                                                                                                                            |
| Launch pipeline pulse reduced-motion guard       | `launch-pipeline.css:212-219`                                                                                                                                                    |
| Global reduced-motion blanket rule               | `theme.css:4760-4773`                                                                                                                                                            |
| Test files for core components                   | AppShell, Inspector, ConsoleDrawer, Sidebar, CommandPalette, GameDetail, GameInspector, HeroDetailPanels, LibraryCard, LibraryGrid, LibraryToolbar, useBreakpoint, useGamepadNav |
| `fireMatchMediaChangeListeners()` test mock      | `src/crosshook-native/src/test/setup.ts`                                                                                                                                         |
| `docs/internal-docs/design-tokens.md` (partial)  | Exists at 111 lines — shell surfaces, accent, status-muted, forbidden-literals table, CI enforcement                                                                             |

### Gaps to close in this phase

**A. axe-core** — not installed; zero automated a11y scanning anywhere in Vitest or Playwright

**B. Focus-visible CSS gaps** (`library.css`, `themed-select.css`):

- `library.css` toolbar: no `:focus-visible` on `.crosshook-library-toolbar__chip`,
  `__view-btn`, `__palette-trigger`; `__search:focus` sets only `border-color` (no ring)
- `library.css` list row buttons: no `:focus-visible` on `__btn--launch`, `__btn--icon`
- `themed-select.css:78`: `[data-highlighted]` is pointer-only; no keyboard `:focus-visible`

**C. Reduced-motion CSS gaps** (animations not covered by the global blanket rule in
`theme.css:4760-4773` because they live in separate CSS files):

- `themed-select.css:52`: `@keyframes crosshook-select-in` — no guard
- `host-tool-dashboard.css:393`: `@keyframes crosshook-host-tool-dashboard-pulse` — no guard
- `sidebar.css:13-15, 139-144`: `width`/`padding`/`transform` transitions — no guard
- `library.css:426-429`: `.crosshook-library-list-row` `transform` on hover — no guard
- `palette.css:109-115`: `.crosshook-palette__row` transitions — no guard

**D. ARIA gaps**:

- `ConsoleDrawer.tsx`: toggle button has no explicit `aria-label`; label text is inside
  `aria-hidden` spans so relies on `useAriaLabelHydration` side-effect
- `gamepad-nav/types.ts:3`: `FocusZone = 'sidebar' | 'content'`; missing `'inspector'`
  (Inspector sets `data-crosshook-focus-zone="inspector"` but the type doesn't include it)

**E. Docs gaps**:

- `docs/internal-docs/design-tokens.md` (111 lines) missing: typography, radius, shadow,
  layout/spacing, capability indicators, pipeline connectors, autosave indicators, palette
  overlay tokens, controller-mode overrides, `@media` responsive overrides, high-contrast
  theme token catalogue
- PRD references `docs/internal/design-tokens.md` — **wrong path**; actual canonical
  path (used by `check-legacy-palette.sh:136` and the doc itself) is
  `docs/internal-docs/design-tokens.md`. Do not create a new file at the PRD path;
  expand the existing file.

**F. CI gap**:

- `scripts/check-legacy-palette.sh` is **not** invoked by `.github/workflows/lint.yml`.
  The `shell` job calls only `shellcheck`, `check-host-gateway.sh`, `check-mock-coverage.sh`.

**G. Missing smoke test**:

- No `prefers-reduced-motion` emulation smoke test (Playwright `page.emulateMedia`)

**H. Missing Vitest tests**:

- No test file for `ContextRail`, `HeroDetailTabs`, `HeroDetailHeader`, `LibraryListRow`
  (add minimal smoke-render tests alongside the axe coverage)

---

## Worktree Setup

```bash
git worktree add ~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13 \
  -b feat/unified-desktop-phase-13
cd ~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13
cd src/crosshook-native && npm ci && cd ../..
npm install -D --no-save typescript biome   # worktree-local dev tools
```

Run tests: `cd src/crosshook-native && npm test`  
Run smoke: `cd src/crosshook-native && npm run test:smoke`  
Run lint: `./scripts/lint.sh`

---

## Tasks

### Batch 1 — Parallel (no dependencies between tasks)

---

#### Task 1.1 — Install jest-axe and configure axe test utilities

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

**File: `src/crosshook-native/package.json`**

Add to `devDependencies`:

```json
"jest-axe": "^8.0.0"
```

Run `npm install` to update the lockfile.

**File: `src/crosshook-native/src/test/setup.ts`**

Append at the end of the existing setup file (after the existing `@testing-library/jest-dom` and mock setup):

```ts
import { configureAxe, toHaveNoViolations } from 'jest-axe';

expect.extend(toHaveNoViolations);

// Color contrast requires real CSS rendering; not meaningful in happy-dom.
configureAxe({
  rules: { 'color-contrast': { enabled: false } },
});
```

**Verification**:

```bash
cd src/crosshook-native && npm install && npm run typecheck 2>&1 | head -10
```

---

#### Task 1.2 — Fix focus-visible CSS gaps

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

**File: `src/crosshook-native/src/styles/library.css`**

1. **Library toolbar search** — upgrade the `:focus` rule (around line 55-62) to `:focus-visible`
   and add a box-shadow ring. Change:

   ```css
   .crosshook-library-toolbar__search:focus {
     border-color: var(--crosshook-color-accent);
   }
   ```

   To:

   ```css
   .crosshook-library-toolbar__search:focus-visible {
     outline: none;
     border-color: var(--crosshook-color-accent);
     box-shadow: 0 0 0 3px var(--crosshook-color-accent-soft);
   }
   ```

2. **Library toolbar chips and triggers** — add `:focus-visible` rules after the existing
   `.crosshook-library-toolbar__chip` block (grep for it and append after):

   ```css
   .crosshook-library-toolbar__chip:focus-visible,
   .crosshook-library-toolbar__view-btn:focus-visible,
   .crosshook-library-toolbar__palette-trigger:focus-visible {
     outline: none;
     box-shadow: 0 0 0 3px var(--crosshook-color-accent-soft);
   }
   ```

3. **Library list row buttons** — add `:focus-visible` rules after the existing
   `.crosshook-library-list-row__btn--launch` and `__btn--icon` blocks:
   ```css
   .crosshook-library-list-row__btn--launch:focus-visible,
   .crosshook-library-list-row__btn--icon:focus-visible {
     outline: none;
     box-shadow: 0 0 0 3px var(--crosshook-color-accent-soft);
   }
   ```

**File: `src/crosshook-native/src/styles/themed-select.css`**

Add after the `.crosshook-themed-select__item` base block (around line 78):

```css
.crosshook-themed-select__item[data-highlighted] {
  box-shadow: inset 0 0 0 2px var(--crosshook-color-accent-soft);
}
```

Note: Radix Select sets `data-highlighted` on both pointer-hover and keyboard-focus,
so this selector covers keyboard navigation without needing `:focus-visible`.

**Verification**:

```bash
./scripts/lint.sh 2>&1 | grep -E "error|warning" | head -20
```

---

#### Task 1.3 — Fix reduced-motion CSS gaps

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

Add `@media (prefers-reduced-motion: reduce)` guards to each file. The guards must appear
**after** the animation/transition declarations they cover so they override cleanly.

**File: `src/crosshook-native/src/styles/themed-select.css`**

Append at end of file:

```css
@media (prefers-reduced-motion: reduce) {
  .crosshook-themed-select__content {
    animation: none;
  }
}
```

**File: `src/crosshook-native/src/styles/host-tool-dashboard.css`**

Identify the selector that uses `crosshook-host-tool-dashboard-pulse` (around line 393).
Append at end of file:

```css
@media (prefers-reduced-motion: reduce) {
  [class*='crosshook-host-tool-dashboard'][class*='skeleton'],
  [class*='crosshook-host-tool-dashboard'][class*='pulse'] {
    animation: none;
    opacity: 0.6;
  }
}
```

If the exact class is visible from `host-tool-dashboard.css:393`, use the literal class
name instead of the `[class*=]` pattern.

**File: `src/crosshook-native/src/styles/sidebar.css`**

Append at end of file:

```css
@media (prefers-reduced-motion: reduce) {
  .crosshook-sidebar {
    transition: none;
  }
  .crosshook-sidebar__item {
    transition: none;
    transform: none;
  }
}
```

**File: `src/crosshook-native/src/styles/library.css`**

Append after the existing hover-reveal guard block (after `library.css:297-344`):

```css
@media (prefers-reduced-motion: reduce) {
  .crosshook-library-list-row {
    transition: none;
  }
  .crosshook-library-list-row:hover {
    transform: none;
  }
}
```

**File: `src/crosshook-native/src/styles/palette.css`**

Append at end of file:

```css
@media (prefers-reduced-motion: reduce) {
  .crosshook-palette__row {
    transition: none;
    transform: none;
  }
}
```

**Verification**:

```bash
./scripts/lint.sh 2>&1 | grep -E "error|warning" | head -20
```

---

#### Task 1.4 — ARIA fixes: ConsoleDrawer toggle label and FocusZone type

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

**File: `src/crosshook-native/src/components/layout/ConsoleDrawer.tsx`**

Locate the toggle button element (around line 106-239). It currently has `aria-expanded`
and `aria-controls` but no `aria-label`. Change the button opening tag to:

```tsx
<button
  type="button"
  aria-label={isExpanded ? 'Collapse runtime console' : 'Expand runtime console'}
  aria-expanded={isExpanded}
  aria-controls="crosshook-console-drawer-body"
  ...existing props...
>
```

Add `id="crosshook-console-drawer-body"` to the drawer body `div` that `aria-controls`
references. Match the existing `data-testid="console-drawer"` element.

**File: `src/crosshook-native/src/hooks/gamepad-nav/types.ts`**

Line 3. Change:

```ts
export type FocusZone = 'sidebar' | 'content';
```

To:

```ts
export type FocusZone = 'sidebar' | 'content' | 'inspector';
```

After this change, run TypeScript compilation to confirm no exhaustive switch/if in
`effects.ts` or `focusManagement.ts` breaks. The `switchZone` function should default-
handle unknown zones already; if a type-narrowing `switch` without default breaks, add a
`default: break` arm.

**Verification**:

```bash
cd src/crosshook-native && npm run typecheck 2>&1 | head -30
npm test -- --reporter=verbose --testPathPattern="ConsoleDrawer|gamepad" 2>&1 | tail -20
```

---

#### Task 1.5 — Add check-legacy-palette.sh to GitHub Actions lint workflow

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

**File: `.github/workflows/lint.yml`**

In the `shell` job, add a new step after the `check-host-gateway.sh` step and before
`check-mock-coverage.sh`. The new step:

```yaml
- name: Check legacy palette
  run: ./scripts/check-legacy-palette.sh
```

The `shell` job's `runs-on` should already provide `bash`. The script already exits
non-zero on violations (it is designed for CI use — see `check-legacy-palette.sh:70`).

**Verification**:

```bash
./scripts/check-legacy-palette.sh && echo "EXIT 0 — no violations"
```

(Actual CI confirmation requires a pushed branch; validate locally with the above.)

---

### Batch 2 — Parallel (Depends on Batch 1)

**Depends on**: Batch 1 (jest-axe must be installed before writing axe tests)

---

#### Task 2.1 — Write axe unit tests for all route pages and key components

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

Create two new test files to stay under the 500-line cap.

**File: `src/crosshook-native/src/__tests__/a11y/routes.a11y.test.tsx`**

```tsx
import { axe } from 'jest-axe';
import { renderWithMocks } from '../../test/render';

// Import all route page components
import { LibraryPage } from '../../pages/LibraryPage';
import { ProfilesPage } from '../../pages/ProfilesPage';
import { LaunchPage } from '../../pages/LaunchPage';
import { HealthPage } from '../../pages/HealthPage';
import { HostToolsPage } from '../../pages/HostToolsPage';
import { ProtonManagerPage } from '../../pages/ProtonManagerPage';
import { CommunityPage } from '../../pages/CommunityPage';
import { DiscoverPage } from '../../pages/DiscoverPage';
import { CompatibilityPage } from '../../pages/CompatibilityPage';
import { SettingsPage } from '../../pages/SettingsPage';
import { InstallPage } from '../../pages/InstallPage';

const ROUTE_PAGES = [
  { name: 'LibraryPage', Component: LibraryPage },
  { name: 'ProfilesPage', Component: ProfilesPage },
  { name: 'LaunchPage', Component: LaunchPage },
  { name: 'HealthPage', Component: HealthPage },
  { name: 'HostToolsPage', Component: HostToolsPage },
  { name: 'ProtonManagerPage', Component: ProtonManagerPage },
  { name: 'CommunityPage', Component: CommunityPage },
  { name: 'DiscoverPage', Component: DiscoverPage },
  { name: 'CompatibilityPage', Component: CompatibilityPage },
  { name: 'SettingsPage', Component: SettingsPage },
  { name: 'InstallPage', Component: InstallPage },
] as const;

for (const { name, Component } of ROUTE_PAGES) {
  describe(`${name} accessibility`, () => {
    it('has no axe violations', async () => {
      const { container } = renderWithMocks(<Component />);
      const results = await axe(container);
      expect(results).toHaveNoViolations();
    });
  });
}
```

Adjust import paths to match the actual file locations. If pages live under
`src/pages/` or `src/routes/`, use the correct path. Use `find src/crosshook-native/src
-name "*Page.tsx" | sort` to confirm.

If a page requires required props (e.g., a selected game), pass a minimal stub from the
existing fixtures used in other tests. Check `src/crosshook-native/src/test/render.tsx`
for available mock utilities.

**File: `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx`**

```tsx
import { axe } from 'jest-axe';
import { renderWithMocks } from '../../test/render';
import { CommandPalette } from '../../components/palette/CommandPalette';
import { Inspector } from '../../components/layout/Inspector';
import { ContextRail } from '../../components/layout/ContextRail';
import { GameDetail } from '../../components/library/GameDetail';
import { HeroDetailHeader } from '../../components/library/HeroDetailHeader';
import { HeroDetailTabs } from '../../components/library/HeroDetailTabs';
import { LibraryListRow } from '../../components/library/LibraryListRow';

describe('CommandPalette accessibility', () => {
  it('has no axe violations when open', async () => {
    // Render with open=true; use minimal command list
    const { container } = renderWithMocks(
      <CommandPalette
        open
        commands={[{ id: 'go-library', title: 'Go to Library', icon: null, execute: () => {} }]}
        onClose={() => {}}
      />
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('Inspector accessibility', () => {
  it('renders without axe violations (empty route)', async () => {
    const { container } = renderWithMocks(<Inspector inspectorComponent={null} selection={undefined} />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('ContextRail accessibility', () => {
  it('has no axe violations', async () => {
    const { container } = renderWithMocks(<ContextRail />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('GameDetail accessibility', () => {
  it('has no axe violations', async () => {
    const { container } = renderWithMocks(<GameDetail gameId="mock-game-1" onBack={() => {}} />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('HeroDetailHeader accessibility', () => {
  it('has no axe violations', async () => {
    // Use the mock game fixture from existing GameDetail tests
    const { container } = renderWithMocks(<HeroDetailHeader game={mockGame} onBack={() => {}} />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('HeroDetailTabs accessibility', () => {
  it('has no axe violations', async () => {
    const { container } = renderWithMocks(<HeroDetailTabs gameId="mock-game-1" />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('LibraryListRow accessibility', () => {
  it('has no axe violations', async () => {
    const { container } = renderWithMocks(
      <LibraryListRow game={mockGame} onSelect={() => {}} onOpenDetails={() => {}} />
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
```

Adapt prop interfaces by reading the actual component signatures. Use the `mockGame`
fixture already defined in `src/crosshook-native/src/test/` or the mock from
`library/__tests__/LibraryCard.test.tsx`.

**Verification**:

```bash
cd src/crosshook-native && npm test -- --reporter=verbose --testPathPattern="a11y" 2>&1 | tail -40
```

All tests should pass. If axe reports violations, fix them per the guidance in Task 3.1.

---

#### Task 2.2 — Add reduced-motion Playwright smoke test

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

**File: `src/crosshook-native/tests/smoke.spec.ts`**

Append a new describe block at the end of the file (after all existing describe blocks):

```ts
test.describe('reduced-motion smoke', () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
  });

  test('library renders without console errors under prefers-reduced-motion', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');
    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();
    await expect(libraryTab).toHaveAttribute('aria-current', 'page');
    // Hover-reveal transition must be suppressed: transitionDuration should be '0s'
    const transitionDuration = await page.evaluate(() => {
      const el = document.querySelector('.crosshook-library-card__hover-reveal');
      return el ? window.getComputedStyle(el).transitionDuration : null;
    });
    if (transitionDuration !== null) {
      expect(transitionDuration, 'hover-reveal transition must be 0s under reduced-motion').toBe('0s');
    }
    expect(capture.errors, `Reduced-motion library errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('command palette opens without animation under prefers-reduced-motion', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');
    await page.keyboard.press('Control+k');
    const dialog = page.locator('[role="dialog"]');
    await expect(dialog).toBeVisible({ timeout: 3000 });
    const rowTransition = await page.evaluate(() => {
      const el = document.querySelector('.crosshook-palette__row');
      return el ? window.getComputedStyle(el).transitionDuration : null;
    });
    if (rowTransition !== null) {
      expect(rowTransition, 'palette row transition must be 0s under reduced-motion').toBe('0s');
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    expect(capture.errors, `Reduced-motion palette errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});
```

**Verification**:

```bash
cd src/crosshook-native && npm run test:smoke -- --grep "reduced-motion" 2>&1 | tail -20
```

---

#### Task 2.3 — Expand docs/internal-docs/design-tokens.md

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

**File: `docs/internal-docs/design-tokens.md`** (currently 111 lines)

The existing file documents: forbidden-literal table, shell surfaces (10 tokens), accent
(4 tokens + glow), status-muted (3 tokens), adding-a-token workflow, suppression grammar
(`/* allow: legacy-palette */`), high-contrast carve-out, CI enforcement.

Append the following sections. Keep each section concise — document token name, value,
and usage intent. The goal is a navigable reference, not a design spec.

---

**Append after the existing CI enforcement section:**

```markdown
---
## Path note

The PRD (`unified-desktop-redesign.prd.md:143`) references `docs/internal/design-tokens.md`
but the canonical path is **`docs/internal-docs/design-tokens.md`** (this file).
The CI sentinel (`scripts/check-legacy-palette.sh:136`) and all cross-references use this path.
---

## Typography tokens

| Token                   | Value                                                                               | When to use                                            |
| ----------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `--crosshook-font-body` | `'Avenir Next', 'Segoe UI', 'Helvetica Neue', system-ui, -apple-system, sans-serif` | All non-code text. Never hardcode `font-family`.       |
| `--crosshook-font-mono` | `'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace`                   | Code, paths, IDs, environment values, terminal output. |

---

## Radius and shadow tokens

| Token                       | Value                             | When to use                                 |
| --------------------------- | --------------------------------- | ------------------------------------------- |
| `--crosshook-radius-lg`     | `20px`                            | Cards, panels, modals                       |
| `--crosshook-radius-md`     | `14px`                            | Buttons, inputs, chips                      |
| `--crosshook-radius-sm`     | `10px`                            | Badges, tags, small pills                   |
| `--crosshook-shadow-soft`   | `0 18px 40px rgba(0, 0, 0, 0.32)` | Elevated surfaces: modals, overlays, panels |
| `--crosshook-shadow-strong` | `0 28px 70px rgba(0, 0, 0, 0.42)` | Floating UI: palette, popover, tooltips     |

---

## Spacing and layout tokens

These tokens enforce layout consistency. Use them instead of ad-hoc `px` values.

| Token                               | Value   | Purpose                                           |
| ----------------------------------- | ------- | ------------------------------------------------- |
| `--crosshook-page-padding`          | `32px`  | Outer padding of every route page body            |
| `--crosshook-panel-padding`         | `20px`  | Interior padding of dashboard panels              |
| `--crosshook-card-padding`          | `28px`  | Interior padding of content cards                 |
| `--crosshook-grid-gap`              | `20px`  | Gap between grid items (library, dashboard grids) |
| `--crosshook-touch-target-min`      | `48px`  | Minimum tap target height (WCAG 2.5.5 AA)         |
| `--crosshook-touch-target-compact`  | `44px`  | Compact tap target (used in dense lists)          |
| `--crosshook-button-height-compact` | `44px`  | Standard button height                            |
| `--crosshook-transition-fast`       | `140ms` | Micro-interactions: hover, active state           |
| `--crosshook-transition-standard`   | `220ms` | Panel open/close, sidebar collapse                |
| `--crosshook-library-card-width`    | `190px` | Library grid card base width                      |
| `--crosshook-library-card-aspect`   | `3 / 4` | Library card aspect ratio                         |

Responsive overrides exist in `variables.css` `@media` blocks — see § Responsive overrides below.

---

## Capability indicator tokens

Used by host-readiness and health-check UIs to communicate tool status.

| Token                                             | Value                       | Meaning                         |
| ------------------------------------------------- | --------------------------- | ------------------------------- |
| `--crosshook-color-capability-available`          | `#4ade80`                   | Tool present and working        |
| `--crosshook-color-capability-available-bg`       | `rgba(74, 222, 128, 0.12)`  | Chip background for available   |
| `--crosshook-color-capability-available-border`   | `rgba(74, 222, 128, 0.28)`  | Chip border for available       |
| `--crosshook-color-capability-degraded`           | `#fbbf24`                   | Tool present but limited        |
| `--crosshook-color-capability-degraded-bg`        | `rgba(251, 191, 36, 0.12)`  | Chip background for degraded    |
| `--crosshook-color-capability-degraded-border`    | `rgba(251, 191, 36, 0.28)`  | Chip border for degraded        |
| `--crosshook-color-capability-unavailable`        | `#f87171`                   | Tool missing or broken          |
| `--crosshook-color-capability-unavailable-bg`     | `rgba(248, 113, 113, 0.12)` | Chip background for unavailable |
| `--crosshook-color-capability-unavailable-border` | `rgba(248, 113, 113, 0.28)` | Chip border for unavailable     |

---

## Pipeline connector tokens

Connector lines between launch pipeline nodes. Use `color-mix()` variants only — never
hardcode a hex for connector states.

| Token                                          | Value                                                                       |
| ---------------------------------------------- | --------------------------------------------------------------------------- |
| `--crosshook-color-pipeline-connector-success` | `color-mix(in srgb, var(--crosshook-color-success) 35%, transparent)`       |
| `--crosshook-color-pipeline-connector-active`  | `color-mix(in srgb, var(--crosshook-color-accent-strong) 40%, transparent)` |
| `--crosshook-color-pipeline-connector-error`   | `color-mix(in srgb, var(--crosshook-color-danger) 35%, transparent)`        |
| `--crosshook-color-pipeline-connector-waiting` | `color-mix(in srgb, var(--crosshook-color-warning) 40%, transparent)`       |

---

## Autosave indicator tokens

Eight tokens for the four autosave states × background/border.

| Token                                                                     | State               |
| ------------------------------------------------------------------------- | ------------------- |
| `--crosshook-autosave-saving-bg` / `--crosshook-autosave-saving-border`   | Save in progress    |
| `--crosshook-autosave-success-bg` / `--crosshook-autosave-success-border` | Save succeeded      |
| `--crosshook-autosave-warning-bg` / `--crosshook-autosave-warning-border` | Saved with warnings |
| `--crosshook-autosave-error-bg` / `--crosshook-autosave-error-border`     | Save failed         |

---

## Command palette overlay tokens

Used only in `palette.css` for the overlay surface. Do not use elsewhere — the palette
intentionally uses a deeper dark than the standard `--crosshook-color-bg`.

| Token                                | Value                       | Usage                     |
| ------------------------------------ | --------------------------- | ------------------------- |
| `--crosshook-palette-border-on-dark` | `rgba(255, 255, 255, 0.08)` | Palette surface border    |
| `--crosshook-palette-bg-dark-98`     | `rgba(13, 19, 34, 0.98)`    | Main palette backdrop     |
| `--crosshook-palette-bg-dark-90`     | `rgba(13, 19, 34, 0.9)`     | Palette row hover surface |

---

## Controller-mode overrides

`variables.css` defines a `:root[data-crosshook-controller-mode='true']` block that
overrides layout and spacing tokens for gamepad/Steam Deck controller mode. These apply
automatically when `useGamepadNav` detects a gamepad and sets the attribute on `<html>`.

Overridden tokens in controller mode include touch-target sizes, padding, and subtab
heights — all increased for D-Pad navigation comfort. Never reference these overrides
directly in component CSS; they apply globally via the attribute selector.

---

## Responsive @media overrides

`variables.css` contains three breakpoint-specific override blocks. These adjust spacing
and layout tokens automatically — no JS involvement.

| Block                        | Overrides                                          | Purpose                            |
| ---------------------------- | -------------------------------------------------- | ---------------------------------- |
| `@media (max-width: 1360px)` | `--crosshook-page-padding`, `--crosshook-grid-gap` | Laptop/narrow tightening           |
| `@media (max-width: 900px)`  | `--crosshook-page-padding`, `--crosshook-grid-gap` | Compact tightening                 |
| `@media (max-height: 820px)` | Touch targets, padding, launch panel tokens        | Short-viewport (Steam Deck native) |

---

## High-contrast theme token overrides

When `data-crosshook-theme='high-contrast'` is set on `<html>` (by
`useHighContrastTheme`), the following tokens are overridden. Components that use these
tokens automatically get high-contrast values without any conditional CSS.

Key overrides:

- Accent pair swaps from steel-blue to amber: `--crosshook-color-accent → #facc15`,
  `--crosshook-color-accent-strong → #f97316`
- Background and surface tokens shift to near-black for maximum contrast
- Border tokens increase opacity for higher contrast
- Status tokens shift to saturated values for unambiguous state communication

See `variables.css` `:root[data-crosshook-theme='high-contrast']` block for the complete
list. If adding new tokens that should be high-contrast-aware, add an override in that
block.
```

**Verification**:

```bash
wc -l docs/internal-docs/design-tokens.md   # should be substantially >111
```

---

### Batch 3 — Sequential (Depends on Batch 1 + Batch 2)

---

#### Task 3.1 — Run full test suite; fix any residual axe violations

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

```bash
cd src/crosshook-native
npm test 2>&1 | grep -E "FAIL|✗|axe|violation|×" | head -60
npm run test:smoke 2>&1 | tail -30
npm run typecheck 2>&1 | head -20
```

**Expected outcome**: all tests green. If axe violations appear, fix per the table:

| axe Violation                                                   | Fix                                                 |
| --------------------------------------------------------------- | --------------------------------------------------- |
| `button-name`: button has no accessible name                    | Add `aria-label` to the button                      |
| `image-alt`: `<img>` missing `alt`                              | Add `alt=""` (decorative) or descriptive `alt` text |
| `list`: `<ul>/<ol>` contains non-`<li>` children                | Fix the DOM structure                               |
| `duplicate-id`: same `id` on multiple elements                  | Scope IDs or use `useId()` hook                     |
| `landmark-unique`: multiple landmarks with same accessible name | Add unique `aria-label`                             |
| `aria-required-attr`: ARIA role missing required attribute      | Add the missing attribute                           |
| `aria-allowed-attr`: invalid ARIA attribute for role            | Remove or correct the attribute                     |

Re-run `npm test -- --testPathPattern="a11y"` after each fix to confirm resolution.

**TypeScript errors from FocusZone change (Task 1.4)**:
If `gamepad-nav/effects.ts` or `gamepad-nav/focusManagement.ts` has a type-narrowing
`switch` over `FocusZone` without a `default` branch, add `default: break` or handle
`'inspector'` explicitly. The inspector zone behavior (navigate into the inspector
panel with D-Pad) can be a no-op for v1 — the ContextRail is read-only and the
inspector scrolls via `useScrollEnhance`.

---

#### Task 3.2 — Steam Deck manual QA checklist

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

Check if `docs/internal-docs/steam-deck-checklist.md` exists. If it does, append a
Phase 13 section. If not, create it.

```markdown
## Phase 13 — Unified Desktop Polish & A11y (2026-04)

Target: Steam Deck native (1280×800, WebKitGTK, gamepad + touchscreen)

### Shell layout at 1280×800

- [ ] Sidebar renders as 56px icon rail; no text labels visible
- [ ] Inspector panel absent (0px width) — no phantom column
- [ ] Console shows 32px compact status bar, not the full drawer
- [ ] Status bar shows readiness chips + "⌘K commands" tip text
- [ ] No horizontal overflow or scrollbar on any route

### Library at deck viewport

- [ ] Library grid renders cards at correct widths; no wrapping issues
- [ ] Touching a card shows action buttons
- [ ] Selecting a card (double-tap or Enter) enters hero detail; sidebar rail remains
- [ ] Back button / B button returns to library grid

### Command palette at deck

- [ ] Ctrl+K or ⌘K opens the palette overlay
- [ ] D-Pad Up/Down navigates command items
- [ ] A button executes the selected command
- [ ] B button closes the palette; focus returns to prior element

### Gamepad zone navigation

- [ ] D-Pad Left moves focus to sidebar; D-Pad Right to content
- [ ] D-Pad Up/Down moves within active zone
- [ ] A button activates focused element; B moves focus out/back
- [ ] Controller-mode focus rings visible on all focused elements

### Focus rings (keyboard navigation mode)

- [ ] Library toolbar chips show focus ring on Tab focus
- [ ] Library toolbar search shows focus ring on Tab focus
- [ ] Library list row buttons show focus rings individually
- [ ] Themed-select items show ring on keyboard navigation
- [ ] Console toggle button shows focus ring

### Reduced motion (OS setting: Reduce Motion = On)

- [ ] Library card hover-reveal does not animate
- [ ] Sidebar collapse is instant (no width transition)
- [ ] Palette rows do not slide-in
- [ ] No animation flash on any surface

### Blocking issues (sign-off gate — must be zero)

- [ ] NONE
```

**Verification**: Documented; actual hardware sign-off is the gate for closing #425.

---

#### Task 3.3 — Final lint, format, and cargo check

**Worktree**: `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13`

```bash
./scripts/format.sh
./scripts/lint.sh
./scripts/check-legacy-palette.sh && echo "PASS — 0 legacy palette violations"
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core 2>&1 | tail -10
```

All must exit 0. Commit all changes with the conventional commit messages below.

---

## Commit Sequence

```
chore(a11y): install jest-axe and configure axe test utilities
fix(ui): add missing focus-visible rings on library toolbar and list-row buttons
fix(ui): add prefers-reduced-motion guards for unguarded animations
fix(ui): add aria-label to ConsoleDrawer toggle and inspector to FocusZone type
ci: invoke check-legacy-palette.sh in GitHub Actions lint workflow
test(a11y): add axe unit tests for all route pages and shell components
test(smoke): add reduced-motion Playwright smoke test
docs(internal): expand design-tokens.md with full token catalogue
chore(a11y): fix residual axe violations (if any from Task 3.1)
docs(internal): add Phase 13 Steam Deck manual QA checklist
```

Use `docs(internal): …` for all files under `docs/`.

---

## Batches Summary

| Batch | Tasks                                                                                                          | Depends on                            | Can run in parallel?               |
| ----- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ---------------------------------- |
| 1     | 1.1 (jest-axe install), 1.2 (focus-visible CSS), 1.3 (reduced-motion CSS), 1.4 (ARIA fixes), 1.5 (CI lint.yml) | —                                     | Yes — all independent              |
| 2     | 2.1 (axe unit tests), 2.2 (reduced-motion smoke), 2.3 (design-tokens.md)                                       | Batch 1 (1.1 must be done before 2.1) | Yes — all independent within batch |
| 3     | 3.1 (full test run + fixes), 3.2 (Deck QA checklist), 3.3 (final lint)                                         | Batch 2                               | Sequential                         |

---

## Verification Checklist

- [ ] `npm test` — all Vitest green (axe tests pass, reduced-motion mock test passes)
- [ ] `npm run test:smoke` — all Playwright green (reduced-motion smoke passes)
- [ ] `npm run typecheck` — zero TypeScript errors
- [ ] `./scripts/lint.sh` — zero violations (Rust, Biome, ShellCheck, legacy-palette)
- [ ] `./scripts/check-legacy-palette.sh` — 0 matches
- [ ] `.github/workflows/lint.yml` — `shell` job invokes `check-legacy-palette.sh`
- [ ] `docs/internal-docs/design-tokens.md` — all 10+ token categories documented
- [ ] `docs/internal-docs/steam-deck-checklist.md` — Phase 13 section added
- [ ] Steam Deck hardware pass — no blocking issues found
- [ ] Issues: #425 closed, #452 checked off

---

_Generated: 2026-04-23_  
_Researchers: 3× parallel ycc:prp-researcher (a11y infrastructure, docs/changelog, component audit)_
