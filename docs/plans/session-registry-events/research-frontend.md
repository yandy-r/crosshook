# YAN-824 research: Classic frontend (React/Tauri)

App root is `src/crosshook-native/`. Unless noted, paths below are relative to it. The research was read-only.

## 1. Running-state hooks and their consumers

### `src/hooks/useRunningProfiles.ts` (59 lines)

- Signature: `useRunningProfiles(options?: { enabled?: boolean }): Set<string>` (:17). No caller passes `enabled`.
- `refresh()` calls `callCommand<string[]>('list_running_profiles')` (:28-43). It keeps the same Set reference when the contents are unchanged (`setsEqualString`, :5-15, :35). On an IPC error it resets to an empty set (:37-42).
- **Polling: `window.setInterval(refresh, 3000)` (:46)**. It also re-fetches on every `launch-complete` event (:47-49).
- Every caller creates its own effect, its own interval and its own IPC poll. There is no sharing.

### Consumers (all of them, from grep)

| File:line                                                     | Use                                                                                                                            |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `src/components/layout/AppShell.tsx:93-97`                    | `runningProfiles.size` becomes `libraryFilterBadges = { currentlyRunning: n }`. This is the sidebar "Currently Playing" badge. |
| `src/components/pages/LibraryPage.tsx:68`, `:135-137`, `:145` | The `filterKey === 'currentlyRunning'` filter, `list.filter(p => runningProfiles.has(p.name))`                                 |
| `src/components/layout/Sidebar.tsx:77`                        | `{ filterKey: 'currentlyRunning', label: 'Currently Playing' }`; the badge renders at `:19`, `:34`, `:56`                      |
| `src/components/library/LibraryToolbar.tsx`                   | The "Running" chip (filter key only, no data)                                                                                  |

LibraryPage renders inside AppShell (`ContentArea.tsx:76`), so **two pollers run at the same time today**. AppShell calls the hook at `:93`, which is above the providers in its own JSX (`<LaunchStateProvider>` at `:435`). A new single provider therefore has to sit above `<AppShell>`, in `src/App.tsx:37-47`, or the hook has to read a shared module-level store (`useSyncExternalStore` plus a ref-counted subscription).

### `src/hooks/useLaunchState.ts` (618 lines)

- It is wrapped by `src/context/LaunchStateContext.tsx:19-41`, which is mounted once in `AppShell.tsx:435`. It reads `profileName = selectedProfile`, `request = buildProfileLaunchRequest(...)` and `request.profile_name = profileName.trim() ? profileName : undefined` (`src/utils/launch.ts:56`).
- **Polling: `:236-262`.** It derives `exeName` as the basename of `request.game_path`, calls `check_game_running({ exeName })` once, then calls **`window.setInterval(check, 3000)` (:257)**. It depends on `[request?.game_path]`. IPC failures keep the previous state (:251-253).
- `isGameRunning` (:148) is used in four places:
  1. **Exit detection (:218-234).** `observedGameProcessRef` latches once `isGameRunning` turns true while `phase !== Idle`. When it later goes false and `phase` is `WaitingForTrainer` or `SessionActive`, the hook dispatches `game-stopped`, which sets `phase` to `Idle` (reducer :78-82). **This is the only path that returns a session to Idle when the game exits.** The `launch-complete` reducer case is a no-op (:83-84, :283-287).
  2. `statusText`/`hintText` show "Game process detected / already running" when `isGameRunning && phase === Idle` (:525-529, :559-563).
  3. `canLaunchGame` includes `&& !isGameRunning` (:584).
  4. It is returned as `isGameRunning` (:601). `HeroLaunchGate.tsx:85,207` passes it to `HeroLaunchCommandSection.tsx:122,153,302-305`, where the button label reads "Game Running".
- Other code resets `isGameRunning`: the profile switch (:171), `reset()` (:493), and `launchGame` clears `observedGameProcessRef` (:304).
- The hook has no other timers. `HeroLaunchGate.tsx:103-121` and `HeroLaunchCommandSection.tsx:169,228` have copy-feedback `setTimeout`s that are unrelated to running state.
- After this change, no `setInterval` remains for running state: these two (`useRunningProfiles.ts:46` and `useLaunchState.ts:257`) are the only intervals in `src/` (grep). `usePrefixDeps.ts:125-141` polls with `setTimeout`, but only while a prefix is busy, so it is unrelated.

