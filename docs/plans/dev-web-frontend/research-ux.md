# UX Research — dev-web-frontend

Research into UX patterns for the browser-only Vite dev mode in CrossHook: how to
signal "mock data active", how to switch fixture states, what diverges visually between
Chrome and the Tauri WebKitGTK webview, and how to make those affordances accessible.

---

## Executive Summary

The `dev-web-frontend` feature runs CrossHook's Vite frontend at `http://localhost:5173`
in a plain browser with static mock data standing in for Tauri IPC. The UX challenge is
not the production UI design — that already exists. The challenge is: what visual
infrastructure keeps the designer oriented inside a mock browser session, and how do they
quickly cycle through UI states without a Rust backend?

Three patterns are required:

1. **A persistent but non-intrusive mock-mode indicator** — makes it impossible to
   mistake a mock session for the real app, without covering up design surfaces.
2. **A fixture-state switcher** — lets the developer flip between empty, populated, error,
   and loading states with no rebuild. Query-string approach wins over a floating panel
   for simplicity.
3. **Chrome-vs-WebKitGTK parity awareness** — the browser will render scrollbars and a
   few CSS features differently from the Tauri webview. The designer needs to understand
   which differences matter and which do not.

All three must be accessible (`role="status"`, focusable toggles, screen-reader-friendly
labels) so that automated audits in dev mode do not produce false positives that obscure
real issues.

**Confidence**: High — based on official documentation and multiple corroborating sources
for each major finding.

---

## User Workflows

Three primary workflows this feature enables, each cross-referenced to the detailed section
that owns it:

### Primary workflow — design iteration loop

1. **Start dev mode** — `./scripts/dev-native.sh --browser` starts Vite, opens
   `http://localhost:5173`. The dev-mode indicator appears immediately (Layer 1 viewport
   outline + Layer 2 corner chip).
2. **Populated state loads** — boot-critical handlers (`settings_load`, `profile_list`,
   etc.) return deterministic fixtures. The UI is fully navigable across every tab.
3. **Open DevTools** — Ctrl+Shift+I. Element picker, live CSS editing, Workspaces, and
   Local Overrides are all available. See **Element Inspection Workflow** below for the
   full Chrome DevTools toolkit this unlocks.
4. **Edit component or token** — CSS changes apply via HMR. Component edits trigger hot
   reload. The mock singleton and in-memory store reset on HMR; fixture state re-seeds
   deterministically.
5. **Switch fixture state** — append `?fixture=empty`, `?fixture=error`, `?fixture=loading`
   to the URL. No rebuild. See **Fixture Toggling — Implementation Pattern** for the full
   query-string API.
6. **Verify WebKitGTK parity** — periodically re-check the change in `tauri dev` to catch
   the handful of Chrome-vs-WebKit differences documented in **Browser-vs-Tauri Parity**.

### Secondary workflow — error-state review

1. Navigate to `http://localhost:5173?fixture=error`.
2. Every IPC-dependent surface renders its error UI. Fixtures populate error messages and
   error codes consistent with real Rust error shapes.
3. Iterate on error-state copy, icon usage, recovery affordances.

### Tertiary workflow — empty-state review

1. Navigate to `http://localhost:5173?fixture=empty`.
2. Profile list, recent files, trainer catalog, etc. all render their empty states.
3. Iterate on empty-state illustrations, CTAs, and onboarding copy.

### What this feature does NOT enable

- **No backend testing**: mocks cannot exercise Rust-side business logic, race conditions,
  or real game-launch flows.
- **No cross-tab persistence**: reloading the tab resets the in-memory store. This is
  intentional per the runtime-only storage classification.
- **No production-parity rendering guarantee**: see **Browser-vs-Tauri Parity** for the
  short list of differences that require a final check in the real Tauri WebView.

---

## Sidebar Layout Analysis and Indicator Placement Decision

The Sidebar component (`src/crosshook-native/src/components/layout/Sidebar.tsx`) offers
three candidate placements for the dev-mode indicator:

1. Brand area (`crosshook-sidebar__brand`) — lines 103–132
2. Status group (`crosshook-sidebar__status-group`) — lines 162–166
3. `position: fixed` corner element, outside the sidebar entirely

The collapsible sidebar state (`sidebar.css` lines 203–237) hides both candidate 1 and
candidate 2 via `display: none` when `data-collapsed="true"` is set on the `<aside>`:

```css
.crosshook-sidebar[data-collapsed='true'] .crosshook-sidebar__brand,
.crosshook-sidebar[data-collapsed='true'] .crosshook-sidebar__status,
.crosshook-sidebar[data-collapsed='true'] .crosshook-sidebar__status-group {
  display: none;
}
```

Any indicator placed inside the sidebar (brand area or status group) disappears when the
sidebar collapses to its 56px icon-only state. This violates BR-3: "visible on all 9
routes." The sidebar can collapse on any route.

**Decision: `position: fixed` corner chip is the only correct placement.**

A `position: fixed` element sits outside the sidebar's flex/grid context. It is
unaffected by sidebar collapse, route changes, modal layers, or scroll position. It
matches the Vercel Toolbar precedent and the react-query-devtools pattern.

