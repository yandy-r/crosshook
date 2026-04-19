# Tauri WebKitGTK E2E: Prototype Setup Guide

**Status**: Ready for Phase 1 execution
**Date**: 2026-04-19

---

## Overview

This guide provides step-by-step instructions for setting up a local tauri-driver prototype to validate the technical feasibility of WebKitGTK E2E testing for CrossHook.

**Time estimate**: 2-4 hours (initial setup + first test)

---

## Prerequisites

### System Requirements

| Requirement                   | How to Verify                            | Installation                             |
| ----------------------------- | ---------------------------------------- | ---------------------------------------- |
| **Ubuntu/Debian-based Linux** | `lsb_release -a`                         | N/A (required platform)                  |
| **WebKitGTK 2.28+**           | `pkg-config --modversion webkit2gtk-4.1` | Already installed for Tauri dev          |
| **webkit2gtk-driver**         | `which WebKitWebDriver`                  | `sudo apt-get install webkit2gtk-driver` |
| **Rust toolchain**            | `rustc --version`                        | Already installed for CrossHook dev      |
| **Node.js 18+**               | `node --version`                         | Already installed for CrossHook dev      |
| **CrossHook dev environment** | `./scripts/dev-native.sh` works          | Follow CrossHook setup docs              |

### Optional (for headless testing)

```bash
sudo apt-get install xvfb
```

---

## Phase 1: Install tauri-driver

### Option A: Cargo Install (Recommended for Development)

```bash
cargo install tauri-driver
```

**Expected output**:

```
    Updating crates.io index
  Downloaded tauri-driver v2.0.5
  ...
   Compiling tauri-driver v2.0.5
    Finished release [optimized] target(s) in 1m 32s
  Installing ~/.cargo/bin/tauri-driver
   Installed package `tauri-driver v2.0.5`
```

**Verify installation**:

```bash
tauri-driver --version
# Should print: tauri-driver 2.0.5 (or later)
```

### Option B: npm Install (Project-local)

```bash
cd src/crosshook-native
npm install --save-dev tauri-driver
```

**Verify**:

```bash
npx tauri-driver --version
```

---

## Phase 2: Install WebDriverIO

### Install Test Runner

```bash
cd src/crosshook-native
npm install --save-dev \
  webdriverio \
  @wdio/cli \
  @wdio/local-runner \
  @wdio/mocha-framework \
  @wdio/spec-reporter
```

**package.json changes**:

```json
{
  "devDependencies": {
    "@wdio/cli": "^9.0.0",
    "@wdio/local-runner": "^9.0.0",
    "@wdio/mocha-framework": "^9.0.0",
    "@wdio/spec-reporter": "^9.0.0",
    "tauri-driver": "^2.0.5",
    "webdriverio": "^9.0.0"
  }
}
```

---

## Phase 3: Create Test Infrastructure

### Directory Structure

```
src/crosshook-native/
├── tests/
│   ├── e2e/               # New: tauri-driver E2E tests
│   │   ├── specs/
│   │   │   └── webkit-smoke.spec.js
│   │   ├── helpers/
│   │   │   └── workarounds.js
│   │   └── wdio.conf.js
│   ├── smoke.spec.ts      # Existing: Playwright smoke tests
│   ├── collections.spec.ts
│   └── pipeline.spec.ts
```

### Create wdio.conf.js

Create `src/crosshook-native/tests/e2e/wdio.conf.js`:

```javascript
const path = require('node:path');

// Path to the debug Tauri binary (must be built before running tests)
const TAURI_BINARY = path.resolve(__dirname, '../../src-tauri/target/debug/crosshook-native');

exports.config = {
  // Test runner
  runner: 'local',

  // Use tauri-driver as the WebDriver server
  // Note: tauri-driver will auto-detect and launch WebKitWebDriver
  port: 4444,
  path: '/',

  // Test specs
  specs: ['./tests/e2e/specs/**/*.spec.js'],

  // Capabilities
  capabilities: [
    {
      'tauri:options': {
        application: TAURI_BINARY,
        args: [],
        webviewOptions: {},
      },
    },
  ],

  // Test framework
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000, // 60s per test (Tauri launch can be slow)
  },

  // Reporters
  reporters: ['spec'],

  // Logging
  logLevel: 'info',

  // Hooks
  beforeSession: async function (config, capabilities, specs) {
    // Verify Tauri binary exists
    const fs = require('node:fs');
    if (!fs.existsSync(TAURI_BINARY)) {
      throw new Error(
        `Tauri binary not found at: ${TAURI_BINARY}\n` +
          `Build it with: cargo build --manifest-path src/crosshook-native/Cargo.toml -p src-tauri`
      );
    }
  },

  // Services: tauri-driver will be launched automatically by WebDriverIO
  // when it detects the 'tauri:options' capability
  services: [],
};
```

