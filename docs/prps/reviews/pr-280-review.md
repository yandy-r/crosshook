# PR Review #280 — feat(flatpak): add GameMode + Background portal integrations

**Reviewed**: 2026-04-17
**Mode**: PR
**Author**: yandy-r
**Branch**: `feat/271-flatpak-gamemode-portal-and-requestbackground` → `main`
**Head SHA**: `8509c65c835adb7a2a3f0bbcde44d35e5f45f8c5`
**Decision**: REQUEST CHANGES
**Closes**: #271

## Summary

Solid, well-documented Flatpak portal work with strong test coverage, a clean `platform.rs → platform/mod.rs` rename, and disciplined respect for the ADR-0001 host-tool gateway (no new denylist bypasses). However the **Background portal contract is incomplete**: `request_background` returns `BackgroundGrant::Available` as soon as the portal returns the intermediate `Request` object path, without awaiting the `Response` signal that actually carries the grant decision — so `parse_response_payload` (implemented and unit-tested) is dead code at runtime and the reported state is permanently inaccurate on Flatpak. Address the HIGH findings (Response signal wiring, debounce enforcement, Notify race, D-Bus connection duplication, Send+Sync documentation) before merge.

## Findings

### CRITICAL

_None._

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs:53-93` — `request_background` is not debounced at the function level. ADR-0002 and the `BackgroundGrantHolder` doc comments state "Must be called exactly once per process lifetime", but the function itself has no guard (`OnceLock`, `AtomicBool`, or `debug_assert!`). The invariant is enforced only by the single call site in `lib.rs`. A future re-trigger path would silently open a second D-Bus connection and register a second portal `Request`.
  - **Status**: Fixed
  - **Category**: Pattern Compliance [quality]
  - **Suggested fix**: Enforce the one-call contract in code — e.g., gate the body on a `static CALLED: OnceLock<()>` so a second invocation returns early (or returns `BackgroundError::AlreadyRequested`). Reconcile with the ADR's "retried at most once after initial failure" note (§Error handling) so implementation and contract agree.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs:80` — `request_background` returns `Ok(BackgroundGrant)` as soon as the D-Bus method call for `RequestBackground` returns an object path, without subscribing to the `Response` signal. Per the xdg-desktop-portal spec, the returned object path is an `org.freedesktop.portal.Request` handle; the actual grant outcome (approved / denied / user-cancelled) arrives asynchronously as a `Response` signal on that path. Consequence: `BackgroundProtectionState::Available` is reported on every Flatpak host with D-Bus reachable, even when the user explicitly declines the dialog. `parse_response_payload` and `BackgroundError::PortalDenied` are dead at runtime — they are only reached by unit tests. The `tracing::info!` at line 86 ("watchdog protection active") is emitted before the grant is confirmed. Flagged independently by correctness and security reviewers.
  - **Status**: Fixed
  - **Category**: Correctness [correctness, security]
  - **Suggested fix**: After receiving `request_path`, create a `zbus::Proxy` on that object path for `org.freedesktop.portal.Request`, subscribe to the `Response` signal, and `await` it with a timeout. Feed `(response_code, results)` through the existing `parse_response_payload` — that is exactly the function it was written for. Minimal sketch (zbus 5):

    ```rust
    let req = zbus::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        request_path.as_str(),
        "org.freedesktop.portal.Request",
    ).await?;
    let mut stream = req.receive_signal("Response").await?;
    let msg = tokio::time::timeout(Duration::from_secs(60), stream.next())
        .await
        .map_err(|_| BackgroundError::PortalDenied)?
        .ok_or(BackgroundError::PortalDenied)?;
    let (code, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;
    parse_response_payload(code, &results)?;
    ```

    Once wired, retarget the `tracing::info!` to fire _after_ the signal is parsed, and downgrade the intermediate "submitted" log to `debug!`.

