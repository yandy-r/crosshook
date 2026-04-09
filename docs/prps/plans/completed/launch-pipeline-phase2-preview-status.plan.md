# Plan: Launch Pipeline Visualization — Phase 2 (Preview-Derived Status)

## Summary

Upgrade the launch pipeline visualization from Tier 1 (config-only) to Tier 2 (preview-derived)
status. When a user clicks "Preview", pipeline nodes upgrade to reflect the resolved `LaunchPreview`
output — validation issues map to specific nodes as `error` status with detail text, successfully
resolved fields confirm `configured` status with resolved paths, and `directives_error` maps to the
Optimizations node. The `derivePipelineNodes()` utility gains a Tier 2 code path that activates when
`preview` is non-null, and `LaunchPanel` wires the real `preview` state to `<LaunchPipeline>`.

A small Rust change populates the `code` field on all `ValidationError` variants (the field already
exists on `LaunchValidationIssue` but is `None` for all non-trainer-hash issues). This enables
robust, machine-readable issue-to-node mapping on the frontend without fragile message-string
matching.

**No new IPC commands, no new backend types, no schema migrations, no new dependencies.**

## User Story

As a **profile configurator**, I want the pipeline nodes to reflect validation results from a
Preview so that I can **see which specific step has an error** (red node with detail text) without
opening the Preview modal and reading through a flat issue list.

## Problem -> Solution

**Current state (Phase 1 merged, commit `5a3855e`)**:

- `derivePipelineNodes()` (`src/crosshook-native/src/utils/derivePipelineNodes.ts:14-39`) accepts
  `_preview: LaunchPreview | null` and `_phase: LaunchPhase` but ignores both (underscore-prefixed).
  Only `tier1Status()` is called, producing `'configured'` or `'not-configured'` from profile field
  emptiness checks.
- `LaunchPanel.tsx:904` passes `preview={null}` to `<LaunchPipeline>`, even though the `preview`
  variable from `usePreviewState()` is already in scope at line 635.
- `PipelineNode.detail` is typed (`types/launch.ts:173`) but never populated.
- `ValidationError::issue()` (`request.rs:324-333`) always sets `code: None`. Only
  `trainer_hash_mismatch` and `trainer_hash_community_mismatch` have machine-readable codes. This
  means frontend cannot reliably map issues to nodes by code.

**Desired state**:

- After clicking "Preview", pipeline nodes reflect resolved validation:
  - Nodes with fatal validation issues show `error` status (red X) with the issue message as `detail`
  - Nodes whose preview data resolved successfully show `configured` with resolved path as `detail`
  - `directives_error` maps to the Optimizations node as `error`
  - Launch (summary) node shows `error` if any fatal issue exists
- Rust `ValidationError::issue()` populates `code` from variant names, enabling robust mapping
- Mock preview handler includes validation issue fixtures for browser dev mode testing

## Metadata

- **Complexity**: **Medium** (10 tasks across 3 batches; 3 files UPDATE, 1 file CREATE; 1 Rust file
  UPDATE)
- **Source PRD**:
  [`docs/prps/prds/launch-pipeline-visualization.prd.md`](../prds/launch-pipeline-visualization.prd.md)
  Phase 2 (lines 367-373)
