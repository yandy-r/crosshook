# Launch service (M1-05) — React-facing contract research

YAN-798 / GitHub #534. This document covers the user-visible contract that the launch-service refactor must preserve: event names, payload shapes, command signatures, TS types, browser-dev mocks and tests. It also covers how an optional `session_id` can be added without breaking any of them. Paths are relative to `src/crosshook-native/` unless absolute. Line numbers are from the `crosshook-yan-798` worktree as read on 2026-10-25.

Evidence limits: read-only source research. Nothing was built, run or rendered. Statements about runtime ordering and latency are inferred from control flow and marked **[INFERENCE]**.

## 1. Executive summary

- **Four events and six commands are in scope.** React reaches all of them only through the shared adapters `subscribeEvent` (`src/lib/events.ts:7-18`) and `callCommand` (`src/lib/ipc.ts:7-17`). In browser-dev mode they are served by `src/lib/mocks/handlers/launch.ts`. No React code calls Tauri `listen`/`invoke` directly for these.
- **`launch-log` is the only event whose wire shape must change.** Today the payload is a bare JSON string (`src-tauri/src/commands/launch/streaming.rs:93,126,134,183,204`; `src-tauri/src/commands/bench.rs:292`). A string cannot carry `session_id`, so the payload has to become an object.
  - `{ "line": string, "session_id": string }` is already accepted by the frontend: `normalizeLogMessage` handles `{ line }` (`src/utils/log.ts:6,26-28`).
  - So ConsoleView and ConsoleDrawer need no logic change, only a type addition.
  - **Do not use a `{ lines: [...] }` shape.** It would fall through to `JSON.stringify` (`log.ts:36`) and print raw JSON in the console.
- **`launch-diagnostic`** carries `DiagnosticReport`. That Rust struct is also persisted to SQLite `launch_operations.diagnostic_json` (`crates/crosshook-core/src/metadata/launch_history.rs:66`).
  - Adding `session_id` to the struct would change the persisted format and inflate rows toward the 4 KB cap. That violates the "No format changes" storage boundary.
  - The adapter must add `session_id` at emit time only, using a flattening wrapper.
  - The TS guard `isDiagnosticReport` ignores unknown keys (`src/types/diagnostics.ts:127-147`), so the extra field is safe.
- **`launch-complete`** is `{code, signal}` built with `json!` (`streaming.rs:433-441`). All three consumers ignore the payload, so adding `session_id` is free.
- **`injection-log`** already carries a required `session_id: string` (`commands/launch/shared.rs:111-123`; `src/types/injection.ts:10-46`).
  - No change is needed.
  - The new ids on `launch-log`, `launch-diagnostic` and `launch-complete` must be **the same string**: `SessionId`'s `Display`, a hyphenated UUID (`crates/crosshook-core/src/launch/session/types.rs:21,29-32`).
- **Strict validation exists in exactly one place: `isStrictLaunchValidationIssue`** (`src/components/community-import/utils.ts:85-101`), which allow-lists keys. **Do not** add `session_id` or any other field to `LaunchValidationIssue`, the `validate_launch` error type.
- **Batching:** the recommended mapping is **one `launch-log` event per line** from each `LogBatch::Lines`. That keeps rendering, line counts and timestamps exactly as today. Emitting one joined string per batch is React-compatible but visibly changes the console (one timestamp per batch). See §5.

## 2. Event contract (current → required)

### 2.1 Emission points and finalizer order (must be preserved)

`spawn_log_stream` (`streaming.rs:26-58`) runs one task per session. Within a session, events are strictly sequential:

1. **Log lines**, from one of two paths:
   - File-poll path (`stream_log_lines`, `:61-138`): a whole-file `read_to_string` every 500 ms (`:80-117`), then a final read (`:121-132`).
   - Pipe path (`stream_log_pipes`, `:140-210`): one line per `rx.recv()`. stdout and stderr share a single unbounded mpsc (`:148-154`), so they interleave in arrival order.
