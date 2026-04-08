# Plan: Extract Remaining Component `callCommand()` Usage Into Hooks (Issue #174)

## Summary
This plan removes the last direct IPC adapter calls from `LaunchPage` and `ProfileActions` by moving command orchestration into hooks under `src/crosshook-native/src/hooks/`. It preserves current user-visible behavior (especially error/alert paths) while restoring the architectural invariant that presentational components are not IPC-aware.

Implementation is scoped to the two components confirmed in issue #174 and follows existing hook and context patterns already used across the frontend.

## User Story
As a CrossHook frontend maintainer, I want component-level IPC calls extracted into hooks so that component files remain presentation-focused and easier to test, reuse, and evolve across Tauri and browser-only modes.

## Problem → Solution
`LaunchPage` / `ProfileActions` call `callCommand()` directly → hooks own IPC and components consume hook state/actions only.

## Metadata
- **Complexity**: Medium
- **Source PRD**: N/A (GitHub issue `#174`)
- **PRD Phase**: N/A
- **Estimated Files**: 4-6

---

## UX Design

### Before
```
┌──────────────────────────────────────────────────────────┐
│ Components call `callCommand()` directly                │
│ - LaunchPage checks/install prefix deps via IPC         │
│ - ProfileActions acknowledges version change via IPC     │
│                                                          │
│ Result: UI + IPC logic coupled in component files       │
└──────────────────────────────────────────────────────────┘
```

### After
```
┌──────────────────────────────────────────────────────────┐
│ Components call hook actions only                        │
│ - Hooks encapsulate command names/args/error plumbing    │
│ - Components render state + trigger hook methods         │
│                                                          │
│ Result: same UX, cleaner architecture boundary           │
└──────────────────────────────────────────────────────────┘
```

### Interaction Changes
| Touchpoint | Before | After | Notes |
|---|---|---|---|
| Launch page dependency gate | Component executes `get_dependency_status` / `install_prefix_dependency` | Hook executes those commands; component consumes returned state/actions | No UX change expected |
| Profiles footer “Mark as Verified” | Component executes `acknowledge_version_change` | Hook executes command; component handles result display | Preserve existing alert behavior |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File | Lines | Why |
|---|---|---|---|
| P0 (critical) | `src/crosshook-native/src/components/pages/LaunchPage.tsx` | 155-205, 390-426 | Current direct IPC calls and dep-gate flow |
| P0 (critical) | `src/crosshook-native/src/components/ProfileActions.tsx` | 99-117 | Current direct IPC + error/alert behavior |
| P0 (critical) | `src/crosshook-native/src/hooks/usePrefixDeps.ts` | 19-106 | Existing dependency hook contract/style to mirror |
| P0 (critical) | `src/crosshook-native/src/hooks/useAcknowledgeVersionChange.ts` | 4-27 | Existing acknowledge hook and busy-guard behavior |
| P1 (important) | `src/crosshook-native/src/components/LaunchPanel.tsx` | 625-641 | Existing consumer pattern for `useAcknowledgeVersionChange` |
| P1 (important) | `src/crosshook-native/src/components/PrefixDepsPanel.tsx` | 53-141 | Existing consumer pattern for `usePrefixDeps` |
| P2 (reference) | `src/crosshook-native/src/lib/ipc.ts` | 7-17 | IPC boundary contract/invariant |
| P2 (reference) | `docs/plans/dev-web-frontend/research-practices.md` | 55-83 | Architectural rationale for IPC adapter boundary |

## External Documentation

No external research needed — feature uses established internal patterns and existing project scripts.

| Topic | Source | Key Takeaway |
|---|---|---|
| Architecture intent | `docs/plans/dev-web-frontend/research-practices.md` | Component layer should stay IPC-agnostic |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION
// SOURCE: `src/crosshook-native/src/hooks/usePrefixDeps.ts:6-13`, `src/crosshook-native/src/hooks/useOfflineReadiness.ts:78-90`
```ts
export interface UsePrefixDepsResult {
  deps: PrefixDependencyStatus[];
  loading: boolean;
  error: string | null;
  checkDeps: (packages: string[]) => Promise<void>;
}

export function useOfflineReadiness() {
  const [state, dispatch] = useReducer(reducer, initialState);
}
```

### ERROR_HANDLING
// SOURCE: `src/crosshook-native/src/hooks/usePrefixDeps.ts:15-17`, `src/crosshook-native/src/hooks/useLaunchState.ts:123-125`
```ts
function normalizeError(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
```

### LOGGING_PATTERN
// SOURCE: `src/crosshook-native/src/components/ProfileActions.tsx:106-113`
```ts
const message = error instanceof Error ? error.message : String(error);
console.error('Failed to acknowledge version change', error);
window.alert(`Could not mark profile as verified: ${message}`);
```

