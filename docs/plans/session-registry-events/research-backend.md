# YAN-824 / #551 — Backend (Rust) research: session registry events

Paths are relative to `src/crosshook-native/`. Abbreviations used below: `core` = `crates/crosshook-core/src`,
`app` = `crates/crosshook-app/src`, `tauri` = `src-tauri/src`.

## 1. Registry call sites and the exit path

### Production call sites (the full list)

| Call                                                               | Location                                                                                                       | Notes                                                                                                       |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `registry.register(kind, key)` (Game)                              | `app/services/launch/run.rs:180`                                                                               | inside `register_session`, called from `run_session` at `run.rs:75`. **Registration happens before spawn.** |
| `registry.register_and_link_to_parent_of_kind(Trainer, key, Game)` | `app/services/launch/run.rs:182`                                                                               | atomically links the trainer to the newest Game with the same key                                           |
| `registry.deregister(id)` on spawn failure                         | `app/services/launch/run.rs:225` (`fail_spawn`)                                                                | the session never ran; no exit status exists                                                                |
| `registry.deregister(id)` normal exit                              | `app/services/launch/finalizer.rs:240` (`SessionGuard::deregister`, called from `finish` at `finalizer.rs:78`) | guarded by the `deregistered` flag                                                                          |
| `registry.deregister(id)` drop/panic                               | `app/services/launch/finalizer.rs:302` (`deregister_from_drop`, inside `catch_registry`)                       | runs from `Drop` at `finalizer.rs:258-265`                                                                  |
| `LaunchSessionRegistry::new()` (the only production instance)      | `app/services/launch/mod.rs:101`                                                                               | owned by `LaunchService.registry: Arc<_>` (`mod.rs:75`)                                                     |

- **src-tauri:** no direct registry calls. It goes through `LaunchService` only:
  - `list_running_profiles` at `tauri/commands/launch/queries.rs:83-85`
  - `launch_reset_sessions` at `tauri/commands/launch/execution.rs:61`
  - `launch_game` / `launch_trainer` at `execution.rs:27-56`
- **crosshook-cli:** no registry and no `LaunchService`. `crates/crosshook-cli/src/launch.rs` spawns directly and reuses only `read_log_tail` / `stream_child_log` (`launch.rs:10`). **No CLI changes are needed.**
- **Tests** call `register` / `deregister` directly in these places. They use standalone `LaunchSessionRegistry::new()` instances, or `harness.service.registry()`, which is `#[cfg(test)]` at `mod.rs:158-161`:
  - `core/launch/session/registry_tests.rs`
  - `core/launch/session/drain.rs:72-103`
  - `crates/crosshook-core/tests/trainer_watchdog_parity.rs`
  - `app/services/launch/tests/{finalizer.rs:139-546, gamescope.rs:529-554, queries.rs:131-191, snapshot.rs:61, start.rs:405-419, streaming.rs:89-360}`

  Keeping `new()` as the observer-less default and the `deregister(id)` signature unchanged avoids touching any of these.

### Exit path, and where the code/signal live

- `spawn_session_task` is at `run.rs:306-319`. It builds the `SessionGuard` **before** `tokio::spawn`, so a drop still tears down. It then runs `stream_session(child, ..)`, which returns `Option<ExitStatus>`, followed by `guard.finish(status)`.
- `SessionGuard::finish` is at `finalizer.rs:62-80`:
  - `exit_code` / `signal` are derived at `finalizer.rs:63-64` through `ExitStatus::code` and `ExitStatusExt::signal`.
  - The order is: build the report (reads the log tail), record history in SQLite, success effects, diagnostic, trainer wrap-up, **`publish_exited(code, signal)`** (the `LaunchEvent::Exited` → Tauri `launch-complete`), cascade, **post-exit hooks (blocking `run_post_exit_hooks`)**, **deregister**, portal unregister.
  - ⇒ `deregister()` (`finalizer.rs:236-242`) currently takes no code/signal. They must be passed in, or stashed in the guard (for example `self.exit = Some((code, signal))`).
  - Consequence: deregistration, and so `SessionExited`, comes _after_ the post-exit hooks. A slow hook delays the "not running" flip. That matches today's `running_profiles` semantics, but it is worth knowing.