### Create Helper: Command Gap Workarounds

Create `src/crosshook-native/tests/e2e/helpers/workarounds.js`:

```javascript
/**
 * Workarounds for tauri-driver command gaps.
 * See: https://github.com/tauri-apps/tauri/issues/6541
 */

/**
 * Click element via JavaScript (workaround for .click() not supported)
 * @param {Browser} browser - WebDriverIO browser instance
 * @param {Element} element - Element to click
 */
async function clickElement(browser, element) {
  await browser.execute('arguments[0].click();', element);
}

/**
 * Set value via JavaScript (workaround for .setValue() not supported)
 * @param {Browser} browser - WebDriverIO browser instance
 * @param {Element} element - Input element
 * @param {string} value - Value to set
 */
async function setElementValue(browser, element, value) {
  await browser.execute(
    '(el, val) => { el.value = val; el.dispatchEvent(new Event("input", { bubbles: true })); }',
    element,
    value
  );
}

/**
 * Get scroll position of an element
 * @param {Browser} browser - WebDriverIO browser instance
 * @param {Element} element - Scroll container
 * @returns {Promise<{scrollTop: number, scrollLeft: number}>}
 */
async function getScrollPosition(browser, element) {
  return await browser.execute('el => ({ scrollTop: el.scrollTop, scrollLeft: el.scrollLeft })', element);
}

/**
 * Set scroll position of an element
 * @param {Browser} browser - WebDriverIO browser instance
 * @param {Element} element - Scroll container
 * @param {number} scrollTop - Vertical scroll position
 */
async function setScrollPosition(browser, element, scrollTop) {
  await browser.execute('(el, top) => { el.scrollTop = top; }', element, scrollTop);
}

module.exports = {
  clickElement,
  setElementValue,
  getScrollPosition,
  setScrollPosition,
};
```

---

## Phase 4: Write First Test

Create `src/crosshook-native/tests/e2e/specs/webkit-smoke.spec.js`:

```javascript
const { expect } = require('@wdio/globals');
const { clickElement, setScrollPosition, getScrollPosition } = require('../helpers/workarounds');

describe('WebKitGTK Smoke Test', () => {
  it('should launch the Tauri application', async () => {
    const { $ } = browser;

    // Wait for app to be ready (check for a known element)
    const appShell = await $('[data-testid="app-shell"]');
    await appShell.waitForExist({ timeout: 10000 });

    // Verify app launched
    expect(await appShell.isDisplayed()).toBe(true);
  });

  it('should navigate to library route', async () => {
    const { $ } = browser;

    // Find and click library nav item (using workaround)
    const libraryNav = await $('[data-route="/library"]');
    await clickElement(browser, libraryNav);

    // Wait for library page to load
    const libraryPage = await $('[data-testid="library-page"]');
    await libraryPage.waitForExist({ timeout: 5000 });

    // Verify navigation succeeded
    expect(await libraryPage.isDisplayed()).toBe(true);
  });

  it('should test useScrollEnhance behavior (WebKitGTK-specific)', async () => {
    const { $ } = browser;

    // This test targets the WebKitGTK scroll workaround
    // that Chromium smoke cannot validate

    // Navigate to a scrollable route (e.g., library with many items)
    const libraryNav = await $('[data-route="/library"]');
    await clickElement(browser, libraryNav);

    // Find scroll container (adjust selector based on actual markup)
    const scrollContainer = await $('.crosshook-route-stack__body--scroll');

    // Verify container exists
    expect(await scrollContainer.isDisplayed()).toBe(true);

    // Set scroll position
    await setScrollPosition(browser, scrollContainer, 500);

    // Allow time for scroll event handlers (useScrollEnhance)
    await browser.pause(100);

    // Verify scroll position was set
    const { scrollTop } = await getScrollPosition(browser, scrollContainer);
    expect(scrollTop).toBeGreaterThan(400); // Allow some tolerance

    // Additional assertions:
    // - Verify scroll indicators updated
    // - Verify scroll didn't trigger dual-scroll jank
    // (Requires specific test data setup)
  });
});
```

---

## Phase 5: Build and Run

### Build Tauri Debug Bundle

```bash
cd src/crosshook-native

# Build frontend
npm run build

# Build Tauri binary (debug mode for speed)
cargo build -p src-tauri
```

**Expected binary location**:

```
src/crosshook-native/src-tauri/target/debug/crosshook-native
```

### Run tauri-driver Manually (Optional Verification)

In one terminal:

```bash
tauri-driver --port 4444
```