2. **Suppression summary line**, if any gamescope `xdg_backend` lines were suppressed (`:133-137` / `:203-207`; text from `shared.rs:56-63`).
3. `finalize_launch_stream` (`:236-445`) does three things in order:
   - history row, version snapshot and known-good tag;
   - **`launch-diagnostic`**, only when `should_surface_report(&report)` (`:427-431`);
   - **`launch-complete`** `{code, signal}`, always (`:433-441`).
4. `emit_injection_stream_finalization` (`:447-512`): trainer sessions only; emits 1–2 `injection-log` rows.
5. `finalize_launch_session` (`:518-542`): `cancel_linked_children` cascade, post-exit hooks, deregister.

Trainer-only `injection-log` rows are also emitted around spawn in `launch_trainer` (`execution.rs:367-418`):

- "Trainer launch requested." (`:367`);
- the spawn-failure error (`:381`), emitted **before** `Err` is returned (`:390-393`);
- "Trainer process started." (`:399`) and the unsupported-runtime warning (`:409`).

So the service must surface pre-spawn events even when no `LaunchSession` is ever returned to the adapter. Two ways to do that: an event sink that is live before spawn, or returning the pre-spawn events alongside the error.

**Required invariant for the service:** every `Log` for a session reaches the adapter before that session's `Diagnostic`, which reaches it before `Exited`.

- Use one ordered channel per session (`LaunchSession.events`), drained by one adapter task.
- If `Log` and `Diagnostic`/`Exited` travel on separate channels, the console could show trailing lines after the diagnostic banner. That cannot happen today. **[INFERENCE]**

### 2.2 Per-event table

| Event               | Rust emitter / payload today                                                                               | Required payload                                                                    | React consumers (all via `subscribeEvent`)                                                                                                                                                                                                      | Validation                                                                                                                                                                                                           |
| ------------------- | ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `launch-log`        | `app.emit("launch-log", String)` per UI line, after `transform_launch_log_line_for_ui` (`shared.rs:34-54`) | `{ line: String, session_id: String }` per line                                     | `components/ConsoleView.tsx:46-72`: one `ConsoleLine` per event, `normalizeLogMessage(payload).trimEnd()`, drops empty. `components/layout/ConsoleDrawer.tsx:135-157`: badge count via `countLogLines` (`:9-16`), splits `\r?\n`, ignores blank | none (structural `normalizeLogMessage`)                                                                                                                                                                              |
| `launch-diagnostic` | `app.emit(..., &DiagnosticReport)` (`streaming.rs:428`)                                                    | same JSON object **plus** top-level `session_id` (wrapper with `#[serde(flatten)]`) | `hooks/useLaunchState.ts:266-281`: `isDiagnosticReport`, then accepted only if `payload.log_tail_path === activeHelperLogPathRef.current`                                                                                                       | `isDiagnosticReport` checks known fields only; extra keys already pass. Proof: `coexistence_advisories` is serialized today (`crosshook-core/src/launch/diagnostics/models.rs:31-32`) but is absent from the TS type |
| `launch-complete`   | `json!({code, signal})`; both `Option<i32>` → number or null                                               | `{ code, signal, session_id }`                                                      | `useLaunchState.ts:282-286` (reducer `'launch-complete'` is a no-op, `:82-83`); `hooks/useProfileHealth.ts:292-295` (`batchValidate()`); `hooks/useRunningProfiles.ts:47-49` (refresh)                                                          | none; payload ignored                                                                                                                                                                                                |
| `injection-log`     | `InjectionLogEvent` (`shared.rs:111-123`) via `emit_injection_log_event` (`:156-181`)                      | unchanged                                                                           | `components/library/trainer/InjectionLogTail.tsx:74-87`, mounted with `profileName` only (`HeroDetailTrainerTab.tsx:138`); filters by profile, plus `sessionId`/`sessionKind` props when given (`:29-41`)                                       | `isInjectionLogEvent` requires `session_id: string` (`injection.ts:27-46`)                                                                                                                                           |

Notes:

- **Duplicate `launch-complete`:** game and trainer each emit their own, so health revalidates twice per two-step session (`useProfileHealth.ts:292`). Existing behaviour; keep it.
- **Diagnostic matching is path-based.**
  - The finalizer sets `report.log_tail_path = sanitize_display_path(log_path)` (`streaming.rs:245`), which wraps `display_path_tilde` (`src-tauri/src/commands/shared.rs:6-8`).
  - The UI compares that against the **raw** `LaunchResult.helper_log_path` (`execution.rs:230-235,486-489`).
  - They match only because `/tmp/crosshook-logs/…` (`crates/crosshook-app/src/jobs/process.rs:228-243`) is never under `$HOME`.
  - The service must keep both strings byte-identical: same `create_log_path`, same sanitizer.
  - `session_id` would be a more robust key, but switching the filter is a React change. Leave it out of this PR unless the plan explicitly accepts it.
- **Trainer-then-game ordering:** once the trainer launches, `activeHelperLogPathRef` points at the trainer log (`useLaunchState.ts:433`), so a later **game** diagnostic is dropped. Existing behaviour; must stay unchanged.

## 3. Command contract (unchanged; the adapter keeps these signatures)

- **Argument names:** camelCase on the JS side (Tauri v2, `src-tauri/Cargo.toml:21`).
- **Where commands live:** they stay `#[tauri::command]` in `src-tauri`.
  - `scripts/check-mock-coverage.sh:27-30` scans only `src-tauri/src` and `crates/crosshook-core/src`, and diffs command names against the mock registry.
  - So the `crosshook-app` service must not declare commands, and the names must not change.

| Command                              | Rust today             | JS caller(s)                                                                 | Args → return / error shape React relies on                                                                                                                                                                                   |
| ------------------------------------ | ---------------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `launch_game`                        | `execution.rs:61-236`  | `useLaunchState.ts:324` (via `context/LaunchStateContext.tsx:33`)            | `{request: LaunchRequest}` → `LaunchResult {succeeded, message, helper_log_path, warnings?}` (`shared.rs:65-72` skips empty warnings; TS `types/launch.ts:144-150`). Error is a **string** shown as `feedback.kind='runtime'` |
| `launch_trainer`                     | `execution.rs:239-490` | `useLaunchState.ts:423`                                                      | same as above; errors include `"native launch does not support trainer launch."` (`:328`) and `"failed to launch trainer (method=…): …"`                                                                                      |
| `validate_launch`                    | `queries.rs:22-25`     | `useLaunchState.ts:132-142`; `components/CommunityImportWizardModal.tsx:355` | `{request}` → `null` or thrown **`LaunchValidationIssue` object**; `isStrictLaunchValidationIssue` allow-lists `message, help, severity, code, trainer_hash_*`                                                                |
| `preview_launch`                     | `queries.rs:28-53`     | `hooks/usePreviewState.ts:23`; `CommunityImportWizardModal.tsx:194`          | `{request}` → `LaunchPreview` (`types/launch.ts:249+`), mod-coexistence issues appended to `validation.issues`; error string                                                                                                  |
| `build_steam_launch_options_command` | `queries.rs:62-93`     | `components/SteamLaunchOptionsPanel.tsx:70-76`                               | `{enabledOptionIds, customEnvVars, gamescope, enabledArgumentIds, customCommandArgs}` → `string`; error string shown inline                                                                                                   |
| `list_launch_history_for_profile`    | `queries.rs:124-141`   | `hooks/useLaunchHistoryForProfile.ts:30`; `hooks/useProfileCardMeta.ts:39`   | `{profileName, limit?}` → `LaunchHistoryEntry[]` (`types/library.ts:35-45`). Blank name → `Err("profile_name is required")`; metadata unavailable → `[]`; `limit` defaults to 20, capped at `MAX_HISTORY_LIST_LIMIT`          |

Session-adjacent commands React also uses: `launch_reset_sessions` (`execution.rs:503-509`), `list_running_profiles` and `check_game_running` (`queries.rs:107-120`). They must keep reading the same `LaunchSessionRegistry` the service registers into.

