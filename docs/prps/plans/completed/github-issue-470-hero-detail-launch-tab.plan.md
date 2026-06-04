# Plan: Hero Detail Launch Tab - Highlighted Command Stack (Issue #470, Phase 5)

## Summary

Build the Hero Detail Launch tab as a single-column, three-section stack: Launch command, Environment, and Pre/post hooks. The Launch command section adds a hand-rolled highlighted command block and four actions; the Environment section reuses the existing custom environment editor and 400ms autosave hook; the Pre/post hooks section remains a disabled placeholder for Phase 6.

This plan intentionally keeps the existing `launch-options` tab id and `hero-detail-launch-tab` test id stable. It does not add storage, backend IPC, a syntax-highlighting dependency, hook editing, or direct launcher execution from the new components.

## User Story

As a Linux gamer using CrossHook to configure and launch a per-game profile, I want the Hero Detail Launch tab to combine command preview, environment editing, and hook status in one stack, so that I can inspect and tune launch behavior without leaving Hero Detail.

## Problem -> Solution

Hero Detail currently renders `launch-options` as a read-only structured preview split across Summary, Validation, Command chain, Proton setup, Trainer, Environment, and Raw preview cards. -> Replace that branch with a focused Launch tab: colored command preview plus actions, editable environment variables with autosave, and a disabled pre/post hooks placeholder that preserves the Phase 6 boundary.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 5 - Hero Detail Launch tab (3-section stack) + highlighted command preview
- **GitHub Issue**: #470
- **Estimated Files**: 10
- **Storage Boundary**: No new SQLite metadata, no new settings, no new runtime state persisted. Environment edits reuse per-profile TOML `launch.custom_env_vars`; hook fields already exist as per-profile TOML from Phase 3 and remain read-only/disabled here.
- **Worktree Mode**: Disabled by request (`--no-worktree`); no worktree setup section is included.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch can run concurrently; batches run in order.

| Batch | Tasks              | Depends On | Parallel Width |
| ----- | ------------------ | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3, 1.4 | -          | 4              |
| B2    | 2.1                | B1         | 1              |
| B3    | 3.1, 3.2, 3.3      | B2         | 3              |
| B4    | 4.1, 4.2           | B3         | 2              |
| B5    | 5.1                | B4         | 1              |

- **Total tasks**: 11
- **Total batches**: 5
- **Max parallel width**: 4
- **Same-file collision check**: B1 touches a new export helper/`LauncherExport.tsx`, a new highlighted block, `hero-detail.css`, and `CustomEnvironmentVariablesSection.tsx`. B3 splits panel wiring, a new highlighted-block test, and a new launch-tab test. B4 splits existing panel tests and `GameDetail` tests. No batch assigns the same file to two tasks.

---

## Storage Boundary & Persistence

| Datum                                             | Classification                            | Behavior                                                                                      |
| ------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------- |
| `launch.custom_env_vars` edits                    | TOML settings, per-profile user-editable  | Existing profile save path persists changes through `persistProfileDraft` and `profile_save`. |
| Copy status, export status, preview loading state | Runtime-only UI state                     | Local component state only; not persisted.                                                    |
| `pre_launch_hooks` / `post_exit_hooks` display    | TOML settings already declared by Phase 3 | Phase 5 reads counts/emptiness only and renders disabled UI; no edits are saved.              |
| SQLite metadata                                   | None introduced                           | `metadata/migrations.rs` remains untouched.                                                   |
| App settings                                      | None introduced                           | `settings.toml` remains untouched.                                                            |

- **Migration / backward compatibility**: No migration. Existing profiles with no custom env vars or hook arrays still render empty states.
- **Offline behavior**: Fully local. Preview/export IPC behavior is the same as existing Launch and launcher export surfaces.
- **Degraded fallback**: Missing `launchRequest` disables actions and shows the unavailable message. Preview errors render inline. Clipboard and export failures stay non-fatal and visible.
- **User visibility / editability**: Users can edit custom environment variables. Hooks are visible only as a disabled placeholder until Phase 6.

---

## Testing Strategy

### Unit and Component Tests

