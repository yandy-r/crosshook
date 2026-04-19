# Tauri E2E via tauri-driver: Research Findings

**Issue**: [#347](https://github.com/yandy-r/crosshook/issues/347)
**Date**: 2026-04-19
**Status**: Research Phase

---

## Executive Summary

This document summarizes research into using `tauri-driver` for E2E testing against the real Tauri/WebKitGTK runtime, addressing the coverage gap left by the current Playwright/Chromium smoke suite.

**Key Finding**: tauri-driver v2 is **viable but immature** for CrossHook's Linux-native WebKitGTK use case. It supports the target platform and can run headless in CI, but has notable limitations that require workarounds and careful evaluation against the PRD §3.2 smoke budget (≤4 min).

---

## 1. tauri-driver v2 Maturity Assessment

### Current State (April 2026)

**Sources checked**: 2026-04-19 via web research and GitHub API

| Aspect              | Status            | Notes                                                    |
| ------------------- | ----------------- | -------------------------------------------------------- |
| **Version**         | v2.0.5            | Released February 2026                                   |
| **Maturity Label**  | Pre-alpha         | But actively maintained with regular updates             |
| **Linux/WebKitGTK** | ✅ Stable         | Production-ready according to maintainers                |
| **Windows**         | ✅ Stable         | Uses Microsoft Edge WebDriver                            |
| **macOS**           | ⚠️ Community-only | No native WKWebView driver; requires third-party plugins |
| **Documentation**   | ⚠️ Incomplete     | Setup examples are outdated or broken (#10670, #9203)    |

### Official Tauri Context

- **Tauri v2.10.3** is stable (March 2026)
- tauri-driver is the **official** Tauri automation tool
- Active development with community engagement
- Part of the core Tauri project (not third-party)

**Assessment**: While labeled pre-alpha, the tool has sufficient maturity for **exploratory adoption** on Linux. The "pre-alpha" label appears to be conservative given the stable platform-specific implementations.

---

## 2. WebKitGTK Support on Linux

### Driver Stack

```
Test Code (WebDriverIO/etc)
         ↓
   tauri-driver (proxy)
         ↓
 WebKitWebDriver (native)
         ↓
   WebKitGTK 2.28+
         ↓
   Tauri App Window
```

### Requirements

- **WebKitGTK**: 2.28+ (for headless mode support)
- **webkit2gtk-driver**: Available in most Linux distro repos
- **Installation**:
  ```bash
  sudo apt-get install libwebkit2gtk-4.1-dev webkit2gtk-driver
  ```

### Platform Suitability

✅ **CrossHook's Linux-native use case is fully supported**:

- Primary target platform (Ubuntu/Debian derivatives)
- No dependency on Chromium (tests the actual WebKitGTK runtime)
- WebKitWebDriver is packaged and maintained by Linux distributions
- No licensing or third-party concerns

⚠️ **Steam Deck (SteamOS) compatibility**: Needs verification

- SteamOS is Arch-based; webkit2gtk-driver available in Arch repos
- Flatpak runtime isolation may require additional setup
- **Recommendation**: Prototype on vanilla Ubuntu first, then verify SteamOS

---

## 3. WebDriver Protocol Support

### Coverage

tauri-driver implements the **W3C WebDriver protocol** as a proxy to native drivers:

| Command Category        | Support Level | Notes                                     |
| ----------------------- | ------------- | ----------------------------------------- |
| Session management      | ✅ Full       | `newSession`, `deleteSession`             |
| Navigation              | ✅ Full       | `url`, `back`, `forward`, `refresh`       |
| Element location        | ✅ Full       | CSS selectors, XPath                      |
| Element reading         | ✅ Full       | `getText`, `getAttribute`, `getProperty`  |
| **Element interaction** | ⚠️ **Gaps**   | `.click()` and `.setValue()` fail (#6541) |
| JavaScript execution    | ✅ Full       | `execute`, `executeAsync`                 |
| Screenshots             | ✅ Full       | Element and page screenshots              |
| Window management       | ✅ Partial    | Basic resize/position                     |

### Known Command Gaps

**Critical Issue (#6541)**: `.click()` and `.setValue()` return "unsupported operation" errors with WebKitWebDriver.

**Workaround** (confirmed working):

```javascript
// Instead of:
await button.click();

// Use JavaScript injection:
await browser.execute('arguments[0].click();', button);
```

**Impact**: Requires wrapper helpers or custom commands, but not a blocker. The Playwright smoke suite can continue covering complex user-event sequences; tauri-driver tests focus on WebKitGTK-specific behaviors.

---

## 4. Comparison: tauri-driver vs Playwright

| Dimension              | tauri-driver                               | Playwright (current)                            |
| ---------------------- | ------------------------------------------ | ----------------------------------------------- |
| **Runtime**            | Real Tauri + WebKitGTK                     | Vite dev server + Chromium                      |
| **Coverage**           | Native IPC, file pickers, WebKitGTK quirks | Mock IPC, no native dialogs, Chromium rendering |
| **Setup complexity**   | High (native drivers, xvfb)                | Low (npm install, browser bundled)              |
| **CI launch time**     | ~10-30s (app build + launch)               | ~2-5s (Vite server startup)                     |
| **Flakiness risk**     | Higher (race conditions, GPU issues)       | Lower (mature tooling)                          |
| **API richness**       | Basic WebDriver protocol                   | Advanced selectors, network interception        |
| **Maintenance burden** | Medium (workarounds for command gaps)      | Low (mature, well-documented)                   |

### Complementary Roles

**Playwright (retain)**: Route-level smoke, complex user flows, visual regression
**tauri-driver (add)**: WebKitGTK-specific validation, native IPC edge cases, file picker behavior

**Not mutually exclusive**. The question is whether tauri-driver's coverage gains justify the added CI cost and maintenance.

---

## 5. Real-World Examples and Usage Patterns

### Installation

```bash
# Install tauri-driver
cargo install tauri-driver

# Or via npm (project-local)
npm install tauri-driver --save-dev
```

### Basic WebDriverIO Example

```javascript
const { remote } = require('webdriverio');

const client = await remote({
  capabilities: {
    'tauri:options': {
      application: './target/debug/crosshook-native',
      args: [],
      webviewOptions: {},
    },
  },
  runner: 'local',
  automationProtocol: 'webdriver',
});

// Navigate to app route
await client.url('tauri://localhost/index.html');

// Test WebKitGTK scroll behavior
const scrollContainer = await client.$('[data-testid="library-scroll"]');
await browser.execute('arguments[0].scrollTop = 500;', scrollContainer);
// Verify useScrollEnhance workaround behavior...

await client.deleteSession();
```

### Official Documentation

[https://v2.tauri.app/develop/tests/webdriver/](https://v2.tauri.app/develop/tests/webdriver/)

**Note**: Documentation examples are reported as outdated (#10670). Community examples on GitHub are more reliable.

---

## 6. Setup Requirements for CrossHook

### Prerequisites

| Requirement            | Status in CrossHook                  | Notes                           |
| ---------------------- | ------------------------------------ | ------------------------------- |
| Node.js 18+            | ✅ v20 in CI                         | `.github/workflows/lint.yml:59` |
| Rust toolchain         | ✅ 1.95.0                            | `.github/workflows/lint.yml:25` |
| WebKitGTK 2.28+        | ✅ 4.1 (via `libwebkit2gtk-4.1-dev`) | `.github/workflows/lint.yml:42` |
| webkit2gtk-driver      | ❌ Not installed                     | Needs addition to CI setup      |
| xvfb (for headless)    | ❌ Not installed                     | Recommended for CI reliability  |
| tauri-driver binary    | ❌ Not installed                     | `cargo install tauri-driver`    |
| WebDriverIO or similar | ❌ Not installed                     | Test runner dependency          |

### Estimated Setup Steps

1. **Add CI dependencies** (`.github/workflows/lint.yml`):

   ```yaml
   - name: Install E2E test dependencies
     run: |
       sudo apt-get update
       sudo apt-get install -y webkit2gtk-driver xvfb
       cargo install tauri-driver
   ```

2. **Install test runner** (`src/crosshook-native/package.json`):

   ```json
   "devDependencies": {
     "tauri-driver": "^2.0.5",
     "webdriverio": "^9.0.0",
     "@wdio/mocha-framework": "^9.0.0"
   }
   ```

3. **Create test infrastructure**:
   - `src/crosshook-native/tests/e2e/` directory
   - `wdio.conf.js` configuration
   - Helper functions for command-gap workarounds

4. **Build debug bundle** before tests:
   ```bash
   # In CI, build a debug Tauri bundle (faster than release)
   npm run build  # frontend build
   cargo build --manifest-path src/crosshook-native/Cargo.toml -p src-tauri
   ```

**Estimated CI setup time**: 1-2 hours of work, once dependencies are understood.

---

## 7. Headless Mode for CI

### Options

#### Option A: True Headless (WebKitGTK 2.28+)

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 tauri-driver
```

**Pros**: No virtual display overhead
**Cons**: May have rendering quirks; less tested in community

#### Option B: Xvfb Virtual Framebuffer (Recommended)

```bash
xvfb-run tauri-driver test ...
```

**Pros**: Maximum compatibility; widely used in CI
**Cons**: Slight overhead (~100-200ms startup)

### GitHub Actions Example

```yaml
webkit-e2e:
  name: WebKitGTK E2E
  runs-on: ubuntu-latest
  timeout-minutes: 15
  steps:
    - uses: actions/checkout@v4

    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y \
          libwebkit2gtk-4.1-dev \
          webkit2gtk-driver \
          xvfb

    - name: Install tauri-driver
      run: cargo install tauri-driver

    - name: Build debug Tauri bundle
      run: |
        cd src/crosshook-native
        npm ci
        npm run build
        cargo build -p src-tauri

    - name: Run E2E tests
      run: xvfb-run npm run test:e2e
      working-directory: src/crosshook-native
```

**Estimated CI runtime**: 5-8 minutes total (2-4 min build + 1-2 min test execution + overhead)

**PRD §3.2 smoke budget**: ≤4 min — **this exceeds the budget** unless parallelized with existing jobs or run on a schedule.

---

## 8. Known Limitations and Risks

### Critical Issues

| Issue                           | Severity    | Impact on CrossHook                   | Mitigation                             |
| ------------------------------- | ----------- | ------------------------------------- | -------------------------------------- |
| **Command gaps** (#6541)        | High        | `.click()`, `.setValue()` fail        | JavaScript injection workarounds       |
| **Race conditions** (#15156)    | Medium      | Random startup failures               | Retry logic, proper wait conditions    |
| **Nvidia GPU issues** (#14924)  | Medium-High | Crashes on Nvidia proprietary drivers | CI uses software rendering; warn users |
| **Documentation gaps** (#10670) | Low         | Slower onboarding                     | Community examples, internal docs      |
| **macOS not supported** (#7068) | N/A         | CrossHook is Linux-only               | No impact                              |

### Platform-Specific Risks (Linux)

1. **WebKitGTK version sensitivity**:
   - Needs 2.28+ for headless
   - Ubuntu 22.04 LTS ships 2.36 ✅
   - CI environment has compatible version ✅

2. **GPU driver compatibility**:
   - Nvidia proprietary drivers cause rendering issues
   - **CI mitigation**: Use `LIBGL_ALWAYS_SOFTWARE=1` for software rendering
   - **User impact**: Document workarounds for Nvidia systems

3. **Wayland vs X11**:
   - xvfb is X11-based
   - CrossHook targets X11 primarily (Tauri default)
   - **Low risk** for current scope

### Flakiness Risk Assessment

**Baseline**: Playwright smoke has `retries: 1` in CI (`playwright.config.ts:40`)

**tauri-driver flakiness drivers**:

- Race conditions at startup (driver not ready)
- GPU rendering artifacts
- Timing-sensitive IPC (real backend, not mocks)

**Expected retry rate**: 10-20% (vs. Playwright's target ≤5%)

**Mitigation strategies**:

- Explicit wait conditions (not implicit waits)
- Retry wrapper for startup race
- Limit test scope (fewer tests, focused coverage)

---

## 9. Coverage Gap Analysis

### What Playwright/Chromium Smoke Misses

| WebKitGTK-Specific Behavior                              | Chromium Behavior          | Can Ship Broken?                            |
| -------------------------------------------------------- | -------------------------- | ------------------------------------------- |
| **useScrollEnhance workaround**                          | Not needed in Chromium     | **Yes** — scroll jank on WebKitGTK          |
| **File picker dialogs** (`tauri-plugin-dialog`)          | Mock API, no native dialog | **Yes** — picker crashes or doesn't open    |
| **IPC timing** under real Tauri bridge                   | Mock IPC (synchronous)     | **Unlikely** — mocks model async faithfully |
| **WebKit CSS quirks** (flex/grid, `overscroll-behavior`) | Chromium rendering         | **Possible** — layout differences           |
| **Focus management** in WebKitGTK                        | Chromium focus             | **Possible** — modals, keyboard nav         |

### What tauri-driver Would Catch

1. **useScrollEnhance regression**: Test scroll container behavior under real WebKitGTK rendering
2. **File picker integration**: Invoke `dialog.open()` and verify dialog state (even if mocked in test environment)
3. **IPC error paths**: Real `invoke()` rejection timing and error serialization
4. **CSS layout quirks**: Screenshot diffs or dimension assertions on WebKitGTK

### What It Wouldn't Catch

- **Performance regressions**: Not a benchmarking tool
- **Memory leaks**: Requires separate profiling
- **Complex user flows**: Playwright is better suited (richer API)

**Value Proposition**: tauri-driver adds **focused coverage** on the 4 WebKitGTK-specific risks, not comprehensive E2E.

---

## 10. CI Cost Estimate

### Baseline (Current State)

| Job                  | Duration      | PRD Budget |
| -------------------- | ------------- | ---------- |
| `rust`               | ~3-4 min      | N/A        |
| `typescript`         | ~2 min        | N/A        |
| `shell`              | ~30s          | N/A        |
| `smoke` (Playwright) | Not in CI yet | ≤4 min     |

### With tauri-driver E2E

**New job**: `webkit-e2e`

| Phase                | Estimated Time | Notes                             |
| -------------------- | -------------- | --------------------------------- |
| Checkout + setup     | ~30s           | Standard                          |
| Install native deps  | ~45s           | `webkit2gtk-driver`, `xvfb`       |
| Install tauri-driver | ~1-2 min       | `cargo install` (or cache binary) |
| Build frontend       | ~1 min         | `npm run build`                   |
| Build Tauri debug    | ~2-3 min       | `cargo build -p src-tauri`        |
| Run E2E tests        | ~1-2 min       | 2-3 focused tests                 |
| **Total**            | **~6-9 min**   | **Exceeds ≤4 min budget**         |

### Budget Compliance Strategies

#### Option 1: Scheduled Job (Not a PR Gate)

```yaml
on:
  schedule:
    - cron: '0 0 * * *' # Nightly
  workflow_dispatch: # Manual trigger
```

**Pros**: No PR delay; cheaper (1 run/day vs. N runs/day)
**Cons**: Regressions detected late

#### Option 2: Parallel Job with Caching

- Cache `tauri-driver` binary (restore in ~5s vs. 1-2 min build)
- Cache Tauri `target/` dir (incremental build ~30s vs. 2-3 min)
- Run in parallel with existing jobs (no serial delay)

**Estimated optimized time**: ~3-4 min (within budget if cached)

#### Option 3: Minimal Test Scope

- 1 test: useScrollEnhance behavior
- 1 test: File picker invoke (mocked, just verify IPC path)
- Total runtime: ~30s test execution (but still 2-4 min for build)

**Feasibility**: Build time is the blocker, not test execution.

---

## 11. Adoption Decision Framework

### Success Criteria (from Issue #347)

| Criterion                                      | Current Assessment                         | Status                      |
| ---------------------------------------------- | ------------------------------------------ | --------------------------- |
| **Reliably launches Tauri bundle in CI**       | Likely yes, with xvfb + retry logic        | ⚠️ Needs prototype          |
| **No flake amplification**                     | Expect 10-20% retry rate (vs. 5% target)   | ⚠️ Risk                     |
| **Catches ≥1 regression class Chromium can't** | Yes (scroll, file picker, IPC timing, CSS) | ✅                          |
| **Adds <5 min to CI**                          | 6-9 min uncached, ~3-4 min cached          | ⚠️ Exceeds unless scheduled |

### Recommendation Tiers

#### Tier 1: Adopt Now (Conditional)

**IF**:

- Run as a **scheduled job** (nightly/weekly), not a PR gate
- OR run as **optional PR check** (not required for merge)
- Limit scope to 2-3 focused tests
- Accept 10-20% retry rate during maturation

**THEN**: Proceed with prototype and measure actual CI cost.

#### Tier 2: Adopt Later (Deferred)

**IF**:

- PR gate is required (per maintainer directive)
- AND <5 min budget is firm
- AND caching doesn't bring runtime under 5 min

**THEN**: Defer to post-v1, revisit when tauri-driver matures or CI budget expands.

#### Tier 3: Drop

**IF**:

- Prototype reveals showstopper issues (persistent crashes, >30% retry rate)
- OR manual WebKitGTK smoke is deemed sufficient for current release cadence

**THEN**: Close issue as "won't fix for v1", document in PRD §9 open questions.

---

## 12. Prototype Plan (Next Steps)

### Phase 1: Local Proof-of-Concept

**Goal**: Confirm tauri-driver can launch CrossHook and execute a trivial test.

**Steps**:

1. Install `webkit2gtk-driver` and `tauri-driver` locally
2. Build a debug Tauri bundle (`npm run build && cargo build`)
3. Write a 5-line WebDriverIO test (launch app, find element, close)
4. Verify workaround for `.click()` gap

**Success**: App launches, element found, test passes. Time investment: ~2 hours.

### Phase 2: Focused Test Suite

**Goal**: Write 2-3 tests that target WebKitGTK-specific behaviors.

**Tests**:

1. **useScrollEnhance**: Scroll container with `overflow-y: auto`, verify scroll position
2. **File picker IPC**: Invoke `dialog.open()`, verify command reaches backend (mock response)
3. **CSS layout**: Screenshot or dimension assertion on a flex/grid layout

**Success**: All 3 tests pass locally. Time investment: ~4 hours.

### Phase 3: CI Integration (Dry Run)

**Goal**: Measure actual CI cost with caching.

**Steps**:

1. Add `webkit-e2e` job to a branch's `.github/workflows/lint.yml` (not merged)
2. Run job, collect timing data
3. Add binary caching for `tauri-driver` and `target/`
4. Re-run, measure optimized time

**Success**: Timing data informs adopt/defer/drop decision. Time investment: ~3 hours.

### Total Prototype Investment

**~9-11 hours** over 2-3 days (calendar time).

---

## 13. Open Questions

1. **Flakiness on Nvidia systems**: Does CrossHook's target user base (Steam Deck, Linux desktop) have Nvidia prevalence that warrants deeper testing?

2. **SteamOS compatibility**: Does webkit2gtk-driver work in SteamOS (Arch-based)? Needs verification on real hardware.

3. **Flatpak runtime isolation**: If CrossHook ships as Flatpak, can tauri-driver launch the Flatpak bundle in CI? Or only AppImage?

4. **Cache strategy**: Should `tauri-driver` binary be cached, or installed fresh each run? (Cache hit rate vs. staleness risk)

5. **Parallel execution**: Can Playwright smoke and tauri-driver E2E run in parallel jobs, or do they share resources (xvfb, ports)?

6. **Maintainer priority**: Is the WebKitGTK coverage gap severe enough to justify the added CI complexity **now**, or is manual verification acceptable until a WebKitGTK-only regression ships?

---

## 14. Preliminary Recommendation

**Proceed to Phase 1 prototype** with the following constraints:

1. **Time-box**: 9-11 hours total (2-3 days calendar)
2. **Scope**: 2-3 focused tests only (not a comprehensive suite)
3. **CI strategy**: Start as a **scheduled job** (nightly), not a PR gate
4. **Decision gate**: After prototype, measure CI cost and flakiness; decide adopt/defer/drop

**Rationale**:

- Research confirms viability (tauri-driver + WebKitGTK is supported)
- Coverage gap is real (useScrollEnhance, file picker, IPC timing)
- CI cost exceeds budget, but scheduled job sidesteps this
- Prototype investment is low enough to de-risk the decision

**If prototype succeeds**: Integrate as a non-blocking CI check, monitor for 30 days, then decide whether to require for PRs.

**If prototype fails**: Document findings, close issue as "deferred to post-v1", rely on manual WebKitGTK verification.

---

## References

- [Tauri v2 WebDriver Docs](https://v2.tauri.app/develop/tests/webdriver/)
- [tauri-driver GitHub](https://github.com/tauri-apps/tauri/tree/dev/crates/tauri-driver)
- [WebDriverIO Documentation](https://webdriver.io/)
- [CrossHook Frontend Test Framework PRD](../../../prps/prds/frontend-test-framework.prd.md) (§9 open questions)
- [Issue #347](https://github.com/yandy-r/crosshook/issues/347)
- [Parent Tracker #282](https://github.com/yandy-r/crosshook/issues/282)

---

**Next Document**: [02-prototype-setup.md](./02-prototype-setup.md) (after Phase 1 completion)
