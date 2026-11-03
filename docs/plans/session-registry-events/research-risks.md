# YAN-824 / #551 — Risks, correctness and test strategy

Paths are relative to `src/crosshook-native/` unless they start with `scripts/` or `docs/`.
Scope is read-only research. Issue spec source: Linear YAN-824 (fetched 2026-11-02).

## TL;DR

1. **Assign the seq in the same critical section as the registry mutation.** Publish it there too. Then any order of subscribe → snapshot → event is consistent. If the seq is assigned outside the lock, the snapshot race test fails (events are lost or doubled).
2. **`deregister` must report whether it removed anything** (`Option<SessionEntry>` or `bool`). Publish `SessionExited` only on an actual removal. Today it returns `()` (`crates/crosshook-core/src/launch/session/registry.rs:54-57`). Only three production call sites exist: `finalizer.rs:240`, `finalizer.rs:302` (drop path) and `run.rs:225` (spawn failure). **No watchdog calls `deregister` today.** The doc comment at `registry.rs:52-53` is stale.
3. **No backend game-exit detection exists for `steam_applaunch`.** For a game launch the child is `steam-launch-helper.sh`. It starts `steam -applaunch` in the background, sleeps 30 s, then exits 0 (`runtime-helpers/steam-launch-helper.sh:331-339,355-357`; `crates/crosshook-core/src/launch/script_runner/steam_helpers.rs:21`). The finalizer then deregisters the session while the game is still running. Steam sessions always get a `Drain` cancel consumer, never a watchdog (`crates/crosshook-app/src/services/launch/gamescope.rs:54-57,122-124`). Today only the frontend detects real game exit, through its 3 s `check_game_running` poll (`src/hooks/useLaunchState.ts:218-262`). The spec's "watchdog exit detection for steam_applaunch games" is **new backend code**.
4. **Anonymous sessions:** `list_running_profiles` currently leaks the sentinel `__crosshook_anonymous_profile__` to the webview (`crates/crosshook-app/src/services/launch/tests/queries.rs:181-202`). Events should carry `profile_name: Option<String>`, with `None` for anonymous. Classic should exclude anonymous sessions from Currently Playing.
5. **Lag recovery:** `EventReceiver::recv` turns `Lagged` into `AppEvent::Resync` (`crates/crosshook-app/src/events.rs:170-175`). The Tauri bridge must turn that into a session resync, and the UI must re-take the snapshot and replace its state.

---

## 1. Race conditions

### 1.1 Register/deregister vs snapshot (seq design)

- The registry is `Mutex<HashMap<SessionId, SessionEntry>>`, and every operation takes one short lock (`registry.rs:27-29,47,55`). It knows nothing about events. Core has no `EventBus`, which lives in `crosshook-app` (`events.rs:117-151`).
- **Required invariant:** a mutation (insert or remove) and its `seq` assignment are atomic with respect to `running_sessions_snapshot()`. The snapshot reads the session list and the current seq under the same lock.
- Recommended shape: an app-side tracker owned by `LaunchService`, for example `SessionEvents { registry: Arc<LaunchSessionRegistry>, bus: EventBus, seq: Mutex<u64> }`.
  - `register`, `deregister` and `snapshot` take `seq` first, then call the registry, then `bus.publish(..)`, then release.
  - `broadcast::Sender::send` is sync and non-blocking (`events.rs:140-143`), so publishing under a `std::sync::Mutex` is safe.
  - **Never hold the lock across `.await`.**
  - Lock order is tracker → registry. `cancel_*` and cascade paths take only the registry lock (`registry.rs:188-238`), so there is no reverse-order deadlock.
  - Publishing under the lock makes bus order equal seq order. The UI can then use either "drop `seq <= snapshot.seq`" or a running watermark safely.
- If the seq instead lives in core's registry lock and the publish happens after unlock, the snapshot dedup is still correct. However, events can then reach the bus out of seq order across threads. Any consumer that keeps a running max-seq watermark would then wrongly drop a late lower-seq event. **Tell the frontend: compare only against the snapshot seq, and never assume contiguous seqs.**
- Ordering per session holds by construction:
  - `register_session` runs before spawn (`run.rs:75`).
  - The session task that later deregisters is spawned after that (`run.rs:154`, `run.rs:306-319`).
  - So `SessionStarted` happens-before `SessionExited` for the same id.