| Test                                       | Input                                                          | Expected Output                                                               | Edge Case? |
| ------------------------------------------ | -------------------------------------------------------------- | ----------------------------------------------------------------------------- | ---------- |
| `HighlightedCommandBlock` token classes    | Preview with env, wrappers, command, proton setup              | Comment, env-key, value, binary, and flag spans render with expected classes  | No         |
| `HighlightedCommandBlock` malicious values | Env/path values containing `<script>`, quotes, `$()`, newlines | Values render literally as text and no `script` element appears               | Yes        |
| `HeroDetailLaunchTab` section stack        | Valid profile, request, preview                                | Launch command, Environment, Pre/post hooks in order                          | No         |
| `HeroDetailLaunchTab` no request           | `launchRequest: null`                                          | Actions disabled and unavailable state shown                                  | Yes        |
| `HeroDetailLaunchTab` copy action          | Preview with `effective_command`                               | `copyToClipboard` called and success/failure label updates                    | Yes        |
| `HeroDetailLaunchTab` env autosave         | Edit env value then blur                                       | `persistProfileDraft(profileName, updatedProfile)` after 400ms                | No         |
| `HeroDetailLaunchTab` invalid env row      | Invalid key/reserved key/duplicate/NUL then blur               | `persistProfileDraft` is not called for invalid env                           | Yes        |
| `HeroDetailPanels` branch                  | `mode="launch-options"`                                        | New tab renders; old Summary/Raw preview assumptions removed                  | No         |
| `GameDetail` panel props                   | Mounted detail page                                            | `onLaunch`, `launchingName`, display profile name, preview callback forwarded | No         |

### Edge Cases Checklist

- [ ] `preview` is `null`
- [ ] `preview.effective_command` is `null`
- [ ] `preview.environment` is `null`
- [ ] `preview.wrappers` is `null` or empty
- [ ] Clipboard rejects
- [ ] Launcher export validation rejects
- [ ] Profile selection is not currently aligned to this game
- [ ] Custom env has invalid, duplicate, reserved, or NUL-containing keys
- [ ] Narrow viewport command block scrolls horizontally without wrapping

---

## Validation Commands

### Focused Tests

```bash
cd src/crosshook-native && npm exec vitest run \
  src/components/library/__tests__/HighlightedCommandBlock.test.tsx \
  src/components/library/__tests__/HeroDetailLaunchTab.test.tsx \
  src/components/library/__tests__/HeroDetailPanels.test.tsx \
  src/components/library/__tests__/GameDetail.test.tsx
```