- The drop path publishes `Exited(None, None)` (`finalizer.rs:260`) and then deregisters. Use `None/None` there too.
- `fail_spawn` (`run.rs:205-236`): the registry already holds the entry, because registration precedes spawn. Any register-time publish therefore yields `Started` followed by `Exited(None, None)`. The alternative is to make the publish conditional; see §4.

### steam_applaunch exit detection: no watchdog exists today

- The game helper for steam_applaunch is `runtime-helpers/steam-launch-helper.sh`. `LaunchService` always forces `launch_game_only = true` for Game sessions (`app/services/launch/prepare.rs:83-93`), so the helper gets `--game-only` (`core/launch/script_runner/steam_helpers.rs:300-302`). The script then:
  1. runs `steam -applaunch <appid> &` (`steam-launch-helper.sh:331-336`)
  2. sleeps `--game-startup-delay-seconds` (30 s, `steam_helpers.rs:21,288-289`; loop at `steam-launch-helper.sh:308-320,339`)
  3. checks `process_visible "$game_exe_name"` once (`:341`)
  4. **`exit 0`** (`:355-357`)
- ⇒ About 30 s after launch, the CrossHook child (the helper) exits while the game keeps running. The finalizer runs and the session deregisters, so `running_profiles` drops the profile. The finalizer test `tests/finalizer.rs:349-370` records exit 0 for steam as `indeterminate`.
- That gap is why Classic polls `check_game_running(exeName)` every 3 s (`src/hooks/useLaunchState.ts:236-262`). `check_game_running` is `tauri/commands/launch/queries.rs:73-80` → `core::launch::is_process_running` (`core/launch/watchdog/mod.rs:38-71`):
  - it scans `/proc/*/comm`, falling back to cmdline at 15 chars
  - under Flatpak it also runs a host `ps` probe through `flatpak-spawn`, which blocks
- The existing core watchdog `gamescope_watchdog` (`core/launch/watchdog/tasks.rs:31-134`) is **gamescope-only**:
  - It is chosen only when `gamescope_required` is true, i.e. the execution method is `proton_run` (`app/services/launch/gamescope.rs:54-60,116-140`).
  - Steam sessions always get `CancelConsumer::Drain` (`gamescope.rs:122-123`; table in `tests/gamescope.rs:59-74`).
  - Its poll pattern is the template to copy: `GAMESCOPE_POLL_INTERVAL = 2s` (`watchdog/mod.rs:16`), with a startup poll followed by a main poll.
- The steam _trainer_ helper detaches the runner with `setsid … &` and exits 0 immediately (`steam-launch-trainer.sh:195-199`). Steam trainer sessions therefore also deregister at once. This is out of scope: the spec says "games".

**What "watchdog exit detection for steam_applaunch games" has to be:** a new post-helper liveness watch for `kind == Game && ctx.method == METHOD_STEAM_APPLAUNCH`.

- **Placement:** in `SessionGuard::finish`, after `publish_exited`, while the session is still registered.
- **Loop:** `run_blocking(is_process_running(exe))` every 2 s until the process is gone, then continue into deregister.
  - `exe` is `request.game_executable_name()` (`core/launch/request/models.rs:226-243`), the same value the helper gets via `--game-exe-name` (`steam_helpers.rs:259-260`). Add it to `SessionContext` (`app/services/launch/types.rs:336-354`).
  - If the process is not visible at helper exit, stop immediately. Optionally add a short bounded grace; the helper already waited 30 s.
  - **Stop on reset/cascade without any new registry API:** each tick, check `ctx.watchdog_outcome.reason().is_some()`. The Drain consumer records any cancel reason into the outcome (`core/launch/session/drain.rs:31-61`), and `reason()` is already used at `finalizer.rs:101-105`.
- **Ordering choice for the main agent:**
  - (a) Watch just before `deregister()` (after the hooks). Minimal change: the `launch-complete` timing, history row, hooks and cascade all stay as they are. Side effect: a trainer launched during the watch links to the still-registered steam game (`registry.rs:142-180`), but the game's cascade already ran. **Recommended for scope.**
  - (b) Watch before `cascade()`, so cascade, hooks and deregister all mean "game really exited". More correct semantically, but it changes when post-exit hooks run for steam.