Placement: `bottom: 12px; right: 12px` — below the console drawer resize handle zone
and outside the `crosshook-shell-group` layout entirely. The chip is approximately
120×24px at `0.7rem` font size and will not collide with any interactive element.

---

## Dev-Mode Visual Indicators

### How production apps signal non-production environments

#### Vercel Toolbar (preview deployments)

Vercel's approach is the most studied floating dev indicator in production use.

- **Appearance**: A small circle with a menu icon, fixed to a corner of the viewport.
  Starts "sleeping" (shows Vercel logo over menu icon). Clicking activates it.
- **Position**: Bottom-right corner by default. Draggable to any edge.
- **Toggle behaviour**: "Disable for Session" hides it until a new browser session. A
  keyboard shortcut restores it. Auto-disabled via the `x-vercel-skip-toolbar` header
  for E2E test runs.
- **Key insight**: The toolbar is always present but does not expand until the developer
  interacts with it. It never covers content. It surfaces environment-specific tools
  (comments, feature flags, accessibility audit) only when activated.
- **Source**: [Vercel Toolbar docs](https://vercel.com/docs/workflow-collaboration/vercel-toolbar),
  [Managing toolbar visibility](https://vercel.com/docs/vercel-toolbar/managing-toolbar)

**Confidence**: High — official Vercel documentation.

#### GitHub `<dev>` hostname prefix

GitHub suffixes internal staging hostnames with `-dev` or shows a distinct subdomain.
No in-page banner. Environment is clear from the URL. This approach is only viable when
the URL is visible (browser address bar) — not useful for an embedded webview, but
confirms the principle: environment signal should be cheap and glanceable.

**Confidence**: Medium — observed pattern, no official reference page.

#### Linear staging / feature-flag rollouts

Linear uses feature flags to gate UI changes to internal users before rollout. Their 2024
UI redesign used a flag for private-beta access. In-page signal was subtle: no banner in
production-visible UI; the environment signal was reserved for tooling (flag dashboard,
not user-facing UI). Key takeaway: flag state is surfaced to developers, not embedded in
the live design surface.

- **Source**: [How we redesigned the Linear UI (part II)](https://linear.app/now/how-we-redesigned-the-linear-ui)

**Confidence**: Medium — described pattern, not a visual specification.

#### Storybook's sidebar / story title

Storybook does not use a banner. Instead, the entire shell is the indicator: the sidebar,
the canvas toolbar, and the story title make it unambiguous that you are in a component
workshop, not the running app. Effective for dedicated component-isolation tools but not
applicable to a full-app browser session.

**Source**: [Storybook docs](https://storybook.js.org/docs/writing-stories/mocking-data-and-modules/mocking-network-requests)

#### JavaScript environment-indicator library

`coderpatros/environment-indicator` (GitHub) is a lightweight JS library that injects a
floating chip in a corner of the page labelled with the environment name (dev/staging/prod).
CSS-only, no framework dependency. Exactly the pattern needed for CrossHook's dev mode.

- **Source**: [coderpatros/environment-indicator on GitHub](https://github.com/coderpatros/environment-indicator)

**Confidence**: Medium — matches the pattern; not Tauri-specific.

### Recommended indicator pattern for CrossHook (updated for A-6)

Security advisory A-6 requires the indicator be **non-dismissable** and **visually
distinct from any element in production screenshots**. A corner chip alone can be cropped
out of a screenshot. The updated pattern uses two layers:

#### Layer 1: Inset viewport outline (non-dismissable, crop-resistant)

An amber inset `box-shadow` applied via a `--webdev` modifier class on the root
`.crosshook-app` element. The class is added conditionally by `App()` using the
`__WEB_DEV_MODE__` compile-time constant — the same constant that guards `lib/ipc.ts`.

```tsx
// App.tsx
<main
  ref={gamepadNav.rootRef}
  className={`crosshook-app crosshook-focus-scope${__WEB_DEV_MODE__ ? ' crosshook-app--webdev' : ''}`}
>
```

```css
/* dev-indicator.css — imported only in the webdev entry point,
   never in the production CSS bundle */
.crosshook-app--webdev {
  box-shadow: inset 0 0 0 3px var(--crosshook-color-warning);
}
```

The CSS file is imported inside the `__WEB_DEV_MODE__` branch at the module level (or
via a webdev-only Vite entry point) so it is excluded from the production stylesheet
entirely — the class name never appears in a production bundle.

- `inset box-shadow` does **not** affect the box model — no layout shift, no size change.
- Visible on every route, every modal, every scroll position.
- Cannot be dismissed (it is a CSS modifier class, not a toggleable component).
- Survives any reasonable screenshot crop: the amber outline appears on all four edges.
- Does not exist in any production `.crosshook-app` style — unmistakably dev-only.

**Guard condition (security requirement):** Use `__WEB_DEV_MODE__` exclusively — not
`import.meta.env.VITE_WEB_DEV`. A `VITE_` env variable is inlined at build time from
the shell environment: `VITE_WEB_DEV=true vite build` would accidentally ship the
indicator CSS in a production bundle. `__WEB_DEV_MODE__` is set to a literal `false` for
all non-webdev builds via `vite.config.ts` `define`, so Rollup eliminates the branch
before the CSS class can be included. Using two activation mechanisms (`VITE_WEB_DEV`
alongside `__WEB_DEV_MODE__`) would violate the single-guard-point rule in the security
doc.

#### Layer 2: Corner chip (human-readable label)

A `position: fixed; bottom: 12px; right: 12px; z-index: 9999` chip:

```
[ DEV · {fixture} ]
```

Use `crosshook-status-chip crosshook-status-chip--warning crosshook-dev-chip`:

- `crosshook-status-chip` (theme.css:1007) provides `border-radius: 999px`, `border`,
  `display: inline-flex`, `align-items: center`, `gap: 8px`.
- `crosshook-status-chip--warning` (theme.css:5423) provides the warning amber:
  `background: rgba(217,119,6,0.12)`, `border-color: rgba(217,119,6,0.28)`,
  `color: #d97706`. The darker amber `#d97706` reads better as a status indicator than
  the autosave yellow `#f5c542` — use this class, not the autosave tokens.
- `crosshook-dev-chip` overrides `min-height: var(--crosshook-touch-target-min)` (48px)
  down to a compact size. Follow the same pattern as `.crosshook-offline-badge`
  (theme.css:1435, `min-height: 32px`): `min-height: 32px; padding: 0 10px; font-size: 0.78rem`.
  28px was too small — the offline badge's 32px is already the established compact floor.

**No close button.** Non-dismissable per A-6.

ARIA: `role="status"`, `aria-label="Developer mode active. Fixture: {fixture}"`.

Component shape — MVP (no props, stateless):

```tsx
// Rendered in App.tsx outside <ProfileProvider> — visible even during boot failures
export function DevModeChip() {
  return (
    <span
      className="crosshook-status-chip crosshook-status-chip--warning crosshook-dev-chip"
      role="status"
      aria-label="Browser dev mode — mock data active"
    >
      Dev mode
    </span>
  );
}
```

No new tokens, no inline styles. New CSS required: only the `crosshook-dev-chip` size
override (3 declarations: `min-height`, `padding`, `font-size`).

**Fixture label in chip (UX-7, "Should Have"):** If the fixture-switching mechanism is
implemented (see Fixture Toggling section), the chip label may optionally display the
active fixture: `DEV · empty`. This requires reading `?fixture` from the query string
and passing it as a prop. This is a "Should Have" enhancement on top of the zero-prop
MVP component — the chip is useful and complete without it.

#### Rendering constraint (security requirement)

The chip must render from the layout root — outside `<ProfileProvider>`,
`<PreferencesProvider>`, and all other context providers — so that it appears before any
async fixture data loads and remains visible even if a provider throws during mock
initialization. It must not depend on React state, context, or async resolution to
appear. A screenshot taken at any point after first paint must include the chip.

In `App.tsx`, `__WEB_DEV_MODE__` controls both the outline class and the chip. The
component takes no props — it reads nothing from async state:

```tsx
// App.tsx — both layers applied at the layout root, before providers
declare const __WEB_DEV_MODE__: boolean;

export function App() {
  // ...
  return (
    <main
      ref={gamepadNav.rootRef}
      className={`crosshook-app crosshook-focus-scope${__WEB_DEV_MODE__ ? ' crosshook-app--webdev' : ''}`}
    >
      {__WEB_DEV_MODE__ && <DevModeChip />}
      <ProfileProvider>{/* ... rest of app ... */}</ProfileProvider>
    </main>
  );
}
```

Both layers use the same constant. No `VITE_WEB_DEV` check. Single guard point.

#### Why not a top banner

A full-width top banner shifts all vertical layout by its height. Any height-sensitive
design check (sidebar height, console drawer proportions, route card scroll area) would
measure differently in dev mode than in production. This directly undermines the purpose
of the feature. The inset outline + corner chip combination provides an equally
unmistakable signal with zero layout impact.

#### Why not a full-screen overlay or watermark

A diagonal "PREVIEW" watermark covering the viewport would obscure the design surfaces
being iterated on. The outline + chip is the minimum viable non-dismissable signal that
does not impede design work.

---

## Mock-Data Tooling UX

### How Storybook / MSW signal fake data

MSW (Mock Service Worker) intercepts at the network level — the browser DevTools Network
tab shows requests as handled by the service worker, making the mock origin visible
without any in-page badge. Storybook's Controls panel (bottom tab) lets you switch
between story variants (empty, populated, error, loading) in one click with no rebuild.

Key points from the Storybook / MSW ecosystem:

- Each UI state is a named story with its own fixture set. Switching is immediate.
- A loading state requires a deliberate infinite-delay handler (MSW does not provide this
  by default — a workaround of ~3,600,000 ms delay is commonly used).
- The DevTools Console shows `[MSW] Mocking enabled` at startup and per-request
  interception messages.
- **Source**: [msw-storybook-addon](https://storybook.js.org/addons/msw-storybook-addon),
  [Loading states with MSW](https://dev.to/tmikeschu/loading-states-with-storybook-and-mock-service-worker-50cf)

**Confidence**: High — official addon documentation and corroborating community articles.

### Fixture state switching: query-string vs floating panel

Two approaches are used in practice:

| Approach           | Example                    | Pros                                            | Cons                                  |
| ------------------ | -------------------------- | ----------------------------------------------- | ------------------------------------- |
| Query string       | `?fixture=empty`           | No UI overhead; shareable URL; trivial to parse | Cannot be changed without editing URL |
| Floating dev panel | react-query-devtools style | Click-to-switch; visible state                  | Extra component; covers content       |

**react-query-devtools** is the canonical React floating dev panel: `position: fixed`,
corner-mounted, toggle stored in `localStorage`. It shows a small icon (collapsed) that
expands to a panel. It never causes layout shifts because it is `position: fixed` outside
the document flow.

- **Source**: [react-query-devtools GitHub](https://github.com/tannerlinsley/react-query-devtools)

**Recommendation for CrossHook**: Use query strings as the primary fixture mechanism
(`?fixture=empty`, `?fixture=populated`, `?fixture=error`, `?fixture=loading`) because:

1. The IPC adapter can read `new URLSearchParams(location.search).get('fixture')` at
   startup — no React component needed.
2. Fixture state is preserved on page reload (the URL stays).
3. The current fixture state is shareable: paste the URL into a comment.
4. No floating panel to accidentally screenshot.

The mock-mode chip (described above) can display the current fixture name alongside the
"DEV" label: `[ DEV · empty ]`.

### Required fixture states (IPC surface)

Based on the CrossHook tab structure (Library, Profiles, Launch, Install, Community,
Discover, Compatibility, Settings, Health) and the 84 `invoke()` call sites catalogued in
`research-practices.md`, the minimum set of named fixtures is:

| Fixture name | What it populates                                                                    |
| ------------ | ------------------------------------------------------------------------------------ |
| `populated`  | All lists non-empty: profiles, library summaries, community profiles, health records |
| `empty`      | All lists empty (zero profiles, zero library entries, zero history)                  |
| `error`      | All async IPC calls resolve to error objects (exercises error UI paths)              |
| `loading`    | All async IPC calls never resolve (exercises skeleton/spinner states)                |

The `populated` fixture should be the default when no query param is present. This makes
a fresh `npm run dev` show the full, designed UI immediately — the most common use case.

### Fixture content policy (mandatory — W-3 / A-6)

Fixture data must be **obviously synthetic**. This is not a style preference — it is a
security requirement (W-3 in `research-security.md`) and a condition of A-6. A
screenshot of `populated` fixture data that contains real game names, real Steam App IDs,
or plausible file paths is indistinguishable from a screenshot of the real application.

**Prohibited in fixture files:**

| Category             | Prohibited                                              | Required instead                                             |
| -------------------- | ------------------------------------------------------- | ------------------------------------------------------------ |
| Game names           | Real game titles (`Counter-Strike 2`, `Elden Ring`)     | Fictional names (`Void Protocol`, `Ironfall Alpha`)          |
| Steam App IDs        | Valid range values (e.g. `730`, `1091500`)              | Values ≥ `9_000_001` or clearly fictional prefix (`DEV_001`) |
| File paths           | Real home directories (`/home/yandy/`, `/Users/alice/`) | Fake paths (`/home/devuser/Games/`, `C:/Users/TestUser/`)    |
| Profile names        | Names resembling real user data                         | `Dev Profile 1`, `Test Config Alpha`                         |
| Credentials / tokens | Any value resembling a key or token                     | Not present at all                                           |

Steam App IDs deserve special attention: `730` is CS2, `1091500` is Elden Ring. These
are widely known. A fixture containing `730` in a screenshot is visually identical to a
real CrossHook session with CS2 installed. The primary enforcement mechanism is a named
constant in the fixture module:

```ts
// lib/mocks/fixtures/constants.ts
export const MOCK_APP_ID_BASE = 9_000_001;
```

All mock App IDs are derived from `MOCK_APP_ID_BASE` (e.g. `MOCK_APP_ID_BASE + 0`,
`MOCK_APP_ID_BASE + 1`). Values at or above `9_000_001` are outside the current Steam
catalog range and are unmistakably fake.

**CI grep backstop** — scan `lib/mocks/` only (not the whole repo) for two patterns:

| Pattern                        | What it catches                                                        |
| ------------------------------ | ---------------------------------------------------------------------- |
| `\b[0-9]{7,10}\b`              | Steam App IDs in valid range (7–10 digits; current catalog tops ~2.9B) |
| `\b[0-9]{17}\b`                | Steam User IDs / SteamID64 (17 digits — PII, security scanner flag)    |
| `/home/`, `/users/`, `/Users/` | Real home directory paths                                              |

The `\b[0-9]{7,10}\b` pattern is scoped to `lib/mocks/fixtures/` to avoid false
positives from timestamps, version strings, or port numbers elsewhere in the codebase.
The constant-based primary gate makes the grep a last-resort backstop, not the primary
policy. Both checks belong in `CONTRIBUTING.md` and as a CI lint step.

---

## Browser-vs-Tauri Parity

### What differs between Chrome and WebKitGTK

CrossHook runs in WebKitGTK on Linux (Tauri's webview for the Linux platform). When the
developer opens `http://localhost:5173` in Chrome, they are using a Chromium (Blink)
engine. Differences to be aware of:

#### Scrollbars

| Feature                                          | Chrome (Blink) | WebKitGTK (Tauri/Linux) |
| ------------------------------------------------ | -------------- | ----------------------- |
| `::-webkit-scrollbar-*` support                  | Full           | Partial / inconsistent  |
| `scrollbar-color` / `scrollbar-width` (standard) | Chrome 121+    | Version-dependent       |
| GTK theme influence on scrollbars                | None           | Yes — may override CSS  |

`useScrollEnhance.ts` exists **because** WebKitGTK's native scroll velocity is sluggish.
In Chrome, the same hook runs but the multiplier is unnecessary — Chrome's native scroll
velocity is already responsive. The hook is safe to run in Chrome (it still applies
`scrollTop` deltas; the feel will just be slightly snappier than in the Tauri webview).

**Implication for design iteration**: Do not spend time perfecting scroll feel in Chrome.
Scroll UX must be validated in the actual Tauri webview. All other visual design work
(typography, spacing, color, layout, hover states) is accurate in Chrome.

- **Source**: [Tauri webview discussion](https://github.com/tauri-apps/tauri/discussions/8524),
  [Chrome scrollbar styling](https://developer.chrome.com/docs/css-ui/scrollbar-styling),
  [WebKit scrollbar styling](https://webkit.org/blog/363/styling-scrollbars/)

**Confidence**: High — corroborated by Tauri official discussion and Chrome/WebKit docs.

#### CSS custom scrollbar appearance in dev mode

In Chrome, any `::-webkit-scrollbar` styles will render as defined. In WebKitGTK, the
GTK theme may override them. If the designer adds a custom scrollbar style in Chrome, it
must be validated in Tauri before merging.

#### Window chrome and native menus

Tauri removes the browser window chrome (address bar, tab bar). In Chrome at
`localhost:5173`, the full browser UI is visible. This does not affect the app's internal
design surface but means the total viewport height is smaller in Chrome than in Tauri.
Use `F11` (fullscreen) or a responsive devtools preset at 1400×900 to approximate the
Tauri window dimensions.

#### Font rendering

Chrome on Linux uses FreeType via Skia. WebKitGTK uses FreeType via Cairo. Subpixel
rendering differs subtly between engines. CrossHook uses `'Avenir Next'` as the primary
typeface with system-ui fallback. Avenir is unlikely to be installed on Linux; the
fallback `'Segoe UI'` is also not native to Linux. Both Chrome and WebKitGTK will fall
through to `system-ui`. Rendering is engine-dependent. This is a pre-existing font stack
issue unrelated to dev mode.

#### `color-mix()` and modern CSS

`OfflineStatusBadge.tsx` uses `color-mix(in srgb, ...)`. Chrome 111+ and WebKitGTK
(recent versions) both support this. No parity issue.

#### The `useScrollEnhance` scroll selector list

The SCROLLABLE selector in `useScrollEnhance.ts` lists specific class names:

```
.crosshook-route-card-scroll, .crosshook-page-scroll-body,
.crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body,
.crosshook-modal__body, .crosshook-prefix-deps__log-output,
.crosshook-discovery-results
```

In the browser, these same classes must exist for the hook to function. If mock
components use simplified markup that omits these classes, scroll enhancement will
silently degrade. The mock fixtures should use the real component tree (not stubs) so
that class names are preserved.

---

## Element Inspection Workflow

### What the feature enables

Opening CrossHook's frontend in Chrome unlocks the full Chrome DevTools workflow:

1. **Element picker** (Ctrl+Shift+C): click any element to jump to it in the Elements
   panel. Inspect computed styles, margins, and padding visually with the box model overlay.
2. **Live CSS editing** in the Styles pane: change `--crosshook-color-accent` directly
   on `:root` to preview a palette change across the entire UI instantly.
3. **Workspaces** (Sources → Filesystem → Add folder): Chrome can map network resources
   to local files. Edits in DevTools persist directly to the `src/` directory without
   leaving the browser.
4. **Local Overrides** (Sources → Overrides): mock specific CSS responses without
   modifying source files. Useful for trying destructive changes safely.
5. **Responsive device emulation**: test layout at arbitrary viewport sizes.
6. **Performance panel**: record a layout reflow, identify expensive style recalculations.

- **Sources**: [Chrome DevTools Workspaces](https://developer.chrome.com/docs/devtools/workspaces),
  [View and change CSS](https://developer.chrome.com/docs/devtools/css),
  [Elements panel overview](https://developer.chrome.com/docs/devtools/elements)

**Confidence**: High — official Chrome DevTools documentation.

### What is not available in the Tauri webview

The Tauri webview on Linux exposes the WebKit Web Inspector (accessible via context menu
in dev builds), not Chrome DevTools. It lacks Workspaces integration and some 2024
DevTools features (scroll-driven animation inspector, INP measurement). All design
iteration that benefits from Chrome DevTools should happen in the browser dev mode.

---

## Fixture Toggling — Implementation Pattern

### Query-string approach

```
http://localhost:5173                    → default (populated)
http://localhost:5173?fixture=empty      → empty state
http://localhost:5173?fixture=error      → error state
http://localhost:5173?fixture=loading    → infinite loading
http://localhost:5173?fixture=populated  → explicit populated
```

Parsed once at app startup in the IPC adapter (not in a React component):

```ts
// src/crosshook-native/src/lib/fixtures.ts
export type FixtureKey = 'populated' | 'empty' | 'error' | 'loading';

export function activeFixture(): FixtureKey {
  const param = new URLSearchParams(window.location.search).get('fixture');
  if (param === 'empty' || param === 'error' || param === 'loading') return param;
  return 'populated';
}
```

The mock IPC function map uses `activeFixture()` to select which static objects to
return. Changing the fixture requires a URL change + page reload — no runtime toggle
needed.

### Why not a floating panel

A floating panel (react-query-devtools style) adds a React subtree that the designer
might accidentally screenshot or that might capture focus during keyboard navigation
testing. For a single-developer iteration tool, the URL is sufficient and less intrusive.

If a panel is added later for richer state control (e.g., per-command overrides), the
`react-query-devtools` architecture is the correct model: `position: fixed`,
`z-index: 9999`, toggle stored in `localStorage`, rendered outside the app root via
`ReactDOM.createPortal`.

---

## Performance UX

Performance-related UX concerns for the mock-data dev mode, separated from functional UX.

### Loading states

Each mock handler resolves synchronously-ish via `Promise.resolve(fixture)`. Real Tauri IPC
has a small but non-zero latency (~1–5 ms for trivial commands, higher for filesystem
scans). The mock adapter matches this shape using microtask resolution, which means:

- **Spinners and skeletons flash very briefly** — good enough for screenshot QA, but not
  a realistic timing profile. If the developer needs to see a 500 ms loading state they
  must navigate to `?fixture=loading` which holds the promise indefinitely.
- **No waterfall artifacts** — parallel `callCommand` calls resolve in a single microtask
  flush, so developers will not see the staggered-response behavior a real backend
  produces. This is a documented trade-off; fixing it would require artificial delays
  that hurt iteration speed.

### Optimistic updates

Mock write handlers (`profile_save`, `settings_save`) mutate the in-memory store
synchronously and return the saved payload. Components that optimistically update local
state will feel instant in browser mode just as they do in Tauri mode. Components that
wait for server confirmation (anti-pattern in CrossHook, but present in a few places)
will also feel instant — which may mask real production latency. Document in the PR
that browser mode is not a timing simulator.

### Error feedback timing

Error fixtures (`?fixture=error`) trigger the mock adapter to reject promises. Toast
timing, error-boundary recovery paths, and retry affordances are all testable. Error
severity and color coding can be iterated without touching the Rust side.

### Fixture-state switch cost

Query-string changes require a full page reload (`window.location.search` + reload)
rather than a hot swap. This costs one page load (~200 ms on localhost) but guarantees a
clean store. The alternative — live-swapping fixtures via a floating panel — was
rejected in **Fixture Toggling — Implementation Pattern** as over-engineering for the
stated iteration use case.

### HMR and state reset

Vite HMR invalidates `lib/mocks/*` when fixture files change. The mock singleton and
in-memory store reset. From the developer's perspective:

- **Edit a fixture** → tab re-renders with the new data, no manual reload needed.
- **Edit a component** → React Fast Refresh preserves component state where possible;
  mock store is reset.
- **Edit a CSS file** → style-only HMR, neither store nor component state resets.

This is the expected Vite behavior and requires no custom HMR handling.

### Perceived performance vs real performance

Dev mode is a design tool, not a performance tool. Any profiling, frame-rate analysis, or
real-world latency measurement must happen against a real Tauri build. Document this in
`docs/internal/dev-web-frontend.md` so contributors do not file false positive
performance issues from browser-mode screenshots.

---

## Accessibility of Dev Affordances

### Mock-mode chip

Per MDN and ARIA best practices:

- Use `role="status"` (not `role="alert"`) — the chip is informational, not urgent.
  `role="status"` carries `aria-live="polite"` implicitly, so screen readers announce it
  once at page load without interrupting.
- Do **not** steal focus when the chip mounts. The chip should not be focusable unless it
  is interactive (e.g., a toggle button).
- If the chip shows the current fixture name (e.g., "DEV · empty"), update the
  `aria-label` to match: `aria-label="Developer mode active. Fixture: empty"`.
- Do not add `aria-live="polite"` and `role="status"` together — the role already implies
  the live property.

```tsx
<div role="status" aria-label={`Developer mode active. Fixture: ${fixture}`} className="crosshook-dev-chip">
  DEV · {fixture}
</div>
```

- **Source**: [MDN ARIA alert role](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Reference/Roles/alert_role),
  [Use ARIA to announce updates](https://universaldesign.ie/communications-digital/web-and-mobile-accessibility/web-accessibility-techniques/developers-introduction-and-index/use-aria-appropriately/use-aria-to-announce-updates-and-messaging)

**Confidence**: High — official MDN documentation.

### Fixture links / keyboard access

If fixture-switching links are surfaced in the UI (e.g., a small set of `<a>` tags in
the chip), each must be keyboard-reachable (`tabIndex` is automatic for `<a href>`).
Provide `aria-current="true"` on the active fixture link.

### Dev-mode chip should not break WCAG contrast

`--crosshook-color-warning` (`#f5c542`) on a dark surface (`#1a1a2e`) yields
approximately 8.5:1 contrast ratio — well above WCAG AA (4.5:1) and AAA (7:1) for
normal text. No contrast issue.

---

## Reusable Existing Components

Verified against actual files:

| Asset                             | Location                                | Role in dev chip                                                                                      |
| --------------------------------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `.crosshook-status-chip`          | `theme.css:1007`                        | Base class: `border-radius: 999px`, flex layout, border                                               |
| `.crosshook-status-chip--warning` | `theme.css:5423`                        | Amber colors: `background rgba(217,119,6,0.12)`, `border-color rgba(217,119,6,0.28)`, `color #d97706` |
| `.crosshook-offline-badge`        | `theme.css:1435`                        | Precedent for overriding `min-height` and `padding` on a status chip                                  |
| `OfflineStatusBadge`              | `src/components/OfflineStatusBadge.tsx` | `aria-label` pattern on the outer `<span>`                                                            |
| `HealthBadge`                     | `src/components/HealthBadge.tsx`        | `role="button"` + keyboard handler pattern when interactive                                           |

The dev chip is `crosshook-status-chip crosshook-status-chip--warning crosshook-dev-chip`.
No new tokens. No inline styles. The only new CSS is three declarations in
`crosshook-dev-chip` to compact the base chip's 48px touch target.

---

## Recommendations

### Must Have

| ID   | Recommendation                                                                                        | Rationale                                                                                          |
| ---- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| UX-1 | Inset `box-shadow: inset 0 0 0 3px var(--crosshook-color-warning)` on `.crosshook-app`                | Non-dismissable, crop-resistant outline; zero layout impact; satisfies A-6                         |
| UX-2 | Corner chip `[ Dev mode ]` at `position: fixed; bottom: 12px; right: 12px`, no props, no close button | Human-readable label; survives sidebar collapse; renders before providers; non-dismissable per A-6 |
| UX-3 | `role="status"` + `aria-label` on the dev chip                                                        | Passes a11y audits in dev mode; sets correct precedent                                             |
| UX-4 | Query-string fixture switching (`?fixture=empty/populated/error/loading`)                             | Zero UI overhead; shareable; no React subtree needed                                               |
| UX-5 | `populated` as the default fixture (no query param)                                                   | First `npm run dev` shows a fully designed UI, not an empty shell                                  |
| UX-6 | Document which visual differences between Chrome and Tauri are expected                               | Prevents designers from chasing bugs that only exist in Chrome                                     |

### Should Have

| ID   | Recommendation                                                                                    | Rationale                                                                                                           |
| ---- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| UX-7 | Display active fixture name in the chip (`DEV · empty`) by reading `?fixture` and passing as prop | Removes ambiguity about which fixture state is active; requires fixture-switching mechanism to be implemented first |
| UX-8 | Include a console log at startup: `[dev] mock mode active, fixture: populated`                    | Pairs with the visual chip; helps when chip is partially off-screen                                                 |
| UX-9 | Add scroll containers to `useScrollEnhance` SCROLLABLE selector as new views are added            | Keeps scroll parity consistent; already documented in CLAUDE.md                                                     |

### Nice to Have

| ID    | Recommendation                                                    | Rationale                                                     |
| ----- | ----------------------------------------------------------------- | ------------------------------------------------------------- | ----- | ---------- | --------------------------------------------------- |
| UX-10 | Floating panel (react-query-devtools pattern) as a future upgrade | Useful if per-command overrides are needed; not needed for v1 |
| UX-11 | Playwright screenshot tests that capture all four fixture states  | Guards against fixture regressions; requires Playwright setup |
| UX-12 | Link list in chip: `[ DEV · empty                                 | populated                                                     | error | loading ]` | One-click fixture switching without editing the URL |

---

## Resolved Decisions (from architect + business-analyst input)

| Decision                         | Resolution                                                                                                                                                                            |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Indicator placement              | `position: fixed; bottom: 12px; right: 12px` — sidebar brand and status group are hidden when sidebar collapses, so only `fixed` satisfies BR-3                                       |
| Artificial delay default         | 0ms (instant). Override via `?delay=<ms>`. Exception: `batch_validate_profiles` hard-coded 500ms to keep multi-stage progress indicator visible                                       |
| Cover art in dev mode            | Local `public/mock-art/` PNGs — no CDN dependency, works offline, no CSP questions                                                                                                    |
| Onboarding wizard visibility     | Needs `?onboarding=show` trigger; wizard never fires naturally since the `onboarding-check` Tauri event is never emitted in browser mode                                              |
| `?fixture=loading` resolution    | Never resolves (keeps promises pending indefinitely). Timeout variant (`?fixture=loading&timeout=5000`) is a v2 concern                                                               |
| `?errors=true` orthogonal toggle | Yes: `?fixture=populated&errors=true` returns populated read data but makes all write/action commands throw. Lets designers see error UI in context of a full UI                      |
| Global delay toggle              | `?delay=<ms>` (e.g. `?delay=800` documents "simulated Steam Deck" behavior). No separate flag                                                                                         |
| Library fixture size             | ~12-20 profiles; at least 2 without cover art, at least 1 with broken/stale health status, at least 1 with a very long game name                                                      |
| Onboarding fixture split         | `?onboarding=failed` = 3 passed / 2 failed items; `?onboarding=passed` = all passed                                                                                                   |
| Action command progress          | Synthesized via `setTimeout` → fake `listen` event chain (5–10 events over ~2 seconds). The launch/install/update/run_executable modal is only reachable this way in browser dev mode |

## Open Questions

1. **Does the dev chip need a close button?** Using `role="status"` on a non-landmark
   element should not trigger "redundant landmark" warnings in axe. Verify with axe or
   Lighthouse before shipping. If it does trigger a warning, add a dismiss button with
   `localStorage` persistence.

2. **Chrome window dimensions vs Tauri**: CrossHook's Tauri window has a minimum size
   enforced by Rust. Should a corresponding `min-width` / `min-height` CSS rule be applied
   only in dev mode so Chrome's responsive emulation defaults approximate the Tauri window?

3. **Font stack**: `'Avenir Next'` will not be available in most Linux Chrome sessions.
   Accept the discrepancy (both engines fall through to `system-ui`) and document it, or
   inject a webfont that more closely approximates the intended typeface?

4. **Expected silent no-ops to document**: File picker buttons return null, console drawer
   is empty, destructive actions are no-ops. These should appear as a single "Known
   limitations in dev mode" note in the developer README for this feature, not as UI
   states that need special handling.

---

## Search Queries Executed

1. `Vercel preview deployment banner indicator UI 2024 "dev mode" visual indicator`
2. `Storybook MSW Mock Service Worker DevTools fixture toggle UX 2024`
3. `Linear staging environment indicator Notion debug bar dev mode banner floating chip UI pattern 2024`
4. `WebKitGTK vs Chrome browser CSS scrollbar difference Tauri desktop web parity`
5. `Sentry environment chip GitHub dev indicator React floating dev panel query string fixture toggle pattern`
6. `Chrome DevTools element picker workflow CSS overrides Workspaces design iteration browser 2024`
7. `dev mode banner accessibility ARIA screen reader announcement focusable role 2024 best practice`
8. `Histoire Storybook "fixture" OR "mock" state toggle UI empty state error state loading state developer experience 2024`
9. `"floating dev panel" OR "dev toolbar" React component query param fixture toggle open source github 2024`
10. `WebKitGTK CSS scrollbar webkit-scrollbar vs Chrome scrollbar appearance difference Linux Tauri`

---

## Sources

- [Vercel Toolbar — official docs](https://vercel.com/docs/workflow-collaboration/vercel-toolbar)
- [Managing Vercel Toolbar visibility](https://vercel.com/docs/vercel-toolbar/managing-toolbar)
- [MSW Storybook addon](https://storybook.js.org/addons/msw-storybook-addon)
- [Storybook — Mocking network requests](https://storybook.js.org/docs/writing-stories/mocking-data-and-modules/mocking-network-requests)
- [Loading states with Storybook and MSW](https://dev.to/tmikeschu/loading-states-with-storybook-and-mock-service-worker-50cf)
- [msw-storybook-addon GitHub](https://github.com/mswjs/msw-storybook-addon)
- [Storybook addons for data & state](https://storybook.js.org/blog/storybook-addons-to-manage-data-state/)
- [How we redesigned the Linear UI (part II)](https://linear.app/now/how-we-redesigned-the-linear-ui)
- [coderpatros/environment-indicator](https://github.com/coderpatros/environment-indicator)
- [react-query-devtools GitHub](https://github.com/tannerlinsley/react-query-devtools)
- [Tauri WebKitGTK stability discussion](https://github.com/tauri-apps/tauri/discussions/8524)
- [Tauri webview versions](https://v2.tauri.app/reference/webview-versions/)
- [Chrome scrollbar styling docs](https://developer.chrome.com/docs/css-ui/scrollbar-styling)
- [WebKit — Styling Scrollbars](https://webkit.org/blog/363/styling-scrollbars/)
- [Chrome DevTools Workspaces](https://developer.chrome.com/docs/devtools/workspaces)
- [Chrome DevTools — View and change CSS](https://developer.chrome.com/docs/devtools/css)
- [Chrome DevTools Elements panel](https://developer.chrome.com/docs/devtools/elements)
- [MDN — ARIA alert role](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Reference/Roles/alert_role)
- [MDN — ARIA status role](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Reference/Roles/status_role)
- [Use ARIA to announce updates — CEUD](https://universaldesign.ie/communications-digital/web-and-mobile-accessibility/web-accessibility-techniques/developers-introduction-and-index/use-aria-appropriately/use-aria-to-announce-updates-and-messaging)
- [ARIA live regions — a11y collective](https://www.a11y-collective.com/blog/aria-alert/)
