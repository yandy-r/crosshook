# Plan: YAN-874 / GH #568 (M1-12): single-instance lock + window activation

Spec and research: [`feature-spec.md`](feature-spec.md). Paths are relative to `src/crosshook-native/` unless noted. Every decision is listed as **D#: decision. _Why:_ reason.**

## Decisions

- **D1:** The lock API lives in `crates/crosshook-app/src/instance_lock.rs` (plus `instance_lock/bus.rs` and `instance_lock/tests.rs`) and is UI-agnostic. _Why:_ the issue mandates the location, and `crosshook-ui` (YAN-880) adopts it unchanged.
- **D2:** `crosshook-app` gains a direct `zbus = { version = "5", default-features = false, features = ["tokio"] }` dependency, and the `nix` features gain `"fs", "inotify", "poll"`. _Why:_ both crates are already in `Cargo.lock` at the same versions, so no new crate is added. `check-app-deps.sh` bans only toolkit crates.
- **D3:** `BootstrapStep::InstanceLock` becomes step 0. `PRE_RUNTIME_ORDER` grows to `[_; 5]`, the name is `"instance_lock"` and the trace marks are `instance_lock:begin`/`instance_lock:end`. _Why:_ this is the existing SEAM(M1-12), and it guarantees the lock is taken before any migration.
- **D4:** Lock contention returns `Err(BootstrapError::AlreadyRunning)` from `pre_runtime` before the gate or any migration. The new unit variant is `#[error("another CrossHook instance is already running")]`. Shells match it **before** the fatal dialog. _Why:_ the enum is already `#[non_exhaustive]`, no new return type is needed, and a second instance never touches the stores.
- **D5:** The lock work goes through a new `PreRuntimeHost::acquire_instance_lock(&mut self, wait: Option<Duration>) -> InstanceLockAttempt` method. _Why:_ every Phase A side effect is behind the seam, so unit tests can fake contention without the filesystem.
- **D6:** The lock path is resolved from `host.env_var("XDG_RUNTIME_DIR")` plus `host.is_flatpak()`, never through `BaseDirs`.
  - Flatpak: `$XDG_RUNTIME_DIR/app/dev.crosshook.CrossHook/instance.lock`.
  - Native: `$XDG_RUNTIME_DIR/crosshook/instance.lock`.

  _Why:_ the read happens before the gate, and trace rule R3 forbids base-dir lookups before `gate:end`.

- **D7:** Lock file and directory safety:
  - The file is opened with `O_RDWR|O_CREAT|O_NOFOLLOW` (std adds `O_CLOEXEC`), mode `0600`, and `fstat` must show a regular file.
  - The lock is `nix::fcntl::Flock::lock(file, LockExclusiveNonblock)`.
  - Missing directories are created with `DirBuilder` mode `0700` (recursive in Flatpak mode).
  - The final directory must be a real directory (no symlink), owned by `geteuid()`, with no group/other write bits.
  - Nothing is ever written to the file.

  _Why:_ a hostile symlink or a world-writable directory must not be able to block or redirect the lock, and an inherited fd would keep the lock alive in game children.

- **D8:** `PreRuntime` carries `pub(crate) instance: InstanceLockState` (`Held(InstanceLock)` or `Unavailable(&'static str)`), which moves into `BootstrapData` and from there into `InstanceGuard` for the process lifetime. _Why:_ this is the orchestrator decision. The kernel releases the lock on death and tao ends the process with `process::exit`, so no cleanup is needed.
- **D9:** Phase B adds `BootstrapStep::InstanceBus` (`"instance_bus"`) right after `Runtime`, before `SettingsStore`. It owns `dev.crosshook.CrossHook` through `zbus::connection::Builder::session()?.serve_at("/dev/crosshook/CrossHook", iface)?.name(..)?.build()` (always `DoNotQueue`), bounded by a 2 s timeout. _Why:_ the name must exist before the slow store init so that racing launches find it. The step needs only the runtime.
- **D10:** `InstanceBus` outcomes and their results:

  | Bus result                                                        | Outcome / result                                                       |
  | ----------------------------------------------------------------- | ---------------------------------------------------------------------- |
  | Owned                                                             | `Ran`                                                                  |
  | `zbus::Error::NameTaken`                                          | `Err(BootstrapError::AlreadyRunning)`, whether or not the lock is held |
  | Connect error or timeout                                          | `Degraded("session bus unavailable")`                                  |
  | `InstanceBus::Skip{reason}`                                       | `Skipped(reason)`                                                      |
  | Called inside a runtime context (`Handle::try_current().is_ok()`) | `Degraded("instance bus needs a non-runtime caller")`                  |

  _Why:_ treating a taken name as a second instance also catches a native dev run alongside the Flatpak (different lock dirs, same bus name). `block_on` inside a runtime would panic.