- The snapshot needs a new registry read that returns `(id, kind, profile_key)` per entry. `active_profile_keys` only returns sorted, de-duplicated keys (`registry.rs:112-124`).

### 1.2 Exactly-once `SessionExited`

- `SessionGuard` has a `deregistered` flag:
  - In `finish`, the flag is set **after** the registry call (`finalizer.rs:236-242`). If `registry.deregister` panics (poison policy, `registry.rs:20-25`), the flag stays false and `Drop` calls again (`finalizer.rs:295-303`).
  - In `Drop`, the flag is set before the call, which is wrapped in `catch_unwind` (`finalizer.rs:349-353`).
- So exactly-once must come from the **registry's remove result**, not the flags: publish only when `remove` returned `Some`.
- The drop path should wrap the publish inside the same `catch_registry` closure. `Drop` must stay panic-free while unwinding.
- Exit details:
  - `finish` has the code and signal (`finalizer.rs:63-64`). Pass them into the deregister step.
  - The drop path has none, so use `None, None`, matching `publish_exited(None, None)` at `finalizer.rs:260`.
- Timing: `deregister` is step 78, after `publish_exited` (75), cascade (76) and blocking post-exit hooks (77) (`finalizer.rs:62-80`). `SessionExited` therefore trails `launch-complete` by the hook runtime. Tests must not assume an ordering between `launch-complete` and `launch-session-changed`. If the UI needs the session gone sooner, moving deregister earlier is a design change; keep it at step 78 unless decided otherwise.
- Spawn failure: the session is registered, then `fail_spawn` deregisters it (`run.rs:90-103,225`). If `SessionStarted` is published on register as the spec says, a failed spawn must publish `SessionExited { code: None, signal: None }`. The alternative is to publish Started only after a successful spawn. Either way, a snapshot taken in between stays consistent. Cover this case in `tests/start.rs:79` (`spawn_failures_deregister_without_history_…`).

### 1.3 Linked trainer cascade

- `cascade_linked_children` only **signals** trainer cancel channels (`finalizer.rs:338-347` → `registry.rs:188-207`). It does not deregister them.
- A trainer stays registered until its own process exits and its own guard deregisters it. Existing tests assert "the trainer stays: only the cascade signalled it" (`tests/finalizer.rs:573-577`, also `:482-486`).
- So no synthetic Exited is sent for children. Each trainer's `SessionExited` comes from its own finalizer.
- `cancel_sessions_for_profile` (reset) also only signals (`queries.rs:164-168`; test `tests/queries.rs:171-178`). Sessions stay listed until they exit. The UI must not optimistically drop a session on reset; it should wait for `SessionExited`.
- Currently Playing counts **games only**: `running_profiles` filters `SessionKind::Game` (`queries.rs:172-174`). Events carry `kind`, so the provider has to filter `kind === 'game'`.

### 1.4 Lagged subscribers / Resync

- The bus is bounded at 256 by default (`events.rs:8`). A slow receiver gets `AppEvent::Resync { missed }` followed by the retained tail (`events.rs:170-175`; tested in `events/tests.rs:27`).
- Bridge: the existing Resync arm re-emits conservative payloads (`src-tauri/src/startup_bridge.rs:182-188`). Add a session resync emission there. Two options:
  - (a) `{"kind":"resync"}`, after which the UI re-invokes the snapshot command.
  - (b) The full snapshot `{kind:"snapshot", sessions, seq}`. This requires the bridge to hold a `LaunchService` clone. That is allowed: the bridge comment forbids only an `App` clone (`startup_bridge.rs:169-171`).
- UI recovery: on resync, re-fetch the snapshot, **replace** the state (no merge), and set `snapshotSeq = snap.seq`. Buffered or late events with `seq <= snapshotSeq` are then dropped as usual.
- The bridge's bus subscription is taken before Tauri starts (`src-tauri/src/lib.rs:43,84`), so the bridge misses no bus event. The only gap is on the webview side, between page load and `listen()` resolving. Subscribe-then-snapshot plus seq covers it.
- Webview side: `subscribeEvent` is async (`src/lib/events.ts:7-11`). The provider must **await** the subscription promise before calling the snapshot command. It must buffer events that arrive before the snapshot resolves, then apply only `seq > snap.seq`.

### 1.5 Multiple windows

