# YAN-824 / #551 — Binding decisions

Paths are relative to `src/crosshook-native/`. Research: `research-backend.md`, `research-frontend.md`, `research-risks.md`.

## D1 — Seq and publish live in the core registry lock (observer hook)

- `LaunchSessionRegistry` state becomes `Mutex<State { sessions: HashMap<..>, seq: u64 }>`.
- Every insert (`register`, `register_and_link_to_parent_of_kind`) and every **actual** removal bumps `seq` inside the same critical section and invokes an optional observer `Arc<dyn Fn(&SessionChange) + Send + Sync>` **while the lock is held**, so bus order equals seq order.
- Observer contract (documented): must not block, must not call back into the registry. The app observer only calls `EventBus::publish` (sync, non-blocking).
- `LaunchSessionRegistry::new()` stays observer-less (no churn in ~40 test call sites); add `with_observer(observer)`.
- `snapshot() -> (Vec<SessionInfo>, u64)` reads sessions and seq under the same lock.
- Why: covers every mutation path (finalizer `finish`, `Drop`, spawn failure, Steam watcher) without an app-side wrapper that callers could bypass; core stays free of `crosshook-app`.

## D2 — Exactly-once exit comes from the remove result

- `deregister(id)` keeps its signature for no-exit callers but delegates to `deregister_with_exit(id, code: Option<i32>, signal: Option<i32>) -> bool`, which returns `true` and emits `SessionChange::Exited` only when the entry was actually removed. Double deregister stays a silent no-op.
- `SessionGuard::finish` passes the real `code`/`signal`; `Drop` and spawn failure pass `None, None`. Spawn failure therefore emits Started then Exited (harmless, consistent with snapshots).
- Fix the stale doc comment at `registry.rs:52-53`.

## D3 — Steam game exit watcher (new backend code)

- Today `steam_applaunch` game sessions are deregistered when `steam-launch-helper.sh` exits (~30 s), while the game keeps running. The spec requires exit from watchdog detection.
- New cancel consumer for `kind == Game && method == steam_applaunch && !trainer_only`: a watcher that owns deregistration. The finalizer keeps `launch-complete`, cascade, hooks and portal steps unchanged but **hands off** its deregister step to the watcher. `Drop` still deregisters if the handoff never happened (explicit, tested).
- Watcher: a helper that exits 0 hands off with a 10 min appear window (shader pre-caching, updates); a signalled helper hands off with a 60 s window (the game may already have started); a non-zero exit code or no status deregisters at once with the real status. The watcher then polls every 2 s until the game is gone, then `deregister_with_exit(id, None, None)`. Never appeared → deregister at the window's end. `UserRequest` (reset) or closed cancel channel → deregister immediately. Pending cancel messages are drained before the finalizer builds its report, so a reset just before helper exit is recorded as `UserRequest`.
- Probe is injected and tri-state (`Option<bool>`; `None` = the host `ps` query failed, which neither counts as a miss nor resets the miss count). Production = core `probe_process_running` via `run_blocking`; `is_process_running` stays `bool` for its other callers. Three consecutive misses end the session. Tests use paused tokio time; no real sleeps.
- Never signals or kills anything. No cascade at real game exit (the existing cascade at helper exit is unchanged) — adding one would newly kill Flatpak Steam trainer trees.
- Idle wakeups stay 0: the watcher only exists while a Steam game session is registered.
- Known limit (documented): name-based match, same as the old UI poll.

## D4 — Wire contract (Tauri event + snapshot command)

App event (`crates/crosshook-app/src/events.rs`):

```rust
AppEvent::SessionStarted { session_id: SessionId, profile_name: Option<String>, kind: SessionKind, seq: u64 }
AppEvent::SessionExited  { session_id: SessionId, profile_name: Option<String>, kind: SessionKind, code: Option<i32>, signal: Option<i32>, seq: u64 }
```

(`kind` added to Exited so consumers can filter games without a lookup.)

- `profile_name` is the trimmed registry key, or `None` when the key is `ANONYMOUS_PROFILE_KEY`. Never send paths, PIDs, app ids or exe names.