- **D11:** A new `BootstrapOptions::with_instance_bus(InstanceBus)` option, with `InstanceBus::{Enabled, Skip { reason: &'static str }}` and `Enabled` as the default. _Why:_ `#[tokio::test]` callers pass `Handle::current()` on a current-thread runtime, where `block_on` would panic or deadlock. Tests opt out explicitly, the same way as `UmuRefresh::Skip`.
- **D12:** `DegradedStores.instance_unguarded: bool` (frozen) is `!pre.holds_lock && bus_outcome == Degraded`. A skipped bus counts as guarded. _Why:_ this reuses the existing frozen-report and banner path (the `get_startup_report` pull), and tests that skip the bus stay `false`.
- **D13:** Activations are buffered. `ActivationRelay` stores the latest `ActivationRequest` until `InstanceGuard::set_activation_handler` is called (Tauri `setup`), then delivers it. _Why:_ the name is owned before any window exists, so an early second launch must not be lost.
- **D14:** `ActivationHandler: Send + Sync + 'static { fn activate(&self, request: ActivationRequest); }` is called on a tokio worker, and each UI marshals the call to its own main thread. _Why:_ this keeps `crosshook-app` toolkit-free.
- **D15:** `ActivationRequest::from_platform_data` accepts only `activation-token` and `desktop-startup-id` string values, each at most 512 bytes and without control characters; everything else is dropped. _Why:_ `Activate` can be called by any session peer, so input is bounded and untrusted.
- **D16:** The second instance calls `instance_lock::activate_running_instance(ActivationRequest::from_env(), Duration::from_secs(2)) -> Result<(), ActivationError>`. It is synchronous and runs on its own `new_current_thread().enable_all()` runtime:
  1. Subscribe to `NameOwnerChanged(arg0 = name)`.
  2. If `NameHasOwner` is false, wait for a non-empty new owner.
  3. Call `Activate(a{sv})`.

  The whole sequence shares one deadline. _Why:_ it covers the race where the first instance holds the lock but has not reached Phase B yet, without sleeps, and threads are fine because the process exits.

- **D17:** The shell decides the exit code. `src-tauri/src/activation.rs::exit_second_instance() -> !`:
  - `Ok` → exit 0.
  - `Err(TimedOut)` → `eprintln!("CrossHook is already running but did not respond within 2 s; close it or end the process and try again.")` and exit 1.
  - Other errors → the same message plus the sanitized detail, exit 1.

  _Why:_ `check-startup-path.sh` forbids process exits in app bootstrap, and `fatal.rs` already exits from the shell.

- **D18:** Handoff has two layers:
  - `InstanceLock::release_and_handoff(self, mut cmd: Command) -> Result<Child, (InstanceLock, io::Error)>` sets `CROSSHOOK_INSTANCE_HANDOFF=1` on `cmd`, spawns it, and only then drops the `Flock`.
  - `InstanceGuard::release_and_handoff(&self, cmd).await -> Result<Child, HandoffError>` first awaits `release_name(BUS_NAME)` and drops the connection, then calls the lock handoff.

  _Why:_ if the spawn fails, the lock is still held. The bus name is free before the child's Phase B, so the child does not mistake the old process for a running instance.

- **D19:** A handed-off launch waits for the lock. The step reads `CROSSHOOK_INSTANCE_HANDOFF`; if it is `"1"`, it removes the variable through `PreRuntimeEnv` (so the removal is traced) and passes `wait = Some(5 s)`. `acquire_with_wait` sets an inotify `IN_CLOSE` watch on the lock file **before** the first `LOCK_NB` try, then loops `poll(inotify, remaining)` → retry. If the deadline passes it returns `Contended`, which becomes `AlreadyRunning`. _Why:_ there are no sleeps and no threads in Phase A, the holder's exit or close always fires `IN_CLOSE`, and the variable is not inherited by games.
- **D20:** In Tauri, `ActivationHandler` runs `run_on_main_thread` → `get_webview_window("main")`, then `unminimize()`, `show()`. If a token is present it calls `gtk_window()?.set_startup_id(token)`, which on Wayland calls `xdg_activation_v1_activate(token, surface)` and on X11 terminates startup notification. Finally it calls `set_focus()`, which `present`s on X11 and with no token. _Why:_ tao's `set_focus` uses `GDK_CURRENT_TIME` without the forwarded token, and GNOME/KDE Wayland focus-stealing prevention needs the launcher's token. This path was verified in GTK 3.24 sources.
- **D21:** `src-tauri` gets `[target.'cfg(target_os = "linux")'.dependencies] gtk = "0.18"`. _Why:_ tauri does not re-export gtk, and `gtk 0.18.2` is already in `Cargo.lock`.
- **D22:** No `tauri.conf.json` change; a test pins `app.enableGTKAppId` absent or false. _Why:_ with the flag off, tao registers a GApplication without an id, so it owns no bus name. Enabling it would make GTK try to own `dev.crosshook.CrossHook` and export a competing `org.freedesktop.Application`.
- **D23:** No Flatpak manifest change. _Why:_ a sandboxed app may own its own app-id name by default. Verify this in the manual Flatpak run.
- **D24:** `crosshook-cli` is unchanged. _Why:_ it never calls `pre_runtime`.
- **D25:** Classic shows a session-dismissible `InstanceGuardBanner` when `degraded.instance_unguarded` is true. _Why:_ the spec requires a warning banner, and this mirrors `MetadataStatusBanner`.
- **D26:** `relog_degraded_phase_a` becomes `relog_degraded_before_logging` and re-logs every `Degraded` record recorded before the `Logging` step. _Why:_ the `InstanceBus` step now also runs before the subscriber exists.