- Only window `main` exists (`src-tauri/tauri.conf.json:14`, `src-tauri/capabilities/default.json:5`). `AppHandle::emit` broadcasts to every webview (`startup_bridge.rs:129`; `commands/launch/relay/mod.rs:165-172`).
- "Two windows" is therefore verified as:
  - two bus subscribers (Rust);
  - two provider consumers sharing one subscription (Vitest);
  - two `subscribeEvent` listeners in browser-dev, since `emitMockEvent` fans out to every listener (`src/lib/events.ts:21-29`).

## 2. Data crossing to the webview

- Proposed payload: `session_id` (UUIDv4, `crates/crosshook-core/src/launch/session/types.rs:20-26`), `profile_name: Option<String>`, `kind`, `seq: u64`, plus `code`/`signal: Option<i32>` on exit.
  - Every field is already exposed today: `session_id` in the `launch-*` payloads (`relay/mod.rs:24-31`), profile names via `list_running_profiles` and `profiles-changed` names.
- **Must not include:** `log_path`, `game_path`, the exe basename, the PID, the Steam app id, `trainer_host_path` or `teardown_reason` detail. `SessionContext` holds all of these (`run.rs:134-151`); do not serialize it.
- `SessionKind` serializes as `"Game"`/`"Trainer"`, because it has no `rename_all` (`types.rs:37-41`). Use a wire DTO with explicit snake_case values to match the other payloads (`ProfileChangeReason`, `events.rs:31-33`). Do not change core's serde on `SessionKind`: it already flows into diagnostics/IPC.
- Profile names are user-controlled text. React escapes them, so there is no new XSS surface. Do not log them above the levels already used (`run.rs:188-198` logs `profile_key` at info/debug).
- **Anonymous sessions:**
  - `session_profile_key_for_name` maps a blank or `None` name to `ANONYMOUS_PROFILE_KEY` (`crates/crosshook-core/src/launch/session/keys.rs:14-25`). Profile validation reserves that string (`crates/crosshook-core/src/profile/toml_store/utils.rs:68`, `crates/crosshook-core/src/profile/legacy.rs:124`).
  - Today the sentinel reaches the UI. `LibraryPage` never matches it (`runningProfiles.has(p.name)`, `src/components/pages/LibraryPage.tsx:136`), yet the AppShell badge counts it (`src/components/layout/AppShell.tsx:93-97`).
  - Recommendation: map the key to `profile_name: None` when `key == ANONYMOUS_PROFILE_KEY`. Keep emitting the event so other consumers can show "unnamed launch", and have Classic exclude `None` from the running-name set and the badge.
  - Use the **trimmed key** as the name, not `ctx.profile_name`, which is untrimmed (`run.rs:138`). That keeps it identical to the library `p.name` and to `list_running_profiles`.
- Runtime-only: no TOML or SQLite change, and no `launch_operations` write.
- Browser-dev mocks must keep the loopback-only and `verify:no-mocks` guarantees (AGENTS.md § Browser Dev Mode).

## 3. `steam_applaunch` exit semantics

Current behavior:

| Step                                                                                                                                    | Evidence                                                                                                                         |
| --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Classic game launch sets `launch_game_only: true`                                                                                       | `src/hooks/useLaunchState.ts:112-120,302`                                                                                        |
| Helper argument `--game-only`                                                                                                           | `steam_helpers.rs:300-302`                                                                                                       |
| Helper starts `steam -applaunch` in the background and waits a fixed 30 s                                                               | `steam-launch-helper.sh:331-339`; `steam_helpers.rs:21`                                                                          |
| Helper exits 0 in game-only mode, whether or not the game is still running                                                              | `steam-launch-helper.sh:355-357`                                                                                                 |
| Finalizer: steam exit 0 → `indeterminate`, then cascade, hooks, **deregister**                                                          | `tests/finalizer.rs:349-370`; `finalizer.rs:62-80`                                                                               |
| No watchdog for steam: the gamescope watchdog runs only for the `proton_run` execution method                                           | `gamescope.rs:54-57,116-124`                                                                                                     |
| Real exit is detected only in the UI: poll `check_game_running(exe)` every 3 s; `game-stopped` fires after "seen running → not running" | `useLaunchState.ts:218-262`                                                                                                      |
| `check_game_running` = sync `/proc` scan, host `ps` under Flatpak (through the gateway)                                                 | `src-tauri/src/commands/launch/queries.rs:73-80`; `crates/crosshook-core/src/launch/watchdog/mod.rs:38`; `watchdog/host.rs:4,72` |