EXPECT: New and updated Hero Detail Launch tests pass.

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
```

EXPECT: Zero TypeScript errors.

### Full Frontend Suite

```bash
cd src/crosshook-native && npm test
```

EXPECT: Vitest suite passes.

### Lint

```bash
./scripts/lint.sh --modified
```

EXPECT: Modified files pass repo lint checks.

### Dependency Guard

```bash
git diff -- package.json src/crosshook-native/package.json package-lock.json src/crosshook-native/package-lock.json
```

EXPECT: No new syntax-highlighting, command parsing, clipboard, or launcher export dependency.

### Manual Validation

- [ ] Open Hero Detail and select the Launch options tab.
- [ ] Confirm Launch command, Environment, and Pre/post hooks render as a single-column stack.
- [ ] Confirm command block scrolls horizontally on narrow width and does not wrap.
- [ ] Click Dry-run and confirm preview loading/error states behave as before.
- [ ] Click Copy and confirm copied/failed feedback.
- [ ] Click `.desktop` and confirm existing export success/error feedback appears.
- [ ] Edit a custom environment variable, blur, wait 400ms, and confirm the profile is saved.
- [ ] Confirm hooks show `No pre/post hooks configured yet` and disabled `Add hook`.

---

## Acceptance Criteria

- [ ] `launch-options` renders `HeroDetailLaunchTab` instead of the old read-only `LaunchPreviewStructuredView` branch.
- [ ] The tab renders exactly three top-level sections in order: Launch command, Environment, Pre/post hooks.
- [ ] Launch tab remains single-column at all breakpoints.
- [ ] The command block renders React text spans for comment, env-key, value, binary, and flag token classes.
- [ ] Token classes have distinct theme colors using warning, success, accent-strong, text-muted, and faint/subtle text tones.
- [ ] The command block scrolls horizontally on narrow viewports and does not wrap command text.
- [ ] Dry-run uses the existing preview callback/`preview_launch` path and renders loading/error feedback.
- [ ] Copy uses `copyToClipboard(preview.effective_command)` on explicit click and handles failure without throwing.
- [ ] `.desktop` uses the existing launcher export request/validation/export flow.
- [ ] Launch calls the existing `onLaunch` boundary only.
- [ ] Environment section reuses `CustomEnvironmentVariablesSection`.
- [ ] Environment header shows `{N} ON` for non-empty custom environment variables.
- [ ] Environment blur autosave calls `persistProfileDraft` after the existing 400ms debounce.
- [ ] Invalid env keys/values do not trigger autosave.
- [ ] Pre/post hooks section shows `No pre/post hooks configured yet` and a disabled `Add hook` button.
- [ ] Tests cover env autosave and highlighted command token classes.
- [ ] No new npm dependency is added.
- [ ] No SQLite, settings, backend schema, or launch runtime code is changed.

## Completion Checklist

- [ ] Code follows `HeroDetail*Tab` component naming and default export convention.
- [ ] Existing tab id and `hero-detail-launch-tab` test id remain stable.
- [ ] No `dangerouslySetInnerHTML` or `innerHTML` appears in highlighted command rendering.
- [ ] No direct launch IPC appears in new components.
- [ ] `.desktop` action uses backend validation/export, not React-generated desktop content.
- [ ] Custom env validation remains visible and prevents invalid autosave.
- [ ] Command copy uses exact `preview.effective_command`.
- [ ] Actions are disabled when required request/preview/profile state is missing.
- [ ] Focused tests, typecheck, full frontend tests, and modified lint pass.
- [ ] Package manifests and lockfiles have no new highlighter/clipboard dependency.

## Risks

| Risk                                                       | Likelihood | Impact | Mitigation                                                                                                            |
| ---------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------- |
| Highlighting turns user-controlled strings into HTML       | Medium     | High   | Render React text children only; add malicious-token tests.                                                           |
| Command preview exposes host env or secret-like env values | Medium     | Medium | Omit host-source env from prominent command block and avoid logging copied command/env text.                          |
| Copy places sensitive command/env values on clipboard      | Medium     | Medium | Require explicit click, use `copyToClipboard`, show feedback, never auto-copy.                                        |
| Env validation is bypassed by new autosave wiring          | Medium     | Medium | Keep validation in `CustomEnvironmentVariablesSection` and guard invalid blur autosave.                               |
| `.desktop` action duplicates request/content logic         | Low        | High   | Factor/reuse existing request builder and `useLauncherExport`; backend remains source of validation and file content. |
| Actions operate on stale preview/request after env edits   | Medium     | Medium | Disable during preview/profile save where available and always bind action callbacks to current `launchRequest`.      |
| Launch action bypasses launch validation/session gates     | Low        | High   | Call only the existing `onLaunch` prop.                                                                               |
| New scroll behavior regresses in WebKitGTK                 | Low        | Medium | Do not create a new vertical scroll container; if needed, add selector to `useScrollEnhance.ts`.                      |

## Notes

- Research dispatch: enhanced mode target was used. Six standalone researcher roles completed; the recommendations slice was synthesized locally because the runtime agent thread limit blocked a seventh spawn.
- Enhanced preflight note: the installed YCC cache path derived by `preflight-enhanced-agents.sh` did not contain `ycc/agents`, but the source YCC bundle at `/home/yandy/Projects/github.com/yandy-r/claude-plugins/ycc` contains `agents/prp-researcher.md` and passes the same preflight check. This affects planning tooling only, not CrossHook implementation.
- External research needed only for official Clipboard and Tauri command behavior; no product/API integration is introduced.
- Confidence score: 8/10. Main uncertainty is the exact display token reconstruction from current `LaunchPreview` fields; the plan constrains tokenization to presentation and keeps backend `effective_command` as the source of truth for copy.

## Step-by-Step Tasks

### Task 1.1: Factor launcher export request helpers - Depends on [none]

- **BATCH**: B1
- **ACTION**: Create shared launcher export helpers and update `LauncherExport` to use them.
- **IMPLEMENT**: Move `automaticLauncherSuffix`, `safeTrim`, `stripAutomaticLauncherSuffix`, `deriveLauncherName`, and `buildExportRequest` from `LauncherExport.tsx` into `src/crosshook-native/src/utils/launcherExport.ts`. Export the request builder as `buildLauncherExportRequest` and keep existing `LauncherExport` labels/status behavior unchanged.
- **MIRROR**: `LAUNCHER_EXPORT_FLOW`; preserve the existing `validate_launcher_export` then `export_launchers` sequence in `useLauncherExport`.
- **IMPORTS**: `GameProfile`, `LaunchMethod`, `UmuPreference`, `SteamExternalLauncherExportRequest`.
- **GOTCHA**: Do not make React generate `.desktop` content. The helper only builds the existing request object that backend validation/export already owns.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/__tests__/SettingsPanel.test.tsx src/components/library/__tests__/HeroDetailPanels.test.tsx`

