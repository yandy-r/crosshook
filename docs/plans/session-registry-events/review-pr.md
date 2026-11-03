# YAN-824 / PR #554 — whole-PR review (post-fix)

Scope: `git diff origin/main...HEAD` at `119092f` (62 files). Paths are relative to `src/crosshook-native/` unless they start at the repo root. This pass looked for new defects and incomplete fixes after `review-rust.md` and `review-frontend.md`.

Verdict: **1 medium, 2 low (docs).** The post-review fixes are correct. I found no exactly-once, ordering, retry-storm or mock-leak defect.

## Findings

### M1 — One negative probe permanently ends a running Steam game session (no debounce)

- **Where:** `crates/crosshook-app/src/services/launch/steam_watch.rs:172-177` (the disappear loop returns on the first `!probe(watch).await`). The probe is `crosshook_core::launch::is_process_running` (`crates/crosshook-core/src/launch/watchdog/mod.rs:38-71`). Under Flatpak it depends on `host::probe_host_processes`, which returns "no match" when `flatpak-spawn --host ps` fails to start or exits non-zero (`watchdog/host.rs:155-164`). The sandbox `/proc` cannot see host Steam games because the PID namespace is unshared, so the host probe is the only signal.
- **Scenario:**
  1. A Flatpak user launches a Steam game. The helper exits 0, the watch hands off, the game appears, and polling starts.
  2. One `flatpak-spawn --host ps` call fails transiently (portal/D-Bus hiccup or timeout). The same happens on native when the game briefly re-execs itself: some launchers and settings-apply restarts leave a gap of under 2 s. Either way, `is_process_running` returns `false` once.
  3. The watch returns and calls `deregister_with_exit(id, None, None)`. `SessionExited` is published and the session is gone for good.
  4. In the UI, `useLaunchState` fires `game-stopped` because `trackedGameRunning` went false after being observed (`hooks/useLaunchState.ts:248-261`). `isGameRunning` becomes false, so `canLaunchGame` flips to true while the game runs. The "Currently Playing" badge drops the profile. Nothing recovers until the next launch.
- **Why this is a regression:** the old 3 s UI poll also reacted to a single miss, but the next tick corrected it: `isGameRunning` returned to true, Launch Game was disabled again, and "Game process detected" came back. The registry session is now the only source of truth and cannot come back after a false negative. D3 calls the probe name-based but does not accept one-shot false negatives.
- **Fix:** require several consecutive misses before ending, for example 3 polls (about 6 s at `POLL_INTERVAL`), and reset the counter on any hit. Add a paused-time test in which the scripted probe answers `[true, false, true, true, false, false, false]` and the session survives the single `false`. Check that the test fails when the threshold is 1. A better fix needs a core probe that can tell "probe failed" from "not running", but that is a larger change and debouncing alone covers both triggers.

### L1 — Docs say a reset ends a Steam session "at once"; during the helper phase it does not

- **Where:**
  - `crates/crosshook-app/README.md` § Steam game watch (about line 260): "a reset or closed cancel channel deregisters at once".
  - `steam_watch.rs:18-19`: "A cancel (the user reset) or a closed cancel channel deregisters immediately."
- **Actual behaviour (`steam_watch.rs` phase 1):**
  - A reset that arrives while `steam-launch-helper.sh` is still running (its 30 s startup delay, `runtime-helpers/steam-launch-helper.sh:13`) only records the reason.
  - The session is deregistered when the helper exits: at the handoff, or by the finalizer if the helper fails.
  - Until then the profile match keeps `isGameRunning` true, so after Reset the Launch Game button shows "Game Running", disabled, for up to about 30 s.
  - The behaviour is reasonable; the docs and module comment are what's wrong. The `review-rust.md` "checked" note ("A reset in phase 1 deregisters at handoff") describes it correctly.
- **Two smaller inaccuracies in the same area:**
  - The README "Running-session events" bullet (about line 247) says "finish passes the child's status". For a Steam game whose helper exits 0, finish hands off, and the watch later publishes `SessionExited` with no status.
  - `events.rs:128-129` lists the `None`/`None` cases but omits a `start_game` future dropped before the guard exists, which the `RegistrationGuard` now covers.
- **Fix:** reword the README and module doc to say "a reset deregisters at once after the helper has exited; during the helper phase it deregisters when the helper exits". Add the Steam-watch and cancelled-start cases to the two status lists.

### L2 — The user feature doc still describes UI polling for the game process

- **Where:** `docs/features/steam-proton-trainer-launch.doc.md:218` (repo root): "CrossHook polls for the game process and transitions to the 'Launch Trainer' state once the game is detected."
- **Why it is wrong:**
  - The UI no longer polls.
  - The phase becomes "waiting for trainer" as soon as `launch_game` returns.
  - The running state now comes from the session registry, and for Steam it ends when the game exits rather than when the helper exits.
  - The sentence was already imprecise and this PR makes it inaccurate.
- **Fix:** say that CrossHook moves to the Launch Trainer step once the launch starts and tracks the game session until the game exits. Optionally mention that Steam sessions follow the game process by executable name.

## Verified, no defect