## 2. Event subscription and provider patterns

- **Subscription wrapper:** `src/lib/events.ts:7-18`, `subscribeEvent<T>(name, handler): Promise<UnlistenFn>`.
  - Under Tauri it calls `listen()` from `@tauri-apps/api/event`.
  - In the browser it registers on an in-memory `browserBus` Map.
  - `emitMockEvent(name, payload)` (:21-29) runs listeners synchronously and is a no-op under Tauri. `resetBrowserEventBus()` (:31) clears the bus.
- **IPC:** `callCommand` comes from `@/lib/ipc`. In webdev mode `ipc.ts` dynamically imports `./mocks` and calls `registerMocks()` (see the mocks README). Playwright reaches it through `window.__CROSSHOOK_DEV__.callCommand` (`src/vite-env.d.ts:22`, `tests/navigation-helpers.ts:40-55`).
- **Payload validation precedent:** `src/lib/profileEvents.ts:37-47` (`parseProfilesChanged`) validates at the event boundary and returns `null` for malformed input, which callers treat as a conservative resync. Its type is in `src/types/profile-events.ts`. Use the same pattern for `launch-session-changed`.
- **`profiles-changed` subscribers:** `useProfile.ts:227`, `useLibrarySummaries.ts:67`, `useProfileSummaries.ts:42`, `useCommunityProfiles.ts:436`, `useProfileHealth.ts:285`.
- **Subscribe-then-snapshot precedent:** `useProfileHealth.ts:200-265`. It awaits `Promise.all([unlisten promises])` (:247), then calls `fetchStartupReport({ fresh: true })` and ignores the snapshot if an event already applied (`startup.current`). The new provider should follow the same ordering: `await subscribeEvent(...)`, buffer or seq-filter, then fetch the snapshot.
- **Context files:** `src/context/*Context.tsx` (Collections, HostReadiness, InspectorSelection, LaunchState, Preferences, Profile, ProfileHealth). The convention is `createContext<T | null>(null)` plus a `useXContext()` that throws outside its provider (`LaunchStateContext.tsx:17,43-49`).
- **Other `launch-complete` subscribers:** `useProfileHealth.ts:292` (it re-validates health, so leave it) and `useRunningProfiles.ts:47`, which the new provider removes.

## 3. Browser-dev mocks

- **Layout:** `src/lib/mocks/{index.ts, store.ts, eventBus.ts, wrapHandler.ts, handlers/*.ts, README.md, __tests__/}`.
  - `eventBus.ts` only re-exports `emitMockEvent` (:1).
  - `index.ts:77-95` is `resetMockEnvironment()`. It calls `resetBrowserEventBus()` and `resetLaunchMockState()`.
- **Launch mock** (`src/lib/mocks/handlers/launch.ts`):
  - Module state: `runningGames` and `runningProfiles` Sets, and `pendingLaunchTimers` (:24-26).
  - `check_game_running` (:731-739) returns `runningGames.has(exeName)`. With the `empty` fixture it returns false; with `loading` it never resolves; `error` deliberately resolves because the read is polled (comment :735-736, which needs updating).
  - `list_running_profiles` (:744-751) returns the sorted `runningProfiles`, with the same fixture handling.
  - `launch_game` (:429-459) and `launch_trainer` (:464-488) schedule log, diagnostic and complete events through `scheduleLaunchLogSequence` (:292-323). **They never add to `runningProfiles`.** Only the `launch-complete` timer removes the profile (:316-322).
  - Dev helpers `_mock_set_game_running` (:808-816) and `_mock_set_profile_running` (:818-826) only mutate the Sets. **They emit no event today; the 3 s poll picks up the change.**
  - `resetLaunchMockState` (:837-846) clears both Sets and the timers.