### IPC_BOUNDARY_PATTERN
// SOURCE: `src/crosshook-native/src/lib/ipc.ts:7-16`
```ts
export async function callCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(name, args);
  }
  // webdev mock path...
}
```

### SERVICE_PATTERN (Hook Consumed by Component)
// SOURCE: `src/crosshook-native/src/components/PrefixDepsPanel.tsx:53-57`
```ts
const { deps, loading, error, checkDeps, installDep, reload } = usePrefixDeps(
  profileName,
  prefixPath,
);
```

### TEST_STRUCTURE
// SOURCE: `src/crosshook-native/crates/crosshook-core/src/discovery/matching.rs:241-257`
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_splits_on_non_alphanumeric() {
        assert_eq!(tokenize("Elden Ring"), vec!["elden", "ring"]);
    }
}
```

---

## Files to Change

| File | Action | Justification |
|---|---|---|
| `src/crosshook-native/src/components/pages/LaunchPage.tsx` | UPDATE | Replace direct IPC calls with hook usage |
| `src/crosshook-native/src/components/ProfileActions.tsx` | UPDATE | Remove direct IPC import/call and consume hook |
| `src/crosshook-native/src/hooks/useAcknowledgeVersionChange.ts` | UPDATE | Ensure API can support current `ProfileActions` behavior without regression |
| `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts` | CREATE | Encapsulate Launch page prefix dependency IPC logic |
| `src/crosshook-native/src/hooks/index.ts` (if present) | UPDATE (optional) | Export new hook if hook barrel pattern is used |

## NOT Building

- New backend (`src-tauri` / `crosshook-core`) IPC commands
- Frontend test framework adoption (Vitest/Jest setup)
- Refactor of unrelated components already at 0 `callCommand()` usage
- Broader “all components” architecture sweep beyond issue #174 scope

---

## Step-by-Step Tasks

### Task 1: Preflight and Contract Freeze
- **ACTION**: Re-scan target files to confirm direct `callCommand` usage still matches issue scope.
- **IMPLEMENT**: Validate `LaunchPage.tsx` (2x install + 1x status check) and `ProfileActions.tsx` (1x acknowledge).
- **MIRROR**: `usePrefixDeps` for result shape and `useAcknowledgeVersionChange` for busy-guard behavior.
- **IMPORTS**: N/A
- **GOTCHA**: Do not accidentally include already-clean files (e.g., `LaunchPanel`, `HealthDashboardPage`) in this change.
- **VALIDATE**: `rg "callCommand" src/crosshook-native/src/components/pages/LaunchPage.tsx src/crosshook-native/src/components/ProfileActions.tsx` (matches generic-invoked forms like `callCommand<boolean>(...)`; a literal `callCommand\\(` pattern misses those)

### Task 2: Introduce/Adapt Hooks for IPC Extraction
- **ACTION**: Move Launch dependency-gate command calls into a hook and ensure acknowledge hook supports current UX.
- **IMPLEMENT**:
  - Create `useLaunchPrefixDependencyGate` with typed actions for:
    - reading dependency status (`get_dependency_status`)
    - starting installs (`install_prefix_dependency`)
  - Update `useAcknowledgeVersionChange` to return structured outcome (or equivalent) so `ProfileActions` can preserve current alert semantics.
- **MIRROR**: `usePrefixDeps` (`deps/loading/error` + imperative methods), `normalizeError()` pattern.
- **IMPORTS**: `useCallback`, `useState`/`useRef`, `callCommand`, `PrefixDependencyStatus`.
- **GOTCHA**: Existing `LaunchPanel` uses `useAcknowledgeVersionChange`; maintain backward compatibility for that caller.
- **VALIDATE**: Type-check hook signatures against existing consumers.

### Task 3: Refactor `ProfileActions` to Consume Hook
- **ACTION**: Remove direct IPC import and route verify action through hook.
- **IMPLEMENT**:
  - Replace `callCommand('acknowledge_version_change', ...)` in `handleMarkVerified`.
  - Keep busy state wiring and current alert/error behavior for:
    - acknowledge failure
    - post-ack refresh failure
- **MIRROR**: Existing component async handler pattern and `console.error` + `window.alert` messaging style.
- **IMPORTS**: New hook import, remove `callCommand` import.
- **GOTCHA**: `selectedProfile` can be empty in edge states; guard in hook or handler before invoking backend.
- **VALIDATE**: `rg "callCommand" src/crosshook-native/src/components/ProfileActions.tsx` finds no matches (component IPC-agnostic).

### Task 4: Refactor `LaunchPage` to Consume Hook
- **ACTION**: Replace dependency-related direct IPC calls with hook methods.
- **IMPLEMENT**:
  - In `handleBeforeLaunch`, use hook status-check action in place of direct `get_dependency_status`.
  - In auto-install and modal install paths, call hook install action instead of direct `install_prefix_dependency`.
  - Preserve dep-gate state transitions and event-driven completion behavior.
- **MIRROR**: Existing dep-gate state machine in `LaunchPage` and method contract style from `usePrefixDeps`.
- **IMPORTS**: New hook import; remove `callCommand` usage for dependency gate commands.
- **GOTCHA**: Move `check_gamescope_session` into `useLaunchPrefixDependencyGate` (or a dedicated hook) so `LaunchPage` stays IPC-agnostic.
- **VALIDATE**: `rg "callCommand" src/crosshook-native/src/components/pages/LaunchPage.tsx` finds no matches after gamescope logic lives in the hook.

### Task 5: Verification and Scope Guard
- **ACTION**: Run project checks and manual parity smoke tests in both app modes.
- **IMPLEMENT**: Execute static checks + crosshook-core tests + both dev modes.
- **MIRROR**: Repo script guidance in `.cursorrules` and `scripts/dev-native.sh`.
- **IMPORTS**: N/A
- **GOTCHA**: Browser mode uses mocked IPC; verify interaction parity not backend effects.
- **VALIDATE**: Commands below all succeed; manual checklist completed.

---

## Testing Strategy

### Unit Tests

| Test | Input | Expected Output | Edge Case? |
|---|---|---|---|
| Hook command argument mapping | `profileName`, `prefixPath`, package array | Hook sends correct command payload | Yes |
| Acknowledge busy guard | Double-click verify action | Only one in-flight action | Yes |
| Error normalization | Non-`Error` throw payload | String fallback message displayed/logged | Yes |

### Edge Cases Checklist
- [ ] Empty profile name
- [ ] Missing/empty prefix path
- [ ] `get_dependency_status` IPC failure (launch should still follow existing fallback behavior)
- [ ] Install command rejected before event emission
- [ ] Revalidate failure after successful acknowledge
- [ ] Double-trigger while busy

---

## Validation Commands

### Static Analysis
```bash
cd src/crosshook-native && npm run build
```
EXPECT: TypeScript + Vite build succeeds with zero type errors.

### Targeted Scan
```bash
rg "callCommand" src/crosshook-native/src/components/pages/LaunchPage.tsx src/crosshook-native/src/components/ProfileActions.tsx
```
EXPECT: No `callCommand` references in those component files (matches generic forms like `callCommand<T>(...)`; use this instead of `callCommand\\(` which misses them). Alternative: `rg -P 'callCommand(\\s*<[^>]+>)?'` if you need to anchor optional type parameters only.

### Unit Tests / Core Regression
```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```
EXPECT: All `crosshook-core` tests pass.

### Full Test Suite
```bash
cd src/crosshook-native && npm run test:smoke
```
EXPECT: Smoke tests pass (if local Playwright env is configured).

### Browser Validation (if applicable)
```bash
./scripts/dev-native.sh --browser
```
EXPECT: Affected screens render and interactions remain functionally equivalent in webdev mode.

### Native Validation
```bash
./scripts/dev-native.sh
```
EXPECT: Affected screens behave the same in Tauri runtime.

### Manual Validation
- [ ] Open Profiles page, trigger “Mark as Verified” on mismatched profile, confirm behavior unchanged.
- [ ] Force/observe acknowledge failure path and confirm alert messaging still appears.
- [ ] Open Launch page with required prefix deps missing, confirm gate still blocks/installs/continues as before.
- [ ] Confirm no regressions in launch-game / launch-trainer flow transitions.

---

## Acceptance Criteria
- [ ] No direct `callCommand` usage remains in `ProfileActions.tsx`
- [ ] No direct `callCommand` usage remains in `LaunchPage.tsx` (dependency gate and Gamescope session check live in hooks)
- [ ] Hook-based orchestration exists under `src/crosshook-native/src/hooks/`
- [ ] Existing behavior preserved (including busy and error feedback semantics)
- [ ] `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes
- [ ] Manual parity check passes in both native and browser-only dev modes

## Completion Checklist
- [ ] Code follows existing `use*` hook naming and return-shape conventions
- [ ] Component files are IPC-agnostic for issue-scoped logic
- [ ] Error normalization and logging match repository style
- [ ] No hardcoded command payload drift from current behavior
- [ ] No out-of-scope files modified
- [ ] Plan remains self-contained for single-pass implementation

## Risks
| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Behavior drift in `ProfileActions` error messaging | Medium | Medium | Preserve/port existing message strings and flow in hook API contract |
| Launch dep-gate state race during install events | Medium | High | Keep existing event listener flow untouched; only replace command invocation source |
| Hook API change breaks existing `LaunchPanel` usage | Low | Medium | Maintain backward-compatible `useAcknowledgeVersionChange` contract or update both call sites atomically |

## Notes
- Issue #174 scope is explicitly narrowed to two files by preflight scan.
- Existing hooks (`useAcknowledgeVersionChange`, `usePrefixDeps`) provide pattern anchors; implementation can extend them or add page-specific wrappers.
- Architectural north star remains: component layer should not know IPC command names or adapter details.