**Expected output**:

```
[tauri-driver] Listening on 0.0.0.0:4444
```

In another terminal:

```bash
# Verify WebKitWebDriver is accessible
which WebKitWebDriver
# Should print: /usr/bin/WebKitWebDriver
```

### Run WebDriverIO Tests

```bash
cd src/crosshook-native
npx wdio tests/e2e/wdio.conf.js
```

**Expected output (success)**:

```
Execution of 1 workers started at 2026-04-19T05:35:00.000Z

[0-0] RUNNING in chrome - /tests/e2e/specs/webkit-smoke.spec.js
[0-0] PASSED in chrome - /tests/e2e/specs/webkit-smoke.spec.js

Spec Files:  1 passed, 1 total (100% completed) in 00:00:15

[tauri-driver] Session ended
```

**Expected output (failure scenarios)**:

1. **Binary not found**:

   ```
   Error: Tauri binary not found at: .../crosshook-native
   Build it with: cargo build ...
   ```

   **Fix**: Run `cargo build -p src-tauri`

2. **WebKitWebDriver not found**:

   ```
   Error: spawn WebKitWebDriver ENOENT
   ```

   **Fix**: Install `webkit2gtk-driver` package

3. **Command gap error**:
   ```
   Error: unknown command: click
   ```
   **Fix**: Use `clickElement()` helper instead of `.click()`

---

## Phase 6: Add npm Script

Add to `src/crosshook-native/package.json`:

```json
{
  "scripts": {
    "test:e2e": "wdio tests/e2e/wdio.conf.js",
    "test:e2e:build": "npm run build && cargo build -p src-tauri && npm run test:e2e"
  }
}
```

**Usage**:

```bash
npm run test:e2e:build  # Build + test
npm run test:e2e        # Test only (assumes binary already built)
```

---

## Phase 7: Headless Mode (Optional)

### Using xvfb

```bash
xvfb-run npm run test:e2e
```

### Using Environment Variable

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 npm run test:e2e
```

**Note**: True headless may have rendering quirks. xvfb is more reliable.

---

## Troubleshooting

### Issue: Tests hang at startup

**Symptoms**: tauri-driver connects but app never launches

**Causes**:

1. Binary path is incorrect
2. Binary has missing dependencies (check with `ldd`)
3. WebKitGTK version mismatch

**Debug**:

```bash
# Verify binary runs manually
./src-tauri/target/debug/crosshook-native

# Check dependencies
ldd ./src-tauri/target/debug/crosshook-native | grep webkit
```

### Issue: "unsupported operation" errors

**Symptoms**: Tests fail with "unknown command" or "unsupported operation"

**Cause**: Using unsupported WebDriver commands (`.click()`, `.setValue()`)

**Fix**: Use JavaScript injection helpers from `workarounds.js`

### Issue: GPU rendering artifacts

**Symptoms**: Screenshots show black/transparent windows, or crashes

**Cause**: Nvidia proprietary driver incompatibility

**Fix**: Use software rendering

```bash
LIBGL_ALWAYS_SOFTWARE=1 npm run test:e2e
```

### Issue: Port conflicts

**Symptoms**: "Address already in use" error on port 4444

**Cause**: Previous tauri-driver instance didn't shut down

**Fix**:

```bash
# Kill stale processes
pkill -f tauri-driver
pkill -f WebKitWebDriver

# Then retry
npm run test:e2e
```

---

## Success Criteria (Phase 1)

After completing this setup, you should be able to:

- [ ] Launch CrossHook via tauri-driver
- [ ] Execute a test that finds an element in the app
- [ ] Navigate between routes using the click workaround
- [ ] Test scroll behavior (WebKitGTK-specific validation)
- [ ] Run tests in headless mode with xvfb
- [ ] Collect timing data (binary build time + test execution time)

**Time investment**: 2-4 hours (initial setup) + 30-60 min per additional test

---

## Next Steps

After Phase 1 success:

1. **Write 2 more focused tests** (file picker IPC, CSS layout assertion)
2. **Measure flakiness**: Run tests 10 times, count failures
3. **Proceed to Phase 3**: CI integration dry run (see [03-ci-integration.md](./03-ci-integration.md))

If Phase 1 fails or reveals showstoppers, document findings in [04-decision-log.md](./04-decision-log.md) and make adopt/defer/drop recommendation.

---

## References

- [tauri-driver Documentation](https://v2.tauri.app/develop/tests/webdriver/)
- [WebDriverIO v9 Docs](https://webdriver.io/docs/gettingstarted)
- [CrossHook Build Scripts](../../scripts/build-native.sh)
- [Issue #350](https://github.com/yandy-r/crosshook/issues/350)