- **Mock event emission pattern:** `emitMockEvent('<name>', payload)` from a handler. Examples: `launch.ts:303,312,321`, `install.ts:72-160`, and `profile-utils.ts:65-71` (a single-seam helper, `emitProfilesChanged`). Add a matching seam such as `emitLaunchSessionChanged()` in `launch.ts`, with a mock `seq` counter that `resetLaunchMockState` resets.
- **`wrapHandler.ts:45-76`:** `EXPLICIT_READ_COMMANDS` lists `check_game_running` and `list_running_profiles` (:53-54). A new or renamed snapshot command must go here so `?errors=true` never rejects it. A name ending in `_snapshot` already matches `READ_NOUN_RE` (:87).
- **`scripts/check-mock-coverage.sh`** (repo root; also `npm run dev:browser:check`, `package.json:12`):
  - It diffs `#[tauri::command] pub (async )?fn NAME` in `src-tauri/src` and `crates/crosshook-core/src` (:27-30, :67-72) against `map.set('NAME'` in `src/lib/mocks/handlers` (:89-95), excluding `_mock_*`.
  - It exits 1 on any missing or orphaned entry (:138-140).
  - **It checks commands only, not events.** A new event needs no script change. A new or renamed command needs a matching `map.set`, and removing a command means removing its mock.
  - It is a shell script, so "updated" in the spec really means keeping it passing. Optionally update the README command count (`README.md` "163 IPC commands").
- **Mock tests:** `src/lib/mocks/__tests__/launch.session.test.ts` uses fake timers, `configureMockHandlers()`, and `subscribeEvent` captures of mock launch events. This is the natural place for a mock `launch-session-changed` sequence test.

## 4. Tauri bridge

- **`src-tauri/src/startup_bridge.rs`:**
  - `spawn(handle, startup, rx: EventReceiver)` (:172-202) loops on `rx.recv().await`, filters with `is_bridged` (:160-167), and matches `AppEvent`.
  - Each arm calls a small `emit_*` helper that `tracing::warn!`s on `handle.emit` errors (e.g. `emit_profiles_changed` :38-42 and `emit_ui_settings_persist_failed` :55-60, with the event name in a `const`, :46).
  - It ends with a `_ => {}` arm because `AppEvent` is non-exhaustive (:197-198).
  - **Add** `AppEvent::SessionStarted{..}` and `AppEvent::SessionExited{..}` arms that emit `launch-session-changed` with a serde wire struct, plus a resync emission inside the `AppEvent::Resync` arm (:182-188) so Classic re-snapshots after bus lag.
  - The payload-shape tests use `serde_json::to_value` in `src-tauri/src/startup_bridge/tests.rs` (e.g. `:160-207`, the profiles-changed and ui-settings tests).
- **`src-tauri/src/lib.rs`:** `let startup_events = core.subscribe();` (:43). In `.setup(...)` it calls `startup_bridge::spawn(app.handle().clone(), core.startup(), startup_events)` (:83-84). `services.launch` is managed (:64). Extend the existing bridge rather than spawning a second one; the bridge already receives every `AppEvent`.
- **Commands:** `src-tauri/src/commands/launch/queries.rs`.
  - `check_game_running(exe_name)` (:73-80) is a raw `/proc` scan via `crosshook_core::launch::is_process_running`. It is not registry-based.
  - `list_running_profiles(service)` (:82-85) calls `service.running_profiles()`, which is `registry.active_profile_keys(Some(SessionKind::Game))` (`crates/crosshook-app/src/services/launch/queries.rs:172-174`).
  - A snapshot command such as `launch_running_sessions` would return `{ sessions, seq }`; register it in `generate_handler!` in `lib.rs` and add a mock.
- **Capabilities:** `src-tauri/capabilities/default.json` has `"windows": ["main"]` and `"permissions": ["core:default", ...]`. `core:default` includes `core:event:default`, which already lets the frontend `listen`, as all current events do. **No capability change is needed.** `tauri.conf.json:12-14` declares only the `main` window. `app.emit` broadcasts to every webview, so a second window would receive the event if it were added to the capability's `windows`.
- **Wire details to settle with the backend owner:**
  - `SessionKind` derives `Serialize` without `rename_all`, so it serializes as `"Game"`/`"Trainer"` (`crates/crosshook-core/src/launch/session/types.rs:37-41`).
  - `SessionId` is a newtype over a UUID and serializes as a string (:19-21).
  - The profile key is the trimmed name, or `"__crosshook_anonymous_profile__"` for unnamed launches (`crosshook-core/src/launch/session/keys.rs:14-25`). Today `list_running_profiles` can already return the anonymous key; the frontend should ignore it or the backend should omit it.
  - Only `Game` sessions count as "running" today (`running_profiles` filters on Game), so the provider should apply the same filter.

## 5. Existing tests that touch running state

**Vitest**