## Files

### crosshook-app

| File                                                                   | Change                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-app/Cargo.toml`                                      | `nix` features `["fs","inotify","poll","process","signal","user"]`; add `zbus` (D2); add `[[test]] name = "instance_lock_process"` (harness default) only if needed for filters; no other dependency.                                                                                                                                                                                                   |
| `src/lib.rs`                                                           | `pub mod instance_lock;`                                                                                                                                                                                                                                                                                                                                                                                |
| `src/instance_lock.rs` (new, ≤400 lines)                               | Constants, paths, `InstanceLock`, `acquire_with_wait`, `InstanceLockAttempt`, `InstanceLockState`, `InstanceGuard`, `HandoffError`. `mod bus; pub use bus::*;` and `#[cfg(test)] mod tests;`                                                                                                                                                                                                            |
| `src/instance_lock/bus.rs` (new, ≤300 lines)                           | `ActivationRequest`, `ActivationHandler`, `ActivationRelay`, `ApplicationIface`, `claim_bus_name`, `BusClaim`, `activate_running_instance`, `ActivationError`.                                                                                                                                                                                                                                          |
| `src/instance_lock/tests.rs` (new)                                     | Unit tests (see Tests).                                                                                                                                                                                                                                                                                                                                                                                 |
| `src/bootstrap/steps.rs`                                               | Add `InstanceLock` (first variant) and `InstanceBus` (after `Runtime`); `PRE_RUNTIME_ORDER: [_; 5]` starting with `InstanceLock`; names `"instance_lock"`, `"instance_bus"`; update the doc comment.                                                                                                                                                                                                    |
| `src/bootstrap/steps/tests.rs`                                         | Length 5; `swapped_steps_fail_verification` builds a log without `InstanceLock` → `index 0, expected InstanceLock, found LegacyAppIdMigration`.                                                                                                                                                                                                                                                         |
| `src/bootstrap/host.rs`                                                | Add the trait method `fn acquire_instance_lock(&mut self, wait: Option<Duration>) -> InstanceLockAttempt;` (doc: must not spawn threads).                                                                                                                                                                                                                                                               |
| `src/bootstrap/env_policy.rs`                                          | `SystemHost` impl: `InstanceLockPaths::resolve(std::env::var_os("XDG_RUNTIME_DIR").as_deref(), self.is_flatpak())`, then `try_acquire` or `acquire_with_wait`, mapped into `InstanceLockAttempt`.                                                                                                                                                                                                       |
| `src/bootstrap/pre_runtime.rs`                                         | Replace the SEAM with `let instance = step_instance_lock(host, &mut log)?;` before the gate, in both the production and misordered paths (the misordered path keeps the lock first). `PreRuntime` gets `instance`, `into_parts` returns it, and `pub fn holds_instance_lock(&self) -> bool` is added. Update the module and doc lists (step 0).                                                         |
| `src/bootstrap/pre_runtime/tests.rs`                                   | `RecordingHost` gets `lock: FakeLock { Acquire(TempDir), Contended, Unavailable }` (default `Acquire` using `InstanceLockPaths::in_dir`) and records the call `"instance_lock"`/`"instance_lock:wait"`. Update every expected `outcomes_of` and `calls` vector (prepend `Ran`/`"instance_lock"`). Add new tests (see Tests).                                                                            |
| `src/bootstrap/error.rs` (+`error/tests.rs`)                           | `AlreadyRunning` variant; test its `user_message()`.                                                                                                                                                                                                                                                                                                                                                    |
| `src/bootstrap/options.rs`                                             | `pub enum InstanceBus { Enabled, Skip { reason: &'static str } }`, field `instance_bus`, default `Enabled`, `with_instance_bus`.                                                                                                                                                                                                                                                                        |
| `src/bootstrap/instance.rs` (new, ≤120 lines)                          | `pub(super) fn instance_bus_step(app: &App, choice: InstanceBus, steps: &mut StepLog) -> Result<Option<BusClaim>, BootstrapError>` (D9/D10); `fn unguarded(held: bool, steps: &StepLog) -> bool` (D12). Keeps `app_bootstrap.rs` under 350 lines.                                                                                                                                                       |
| `src/bootstrap/app_bootstrap.rs`                                       | `into_parts` → `(steps, mode, migration, instance)`; after `steps.push(Runtime)` call `instance::instance_bus_step(&app, opts.instance_bus, &mut steps)?`; build `InstanceGuard::new(instance, bus_claim, app.runtime_handle())`; store it in `BootstrapData`; add `pub fn instance(&self) -> &InstanceGuard` on `BootstrappedApp`; set `instance_unguarded` in `DegradedStores`; apply the D26 rename. |
| `src/bootstrap/mod.rs`                                                 | Update the ordering-contract doc (0. instance lock; Phase B instance bus); `pub use options::InstanceBus;`                                                                                                                                                                                                                                                                                              |
| `src/bootstrap/report.rs`                                              | `DegradedStores.instance_unguarded: bool` with a doc comment.                                                                                                                                                                                                                                                                                                                                           |
| `src/bootstrap/report/tests.rs`, `src/services/startup_scans/tests.rs` | Add `instance_unguarded: false` to the literals; pin the JSON key in the report serde test.                                                                                                                                                                                                                                                                                                             |
| `src/bootstrap/ui_import/tests.rs`                                     | Literal `PreRuntime { …, instance: InstanceLockState::Unavailable("test") }`, steps from the 5-element order, and `.with_instance_bus(InstanceBus::Skip { reason: "ui_import test" })`.                                                                                                                                                                                                                 |
| `tests/bootstrap_report.rs`                                            | `FakeFlatpakHost::acquire_instance_lock` → `Unavailable("test host")`; `test_options` adds `.with_instance_bus(Skip)`; the full-sequence assertion becomes 18 steps (`InstanceLock` first, `InstanceBus` after `Runtime`); assert `report.degraded.instance_unguarded == false`.                                                                                                                        |
| `tests/bootstrap_ordering.rs`                                          | See Tests T9.                                                                                                                                                                                                                                                                                                                                                                                           |
| `tests/instance_lock_process.rs` (new)                                 | See Tests T6–T8.                                                                                                                                                                                                                                                                                                                                                                                        |
| `tests/instance_activation.rs` (new)                                   | See Tests T10.                                                                                                                                                                                                                                                                                                                                                                                          |
| `README.md` (crate)                                                    | Bootstrap: Phase A step 0, Phase B instance bus, `AlreadyRunning`, step count 18. New "Single instance" section: paths, D7, activation, handoff, runtime-only storage.                                                                                                                                                                                                                                  |

