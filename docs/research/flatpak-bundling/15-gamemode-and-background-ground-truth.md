# Flatpak GameMode & Background Portal — Ground Truth

> Closes the "assumption vs. implementation" gap flagged by Issue [#271] and
> deep-research Phase 1 task 1.4 / Phase 2 task 2.4 in
> [`14-recommendations.md`](./14-recommendations.md).
>
> This document captures **what CrossHook actually does today**, verified by
> reading the code and manifest in place, before any code changes are made.
>
> All line numbers below are from the tree at commit `main` ≈ 2026-04-17.

[#271]: https://github.com/yandy-r/crosshook/issues/271

## TL;DR

- **GameMode** under Flatpak is reached **only** via the `gamemoderun` wrapper,
  prepended to the launch chain and executed through `flatpak-spawn --host`.
  **`org.freedesktop.portal.GameMode` is not called anywhere in the repository.**
- **CrossHook's Flatpak manifest does not request access to xdg-desktop-portal**
  — it only talks to `org.freedesktop.Flatpak` (for `flatpak-spawn --host`).
  Any future portal use needs `--talk-name=org.freedesktop.portal.Desktop`.
- **`RequestBackground` is never called.** The `gamescope_watchdog` is a
  sandbox-side Tokio task with no lifetime protection — minimizing the Tauri
  window during a long game session can let xdg-desktop-portal reap the
  sandbox process and leak the compositor.

---

## §1 — GameMode reach today

### 1.1 Catalog entry

`src/crosshook-native/assets/default_optimization_catalog.toml:68-76`:

```toml
id = "use_gamemode"
...
wrappers = ["gamemoderun"]
...
required_binary = "gamemoderun"
...
help_text = "Launches through gamemoderun when the GameMode service is available."
```

The optimization declares `gamemoderun` both as a required host binary
(for dependency gating) and as a wrapper to prepend to the command line.

### 1.2 Dependency gating

`src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs:254-272`
— `is_command_available()`:

```rust
if platform::is_flatpak() {
    return platform::host_command_exists(binary);
}
```

Under Flatpak the probe routes through the `platform.rs` host gateway (ADR-0001).
If `gamemoderun` is missing on the host, `resolve_directives_with_catalog` returns
`ValidationError::LaunchOptimizationDependencyMissing { option_id: "use_gamemode",
dependency: "gamemoderun" }`
(`src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs:104-109`),
and the surface-level error message — per
`src/crosshook-native/crates/crosshook-core/src/launch/request.rs:1672-1677` —
is:

> Install `gamemoderun` and make sure it is available on PATH, or disable `use_gamemode`.

### 1.3 Wrapper chain composition

`src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs:122-124`
copies wrappers into `LaunchDirectives::wrappers` in catalog order. The wrapper
chain is then assembled by `script_runner.rs` and launched via the host gateway
in `platform::host_command*`; the Steam-launch-options formatter in the same
module prepends wrappers as space-separated tokens before `%command%`
(`optimizations.rs:194-252`).

Test coverage for the wrapper path exists at
`optimizations.rs:372-391` (`resolves_wrapper_directives_in_deterministic_order`)
and `script_runner.rs:1147-1199` / `script_runner.rs:1428-1462` — both seed a
fake `gamemoderun` executable, assert the chain order, and do **not**
exercise a portal path.

### 1.4 Version probe

`src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs:133,231-233,312-313`
runs `gamemoderun --version` (or falls back to `gamemode --version`), strips the
leading `gamemoderun` / `gamemode` token (plus the trailing space) from the
output line, and reports the version. Capability
registration: `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs:394-397`
lists a single `gamemode` required-tool entry with `required_tools: ["gamemode"]`.

### 1.5 Flatpak manifest — what the sandbox can reach

`packaging/flatpak/dev.crosshook.CrossHook.yml:39-42`:

```yaml
# Host command execution via flatpak-spawn --host
# Required for Proton, Wine, Steam, gamescope, git, and system-installed
# compatibility tools whose host /usr paths are masked by the runtime.
- --talk-name=org.freedesktop.Flatpak
```

This is the **only** D-Bus peer the sandbox can reach. There is **no**
`--talk-name=org.freedesktop.portal.Desktop`, therefore no route to
`org.freedesktop.portal.GameMode` or `org.freedesktop.portal.Background` even
if the code tried to use them today.

### 1.6 Portal calls — grep evidence

```bash
$ rg -n 'RequestBackground|org\.freedesktop\.portal|zbus' src/crosshook-native
src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs
# (only hits in runtime_helpers are for resolve_host_dbus_session_bus_address —
#  that is Wine session-bus address handling for host-launched games, not a portal call)
```

No `zbus`, no `dbus`, no `RequestBackground`, no portal interface anywhere in
the orchestrator code. The only D-Bus artifact present is a string-level
`DBUS_SESSION_BUS_ADDRESS` rewrite used when preparing the env passed to
host-launched Wine/Proton processes.

### 1.7 Conclusion — §1

> **CrossHook reaches GameMode today by shelling out to `gamemoderun` on the
> host through `flatpak-spawn --host`, not by calling
> `org.freedesktop.portal.GameMode`.**

This is correct as a way to protect **host games** (games run on the host and
are already host PIDs that `gamemoderun` wraps with the full `libgamemode`
integration). It is **not** a way to register CrossHook's **own** sandbox-side
PID with the GameMode daemon — for that, the portal is the right path.
Issue #271 is specifically about adding the portal path for CrossHook's own
PID while keeping the existing `gamemoderun` path for host games.

### 1.8 Research anchors — §1

- [`14-recommendations.md`](./14-recommendations.md) §2 Phase 1 row 1.4 —
  "Verify GameMode portal path".
- [`10-evidence.md`](./10-evidence.md) Tier 1 #4 — "GameMode works from Flatpak
  via `org.freedesktop.portal.GameMode`" (confidence **High**).
- [`12-risks.md`](./12-risks.md) P-T2 — "GameMode already works via portal —
  bundling GameMode is redundant; `org.freedesktop.portal.GameMode` (v4)
  provides sandbox-to-host bridging".
- [`10-evidence.md`](./10-evidence.md) §"Uncertainty" entry —
  "GameMode PID registration bug (#1270) affects CrossHook's use case" — the
  portal has a known PID-namespace caveat that must be captured in the ADR.

---

## §2 — Watchdog ownership and `RequestBackground` applicability

### 2.1 `gamescope_watchdog` is a sandbox-side Tokio task

`src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs:427-432`:

```rust
pub async fn gamescope_watchdog(
    gamescope_pid: u32,
    exe_name: &str,
    killed_flag: Arc<AtomicBool>,
    host_pid_capture_path: Option<PathBuf>,
) {
```

This function polls a host-PID target (`resolve_watchdog_target` at the top
of the same file, using `read_pid_capture_file` and, under Flatpak,
`read_host_text_file` via `platform::host_std_command("cat")`) and, when the
game exe inside the gamescope session disappears, fires a SIGTERM to the
gamescope compositor via the host gateway. The polling loop is pure
async-Rust running inside the CrossHook process.

### 2.2 Spawn site

`src/crosshook-native/src-tauri/src/commands/launch.rs:1067-1083`:

```rust
fn spawn_gamescope_watchdog(
    gamescope_pid: u32,
    exe_name: String,
    killed_flag: Arc<AtomicBool>,
    host_pid_capture_path: Option<PathBuf>,
) {
    ...
    tauri::async_runtime::spawn(async move {
        gamescope_watchdog_core(gamescope_pid, &exe_name, killed_flag, host_pid_capture_path).await;
    });
}
```

Spawn happens on the Tauri async runtime inside the **sandbox process**. The
task is owned entirely by CrossHook's own Tokio runtime; its lifetime is bound
to the lifetime of the CrossHook process.

### 2.3 Ownership implications

Because the watchdog is a sandbox-side task:

1. It **is** a legitimate `RequestBackground` target — `RequestBackground` is
   explicitly scoped to sandbox processes (a sandbox tells the portal
   "please do not reap me when my window is minimized").
2. The **game** it supervises is **not** — games are host processes launched
   via `flatpak-spawn --host` (Proton/Wine/Steam/gamescope all run on the
   host). `RequestBackground` does not apply to them; their lifetime is
   managed by the host. This aligns with
   [`12-risks.md`](./12-risks.md) §1 Correction 2 and
   [`10-evidence.md`](./10-evidence.md) Theme E.

### 2.4 Current vulnerability

`src/crosshook-native/src-tauri/src/lib.rs:141-147` shows the Tauri
`setup` closure (starts at line 147) — and critically, there is **no**
`on_window_event` hook, no `RunEvent::WindowEvent` handler, and no call to
`RequestBackground` anywhere:

```bash
$ rg -n 'RequestBackground|on_window_event|RunEvent::WindowEvent' src/crosshook-native
# (no results)
```

Concrete failure mode:

1. User launches a long gameplay session (e.g. a multi-hour RPG boot through
   gamescope on Flatpak CrossHook).
2. User minimizes the CrossHook window to reclaim desktop real estate.
3. `xdg-desktop-portal-*` may, under memory pressure or idle-reclaim policy,
   signal the sandbox to exit — nothing has told the portal the sandbox has
   a live background commitment.
4. CrossHook's Tokio runtime dies; `gamescope_watchdog` never fires the
   SIGTERM to the compositor when the game exits.
5. `gamescope` + its reaper + `mangoapp` + `winedevice.exe` **leak** on the
   host. The user is left with a frozen compositor until they notice and
   `kill` it manually.

### 2.5 Conclusion — §2

> **The watchdog is a CrossHook-owned sandbox task whose continued execution
> is required for correct host-game cleanup. It is the textbook case for
> `org.freedesktop.portal.Background.RequestBackground`. The game is not — and
> the research explicitly warns against confusing the two models.**

### 2.6 Research anchors — §2

- [`14-recommendations.md`](./14-recommendations.md) §2 Phase 2 row 2.4 —
  "`RequestBackground` portal … Protects CrossHook, not games (per Crucible
  correction)".
- [`12-risks.md`](./12-risks.md) §1 Correction 2 — "Background portal does
  not apply to host games".
- [`10-evidence.md`](./10-evidence.md) Theme E — sandbox/host ownership
  model.

---

## §3 — Flatpak manifest permissions & dependency delta for portal work

### 3.1 Current `finish-args` state

Today `packaging/flatpak/dev.crosshook.CrossHook.yml:26-58` grants:

```yaml
- --socket=wayland
- --socket=fallback-x11
- --share=ipc
- --device=dri
- --socket=pulseaudio
- --share=network
- --talk-name=org.freedesktop.Flatpak
- --filesystem=home
- --filesystem=/mnt
- --filesystem=/run/media
- --filesystem=/media
- --filesystem=~/.var/app/com.valvesoftware.Steam:ro
- --filesystem=xdg-data/umu:create
```

The session bus itself is **implicitly** available to the sandbox through
Flatpak's xdg-dbus-proxy (every Flatpak app gets a proxied session bus
socket via `DBUS_SESSION_BUS_ADDRESS`), but access to any given peer name
is gated by `--talk-name=…` entries.