- There are **no direct tests of `useRunningProfiles`**.
- `src/hooks/__tests__/useLaunchState.test.ts:21-23` mocks `subscribeEvent` as a no-op. `:44-45` mocks `check_game_running` returning false.
- `src/hooks/__tests__/useLaunchState.events.test.ts:22-29` stores handlers in a `Map`; `emit(name, payload)` (:64-70) fires them. `:81-84` waits for `launch-diagnostic` and `launch-complete` subscriptions. It also mocks `check_game_running` (~:100). This is the template for a test that shows a `launch-session-changed` exit moving `WaitingForTrainer` to `Idle`.
- `src/components/pages/__tests__/LibraryPage.test.tsx:248-264`: "filters to running profiles from the runtime read hook". It sets `handlerOverrides: { list_running_profiles: async () => ['Test Game Alpha'] }`. Its harness (:74-104) has no running-profiles provider.
- `src/components/layout/__tests__/AppShell.test.tsx:152-155` clicks "Currently Playing" and expects the "Running" chip to be pressed. Its harness is `AppShellInAppProviders` (:21-35).
- `src/components/layout/__tests__/Sidebar.test.tsx:56-66` renders the badge from `{ currentlyRunning: 1 }` props only, so it is unaffected.
- `src/components/__tests__/LaunchSubTabs.test.tsx:73` overrides `check_game_running: async () => false`.
- `src/components/library/__tests__/HeroLaunchGate.test.tsx:196` mocks context `isGameRunning: false`; `HeroLaunchCommandSection.test.tsx:483-487` covers the "Game Running" label.
- **Harnesses that mount AppShell or LibraryPage** need the new provider if it is context-based:
  - `src/App.tsx:37-47`
  - `AppShell.test.tsx:21`
  - `AppShell.startup.test.tsx:20-34`
  - `src/__tests__/a11y/skip-link.a11y.test.tsx:21`
  - `src/__tests__/a11y/routes.a11y.test.tsx:72-90`
  - `LibraryPage.test.tsx:80-103`
  - `LibraryPage.viewMode.test.tsx:73+`

  A module-store hook needs none of these changes.

- **Listener-count precedent:** `src/hooks/__tests__/profilesChangedListeners.test.ts:13-48` counts how often each command is called after `emitMockEvent` (a `callSpy` plus a macrotask `settle()`). Use it to assert no polling: `list_running_profiles` or the snapshot command is called exactly once per mount, and never again under `vi.advanceTimersByTime`.

**Playwright** (`tests/`, `playwright.config.ts`, `npm run test:smoke`)

- `tests/smoke.spec.ts:233-268`: "Favorites and Currently Playing activate matching library chips". It calls `seedMockProfileRunning(page, 'Test Game Alpha', true)`, then expects only Test Game Alpha under the Running filter.
- `tests/navigation-helpers.ts:44-55` (`seedMockProfileRunning`) calls `_mock_set_profile_running`.
- **This test only passes because the 3 s poll (or the initial fetch after navigation) picks up the seeded state.** Once polling is gone, `_mock_set_profile_running` must emit `launch-session-changed` (with a seq), or the seed must happen before the provider takes its snapshot. Today the seed runs after `waitForCrosshookDevIpc`, possibly after the provider has mounted, so the emit is required.

## 6. What `check_game_running` does in useLaunchState, and what regresses

**What it does today.** It is a /proc scan by executable basename (`queries.rs:73-80`). It is **independent of the session registry**, so it also detects a game started outside CrossHook, for example from Steam directly. It drives four things:

- (a) the automatic return to Idle on game exit (`useLaunchState.ts:218-234`)
- (b) the "Game process detected / already running" status and hint text (:525-529, :559-563)
- (c) disabling Launch Game when the game is already running (`canLaunchGame`, :584)
- (d) the "Game Running" label on the hero button

**Replacement.**

- `launch-session-changed` events for the selected profile (session kind Game, `profile_name === profileName`) update `isGameRunning`.
- `SessionStarted` (Game) sets it to true, which latches `observedGameProcessRef`.
- `SessionExited` (Game) sets it to false, which dispatches `game-stopped` through the existing effect.
- Seed the initial value from the snapshot, or from the shared running-profiles provider (`runningProfiles.has(profileName)`). Deriving it from the shared provider avoids a second subscription; useLaunchState already has `profileName`.
- Per the spec, `check_game_running` remains an on-demand query, called once when the profile or `game_path` changes, to cover games launched outside CrossHook.

**Behavior that could regress:**