### Public types (`crosshook_app::instance_lock`)

```rust
pub const APP_ID: &str = "dev.crosshook.CrossHook";            // bus name and Flatpak dir
pub const OBJECT_PATH: &str = "/dev/crosshook/CrossHook";
pub const HANDOFF_ENV: &str = "CROSSHOOK_INSTANCE_HANDOFF";
pub const HANDOFF_WAIT: Duration = Duration::from_secs(5);
pub const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(2);

pub struct InstanceLockPaths { dir: PathBuf, file: PathBuf }
impl InstanceLockPaths {
    pub fn resolve(runtime_dir: Option<&OsStr>, flatpak: bool) -> Result<Self, LockUnavailable>; // None/relative → RuntimeDirUnavailable
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self;  // native layout under `dir`; tests + harness
    pub fn file(&self) -> &Path;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockUnavailable { RuntimeDirUnavailable, RuntimeDirUnsafe, Io }
impl LockUnavailable { pub fn reason(self) -> &'static str } // "runtime dir unavailable" | "runtime dir unsafe" | "instance lock unavailable"

#[derive(Debug)]
pub enum AcquireError { Contended, Unavailable(LockUnavailable, Option<io::Error>) }

#[derive(Debug)]
pub struct InstanceLock { flock: Flock<File>, paths: InstanceLockPaths }
impl InstanceLock {
    pub fn try_acquire(paths: &InstanceLockPaths) -> Result<Self, AcquireError>;             // D7
    pub fn acquire_with_wait(paths: &InstanceLockPaths, timeout: Duration) -> Result<Self, AcquireError>; // D19
    pub fn release_and_handoff(self, cmd: Command) -> Result<Child, (Self, io::Error)>;     // D18
}

pub enum InstanceLockAttempt { Acquired(InstanceLock), Contended, Unavailable(&'static str) }
pub enum InstanceLockState { Held(InstanceLock), Unavailable(&'static str) }

pub struct InstanceGuard { lock: Mutex<Option<InstanceLock>>, bus: tokio::sync::Mutex<Option<BusClaim>>, relay: Arc<ActivationRelay>, unguarded: bool }
impl InstanceGuard {
    pub(crate) fn new(state: InstanceLockState, bus: Option<BusClaim>, relay: Arc<ActivationRelay>) -> Self;
    pub fn holds_lock(&self) -> bool;
    pub fn owns_bus_name(&self) -> bool;
    pub fn is_unguarded(&self) -> bool;
    pub fn set_activation_handler(&self, handler: impl ActivationHandler);   // D13 delivers pending
    pub async fn release_and_handoff(&self, cmd: Command) -> Result<Child, HandoffError>; // D18
}
#[derive(Debug, thiserror::Error)]
pub enum HandoffError { #[error("instance lock not held")] NotHeld, #[error("could not start the other CrossHook: {0}")] Spawn(io::Error) }
static_assertions::assert_impl_all!(InstanceGuard: Send, Sync);

// bus.rs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivationRequest { pub activation_token: Option<String>, pub desktop_startup_id: Option<String> }
impl ActivationRequest {
    pub fn from_env() -> Self;                                      // reads XDG_ACTIVATION_TOKEN / DESKTOP_STARTUP_ID (read-only)
    pub fn from_platform_data(data: &HashMap<String, OwnedValue>) -> Self; // D15
    pub fn startup_id(&self) -> Option<&str>;                       // token first, then startup id
    fn platform_data(&self) -> HashMap<&'static str, Value<'_>>;
}
pub trait ActivationHandler: Send + Sync + 'static { fn activate(&self, request: ActivationRequest); }
pub(crate) struct ActivationRelay { state: std::sync::Mutex<RelayState> } // deliver(req), set_handler(h)
struct ApplicationIface { relay: Arc<ActivationRelay> }
#[zbus::interface(name = "org.freedesktop.Application")]
impl ApplicationIface { fn activate(&self, platform_data: HashMap<String, OwnedValue>) { … } }

pub struct BusClaim { connection: zbus::Connection, name: String }
pub enum ClaimOutcome { Owned(BusClaim), AlreadyOwned, Unavailable(String) }
pub(crate) async fn claim_bus_name(relay: Arc<ActivationRelay>) -> ClaimOutcome;  // APP_ID, OBJECT_PATH, 2 s
#[cfg(feature = "test-seams")] #[doc(hidden)]
pub async fn claim_bus_name_at(name: &str, relay_handler: impl ActivationHandler) -> ClaimOutcome; // T10 unique names
pub fn activate_running_instance(request: ActivationRequest, timeout: Duration) -> Result<(), ActivationError>; // D16
#[cfg(feature = "test-seams")] #[doc(hidden)]
pub async fn activate_named(name: &str, request: ActivationRequest, timeout: Duration) -> Result<(), ActivationError>;
#[derive(Debug, thiserror::Error)]
pub enum ActivationError { #[error("session bus unavailable: {0}")] Bus(String), #[error("timed out")] TimedOut, #[error("activation rejected: {0}")] Rejected(String) }
```