- **[F003]** `src/crosshook-native/src-tauri/src/background_portal.rs:45-52` — `BackgroundGrantHolder` is registered with Tauri's `.manage()`, which requires `Send + Sync`. The compile-time guarantee depends on `zbus::Connection: Send + Sync` (true in zbus 5). There is no `static_assertions::assert_impl_all!` or explicit comment documenting this dependency; if a future zbus release narrows those bounds, the break surfaces at the `.manage()` call site with an opaque error rather than a readable assertion.
  - **Status**: Fixed
  - **Category**: Maintainability [quality]
  - **Suggested fix**: Add `static_assertions::assert_impl_all!(BackgroundGrantHolder: Send, Sync);` near the `impl` block (add `static_assertions` as a `dev-dependency` if not present), or at minimum annotate the struct with a `// SAFETY: relies on zbus::Connection: Send + Sync (zbus >= 5)` doc comment so the invariant is greppable.

- **[F004]** `src/crosshook-native/src-tauri/src/background_portal.rs:175` — `wait_for_initialization` has a race window in its `notify_waiters` / `notified()` pattern. `tokio::sync::Notify::notify_waiters` only wakes futures that are _already registered_ (polled at least once). The `Notified` future created at line 175 is unpolled at creation; if `store_result` fires `notify_waiters` between line 175 and the second `is_initialized()` check (line 176), the notification is dropped and the caller waits the full `timeout` (500 ms for the watchdog). The second `is_initialized()` read at line 180 prevents a stale return, so the functional impact is a spurious ≤500 ms diagnostic delay in the Flatpak watchdog spawn path (not exercised on native builds).
  - **Status**: Fixed
  - **Category**: Correctness [correctness]
  - **Suggested fix**: `enable()` the `Notified` future before the double-check so any `notify_waiters` between enable and await is captured:

    ```rust
    let mut notified = std::pin::pin!(self.ready.notified());
    notified.as_mut().enable();
    if self.is_initialized() {
        return self.protection_state();
    }
    let _ = tokio::time::timeout(timeout, notified).await;
    ```

    Or switch `store_result` to `notify_one()` so the permit persists for any subsequent `notified().await`.

- **[F005]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/gamemode.rs:85,126` — `try_register_gamemode_portal_for_launch` opens **two** separate `zbus::Connection::session()` instances per game or trainer launch: one in `portal_available()` (introspection probe) and another in `register_self_pid_with_portal()` (the registration call). The introspection connection is used once for a string search on XML and then dropped. Each `Connection::session()` is a socket connect + SASL handshake + `Hello` exchange — running both on every launch doubles the per-launch bus overhead. Security reviewer flagged this HIGH; correctness reviewer flagged the same spot as MEDIUM.
  - **Status**: Fixed
  - **Category**: Performance [security, correctness]
  - **Suggested fix**: Probe the portal once at app startup (alongside the Background portal request) and cache the result in `LaunchPlatformCapabilities` or a `OnceLock<bool>`; reuse a shared session connection across introspection + registration. Alternative: fold `portal_available()` and `register_self_pid_with_portal()` into one function that opens a single connection, introspects, and if the interface exists, immediately registers and returns the RAII guard holding the connection.

### MEDIUM

- **[F006]** `docs/architecture/adr-0002-flatpak-portal-contracts.md:172-181` — The ADR snippet for `BackgroundError` shows `#[derive(Debug, thiserror::Error)]` with `#[error(...)]` attributes, but the implementation (`background.rs:154-174`) is hand-written `fmt::Display` + `std::error::Error` impls (as the Cargo comment notes: "thiserror-like impls are hand-written"). Readers following the ADR to understand the error surface will find the code doesn't match.
  - **Status**: Fixed
  - **Category**: Pattern Compliance [quality]
  - **Suggested fix**: Update the ADR code block to show the hand-written form, or add a sentence under the snippet noting the intentional discrepancy ("Implementation uses hand-written Display/Error impls to avoid the thiserror dependency").

- **[F007]** `src/crosshook-native/crates/crosshook-core/Cargo.toml:38` — `zvariant = { version = "5", default-features = false }` disables default features without naming the non-default features the crate actually needs. `OwnedValue` serde support (required for the `a{sv}` payload used by `parse_response_payload`) comes from the `serde` feature. Intent should be explicit.
  - **Status**: Failed
  - **Category**: Pattern Compliance [quality]
  - **Suggested fix**: `zvariant = { version = "5", default-features = false, features = ["serde"] }` — matches the symmetry already used for `zbus = { ..., features = ["tokio"] }`.

