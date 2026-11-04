# YAN-874 / GH #568 (M1-12): cross-binary single-instance lock with window activation

Paths are relative to `src/crosshook-native/` unless absolute or repo-rooted (`scripts/`, `docs/`, `packaging/`).

## Problem

Classic (`crosshook-native`, Tauri) and the future native UI (`crosshook-ui`, YAN-880, not started) share one app-id and the same stores. Per-process state includes `LaunchSessionRegistry`, the gamescope watchdogs, and the job registry, and `SettingsStore::save` only has an in-process mutex. If two UIs run at the same time they can double-launch a game, orphan watchdogs, or race on migrations and settings writes. Nothing guards against this today.

## Behaviour (user-visible)

| Situation                                                          | Result                                                                                                                                                                     |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| First launch                                                       | Takes the lock in Phase A step 0 and owns the session-bus name `dev.crosshook.CrossHook` in Phase B. Starts normally.                                                      |
| Second launch (either binary) while one runs                       | Phase A fails the lock **before any migration**. Calls `org.freedesktop.Application.Activate` on the running instance, which un-minimizes and focuses its window. Exits 0. |
| Second launch, and the running instance does not answer within 2 s | Prints `CrossHook is already running but did not respond within 2 s…` to stderr and exits 1. No dialog.                                                                    |
| First instance killed with `kill -9` or crashed                    | The kernel releases the flock. The next launch acquires it immediately, with no stale-lock cleanup.                                                                        |
| `$XDG_RUNTIME_DIR` unset, relative, unsafe or unwritable           | The lock step is `Degraded` (warning in the log). The D-Bus name alone guards: if the name is already owned in Phase B, the process activates the owner and exits 0.       |
| No lock **and** no session bus                                     | Starts unguarded. The bus step is `Degraded`, `degraded.instance_unguarded = true` in the startup report, and Classic shows a dismissible warning banner.                  |
| "Open in Classic UI" handoff (M1-19, API only now)                 | `release_and_handoff(cmd)` releases the bus name, spawns the other binary with `CROSSHOOK_INSTANCE_HANDOFF=1`, then unlocks. The new binary waits up to 5 s for the lock.  |
| `crosshook-cli`                                                    | Never takes the lock. It does not call `pre_runtime` (verified: no bootstrap use in `crates/crosshook-cli/src`).                                                           |

## Research findings (facts the plan depends on)