- **Source Issue**:
  [`yandy-r/crosshook#188`](https://github.com/yandy-r/crosshook/issues/188)
- **Parent epic**:
  [`yandy-r/crosshook#74`](https://github.com/yandy-r/crosshook/issues/74) — Launch pipeline
  visualization
- **Depends on**: Phase 1 merged (`5a3855e` — pipeline component, Tier 1 config status, CSS, a11y)
- **Blocks**: Phase 3 (live launch animation) which overlays runtime phase on top of Tier 1/2 status
- **Estimated files**: ~5 (1 CREATE, 4 UPDATE)
  - CREATE: `src/crosshook-native/src/utils/mapValidationToNode.ts`
  - UPDATE: `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (populate `code`)
  - UPDATE: `src/crosshook-native/src/utils/derivePipelineNodes.ts` (Tier 2 logic)
  - UPDATE: `src/crosshook-native/src/components/LaunchPanel.tsx` (wire `preview` prop)
  - UPDATE: `src/crosshook-native/src/lib/mocks/handlers/launch.ts` (validation issue fixtures)

## Storage / Persistence

**No new persisted data in Phase 2.** All pipeline node status is runtime-only, derived from
existing `GameProfile` + `LaunchPreview` + `LaunchPhase` on each render.

| Datum                      | Classification             | Where                                        | Migration  |
| -------------------------- | -------------------------- | -------------------------------------------- | ---------- |
| Pipeline node status       | **Runtime-only** (derived) | `derivePipelineNodes()` return value          | N/A        |
| Pipeline node `detail`     | **Runtime-only** (derived) | `PipelineNode.detail` field                  | N/A        |
| Preview data (Tier 2 src)  | **Runtime-only** (state)   | `usePreviewState()` hook in `LaunchPanel`     | N/A        |
| Validation issue `code`    | **Runtime-only** (IPC DTO) | `LaunchValidationIssue.code` (already exists) | N/A        |

**Offline**: Pipeline derives from local profile + preview data. Fully functional offline.
**Degraded**: When `LaunchPreview` is null, falls back to Tier 1 (config-derived). Never blank.
**User visibility**: Read-only. Users see richer status after Preview; edit config via existing tabs.

---

## Patterns to Mirror

| Pattern                                      | Source                                                             | Use it for                                                        |
| -------------------------------------------- | ------------------------------------------------------------------ | ----------------------------------------------------------------- |
| `tier1Status()` per-node switch              | `derivePipelineNodes.ts:60-81`                                     | `tier2Status()` follows same structure with preview data          |
| `data-status` CSS selectors                  | `launch-pipeline.css:84-137`                                       | `error` status styling is already implemented (lines 108-116)     |
| `PipelineNode.detail` field                  | `types/launch.ts:173`                                              | Populate with resolved paths or error messages                    |
| `ValidationError` variant naming             | `request.rs:243-321`                                               | Derive `code` strings from variant names (snake_case)             |
| `code`-based routing in UI                   | `LaunchSubTabs.tsx:359,374` (`issue.code === 'trainer_hash_...'`)  | Same pattern for node-to-issue routing via `code`                 |
| Mock handler fixture dispatch                | `handlers/launch.ts:211-258`                                       | Add conditional validation issues to `preview_launch` mock        |
| `sortIssuesBySeverity()` precedence          | `LaunchPanel.tsx:66-69` (`fatal: 0, warning: 1, info: 2`)         | Issue severity ranking when multiple issues hit the same node     |
| Conventional commit title                    | `5a3855e` (`feat(ui): add launch pipeline stepper phase 1 (#191)`) | `feat(ui): add preview-derived pipeline status phase 2 (#188)`    |

---

## References (file:line)

### Code under modification

- `src/crosshook-native/src/utils/derivePipelineNodes.ts:1-82` (entire file — add Tier 2 logic)
- `src/crosshook-native/src/components/LaunchPanel.tsx:904` (`preview={null}` -> `preview={preview}`)
- `src/crosshook-native/src/components/LaunchPanel.tsx:635` (`preview` from `usePreviewState()`)
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs:324-333` (`issue()` method)
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts:211-258` (mock `preview_launch` handler)

### Types & contracts

- `src/crosshook-native/src/types/launch.ts:54-64` (`LaunchValidationIssue`)
- `src/crosshook-native/src/types/launch.ts:142-158` (`LaunchPreview`)
- `src/crosshook-native/src/types/launch.ts:160-173` (`PipelineNode`, `PipelineNodeStatus`)
- `src/crosshook-native/src/types/launch.ts:122-127` (`ProtonSetup`)
- `src/crosshook-native/src/types/launch.ts:129-134` (`PreviewTrainerInfo`)

### Rust validation (read-only reference)

- `crates/crosshook-core/src/launch/request.rs:243-321` (`ValidationError` enum — all variants)
- `crates/crosshook-core/src/launch/request.rs:336-671` (`message()`, `help()`, `severity()`)
- `crates/crosshook-core/src/launch/request.rs:777-800` (`validate_all()` dispatch)
- `crates/crosshook-core/src/launch/request.rs:893-978` (`collect_steam_issues`,
  `collect_proton_issues`, `collect_native_issues`)
- `crates/crosshook-core/src/launch/request.rs:721-728` (`collect_custom_env_issues`)
- `crates/crosshook-core/src/launch/request.rs:729-776` (`collect_gamescope_issues`)
- `crates/crosshook-core/src/launch/preview.rs:267-414` (`build_launch_preview()`)
- `crates/crosshook-core/src/launch/preview.rs:716-807` (`build_proton_setup()`,
  `build_trainer_info()`)

### Sibling artifacts

- `docs/prps/prds/launch-pipeline-visualization.prd.md` (PRD — Phases 1-4)
- Phase 1 commit: `5a3855e` (`feat(ui): add launch pipeline stepper phase 1 (#191)`)

---

## Gotchas / Risks

| #   | Risk                                                                                          | Likelihood | Mitigation                                                                                                                                                                                                                                           |
| --- | --------------------------------------------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **`ValidationError::issue()` code change is a Rust modification**                             | Certain    | The PRD says "no new IPC commands or backend types." This change populates an **existing** field (`code: Option<String>`) on an **existing** struct. No new types, no new commands. The alternative — message-string matching — is fragile and unmaintainable. |
| 2   | **Multiple validation issues can target the same node**                                       | High       | Use severity precedence: `fatal > warning > info`. The first (highest severity) issue's message becomes the node's `detail`. Lower-severity issues are visible in the Preview modal.                                                                  |
| 3   | **Gamescope/env-var/advisory issues have no natural pipeline node**                            | Medium     | Map gamescope and custom env-var issues to the `optimizations` node (closest semantic match). Advisory warnings (`OfflineReadinessInsufficient`, `LowDiskSpaceAdvisory`) map to the `launch` (summary) node. `UnshareNetUnavailable` maps to `trainer`. |
| 4   | **Trainer hash warnings (`trainer_hash_mismatch`) only appear at launch time, not in preview** | Low        | Phase 2 is preview-derived status. These codes are handled correctly if they ever appear in preview data (mapping to `trainer` node), but they won't surface until Phase 3 (live launch). No action needed now.                                        |
| 5   | **`isStale()` preview indicator not wired to pipeline**                                        | Low        | Per PRD risk table: "Show subtle 'stale' indicator when preview is > 60s old." Defer to Phase 4 (Polish). Tier 2 shows whatever preview data exists; Tier 1 is always current as fallback.                                                            |
| 6   | **Mock preview always returns empty validation issues**                                        | Certain    | T3 adds validation issue fixtures to the mock handler so browser dev mode can exercise error states. Without this, the `error` status path is untestable in dev mode.                                                                                 |

---

## Validation Issue -> Pipeline Node Mapping

The mapping is based on which `collect_*` function emits each `ValidationError` variant and the
semantic domain of the error. Each variant gets a `code` string derived from its Rust enum name
(PascalCase -> snake_case).

| Validation Error Variant                    | Code (snake_case)                            | Pipeline Node      | Severity |
| ------------------------------------------- | -------------------------------------------- | ------------------ | -------- |
| `GamePathRequired`                          | `game_path_required`                         | `game`             | Fatal    |
| `GamePathMissing`                           | `game_path_missing`                          | `game`             | Fatal    |
| `GamePathNotFile`                           | `game_path_not_file`                         | `game`             | Fatal    |
| `NativeWindowsExecutableNotSupported`       | `native_windows_executable_not_supported`    | `game`             | Fatal    |
| `RuntimePrefixPathRequired`                 | `runtime_prefix_path_required`               | `wine-prefix`      | Fatal    |
| `RuntimePrefixPathMissing`                  | `runtime_prefix_path_missing`                | `wine-prefix`      | Fatal    |
| `RuntimePrefixPathNotDirectory`             | `runtime_prefix_path_not_directory`          | `wine-prefix`      | Fatal    |
| `RuntimeProtonPathRequired`                 | `runtime_proton_path_required`               | `proton`           | Fatal    |
| `RuntimeProtonPathMissing`                  | `runtime_proton_path_missing`                | `proton`           | Fatal    |
| `RuntimeProtonPathNotExecutable`            | `runtime_proton_path_not_executable`         | `proton`           | Fatal    |
| `SteamAppIdRequired`                        | `steam_app_id_required`                      | `steam`            | Fatal    |
| `SteamCompatDataPathRequired`               | `steam_compat_data_path_required`            | `steam`            | Fatal    |
| `SteamCompatDataPathMissing`                | `steam_compat_data_path_missing`             | `steam`            | Fatal    |
| `SteamCompatDataPathNotDirectory`           | `steam_compat_data_path_not_directory`       | `steam`            | Fatal    |
| `SteamProtonPathRequired`                   | `steam_proton_path_required`                 | `steam`            | Fatal    |
| `SteamProtonPathMissing`                    | `steam_proton_path_missing`                  | `steam`            | Fatal    |
| `SteamProtonPathNotExecutable`              | `steam_proton_path_not_executable`           | `steam`            | Fatal    |
| `SteamClientInstallPathRequired`            | `steam_client_install_path_required`         | `steam`            | Fatal    |
| `TrainerPathRequired`                       | `trainer_path_required`                      | `trainer`          | Fatal    |
| `TrainerHostPathRequired`                   | `trainer_host_path_required`                 | `trainer`          | Fatal    |
| `TrainerHostPathMissing`                    | `trainer_host_path_missing`                  | `trainer`          | Fatal    |
| `TrainerHostPathNotFile`                    | `trainer_host_path_not_file`                 | `trainer`          | Fatal    |
| `NativeTrainerLaunchUnsupported`            | `native_trainer_launch_unsupported`          | `trainer`          | Fatal    |
| `UnknownLaunchOptimization`                 | `unknown_launch_optimization`                | `optimizations`    | Fatal    |
| `DuplicateLaunchOptimization`               | `duplicate_launch_optimization`              | `optimizations`    | Fatal    |
| `IncompatibleLaunchOptimizations`           | `incompatible_launch_optimizations`          | `optimizations`    | Fatal    |
| `LaunchOptimizationDependencyMissing`       | `launch_optimization_dependency_missing`     | `optimizations`    | Fatal    |
| `LaunchOptimizationsUnsupportedForMethod`   | `launch_optimizations_unsupported_for_method`| `optimizations`    | Fatal    |
| `LaunchOptimizationNotSupportedForMethod`   | `launch_optimization_not_supported_for_method`| `optimizations`   | Fatal    |
| `GamescopeBinaryMissing`                    | `gamescope_binary_missing`                   | `optimizations`    | Fatal    |
| `GamescopeNotSupportedForMethod`            | `gamescope_not_supported_for_method`         | `optimizations`    | Fatal    |
| `GamescopeResolutionPairIncomplete`         | `gamescope_resolution_pair_incomplete`       | `optimizations`    | Fatal    |
| `GamescopeFsrSharpnessOutOfRange`           | `gamescope_fsr_sharpness_out_of_range`       | `optimizations`    | Fatal    |
| `GamescopeFullscreenBorderlessConflict`     | `gamescope_fullscreen_borderless_conflict`   | `optimizations`    | Fatal    |
| `GamescopeNestedSession`                    | `gamescope_nested_session`                   | `optimizations`    | Warning  |
| `CustomEnvVarKeyEmpty`                      | `custom_env_var_key_empty`                   | `optimizations`    | Fatal    |
| `CustomEnvVarKeyContainsEquals`             | `custom_env_var_key_contains_equals`         | `optimizations`    | Fatal    |
| `CustomEnvVarKeyContainsNul`               | `custom_env_var_key_contains_nul`            | `optimizations`    | Fatal    |
| `CustomEnvVarValueContainsNul`             | `custom_env_var_value_contains_nul`          | `optimizations`    | Fatal    |
| `CustomEnvVarReservedKey`                   | `custom_env_var_reserved_key`                | `optimizations`    | Fatal    |
| `UnsupportedMethod`                         | `unsupported_method`                         | `launch`           | Fatal    |
| `UnshareNetUnavailable`                     | `unshare_net_unavailable`                    | `trainer`          | Warning  |
| `OfflineReadinessInsufficient`              | `offline_readiness_insufficient`             | `launch`           | Warning  |
| `LowDiskSpaceAdvisory`                      | `low_disk_space_advisory`                    | `launch`           | Warning  |
| *(existing)* `trainer_hash_mismatch`        | `trainer_hash_mismatch`                      | `trainer`          | Warning  |
| *(existing)* `trainer_hash_community_mismatch` | `trainer_hash_community_mismatch`         | `trainer`          | Warning  |

---

## Test Plan

### Automated — Rust unit tests

Existing tests in `crates/crosshook-core/src/launch/preview.rs:843-1409` exercise
`build_launch_preview()` with various method/validation scenarios. After T1 populates `code` on
validation issues, these tests must still pass — `code` is a new non-breaking field.

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

### Automated — Frontend build

TypeScript compilation validates type safety of the new mapping utility and updated
`derivePipelineNodes()`.

```bash
cd src/crosshook-native && npm run build
```

### Manual — Browser dev mode verification

After T3 (mock fixtures) and T5 (wiring), open browser dev mode and:

1. Navigate to Launch page with a populated profile
2. Click "Preview" — pipeline nodes should upgrade from Tier 1 to Tier 2
3. Switch mock fixture to one with validation issues — nodes should show `error` status
4. Verify `detail` text appears (tooltip or in status text area)
5. Verify the Pipeline component renders without console errors

### Automated — Playwright smoke

Existing `tests/smoke.spec.ts` navigates to the launch route and asserts zero console errors. Phase
2 changes must not break this.

```bash
cd src/crosshook-native && npm run test:smoke
```

---

## Tasks

Each task is self-contained with file paths, dependencies, and explicit acceptance. Tasks annotated
`Depends on [...]` form a DAG for parallel execution.

### Batch A — Independent (all parallel-safe)

**T1 — Rust: Populate `code` on `ValidationError::issue()`** _Depends on []._ Update
`src/crosshook-native/crates/crosshook-core/src/launch/request.rs:324-333`. The `issue()` method
currently returns `code: None` for all `ValidationError` variants. Change it to derive a
`code: Some(...)` string from the variant name using PascalCase-to-snake_case conversion.

Implementation approach:
- Add a `code(&self) -> &'static str` method on the `ValidationError` enum that returns a
  snake_case string for each variant (e.g., `GamePathRequired` -> `"game_path_required"`,
  `SteamAppIdRequired` -> `"steam_app_id_required"`). For variants with inner data
  (e.g., `UnknownLaunchOptimization(String)`), the code is the variant name only (no inner data
  in the code string).
- Update `issue()` at line 329 from `code: None` to `code: Some(self.code().to_string())`.
- The existing `trainer_hash_mismatch` and `trainer_hash_community_mismatch` codes (set on
  `LaunchValidationIssue` named constructors at lines 218-240) are NOT changed — they use a
  different code path and are already correct.

Verification:
```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

Acceptance: all existing tests pass; every `ValidationError::issue()` call now produces a non-None
`code` field; no new test file needed (existing preview tests exercise `issue()` indirectly).

---

**T2 — Create `mapValidationToNode` utility** _Depends on []._ Create
`src/crosshook-native/src/utils/mapValidationToNode.ts`. This pure function maps a validation issue
`code` string to a `PipelineNodeId`.

```typescript
import type { LaunchValidationIssue } from '../types/launch';

type PipelineNodeId = 'game' | 'wine-prefix' | 'proton' | 'steam' | 'trainer' | 'optimizations' | 'launch';

/**
 * Maps a validation issue to the pipeline node it belongs to.
 * Returns 'launch' (summary node) for unmapped or code-less issues.
 */
export function mapValidationToNode(issue: LaunchValidationIssue): PipelineNodeId { ... }
```

The mapping uses the `code` field (populated by T1) with a prefix-match strategy:
- Codes starting with `game_path` or `native_windows_executable` -> `'game'`
- Codes starting with `runtime_prefix` or `low_disk_space` -> `'wine-prefix'`
- Codes starting with `runtime_proton` -> `'proton'`
- Codes starting with `steam_` -> `'steam'`
- Codes starting with `trainer_` or `native_trainer` or `unshare_net` -> `'trainer'`
- Codes starting with `unknown_launch_optimization`, `duplicate_launch_optimization`,
  `incompatible_launch_optimization`, `launch_optimization`, `gamescope_`, `custom_env_var` ->
  `'optimizations'`
- Everything else (including `unsupported_method`, `offline_readiness_insufficient`, unknown codes,
  or missing `code`) -> `'launch'`

Also export a helper:

```typescript
/**
 * Groups validation issues by pipeline node, returning a Map<PipelineNodeId, LaunchValidationIssue[]>
 * sorted by severity (fatal first) within each group.
 */
export function groupIssuesByNode(issues: LaunchValidationIssue[]): Map<PipelineNodeId, LaunchValidationIssue[]> { ... }
```

Acceptance: file compiles with no TS errors; both functions exported; all codes from the mapping
table above are handled; unmapped codes fall through to `'launch'`.

---

**T3 — Update mock `preview_launch` handler with validation fixtures** _Depends on []._ Update
`src/crosshook-native/src/lib/mocks/handlers/launch.ts:211-258`. The current mock always returns
`validation: { issues: [] }`. Add a conditional branch so that when the mock request contains a
game path that is empty or a specific sentinel value (e.g., `__MOCK_VALIDATION_ERROR__`), the mock
returns validation issues that exercise the error state:

```typescript
// When game_path is empty, return issues that would trigger error states on multiple nodes
validation: {
  issues: [
    {
      message: 'A game executable path is required.',
      help: 'Set a game executable path in the profile.',
      severity: 'fatal' as const,
      code: 'game_path_required',
    },
    {
      message: 'The runtime prefix path does not exist.',
      help: 'Check that the Wine prefix directory exists.',
      severity: 'fatal' as const,
      code: 'runtime_prefix_path_missing',
    },
  ],
},
```

When the request has a populated game path (normal case), continue returning
`validation: { issues: [] }` (existing behavior).

Also add `directives_error: 'Mock directive resolution error'` to the error-path fixture to exercise
the Optimizations node error state, setting `environment: null`, `wrappers: null`, and
`effective_command: null` for consistency with the real cascading behavior.

Acceptance: mock compiles; `?fixture=populated` with a populated profile returns clean preview;
empty game path returns issues; no `verify:no-mocks` CI sentinel violations (mock code stays in
`lib/mocks/`).

---

### Batch B — Core logic (depends on Batch A)

**T4 — Enhance `derivePipelineNodes()` with Tier 2 status** _Depends on [T1, T2]._ Update
`src/crosshook-native/src/utils/derivePipelineNodes.ts`. The function currently ignores `_preview`
and `_phase`. Phase 2 activates the `preview` parameter (remove underscore prefix; `_phase` stays
underscored for Phase 3).

When `preview` is non-null, the function calls a new internal `tier2Status()` function that:

1. **Maps validation issues to nodes**: Call `groupIssuesByNode(preview.validation.issues)` (from
   T2). If a node has any fatal issue, its status is `'error'` and `detail` is the first fatal
   issue's `message`. If a node has only warnings, status remains `'configured'` (warnings don't
   override configured status in the pipeline — they're visible in the Preview modal).

2. **Checks `directives_error`**: If `preview.directives_error` is non-null, the `optimizations`
   node gets `'error'` status with `detail` set to `directives_error` (unless a fatal validation
   issue already set it to error — validation issues take precedence since they're more specific).

3. **Enriches `detail` on configured nodes**: When a node is `'configured'` (no errors), populate
   `detail` from preview data:
   - `game`: `preview.game_executable_name` (the resolved filename)
   - `wine-prefix`: `preview.proton_setup?.wine_prefix_path` (truncated to last path segment)
   - `proton`: `preview.proton_setup?.proton_executable` (truncated to last path segment)
   - `steam`: `preview.steam_launch_options ? 'Launch options set' : 'Ready'`
   - `trainer`: `preview.trainer?.path` (truncated to last path segment) or `'Not configured'`
   - `optimizations`: `preview.environment?.length` env vars or `'No optimizations'`
   - `launch`: `preview.effective_command ? 'Command ready' : 'Not ready'`

4. **Launch (summary) node logic**: If any node in the pipeline has `'error'` status, the launch
   node is `'error'` with `detail` = `'Resolve errors above'`. If all are configured, launch is
   `'configured'` with `detail` from effective command.

5. **Fallback to Tier 1**: When `preview` is null, the existing `tier1Status()` logic runs
   unchanged.

Implementation structure:
```typescript
export function derivePipelineNodes(
  method: ResolvedLaunchMethod,
  profile: GameProfile,
  preview: LaunchPreview | null,  // remove underscore
  _phase: LaunchPhase             // keep underscore for Phase 3
): PipelineNode[] {
  const ids = METHOD_NODE_IDS[method];
  const issuesByNode = preview ? groupIssuesByNode(preview.validation.issues) : null;
  const nodes: PipelineNode[] = [];

  for (let i = 0; i < ids.length; i += 1) {
    const id = ids[i];
    const label = NODE_DEFS[id]?.label ?? id;

    if (preview && id !== 'launch') {
      // Tier 2: preview-derived status
      nodes.push(tier2NodeStatus(id, profile, preview, issuesByNode, method));
    } else if (id === 'launch') {
      // Launch node: aggregate check
      nodes.push(launchNodeStatus(nodes, preview));
    } else {
      // Tier 1: config-only fallback
      nodes.push({ id, label, status: tier1Status(id, profile, method) });
    }
  }

  return nodes;
}
```

Acceptance: function compiles; Tier 1 behavior is unchanged when `preview` is null; Tier 2
activates when `preview` is non-null; nodes with fatal issues show `'error'`; `detail` is populated
on both error and configured nodes; `directives_error` maps to optimizations node.

---

**T5 — Wire `preview` to `<LaunchPipeline>` in `LaunchPanel`** _Depends on [T4]._ Update
`src/crosshook-native/src/components/LaunchPanel.tsx:904`. Change:

```diff
- <LaunchPipeline method={method} profile={profile} preview={null} phase={phase} />
+ <LaunchPipeline method={method} profile={profile} preview={preview} phase={phase} />
```

The `preview` variable is already destructured from `usePreviewState()` at line 635. This single
line change wires Tier 2 status to the pipeline component.

No other changes to `LaunchPanel.tsx` are needed. The `<LaunchPipeline>` component's `useMemo` will
recompute nodes when `preview` changes from null to a `LaunchPreview` object (after the user clicks
Preview).

Acceptance: after clicking "Preview" in the Launch page, pipeline nodes reflect preview-derived
status; before clicking Preview, pipeline still shows Tier 1 (config-derived) status; `preview`
prop is the live preview state, not hardcoded null.

---

### Batch C — Verification (depends on Batch B)

**T6 — Show `detail` text in pipeline UI** _Depends on [T4]._ Update
`src/crosshook-native/src/components/LaunchPipeline.tsx`. Currently the component renders
`STATUS_LABEL[node.status]` in the status span. When `node.detail` is available, render it instead
of (or below) the generic status label.

Update the `<li>` content in the `.map()`:

```tsx
<span className="crosshook-launch-pipeline__node-status">
  {node.detail || STATUS_LABEL[node.status]}
</span>
```

When `node.detail` is present and the status is `'error'`, use the detail text (the validation
error message). When the status is `'configured'` and detail is present, show the resolved path
segment. When no detail, fall back to the generic label (`'Ready'`, `'Not configured'`, etc.).

Add `title={node.detail}` to the `<li>` element for tooltip on hover (progressive disclosure per
PRD Open Question #2 — lightweight implementation):

```tsx
<li
  key={node.id}
  className="crosshook-launch-pipeline__node"
  data-status={node.status}
  aria-current={index === currentStepIndex ? 'step' : undefined}
  aria-label={`${node.label}: ${node.detail || STATUS_LABEL[node.status]}`}
  title={node.detail}
>
```

Acceptance: pipeline nodes show detail text when available; tooltip appears on hover; aria-label
includes detail text for screen readers; no visual regressions on Tier 1 (detail-less) nodes.

---

**T7 — Update `LaunchPipeline` `aria-current` logic for error states** _Depends on [T4]._ Update
`src/crosshook-native/src/components/LaunchPipeline.tsx`. The current `aria-current="step"` logic
finds the first `'not-configured'` node. With Tier 2, `'error'` nodes should also be considered
as "current" (the step the user should focus on):

```typescript
const firstIssueIdx = nodes.findIndex(
  (n) => n.id !== 'launch' && (n.status === 'not-configured' || n.status === 'error')
);
```

This ensures screen readers announce the first problematic node as the current step.

Acceptance: `aria-current="step"` is set on the first error or not-configured node; if all nodes
are configured, it falls through to the launch node as before.

---

**T8 — Rust: Add unit test for `code()` method** _Depends on [T1]._ Add a test in
`crates/crosshook-core/src/launch/request.rs` (inside the existing `#[cfg(test)] mod tests` block)
that asserts every `ValidationError` variant produces a non-empty `code()` string and that the
existing `issue()` conversion includes the code.

```rust
#[test]
fn validation_error_codes_are_populated() {
    // Spot-check representative variants from each collect_* group
    assert_eq!(ValidationError::GamePathRequired.code(), "game_path_required");
    assert_eq!(ValidationError::SteamAppIdRequired.code(), "steam_app_id_required");
    assert_eq!(ValidationError::RuntimePrefixPathRequired.code(), "runtime_prefix_path_required");
    assert_eq!(ValidationError::RuntimeProtonPathRequired.code(), "runtime_proton_path_required");
    assert_eq!(
        ValidationError::UnknownLaunchOptimization("foo".into()).code(),
        "unknown_launch_optimization"
    );

    // Verify issue() propagates the code
    let issue = ValidationError::GamePathRequired.issue();
    assert_eq!(issue.code.as_deref(), Some("game_path_required"));
}
```

Verification:
```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- validation_error_codes
```

Acceptance: test passes; spot-checks cover at least one variant per pipeline node mapping group.

---

**T9 — Verify frontend build and existing tests** _Depends on [T2, T4, T5, T6, T7]._ Run full
verification suite:

```bash
# Rust tests (including T8's new test)
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core

# Frontend build (type-checks all TS changes)
cd src/crosshook-native && npm run build

# Playwright smoke (no console errors on launch route)
cd src/crosshook-native && npm run test:smoke
```

Acceptance: all three commands pass with zero failures; no new TS errors; no console errors in
Playwright smoke.

---

**T10 — Update PRD phase table** _Depends on [T9]._ Update
`docs/prps/prds/launch-pipeline-visualization.prd.md` Phase 2 row from pending to complete. Update
any Phase 2 descriptions to reflect the implemented approach (code-based mapping instead of
message-pattern matching).

Acceptance: PRD reflects Phase 2 as complete; Phase 3 is the next pending phase.

---

## Batches

| Batch | Tasks           | Can run in parallel                                                    |
| ----- | --------------- | ---------------------------------------------------------------------- |
| A     | T1, T2, T3      | All 3 independent — Rust, TS utility, and mock handler                 |
| B     | T4, T5          | T4 depends on T1+T2; T5 depends on T4                                 |
| C     | T6, T7, T8, T9, T10 | T6+T7+T8 depend on T4; T9 depends on all; T10 depends on T9       |

Batch A tasks can be dispatched to 3 parallel agents. Batch B requires sequential execution (T4
then T5). Batch C tasks T6, T7, T8 are parallel; T9 is a serial gate; T10 is final.