`activate_running_instance` builds its runtime and calls the same body as `activate_named(APP_ID, …)`, so production and tests share one code path. Error strings go through `presenter::sanitize_message`.

### Phase A step (in `pre_runtime.rs`)

```rust
fn step_instance_lock(host: &mut dyn PreRuntimeHost, log: &mut StepLog) -> Result<InstanceLockState, BootstrapError> {
    trace::mark("instance_lock:begin");
    let handoff = host.env_var(OsStr::new(HANDOFF_ENV)).is_some_and(|v| v == "1");
    if handoff { PreRuntimeEnv::new(host).remove(HANDOFF_ENV); }
    let attempt = host.acquire_instance_lock(handoff.then_some(HANDOFF_WAIT));
    let (state, outcome) = match attempt {
        InstanceLockAttempt::Acquired(lock) => (InstanceLockState::Held(lock), StepOutcome::Ran),
        InstanceLockAttempt::Contended => { trace::mark("instance_lock:end"); return Err(BootstrapError::AlreadyRunning); }
        InstanceLockAttempt::Unavailable(reason) => { warn + eprintln!("CrossHook: instance lock unavailable ({reason}); relying on the session bus"); (InstanceLockState::Unavailable(reason), StepOutcome::Degraded(reason)) }
    };
    log.push(BootstrapStep::InstanceLock, outcome);
    trace::mark("instance_lock:end");
    Ok(state)
}
```

`run_pre_runtime` calls this right after `note_entry_threads` (the SEAM line). Note that `PreRuntimeEnv::new(host)` borrows `host` mutably only for the removal.

