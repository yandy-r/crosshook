# YAN-824 frontend review (adversarial)

Scope: the uncommitted TypeScript changes under `src/crosshook-native/src` (store, hooks, parser, types, mocks, tests, `tests/smoke.spec.ts`), checked against `src-tauri/src/session_bridge.rs` and `decisions.md`.

## Checks I ran

- `npx tsc --noEmit -p .`: clean.
- `npx vitest run`: 117 files and 988 tests pass. The 5 suites this change touches pass on their own too.
- `scripts/check-mock-coverage.sh`: 0 missing and 0 orphaned handlers.
- `npx vite build` into `/tmp`, then the release `verify:no-mocks` grep, plus `mock-game-session`, `mock-seeded-game-session` and `_mock_set_profile_running`: no hits. The new `handlers/launchSessions.ts` is only reachable through `lib/mocks/index.ts`, which is the dynamic import behind `__WEB_DEV_MODE__`.
- Playwright: the Currently Playing smoke, the hero card-switch launch and console-chrome launch tests, and the pipeline tooltip test all pass. The tooltip test also passed 16 out of 16 runs (`--repeat-each=16 --workers=8`).

## Things I checked and found correct

- **Wire contract.** It matches `session_bridge.rs`. The `#[serde(tag = "kind", rename_all = "snake_case")]` variants become `started`, `exited` and `resync`. Field names are `session_id`, `profile_name`, `session_kind` (`game`/`trainer`), `code`, `signal` (`Option<i32>`, so an integer or `null`) and `seq` (`u64`, safe as a JS number). The snapshot is `{sessions:[{session_id, profile_name, session_kind}], seq}`. `LaunchResult.session_id` matches `shared.rs`.
- **Parser.** A missing `profile_name` (as opposed to `null`), an unknown `session_kind`, a non-integer or negative `seq`, or a bad `code`/`signal` all return `null`, and the store treats that as a resync. A snapshot with any bad entry is rejected as a whole.
- **Subscribe before snapshot.** `start()` awaits `subscribeEvent` before `requestSnapshot()`. `baselineSeq` is `null` from the start of `start()`, so every event that arrives in that window is buffered. Buffered events are applied only when `seq > snapshot.seq`. After that, events with `seq <= baselineSeq` are dropped.
- **Resync while a snapshot is in flight.** `fetchToken` drops the older response. The buffer is kept and filtered against the newer snapshot's seq, which is correct because that snapshot is the later one.
- **Teardown or remount during a pending subscribe or snapshot.** `stop()` bumps `lifecycle` and `fetchToken`.
  - A stale subscribe that resolves later sees `owner !== lifecycle` and unsubscribes.
  - Callbacks from that stale subscription are ignored by the `owner === lifecycle` check.
  - A stale snapshot is discarded.
  - The StrictMode cycle (setup, cleanup, setup) goes refCount 3 → 0 → 3 and leaves exactly one live subscription.
  - I found no unsubscribe leak.
- **`useLaunchState` flows.** Relaunch clears and then sets the session id. Reset bumps the generation, so a late `launch_game` cannot pin a session id. Trainer-only launches keep the game's session id. The `steam_applaunch` game session now ends when the game exits, so `game-stopped` fires when the game actually exits instead of at helper exit (~30 s).

## Findings

### 1. HIGH — `isGameRunning` is lost on profile switch or reset, which re-enables Launch Game while CrossHook's own game is still running (double launch)

- **Where:** `src/hooks/useLaunchState.ts:150-152` (`isGameRunning = gameSessionId !== null && runningSessions.has(gameSessionId)`), together with the `profileId` reset effect at `:170-180` (`setGameSessionId(null)` at `:175`) and `reset()` at `:471`. Effect at `:562` (`canLaunchGame … && !isGameRunning`).
- **Scenario:**
  1. Launch profile A's game from the hero detail.
  2. Click another library card. `LibraryPage.handleLaunch` calls `selectProfile(B)`, so `profileId` changes and `gameSessionId` becomes `null`.
  3. Go back to A. `isGameRunning` is now `false` and `canLaunchGame` is `true`, while A's game session is still in the store. The sidebar even counts it under "Currently Playing" through `useRunningProfiles`.
  4. The user can press Launch Game again. I found no "already running" guard in the backend (`registry.register` accepts a second Game session per profile). For `proton_run` this starts a second instance in the same prefix.

  The same happens after `resetLaunchSession` when the cancel consumer does not end the game, and after a webview reload.

- **Before:** the 3 s `check_game_running` poll on `request.game_path` kept `isGameRunning` true. That disabled Launch Game and showed "Game process detected. Launch Trainer when ready."
- **Why it is not covered by the design:** D5 only accepts losing detection of games started _outside_ CrossHook. Here CrossHook started the game and the store already knows about it.
- **Side effect:** the `isGameRunning && phase === Idle` branches at `:503` and `:537` ("The game is already running. You can launch the trainer directly.") can no longer be reached. `gameSessionId` is only non-null after `game-success`, which sets a non-Idle phase, and every path back to Idle either clears it or happens because the session left the store.
- **Fix:** when `gameSessionId` is `null` or no longer in the store, fall back to any running session with `session_kind === 'game'` and `profile_name === profileName.trim()` (saved profiles only). Either use it for `isGameRunning` directly or adopt its id with `setGameSessionId`. This uses data the store already holds, adds no timers, and keeps the own-session match for anonymous launches. Add a test: launch, change `profileId`, return, and assert `isGameRunning === true` and `canLaunchGame === false`. If D5 has to stay literal, this needs Main's decision, because D5 as written brings back the double-launch risk the poll prevented.