Tauri event `launch-session-changed` (new `src-tauri/src/session_bridge.rs`, spawned from the existing bridge loop or alongside it), snake_case wire DTO:

```json
{"kind":"started","session_id":"<uuid>","profile_name":"Alpha"|null,"session_kind":"game"|"trainer","seq":3}
{"kind":"exited","session_id":"<uuid>","profile_name":"Alpha"|null,"session_kind":"game","code":0|null,"signal":null,"seq":4}
{"kind":"resync"}
```

- `resync` is emitted on bus `AppEvent::Resync`; the UI re-fetches the snapshot and **replaces** state.

Snapshot command `list_running_sessions` (replaces `list_running_profiles`, which is removed with its mock — clean cutover):

```json
{"sessions":[{"session_id":"<uuid>","profile_name":"Alpha"|null,"session_kind":"game"}],"seq":4}
```

- `LaunchResult` gains `session_id: string` (additive) so `useLaunchState` correlates its own session exactly, including unsaved/anonymous launches.

## D5 — Classic frontend: one module-level store, zero timers

- New `src/hooks/runningSessionsStore.ts` (module singleton, ref-counted): first consumer **awaits** `subscribeEvent('launch-session-changed')`, then calls `list_running_sessions`; events received before the snapshot resolves are buffered and applied only if `seq > snapshot.seq`; afterwards events with `seq <= snapshot.seq` are dropped; `resync` re-fetches and replaces; a failed snapshot keeps the previous state, a failed subscription leaves the store unsubscribed, and both are retried by event-driven triggers only (no timers): the next session event (snapshot), window `focus` / document `visibilitychange` to visible, and another consumer subscribing. The DOM listeners exist only while the store has consumers. The store also remembers a bounded set of session ids seen in `exited` events; last consumer unsubscribes and clears it.
- Exposed via `useSyncExternalStore`. `useRunningProfiles()` keeps its name/shape (`Set` of game profile names, anonymous and trainer sessions excluded), so AppShell/LibraryPage do not change and no render harness needs a provider.
- `useLaunchState`: delete the `check_game_running` interval. `isGameRunning` = the store contains the launch's own game `session_id` (from `LaunchResult`) **or** any game session for the current trimmed profile name (prevents a double launch after profile switch, reset or reload). `game-stopped` dispatches when the tracked session (or, untracked, the profile's game session) exits after being observed, or when the store recorded an `exited` event for the own `session_id` before `launch_game` resolved. Absence from the store alone is never an exit; the launch generation guard keeps an earlier launch's exit from ending a replacement launch. `check_game_running` stays as a Tauri command and mock for on-demand use only.
- Accepted behavior change: a game started outside CrossHook is no longer detected by a 3 s poll (follow-up issue).

## D6 — Mocks

- `_mock_set_profile_running`, mock `launch_game` and the mock completion timer emit `launch-session-changed` with increasing seq; mock `list_running_sessions` returns the mock state + seq under every fixture except `loading` (`empty` included, since its seq covers the live mock sessions). Put session mock logic in a new sibling handler file (`launch.ts` is already 846 lines).
- `check-mock-coverage.sh` covers commands only; adding `list_running_sessions` and removing `list_running_profiles` keeps it green. Event coverage belongs to YAN-812 (M1-56).

## D7 — Tests (critical only)

- Core: remove-result exactly-once; snapshot under lock; observer invoked in seq order.
- App: two subscribers both see Started/Exited; exactly one Exited across finish + drop; spawn failure; deterministic snapshot race (falsifiable — verify by temporarily breaking it); threaded stress invariant; lag → resync; Steam watcher (appear/disappear, timeout, reset, no kill) with paused time.
- Tauri: payload JSON shape + event name; resync payload.
- Vitest: subscribe-before-snapshot order, seq dedupe, resync replace, one subscription for two consumers, anonymous/trainer excluded, `vi.getTimerCount() === 0`, `useLaunchState` `game-stopped` on own session exit.
- Playwright Currently Playing smoke keeps passing via emitted mock events.