### src-tauri

| File                                            | Change                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/Cargo.toml`                          | D21.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `src-tauri/src/activation.rs` (new, ≤120 lines) | `pub(crate) struct TauriActivation(AppHandle)` implementing `ActivationHandler` (D20); `pub(crate) fn exit_second_instance() -> !` (D17); `const MAIN_WINDOW: &str = "main";`                                                                                                                                                                                                                                                                                                                                                         |
| `src-tauri/src/activation/tests.rs` (new)       | Pin D22 (`include_str!("../../tauri.conf.json")` parsed with `serde_json`: `app.enableGTKAppId` absent or false, `identifier == instance_lock::APP_ID`).                                                                                                                                                                                                                                                                                                                                                                              |
| `src-tauri/src/lib.rs`                          | `mod activation;` In the `pre_runtime` and `App::bootstrap` matches, add an `Err(BootstrapError::AlreadyRunning) => activation::exit_second_instance()` arm before the fatal arm (import `BootstrapError`, `InstanceBus` is the default `Enabled`). In `.setup`, add `core.instance().set_activation_handler(activation::TauriActivation::new(app.handle().clone()));` before `startup_bridge::spawn`; `core` is moved into setup today, so take `let instance_core = core.clone();` beforehand or call it on `core` before the move. |
| `src-tauri/src/env_policy/tests.rs`             | `FakeHost::acquire_instance_lock` → `InstanceLockAttempt::Unavailable("test host")`.                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

### Classic frontend

| File                                                                    | Change                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/types/startup.ts`                                                  | `instance_unguarded: boolean` in `DegradedStores`.                                                                                                                                                                                                                                                                                                                                                                                     |
| `src/lib/mocks/handlers/startup.ts`, `src/test/startupReportFixture.ts` | `instance_unguarded: false`.                                                                                                                                                                                                                                                                                                                                                                                                           |
| `src/components/layout/InstanceGuardBanner.tsx` (new)                   | Uses memoized `fetchStartupReport()`; shows `crosshook-warning-banner` with `role="status"`, `aria-label="Single-instance warning"` and the text "CrossHook could not confirm it is the only running copy (no runtime directory or session bus). Avoid opening a second CrossHook window." Dismiss button stores `crosshook.instance.unguarded.dismissed=1` in sessionStorage; catches storage errors like `useFlatpakMigrationToast`. |
| `src/components/layout/AppShell.tsx`                                    | Render `<InstanceGuardBanner />` next to `<MetadataStatusBanner />`.                                                                                                                                                                                                                                                                                                                                                                   |
| `src/components/layout/__tests__/InstanceGuardBanner.test.tsx` (new)    | Hidden when false; shown when true; dismiss hides it and persists for the session.                                                                                                                                                                                                                                                                                                                                                     |

### Docs

- `CLAUDE.md` and `AGENTS.md`, **Startup contract** bullet: change it to "`pre_runtime` (instance lock (step 0; contention → `BootstrapError::AlreadyRunning`, shell activates the running instance over D-Bus and exits) → Flatpak gate …)", and add "`App::bootstrap` (runtime → instance bus name `dev.crosshook.CrossHook` → stores …)". Add one sentence to the Architecture bullet's `crosshook-app` list: "single-instance lock (`instance_lock`)".
- `crates/crosshook-app/README.md`: as listed in the crosshook-app table above.
- Storage classification: the runtime-only entries are recorded in `feature-spec.md`, and the research doc §10 already classifies the instance lock. No ADR is needed, because nothing is persisted and there is no new portal.
- `CHANGELOG.md`: do not edit (git-cliff generates it from the PR title).

## Tests

All Rust tests use tempdir runtime dirs and never touch the real `$XDG_RUNTIME_DIR`.

**Unit tests (`instance_lock/tests.rs`).** These cover the acceptance criterion "unit tests use a tempdir runtime dir: acquire, contend, release on drop, and handoff".

- **T1 `resolve_*`:**
  - Native gives `<rt>/crosshook/instance.lock`; Flatpak gives `<rt>/app/dev.crosshook.CrossHook/instance.lock`.
  - `None`, an empty value or a relative path gives `RuntimeDirUnavailable`.
- **T2 `acquire_creates_private_dir_and_empty_file`:** the directory mode is `0700`, the file mode is `0600`, and the file length is 0.
- **T3 `second_acquire_is_contended` and `drop_releases`:** acquire A, then `try_acquire` B returns `Contended`. Drop A, and B succeeds. This works in one process because flock conflicts between separate open file descriptions.
- **T4 `rejects_unsafe_dir`:** a directory with mode `0777` gives `RuntimeDirUnsafe`. A symlinked lock dir gives `RuntimeDirUnsafe`. A symlinked lock file gives `Unavailable(Io)` (ELOOP).
- **T5 `wait_*`:**
  - Hold A, start `acquire_with_wait(B, 5 s)` in a spawned thread, drop A after about 200 ms: B succeeds in well under 5 s.
  - Hold A, then `acquire_with_wait(B, 300 ms)` returns `Contended` at or after 300 ms.
