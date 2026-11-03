# YAN-824 Rust review (adversarial)

Scope: core `launch/session/*`, app `services/launch/*` (sessions, steam_watch, finalizer, run), `events.rs`, `bootstrap/stores.rs`, src-tauri `session_bridge`, `startup_bridge`, `commands/launch`. Paths are relative to `src/crosshook-native/`.

Verdict: **1 high, 1 medium, 4 low.** I found no deadlock, ordering or exactly-once defect in the core registry or the observer path.

## Findings

### H1 — A Steam game whose helper fails still hands off, so a dead launch shows as running for 90 s

- **Where:** `crates/crosshook-app/src/services/launch/finalizer.rs:242-254` (`deregister` always tries the handoff first), `run.rs:137` (the watcher is chosen before the exit status is known), `steam_watch.rs:128-141`.
- **What happens:** `SessionGuard::deregister(code, signal)` sends the handoff whatever the helper's status was. Some example failures:
  - `steam-launch-helper.sh` reaches `fail` and exits 1 within milliseconds, for example "Could not find a Steam CLI launch command", "Failed to resolve Proton path", or "Unknown trainer loading mode".
  - The helper is killed by a signal.
  - `wait` fails (`status == None`).

  In each case the watcher still takes over and probes for `Game.exe` for the full `APPEAR_TIMEOUT` (90 s) before it deregisters.

- **What the user sees:** the store keeps the session for 90 s after `launch-complete` has already reported a failure diagnostic.
  - `useLaunchState.isGameRunning` stays true, so `canLaunchGame` is false and the user cannot retry for 90 s.
  - The status text reads "Game process detected…".
  - Library "Currently Playing" lists the profile.
  - If a same-named process happens to be running, the session stays until that process exits.
  - The resulting `SessionExited` carries `code: None` even though the real code (1) was known.
- **Answer to the review question:** yes, the watcher still starts after a non-zero exit, and that is the bug.
- **Fix:** hand off only when the helper exited successfully (`code == Some(0)`, no signal). Otherwise call `deregister_with_exit(id, code, signal)` directly. Dropping the `Sender` ends the watcher, because `handed.is_err()` makes it return. Something like:

  ```rust
  let handoff = self.ctx.deregister_handoff.take();
  if code == Some(0) && signal.is_none() {
      if let Some(h) = handoff { if h.send(()).is_ok() { return; } }
  }
  self.ctx.registry.deregister_with_exit(self.ctx.session_id, code, signal);
  ```

  Add a test: a Steam game whose `FakeSpawner` child exits 1 publishes `SessionExited { code: Some(1) }` right after `Exited`, and the probe is never called.

### M1 — The service-level Steam test's probe ignores its argument, so the exe name used in production is never checked

- **Where:** `crates/crosshook-app/src/services/launch/tests/steam_watch.rs:267` (`with_probe(Arc::new(move |_exe: String| …))`).
- **Why it matters:** this is the only test that goes through the real `run.rs` path (`steam_game_exe` → `SteamWatch.exe_name` → probe), and its probe always returns `true` for any name. If `run.rs` passed an empty string, the full `game_path`, or the trainer's name, the test would still pass. In production that bug would mean every Steam session ends at the 90 s timeout, or never ends. The unit tests do assert `"Game.exe"`, but they build `SteamWatch` by hand and skip `run.rs`. This is the same class of bug as the YAN-822 `FakeRoots` issue.
- **Fix:** assert inside the probe: `assert_eq!(exe, harness.steam_request("steam-watch").game_executable_name())`, or send `exe` over the channel and compare it after `recv`. Check the fix by temporarily passing `String::new()` in `run.rs:170` and confirming the test fails.

### L1 — The fallback when the handoff is refused has no test

- **Where:** `finalizer.rs:248-254`.
- **Gap:** no test covers `finish` with `deregister_handoff: Some(tx)` whose receiver is already gone. That is the "watch already finished" fallback, which should deregister with the real code/signal. If a refactor dropped that fallback (`return` regardless of the `send` result), the session would leak with no `SessionExited`, and nothing would catch it.
- **Fix:** add a guard test: drop `handoff_rx`, call `finish(Some(exit_code(0)))`, and assert exactly one `SessionExited { code: Some(0) }`. If H1 is fixed, this test also pins the success-only path.

### L2 — The `startup_bridge` dispatch arms for session events are not falsifiable

- **Where:** `src-tauri/src/startup_bridge.rs:184-202`. Tests: `startup_bridge/tests.rs:235-244` and `session_bridge/tests.rs`.
- **Gap:**
  - The new test only asserts that `is_bridged(SessionStarted)` is true.
  - `session_bridge` tests check payload mapping only.
  - Deleting the `AppEvent::SessionStarted { .. } | AppEvent::SessionExited { .. } => session_bridge::emit(..)` arm would leave every test green, because events would silently fall into `_ => {}`.
  - The same goes for the `session_bridge::emit` call in the `Resync` arm. Without it the UI never re-snapshots after a lag.