1. **Bootstrap.** `crates/crosshook-app/src/bootstrap/pre_runtime.rs` has `// SEAM(M1-12): instance lock acquires here (step 0)` right after `note_entry_threads`, before `StepLog::new()`. Steps use `trace::mark("<name>:begin/:end")` plus `log.push(step, outcome)`. `BootstrapStep::PRE_RUNTIME_ORDER: [_; 4]` drives `StepLog::verify_pre_runtime_order` and `relog_degraded_phase_a`. `PreRuntime { steps, mode, migration }` is consumed by `App::bootstrap` through `into_parts()`. `PreRuntimeOptions` and `BootstrapError` are `#[non_exhaustive]`.
2. **Host seam.** Every Phase A side effect goes through `PreRuntimeHost` (`bootstrap/host.rs`). It has four implementations: `SystemHost` (in `bootstrap/env_policy.rs`), `RecordingHost` (`bootstrap/pre_runtime/tests.rs`), `FakeFlatpakHost` (`tests/bootstrap_report.rs`) and `FakeHost` (`src-tauri/src/env_policy/tests.rs`). The trait is `pub` only under the `test-seams` feature.
3. **Ordering harness.** `tests/bootstrap_ordering.rs` (`harness = false`) re-executes itself per scenario (`native`, `flatpak-isolated`, `flatpak-host-xdg`, `misordered`) with HOME/XDG pointed at a tempdir. It drives the real `SystemHost` and checks trace rules R1–R5. It does **not** isolate `XDG_RUNTIME_DIR` or `DBUS_SESSION_BUS_ADDRESS` today, so both must be isolated once the lock and bus exist.
4. **Degraded UI path.** `StartupReport.degraded: DegradedStores { metadata, metadata_error, settings_unreadable, logging_unavailable }` is frozen at bootstrap and pulled by Classic through `get_startup_report` (`src/lib/startupReport.ts`, memoized). Banners are app-level components in `src/components/layout/` (e.g. `MetadataStatusBanner`, session-dismissible). The TS contract is `src/types/startup.ts`. Mocks are in `src/lib/mocks/handlers/startup.ts` and the fixture in `src/test/startupReportFixture.ts`. Other `DegradedStores` literals: `bootstrap/app_bootstrap.rs:125`, `bootstrap/report/tests.rs` (×2), `services/startup_scans/tests.rs:40`.
5. **zbus.** `crosshook-core` depends on `zbus 5.14` with `default-features = false, features = ["tokio"]` and `zvariant 5`. All usage is async: `zbus::Connection::session().await`, `zbus::Proxy::new(..)`, wrapped in `tokio::time::timeout(2 s)` (see `platform/portals/documents.rs`). The tokio backend spawns its internal tasks on the **current** tokio runtime, so a connection must live on a runtime that keeps running. `connection::Builder::name()` always uses `DoNotQueue`, and `RequestNameReply::Exists` maps to `zbus::Error::NameTaken`. `zbus::interface` is exported unconditionally (`zbus_macros` is not optional). `crosshook-app` has **no** direct zbus dependency.
6. **GApplication / tao (risk check).** tao 0.35.3 `EventLoop::new_gtk` calls `gtk::Application::new(app_id, ApplicationFlags::empty())` and `register()`. Tauri 2.11.2 passes `app_id = Some(identifier)` **only if** `app.enableGTKAppId` is true (`tauri/src/app.rs:2272`, default `false`). `src-tauri/tauri.conf.json` does not set it, so tao registers a GApplication **without an id**: no bus name, no `/dev/crosshook/CrossHook` object. **No conflict today.** Setting `enableGTKAppId: true` later would make GTK try to own `dev.crosshook.CrossHook` and export its own `org.freedesktop.Application`. A src-tauri test will pin the config. winit (the future `crosshook-ui`) registers no D-Bus name.
7. **Focus and activation token (Tauri).** `WebviewWindow::{unminimize, show, set_focus}` exist. `set_focus` becomes tao `present_with_time(GDK_CURRENT_TIME)`. On Wayland, GDK then asks for a **new** token from the last input serial, which a process with no user input does not have, so GNOME may only show a "CrossHook is ready" notice. `WebviewWindow::gtk_window() -> gtk::ApplicationWindow` (main thread only). In GTK 3.24, `gtk_window_set_startup_id(id)` on a mapped window with a real id calls `gdk_window_set_startup_id`, which on Wayland calls `xdg_activation_v1_activate(token, surface)` (verified in `gdk/wayland/gdkwindow-wayland.c`). An X11 `…_TIME<ts>` id gives `present_with_time(ts)`. So the forwarded token must go through `set_startup_id`. `src-tauri` needs a direct `gtk = "0.18"` dependency, which is already in `Cargo.lock` (tauri does not re-export gtk).
8. **Lint constraints.**
   - `scripts/check-startup-path.sh` forbids `sleep(`, `Duration::from_millis(` and `process::exit/abort/exit(` in `crates/crosshook-app/src/bootstrap/**`, and forbids `set_var/remove_var` anywhere in `crosshook-app/src` except `bootstrap/pre_runtime.rs` and `bootstrap/env_policy.rs`. It also scans `src-tauri/src/lib.rs` for sleeps.
   - `scripts/check-host-gateway.sh` flags only denylisted host tools (`proton`, `flatpak-spawn`, `xdg-open`, …) in `Command::new` literals. Spawning our own sibling binary from a caller-provided `Command` is fine.
   - `scripts/check-app-deps.sh` bans toolkit crates (gtk/glib/tauri/…) in crosshook-app's tree, not zbus or nix.
