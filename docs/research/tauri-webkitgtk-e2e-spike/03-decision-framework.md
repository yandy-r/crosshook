# Tauri WebKitGTK E2E: Decision Framework

**Status**: Decision gate after prototype completion
**Date**: 2026-04-19

---

## Purpose

This document provides the decision framework for choosing whether to **adopt**, **defer**, or **drop** tauri-driver for WebKitGTK E2E testing in CrossHook.

Use this after completing the prototype (Phase 1-2) and collecting empirical data.

---

## Decision Inputs

### 1. Prototype Success Metrics

| Metric                        | Measurement                                 | Target  | Weight  |
| ----------------------------- | ------------------------------------------- | ------- | ------- |
| **Technical feasibility**     | Can launch app + execute tests?             | Yes     | Blocker |
| **Local test execution time** | Time from `npm run test:e2e` to completion  | <2 min  | High    |
| **Flakiness rate (local)**    | Failed runs / total runs (N=10)             | ≤20%    | High    |
| **Command gap workarounds**   | Can all target behaviors be tested?         | Yes     | Blocker |
| **Setup complexity**          | Time to set up from scratch (fresh machine) | <1 hour | Medium  |

### 2. CI Feasibility Metrics

| Metric                     | Measurement                        | Target            | Weight  |
| -------------------------- | ---------------------------------- | ----------------- | ------- |
| **CI build time**          | Time to build Tauri debug bundle   | <3 min            | High    |
| **CI test execution time** | Time to run E2E suite              | <2 min            | High    |
| **Total CI overhead**      | Build + test + setup               | <5 min (PRD §3.2) | Blocker |
| **Flakiness rate (CI)**    | Failed CI runs / total runs (N=10) | ≤20%              | High    |
| **Cache effectiveness**    | Time saved with cached binaries    | >50%              | Medium  |

### 3. Coverage Value Metrics

| Metric                             | Measurement                                  | Target     | Weight  |
| ---------------------------------- | -------------------------------------------- | ---------- | ------- |
| **Unique coverage vs. Playwright** | Can catch ≥1 regression class Chromium can't | Yes        | Blocker |
| **Test count needed**              | Number of tests to cover critical gaps       | ≤5         | Medium  |
| **Maintainability**                | Workarounds + brittleness assessment         | Acceptable | Medium  |

---

## Decision Matrix

### Adopt Criteria

**Must satisfy ALL of**:

- [x] Prototype successfully launches app and executes tests
- [x] Catches ≥1 WebKitGTK-specific regression (scroll, file picker, IPC, or CSS)
- [ ] Total CI overhead ≤5 min (with caching), OR can run as scheduled job
- [ ] Flakiness rate ≤20% (acceptable with retries)
- [ ] Setup complexity ≤1 hour (documented)

**AND at least ONE of**:

- [ ] Total CI overhead ≤5 min (fits PR gate budget)
- [ ] Maintainer approves scheduled-job approach (not a PR gate)

**Outcome**: Integrate as either PR gate (if <5 min) or scheduled job (if >5 min)

---

### Defer Criteria

**If ANY of**:

- [ ] Total CI overhead >5 min AND maintainer rejects scheduled-job approach
- [ ] Flakiness rate >20% (even with retries)
- [ ] Prototype reveals showstopper bugs (persistent crashes, data corruption)

**AND**:

- [x] Technical feasibility is confirmed (could work with more maturity)

**Outcome**: Close issue as "deferred to post-v1", document in PRD §9 open questions, revisit after tauri-driver stabilizes

---

### Drop Criteria

**If ANY of**:

- [ ] Prototype fails to launch app (technical infeasibility)
- [ ] Cannot test target behaviors even with workarounds (command gaps too severe)
- [ ] Maintainer assessment: manual WebKitGTK smoke is sufficient for current release cadence

**Outcome**: Close issue as "won't fix", document why, remove from PRD scope

---

## Scoring Template

Fill this out after prototype completion:

### Technical Feasibility