- **Exit code for a watched exit:** the game is not our child, so recommend `code: None, signal: None`. The helper's own code is already reported via `launch-complete`, and 0 means "indeterminate".
- **Wakeups:** none while idle. Polling happens only while a steam game session is alive. Under Flatpak each tick shells out to host `ps`, so `run_blocking` is mandatory (`app/services/launch/mod.rs:54` re-exports `run_blocking`).
- **Drop safety:** if the task is dropped mid-watch, `Drop` still deregisters (`finalizer.rs:295-303`).

## 2. EventBus wiring today

- `EventBus` (`app/events.rs:114-151`) wraps a `tokio::broadcast::Sender<AppEvent>`.
  - `publish` never blocks and returns 0 with no subscribers (`events.rs:140-143`).
  - A lag is surfaced as `AppEvent::Resync { missed }` (`events.rs:168-176`).
  - Default capacity is 256 (`events.rs:8`).
  - `AppEvent` is `#[derive(Debug, Clone, PartialEq, Eq)] #[non_exhaustive]` (`events.rs:57-112`), so new payload fields must be `Eq`. `SessionId`, `SessionKind`, `Option<i32>` and `u64` all are.
- The bus is owned by `App` (`app/app.rs:19,95-121`) and reached through `App::events()` / `App::subscribe()` (`app.rs:120,138`).
- `Services::new(stores, events, runtime, jobs)` is at `app/bootstrap/stores.rs:118-213`. It is called from `bootstrap/app_bootstrap.rs:116`, `bootstrap/scans.rs:264` and `bootstrap/stores/tests.rs:128`. Inside it:
  - `ProfileWriter::new(..., events.clone())` at `stores.rs:121-126`
  - **`events` is moved** into `UiSettingsService::new(..., events, runtime)` at `stores.rs:132`, so `LaunchService` needs a clone taken before that line
  - `LaunchService::new(profiles, settings, metadata, Some(background))` at `stores.rs:182-187` currently gets **no bus**
- Publishing patterns elsewhere:
  - `ProfileWriter` holds `events: EventBus` (`app/services/profiles/mod.rs:50,62`) and publishes `AppEvent::ProfilesChanged` after the persist (`profiles/writer.rs:77-78`).
  - The `[ui]` writer publishes `UiSettingsPersistFailed` (`settings/ui_writer.rs:81`).
  - Startup jobs follow **state-then-publish** (`bootstrap/scans.rs:8-10`, `bootstrap/state.rs:4-6`). This is the same idea as "mutate + seq under the lock, then publish".
- `LaunchService` struct and constructor: `app/services/launch/mod.rs:70-108`. It is `Clone` and statically asserted `Send + Sync` (`mod.rs:166`). Its only constructions are:
  - production: `stores.rs:182`
  - test harness: `app/services/launch/tests/helpers.rs:233-245`

  Adding an `EventBus` parameter, or a `with_events(bus)` builder, touches exactly these two places.

- Tauri managed state is `.manage(services.launch.clone())` (`tauri/lib.rs:64`).
- The bridge is set up at `tauri/lib.rs:43` (`core.subscribe()`) and `lib.rs:84` (`startup_bridge::spawn(handle, core.startup(), startup_events)`):
  - `startup_bridge::spawn` (`tauri/startup_bridge.rs:172-202`) matches `AppEvent` and calls `app.emit(...)`, which is a global emit to all windows. That satisfies "two windows" for free.
  - `is_bridged` (`:160-167`) filters out the named scan events.
  - `Resync` re-emits startup jobs plus a conservative `profiles-changed` (`:182-188`). Session resync has to be added there too, either a resync marker or a pushed snapshot.
  - Payload structs are local `#[derive(Serialize)]` types (`:23-27,50-53`), tested in `tauri/startup_bridge/tests.rs` (for example `profiles_changed_payload_is_an_object_with_tagged_reason`, `:160-180`).
  - New arms for `SessionStarted` / `SessionExited` → `launch-session-changed` go here. Note that the bridge holds only `StartupHandle`. If a resync should push a fresh snapshot instead of a marker, `spawn` also needs a `LaunchService` clone (cheap, `Arc` fields).