- **[F008]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs:83-87` — The D-Bus `request_path` (e.g. `/org/freedesktop/portal/desktop/request/<sender_unique_name>/<token>`) is logged at `tracing::info!`. The object path encodes the caller's D-Bus unique name; under a shipped log pipeline this is a minor operational information-disclosure (process identity correlation).
  - **Status**: Fixed
  - **Category**: Security [security]
  - **Suggested fix**: Downgrade to `tracing::debug!` — consistent with `GameModeRegistration::drop`, which already uses `debug!` for equivalent lifecycle events.

- **[F009]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs:106` — `BackgroundGrant::request_path` is stored "for future signal subscription", but on Drop nothing is logged and the `request_path` is silently discarded. The `zbus::Connection` close implicitly signals teardown to D-Bus, but there is no operator-visible trace of the release.
  - **Status**: Fixed
  - **Category**: Completeness [correctness]
  - **Suggested fix**: Log the `request_path` at `debug!` in `Drop`, matching the `GameModeRegistration::drop` pattern, so grant release is visible in diagnostics.

- **[F010]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs:198-215` — `parse_response_payload` is exposed as `pub` with four unit tests but is never called from the runtime code path (see F002). Until F002 is fixed, it is dead public API that implies a completeness it does not yet provide.
  - **Status**: Fixed
  - **Category**: Maintainability [security]
  - **Suggested fix**: Either (a) wire it up via the F002 fix (preferred), or (b) mark it `pub(crate)` in the interim so consumers do not treat it as a stable surface.

- **[F011]** `src/crosshook-native/src-tauri/src/background_portal.rs:196-201` + `src/crosshook-native/src-tauri/src/lib.rs:471` — `get_background_protection_state` is registered as a `#[tauri::command]` and present in `.invoke_handler`, but no TypeScript consumer invokes it (no `invoke("get_background_protection_state")` anywhere under `src/`). ADR-0002 § Capability integration describes a `background_protection` dashboard row derived from this state. The Rust side is complete; the UI integration is missing.
  - **Status**: Fixed
  - **Category**: Completeness [quality]
  - **Suggested fix**: Either land the frontend wiring in this PR (extend `onboarding/capability.rs` / add a hook + dashboard row), or file a follow-up issue and add a `// TODO(#NNN): wire frontend get_background_protection_state` at the command site so the gap is self-documenting. Update ADR-0002 to mark the UI integration explicitly deferred if it is not landing here.

- **[F012]** `src/crosshook-native/src-tauri/src/background_portal.rs:287-309` — `wait_for_initialization_unblocks_when_store_result_fires` only exercises the native-CI branch (`initialized = true` at construction, so the function returns at line 170 without ever touching `Notify`). The real Flatpak path (`Pending → Degraded` via `notify_waiters`) is not covered, and the F004 race window is untested.
  - **Status**: Fixed
  - **Category**: Completeness [correctness]
  - **Suggested fix**: Refactor `BackgroundGrantHolder::new` to accept an injectable `is_flatpak: bool` (mirroring the `gamemode.rs` / `optimizations.rs` pattern), or expose a `#[cfg(test)] fn new_pending() -> Self` constructor. Then add a test that drives a full `Pending → notify_waiters → protection_state == Degraded` cycle.

- **[F013]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/gamemode.rs:64-76` — `resolve_backend` parameters `is_flatpak: bool` and `portal_available: bool` shadow the imported `is_flatpak` function from `crate::platform` and the `portal_available()` async function on line 85. The function is pure so it's harmless today, but a future reader scanning the body for whether it calls `is_flatpak()` will be misled.
  - **Status**: Fixed
  - **Category**: Maintainability [quality]
  - **Suggested fix**: Rename to `is_in_flatpak: bool` and `portal_is_available: bool` to disambiguate from the same-named callables in scope.

### LOW

- **[F014]** `src/crosshook-native/src-tauri/src/commands/launch.rs:394` — `watchdog_app_handle` is cloned from `app` just before `app` is consumed by `spawn_log_stream`. Correct and necessary, but the name and ordering read as "handle _from_ the watchdog" rather than "handle _for_ the watchdog call".
  - **Status**: Fixed
  - **Category**: Maintainability [quality]
  - **Suggested fix**: Add a one-line `// clone before app is moved into spawn_log_stream` comment, or rename to `watchdog_app_handle_clone` / `app_for_watchdog`.