`LaunchResult` may optionally gain a `session_id` field.

- It is backward-compatible: `useLaunchState` reads only `warnings` and `helper_log_path`, and `__tests__/LaunchSubTabs.test.tsx:399-401` already passes a loose object.
- Recommendation: add it (always set, since every launch registers) so React can correlate later without another contract change. Do not consume it in this PR.

## 4. Adding `session_id` safely — exact changes

### 4.1 Rust (Tauri adapter only; no core or persisted struct changes)

- **launch-log:** a private DTO, `#[derive(Serialize)] struct LaunchLogPayload<'a> { line: &'a str, session_id: &'a str }`.
  - Emit one per UI line, after the per-session `LaunchLogRelayState` transform.
  - The suppression notice and summary lines carry the same `session_id`.
- **launch-diagnostic:** `#[derive(Serialize)] struct LaunchDiagnosticPayload<'a> { #[serde(flatten)] report: &'a DiagnosticReport, session_id: &'a str }`.
  - **Do not** add a field to `DiagnosticReport` (`models.rs:11-33`). It is serialized into `diagnostic_json` (`launch_history.rs:66`), and the 4 KB-cap tests in `metadata/launcher_tests.rs:173-279` depend on its size.
  - `library/launch/HeroLaunchGate.tsx:126-129` copies `JSON.stringify(feedback.report)` to the clipboard, so the copied text will now include `session_id`. It is a runtime UUID, not a path or secret: harmless but visible.
- **launch-complete:** replace `json!` with `struct LaunchCompletePayload { code: Option<i32>, signal: Option<i32>, session_id: String }`. Keep `null` for missing values (no `skip_serializing_if`) to stay byte-compatible with today.
- **injection-log:** unchanged. `build_injection_log_event` (`shared.rs:126-153`) already stringifies `SessionId`. When it moves into the service, it must use the same id as the other three events.
- **Id source:** all four `session_id` values come from `SessionId::to_string()`. Never use a second id scheme such as a `JobId` UUID, or `InjectionLogTail`'s `sessionId` filter (`InjectionLogTail.tsx:33-35`) cannot be correlated with log lines.

### 4.2 TypeScript (types only)

- **`src/utils/log.ts:6`:** add `export interface LaunchLogPayload { line: string; session_id?: string }` to the `LogPayload` union.
  - `normalizeLogMessage` needs no change; `'line' in record` already matches.
  - `update-log` still emits strings through the same handler (`ConsoleView.tsx:66`; `ConsoleDrawer.tsx:149`), so the union must keep `string`.
- **`src/types/diagnostics.ts`:** add `export type LaunchDiagnosticEvent = DiagnosticReport & { session_id?: string }` and use it as the generic at `useLaunchState.ts:266`.
  - The guard can optionally accept `session_id === undefined || typeof === 'string'`. This is not required; the current guard ignores extra keys.
  - **Do not** put `session_id` on `DiagnosticReport`. That type also models persisted `diagnostic_json` rows (doc comment at `:66-74`).
- **`src/types/launch.ts`:**
  - add `export interface LaunchCompletePayload { code: number | null; signal: number | null; session_id?: string }`. Today the payload is untyped: consumers pass no generic or `unknown`.
  - optionally add `session_id?: string` to `LaunchResult` (`:144-150`).
- **`src/types/injection.ts`:** unchanged; `session_id: string` is already required.
- **Why `?:` and no other tooling changes are needed:**
  - `tsconfig.json:10` sets `strict` without `exactOptionalPropertyTypes`, so mocks may omit the field.
  - `emitMockEvent(name, payload: unknown)` (`events.ts:21`) performs no excess-property checks.
  - No ts-rs bindings exist for launch types (`src/types/generated/` covers only health, launch_hooks and onboarding), so nothing needs regenerating.

### 4.3 Browser-dev mocks (`src/lib/mocks/handlers/launch.ts`)

The mocks should emit the new wire shape so they stay representative.

