# PR #191 Fix Report — Launch Pipeline Stepper Phase 1

**Source review**: [`pr-191-review.md`](../pr-191-review.md) **Date**: 2026-04-09 **Mode**:
`--parallel --severity nitpick` **Findings fixed**: 9/9 (3 MEDIUM, 4 MINOR, 2 NITPICK) **Findings
failed**: 0

---

## Validation

| Check                          | Result |
| ------------------------------ | ------ |
| `npx tsc --noEmit`             | Pass   |
| `cargo test -p crosshook-core` | Pass   |

---

## Batch Execution

Single parallel batch (4 agents, all file groups independent):

| Agent | Findings         | Files                                            | Status |
| ----- | ---------------- | ------------------------------------------------ | ------ |
| A     | F001, F003, F005 | `utils/derivePipelineNodes.ts`                   | Fixed  |
| B     | F002, F007, F008 | `styles/theme.css`, `styles/launch-pipeline.css` | Fixed  |
| C     | F004             | `types/launch.ts`                                | Fixed  |
| D     | F006, F009       | `components/LaunchPipeline.tsx`                  | Fixed  |

---

## Finding Details

### F001 — `optimizations` node false negative (MEDIUM) — Fixed

**File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts` **Change**: `optimizations` case
now returns `'configured'` unconditionally. Empty optimization selection is a valid user choice; the
node no longer cascades a false `not-configured` status to the `launch` node.

### F002 — Orphaned CSS for removed runner indicator (MEDIUM) — Fixed

**File**: `src/crosshook-native/src/styles/theme.css` **Change**: Removed ~15 orphaned CSS rules
targeting removed DOM elements (`__runner-primary-row`, `__indicator`, `__indicator-row`,
`__indicator-dot`, `__indicator[data-state=*]`, `__status[data-phase=*]`). Removed
`__action-guidance` from the combined selector (unused in TSX). Removed orphaned selectors from the
responsive media query. Preserved `__runner-stack`, `__indicator-copy`, and `__indicator-guidance`
(still in use). Moved `@keyframes crosshook-pulse` to `launch-pipeline.css` (see F007).

### F003 — `steam` node unconditionally returns `configured` (MEDIUM) — Fixed

**File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts` **Change**: `steam` case now
checks `profile.steam.app_id.trim() !== ''` instead of unconditionally returning `'configured'`. The
`method` parameter of `tier1Status` is now unused and prefixed as `_method`.

### F004 — `ResolvedLaunchMethod` defined in two locations (MINOR) — Fixed

**File**: `src/crosshook-native/src/types/launch.ts` **Change**: Replaced local
`Exclude<LaunchMethod, ''>` definition with
`export type { ResolvedLaunchMethod } from '../utils/launch'`, establishing `utils/launch.ts` as the
single source of truth. Added an `import type` for local usage by `LaunchPreview`.

### F005 — `tier1Status` accepts untyped `string` for node ID (MINOR) — Fixed

**File**: `src/crosshook-native/src/utils/derivePipelineNodes.ts` **Change**: Extracted
`PipelineNodeId` union type. Applied it to `NODE_DEFS`, `METHOD_NODE_IDS`, and the `tier1Status`
parameter. Removed the `default` branch and added an exhaustive `'launch'` case for compiler safety.

### F006 — No memoization on `derivePipelineNodes()` call (MINOR) — Fixed

**File**: `src/crosshook-native/src/components/LaunchPipeline.tsx` **Change**: Wrapped
`derivePipelineNodes()` call with `useMemo` and `[method, profile, preview, phase]` dependency
array. Added `import { useMemo } from 'react'`.

### F007 — `crosshook-pulse` keyframe defined externally (MINOR) — Fixed

**File**: `src/crosshook-native/src/styles/launch-pipeline.css` **Change**: Added
`@keyframes crosshook-pulse` after the "Status: complete" section, co-located with its only consumer
(line 122). Removed from `theme.css` as part of F002.

### F008 — `display: none` accessible but undocumented (NITPICK) — Fixed

**File**: `src/crosshook-native/src/styles/launch-pipeline.css` **Change**: Added comment
`/* Status text hidden at compact size; announced via aria-label on each <li> */` above the
`display: none` rule.

### F009 — `currentStepIndex` fallback unreachable (NITPICK) — Fixed

**File**: `src/crosshook-native/src/components/LaunchPipeline.tsx` **Change**: Added defensive
comment documenting that the fallback to 0 is unreachable because every pipeline ends with a
`'launch'` node.

---

## Files Modified

| File                                                     | Findings         |
| -------------------------------------------------------- | ---------------- |
| `src/crosshook-native/src/utils/derivePipelineNodes.ts`  | F001, F003, F005 |
| `src/crosshook-native/src/styles/theme.css`              | F002             |
| `src/crosshook-native/src/styles/launch-pipeline.css`    | F007, F008       |
| `src/crosshook-native/src/types/launch.ts`               | F004             |
| `src/crosshook-native/src/components/LaunchPipeline.tsx` | F006, F009       |

---

## Next Steps

- Run `/ycc:code-review` to verify fixes resolved findings
- Run `/ycc:git-workflow` to commit changes