| Question                                            | Answer   | Pass? |
| --------------------------------------------------- | -------- | ----- |
| Can tauri-driver launch CrossHook?                  | [Yes/No] | [ ]   |
| Can tests navigate between routes?                  | [Yes/No] | [ ]   |
| Can tests interact with elements (via workarounds)? | [Yes/No] | [ ]   |
| Can tests assert on WebKitGTK-specific behaviors?   | [Yes/No] | [ ]   |

**Overall**: [PASS / FAIL]

### Performance

| Metric                                | Measured Value | Target | Pass? |
| ------------------------------------- | -------------- | ------ | ----- |
| Local test execution                  | [X]s           | <2 min | [ ]   |
| Tauri debug build time (local)        | [X]s           | <3 min | [ ]   |
| Tauri debug build time (CI, uncached) | [X]s           | <3 min | [ ]   |
| Tauri debug build time (CI, cached)   | [X]s           | <1 min | [ ]   |
| Test execution (CI)                   | [X]s           | <2 min | [ ]   |
| **Total CI overhead**                 | [X]s           | <5 min | [ ]   |

**Overall**: [PASS / FAIL]

### Flakiness

| Scenario             | Runs | Failures | Retry Rate | Target | Pass? |
| -------------------- | ---- | -------- | ---------- | ------ | ----- |
| Local (10 runs)      | 10   | [X]      | [X]%       | ≤20%   | [ ]   |
| CI uncached (5 runs) | 5    | [X]      | [X]%       | ≤20%   | [ ]   |
| CI cached (5 runs)   | 5    | [X]      | [X]%       | ≤20%   | [ ]   |

**Overall**: [PASS / FAIL]

### Coverage Value

| Regression Class      | Can Playwright Catch?     | Can tauri-driver Catch? | Unique Coverage? |
| --------------------- | ------------------------- | ----------------------- | ---------------- |
| useScrollEnhance jank | No (Chromium scroll)      | [Yes/No]                | [Yes/No]         |
| File picker crash     | No (mock IPC)             | [Yes/No]                | [Yes/No]         |
| IPC timing edge case  | Partial (mocks are async) | [Yes/No]                | [Yes/No]         |
| WebKitGTK CSS quirk   | No (Chromium rendering)   | [Yes/No]                | [Yes/No]         |

**Unique coverage classes**: [X] / 4

**Overall**: [PASS / FAIL] (≥1 required)

---

## Decision Tree

```
Start
  |
  v
Technical feasibility PASS?
  |
  +-- No --> DROP (document why)
  |
  +-- Yes
      |
      v
    Unique coverage ≥1?
      |
      +-- No --> DROP (not valuable)
      |
      +-- Yes
          |
          v
        Total CI overhead ≤5 min?
          |
          +-- Yes --> ADOPT as PR gate
          |
          +-- No
              |
              v
            Maintainer approves scheduled job?
              |
              +-- Yes --> ADOPT as scheduled job
              |
              +-- No
                  |
                  v
                Flakiness ≤20%?
                  |
                  +-- Yes --> DEFER (revisit post-v1)
                  |
                  +-- No --> DROP (too flaky)
```

---

## Recommendation Template

After completing the scoring, fill this out:

### Decision: [ADOPT / DEFER / DROP]

**Rationale**:

- Technical feasibility: [PASS/FAIL] — [brief explanation]
- Performance: [PASS/FAIL] — [CI overhead: X min]
- Flakiness: [PASS/FAIL] — [retry rate: X%]
- Coverage value: [PASS/FAIL] — [catches X unique regression classes]

**Proposed integration**:

- [ ] PR gate (required check, <5 min overhead)
- [ ] Scheduled job (nightly/weekly, not a PR gate)
- [ ] Deferred to post-v1
- [ ] Dropped (not valuable)

**Next steps**:

1. [Action item 1]
2. [Action item 2]
3. [Action item 3]

**Risks/caveats**:

- [Risk 1]
- [Risk 2]

**Maintainer sign-off required**: [Yes/No]

---

## Example: Adopt as Scheduled Job

**Decision**: ADOPT as scheduled job (nightly)

**Rationale**:

- Technical feasibility: PASS — App launches, tests execute, workarounds functional
- Performance: FAIL (PR gate) — Total CI overhead 7 min (exceeds 5 min budget)
- Performance: PASS (scheduled) — 7 min is acceptable for nightly run
- Flakiness: PASS — 10% retry rate (within 20% tolerance)
- Coverage value: PASS — Catches useScrollEnhance and file picker regressions

**Proposed integration**:

- [x] Scheduled job (nightly at 00:00 UTC)
- [ ] Manual trigger via workflow_dispatch
- [ ] Notify on failures (GitHub issue auto-created)

**Next steps**:

1. Create `.github/workflows/webkit-e2e-nightly.yml`
2. Write 2 additional focused tests (file picker, CSS layout)
3. Monitor for 30 days, then reassess for PR gate inclusion

**Risks/caveats**:

- Regressions detected 1 day late (not caught in PR)
- May require periodic re-caching of tauri-driver binary

**Maintainer sign-off required**: Yes (scheduled job vs. PR gate is a policy decision)

---

## Example: Defer to Post-v1

**Decision**: DEFER to post-v1

**Rationale**:

- Technical feasibility: PASS — Prototype works
- Performance: FAIL — 9 min CI overhead (even with caching)
- Flakiness: MARGINAL — 25% retry rate (exceeds 20% target)
- Coverage value: PASS — Catches unique regressions
- Maintainer directive: Manual WebKitGTK verification acceptable for current release cadence

**Proposed integration**:

- [ ] Not integrated in v1
- [x] Revisit after tauri-driver matures to v2.1+
- [x] Document in PRD §9 open questions

**Next steps**:

1. Document findings in issue #350
2. Update PRD §9 with "deferred; revisit Q3 2026"
3. Close issue as "deferred" with "v2" milestone

**Risks/caveats**:

- WebKitGTK-only regression may ship before detection
- Maintainer must continue manual WebKitGTK verification

**Maintainer sign-off required**: No (aligns with existing manual process)

---

## Example: Drop

**Decision**: DROP (not valuable)

**Rationale**:

- Technical feasibility: FAIL — WebKitWebDriver crashes consistently on CI
- Performance: N/A (not measurable due to crashes)
- Flakiness: N/A
- Coverage value: N/A (cannot test target behaviors)

**Proposed integration**:

- [x] Not integrated
- [x] Close issue as "won't fix"

**Next steps**:

1. Document crash logs and environment details in issue #350
2. File upstream bug with tauri-driver project
3. Rely on manual WebKitGTK verification until upstream fix

**Risks/caveats**:

- No automated WebKitGTK coverage
- Regression detection relies entirely on maintainer testing

**Maintainer sign-off required**: No (technical infeasibility is objective)

---

## Maintenance Considerations (If Adopted)

### Ongoing Costs

| Task                                     | Frequency                      | Effort    | Owner      |
| ---------------------------------------- | ------------------------------ | --------- | ---------- |
| Update tauri-driver version              | Per Tauri release (~monthly)   | 15 min    | Maintainer |
| Re-cache binaries in CI                  | Per tauri-driver update        | 5 min     | Automated  |
| Fix test breakage from app changes       | Per feature PR (~weekly)       | 10-30 min | PR author  |
| Investigate flaky test failures          | Per flake (~1-2/week)          | 30-60 min | Maintainer |
| Update workarounds if command gaps fixed | Per tauri-driver major release | 1-2 hours | Maintainer |

**Total estimated maintenance**: 2-4 hours/month

**Mitigation**:

- Document workarounds clearly (reduce fix time)
- Limit test count to 3-5 (reduce churn surface)
- Pin tauri-driver version (update deliberately, not automatically)

---

## References

- [01-research-findings.md](./01-research-findings.md)
- [02-prototype-setup.md](./02-prototype-setup.md)
- [Issue #350](https://github.com/yandy-r/crosshook/issues/350)
- [Frontend Test Framework PRD](../../prps/prds/frontend-test-framework.prd.md) (§3.2 budget, §9 open questions)