- **`scheduleLaunchLogSequence` (`:292-322`):** add a `sessionId` parameter and emit:
  - `emitMockEvent('launch-log', { line, session_id: sessionId })` (`:302`);
  - `launch-diagnostic` as `{ ...makeDiagnosticReport(path), session_id }` (`:311`);
  - `launch-complete` as `{ code: 0, signal: null, session_id }` (`:320`).
- **`launch_game` (`:430-453`):** use `mock-game-session-${steamAppId}`.
- **`launch_trainer` (`:458-480`):** hoist the id that `scheduleTrainerInjectionLogSequence` already builds, `mock-trainer-session-${steamAppId}` (`:345`). The trainer's `launch-log`, `launch-diagnostic`, `launch-complete` and `injection-log` rows then share one id, exactly as the backend will.
- **`makeLaunchResult` (`:185-191`):** add `session_id` only if the optional `LaunchResult` field from §3 is adopted.
- **Unchanged mocks:** `validate_launch` (`:485-504`), `preview_launch` (`:507+`), `build_steam_launch_options_command` (`:775-797`) and `list_launch_history_for_profile` (`:401-427`).

### 4.4 Tests that cover the contract

| Test                                                                 | What it pins                                                                                                                             | Change needed                                                                                                                                                                                         |
| -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `components/layout/__tests__/ConsoleDrawer.test.tsx:38-62`           | multi-line string payload → "2 lines"; drawer does not auto-expand                                                                       | keep; **add** a case emitting `{ line: 'a', session_id: 's1' }` → "1 line"                                                                                                                            |
| `components/layout/__tests__/AppShell.test.tsx:245-266`              | string `launch-log` → "1 line" badge                                                                                                     | keep (strings stay valid for `update-log`/the union)                                                                                                                                                  |
| `components/library/__tests__/HeroDetailTrainerTab.test.tsx:288-376` | `injection-log` profile filter, malformed rejection, 200-row cap, `session_id` per row                                                   | unchanged                                                                                                                                                                                             |
| `hooks/__tests__/useLaunchState.test.ts:21-23`                       | **mocks `subscribeEvent` to a no-op**, so diagnostic/complete handling is untested today                                                 | **add** a test: `launch-diagnostic` with an extra `session_id` key still matches by path (proves the guard tolerates it)                                                                              |
| `tests/pipeline.spec.ts:63-100` (Playwright)                         | clicks "Launch game" in browser-dev mode (mock emits `launch-log`/`launch-diagnostic`/`launch-complete`); asserts `capture.errors` empty | none; this is the "React smoke" for the mock path                                                                                                                                                     |
| `tests/smoke.spec.ts`                                                | route/console-error smoke; no launch events                                                                                              | none                                                                                                                                                                                                  |
| no test today                                                        | ConsoleView renders an object payload as text, not JSON                                                                                  | **add** `components/__tests__/ConsoleView.test.tsx`: emit `{line, session_id}`, assert the line text renders and no `{"line"` appears                                                                 |
| `src-tauri/src/commands/launch/tests/log_relay.rs:7-85`              | xdg_backend suppression; `injection-log` JSON shape                                                                                      | move with the code; **add** serialization tests: launch-log DTO = `{"line","session_id"}`; diagnostic wrapper flattens and adds `session_id`; complete = `{"code":null,"signal":null,"session_id":…}` |

Nothing in `src/` performs strict (allow-list) validation of `launch-log`, `launch-diagnostic` or `launch-complete` payloads. The only allow-list guard, `isStrictLaunchValidationIssue`, applies to `validate_launch` errors, which this change leaves untouched.

## 5. Log batching cadence and console implications

### 5.1 Today

- **File-poll path:** children without piped stdio.
  - Covers `steam_applaunch` games (`build_helper_command`, `execution.rs:118-124`), `native` games (`:132-133`), and the non-Flatpak Steam trainer (`:314-319`).
  - Every 500 ms the whole log is re-read (`streaming.rs:80`). Every new non-empty line becomes its own event, in a tight loop.
  - The console therefore receives up to ~500 ms of output as N back-to-back events.
  - Truncation is detected only via `content.len() < last_len` (`:82-84`).