- **Helper success-only handoff (`finalizer.rs:241-264`):** the handoff is attempted only when `code == Some(0) && signal.is_none()`. In every other case the sender is dropped and the session is deregistered with the real status. The watch sees `handed.is_err()` and returns without probing. `deregistered = true` is set first, so the following `Drop` is a no-op. A refused `send` falls back to deregistering with the real status. Tests cover a failed helper, a refused handoff and a failed Steam helper (`tests/steam_watch.rs:277-350`). The probe asserts the exe name in both the unit and the service paths (`:46`, `:388`), so the M1 issue from `review-rust.md` is closed.
- **`RegistrationGuard` (`run.rs:81-85, 177, 344-367`):**
  - The guard is armed right after `register_session`. The only return after that is `fail_spawn`, which calls `deregister_with_exit` first. The guard's later `deregister` hits a missing entry, returns `false` and emits nothing, so there is no double `SessionExited`.
  - `disarm()` runs synchronously right before `spawn_session_task`, which builds `SessionGuard` before `tokio::spawn`. There is no `.await` between them, so there is no window where neither guard owns the session.
  - The Steam `handoff_rx` stays alive on the stack until the watch is spawned, again with no await, so the finalizer's `send` cannot be refused there by accident.
  - The cancellation test at `tests/sessions.rs:247-307` exercises the history-row await point.
- **Steam watch 10-minute timeout:** `APPEAR_TIMEOUT = 10 * 60` s. Tests use the constant, and the README, module doc and test arithmetic agree. No `90 s` reference to the watch remains outside the historical `review-*.md` files.
- **`session_payload` in `startup_bridge.rs:175-191`:** it runs for every event that passes `is_bridged`, before the per-event `match`, so no match arm can drop a session emission. `is_bridged` excludes only the three scan-completion events, and a test asserts that `SessionStarted` stays bridged. A bus `Resync` emits both `{"kind":"resync"}` and the existing startup/profile re-emits. The test at `startup_bridge/tests.rs:254` maps started, exited and resync to payloads and maps unrelated events to `None`.
- **`useLaunchState` profile-match fallback (`hooks/useLaunchState.ts:162-173, 239-261`):**
  - **Anonymous sessions:** they never match because `hasProfileGameSession` rejects an empty name, so they rely on the own id.
  - **Tracked id:** while an id is tracked, `game-stopped` follows only that id. A second session for the same profile cannot cause a false stop.
  - **Untracked id:** after a reset or profile switch, the phase is Idle and the effect returns early. A non-Idle phase can only be reached again through `launchTrainer`, and the test at `useLaunchState.events.test.ts:283` covers that path. `game-stopped` then fires only when the last game session for the profile leaves.
  - **Launch in flight:** with a saved profile, the session's own `started` event can match during `GameLaunching`. That sets `observed`, which also closes `review-frontend.md` #4: a game that exits before `LaunchResult` now triggers `game-stopped` once the result moves the phase to WaitingForTrainer or SessionActive.
  - **Stuck states:** none found. Every non-Idle phase either has a tracked id whose exit fires `game-stopped`, or can be cleared with the Reset button, which is shown for every non-Idle phase.
- **`runningSessionsStore` retry (`hooks/runningSessionsStore.ts:96-145`):**
  - **Bounded retries:** `snapshotRetryPending` is cleared at the start of each request. After a failure, at most one retry fetch is in flight, and events that arrive during it are buffered without starting another fetch.
  - **Concurrent fetches:** resync and malformed payloads start a new fetch with a new `fetchToken`, and older results are discarded.
  - **Teardown:** `lifecycle` drops late results from a torn-down store.
  - **Buffered events:** `buffered` is filtered by `seq > snapshot.seq` and cleared only when a snapshot succeeds.
  - **Load:** the remaining unbounded case is a stream of malformed payloads (one fetch each), which would require a backend bug. Events are rare (one per launch start or exit), so this has no practical load.
- **Mock game sessions persisting (`lib/mocks/handlers/launchSessions.ts`, `launch.ts:316-326`):**
  - Game sessions survive helper completion. They end through `launch_reset_sessions`, `_mock_set_profile_running(false)` or `resetLaunchSessionsMockState`.
  - Vitest calls `resetMockEnvironment()` from `createHandlerMap` and from the `afterEach` → `resetMockHandlers`. That resets the mock registry (`seq` and the id counter) and the browser event bus. RTL `cleanup()` runs first and unmounts consumers, so the store's `stop()` clears its singleton state. Pending launch timers are cleared by `resetLaunchMockState`.
  - Each Playwright test gets a fresh page, so no module state carries over.
  - `resetMockEnvironment` has no non-test caller, so a mock `seq` reset can never fall below a live store's baseline.
- **Wire and privacy:** the DTOs carry only `session_id`, `profile_name` (the trimmed key, `null` for anonymous), `session_kind`, `code`, `signal` and `seq`. `LaunchResult.session_id` is additive. The watch logs no exe name.
- **Repo rules:**
  - **Host gateway:** the probe reaches the host only through core `host_std_command`, and the new code adds no `Command::new`.
  - **Persistence:** runtime-only state with no TOML or SQLite change, classified in `research-risks.md`.
  - **IPC naming:** snake_case Tauri command and event names.
  - **File sizes:** new files are 98–452 lines. `tests/helpers.rs` grew to 497. `useLaunchState.ts` (619, +1) and `mocks/handlers/launch.ts` (814, −32) were already over the cap before this PR.
- **Doc counts and stale references:**
  - **`lib/mocks/README.md`:** 169 `map.set` entries − 6 `_mock_*` helpers = 163 commands across 22 `register*` calls, which matches the code. The `launchSessions.ts` description is accurate.
  - **`crates/crosshook-app/README.md`:** the `LaunchService` and `EventBus` rows are accurate apart from L1.
  - **`list_running_profiles`, `running_profiles`, `active_profile_keys`:** none remain in code, AGENTS.md, CLAUDE.md, `.ai/rules` or non-historical docs. They appear only in `docs/plans/`, the completed `docs/prps/` plans and reports, and the historical research snapshot `docs/research/native-ui-migration/README.md:103,117`, which describes the pre-migration state.