### 3.2 Required additions

To call `org.freedesktop.portal.GameMode` and `org.freedesktop.portal.Background`
via D-Bus, the sandbox must be allowed to talk to the xdg-desktop-portal
well-known bus name:

```yaml
# xdg-desktop-portal — needed for org.freedesktop.portal.GameMode (CrossHook's
# own PID registration) and org.freedesktop.portal.Background (keep the
# watchdog task alive when the window is minimized).
- --talk-name=org.freedesktop.portal.Desktop
```

That single line is the entirety of the manifest delta for this feature.
No additional sockets, filesystems, or devices are required.

### 3.3 Rust D-Bus client — dependency delta

The `crosshook-core` crate has **no** existing D-Bus client dependency
(confirmed by `rg 'zbus|dbus' src/crosshook-native/crates/crosshook-core/Cargo.toml`
returning nothing). The GNOME 50 runtime ships `glib`/`gio` (GDBus at the C
level) but we prefer a pure-Rust client that can be built offline into
the same crate.

**Smallest viable addition**:

```toml
# crates/crosshook-core/Cargo.toml
zbus = { version = "5", default-features = false, features = ["tokio"] }
```

Notes:

- `default-features = false` drops the `async-io` default and keeps tokio as
  the sole async runtime (consistent with the rest of `crosshook-core`).