### Task 1.2: Create `HighlightedCommandBlock` - Depends on [none]

- **BATCH**: B1
- **ACTION**: Add the pure highlighted command preview component.
- **IMPLEMENT**: Create `src/crosshook-native/src/components/library/HighlightedCommandBlock.tsx` with props `preview: LaunchPreview`, `profileName?: string`, and optional `className`. Render a `<pre>` composed of React text spans for comment, env key, value, binary, and flag tokens; use `preview.environment`, `preview.wrappers`, `preview.proton_setup`, `preview.game_executable`, and `preview.effective_command` as presentation inputs.
- **MIRROR**: `SAFE_TOKEN_RENDERING`; values are text children only, with no `dangerouslySetInnerHTML`, no `innerHTML`, and no dependency-backed highlighter.
- **IMPORTS**: `LaunchPreview`, `PreviewEnvVar`.
- **GOTCHA**: Tokenization is display-only. Preserve `preview.effective_command` as the copy source and do not try to reconstruct an executable command for launch from token spans.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 1.3: Add Launch tab command styles - Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend `hero-detail.css` with Launch tab and token styles.
- **IMPLEMENT**: Add `crosshook-hero-detail__launch-tab`, section/action row, hook placeholder, and highlighted command block classes. Set the highlighted command block to `white-space: pre`, `overflow-x: auto`, and `overflow-y: hidden`; add token classes `--comment`, `--env-key`, `--value`, `--binary`, and `--flag` using warning/success/accent/text-muted/text-subtle variables or local Hero Detail aliases.
- **MIRROR**: `NAMING_CONVENTION`; use `crosshook-hero-detail__*` BEM classes and existing card primitives.
- **IMPORTS**: None.
- **GOTCHA**: Do not add a new vertical scroll container. If implementation later introduces `overflow-y: auto`, register that selector in `useScrollEnhance.ts`.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 1.4: Guard invalid env blur autosave - Depends on [none]

- **BATCH**: B1
- **ACTION**: Preserve env validation when Hero Detail reuses `CustomEnvironmentVariablesSection`.
- **IMPLEMENT**: Update blur/remove autosave calls so invalid rows do not trigger `onAutoSaveBlur`; reuse `customEnvRowError(row, duplicateIds)` before calling the callback. Keep draft UI updates local so validation messages still show while invalid input is being edited.
- **MIRROR**: `REPOSITORY_PATTERN`; the section remains the single owner of custom env validation and immutable updater logic.
- **IMPORTS**: None.
- **GOTCHA**: Avoid changing reserved key rules or backend validation; this is a frontend autosave guard, not a schema change.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 2.1: Create `HeroDetailLaunchTab` - Depends on [1.1, 1.2, 1.3, 1.4]

- **BATCH**: B2
- **ACTION**: Add the new Launch tab shell and action row.
- **IMPLEMENT**: Create `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx` with props for `summary`, `launchRequest`, `previewLoading`, `preview`, `previewError`, `onPreviewLaunch`, `onLaunch`, and `launchingName`. Mirror `HeroDetailProfilesTab` by reading `ProfileContext` for `profile`, `profileName`, `selectedProfile`, `profiles`, `updateProfile`, `persistProfileDraft`, `steamClientInstallPath`, and `targetHomePath`; use `useLaunchEnvironmentAutosave` for custom env blur saves and render `CustomEnvironmentVariablesSection` with an `{N} ON` pill.
- **MIRROR**: `AUTOSAVE_400MS`, `LAUNCHER_EXPORT_FLOW`, and `CLIPBOARD_FALLBACK`.
- **IMPORTS**: `useMemo`, `useState`, `useProfileContext`, `usePreferencesContext`, `useLaunchEnvironmentAutosave`, `useLauncherExport`, `copyToClipboard`, `resolveLaunchMethod`, `DashboardPanelSection`, `CustomEnvironmentVariablesSection`, `HighlightedCommandBlock`, `buildLauncherExportRequest`.
- **GOTCHA**: Hooks must not be called conditionally. If the `.desktop` export request is unavailable, render a disabled outer button or a small child component that only mounts the `useLauncherExport` hook when the request exists.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 3.1: Replace `launch-options` branch and pass panel callbacks - Depends on [2.1]

