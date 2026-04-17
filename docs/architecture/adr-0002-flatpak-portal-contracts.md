# ADR-0002: Flatpak portal contracts — GameMode and Background

**Status**: Accepted — 2026-04-17
**Supersedes / extends**: [ADR-0001 — `platform.rs` host-command gateway](./adr-0001-platform-host-gateway.md) (additive; no behaviour change to ADR-0001)

---

## Context

CrossHook is packaged as a Flatpak (target tracked under [#276]) and launches Windows games on the **host** via `flatpak-spawn --host`. ADR-0001 established the single host-command gateway contract. Two Flatpak portals are not host commands and therefore sit outside that contract, but are nonetheless required to get the Flatpak build behaving correctly:

1. **`org.freedesktop.portal.GameMode`** — the one gaming tool in the ecosystem with a real sandbox-to-host bridge. It lets CrossHook register **its own sandbox-side PID** with the host's `gamemoded` daemon. The games CrossHook launches on the host already talk to `gamemoded` through `gamemoderun` — that path is unchanged.
2. **`org.freedesktop.portal.Background.RequestBackground`** — the only mechanism that prevents `xdg-desktop-portal` from reaping a sandboxed process when the user minimizes its window. CrossHook runs a sandbox-side `gamescope_watchdog` that supervises long-running host gameplay; losing that task leaks the compositor.

[`docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md`](../research/flatpak-bundling/15-gamemode-and-background-ground-truth.md) documents the verified current state: neither portal is called anywhere in the repo today, and the manifest does not request access to them.

This ADR defines the Rust contracts that encode these two portal integrations so every future launch path consults one owner per concern rather than scattering portal checks across `launch/`, `src-tauri/`, and `onboarding/`.

[#271]: https://github.com/yandy-r/crosshook/issues/271
[#276]: https://github.com/yandy-r/crosshook/issues/276

---

## Decision

Add a `portals/` submodule tree under the existing platform gateway:

```
crosshook-core/src/platform/
├── mod.rs              (verbatim body of the former `platform.rs`,
│                        plus `pub mod portals;` — unchanged public API)
└── portals/
    ├── mod.rs          (module root, documents scope and lists sub-portals)
    ├── gamemode.rs     (this ADR — §GameMode)
    └── background.rs   (this ADR — §Background)
```

The former `platform.rs` moved into `platform/mod.rs` unchanged, which
means the public `crate::platform::*` API surface is preserved without
any `pub use` juggling — every existing import continues to resolve.
ADR-0001's gateway contract, denylist, and `scripts/check-host-gateway.sh`
enforcement remain authoritative for host-tool calls; the portal modules
are additive and orthogonal.

> **Deferred**: a further split of `platform/mod.rs` into focused files
> (`host_gateway.rs` for `host_command*` / `host_std_command*`, `host_fs.rs`
> for host path probes, `xdg.rs` for `override_xdg_for_flatpak_host_access`)
> was discussed but not executed in the initial Issue #271 landing — the
> literal file move keeps the diff reviewable and `git log --follow` clean.
> A future refactor can split without touching ADR-0002's portal
> submodule tree.

---

## § GameMode portal contract

### Types

```rust
// crates/crosshook-core/src/platform/portals/gamemode.rs

/// How GameMode will actually be reached for a given launch context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameModeBackend {
    /// Running under Flatpak and the portal is reachable.
    /// CrossHook registers its own PID with `org.freedesktop.portal.GameMode`.
    /// Host games continue to use `gamemoderun` as the wrapper.
    Portal,
    /// Native build (AppImage, etc.) or Flatpak with an unreachable portal.
    /// `gamemoderun` is the only path; there is no CrossHook-self registration.
    HostGamemodeRun,
    /// Neither the portal nor `gamemoderun` is reachable.
    /// GameMode is effectively unavailable.
    Unavailable,
}

/// Pure decision function — deterministic, no I/O. Used by tests and callers.
pub fn resolve_backend(
    is_flatpak: bool,
    portal_available: bool,
    host_gamemoderun_available: bool,
) -> GameModeBackend;

/// Probes the session bus for `org.freedesktop.portal.Desktop` and pings the
/// GameMode sub-interface. Returns `false` if not running under Flatpak, if
/// the D-Bus proxy is not reachable, or if the portal call errors out.
/// Cheap (a single async method call). Called at dependency-gating time and
/// during capability probes.
pub async fn portal_available() -> bool;

/// Registers CrossHook's own sandbox-side PID with the GameMode portal.
/// Returns an RAII guard whose `Drop` unregisters the PID. This MUST be
/// called only when `resolve_backend(...) == GameModeBackend::Portal` —
/// callers pair it with the existing `gamemoderun` wrapper for host games.
pub async fn register_self_pid_with_portal() -> Result<GameModeRegistration, GameModeError>;

/// RAII handle to an active GameMode portal registration. Dropping the
/// value unregisters the PID. Movable but not Clone — there is exactly
/// one registration per CrossHook process.
pub struct GameModeRegistration { /* holds the zbus::Connection + request path */ }
```

### Decision matrix

| `is_flatpak()` | Portal reachable | `gamemoderun` on host | `GameModeBackend` | What happens for CrossHook's own PID | What happens for host game PID        |
| -------------- | ---------------- | --------------------- | ----------------- | ------------------------------------ | ------------------------------------- |
| false          | n/a              | true                  | `HostGamemodeRun` | Nothing (native has no sandbox PID)  | Wrapped by `gamemoderun` (unchanged)  |
| false          | n/a              | false                 | `Unavailable`     | Nothing                              | No GameMode                           |
| true           | true             | true                  | `Portal`          | Registered via portal                | Wrapped by `gamemoderun` (unchanged)  |
| true           | true             | false                 | `Portal`          | Registered via portal                | No GameMode (capability **Degraded**) |
| true           | false            | true                  | `HostGamemodeRun` | Nothing (sandbox PID not registered) | Wrapped by `gamemoderun` (unchanged)  |
| true           | false            | false                 | `Unavailable`     | Nothing                              | No GameMode                           |

### Scope boundaries

- **The portal is for CrossHook's own PID only.** Host game PIDs are already host PIDs and go through the existing `gamemoderun` wrapper in `launch/optimizations.rs`. We do **not** attempt to register host game PIDs through the portal — the PID-namespace translation the portal performs is irrelevant for PIDs that are already in the host namespace.
- **`gamemoderun` stays denylisted.** Its only invocation path is still `platform::host_command*` per ADR-0001. ADR-0002 does not remove, bypass, or reshape the denylist for `gamemoderun`.
- **PID-namespace caveat**: the GameMode portal historically had a bug where the PID namespace translation for sandboxed processes could be incomplete (FeralInteractive/gamemode#1270). The portal auto-translates for CrossHook's own PID, but we do **not** rely on PID-translation semantics for anything else. If the bug re-surfaces, CrossHook degrades to `HostGamemodeRun` for self-registration and keeps host-game wrapping unchanged.

### Capability surface

The existing `gamemode` entry in `onboarding/capability.rs` is augmented with a **derived** state:

| Actual state                              | `CapabilityState` | Rationale                                               |
| ----------------------------------------- | ----------------- | ------------------------------------------------------- |
| `Portal` + `gamemoderun` present          | `Available`       | Full support — self-reg + host-game wrap                |
| `Portal` + no `gamemoderun`               | `Degraded`        | CrossHook self-reg works; host games not wrapped        |
| `HostGamemodeRun`                         | `Available`       | Host-game wrapping works (the user-facing behaviour)    |
| `HostGamemodeRun` (Flatpak, portal fails) | `Available`       | Same user-facing behaviour as native; logged at `warn!` |
| `Unavailable`                             | `Unavailable`     | No GameMode at all                                      |

### Error handling

`GameModeError` is a small enum with `PortalUnreachable`, `DBusProtocol(zbus::Error)`, and `RegistrationRejected(String)` variants. All are non-fatal at the call site — a failed registration logs a single `tracing::warn!` and the launch proceeds with `HostGamemodeRun` semantics for host games.

---

## § Background portal contract

### Types

```rust
// crates/crosshook-core/src/platform/portals/background.rs

/// Cheap probe — returns true iff `is_flatpak()`. Native builds make zero
/// D-Bus calls. Callers use this before attempting `request_background` to
/// avoid spawning unnecessary tokio work on native.
pub fn background_supported() -> bool;

/// Asks `org.freedesktop.portal.Background.RequestBackground` to keep
/// CrossHook running with its window minimized. On success returns an
/// RAII `BackgroundGrant`; on a native build or a denied request returns
/// `Err(BackgroundError)`. Callers MUST debounce — one in-flight
/// request per process lifetime. A dropped `BackgroundGrant` releases
/// the D-Bus request handle but the portal's grant itself persists until
/// the session ends or the user revokes it.
///
/// `reason` is the user-facing string the portal may surface in permission
/// prompts (e.g., GNOME Shell's background apps list).
/// `autostart` is passed straight through; CrossHook always passes `false`.
pub async fn request_background(
    reason: &str,
    autostart: bool,
) -> Result<BackgroundGrant, BackgroundError>;

/// RAII handle to an outstanding Background request. Dropping the value
/// closes the `zbus::Connection` and releases the request; Flatpak's
/// desktop-portal-backend keeps the grant active for the session.
pub struct BackgroundGrant {
    /* holds zbus::Connection + zvariant::OwnedObjectPath for the request */
}

#[derive(Debug, thiserror::Error)]
pub enum BackgroundError {
    #[error("not running under Flatpak — RequestBackground is a no-op")]
    NotSandboxed,
    #[error("xdg-desktop-portal denied the background request")]
    PortalDenied,
    #[error("D-Bus transport error: {0}")]
    DBusProtocol(#[from] zbus::Error),
}
```

> **Note:** The implementation uses hand-written `fmt::Display` + `std::error::Error` impls to avoid the `thiserror` dependency; the `#[derive(thiserror::Error)]` form above is illustrative only.

### Lifecycle

1. **Startup.** The Tauri `.setup(...)` closure in `src-tauri/src/lib.rs` spawns one task on `tauri::async_runtime::spawn` that calls `request_background("keep CrossHook running during launches", /*autostart=*/false)` and hands the result to `BackgroundGrantHolder::store_result`. If `background_supported()` returns `false` the call short-circuits and no D-Bus traffic occurs.
2. **Managed-state storage.** `BackgroundGrantHolder` is registered via `.manage(BackgroundGrantHolder::new())` and owns the RAII `BackgroundGrant` once `store_result` fires. Drop of the Tauri app (shutdown) drops the holder, which drops the grant, which closes the zbus connection.
3. **Init synchronization.** The holder exposes `wait_for_initialization(timeout)` — an async method backed by `tokio::sync::Notify` that resolves either when `store_result` fires or when the timeout elapses. During the in-flight window sync readers (`protection_state`, `has_active_grant`) return `Pending` / `false`. Native builds are initialized from construction (they pre-arm the notifier), so waiting is a no-op.
4. **Watchdog visibility.** `spawn_gamescope_watchdog` in `src-tauri/src/commands/launch.rs` does **not** block the launch on the grant; it spawns a sibling task that awaits `wait_for_initialization(500ms)` and logs the resolved state, so diagnostics reflect the final outcome rather than the initial `Pending`. Functionally the portal's session-scoped grant protects CrossHook regardless of when the watchdog spawned.
5. **Denial path.** If the portal denies the request (KDE Plasma, for example, can prompt the user and the user can decline), `store_result` surfaces `BackgroundProtectionState::Degraded`, we log a single `tracing::warn!` via the setup task, and the host tool dashboard renders the `background_protection` capability row as `CapabilityState::Degraded` (§Capability integration below). The launch still proceeds — we do not block the user.

### Call-site table

| Call site                                                                                           | Action                                                                                                   | Contract function                                                        |
| --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| Tauri `.setup(...)` closure in `src-tauri/src/lib.rs`                                               | Spawn one task: `request_background("...", false)`, hand result to `BackgroundGrantHolder::store_result` | `request_background`                                                     |
| `spawn_gamescope_watchdog` in `src-tauri/src/commands/launch.rs`                                    | Spawn a sibling task that awaits `wait_for_initialization(500ms)` and logs the resolved grant state      | `background_supported`, `BackgroundGrantHolder::wait_for_initialization` |
| (Future) Portal `Running` signal handler                                                            | If the portal revokes background and the watchdog is still active, re-request once                       | `request_background`                                                     |
| Synthetic `background_protection` capability, exposed via `get_background_protection_state` command | Derive the dashboard row from `protection_state()` — `Pending` surfaces as an indeterminate row          | `background_supported`, `BackgroundGrantHolder::protection_state`        |

### Scope boundaries

- **CrossHook itself, not host games.** `RequestBackground` is a sandbox-scoped contract: it tells the portal "do not reap my sandbox process." Host games run outside the sandbox and are never passed to this API. This aligns with [`docs/research/flatpak-bundling/12-risks.md`](../research/flatpak-bundling/12-risks.md) §1 Correction 2.
- **No autostart.** CrossHook is not a system daemon. We pass `autostart: false` unconditionally; the user launches CrossHook explicitly.
- **No singleton.** We explicitly do not install a global static `BackgroundGrant`. Ownership sits in Tauri's managed state so tests can construct a sandbox without leaking tokio tasks.
- **Fake-able seam for tests.** The portal call goes through a thin trait `BackgroundPortal` with a live `ZbusBackgroundPortal` impl and a `#[cfg(test)] FakeBackgroundPortal`. Unit tests for the capability integration inject the fake without touching D-Bus.

### Capability integration

A new synthetic capability `background_protection` joins the existing host-tool capability set in `crosshook-core/src/onboarding/capability.rs`:

| Observed state                               | `CapabilityState` | User-facing rationale                                                             |
| -------------------------------------------- | ----------------- | --------------------------------------------------------------------------------- |
| `!background_supported()` (native build)     | **Omitted**       | Native builds don't show this row at all — it doesn't apply outside the sandbox   |
| Flatpak + grant returned successfully        | `Available`       | "CrossHook will keep its watchdog alive when minimized during a game session."    |
| Flatpak + portal reachable but grant denied  | `Degraded`        | "Minimizing during a game may terminate CrossHook's watchdog; re-grant required." |
| Flatpak + portal unreachable (`zbus::Error`) | `Unavailable`     | "xdg-desktop-portal is not reachable; watchdog protection disabled."              |

This integrates cleanly with `crosshook-core/src/onboarding/capability.rs::check_generalized_readiness` — it's derived state, not a persisted column, so **no SQLite schema change is required** (schema v21 unchanged).

> **UI integration deferred.** The `get_background_protection_state` Tauri command is registered and present in `.invoke_handler`, but no TypeScript consumer currently calls it. A `// TODO(frontend)` comment is in place at the command site in `src-tauri/src/background_portal.rs`. Wiring the dashboard row is tracked as a follow-up frontend task.

### Error handling

`BackgroundError::NotSandboxed` is the "I was called on native" sentinel; the setup closure treats it as a benign skip. `BackgroundError::PortalDenied` degrades to the `Degraded` capability state and logs once. `BackgroundError::DBusProtocol` is logged at `warn!` and retried at most once per process lifetime (the setup task may re-call after initial failure if a subsequent launch observes missing state).

---

## Consequences

### Positive

- **Single owner per portal concern.** Every future launch path (Steam applaunch, proton_run, umu-run) consults `platform::portals::gamemode::resolve_backend` and `platform::portals::background::background_supported` rather than re-deciding portal semantics locally.
- **No fallback scaffolding.** `gamemoderun` is already the baseline path; the portal is strictly additive. Removing the portal module would degrade CrossHook to its current behaviour — not introduce a regression in host-game wrapping.
- **Testable.** Both portal contracts have pure decision functions (`resolve_backend`, `background_supported`) plus `#[cfg(test)]`-only fake implementations behind trait seams. No live D-Bus needed in CI.
- **ADR-0001 preserved.** The `platform/` directory split is a literal move; all public `crate::platform::*` imports keep working; the host-gateway denylist contract is unchanged.

### Negative

- **New dependency.** `zbus 5` (default features off, tokio feature on) adds a proc-macro-heavy crate graph. Documented in the ground-truth doc §3.3; build-time cost measured and acceptable for `crosshook-core`.
- **Runtime dependency on portal presence.** CrossHook now relies on `xdg-desktop-portal-*` being installed on the host. Every modern distro that ships Flatpak already has it. SteamOS ships one; Fedora Atomic variants ship one. Graceful degradation is the mitigation.
- **Two surfaces to document.** The host tool dashboard gains a `background_protection` row; the GameMode row gains a "portal / host wrapper" hint. Both are covered by the UI copy rules in [`docs/internal/host-tool-dashboard.md`](../internal/host-tool-dashboard.md) and do not require new UI components.

### Neutral

- **No schema change.** Grant state is runtime-only; derived capability state reuses `host_readiness_snapshots`. Schema version stays at 21.
- **No new TOML settings.** User-editable preferences for background behaviour are explicitly out of scope (issue's "Storage strategy" rules them out).

---

## Alternatives considered

1. **Use `ashpd` for both portals.** Rejected: `ashpd` binds us to its API churn and transitively pulls in many portals we do not need. A minimal `zbus::Proxy` against two interfaces is smaller, testable with fixtures, and avoids forcing `ashpd`'s executor decisions on `crosshook-core`.
2. **Skip the GameMode portal entirely.** Rejected: host games already use `gamemoderun`, but CrossHook's **own** sandbox PID is not registered. Any intrinsic CrossHook CPU/GPU work (e.g., large metadata scans during launch) does not benefit from GameMode without portal self-registration. Research anchor [`10-evidence.md`](../research/flatpak-bundling/10-evidence.md) Tier 1 #4 specifically documents the portal as the correct path.
3. **Request Background only on demand (per-launch).** Rejected: the grant is a session-scope concept in the portal, not per-launch. Requesting once at startup matches the portal's mental model and avoids race windows where the watchdog is spawned before the grant returns.

---

## References

- [`docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md`](../research/flatpak-bundling/15-gamemode-and-background-ground-truth.md) — verified current state (tasks 1.1–1.3 of [#271]).
- [`docs/research/flatpak-bundling/10-evidence.md`](../research/flatpak-bundling/10-evidence.md) Tier 1 #4 — GameMode portal evidence.
- [`docs/research/flatpak-bundling/12-risks.md`](../research/flatpak-bundling/12-risks.md) §1 Correction 2 / P-T2 — scope boundaries.
- [`docs/research/flatpak-bundling/14-recommendations.md`](../research/flatpak-bundling/14-recommendations.md) Phase 1 row 1.4, Phase 2 row 2.4.
- [`docs/architecture/adr-0001-platform-host-gateway.md`](./adr-0001-platform-host-gateway.md) — host-tool gateway contract (unchanged).
- Issue [#271] (this ADR); parent tracker [#276] (Flatpak distribution); related [#269], [#273].

[#269]: https://github.com/yandy-r/crosshook/issues/269
[#273]: https://github.com/yandy-r/crosshook/issues/273