### 2. LOW — A failed snapshot clears state and is never retried; with an active launch this fires a false `game-stopped`

- **Where:** `src/hooks/runningSessionsStore.ts:99-114` (`.catch(() => null)` → `EMPTY_SESSIONS`, `baselineSeq = -1`) and `:146-151` (subscription failure). The effect in `useLaunchState.ts:225-237` then sees `isGameRunning` go false.
- **Scenario:**
  1. During WaitingForTrainer or SessionActive, a malformed payload or a bus-lag resync triggers `requestSnapshot()`.
  2. The IPC call rejects. The store commits an empty map.
  3. `observedGameProcessRef` is true, so `game-stopped` sends the UI to Idle while the game keeps running.
  4. Nothing ever retries. AppShell keeps refCount > 0, so the store stays empty or partial until the next bus lag, and the "Currently Playing" badge stays wrong for the whole session.

  The old hook recovered within 3 s. Likelihood is low because `list_running_sessions` is infallible on the Rust side (`queries.rs:88`), so only IPC transport errors reach this path. Clearing on failure itself follows D5.

- **Fix (cheap, no timers):** on snapshot failure, keep `baselineSeq = null` so events stay buffered, and retry the snapshot on the next incoming event. Alternatively, keep the previous `sessions` instead of committing `EMPTY_SESSIONS`. If clearing has to stay, at least make `useLaunchState` ignore a store reset caused by a snapshot failure (for example, expose a `ready`/`error` flag and skip `game-stopped` unless an `exited` event or a successful snapshot dropped the id).

### 3. LOW — Browser-dev mock ends the _game_ session at helper completion (~1.7 s); the waiting-for-trainer state collapses and the pipeline tooltip test is latently racy

- **Where:** `src/lib/mocks/handlers/launch.ts:316-319` (`endMockSession(sessionId, 0, null)` inside the shared `scheduleLaunchLogSequence` completion timer), used for game sessions at `:448-449`.
- **Scenario:**
  1. In `dev:browser`, Launch Game moves to WaitingForTrainer, and the store sees the mock session start.
  2. At 150·8 + 300 + 200 = 1700 ms the completion timer emits `exited`. `useLaunchState` dispatches `game-stopped` and the phase returns to Idle.
  3. Before this change the mock `check_game_running` returned false, so the waiting state persisted and developers could exercise Launch Trainer.
  4. This also contradicts D3's real behavior: a `steam_applaunch` game session outlives helper completion.

  `tests/pipeline.spec.ts:76-91` clicks Launch Game and then hovers the trainer node's "Waiting" trigger. That trigger disappears when the phase returns to Idle (`preview` is `null`, so no other node has a detail). Today the hover plus the 200 ms tooltip delay finish well inside 1.7 s (16 of 16 runs passed), but on a slow CI runner it can flake.

- **Fix:** keep mock **game** sessions registered after the completion timer (still emit `launch-complete`). End them via `launch_reset_sessions`, `_mock_set_profile_running(false)` or `resetLaunchSessionsMockState`, which mirrors the D3 watcher. Trainer sessions can keep ending on the timer. Update the `launch.session.test.ts` expectation (`changes[1]` exited at seq 2) to match, for example by asserting the exit after `launch_reset_sessions`.

### 4. LOW — A game session that ends before `LaunchResult` is applied leaves the phase stuck

- **Where:** `src/hooks/useLaunchState.ts:225-237` with `:312`.
- **Scenario:**
  1. A native or `proton_run` game crashes immediately. `exited` is applied to the store before `launch_game` resolves.
  2. `setGameSessionId(id)` runs, but `isGameRunning` is never true, so `observedGameProcessRef` never flips and `game-stopped` never fires.
  3. The phase stays at WaitingForTrainer (two-step) or SessionActive (native) until the user resets.

  The 3 s poll had the same blind spot for games that exited in under 3 s, so this is not a regression. But the hook now has exact data and still ignores it.

- **Fix:** after `game-success`, if the returned `session_id` is not in the store and the store's baseline seq is past the session's start, dispatch `game-stopped` (or expose a "seen exited ids" set from the store). Optional.

## Non-findings / notes for Main

- **File size and shape.** `launch.ts` mock is 808 lines (846 before) and `useLaunchState.ts` is 596 (618 before). Both were already over the ~500-line cap, and this change shrinks both. The new files are small: store 204, mock registry 109.
- **Test fixture.** `LaunchSubTabs.test.tsx:378` still returns a `launch_game` result without `session_id` (an older shape with fields `LaunchResult` doesn't have). `has(undefined)` is false, so it is harmless.
- **Missing store test.** No Vitest case covers teardown/remount while the subscribe or snapshot is pending. The code is correct (see above), but a falsifiable test would lock in the `lifecycle`/`fetchToken` guards.
- **Steam watcher (backend, for ReviewRust).** `APPEAR_TIMEOUT` (90 s after helper exit) ends the session if the game is not yet visible, for example during Steam shader pre-caching or a game update. On the frontend that drops the "Currently Playing" badge and fires `game-stopped` while the game later runs. The old UI poll would have picked the game up whenever it appeared.