- **Fix:** pull the arm logic into a pure helper, e.g. `fn session_payload_for(event) -> Option<LaunchSessionChanged>` that the loop calls for every event (it already returns `None` for unrelated events). Then test that `SessionStarted`, `SessionExited` and `Resync` all yield `Some`. Calling `session_bridge::emit(&handle, &event)` unconditionally before the `match` would make a dropped arm impossible.

### L3 — Pre-existing: cancelling `start_game` after spawn leaks a registered session with no `SessionExited`

- **Where:** `run.rs:76-160`. The session is registered at line ~77, but `SessionGuard` is only created in `spawn_session_task` (line ~163). In between, `record_launch_started(...).await` is a cancellation point.
- **Scenario:** if the caller's future is dropped at that await, the guard never exists. Callers are the Tauri async command or a future CLI/TUI presenter with a timeout. The registry entry then stays forever, there is no `SessionExited`, and Currently Playing / `isGameRunning` stay stuck until a reset. This was already true for `list_running_profiles`. It matters more now that events claim "exactly once per session" (`events.rs:127`).
- **Fix (separate issue is fine):** build the `SessionGuard`, or a small drop-guard that deregisters, right after registration. Or move `record_launch_started` before `register_session`.

### L4 — `tests/finalizer.rs` is over the ~500-line cap and grew further

- **Where:** `crates/crosshook-app/src/services/launch/tests/finalizer.rs`, 583 lines (was 578).
- **Detail:** CLAUDE.md treats ~500 lines as a soft cap that needs a refactor once a file drifts meaningfully past it. This change only touches visibility and adds `deregister_handoff: None`, so it is minor. The shared `LaunchCase` / `session_context` / `exit_code` helpers it now exports to `sessions.rs` and `steam_watch.rs` belong in `tests/helpers.rs`, which would also bring the file back toward the cap.

## Checked, no defect

- **Lock ordering and deadlock:**
  - `record` calls the observer under the registry `Mutex` (`registry.rs:69-75`).
  - The only production observer is `session_observer` → `EventBus::publish` → `broadcast::Sender::send`. That send takes only tokio's internal tail lock and schedules wakers. It never blocks and never touches the registry.
  - No code holds the bus lock and then calls into the registry.
  - The startup bridge consumes on its own task, and its `emit` happens outside the registry lock.
  - `cancel_linked_children` and `cancel_session` still send with the lock released.
  - An observer panic poisons the registry, which matches the existing poison policy. The drop paths wrap registry calls in `catch_registry`.
- **Exactly-once `Exited`:**
  - `deregister_with_exit` emits only on an actual `HashMap::remove` (`registry.rs:110-125`).
  - `finish` sets `deregistered = true` before the handoff or remove, so the following `Drop` is a no-op.
  - When `Drop` runs without a prior `finish`, it removes directly and never hands off. It then drops the `Sender`, so the watcher sees `handed.is_err()` and returns. A `Closed` cancel channel also returns.
  - `fail_spawn` runs before any handoff exists. It emits Started then Exited (None, None).
  - A double deregister is a silent no-op that does not bump `seq`. Core tests cover this.
- **Watcher lifecycle:**
  - The watcher ends on handoff drop, cancel, `Closed`, the 90 s timeout, or game exit.
  - A reset in phase 1 deregisters at handoff without probing. A reset in phase 2 wins the `select!` immediately.
  - The production probe maps a `spawn_blocking` panic to "not visible", so it cannot panic the watcher.
  - The watcher is not tied to `App::shutdown` (plain `tokio::spawn`). That is harmless: the registry is in-memory, and Tauri tears down the process. In a borrowed-runtime host it would keep probing until the game exits, but no such host uses `LaunchService` today.
  - Phase-1 lag recovery mirrors `drain_cancel_into_outcome`.
  - The paused-time tests match the loop arithmetic (probe at t=0…88 keeps the session; t=90 deregisters).
- **Seq and snapshot linearizability:**
  - `seq` is bumped and published in the same critical section as the mutation. `snapshot()` reads sessions and `seq` under one lock.
  - Events published before the snapshot but emitted to the webview afterwards have `seq <= snapshot.seq` and are dropped by the store.
  - Events published after the snapshot have a greater `seq` and are applied.
  - The parked-observer race test is falsifiable: it gates on the observer holding the lock and asserts that neither a writer nor a reader completes. The threaded stress tests check gap-free order and snapshot+tail = final state.
- **Resync:** a bus `Lagged` becomes `AppEvent::Resync`. The bridge emits `{"kind":"resync"}` alongside the existing startup/profile re-emits, and the retained tail that follows is filtered by `seq`.
- **Payload privacy:**
  - The DTOs carry only `session_id`, `profile_name` (the trimmed key, `None` for `ANONYMOUS_PROFILE_KEY`), `session_kind`, `code`, `signal` and `seq`.
  - No path, PID, app id or exe name is included.
  - The watcher logs only `session_id` and flags, never `exe_name`.
  - `LaunchResult.session_id` adds nothing sensitive.
- **Clean cutover:** `active_profile_keys`, `running_profiles` and `list_running_profiles` are gone from code, IPC registration and the contract test. They remain only in historical `docs/plans`. The CLI does not construct `LaunchService`, so the new `EventBus` argument does not affect it.
