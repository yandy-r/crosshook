import { defineConfig, devices } from '@playwright/test';

// Local ambient declaration: this config file is loaded by Playwright's own
// TS loader (tsx) at runtime, where Node's `process` global is always
// present. The repo's `tsc --noEmit` only compiles `src/` and never sees
// this file, so we declare the bits we use here instead of adding
// `@types/node` as a devDependency (which would leak Node typings into the
// browser-only `src/` build).
declare const process: { env: Record<string, string | undefined> };

/**
 * Playwright smoke test config for CrossHook browser dev mode.
 *
 * Boots the Vite dev server in `webdev` mode (hand-rolled mock IPC layer
 * under `src/lib/mocks/`), navigates each of the 9 application "routes"
 * (Radix Tabs values, not URL paths), and captures a screenshot per route.
 *
 * Run:
 *   npm run test:smoke
 *
 * First-time setup (downloads ~150MB Chromium binary):
 *   npm run test:smoke:install
 *
 * Update visual baselines:
 *   npm run test:smoke:update
 *
 * This is a Phase 3 polish tool, not a CI gate. Visual regression baselines
 * are opt-in via `test:smoke:update`. Real Tauri behavior must still be
 * verified manually with `./scripts/dev-native.sh` (no flag) before merging
 * UI changes — Playwright in browser dev mode is NOT a parity test for
 * WebKitGTK rendering.
 */
export default defineConfig({
  testDir: './tests',
  testMatch: /.*\.spec\.ts$/,
  // Smoke tests share a single dev server and a module-scoped MockStore
  // singleton — keep them sequential to avoid cross-test state bleed.
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://127.0.0.1:5173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'npm run dev:browser',
    // vite.config.ts pins webdev mode to 127.0.0.1:5173 (loopback only)
    url: 'http://127.0.0.1:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
