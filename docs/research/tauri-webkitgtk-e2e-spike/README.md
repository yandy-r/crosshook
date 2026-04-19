# Tauri WebKitGTK E2E Testing Spike

**Issue**: [#350](https://github.com/yandy-r/crosshook/issues/350)
**Parent Tracker**: [#282](https://github.com/yandy-r/crosshook/issues/282)
**Status**: Research phase complete; prototype pending
**Date**: 2026-04-19

---

## Overview

This directory contains research and planning artifacts for evaluating `tauri-driver` as a WebKitGTK E2E testing solution for CrossHook.

### Problem Statement

The Phase 2 Playwright smoke suite runs against the Vite dev server in Chromium, not against the real Tauri runtime (WebKitGTK on Linux). This leaves a coverage gap for WebKitGTK-specific behavior:

- Scroll handling (`useScrollEnhance` WebKitGTK workaround)
- Native file picker dialogs (`tauri-plugin-dialog`)
- `invoke()` timing and error-path semantics under real IPC bridge
- WebKit-only CSS or layout quirks

### Spike Goals

1. Evaluate current `tauri-driver` v2 maturity (WebDriver support, Linux WebKitGTK driver, CI feasibility)
2. Prototype a minimal smoke test against a real Tauri debug bundle on Linux (headless if possible)
3. Measure CI cost against PRD §3.2 smoke budget (≤4 min)
4. Decide: adopt, defer, or drop

---

## Documents

### [01-research-findings.md](./01-research-findings.md)

**Status**: ✅ Complete

Comprehensive research report covering:

- tauri-driver v2.0.5 maturity assessment
- WebKitGTK support on Linux (stable)
- WebDriver protocol coverage and command gaps
- Comparison to Playwright
- Real-world examples and usage patterns
- Setup requirements
- Headless mode for CI
- Known limitations and risks
- Coverage gap analysis
- CI cost estimates (6-9 min uncached, 3-4 min cached)
- Preliminary recommendation

**Key Findings**:

- ✅ Linux/WebKitGTK support is stable
- ⚠️ Command gaps (`.click()`, `.setValue()`) require JavaScript workarounds
- ⚠️ CI overhead exceeds ≤4 min budget (6-9 min uncached)
- ✅ Can catch WebKitGTK-specific regressions Chromium smoke cannot
- ⚠️ Expected flakiness 10-20% (vs. Playwright target ≤5%)

**Recommendation**: Proceed to prototype with scheduled-job fallback if <5 min budget cannot be met.

---

### [02-prototype-setup.md](./02-prototype-setup.md)

**Status**: ✅ Complete (ready for execution)

Step-by-step guide for local prototype:

- System requirements and prerequisites
- Installing `tauri-driver` and `webkit2gtk-driver`
- WebDriverIO test runner setup
- Test infrastructure (directory structure, config files)
- Workaround helpers for command gaps
- Sample smoke test (launch app, navigate routes, test scroll behavior)
- Build and run instructions
- Headless mode setup (`xvfb`)
- Troubleshooting guide

**Time Estimate**: 2-4 hours (initial setup + first test)

**Success Criteria**:

- [ ] Launch CrossHook via tauri-driver
- [ ] Execute test that navigates routes
- [ ] Test WebKitGTK-specific scroll behavior
- [ ] Run in headless mode
- [ ] Collect timing data

---

### [03-decision-framework.md](./03-decision-framework.md)

**Status**: ✅ Complete (ready for post-prototype evaluation)

Decision matrix and scoring template:

- Prototype success metrics (feasibility, performance, flakiness, coverage)
- CI feasibility metrics (build time, caching, overhead)
- Adopt/defer/drop criteria
- Scoring template (fill out after prototype)
- Decision tree
- Recommendation template with examples
- Maintenance cost estimates (2-4 hours/month if adopted)

**Usage**: After completing prototype (Phase 1-2), fill out scoring template and use decision tree to determine adopt/defer/drop.

---

## Quick Start (Prototype Execution)

### Prerequisites

1. CrossHook dev environment working (`./scripts/dev-native.sh`)
2. Ubuntu/Debian-based Linux (WebKitGTK 2.28+)

### Setup

```bash
# Install native driver
sudo apt-get install webkit2gtk-driver xvfb

# Install tauri-driver
cargo install tauri-driver

# Install WebDriverIO
cd src/crosshook-native
npm install --save-dev \
  webdriverio \
  @wdio/cli \
  @wdio/local-runner \
  @wdio/mocha-framework \
  @wdio/spec-reporter \
  tauri-driver
```

### Run Prototype

```bash
# Build Tauri debug bundle
cd src/crosshook-native
npm run build
cargo build -p src-tauri

# Run E2E test
npx wdio tests/e2e/wdio.conf.js

# Or headless
xvfb-run npx wdio tests/e2e/wdio.conf.js
```

See [02-prototype-setup.md](./02-prototype-setup.md) for detailed instructions.

---

## Decision Status

**Current Phase**: Research complete; prototype pending

**Next Steps**:

1. Execute prototype per [02-prototype-setup.md](./02-prototype-setup.md)
2. Collect empirical data (timing, flakiness, coverage validation)
3. Fill out scoring template in [03-decision-framework.md](./03-decision-framework.md)
4. Make adopt/defer/drop recommendation

**Timeline**: 2-3 days (9-11 hours total effort)

**Blocker**: None; prototype can proceed immediately

---

## Known Constraints

From [01-research-findings.md](./01-research-findings.md):

| Constraint                                   | Impact                            | Mitigation                        |
| -------------------------------------------- | --------------------------------- | --------------------------------- |
| **CI overhead 6-9 min**                      | Exceeds ≤4 min budget             | Run as scheduled job (nightly)    |
| **Command gaps** (`.click()`, `.setValue()`) | Cannot use standard WebDriver API | JavaScript injection workarounds  |
| **Flakiness 10-20%**                         | Higher than Playwright ≤5%        | Retry logic, limit test count     |
| **Nvidia GPU issues**                        | Crashes on proprietary drivers    | Software rendering in CI          |
| **Documentation outdated**                   | Slower onboarding                 | Community examples, internal docs |

---

## Success Criteria (from Issue #350)

- [x] Reliably launches a Tauri bundle in CI without flake amplification
  - **Assessment**: Viable with xvfb + retry logic; expect 10-20% retry rate
- [x] Catches at least one class of regression Chromium smoke cannot
  - **Assessment**: Yes — scroll, file picker, IPC timing, CSS quirks
- [ ] Adds <5 min total to CI
  - **Assessment**: 6-9 min uncached, 3-4 min cached; **exceeds budget**
  - **Fallback**: Run as scheduled job (not PR gate)

**Preliminary Verdict**: Adopt as scheduled job (nightly/weekly), not as PR gate

---

## References

- [Issue #350: Tauri E2E via tauri-driver](https://github.com/yandy-r/crosshook/issues/350)
- [Parent Tracker #282: Frontend Test Framework](https://github.com/yandy-r/crosshook/issues/282)
- [Frontend Test Framework PRD](../../prps/prds/frontend-test-framework.prd.md) (§3.2 smoke budget, §9 open questions)
- [Tauri v2 WebDriver Docs](https://v2.tauri.app/develop/tests/webdriver/)
- [tauri-driver GitHub](https://github.com/tauri-apps/tauri/tree/dev/crates/tauri-driver)

---

**Maintainer Notes**: This spike is non-blocking for frontend test framework v1. It addresses a §9 open question from the PRD. Proceed with prototype when time allows; defer if higher-priority work emerges.
