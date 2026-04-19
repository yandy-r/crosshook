# CrossHook Native

React + Tauri workspace for the CrossHook desktop app. Run all scripts from this directory.

## Scripts

- `npm run dev` — Vite dev server with the Tauri backend
- `npm run dev:browser` — browser-only dev mode (`--mode webdev`, mock IPC)
- `npm run build` — TypeScript build + Vite production bundle
- `npm run typecheck` — `tsc --noEmit` for app + tests

## Testing

- `npm test` / `npm run test:watch` / `npm run test:coverage` — Vitest + RTL (happy-dom). IPC calls
  should go through `renderWithMocks` + `mockCallCommand`; see `docs/TESTING.md` for patterns and
  pitfalls.
- `npm run test:smoke` — Playwright smoke suite against browser dev mode. Details:
  `src/crosshook-native/tests/README.md`.
- Install browsers once with `npm run test:smoke:install`; update snapshots with
  `npm run test:smoke:update`.