- **BATCH**: B3
- **ACTION**: Wire the new tab into Hero Detail.
- **IMPLEMENT**: Extend `HeroDetailPanelsProps` with `onPreviewLaunch?: (request: LaunchRequest) => void | Promise<void>`, `onLaunch?: (name: string) => void | Promise<void>`, `launchingName?: string`, and `displayProfileName?: string`. Pass these from `GameDetail` using existing `requestPreview`, `onLaunch`, `launchingName`, and `displayProfileName`; replace only the `case 'launch-options'` branch with `HeroDetailLaunchTab` and remove obsolete local preview helper functions if unused.
- **MIRROR**: `STABLE_TAB_TEST_ID`; keep `HeroDetailTabId` and `HERO_DETAIL_TAB_TESTIDS` unchanged.
- **IMPORTS**: `HeroDetailLaunchTab`.
- **GOTCHA**: The Launch action must call `onLaunch` only. Do not import or call launch IPC commands in `HeroDetailLaunchTab`.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroDetailPanels.test.tsx src/components/library/__tests__/GameDetail.test.tsx`

### Task 3.2: Add `HighlightedCommandBlock` tests - Depends on [2.1]

- **BATCH**: B3
- **ACTION**: Test highlighted token output and unsafe input rendering.
- **IMPLEMENT**: Add `src/crosshook-native/src/components/library/__tests__/HighlightedCommandBlock.test.tsx` with a structured `LaunchPreview` fixture. Assert token classes for `--comment`, `--env-key`, `--value`, `--binary`, and `--flag`; include env/path values containing `<script>`, quotes, `$()`, and newlines and assert they render as text with no `script` element.
- **MIRROR**: `TEST_STRUCTURE`; use Testing Library queries and `toHaveClass` assertions.
- **IMPORTS**: `render`, `screen`, `within`, `describe`, `expect`, `it`, `HighlightedCommandBlock`, `LaunchPreview`.
- **GOTCHA**: Do not snapshot the entire visual tree if simple class/text assertions prove the behavior; keep the test stable.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HighlightedCommandBlock.test.tsx`

### Task 3.3: Add `HeroDetailLaunchTab` focused tests - Depends on [2.1]

- **BATCH**: B3
- **ACTION**: Test the new tab in isolation with ProfileContext and IPC mocks.
- **IMPLEMENT**: Add `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx` using the ProfileContext spy pattern and `renderWithMocks` when export/preview commands are exercised. Cover section order, disabled states without request/preview, Copy success/failure through `copyToClipboard`, `.desktop` export request reuse, Launch calling `onLaunch`, and env edit/blur autosave calling `persistProfileDraft` after the 400ms window.
- **MIRROR**: `CONTEXT_SPY_TESTS`, `AUTOSAVE_400MS`, and `CLIPBOARD_FALLBACK`.
- **IMPORTS**: `renderWithMocks`, `userEvent`, `waitFor`, `vi`, `PreferencesProvider` if needed by export request building.
- **GOTCHA**: The env section's current UI edits key/value rows; there is no row enabled toggle. Test editing/removing a custom env var, not a non-existent toggle control.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroDetailLaunchTab.test.tsx`

### Task 4.1: Update `HeroDetailPanels` tests - Depends on [3.1, 3.2, 3.3]

- **BATCH**: B4
- **ACTION**: Replace old read-only launch preview expectations with new Launch tab expectations.
- **IMPLEMENT**: Update `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` so `mode="launch-options"` asserts the three-section stack, loading/error/unavailable states, and the stable `hero-detail-launch-tab` host behavior. Keep the existing `makeLaunchRequest` and `makePreview` fixtures where useful.
- **MIRROR**: `TEST_STRUCTURE`.
- **IMPORTS**: Existing test imports plus any new helpers needed for providers/mocks.
- **GOTCHA**: Removing `LaunchPreviewStructuredView` will invalidate old headings like Summary and Raw preview; update tests to the new section headings instead of weakening coverage.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroDetailPanels.test.tsx`

### Task 4.2: Update `GameDetail` tests - Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Lock the new panel contract from the container side.
- **IMPLEMENT**: Update `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx` to assert `GameDetail` forwards `onLaunch`, `launchingName`, `displayProfileName`, and the preview-refresh callback into `HeroDetailPanels`. If the test mocks `HeroDetailPanels`, extend the mock props assertion rather than mounting the full Launch tab.
- **MIRROR**: Existing `GameDetail` test mocking style.
- **IMPORTS**: Existing test utilities.
- **GOTCHA**: Keep this test focused on container wiring; detailed button behavior belongs in `HeroDetailLaunchTab.test.tsx`.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/GameDetail.test.tsx`

