# Frontend research: install / update / run-executable / prefix-deps (YAN-815, YAN-1538)

Scope: every frontend consumer of the 16 Tauri commands and their events, plus browser-dev mocks,
the mock-coverage gate, and existing tests. Paths are relative to `src/crosshook-native/` unless they
start with `scripts/` or `docs/`. Snapshot: worktree HEAD `1eea9a5`. This is read-only research.

## 1. Command → frontend consumer map

| Command (src-tauri file:line)                                | Frontend call sites                                                   | Args (camelCase over IPC)                                          |
| ------------------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `install_default_prefix_path` (commands/install.rs:9-18)     | hooks/useInstallGame.ts:217                                           | `{ profileName }`                                                  |
| `validate_install_request` (install.rs:20-27)                | hooks/useInstallGame.ts:282                                           | `{ request }`                                                      |
| `install_game` (install.rs:29-37)                            | hooks/useInstallGame.ts:288                                           | `{ request }` → `InstallGameResult`                                |
| `validate_update_request` (update.rs:24-31)                  | hooks/useUpdateGame.ts:203                                            | `{ request }`                                                      |
| `update_game` (update.rs:33-69)                              | hooks/useUpdateGame.ts:254                                            | `{ request }` → `UpdateGameResult`                                 |
| `cancel_update` (update.rs:71+)                              | hooks/useUpdateGame.ts:271 (also from `reset`, 277-280)               | none                                                               |
| `validate_run_executable_request` (run_executable.rs:95-103) | hooks/useRunExecutable.ts:149                                         | `{ request }`                                                      |
| `run_executable` (run_executable.rs:105-171)                 | hooks/useRunExecutable.ts:200                                         | `{ request }` → `RunExecutableResult`                              |
| `cancel_run_executable` (run_executable.rs:179-190)          | hooks/useRunExecutable.ts:127 (also `reset`, 228-231)                 | none                                                               |
| `stop_run_executable` (run_executable.rs:203+)               | hooks/useRunExecutable.ts:135                                         | none                                                               |
| `detect_protontricks_binary` (prefix_deps.rs:213-220)        | components/settings/PrefixDependenciesSection.tsx:23                  | none                                                               |
| `check_prefix_dependencies` (prefix_deps.rs:222-300)         | hooks/usePrefixDeps.ts:99                                             | `{ profileName, prefixPath, packages }`                            |
| `install_prefix_dependency` (prefix_deps.rs:302-498)         | hooks/usePrefixDeps.ts:118; hooks/useLaunchPrefixDependencyGate.ts:47 | `{ profileName, prefixPath, packages }` → `void` (fire-and-forget) |
| `get_prefix_version_repair_status` (prefix_deps.rs:500-514)  | hooks/usePrefixDeps.ts:58; hooks/useLaunchPrefixDependencyGate.ts:55  | `{ profileName, prefixPath }`                                      |
| `repair_prefix_windows_version` (prefix_deps.rs:516-593)     | hooks/usePrefixDeps.ts:136; hooks/useLaunchPrefixDependencyGate.ts:62 | `{ profileName, prefixPath }`                                      |
| `get_dependency_status` (prefix_deps.rs:595-630)             | hooks/usePrefixDeps.ts:57; hooks/useLaunchPrefixDependencyGate.ts:40  | `{ profileName, prefixPath }`                                      |

Hook → component wiring:

- `useInstallGame` → components/InstallGamePanel.tsx:4,99 → pages/InstallPage.tsx:11,365.
- `useUpdateGame` → components/UpdateGamePanel.tsx:4,58 → pages/InstallPage.tsx:18,379.
- `useRunExecutable` → components/RunExecutablePanel.tsx:4,44-60 → pages/InstallPage.tsx:17,353,384-390
  (tab `run_executable`, `forceMount`, hidden with `display:none` when not active, so the hook and any
  listener stay mounted across tab switches).
- `usePrefixDeps` → components/PrefixDepsPanel.tsx:3,59-71 → library/profiles/HeroProfileEditorSections.tsx:217-220
  (mounted when there are declared packages or a non-empty prefix path).
- `useLaunchPrefixDependencyGate` → library/launch/useLaunchDepGate.ts:3,49-59 → HeroLaunchGate.tsx:96-97 and
  LaunchDepGateModal.tsx (repair UI 16-48, verify/installing states 94-98).