1. **External game detection becomes one-shot.** If the user starts the game from Steam after the profile is selected, the status, the hint, and `canLaunchGame`'s `!isGameRunning` no longer update live. This is accepted under the spec ("on-demand query only"), but it is a visible change. Keep the on-mount call so a game that is already running is still detected.
2. **steam_applaunch exit detection.** `runtime-helpers/steam-launch-helper.sh:327-357` launches `steam -applaunch &`, waits `game_startup_delay_seconds`, then `exit 0`s in game-only mode. The helper (the registered child) exits long before the game. The /proc poll used to keep `isGameRunning` true until the real game exited. With events only, the Game session must stay registered until a watchdog sees the real exe exit; that is the spec's "watchdog exit detection for steam_applaunch games" (backend). Otherwise the UI returns to Idle about 30 s after launch while the game is still running. The frontend relies on `SessionExited` arriving only on real exit.
3. **The latch requires a "running" observation first.** `observedGameProcessRef` is only set when `isGameRunning` was true (:224-227). If `SessionStarted` fires before the provider subscribes, or between snapshot and subscribe, the latch never sets and the exit never returns the UI to Idle. Subscribe first, take the snapshot, and drop events with `seq <= snapshotSeq`, as the spec says. Also consider latching on `game-success` itself.
4. **Profile key mismatch.** The registry key is the trimmed `request.profile_name` (or the anonymous key for unsaved profiles, `utils/launch.ts:56`). useLaunchState must compare against `profileName.trim()`. An unsaved draft (`profileId === 'new-profile'`) never matches, whereas the /proc poll worked for any `game_path`.
5. **Trainer-only sessions.** `running_profiles` counts only Game sessions. A "Launch Trainer" from Idle with an externally started game previously showed "Game process detected" through /proc. That now relies on the one-shot query (same as item 1).
6. **Mock parity.** The mock `launch_game` never marks a profile running (`launch.ts:429-459`) and `check_game_running` returns false, so browser-dev `isGameRunning` is always false today. If the mock now emits `SessionStarted` on `launch_game` and `SessionExited` at the `launch-complete` timer (:316-322), browser-dev flows will start showing "Game Running" and auto-Idle about 1.5-2 s after launch. Check that the smoke specs (pipeline and hero launch) still pass, or emit only from `_mock_set_profile_running`.
7. **Failure semantics.** Today an IPC failure in `useRunningProfiles` clears the set (:37-42); the new provider should do the same on a snapshot failure. An event error has no retry, so a `Resync` from the bridge must trigger a re-snapshot.
8. **Comment drift.** `launch.ts:735-736` and `:748-749` say "polled status read". Update them.

## Summary of files the Classic slice touches

- `src/hooks/useRunningProfiles.ts`: becomes a provider- or store-backed single subscription (subscribe, snapshot, seq filter, no interval, no `launch-complete` refresh).
- Possibly a new `src/context/RunningProfilesContext.tsx`, mounted in `src/App.tsx:37` plus the six test harnesses listed in section 5.
- `src/hooks/useLaunchState.ts:236-262`: remove the interval, keep a one-shot `check_game_running`, and drive `isGameRunning` from session events or provider membership.
- New `src/types/launch-session-events.ts` (or an addition to `types/launch.ts`) plus a parser in `src/lib/` mirroring `profileEvents.ts`.
- `src/lib/mocks/handlers/launch.ts`: a mock snapshot command with seq, an `emitLaunchSessionChanged` seam, emissions from `_mock_set_profile_running`, and optionally from `launch_game` and the complete timer; reset seq in `resetLaunchMockState`.
- `src/lib/mocks/wrapHandler.ts:53-54`: the read-command list.
- `src-tauri/src/startup_bridge.rs` (+ `startup_bridge/tests.rs`): `launch-session-changed` arms and a resync emission.
- `src-tauri/src/commands/launch/queries.rs`, `lib.rs` `generate_handler!`: the snapshot command.
- Tests:
  - new `src/hooks/__tests__/useRunningProfiles.test.ts`, covering the race (event during snapshot neither lost nor doubled) and asserting no timers via fake timers and the call count
  - an exit-to-Idle test in `useLaunchState.events.test.ts`
  - updated `LibraryPage.test.tsx:248-264`
  - a mock sequence test in `launch.session.test.ts`
- `tests/smoke.spec.ts:233-268`: relies on the mock emitting the event.