- `zbus 5` is the current LTS line (2025-). No breaking changes are expected
  during the lifetime of this feature.
- `zbus` is BSD-3-Clause, already in wide use in the GNOME/Rust ecosystem
  (e.g., GNOME Console, `bluer`, `ashpd`). License is compatible with
  CrossHook (GPL-3.0-or-later project).
- We specifically **do not** pull in `ashpd` (the higher-level portal
  wrapper) for this change. `ashpd` is nice but binds us to its update
  cadence and its surface covers many portals we do not need. A thin
  hand-written `zbus::Proxy` against two portal interfaces is smaller,
  easier to test with fixtures, and keeps the dependency tree minimal.

### 3.4 Alignment with ADR-0001

[`docs/architecture/adr-0001-platform-host-gateway.md`](../../architecture/adr-0001-platform-host-gateway.md)
defines the single-abstraction host gateway. The portal work introduced by
#271 is **additive** to that contract — the ADR's "Scope boundary" section
already carves out non-host-tool code paths, and portal calls are not host
tools. However, the `gamemoderun` wrapper path (used for host games) is in
the denylist and must continue to flow through `platform::host_command*`
even when the portal path is active for CrossHook's own PID. ADR-0002
(next task) formalises that interaction.

### 3.5 Manifest side-effects to watch