### Task 5.1: Validate and dependency guard - Depends on [4.1, 4.2]

- **BATCH**: B5
- **ACTION**: Run focused and broad frontend validation, then confirm no forbidden dependency was added.
- **IMPLEMENT**: Run focused tests first, then typecheck, then the full Vitest suite if focused tests pass. Inspect `git diff -- package.json src/crosshook-native/package.json package-lock.json src/crosshook-native/package-lock.json` to confirm no syntax-highlighting or clipboard dependency was added.
- **MIRROR**: Repo command reference in `AGENTS.md`; frontend tests are Vitest/happy-dom.
- **IMPORTS**: None.
- **GOTCHA**: This is a frontend-only plan. Do not run Rust generation or database migrations unless implementation unexpectedly touches backend/schema files.
- **VALIDATE**: `cd src/crosshook-native && npm test && npm run typecheck && cd ../.. && ./scripts/lint.sh --modified`

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx:43-49
export interface HeroDetailProfilesTabProps {
  /* props */
}
export function HeroDetailProfilesTab({ summary, profileList }: HeroDetailProfilesTabProps) {
  return <div className="crosshook-hero-detail__profiles" />;
}
export default HeroDetailProfilesTab;
```

### STABLE_TAB_TEST_ID

```ts
// SOURCE: src/crosshook-native/src/components/library/hero-detail-model.ts:20-24
export const HERO_DETAIL_TAB_TESTIDS = {
  profiles: 'hero-detail-profiles-tab',
  'launch-options': 'hero-detail-launch-tab',
};
```

### SERVICE_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/usePreviewState.ts:13-24
const seq = ++previewRequestSeq.current;
const result = await callCommand<LaunchPreview>('preview_launch', { request });
if (seq !== previewRequestSeq.current) return;
setPreview(result);
```

### REPOSITORY_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx:144-153
setRows(next);
onUpdateProfile((current) => ({
  ...current,
  launch: { ...current.launch, custom_env_vars: customEnvRowsToRecord(next) },
}));
```

### AUTOSAVE_400MS

```ts
// SOURCE: src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts:72-87
environmentAutosaveTimerRef.current = setTimeout(() => {
  if (latestProfileNameRef.current !== scheduledProfileName) return;
  void persistProfileDraftRef.current(scheduledProfileName, nextProfile);
}, 400);
```

### LAUNCHER_EXPORT_FLOW

```ts
// SOURCE: src/crosshook-native/src/hooks/useLauncherExport.ts:122-130
await callCommand<void>('validate_launcher_export', { request });
const exported = await callCommand<SteamExternalLauncherExportResult>('export_launchers', { request });
setResult(exported);
setStatusMessage('Launcher export completed.');
```

### CLIPBOARD_FALLBACK

```ts
// SOURCE: src/crosshook-native/src/utils/clipboard.ts:1-7
export async function copyToClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
```

### ERROR_HANDLING

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailPanels.tsx:407-416
{
  launchRequest && previewLoading ? <p className="crosshook-hero-detail__muted">Building launch preview...</p> : null;
}
{
  previewError ? <p className="crosshook-hero-detail__warn">{previewError}</p> : null;
}
{
  preview && launchRequest ? <LaunchPreviewStructuredView preview={preview} /> : null;
}
```

### SAFE_TOKEN_RENDERING

```tsx
// SOURCE: src/crosshook-native/src/types/launch.ts:168-186
environment: PreviewEnvVar[] | null;
wrappers: string[] | null;
effective_command: string | null;
game_executable: string;
```

Render token values as React text children only. Do not use `dangerouslySetInnerHTML`, `innerHTML`, or React-side shell parsing as a source of truth. Copy and launch behavior must keep using backend `preview.effective_command` and the existing launch boundary.