So the game session today disappears from the registry about 30 s after launch. Neither the registry nor `list_running_profiles` reflects a running Steam game after that.

Required design (new):

- Add a cancel-consumer variant, for example `CancelConsumer::SteamGameWatch { exe_name }`, chosen when `kind == Game && execution_method == steam_applaunch && !launch_trainer_only`. `game_path` is required for Steam games (`crates/crosshook-core/src/launch/request/validation.rs:484-495`). Use `request.game_executable_name()` (`crates/crosshook-core/src/launch/request/models.rs:226`).
- **Hand off deregistration:** for these sessions, the finalizer must skip its deregister step and leave it to the watcher. Steps 75–77 (`launch-complete`, cascade, hooks) and portal unregister stay unchanged. The `Drop` path must still deregister if the session task dies before the handoff, so the flag or ownership transfer must be explicit and tested.
- Watcher loop (runs only while a Steam session is alive, so idle wakeups stay 0):
  - Wait for the game to appear, with a timeout such as the helper's 90 s `DEFAULT_GAME_TIMEOUT_SECONDS` (`steam_helpers.rs:22`). Mirror the UI's `observedGameProcessRef` "appear, then disappear" rule (`useLaunchState.ts:224-233`).
  - Then poll until it disappears, for example every 2 s like `GAMESCOPE_POLL_INTERVAL` (`watchdog/mod.rs:16`).
  - Then deregister and publish `SessionExited { code: None, signal: None }`. The game's own exit code is unknowable because Steam owns the process.
  - Never appeared → deregister at the timeout.
  - Run `is_process_running` in `spawn_blocking`: it is sync and shells out.
  - `select!` on `cancel_rx`: on `UserRequest` (reset) or a closed channel, deregister immediately. The current Drain consumer only records the reason (`crates/crosshook-core/src/launch/session/drain.rs:31-44`).
- **Invariants:** the watcher never signals or kills (the "No kill path" rule, `crates/crosshook-app/README.md:236-240`; `spawner.rs:4-6`). It uses only `is_process_running`, so host-gateway compliance is preserved (`check-host-gateway.sh`).
- **Known limits to document:**
  - Name-based matching: an unrelated process with the same exe name keeps the session alive. The UI behaves the same today.
  - Steam startup with long shader compiles may exceed the appear-timeout.
  - Trainer linking changes. With the game registered longer, a later trainer now links to it (`registry.rs:142-180`), but the game's cascade already ran at helper exit, so real game exit does **not** cascade. Adding a cascade at real game exit would newly kill a Flatpak-Steam trainer's gamescope tree, because that trainer runs through `proton_run` and gets the watchdog consumer (`gamescope.rs:36-45`). Treat that as out of scope unless decided explicitly.
- **SessionExited timing semantics:**
  - `proton_run` / `native`: published at finalizer step 78 (child or gamescope exit, after hooks).
  - `steam_applaunch` game: published when the watcher sees the game exe disappear, or at timeout, or on reset.
  - Trainers: their own child exit.
  - `launch-complete` still fires when the helper exits, so the existing reducer no-op stays valid (`useLaunchState.ts:83-84`).
- Frontend correlation: `LaunchResult` has no `session_id` (`src-tauri/src/commands/launch/shared.rs:4-11`; `src/types/launch.ts:144-150`). For `useLaunchState`'s `game-stopped`, match `profile_name === profileName.trim() && kind === 'game'`. Better, add an optional `session_id` to `LaunchResult`: an additive IPC field the relay already has (`execution.rs:9-24`).

## 4. Test strategy

### 4.1 Rust — core registry (`crates/crosshook-core/src/launch/session/registry_tests.rs`, 315 lines)

- `deregister` returns `Some` (or `true`) once, then `None` (or `false`). Extend `register_and_deregister_round_trip` (`registry_tests.rs:14`).
- A sessions listing returns `(id, kind, profile_key)`.

### 4.2 Rust — app tracker (new `crates/crosshook-app/src/services/launch/tests/sessions.rs`; keep under ~500 lines)

