# Frontend smoke tests

CrossHook has two frontend layers:

- **Vitest + RTL (happy-dom)** for hooks/components (`src/**/__tests__/`). Commands:
  `npm test`, `npm run test:watch`, `npm run test:coverage`. See `docs/TESTING.md` for patterns and
  IPC mocking guidance.
- **Playwright smoke (this directory)** for route-level sanity against browser dev mode
  (`vite --mode webdev`). Covers navigation, dev-mode chip, and console hygiene.

## First-time setup

Install the Chromium binary used by Playwright (~150 MB, one-time):

```bash
npm run test:smoke:install
```

## Run the smoke suite

```bash
npm run test:smoke
```

The script auto-starts the Vite dev server in `webdev` mode (via the
`webServer` block in `playwright.config.ts`) and tears it down on exit.
If a dev server is already running on `127.0.0.1:5173`, it is reused.

## What the tests assert

For each of the 9 sidebar routes (`library`, `profiles`, `launch`, `install`,
`community`, `discover`, `compatibility`, `settings`, `health`):

1. The dev-mode chip (`role="status"`, label `"Browser dev mode active..."`)
   is visible — proves `__WEB_DEV_MODE__` is true and the mock IPC chunk loaded.
2. The sidebar trigger for the route is reachable by accessible name and clickable.
3. After clicking, the trigger flips to `aria-current="page"`.
4. A full-page screenshot lands in `test-results/smoke-<route>.png`.
5. No `pageerror` events fire and no `console.error` calls occur during the run.

## Update visual baselines

```bash
npm run test:smoke:update
```

Visual regression is opt-in — there are no committed snapshots yet. The
screenshots in `test-results/` are output artifacts, not baselines.

## Reports

- HTML report: `playwright-report/` (open `index.html` in a browser).
- Failure traces: `test-results/` (only on retry of failed tests).

Both directories are gitignored.

## Adding a new route

Append an entry to the `ROUTES` array in `smoke.spec.ts`. The `route`
must match an `AppRoute` value in `src/components/layout/Sidebar.tsx`,
and `navLabel` must match the corresponding `ROUTE_NAV_LABEL` entry in
`src/components/layout/routeMetadata.ts`.

## Caveat (per CLAUDE.md)

Browser dev mode is a developer convenience, **not** a parity test for
WebKitGTK. Real Tauri behavior must still be re-verified manually with
`./scripts/dev-native.sh` (no `--browser` flag) before merging any UI
change. Playwright + Chromium will not catch WebKitGTK-specific scroll
physics, font rendering, focus styles, or `color-mix()` differences.