### TEST_STRUCTURE

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx:146-183
function renderHeroDetailPanels(overrides: Partial<HeroDetailPanelsProps> = {}) {
  const props = { mode: 'launch-options', launchRequest: makeLaunchRequest(), preview: makePreview(), ...overrides };
  return render(<HeroDetailPanels {...props} />);
}
```

### CONTEXT_SPY_TESTS

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx:10-29
const profileContextMock = vi.fn();
const persistProfileDraftSpy = vi.fn();
vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));
```

---

## Files to Change

| File                                                                                     | Action | Justification                                                                                             |
| ---------------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/utils/launcherExport.ts`                                       | CREATE | Factor launcher export request derivation so `LauncherExport` and `HeroDetailLaunchTab` share one mapping |
| `src/crosshook-native/src/components/library/HighlightedCommandBlock.tsx`                | CREATE | Pure highlighted command block using React spans and structured `LaunchPreview` fields                    |
| `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx`                    | CREATE | New three-section Launch tab stack, env editor, action row, and hook placeholder                          |
| `src/crosshook-native/src/components/library/__tests__/HighlightedCommandBlock.test.tsx` | CREATE | Token class, malicious string, and no-dependency behavior tests                                           |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx`     | CREATE | Context-heavy env autosave, action disabled states, and copy/export behavior tests                        |
| `src/crosshook-native/src/components/LauncherExport.tsx`                                 | UPDATE | Import and reuse the factored request/name helpers without changing existing exporter UX                  |
| `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`              | UPDATE | Ensure invalid rows do not trigger blur autosave when reused in Hero Detail Launch                        |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                       | UPDATE | Extend panel contract and replace `launch-options` branch with `HeroDetailLaunchTab`                      |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                             | UPDATE | Pass `onLaunch`, `launchingName`, current profile name, and preview-refresh callback into panel props     |
| `src/crosshook-native/src/styles/hero-detail.css`                                        | UPDATE | Add launch-tab stack, action row, hook placeholder, horizontal command scrolling, and token color classes |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`        | UPDATE | Replace old structured preview assertions with Launch tab rendering and state coverage                    |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`              | UPDATE | Assert new panel channels are forwarded from `GameDetail`                                                 |

## NOT Building

- No hook schema or hook persistence changes; Phase 3 already shipped the arrays.
- No live `HookListPanel`, hook add/edit/toggle, or runtime hook execution; Phase 6 owns that.
- No syntax-highlighting, clipboard, or command parsing dependency.
- No raw Tauri `invoke()` calls from the new components.
- No direct `launch_game` or `launch_trainer` IPC from `HeroDetailLaunchTab` or `HighlightedCommandBlock`.
- No React-side `.desktop` content generation.
- No new route, tab id, or test id rename.
- No replacement for `CustomEnvironmentVariablesSection`.
- No new nested Launch subtabs inside Hero Detail.

---

## UX Design

### Before

```text
Hero Detail -> Launch options tab
  Summary card
  Validation card
  Command chain card with plain pre block
  Proton setup card
  Trainer card
  Environment details dump
  Raw preview details dump
```

### After

```text
Hero Detail -> Launch options tab
  Launch command
    highlighted pre block
    Dry-run | Copy | .desktop | Launch
  Environment
    "{N} ON" pill
    CustomEnvironmentVariablesSection
  Pre/post hooks
    "No pre/post hooks configured yet"
    disabled "Add hook" button
```

### Interaction Changes