## 3. `running_profiles()` and profile keys

- `LaunchService::running_profiles()` (`app/services/launch/queries.rs:170-174`) returns `registry.active_profile_keys(Some(SessionKind::Game))`, which is sorted and deduped (`core/launch/session/registry.rs:112-124`). The doc comment says "in registration order", but the result is actually **sorted**.
- Consumers:
  - Rust: only `tauri/commands/launch/queries.rs:83-85` (`list_running_profiles`), registered at `tauri/lib.rs:128`, contract test `tauri/commands/launch/tests/contract.rs:4,20`
  - app tests: `tests/queries.rs:126-194`, `tests/cascade.rs:212-239`
  - frontend: `src/hooks/useRunningProfiles.ts:29` (3 s `setInterval` at `:46` plus a `launch-complete` refresh at `:47`), mocks `src/lib/mocks/handlers/launch.ts:744`, `src/lib/mocks/wrapHandler.ts:54`, test `src/components/pages/__tests__/LibraryPage.test.tsx:252`
- Keys: `session_profile_key_for_name(Option<&str>)` (`core/launch/session/keys.rs:19-25`) gives the trimmed name, or `ANONYMOUS_PROFILE_KEY = "__crosshook_anonymous_profile__"` (`keys.rs:14`) for `None`/blank.
  - Profile-name validation rejects the sentinel (`core/profile/toml_store/utils.rs:68`, `core/profile/legacy.rs:124`), so for every real profile the key equals the trimmed name.
  - The registry stores **only** `profile_key` (`SessionEntry`, `core/launch/session/types.rs:98-105`). The raw `profile_name` lives in `SessionContext.profile_name` (`app/services/launch/types.rs:341`).
  - Reset uses the same derivation (`queries.rs:164-168`).
  - `list_running_profiles` already leaks the sentinel for anonymous sessions (asserted at `tests/queries.rs:185-194`).
- **Recommendation:** fill `SessionStarted/Exited.profile_name` and `SessionInfo.profile_name` from `profile_key`, for parity with `list_running_profiles` and the reset command. The frontend filters `kind === "game"` to rebuild the running set. Optionally map the sentinel to `null` in the Tauri DTO; that is a UX decision.

## 4. Atomic seq design

**Where the race is:** a mutation must not be both missed and double-applied relative to a snapshot. The protocol is: subscribe → snapshot `(sessions, S)` → apply events with `seq > S`, drop `seq <= S`.

**Requirement:** assign `seq` in the **same critical section** as the HashMap mutation, and have `snapshot()` read `(sessions, seq)` under that lock. That makes every mutation linearizable against every snapshot. Today there is one `Mutex<HashMap<SessionId, SessionEntry>>` (`registry.rs:27-29`). Change it to `Mutex<State { sessions: HashMap<..>, seq: u64 }>`.

**Proof sketch** (mutation M with seq k, subscriber subscribes at t₁ and snapshots at t₂ > t₁):

- If M's lock section precedes the snapshot's, then k ≤ S. The snapshot already reflects M, and M's event (if received) is dropped. Not doubled.
- If M's lock section follows the snapshot's, then k > S. M is not in the snapshot, and M's publish happens after M's lock > t₂ > t₁, so the subscription receives it. Not lost.
- This holds whether the publish happens **inside or outside** the lock, as long as seq assignment and mutation share the lock.

**Inside vs outside the lock:**

- **Outside** (core returns a change record, the app publishes after the call): bus order can invert across _different_ sessions. For example, thread A gets seq 5 and thread B gets seq 6, but B publishes first. Applying set ops per session is still correct, because one session's Started happens-before its Exited: register in `run_session`, then the deregister in the task spawned later, or in `fail_spawn` on the same task. But clients must **not** use a running "drop seq ≤ last applied" filter, only "≤ snapshot seq". Easy to get wrong in the frontend.
- **Inside** (publish while holding the registry lock): bus order equals seq order, strictly increasing. Clients can use either filter, and tests can assert monotonic seq.
  - Cost: `broadcast::Sender::send` takes its own short internal lock, never blocks on receivers and never re-enters the registry, so there is no deadlock risk.
  - The registry's poison policy (`registry.rs:20-25`) is unaffected, because `send` does not panic.
  - The existing comment says "teardown broadcasts are sent with the lock released" (`registry.rs:16-18`). That concerns _cancel_ fan-out to watchdogs (`cancel_linked_children`, `:188-207`), which stays as it is.

