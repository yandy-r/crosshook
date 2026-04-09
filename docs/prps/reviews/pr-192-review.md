# Code Review: PR #192 — feat(ui): add preview-derived launch pipeline status

**PR**: [yandy-r/crosshook#192](https://github.com/yandy-r/crosshook/pull/192)
**Branch**: `feat/launch-pipeline-phase2-preview-status` -> `main`
**Author**: yandy-r
**Reviewed**: 2026-04-09
**Mode**: Parallel (3 specialized reviewers: correctness, security, quality)

## Summary

Phase 2 of the launch pipeline visualization. Upgrades pipeline nodes from Tier 1 (config-only) to
Tier 2 (preview-derived) status. Validation issues map to pipeline nodes via machine-readable `code`
strings; detail text shows resolved paths and error messages.

**Scope**: 10 files, +1009/-45 (code ~420 lines, docs ~589 lines)

## Validation Results

| Check | Status |
|-------|--------|
| `cargo test -p crosshook-core` | Pass (777 tests) |
| `npm run build` | Pass (clean TS, Vite build) |
| `npm run test:smoke` | Not run (Playwright not installed) |

## Decision: APPROVE

One high-severity spec deviation identified (Finding 1). No runtime impact today due to severity
gating, but should be fixed before merge or as an immediate follow-up. Remaining findings are
maintainability improvements.

---

## Findings

### Finding 1 — `low_disk_space_advisory` mapped to wrong pipeline node

- **File**: `src/crosshook-native/src/utils/mapValidationToNode.ts:34`
- **Severity**: high
- **Category**: Correctness
- **Status**: Fixed
- **Description**: `LowDiskSpaceAdvisory` (code `low_disk_space_advisory`) is routed to the
  `wine-prefix` node by the prefix rule `code.startsWith('low_disk_space')`. The plan's authoritative
  mapping table and risk table (row 3) both specify it should map to the `launch` summary node
  alongside `OfflineReadinessInsufficient`. The plan's implementation prose contradicts the table
  by listing it under `wine-prefix` — the code followed the prose.

  **No visible UX impact today**: `LowDiskSpaceAdvisory` has `Warning` severity, and `buildTier2Node`
  only promotes to `error` status on `fatal` issues. But if a warning indicator is introduced in
  Phase 4 (Polish), or if severity is escalated, the wrong node would light up.

- **Suggestion**: Remove `|| code.startsWith('low_disk_space')` from line 34. The code then falls
  through to the `return 'launch'` default at line 56, matching the spec table.

  ```diff
  -  if (code.startsWith('runtime_prefix') || code.startsWith('low_disk_space')) {
  +  if (code.startsWith('runtime_prefix')) {
       return 'wine-prefix';
     }
  ```

### Finding 2 — Mock sentinel not covered by CI mock-leak check

- **File**: `src/crosshook-native/src/lib/mocks/handlers/launch.ts:221`
- **Severity**: medium
- **Category**: Security
- **Status**: Fixed
- **Description**: The sentinel string `__MOCK_VALIDATION_ERROR__` is not in the CI mock-code
  check's search pattern list (`.github/workflows/release.yml`). If tree-shaking ever fails to
  remove this path, the sentinel would leak into the production AppImage without the
  `verify no mock code` step catching it. The related `[dev-mock]` prefix string does satisfy the
  CI check; this companion sentinel does not.
- **Suggestion**: Either rename the sentinel to include a CI-checked prefix (e.g.,
  `[dev-mock]__MOCK_VALIDATION_ERROR__`) or add `__MOCK_VALIDATION_ERROR__` to the sentinel list
  in `release.yml`.

### Finding 3 — `sortIssuesBySeverity` duplicated across modules

- **File**: `src/crosshook-native/src/utils/mapValidationToNode.ts:12-19` and
  `src/crosshook-native/src/components/LaunchPanel.tsx:66-69`
- **Severity**: medium
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: `SEVERITY_RANK` and `sortIssuesBySeverity` now exist in two files with identical
  semantics. If a new severity tier is added, both must be updated.
- **Suggestion**: Export `sortIssuesBySeverity` from `mapValidationToNode.ts` and import it in
  `LaunchPanel.tsx` to replace the local copy. `LaunchPanel.tsx`'s
  `sortPatternMatchesBySeverity` (for `PatternMatch`) can stay local since it uses a different type.

### Finding 4 — Cross-boundary contract between Rust `code()` and TS mapper undocumented

- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/request.rs:324` and
  `src/crosshook-native/src/utils/mapValidationToNode.ts:26`
- **Severity**: medium
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: When a developer adds a new `ValidationError` variant, the Rust compiler
  enforces a `code()` arm, but nothing indicates the TS side also needs updating. Unmapped codes
  silently fall through to `'launch'`.
- **Suggestion**: Add a doc comment above `pub fn code(&self)` noting the frontend coupling:

  ```rust
  /// Returns a stable snake_case identifier for this error variant.
  ///
  /// **Frontend coupling**: consumed by `src/utils/mapValidationToNode.ts` (prefix matching).
  /// When adding a new variant, update that file's mapping table. Unmapped codes default to
  /// the `'launch'` summary node.
  ```

  Add a reciprocal `@see` or coupling note in `mapValidationToNode.ts`.

### Finding 5 — Rust test spot-checks only 5 of 44 variants

- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/request.rs:1752-1777`
- **Severity**: medium
- **Category**: Maintainability
- **Status**: Fixed
- **Description**: `validation_error_codes_are_populated` tests 5 representative variants. The
  remaining 39 are uncovered. Without `strum` / `EnumIter`, an exhaustive iterate-all-variants test
  is not available. Adding at least one variant per prefix group in `mapValidationToNode.ts` would
  make gaps visible sooner.
- **Suggestion**: Add assertions for the currently untested `launch`-node variants:

  ```rust
  assert_eq!(ValidationError::UnsupportedMethod("x".into()).code(), "unsupported_method");
  assert_eq!(
      ValidationError::OfflineReadinessInsufficient { score: 0, reasons: vec![] }.code(),
      "offline_readiness_insufficient"
  );
  assert_eq!(
      ValidationError::LowDiskSpaceAdvisory { available_mb: 0, threshold_mb: 1, mount_path: "/".into() }.code(),
      "low_disk_space_advisory"
  );
  ```

### Finding 6 — `profile` prop stability could cause unnecessary useMemo re-runs

- **File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts:32` and
  `src/crosshook-native/src/components/LaunchPanel.tsx:904`
- **Severity**: medium
- **Category**: Performance
- **Status**: Fixed
- **Description**: The `useMemo` dependency array includes `profile`. If the parent re-creates
  the profile object on every render (common with inline form state derivation), `derivePipelineNodes`
  will re-run every render — calling `groupIssuesByNode` (Map allocation + sort) each time.
  Currently low-cost with 3-6 nodes, but worth verifying the parent stabilizes the reference.
- **Suggestion**: Verify `profile` is not recreated on every render at the call site. If it is,
  stabilize with `useMemo` or a stable selector before passing as a prop.

### Finding 7 — `buildLaunchNode` parameter unnecessarily nullable

- **File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts:149`
- **Severity**: low
- **Category**: Type Safety
- **Status**: Fixed
- **Description**: `buildLaunchNode` declares `issuesByNode: Map<...> | null` but is only called
  from the `preview && id === 'launch'` branch where `issuesByNode` is always non-null.
- **Suggestion**: Narrow the type to `Map<PipelineNodeId, LaunchValidationIssue[]>` and remove the
  optional chaining inside the function.

### Finding 8 — `PipelineNode.id` typed as `string` instead of `PipelineNodeId`

- **File**: `src/crosshook-native/src/types/launch.ts:166`
- **Severity**: low
- **Category**: Type Safety
- **Status**: Fixed
- **Description**: `PipelineNode.id` is `string` while all construction sites use `PipelineNodeId`
  values. Nothing enforces valid IDs at the type level.
- **Suggestion**: Move `PipelineNodeId` to `types/launch.ts`, re-export from
  `mapValidationToNode.ts`, and narrow `PipelineNode.id` to `PipelineNodeId`. Can be done in a
  follow-up.

### Finding 9 — Redundant `as const` on mock severity fields

- **File**: `src/crosshook-native/src/lib/mocks/handlers/launch.ts:229,235`
- **Severity**: nitpick
- **Category**: Pattern Compliance
- **Status**: Open
- **Description**: `severity: 'fatal' as const` is redundant — TypeScript infers the literal type
  from the `LaunchValidationIssue` interface. Other severity literals in the same file use plain
  strings without `as const`.
- **Suggestion**: Remove `as const` from both occurrences to match the existing style.

---

## Findings Summary

| # | Severity | Category | File | Status |
|---|----------|----------|------|--------|
| 1 | High | Correctness | mapValidationToNode.ts:34 | Fixed |
| 2 | Medium | Security | handlers/launch.ts:221 | Fixed |
| 3 | Medium | Maintainability | mapValidationToNode.ts:12 | Fixed |
| 4 | Medium | Maintainability | request.rs:324 | Fixed |
| 5 | Medium | Maintainability | request.rs:1752 | Fixed |
| 6 | Medium | Performance | derivePipelineNodes.ts:32 | Fixed |
| 7 | Low | Type Safety | derivePipelineNodes.ts:149 | Fixed |
| 8 | Low | Type Safety | launch.ts:166 | Fixed |
| 9 | Nitpick | Pattern Compliance | handlers/launch.ts:229 | Open |

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 1 |
| Medium | 5 |
| Low | 2 |
| Nitpick | 1 |

## Strengths

- Clean separation of concerns: mapping utility, derivation logic, and component rendering are
  well-decomposed across modules
- The Rust `code()` approach (machine-readable codes over message-string matching) is the right
  architectural choice — robust and maintainable
- Tier 1 fallback is preserved when preview is null — no regressions
- Good a11y: `aria-current`, `aria-label`, and `title` tooltips for progressive disclosure
- Mock fixtures cover both happy path and error states for browser dev mode
- Comprehensive plan and report documentation

## Reviewer Notes

- The HIGH finding (Finding 1) is a one-line fix with no behavioral impact today, but corrects a
  spec deviation that would surface incorrectly in future phases.
- Findings 3-5 (maintainability) are candidates for a follow-up PR rather than blocking merge.
- Playwright smoke tests were not run (browser binary not installed); recommend running locally.