- **Pipe path:** used for the `proton_run` game (`execution.rs:128-129`), the Flatpak Steam trainer (`:311-312`) and the `proton_run` trainer (`:324-325`).
  - Each line is emitted as soon as it is read and also appended to the log file (`streaming.rs:170-187`). Latency is near-zero, at one event per line. **[INFERENCE]**
- **Cross-session ordering:** game and trainer are independent tasks, so their lines interleave in the single global console with no attribution. Within a session, order is preserved.
- **Bench:** `bench.rs:269-301` replays up to 5,000 lines/s as one string event per line (metric G8, `scripts/bench/bench.py:191-193`). That is the console's current throughput contract.

### 5.2 With M1-04 `log_tail::follow`

`crates/crosshook-app/src/log_tail.rs:317-337` emits:

- one `LogBatch::Lines(Vec<String>)` per 33 ms coalescing window (clamped 16–33 ms, `:266-298`);
- at most `READ_CHUNK` = 256 KiB per batch (`:36`);
- with idle backoff up to 500 ms;
- on cancel, a final drain of up to 16 chunks, then a flush of the partial line.

Differences the adapter must absorb:

- **Empty lines:** `LogTail` yields `""` for blank lines. Today's relay drops them (`streaming.rs:91,174`), and ConsoleView drops empty text too. Filter `""` before emitting so line counts stay identical.
- **CR / invalid UTF-8:** `decode_line` strips a trailing `\r` and replaces invalid bytes with U+FFFD (`log_tail.rs:259-262`).
  - Today `read_to_string` fails on invalid UTF-8: it logs a warning, emits nothing for that poll (`streaming.rs:102-104`) and keeps retrying. The new behaviour is strictly better for the GUI.
  - **CLI caveat** for the "output unchanged" criterion: `stream_helper_log` (`crates/crosshook-cli/src/launch.rs:278-319`) copies raw bytes, passing CRLF, invalid bytes and the final unterminated line through untouched. Printing `LogTail` lines with `println!` will differ for such logs (the unterminated last line gains a `\n`). Cover this in the CLI fixture test.
- **Partial lines:** `follow` buffers an unterminated line until `\n` or cancel. Today's file path splits `.lines()` over the current content, so a half-written last line can be emitted as a fragment and its remainder later as a separate line. The new behaviour removes those split fragments, a small visible improvement. **[INFERENCE]**
- **Pipe-captured children:** if the service routes pipes to the file and then tails it, per-line latency rises by up to one window (≤33 ms; imperceptible). If it batches pipe lines directly, it must keep a single merged mpsc so stdout/stderr interleaving stays as today.
- **Gamescope suppression** (`LaunchLogRelayState`) must stay **per session** and be applied line-by-line inside each batch. The summary line must still follow the last batch and precede `Diagnostic`.

### 5.3 Mapping a `LogBatch` to `launch-log` — options

| Option                    | Wire                                   | Console UX                                                                                                                          | React change                                  | Verdict                                                                                                                                                                               |
| ------------------------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A. One event per line** | `{line, session_id}` × N per batch     | identical to today: one row and one receipt timestamp per line (`ConsoleView.tsx:54-58`); drawer count identical                    | types only                                    | **Recommended.** Satisfies "existing names and payloads" literally. Backend wakeups drop (offset reads replace the 500 ms full re-read); IPC event count unchanged, matching bench G8 |
| B. One event per batch    | `{line: lines.join("\n"), session_id}` | one `<pre>` block per batch with **one timestamp**; up to 256 KiB per row; drawer count still correct (`countLogLines` splits `\n`) | none in code, but visibly different rendering | only with an explicit UX decision; filter blank lines before joining                                                                                                                  |
| C. Array payload          | `{lines: [...], session_id}`           | console prints raw JSON (`log.ts:36`)                                                                                               | ConsoleView and ConsoleDrawer must change     | rejected for this PR                                                                                                                                                                  |