- **Portal fallback on hosts without xdg-desktop-portal**: the GNOME 50
  runtime sandbox expects the host to provide a portal implementation
  (`xdg-desktop-portal-gnome`, `xdg-desktop-portal-kde`, etc.). Steam Deck
  SteamOS ships one; Fedora Atomic variants ship one; niche distros may not.
  If the portal is unreachable, `portal_available()` must degrade gracefully
  (see §2.1 / ADR-0002).
- **Background portal and autostart**: `RequestBackground` has an `autostart`
  option; we explicitly pass `false` — CrossHook is not a daemon and should
  not be auto-launched at login by the portal.

### 3.6 Conclusion — §3

> **One manifest line (`--talk-name=org.freedesktop.portal.Desktop`) and one
> Cargo dependency (`zbus 5` with default features off) are sufficient to
> enable both portal paths. No other packaging changes are required.**

### 3.7 Research anchors — §3

- [`14-recommendations.md`](./14-recommendations.md) §2 Phase 1 row 1.7 —
  "Protect `platform.rs` gateway" (ADR-0001, already accepted).
- ADR-0001 — [`docs/architecture/adr-0001-platform-host-gateway.md`](../../architecture/adr-0001-platform-host-gateway.md).
- Host tool dashboard — [`docs/internal/host-tool-dashboard.md`](../../internal/host-tool-dashboard.md).

---

## §4 — Implementation

Landed under Issue [#271]. Closes Phase 1 task 1.4 and Phase 2 task 2.4 from
[`14-recommendations.md`](./14-recommendations.md).

### 4.1 Artifacts

- ADR: [`docs/architecture/adr-0002-flatpak-portal-contracts.md`](../../architecture/adr-0002-flatpak-portal-contracts.md) (Accepted).
- Rust module tree:
  - `src/crosshook-native/crates/crosshook-core/src/platform/` — the `platform.rs` module was moved to `platform/mod.rs` (public API preserved via `pub use` re-exports) so a `platform/portals/` submodule could be added without churn in call sites.
  - `src/crosshook-native/crates/crosshook-core/src/platform/portals/gamemode.rs` — `GameModeBackend`, `resolve_backend`, `portal_available`, `register_self_pid_with_portal`, `GameModeRegistration`.
  - `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs` — `background_supported`, `request_background`, `BackgroundGrant`, `parse_response_payload` (fixture-testable).
- Tauri glue:
  - `src/crosshook-native/src-tauri/src/background_portal.rs` — `BackgroundGrantHolder` managed-state wrapper and `get_background_protection_state` Tauri command.
  - `src/crosshook-native/src-tauri/src/commands/launch.rs` — `try_register_gamemode_portal_for_launch` around `launch_game` and `launch_trainer`; the returned guard is held for the duration of the log stream task and explicitly `UnregisterGame`d when the child exits.
  - `src/crosshook-native/src-tauri/src/lib.rs` — `.setup(...)` requests background once at startup and stores the grant in the managed-state holder.
- Packaging: `packaging/flatpak/dev.crosshook.CrossHook.yml` now declares `--talk-name=org.freedesktop.portal.Desktop`.
- Dependency delta: `zbus = "5"` (default features off, `tokio` feature on) and `zvariant = "5"` added to `crosshook-core/Cargo.toml`.
- Schema version: **unchanged (21)** — portal/grant state is runtime-only per the issue's "Storage strategy" section.
- No new TOML settings.

### 4.2 Closure path to parent tracker [#276]

The research tracker [#276] can mark the following items closed by this work:

- Phase 1 task 1.4 — "Verify GameMode portal path" — verified and encoded; `gamemoderun` remains authoritative for host games and `org.freedesktop.portal.GameMode` registers CrossHook's own PID under Flatpak.
- Phase 2 task 2.4 — "`RequestBackground` portal" — implemented as a session-scoped grant requested once at startup, scoped to CrossHook-owned sandbox processes (not host games), with graceful degradation documented in ADR-0002.

Remaining Phase 2 items (`2.1 Tool status dashboard`, `2.5 Host tool version probing`) are independent of this work and tracked under [#269].