- **[F015]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1266` — `finalize_launch_stream` is ~197 lines (pre-existing; this PR only added ~4 lines of watchdog-grant logging). Exceeds the 50-line guideline. Not a blocker for this PR, but flagging for a follow-up refactor.
  - **Status**: Failed
  - **Category**: Maintainability [quality]
  - **Suggested fix**: Extract the version-snapshot block (~lines 880-980) and the known-good-tagging block (~lines 982-1024) into `record_version_snapshot` and `tag_known_good_revision` helpers in a follow-up PR.

- **[F016]** `src/crosshook-native/crates/crosshook-core/src/platform/portals/mod.rs:16` — The module doc claims "the D-Bus side lives behind a trait seam so `#[cfg(test)]` fakes can be injected", but no such seam exists — `request_background` and `portal_available` call zbus directly. The ADR-0002 references to a `ZbusBackgroundPortal` / `FakeBackgroundPortal` pair never materialized.
  - **Status**: Fixed
  - **Category**: Completeness [correctness]
  - **Suggested fix**: Update the doc to match reality: "Pure decision helpers (`resolve_backend`, `background_supported`) are unit-testable; the D-Bus entry points (`portal_available`, `request_background`) are guarded by `is_flatpak()` and documented as requiring a live session bus."

## Validation Results

| Check                  | Result                                                                    |
| ---------------------- | ------------------------------------------------------------------------- |
| Host-gateway check     | Pass (`./scripts/check-host-gateway.sh`)                                  |
| Clippy (`-D warnings`) | Pass (`cargo clippy -p crosshook-core --all-targets`)                     |
| Tests                  | Pass (994/994 in crosshook-core main suite; 0 failed across sub-binaries) |
| Build                  | Pass (`cargo build --workspace`)                                          |

## Files Reviewed

- `docs/architecture/adr-0001-platform-host-gateway.md` (Modified)
- `docs/architecture/adr-0002-flatpak-portal-contracts.md` (Added)
- `docs/prps/plans/completed/issue-271-flatpak-gamemode-portal-and-requestbackground.plan.md` (Added)
- `docs/prps/reports/issue-271-flatpak-gamemode-portal-and-requestbackground-report.md` (Added)
- `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md` (Added)
- `packaging/flatpak/dev.crosshook.CrossHook.yml` (Modified)
- `src/crosshook-native/Cargo.lock` (Modified)
- `src/crosshook-native/crates/crosshook-core/Cargo.toml` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/platform/mod.rs` (Renamed from `platform.rs`; +5)
- `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/platform/portals/gamemode.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/platform/portals/mod.rs` (Added)
- `src/crosshook-native/src-tauri/src/background_portal.rs` (Added)
- `src/crosshook-native/src-tauri/src/commands/launch.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)

## Notes

**What checked out cleanly:**

- **ADR-0001 host-gateway compliance**: `check-host-gateway.sh` passes; no new `Command::new("<denylisted-tool>")` bypasses in any of the portal, IPC, or launch files. Portal code uses zbus (in-sandbox D-Bus) — outside the gateway's scope by design.
- **Flatpak manifest**: the only permission added is `--talk-name=org.freedesktop.portal.Desktop`. No new `--filesystem`, `--socket`, `--share`, `--system-talk-name`, or `--device` permissions snuck in.
- **PID source**: `RegisterGame(std::process::id(), …)` — CrossHook's own sandbox PID as documented; no untrusted source.
- **Dependency supply chain**: `zbus 5` / `zvariant 5` both use `default-features = false`. Lockfile confirms `async-io` absent; no `icu_*`/`url`/`idna` pulled in as feared in the PR description (those only appear when the default `async-io` runtime is enabled).
- **Schema / persistence**: confirmed unchanged at v21; grant state is runtime-only as claimed. No new TOML settings, no new SQLite migrations.
- **Drop safety**: `GameModeRegistration::drop` is best-effort and does not panic; `BackgroundGrantHolder` `Mutex` use recovers from poison via `PoisonError::into_inner`.
- **Trainer parity fix (8509c65)**: `effective_method` threads through `should_register_gamemode_portal_with`, Steam trainer → `proton_run` rewrite case is covered.
- **platform.rs → platform/mod.rs rename**: git detects as a rename; only the 5-line `pub mod portals;` addition visible in the content diff.