**Recommendation: publish inside the lock through a core-defined observer** (see §5). It gives strict ordering, a simpler client contract and a testable invariant.

**Idempotency must hold:**

- `deregister` is called up to twice: from finish/drop (flag-guarded) and from `fail_spawn`. Core docs promise it is idempotent (`registry.rs:52-57`, `registry_tests.rs:19-24`).
- Emit `Exited` and bump seq **only when `remove` returned `Some`**. Return that as a `bool` or `Option<SessionInfo>`. With no `#[must_use]`, existing callers that ignore the value still compile.
- `link_to_parent` does not change membership, so it gets no seq bump and no event.
- `register_and_link_to_parent_of_kind` gets one `Started` (optionally carrying `parent`).

**Started on register vs after spawn:** registration precedes spawn (`run.rs:75` vs `:90`). With observer-in-lock, a spawn failure produces `Started` followed by `Exited(None, None)`. That is correct and harmless (two events, net no-op). The alternative is a two-phase "activate after spawn" API, which adds complexity for no visible benefit. **Recommend accepting it.**

## 5. Types, serde, and observer vs wrapper

- `SessionId(Uuid)` derives `Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize` (`core/launch/session/types.rs:19-21`). Display is the UUID string (`:29-33`). In JSON the newtype serializes as the UUID string. Existing Tauri DTOs use `session_id: String` via `to_string()` (`tauri/commands/launch/relay/payloads.rs:87-90,153,175`); follow that.
- `SessionKind { Game, Trainer }` derives `Serialize, Deserialize` with **no `rename_all`** (`types.rs:37-41`), so its serde form is `"Game"`/`"Trainer"`.
  - The Tauri relay maps it to snake_case through its own `InjectionLogSessionKind` (`payloads.rs:130-143`).
  - **Recommend a src-tauri DTO enum emitting `"game"`/`"trainer"`.** Reuse or move `InjectionLogSessionKind`. Do not change the core serde.
- `seq: u64`. It is JSON-safe for JS below 2⁵³.
- `crosshook-core` cannot depend on `crosshook-app` (no such dep: app → core only). Options:
  - **A. App wrapper:** core registry owns seq and returns change records from `register*`/`deregister`, and `LaunchService` publishes. Core stays event-agnostic, but the publish happens outside the lock (the ordering caveat in §4). Every mutation site must remember to publish. There are 3 production sites, plus test callers that won't publish.
  - **B. Core observer hook (recommended):** a core-defined `pub type SessionObserver = Arc<dyn Fn(&SessionChange) + Send + Sync>`, with `LaunchSessionRegistry::with_observer(obs)`. `new()`/`Default` stay observer-less, so standalone test registries are untouched. The observer is called under the lock after mutation and seq bump. The app installs `move |c| bus.publish(AppEvent::from(c))` in `LaunchService::new`. Every path is covered by construction, including test harness calls via `registry()`. Ordering is strict.
- Suggested core surface, all in `core/launch/session/`:
  - `SessionInfo { session_id, kind, profile_key, parent: Option<SessionId> }`
  - `SessionChange::{Started { info, seq }, Exited { info, code: Option<i32>, signal: Option<i32>, seq }}`
  - `fn deregister_with_exit(&self, id, code, signal) -> bool`, with `deregister(id)` = no-exit variant (keeps ~40 test call sites unchanged)
  - `fn snapshot(&self) -> (Vec<SessionInfo>, u64)`
  - Re-export from `core/launch/session/mod.rs:15-18` and `core/launch/mod.rs:62-65`.
  - App: `AppEvent::SessionStarted { session_id, profile_name, kind, seq }` / `SessionExited { session_id, profile_name, code, signal, seq }` per the spec, plus `LaunchService::running_sessions_snapshot() -> (Vec<SessionInfo>, u64)` (next to `running_profiles` in `queries.rs`).
  - Finalizer: `deregister()` → `deregister_with_exit(id, code, signal)`, with `None/None` on drop and in `fail_spawn`.