- **`relay_*`:** a request delivered before the handler is set is delivered exactly once on `set_handler`; later requests go directly to the handler; only the latest pending request is kept.
- **`platform_data_*`:** token and startup id are extracted; non-string values, over-long values, control characters and unknown keys are dropped; `startup_id()` prefers the token.

**Phase A unit tests (`bootstrap/pre_runtime/tests.rs`).**

- Every existing test gets `InstanceLock` (`Ran`) as its first step.
- `contended_lock_stops_before_any_migration`: `RecordingHost{lock: Contended}` in Flatpak isolated mode returns `Err(AlreadyRunning)`, and `calls == ["instance_lock"]` (no `flatpak_migration`, no `legacy_app_id`).
- `unavailable_lock_degrades_and_continues`: the first outcome is `Degraded("runtime dir unavailable")` and the other four steps run.
- `handoff_env_waits_and_is_removed`: the env contains `HANDOFF_ENV=1`, so the calls start with `"remove:CROSSHOOK_INSTANCE_HANDOFF"` and then `"instance_lock:wait"`, and the env no longer contains the key.
- The misordered test now expects `[InstanceLock, LegacyAppIdMigration, FlatpakGate, FlatpakMigration, EnvPolicy]`, the checker fails at index 1, and the calls are `["instance_lock", "legacy_app_id", "flatpak_migration"]`.

**Process integration (`tests/instance_lock_process.rs`, standard harness).** Child processes re-execute the test binary with `current_exe()`, `--exact <child_test> --nocapture` and an env var naming the tempdir. A child test returns immediately when its env var is unset.

- **T6 `kill_9_releases_lock` (acceptance criterion: "after kill -9 of the first instance, the next start acquires the lock immediately"):**
  1. The child `child_hold_lock` acquires the lock, prints `LOCKED`, and then blocks on stdin.
  2. The parent reads `LOCKED` and checks that `try_acquire` returns `Contended`.
  3. The parent calls `child.kill()` (SIGKILL) and `wait()`.
  4. `try_acquire` now succeeds on the first try.
- **T7 `lock_fd_not_inherited`:** acquire the lock, spawn `cat` with piped stdin, drop the lock. `try_acquire` succeeds while `cat` is still alive (this proves `O_CLOEXEC`). Then kill `cat`.
- **T8 `release_and_handoff_transfers_lock`:**
  1. The parent acquires the lock and calls `release_and_handoff(Command::new(current_exe()).args([... "child_wait_handoff"]))`.
  2. The child asserts `HANDOFF_ENV == "1"`, calls `acquire_with_wait(5 s)`, and exits 0 only if it got the lock.
  3. The parent asserts the child exits successfully.
  4. Spawn-error case: a nonexistent program returns `Err((lock, _))`, and the returned lock is still held (a second acquire is `Contended`).

**Ordering harness (`tests/bootstrap_ordering.rs`).** Covers the acceptance criterion "the lock is taken before any migration".

- The child sets `XDG_RUNTIME_DIR=<root>/runtime` (created with mode `0700`; in the `contended` scenario the parent-provided `CROSSHOOK_ORDERING_RUNTIME` is used instead), removes `CROSSHOOK_INSTANCE_HANDOFF`, sets `DBUS_SESSION_BUS_ADDRESS=unix:path=<root>/no-bus` defensively, and passes `.with_instance_bus(InstanceBus::Skip { reason: "ordering test" })`.
- R2 gains `instance_lock:begin` and `instance_lock:end` at the front.
- New **R6**: `instance_lock:end` comes before `flatpak_migration:begin`, before `legacy_app_id:begin`, and before every `BaseDirsLookup`. It is evaluated independently, has a `synthetic_r6_violation()` self-check (lock marks after `flatpak_migration:begin`) asserted like R4, and is added to `check_trace`.
- New scenario `contended`:
  1. The parent creates a tempdir, holds `InstanceLock::try_acquire(&InstanceLockPaths::in_dir(dir))`, and runs the child with `CROSSHOOK_ORDERING_RUNTIME=dir`.
  2. The child expects `pre_runtime` to return `Err(AlreadyRunning)`.
  3. Its trace must contain `instance_lock:begin`/`instance_lock:end` and must not contain `gate:begin`, `flatpak_migration:begin`, `legacy_app_id:begin`, `pre:done` or any `BaseDirsLookup`.
  4. The parent runs this scenario in a separate loop after the four existing ones.