- **Fan-out:** two `bus.subscribe()` receivers each get `SessionStarted` then `SessionExited` with equal seqs. Pattern: `events/tests.rs:11-24`.
- **Exactly once:** run `SessionGuard::finish`, which drops the guard afterwards, then call `deregister` a second time directly. Collect with `try_recv` until empty and assert one Exited. Reuse the finalizer harness (`tests/finalizer.rs:138-160`). Also cover the panic and abort drop paths (`tests/finalizer.rs:436-577`): one Exited with `code/signal = None`.
- **Spawn failure:** extend `tests/start.rs:79` to assert Started+Exited (or nothing, per the chosen design) and an empty snapshot.
- **Snapshot race, deterministic:**
  1. Sequential interleave: subscribe → register A (seq 1) → snapshot `{A}`, seq 1 → register B (2) → deregister A (3). Apply events with seq > 1. The final set `{B}` must equal a fresh snapshot.
  2. Make the race falsifiable with a `#[cfg(test)]` gap hook between mutation and publish, e.g. a `Barrier` or a `oneshot` the test holds. Take the snapshot inside the gap and assert no Started for an id already in the snapshot and no Exited for an id that is absent. **Verify the test by temporarily moving seq assignment outside the lock: it must fail.** Prior repo learning: fakes that ignore arguments made security tests unfalsifiable.
  3. Contention check: `std::thread::scope` with N threads looping register/deregister behind a `Barrier`, while the main thread runs subscribe → snapshot → drain. Use `EventBus::new(large)` to avoid lag. Invariant for every applied event: Started ⇒ id ∉ state, Exited ⇒ id ∈ state. Final state == final snapshot. The assertions hold for every schedule, so the test is not flaky.
- **Lag:** `EventBus::new(2)`, publish more than 2 events, and the receiver sees `Resync { missed }` (pattern `events/tests.rs:27`). The consumer helper re-snapshots and the state matches.
- **Steam watcher:**
  - Inject the process probe (a trait or `Arc<dyn Fn(&str) -> bool + Send + Sync>`) and drive time with `tokio::time::pause()`/`advance` (`test-util` is already in `crates/crosshook-app/Cargo.toml:37`).
  - Assert: no Exited at helper exit; Exited after appear → disappear; Exited at appear-timeout; Exited immediately on `UserRequest`; no signal or kill.
  - Repo convention: no real sleeps or runtime timers in tests (`services/health/tests.rs:132-133`, `services/host_tools/tests.rs:69`).
- The `LaunchService::new` signature gains an `EventBus`. Update the harness (`tests/helpers.rs:234`) and the production wiring (`bootstrap/stores.rs:118,182`). Keep `crosshook-app` free of Tauri (`scripts/check-app-deps.sh`).

### 4.3 Rust — Tauri bridge (`src-tauri/src/startup_bridge/tests.rs` pattern)

- Pure payload builders tested with `serde_json::to_value` (`startup_bridge/tests.rs:160-188`). Example: `{"kind":"started","session_id":"…","profile_name":"Alpha","session_kind":"game","seq":3}`, an exited payload with `code`/`signal`, and `profile_name: null` for anonymous.
- An event-name constant test, `"launch-session-changed"` (pattern `startup_bridge/tests.rs:201-207`).
- Add `SessionStarted`/`SessionExited` to the "must stay bridged" list (`startup_bridge/tests.rs:226-240`). `is_bridged` is at `startup_bridge.rs:160-167`.
- Test the resync payload. `startup_bridge.rs` is 205 lines; put session bridging in a sibling module (for example `src-tauri/src/session_bridge.rs` plus `tests.rs`) rather than growing the startup bridge.
- If a new snapshot command is added, add it to `src-tauri/src/commands/launch/tests/contract.rs:1-22`.

### 4.4 Vitest

- Provider test. Mock `@/lib/events` and `@/lib/ipc` as in `src/hooks/__tests__/useLaunchState.events.test.ts:13-29`, and use deferred promises instead of timers. Assert:
  - the subscription promise resolves **before** the snapshot `callCommand`, by checking mock call order;
  - an event arriving during a pending snapshot is applied iff `seq > snap.seq`;
  - a replayed duplicate does not double;
  - Exited for an unknown id is a no-op;
  - resync triggers a re-fetch and a state replace;
  - two consumers cause one subscribe and one fetch (single provider);
  - unmount unsubscribes;
  - anonymous and trainer sessions are excluded from the name set and badge;
  - `vi.useFakeTimers()` + `expect(vi.getTimerCount()).toBe(0)` after mount.