`LogBatch::Dropped { n }` can appear when a subscriber lags (job-registry semantics, `crates/crosshook-app/src/jobs/registry.rs:505-518`).

- Map it to one visible `launch-log` line with the same `session_id`, e.g. `[crosshook] Console skipped N log batches; the raw launch log is complete.`
- Never drop it silently, and never send it to `injection-log`.

Existing perf caveat, unchanged under option A: ConsoleView keeps an unbounded `lines` array and copies it on every event (`setLines((current) => [...current, entry])`, `:61`). Option B would cut React updates. Capping or virtualizing the list is separate UI work; do not fold it into this refactor.

### 5.4 Game vs trainer ordering in the console

- **Within a session:** order is guaranteed only if one adapter task drains that session's channel sequentially (§2.1).
- **Across sessions:** interleaving remains arbitrary. Under option A it stays line-granular, as today; under option B it becomes ≤33 ms blocks.
- **Future labelling:** the new `session_id` would let a later ConsoleView label or filter game vs trainer rows. That is out of scope here ("React unchanged").
- **Timestamps:** ConsoleView stamps **receipt** time (`new Date()`, `:56`), not log time. Under option A, lines in one batch get the same second-resolution timestamp, which is indistinguishable from today.

## 6. Recommendations for the plan

1. **Commands:** keep all six as thin `#[tauri::command]` wrappers in `src-tauri`, with identical names, args and return/error shapes. In particular, keep the `validate_launch` error as an unmodified `LaunchValidationIssue`.
2. **Event mapping:** map `LaunchEvent` to the four existing event names in a single per-session forwarding task.
   - Order: Log* → suppression summary → Diagnostic (conditional) → Exited → trainer `injection-log` finalization.
   - Still emit the pre-spawn trainer `injection-log` rows, including the spawn-failure row.
3. **Adding `session_id`:** use adapter-local DTOs. Never add it to `DiagnosticReport`, `LaunchValidationIssue` or any persisted model.
4. **Log mapping:** use option A (one `launch-log` per line), filter blank lines, and map `Dropped` to a visible line. Make the bench replay (`bench.rs:292`) emit the same `{line, session_id}` shape (fixed `bench` id) so G8 measures the real payload.
5. **Path matching:** keep `create_log_path` and `display_path_tilde` unchanged so `log_tail_path === helper_log_path` keeps matching (`useLaunchState.ts:271-277`).
6. **Same-PR frontend work:** update TS types (§4.2) and mocks (§4.3). Add the tests from §4.4:
   - ConsoleView and ConsoleDrawer object payloads;
   - `useLaunchState` diagnostic tolerance;
   - Rust DTO serialization.
7. **Checks for the main agent:**
   - `npm test` (ConsoleDrawer, AppShell, HeroDetailTrainerTab, useLaunchState, new ConsoleView test);
   - `npm run typecheck`;
   - Playwright `tests/pipeline.spec.ts` and `tests/smoke.spec.ts`;
   - `./scripts/check-mock-coverage.sh`;
   - `cargo test -p crosshook-native` (moved `log_relay` tests plus the new DTO tests).

## 7. Searches performed

- `launch-log|launch-diagnostic|launch-complete|injection-log` (repo-wide, `src/`, `src-tauri/`, `tests/`)
- `'(launch_game|launch_trainer|validate_launch|preview_launch|build_steam_launch_options_command|list_launch_history_for_profile)'` in `src/`
- `isDiagnosticReport|isInjectionLogEvent|LaunchResult|LaunchHistoryEntry` in `src/types/`
- `emitMockEvent|subscribeEvent` in tests
- `LogBatch|TailConfig|follow`
- `ts-rs`
- `exactOptionalPropertyTypes|strict` (tsconfig)
- `isStrictLaunchValidationIssue`
- `stream_helper_log|safe_read_tail`
- `CROSSHOOK_BENCH_LOG`
- `Stdio::piped|emit_injection_log_event|Ok(LaunchResult` (execution.rs)