9. **Flatpak.** The manifest (`packaging/flatpak/dev.crosshook.CrossHook.yml`) has `--talk-name=org.freedesktop.Flatpak` and `org.freedesktop.portal.Desktop`, and no session-bus own-name entries. Flatpak lets an app own (and talk to) its own app-id name by default, so no manifest change is needed. [INFERENCE from Flatpak sandbox-permission docs; verify in the manual Flatpak run.] `$XDG_RUNTIME_DIR/app/dev.crosshook.CrossHook/` is created by Flatpak and shared by every sandbox instance of the app.
10. **Bench.** `scripts/bench/bench_guard.py` keeps the real `XDG_RUNTIME_DIR` when it exists, which Wayland needs. A bench run while a real CrossHook is open will therefore activate that instance and exit 0. This is a documented limitation, not a code change.
11. **Existing skip-with-reason test pattern.** `crosshook-core/src/launch/preview/tests/csv_coverage.rs:112` uses `eprintln!("skip: …"); return;`. The D-Bus round-trip test reuses this pattern.

## Storage boundary

- **Runtime-only.** `instance.lock` lives on `$XDG_RUNTIME_DIR` (tmpfs). It is empty: no TOML, no SQLite, nothing persisted across reboots. A leftover file without a held lock is simply re-locked.
- **Runtime-only.** The D-Bus name and the activation relay are ephemeral.
- **Runtime-only.** `degraded.instance_unguarded` in the startup report is frozen per process and never written to disk.

## Persistence & usability

- **Migration and backward compatibility:** none. Older builds don't take the lock, which is acceptable because both binaries ship together from alpha.1.
- **Offline:** no network.
- **Degraded:** see the behaviour table above.
- **User visibility:** a second launch focuses the existing window. The unguarded mode shows a banner. Users have nothing to edit.

## Security / stability decisions

- **Lock directory.** The native dir `$XDG_RUNTIME_DIR/crosshook` is created with mode `0700`. Whether it already existed or was just created, it must be a real directory (`symlink_metadata`, no symlink), owned by `geteuid()`, and have no group/other write bits. Otherwise the step is `Degraded("runtime dir unsafe")`. _Why:_ another user or a hostile symlink must not be able to block or redirect the lock.
- **Lock file.** Opened `O_RDWR|O_CREAT|O_CLOEXEC|O_NOFOLLOW`, mode `0600`, and checked to be a regular file with `fstat`. Nothing is written to it. _Why:_ without `O_CLOEXEC`, spawned children (games, the handed-off binary) would inherit the open file description and keep the lock alive after CrossHook exits. `O_NOFOLLOW` blocks symlink swaps.
- **`$XDG_RUNTIME_DIR`.** Read through the host seam (`env_var`), never through `BaseDirs`. _Why:_ the read happens before the Flatpak gate, and trace rule R3 forbids base-dir lookups before `gate:end`.
- **`Activate` platform data is untrusted** (any session peer can call it outside Flatpak). Only `activation-token` and `desktop-startup-id` string values are accepted, each at most 512 bytes and free of control characters; anything else is ignored. The handler only raises the window. _Why:_ bounded input and no privileged action.
- **No sleeps on the startup path.** The handoff wait uses an inotify `IN_CLOSE` watch plus `poll` with a deadline. Activation uses a `NameOwnerChanged` subscription inside the 2 s budget.

## Out of scope

- `crosshook-ui` adoption (YAN-880 implements `ActivationHandler` with the iced/winit xdg-activation request).
- Wiring "Open in Classic UI" (M1-19).
- `Open` / `ActivateAction` methods of `org.freedesktop.Application`: unknown methods get the zbus `UnknownMethod` error.
- `DBusActivatable` desktop entries.