| Touchpoint                           | Before                                           | After                                                                   | Notes                                                                                              |
| ------------------------------------ | ------------------------------------------------ | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Hero Detail `launch-options` content | Read-only launch preview cards                   | `HeroDetailLaunchTab` with three sections                               | Keep tab id and `hero-detail-launch-tab` stable.                                                   |
| Command preview                      | Plain `<pre>` using `pre-wrap`                   | Highlighted `<pre>` using React text spans and horizontal scrolling     | Copy still uses exact backend `preview.effective_command`.                                         |
| Dry-run                              | Automatic preview refresh from `GameDetail` only | Button asks `GameDetail` to refresh preview for current `launchRequest` | Use a callback to preserve `usePreviewState` sequencing.                                           |
| Copy                                 | Preview modal copies `preview.display_text`      | Command row copies `preview.effective_command`                          | Use `copyToClipboard()` and show copied/failed state.                                              |
| `.desktop`                           | Standalone launcher export flow                  | Launch command row exposes export action                                | Reuse existing export request and `useLauncherExport`; do not synthesize desktop content in React. |
| Launch                               | Header button calls existing `onLaunch`          | Command row calls the same `onLaunch` boundary                          | Do not call `launch_game` or `launch_trainer` directly.                                            |
| Environment                          | Read-only grouped resolved env dump              | Editable custom env section with `{N} ON` pill                          | Autosave on blur through `useLaunchEnvironmentAutosave`.                                           |
| Pre/post hooks                       | No section                                       | Disabled placeholder                                                    | No hook add/edit/toggle behavior until Phase 6.                                                    |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File                                                                                   | Lines                     | Why                                                                                                |
| -------- | -------------------------------------------------------------------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`                      | 162-182, 343-361          | Phase 5 scope, responsive contract, launch tab and highlighted block requirements                  |
| P0       | `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                     | 18-40, 90-314, 359-418    | Current panel props, obsolete read-only launch preview helpers, `launch-options` branch to replace |
| P0       | `src/crosshook-native/src/components/library/GameDetail.tsx`                           | 136-168, 173-193, 214-237 | `launchRequest`, `usePreviewState`, `panelProps`, and existing `onLaunch` boundary                 |
| P0       | `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx`                | 43-145, 178-286           | Mutation-capable Hero Detail tab pattern, profile selection alignment, autosave status UI          |
| P0       | `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`            | 109-153, 157-256          | Reusable env editor prop contract, immutable profile updater, validation UI                        |
| P0       | `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`               | 24-93                     | Existing 400ms env autosave and profile-name race guard                                            |
| P0       | `src/crosshook-native/src/hooks/useLauncherExport.ts`                                  | 53-151, 224-245           | Existing launcher validation/export/preview/error state hook                                       |
| P0       | `src/crosshook-native/src/components/LauncherExport.tsx`                               | 21-90, 93-151, 267-358    | Export request construction and existing export UI behavior to factor/reuse                        |
| P0       | `src/crosshook-native/src/types/launch.ts`                                             | 120-186                   | `LaunchPreview`, `PreviewEnvVar`, and structured fields for presentation tokens                    |
| P0       | `src/crosshook-native/src/types/profile.ts`                                            | 141-178, 222-293          | `launch.custom_env_vars`, hook arrays, and normalizer defaults                                     |
| P1       | `src/crosshook-native/src/utils/clipboard.ts`                                          | 1-24                      | Shared clipboard utility with fallback                                                             |
| P1       | `src/crosshook-native/src/lib/ipc.ts`                                                  | 7-16                      | Webdev-aware IPC wrapper; avoid raw `invoke()`                                                     |
| P1       | `src/crosshook-native/src/components/layout/DashboardPanelSection.tsx`                 | 53-89                     | Existing panel shell with header actions                                                           |
| P1       | `src/crosshook-native/src/components/library/hero-detail-model.ts`                     | 5-29                      | Stable tab ids and `hero-detail-launch-tab` test id                                                |
| P1       | `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`                       | 29-40                     | Existing tab content fill/scroll wrappers                                                          |
| P1       | `src/crosshook-native/src/styles/hero-detail.css`                                      | 202-234, 491-608          | Existing Hero Detail BEM classes, cards, command block, responsive patterns                        |
| P1       | `src/crosshook-native/src/styles/variables.css`                                        | 26-36                     | Existing text/accent/success/warning color variables                                               |
| P1       | `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`      | 1-257                     | Current launch preview fixtures and panel render helper                                            |
| P1       | `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` | 10-29, 173-185            | ProfileContext spy pattern and autosave debounce assertions                                        |
| P1       | `src/crosshook-native/src/test/render.tsx`                                             | 35-48                     | `renderWithMocks` for components that call `callCommand()`                                         |

## External Documentation

| Topic                  | Source                                                               | Key Takeaway                                                                                                             |
| ---------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Clipboard API          | https://developer.mozilla.org/en-US/docs/Web/API/Clipboard/writeText | Clipboard writes can reject; use the repo fallback utility and surface failure in UI.                                    |
| Tauri v2 command calls | https://v2.tauri.app/develop/calling-rust/                           | Command calls resolve Serde results and reject command errors; route through `callCommand()` for app/browser-dev parity. |
| GitHub issue #470      | https://github.com/yandy-r/crosshook/issues/470                      | Source issue for Phase 5 scope and acceptance criteria.                                                                  |

---