- `useLaunchState`:
  - `game-stopped` dispatches on its own game's `SessionExited`;
  - `check_game_running` is never called on mount, so remove it from `useLaunchState.test.ts:44` and `useLaunchState.events.test.ts:100` or keep it on-demand only;
  - the timer count is 0.
- Update the `list_running_profiles` stubs in `src/components/pages/__tests__/LibraryPage.test.tsx:252` and `LaunchSubTabs.test.tsx:73` (`check_game_running`) to the new contract.

### 4.5 Playwright smoke

- `tests/smoke.spec.ts:236-265` (Currently Playing) seeds through `_mock_set_profile_running` (`tests/navigation-helpers.ts:45-55`). It currently relies on the 3 s poll or a remount fetch.
- With a single AppShell provider mounted before the seed, the mock must **emit** `launch-session-changed` (incremented seq) from `_mock_set_profile_running` (`src/lib/mocks/handlers/launch.ts:818-826`), from mock `launch_game`, and from the completion timer (`launch.ts:315-322`). Otherwise the filter assertion at `smoke.spec.ts:262` breaks.
- The mock's internal `setTimeout` scheduler (`launch.ts:280-286`) only simulates the backend and is excluded from the grep check.

### 4.6 "No setInterval" AC (grep)

Baseline today: exactly 2 hits, `src/hooks/useLaunchState.ts:257` and `src/hooks/useRunningProfiles.ts:46`. After the change, both of these must print nothing:

```bash
cd src/crosshook-native
rg -n 'setInterval\(' src --glob '!src/lib/mocks/**' --glob '!**/__tests__/**' --glob '!**/*.test.*'
rg -n 'set(Interval|Timeout)\(|check_game_running' src/hooks/useRunningProfiles.ts src/hooks/useLaunchState.ts
# plus the new provider file(s), e.g. src/context/RunningSessionsContext.tsx, if added
```

Idle wakeups (manual): `perf stat -e sched:sched_wakeup -p <crosshook pid> -- sleep 60` with no session running. The Steam watcher only polls while a Steam game session is registered.

## 5. Repo gates (exact commands)

From the repo root of the worktree. CI sources: `.github/workflows/lint.yml:45-156`, AGENTS.md § Commands.

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app -p crosshook-native -p crosshook-cli
(cd src/crosshook-native && npx @biomejs/biome ci src/ && npm run typecheck && npm run test:coverage && npm run test:smoke)
./scripts/check-mock-coverage.sh          # == npm run dev:browser:check
./scripts/check-host-gateway.sh && ./scripts/check-host-gateway.sh --self-test
./scripts/check-startup-path.sh && ./scripts/check-startup-path.sh --selftest   # lib.rs setup/bridge spawn touched
./scripts/check-app-deps.sh               # crosshook-app must stay Tauri-free (not wired in lint.yml; run manually)
./scripts/lint.sh                         # aggregate (clippy, biome, shellcheck, host-gateway, startup-path)
```

- **Mock-coverage rules** (`scripts/check-mock-coverage.sh:67-140`):
  - Every `#[tauri::command] pub fn` needs a `map.set('<name>'` in `src/lib/mocks/handlers/`, and the reverse holds too (orphans fail the gate).
  - A new snapshot command (e.g. `list_running_sessions`) needs a mock handler.
  - Removing `list_running_profiles` requires removing its mock (`launch.ts:744`) and its entry in `wrapHandler.ts:53-54`.
  - `check_game_running` stays, so its mock stays.
  - `_mock_*` helpers are excluded.
- **File-size soft cap (~500 lines, CLAUDE.md § MUST):**
  - Already over: `src/hooks/useLaunchState.ts` (618) and `src/lib/mocks/handlers/launch.ts` (846). Removing the poll shrinks the first. Put the provider in its own file; put new mock session logic in a sibling handler file.
  - Near the cap: `tests/finalizer.rs` (578, so add new tests in a new file), `services/launch/types.rs` (354), `finalizer.rs` (353), `run.rs` (319), `registry.rs` (254), `startup_bridge.rs` (205), `events.rs` (180).
  - Check with `wc -l` on the touched files.
- Docs: update `crates/crosshook-app/README.md:109` (the `LaunchService` row) for the new events and snapshot API, and the stale `registry.rs:52-53` comment.
- PR title: `feat(app): …`, `Closes #551`.