- Native-mode `in_dir` matches `resolve(Some(dir), false)`, so the child's `SystemHost` finds the parent's lock.

**D-Bus round trip (`tests/instance_activation.rs`, `#[tokio::test(flavor = "multi_thread")]`).**

- **T10 setup:** connect to `zbus::Connection::session()` within 2 s. On failure, print `eprintln!("skip: no session bus: {e}")` and return (the repo pattern from `csv_coverage.rs`). Use a unique name, `dev.crosshook.CrossHookTest.p<pid>`, so a real CrossHook is never touched.
- **T10 cases:**
  1. `claim_bus_name_at(name, recorder)` returns `Owned`.
  2. `activate_named(name, {token:"tok"}, 2 s)` returns `Ok`, and the recorder receives `activation_token == Some("tok")`.
  3. A second claim returns `AlreadyOwned`.
  4. `activate_named("…p<pid>.none", _, 300 ms)` returns `Err(TimedOut)`.
  5. Late owner: start `activate_named`, then claim the name about 100 ms later; the activation succeeds (D16).
- Locally run it under `dbus-run-session -- cargo test …`. CI has no session bus, so the test skips with a reason. Wrapping it in CI is optional and not part of this PR.

**src-tauri.** The D22 pin test, and `env_policy/tests.rs` still passes.

**Frontend.** `InstanceGuardBanner.test.tsx`; `AppShell.startup.test.tsx` still passes with the updated fixture.

## Implementation order

1. Cargo dependencies (D2, D21).
2. `instance_lock.rs` and `bus.rs` with their unit tests.
3. `steps.rs` and `host.rs` changes, `SystemHost`, `pre_runtime.rs`, `error.rs`, plus their tests.
4. `options.rs`, `bootstrap/instance.rs`, `app_bootstrap.rs`, `report.rs`, and the report literals.
5. Integration tests (`bootstrap_report`, `bootstrap_ordering`, `instance_lock_process`, `instance_activation`).
6. src-tauri: `activation.rs`, `lib.rs`, the fake host.
7. Frontend types, mocks and banner.
8. Docs.

Steps 1–5 form one crate slice; 6 and 7 can run in parallel after step 4's types exist.

## Validation

```bash
cd src/crosshook-native
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p crosshook-app instance_lock
cargo test -p crosshook-app --test bootstrap_ordering --test bootstrap_report --test instance_lock_process --test instance_activation
dbus-run-session -- cargo test -p crosshook-app --test instance_activation   # local, real round trip
cargo test -p crosshook-app -p crosshook-native -p crosshook-cli -p crosshook-view-model
npm run typecheck && npm run lint && npm test -- InstanceGuardBanner AppShell
cd ../.. && ./scripts/check-startup-path.sh && ./scripts/check-host-gateway.sh && ./scripts/check-app-deps.sh && ./scripts/lint.sh
```

Manual checks (record the results in the PR):

1. **Native dev:** start one instance, then a second. The second focuses the first and `echo $?` prints 0. `kill -9` the first; the next start comes up immediately.
2. **Timeout:** stop the first instance with `kill -STOP`, then start a second. It prints the timeout message and exits 1.
3. **Flatpak:** steps 1 and 2 again; check that no own-name permission error appears (D23).
4. **Wayland focus:** GNOME Wayland and KDE Wayland, launching the second instance from the app grid (token forwarded). The window is raised, not just a "ready" notification.
5. **Unguarded mode:** start with `XDG_RUNTIME_DIR` unset and `DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent`. The banner appears.

## Risks

- **GApplication conflict:** there is none today (D22), but a future `enableGTKAppId: true` would compete for the name. The pin test turns that into a test failure. If the flag is ever needed, it requires a redesign: either route activation through GTK's own `org.freedesktop.Application` or use a different identifier.
- **Wayland focus without a token** (launched from a terminal, so no `XDG_ACTIVATION_TOKEN`): GNOME may only show "CrossHook is ready". This is a compositor policy and is expected.
- **Native dev and Flatpak** use different lock files. The bus name still makes them mutually exclusive (D10), but only when a session bus exists.
- **Startup race:** a second launch that arrives before the first instance owns the name waits on `NameOwnerChanged` within the 2 s budget (D16). If Phase A of the first instance takes longer than 2 s, the second prints the timeout message and exits 1. This is acceptable.
- **zbus tokio backend** needs a live runtime. Phase B uses the shell's multi-thread runtime, and the second instance uses its own runtime inside `block_on` (D16). `#[tokio::test]` callers must pass `InstanceBus::Skip` (D11).
- **Bench runs** reuse the real `XDG_RUNTIME_DIR`, so a bench run while CrossHook is open activates that instance and exits 0. Close CrossHook before benching; this is documented in the crate README.