- `detect_protontricks_binary` → settings/PrefixDependenciesSection.tsx:22-26 (with the
  `auto_install_prefix_deps` toggle at 96-97).

## 2. Event map (backend emit → frontend listener)

| Event                     | Emitted at                                           | Payload today                                                            | Listeners                                                                                                                                              |
| ------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `update-log`              | commands/log_stream.rs:62,75,97,132 via update.rs:59 | plain `String`                                                           | components/ConsoleView.tsx:66; components/layout/ConsoleDrawer.tsx:149 (line count badge)                                                              |
| `update-complete`         | log_stream.rs:106 via update.rs:60                   | `number \| null` (exit code)                                             | hooks/useUpdateGame.ts:235-251                                                                                                                         |
| `run-executable-log`      | log_stream.rs:62,75,97,132 via run_executable.rs:152 | plain `String`                                                           | **none** (confirmed by grep over `src/`)                                                                                                               |
| `run-executable-complete` | log_stream.rs:106 via run_executable.rs:153          | `number \| null`                                                         | hooks/useRunExecutable.ts:186-197                                                                                                                      |
| `prefix-dep-log`          | prefix_deps.rs:412-419 (stdout), 434-441 (stderr)    | `DepLogPayload {profile_name, prefix_path, line}` (prefix_deps.rs:30-34) | components/PrefixDepsPanel.tsx:98-106 (keeps last ~200 lines)                                                                                          |
| `prefix-dep-complete`     | prefix_deps.rs:494                                   | `PrefixDepCompletePayload` (types/prefix-deps.ts:36-47)                  | components/PrefixDepsPanel.tsx:108-114 (`setInstalling(false)` + `reload()`); library/launch/useLaunchDepGate.ts:154-217 (repair/verify/launch resume) |

Install (`install_game`) emits **no** backend events; it blocks until the installer exits and the
hook just awaits the result (useInstallGame.ts:286-292). The browser mock emits
`install-started` / `install-progress` / `install-complete` (lib/mocks/handlers/install.ts:70-109,160-166)
but nothing in `src/` subscribes to them (the only `install-complete` string match is the unrelated
`ProfileReviewSource` literal at InstallGamePanel.tsx:248 / types/install.ts:3).

Wire-compat constraints that the service move must preserve:

- `prefix-dep-*` listeners filter on **raw string equality** of `profile_name` and `prefix_path`
  (PrefixDepsPanel.tsx:101,109; useLaunchDepGate.ts:155). The backend currently echoes the **raw**
  request `prefix_path` (`pfx_clone = prefix_path.clone()`, prefix_deps.rs:393), not the normalized
  one used for locking/journal (prefix_deps.rs:331-332,394). The relay must keep emitting the raw path,
  otherwise both listeners silently drop every event.
- `PrefixDepCompletePayload` fields `succeeded`, `exit_code`, `install_succeeded`, `install_exit_code`,
  `restore_state`, `restore_error` are all read (useLaunchDepGate.ts:161-206). Keep the shape.
- `update-log` payload is consumed via `normalizeLogMessage` (utils/log.ts:18-42), which accepts a
  string or `{ line }`, so switching to `{ line, … }` objects is safe for ConsoleView/ConsoleDrawer.

## 3. The complete-before-invoke workaround (exact lines)

hooks/useRunExecutable.ts:

- 49-55: `NO_EXIT_RECEIVED` sentinel symbol and `ExitHolderValue` type, kept only for the workaround.
- 163-167: comment explaining that `run-executable-complete` may arrive before `invoke` resolves.
- 168-169: `exitHolder = { value: NO_EXIT_RECEIVED }` and the `let invokeResolved = false` flag.
- 171-181: `finalizeFromExitCode` helper (keep: it maps exit code to stage and error).
- 184-198: subscribe before invoke. The callback stores the exit in `exitHolder` (188) and only
  finalizes when `invokeResolved` (192-196).
- 200-208: after `run_executable` resolves, `setResult` → `invokeResolved = true` (202), then either
  finalize from the buffered exit (204-205) or `setStage('running')` (207).
- 224: `cleanupListener()` on invoke failure.