- Keep `registry.rs` under the ~500-line cap: it is 254 lines now (tests already split out, `:249-254`).

## 6. Test harnesses and ts-rs

- **App launch harness** `app/services/launch/tests/helpers.rs`:
  - `Harness::new` (`:210-256`) builds `LaunchService` over tempdir stores with a scripted `FakeSpawner` (`:117-194`).
  - It provides `proton_request` / `steam_request` (`:258-323`) and `Harness::collect` (drain until `Exited`, 10 s timeout, `:325-344`).
  - Add `events: EventBus` to `Harness` and pass it to the service so tests can `subscribe()`.
- **Blocked-child helpers** in `tests/cascade.rs`, good for start/exit event tests:
  - `start_blocked`, `release`, `wait_for_pid`, `wait_deregistered`, `yield_window`, `is_registered` (`:21-137`)
  - existing `running_profiles` assertions at `:212-239`
  - multi-thread runtime: `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`, `:175`
- **Finalizer unit harness** `tests/finalizer.rs:139-143`: a standalone registry, then `SessionGuard::new(ctx, publisher).finish(Some(status))`. This exercises code/signal propagation without spawning.
- **Core registry tests** `core/launch/session/registry_tests.rs`: the place for seq monotonicity, idempotent-deregister-no-double-event, and snapshot seq equality.
- **Suggested race test** (core, std threads, no tokio needed):
  - Observer pushes into `Mutex<Vec<SessionChange>>`. Simulate a subscriber by recording the vec length at "subscribe" time before the snapshot.
  - Run N threads doing register/deregister loops while a thread repeatedly takes `snapshot()`.
  - For each snapshot, apply the events after the subscribe index with `seq > S` and assert the result equals the registry state at a later quiescent snapshot with matching seq. Also assert seq is strictly increasing.
  - An app-level variant can go through the real `EventBus` (capacity 256 is enough for bursts; on `Resync`, re-snapshot).
- **Bridge payload tests** follow the pattern in `tauri/startup_bridge/tests.rs` (JSON shape asserts).
- **ts-rs:**
  - It exists only in `crosshook-core` behind the off-by-default `ts-rs` feature (`crates/crosshook-core/Cargo.toml:10-11,54`), exported via `core/ts_rs_exports.rs` and `core/bin/ts_rs_export.rs`.
  - Generated files are only `src/types/generated/{health,launch_hooks,onboarding,ts_rs_edge_cases}.ts`.
  - `crosshook-app` has no ts-rs, and **no `AppEvent` payload or Tauri event payload is ts-rs exported**. `profiles-changed` and the `launch-*` payloads are hand-written TS.
  - ⇒ The `launch-session-changed` payload and the snapshot command result need hand-written TS types (frontend scope). Do not add ts-rs for this issue.

## Touch list (Rust)

- `core/launch/session/{registry.rs, types.rs (SessionInfo/SessionChange/observer), mod.rs, registry_tests.rs}` and `core/launch/mod.rs` re-exports
- `app/events.rs` (2 variants), `app/services/launch/{mod.rs (bus param + observer install), queries.rs (snapshot), finalizer.rs (exit-carrying deregister + steam watch), types.rs (SessionContext exe name), run.rs (pass exe name)}`, `app/bootstrap/stores.rs:132,182` (clone the bus before the move), `app/services/launch/tests/helpers.rs:234`, plus new tests
- `tauri/startup_bridge.rs` (+tests): `launch-session-changed` emit plus resync handling
- `tauri/commands/launch/{queries.rs, mod.rs}`: new snapshot command (e.g. `list_running_sessions` → `{ sessions, seq }`), registered in `tauri/lib.rs` `generate_handler!` (~`:128`) and `commands/launch/tests/contract.rs`
- Docs: `crates/crosshook-app/README.md:17` (EventBus variants) and `:109` (LaunchService API)