There is no `setTimeout` in the hook. The only `setTimeout` in the panel is the 400 ms preview
debounce (RunExecutablePanel.tsx:79-82), which is unrelated and should stay.

Sibling pattern (out of the issue's AC but the same race): hooks/useUpdateGame.ts:228-260,
`let completedBeforeInvoke = false` + `if (!completedBeforeInvoke) setStage('running_updater')`.

Misleading copy to fix alongside: RunExecutablePanel.tsx:292 says the run will "stream its output to
the runtime console", and useRunExecutable.ts:263,267 hints "Check the runtime console…". ConsoleView
does not listen to `run-executable-log` (ConsoleView.tsx:65-66), so today no live output is shown
anywhere.

## 4. Proposed minimal frontend change: run-executable live log, no workaround

Backend contract the frontend needs (for the Technical/Contract researchers to confirm):

1. `run_executable` returns once the job is **registered**, before the child spawns. The result DTO
   gains `run_id: string` (the JobId) next to `helper_log_path` and `resolved_prefix_path`. Validation,
   prefix resolution and log-path creation stay synchronous so validation errors still reject the
   invoke with the existing `RunCommandError` envelope (types/run-executable.ts:47-57).
2. `run-executable-log` payload becomes `{ run_id, line }` (one emit per line, like launch-log's
   `{line, session_id}` in commands/launch/relay/mod.rs:24-25,164-166). Map `LogBatch::Dropped` and
   `LogBatch::Truncated` to a visible line, as the launch relay does (relay/mod.rs:93-104).
3. `run-executable-complete` payload becomes `{ run_id, code, signal }` (mirrors `LaunchCompletePayload`,
   types/launch.ts:153-157). Today `null` conflates signal and lost status.
4. Single flight: the service uses one global run-executable JobKey, so at most one run is live and the
   frontend can tolerate events that arrive before `run_id` is known.

Even with (1), Tauri does not order a command response against events. `run-executable-complete` can
still reach the webview before `invoke` resolves for a process that exits immediately. The fix is
to stop the terminal stage from depending on `result`, not to buffer the event:

hooks/useRunExecutable.ts:

- Delete 49-55, 163-169 and 192-208. Keep `finalizeFromExitCode`, taking `{ code, signal }`.
- Add `const [logLines, setLogLines] = useState<string[]>([])` and a `runIdRef`.
- Add a mount-scoped listener effect (like PrefixDepsPanel.tsx:97-120) instead of per-run
  subscribe/unsubscribe. Because the panel is `forceMount`ed, this is effectively app-lifetime:
  - `run-executable-log`: if `runIdRef.current` is set and the payload has a different `run_id`,
    ignore it. Otherwise append `normalizeLogMessage(payload)`, capped (for example `slice(-500)`).
  - `run-executable-complete`: same `run_id` check, then `finalizeFromExitCode`.
- `startRun`: `setLogLines([])`, `runIdRef.current = null`, validate (unchanged 148-161), then
  `const runResult = await callCommand('run_executable', { request })`, `runIdRef.current = runResult.run_id`,
  `setResult(runResult)`, `setStage((s) => (s === 'preparing' ? 'running' : s))`. The functional
  update is the whole race fix: if complete already set `complete`/`failed`, it is kept, and
  `result` (log path, resolved prefix) still renders because the panel reads it independently
  (RunExecutablePanel.tsx:87-88,211-223). This needs no flag, no sentinel and no timer.
- `reset` (228-238): also `setLogLines([])` and clear `runIdRef`. `cleanupListener`/`unlistenRef`
  (79-84, 240-242) go away with the mount-scoped effect.
- Export `logLines` in `UseRunExecutableResult` (14-30).
- Hints (260-271): "Live output is shown below." in place of the runtime-console text.

components/RunExecutablePanel.tsx: in the review block (203-237), after the resolved-prefix row,
render a log region when `isRunning || logLines.length > 0`:
`<pre role="log" aria-live="polite" aria-busy={isRunning}>` with `logLines.join('\n')` or
"Waiting for output…". Reuse the existing styles `.crosshook-prefix-deps__log`,
`__log-header` and `__log-output` (styles/theme.css:5302-5318, max-height 200px). Either generalize
them to a shared `crosshook-process-log` class or add a `--run-executable` modifier. Do not add a
second ad-hoc style. Change the confirm copy at 292 to "…and stream its output below."

Optional consistency: ConsoleView.tsx:65-66 and ConsoleDrawer.tsx:148-149 could also subscribe to
`run-executable-log`, but the AC only requires the panel. Keep the panel as the single owner to avoid
duplicate display.

Optional same-pattern cleanup in useUpdateGame.ts:228-260 (`completedBeforeInvoke`): replace with
`setStage((s) => (s === 'preparing' ? 'running_updater' : s))`. The issue only mandates
run-executable, but the update service gets the same JobRegistry shape, so this is cheap.

Install (`useInstallGame`) needs **no** frontend change if the Tauri adapter keeps awaiting the job's
`Finished` outcome before returning `InstallGameResult` (useInstallGame.ts:268-315 has no
listener and no cancel UI).

## 5. Prefix deps panel: showing recovered or failed journal state

What exists today:

- DTO `PrefixVersionRepairStatus { required, state: 'pending'|'failed'|null, last_error }`
  (Rust prefix_deps.rs:50-55; TS types/prefix-deps.ts:22-30). It comes from the journal row by
  `repair_status_from_row` (prefix_deps.rs:74-96): no row → `required:false`; `pending`/`failed` row
  → `required:true` plus `last_error`.
- `usePrefixDeps.load` fetches it alongside `get_dependency_status` with `Promise.allSettled`
  (usePrefixDeps.ts:53-81). It reloads after repair (133-154) and after every `prefix-dep-complete`
  (PrefixDepsPanel.tsx:108-114).
- The panel renders a blocking `role="alert"` banner "Prefix repair required" with
  `Last repair attempt: {last_error}` and a Repair button (PrefixDepsPanel.tsx:181-200). Dependency
  actions are disabled while repair is required, unknown or errored (93-94, 212, 227, 237, 294).
- The launch gate re-checks before each launch (useLaunchDepGate.ts:385-418) and the modal shows the
  same text (LaunchDepGateModal.tsx:18-48).
- `state` is never displayed: pending and failed render identically. Only
  useLaunchDepGate.test.tsx:271 asserts on `.state`.

So **a failed journal entry is already exposed and shown.** If startup recovery marks a crashed
`pending` row as `failed` with a descriptive `last_error` (for example "CrossHook stopped during a
dependency install; automatic restore failed: …"), the existing command, DTO and banner show it with
**zero frontend changes**. Likewise, a crashed row whose startup restore succeeds is deleted and
disappears from the UI.

Gaps the spec implies ("resolved on next start … and shown in Prefix deps panel"):

1. **Recovered (restored on startup) is invisible.** The row is deleted, so no command can report
   it. Minimal additive extension: add an optional
   `last_recovery: { outcome: 'restored' | 'failed'; at: string; error: string | null } | null` to
   `PrefixVersionRepairStatus`. It is sourced from a runtime-only, in-memory startup-recovery report
   held by the prefix-deps service and keyed by normalized prefix. The journal format stays the same,
   which matches the spec rule that registries are runtime-only and there is no migration. Frontend:
   in PrefixDepsPanel, when `!repairStatus.required && repairStatus.last_recovery?.outcome === 'restored'`,
   render a `role="status"` note: "CrossHook restored this prefix's Windows version after an
   interrupted install." The failed outcome keeps using the existing alert banner.
   Mirror the field in lib/mocks/handlers/system.ts:315-319 (`MOCK_PREFIX_VERSION_REPAIR_STATUS`).
2. **Pending versus failed copy.** In the banner, use `state === 'pending'` for "An interrupted restore
   is pending" and keep the current text for `failed`. This is a cosmetic, one-line conditional at
   PrefixDepsPanel.tsx:183-188.
3. **Live install looks like "repair required".** During an active install the journal row is
   `pending` (written at prefix_deps.rs:367-386 before spawn), so any `get_prefix_version_repair_status`
   call during that time returns `required:true`. A panel mounting mid-install, the launch gate
   pre-check, or a "Check Now" reload would show the blocking repair banner, and clicking Repair would
   hit the per-prefix lock (prefix_deps.rs:527-531). Once installs are JobRegistry jobs, the service
   should report an in-flight job for that prefix. Either add `in_progress: boolean` to the DTO, or
   return `required:false` while the prefix JobKey is held. The panel then shows its existing
   `installing` UI instead (PrefixDepsPanel.tsx:252-265).

The issue requires "explicit recovery … before each install". The existing pre-install check
already rejects an install when a journal row exists (prefix_deps.rs:342-351). The error surfaces
through `usePrefixDeps.installDep` → `setError` (usePrefixDeps.ts:125-127) → panel error `<p>`
(PrefixDepsPanel.tsx:246-250). A second concurrent install on the same prefix (the per-prefix JobKey
`Conflict`) surfaces the same way, so no frontend change is needed for the exclusivity AC. The message
just has to be user-readable after the `AppError` → string mapping.

Offline: `install_prefix_dependency` failing with `Unavailable` also surfaces through the same
`setError` path. Nothing else in these hooks performs downloads.

## 6. Browser-dev mocks

Registered through lib/mocks/index.ts: install 19,80; system 31,61,87; update 34,88.

- lib/mocks/handlers/install.ts:112-174: `install_default_prefix_path` (112-120),
  `validate_install_request` (122-137), `install_game` (139-174). It has a single-flight guard, emits
  `install-*` events with no consumer, and resolves after about 1.5 s. Reset is at 177-183.
- lib/mocks/handlers/update.ts:32-78: validate (32-36), `update_game` (38-71) emits `update-complete`
  after 1.5 s (57-64), `cancel_update` (73-78). It never emits `update-log` (55-56 comment). Reset is at 81-88.
- lib/mocks/handlers/system.ts, run-executable block 578-600: all four are `console.warn` stubs.
  `run_executable` returns a static result (584-592) and **never emits** `run-executable-log` or
  `-complete`, so in browser dev mode the stage sticks at `running` forever.
- system.ts, prefix-deps block 626-658: `detect_protontricks_binary` (628-630), `check_prefix_dependencies`
  (632-641, all installed), `install_prefix_dependency` (643-645, a no-op that **never emits**
  `prefix-dep-complete`, so the panel `installing` state sticks), `get_dependency_status` (647-649, `[]`),
  `get_prefix_version_repair_status` / `repair_prefix_windows_version` (651-658,
  `MOCK_PREFIX_VERSION_REPAIR_STATUS` at 315-319, never required).
- `resetSystemMockState` (system.ts:751-753) resets only external sources and has no timer set.

Mock updates needed for the AC "mocks updated":

- `run_executable` mock: return `run_id` (for example `mock-run-<n>`). Through a `scheduleRunTimeout` helper
  with a `pendingRunTimers` set (copy update.ts:9,23-29), emit 3–5 `run-executable-log` `{run_id, line}`
  lines, then `run-executable-complete` `{run_id, code: 0, signal: null}`. `cancel_run_executable` and
  `stop_run_executable` should finish the pending run with `{ code: null, signal: 15 }` or `9`. Clear
  timers in `resetSystemMockState`. Optional: move the run-executable mocks into a new
  `handlers/run_executable.ts` to keep system.ts (753 lines) from growing, then update the README tree
  (lib/mocks/README.md:24-36).
- Optional: make `install_prefix_dependency` emit `prefix-dep-log` and `prefix-dep-complete`
  (`restore_state: 'not_required'`) so the browser panel completes. If `last_recovery` or `in_progress`
  is added, mirror it in `MOCK_PREFIX_VERSION_REPAIR_STATUS`.
- Optional: drop the unconsumed `install-*` emits, or leave them alone (out of scope).

### scripts/check-mock-coverage.sh rules

- It scans only `#[tauri::command] … pub (async )?fn NAME` in `src-tauri/src` and
  `crates/crosshook-core/src` (lines 27-30, 67-72). **`crates/crosshook-app/src` is not scanned**, so
  commands must keep living as thin `#[tauri::command]` adapters in src-tauri, which matches prior moves.
- Mock keys come from `map.set('name'` under `src/lib/mocks/handlers` (89-95). Keys prefixed `_mock_`
  are excluded.
- It exits 1 on any missing **or** orphaned name (136-140). It does **not** check events or payloads.
  Keeping all 16 command names means no change. Any rename or new command (for example a dedicated
  journal-status command) needs a matching `map.set`. Removing a command needs its mock removed.
- It runs as `npm run dev:browser:check` (package.json:12).

## 7. Existing tests touching these paths

Vitest (`npm test`, package.json:16):

- components/**tests**/PrefixDepsPanel.test.tsx: mocks `@/lib/ipc` and `@/lib/events` through an
  `eventHandlers` map (7-22). Cases: repair blocks mutations (58), repair via IPC (69-81), reload
  after `prefix-dep-complete` (83-114), neutral state (116-120).
- hooks/**tests**/usePrefixDeps.test.ts: reload (24), repair refresh (35-64, asserts 5 IPC calls),
  dependency-load failure keeps repair (66-79), preserves known repair on reload failure (81+).
- components/library/**tests**/useLaunchDepGate.test.tsx: mocks `useLaunchPrefixDependencyGate`
  (43-50) and `subscribeEvent`. Cases at 92, 117, 145, 177-226, 271, 385.
- components/library/**tests**/LaunchDepGateModal.test.tsx: repair without skip (38-62), verify busy
  (64-82).
- components/library/**tests**/HeroProfileEditorSections.test.tsx:110-116,304-351 (stubs the panel).
- components/library/**tests**/HeroLaunchGate.test.tsx (`auto_install_prefix_deps` settings only).
- components/**tests**/InstallGamePanel.test.tsx: `useInstallGame` fully mocked (11-17).
- components/**tests**/ConsoleView.test.tsx: only `launch-log` with `{line, session_id}` (8-22), using
  `renderWithMocks` + `emitMockEvent`. components/layout/**tests**/ConsoleDrawer.test.tsx: no
  run or update references.
- **tests**/a11y/routes.a11y.test.tsx:102 renders `InstallPage` (so RunExecutablePanel and
  UpdateGamePanel) through the real mock registry. A new log region must stay axe-clean.
- **No tests** exist for `useRunExecutable`, `RunExecutablePanel`, `useUpdateGame`, `UpdateGamePanel`,
  `useInstallGame`, or `PrefixDependenciesSection`.

Playwright (`npm run test:smoke`): tests/smoke.spec.ts only visits the `install` route and checks
the heading and shell (44, 59, 110). No run-executable, update or prefix-deps flows.

Recommended new frontend tests:

- `hooks/__tests__/useRunExecutable.test.ts` (use the `vi.mock('@/lib/events')` map pattern from
  PrefixDepsPanel.test.tsx:7-22 and a deferred `callCommand` promise):
  (a) complete arrives **before** `run_executable` resolves → final stage `complete`, `result`
  populated, no `running` flash; (b) normal order → `running` then `complete`; (c) log lines
  accumulate and are capped; lines with a foreign `run_id` are ignored once `run_id` is known;
  (d) `signal` → `failed` with the signal message; (e) `reset` clears `logLines`.
- `components/__tests__/RunExecutablePanel.test.tsx`: `renderWithMocks` + `emitMockEvent('run-executable-log', …)`
  → line visible in `role="log"`. This also serves as the "listener exists" assertion.
- If `last_recovery` or `in_progress` is added: extend PrefixDepsPanel.test.tsx with a restored note
  (status, not alert) and an in-progress state (no repair banner).

## 8. Files the frontend slice will touch (minimal set)

- hooks/useRunExecutable.ts: remove the workaround, add the log listener and `logLines`.
- components/RunExecutablePanel.tsx: log region and copy.
- types/run-executable.ts: `run_id` on `RunExecutableResult` (10-21); new
  `RunExecutableLogPayload {run_id, line}` and `RunExecutableCompletePayload {run_id, code, signal}`.
- lib/mocks/handlers/system.ts (or a new handlers/run_executable.ts + index.ts + README): emitting mock.
- styles/theme.css (5302-5318): generalize the log classes if shared.
- New tests: hooks/**tests**/useRunExecutable.test.ts and components/**tests**/RunExecutablePanel.test.tsx.
- Only if the DTO grows: types/prefix-deps.ts (22-30), components/PrefixDepsPanel.tsx (181-200),
  system.ts:315-319, PrefixDepsPanel.test.tsx.
- Optional consistency: hooks/useUpdateGame.ts (228-260) and lib/mocks/handlers/update.ts (`update-log` lines).
