# Implementation Plan: Flatpak GameMode Portal Verification + `RequestBackground` Watchdog Protection (Issue #271)

**Source issue**: https://github.com/yandy-r/crosshook/issues/271
**Parent research tracker**: #276
**Related work**: #269 (host-readiness), #273 (platform gateway)
**Planner**: `ycc:planner` via `/ycc:plan --parallel`
**Status**: Approved — ready for `/ycc:prp-implement --parallel`

## Overview

Issue #271 has two coupled but independent goals in the "orchestrator vs. host" model:

1. Verify the **real** execution path CrossHook uses to reach GameMode under Flatpak and encode the decision (`org.freedesktop.portal.GameMode` vs. `gamemoderun` via `flatpak-spawn --host`) in a reusable abstraction.
2. Call `org.freedesktop.portal.Background.RequestBackground` to keep CrossHook's **own** sandbox process (and its `gamescope_watchdog` supervisor task) alive when the Tauri window is minimized during long game sessions.

Scope: CrossHook-owned processes only — not host games.

## Batches (parallel-execution summary)

| Batch                                         | Step IDs            | Why these can run together                                                                                                                                                         |
| --------------------------------------------- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Batch A (research + baseline)**             | `1.1`, `1.2`, `1.3` | Pure read-only evidence gathering and baseline docs; no code touched; independent.                                                                                                 |
| **Batch B (design abstractions)**             | `2.1`, `2.2`        | Two independent design notes: GameMode portal contract and background/watchdog contract. Depend on Batch A but on different inputs.                                                |
| **Batch C (platform module scaffolding)**     | `3.1`, `3.2`        | Parallel Rust additions: new sibling modules under `crosshook-core/src/platform/` (portals for GameMode + background). Sibling files, no edit-overlap.                             |
| **Batch D (integration + docs)**              | `4.1`, `4.2`, `4.3` | Wire GameMode into the optimization/launch path, wire `RequestBackground` into the Tauri setup/watchdog lifecycle, and update architecture docs. Each touches a distinct file set. |
| **Batch E (tests + host-gateway compliance)** | `5.1`, `5.2`, `5.3` | Unit tests (pure Rust), host-gateway lint exercise, and `docs/internal` sign-off. Independent.                                                                                     |

Batch A must finish before Batch B. Batch B must finish before Batch C. Batch C must finish before Batch D. Batch D must finish before Batch E.

---

## Step 1 — Research + baseline (Batch A)

### 1.1 Verify current GameMode reach and capture ground truth

- Depends on `[]`
- Files (read-only):
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` (the `use_gamemode` optimization wrapper)
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (`LaunchOptimizationDependencyMissing` path for `gamemoderun`)
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (wrapper chain composition)
  - `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` + `details.rs` (`gamemode` capability + version probe)
  - `src/crosshook-native/assets/default_host_readiness_catalog.toml` (installed via host pkgmgr today)
  - `src/crosshook-native/assets/default_optimization_catalog.toml` (`use_gamemode` → `wrappers = ["gamemoderun"]`)
  - `packaging/flatpak/dev.crosshook.CrossHook.yml` (finish-args: no `--talk-name=org.freedesktop.portal.GameMode`)
- Output: write findings into a new `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md` describing:
  1. Today CrossHook reaches GameMode **only** via `gamemoderun` prepended to the wrapper chain, launched through `flatpak-spawn --host` (host gateway).
  2. `org.freedesktop.portal.GameMode` is **not** currently called, and the Flatpak manifest does not request the portal.
  3. Evidence anchors from `docs/research/flatpak-bundling/14-recommendations.md` §2 GameMode row, `10-evidence.md` Tier 1 #4, and `12-risks.md` P-T2.
- Acceptance check: the doc lists every source call site (with line numbers) and matches the current evidence-corrected model (games run on host; `gamemoderun` on host is fine today; the portal is a parallel path for **CrossHook's own PID** registration).
- Risk: Low. Pure documentation.

### 1.2 Verify watchdog ownership and lifecycle

- Depends on `[]`
- Files (read-only):
  - `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs` (the `gamescope_watchdog` function, defined `pub async fn gamescope_watchdog(...)`, owned by the sandbox process)
  - `src/crosshook-native/src-tauri/src/commands/launch.rs` lines 1067–1083 (`spawn_gamescope_watchdog`, `tauri::async_runtime::spawn`)
  - `src/crosshook-native/src-tauri/src/lib.rs` (no current `on_window_event` hook; no minimize handling)
- Output: extend `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md` with:
  1. The watchdog is a **sandbox-side** Tokio task, therefore a legitimate `RequestBackground` target.
  2. The game itself is **not** a sandbox process (confirms `docs/research/flatpak-bundling/12-risks.md` §1 Correction 2 and `10-evidence.md` Theme E).
  3. Current risk: `xdg-desktop-portal-*` may terminate CrossHook's sandbox process if the user minimizes the Tauri window during a long game session; the watchdog then never fires the gamescope SIGTERM and the compositor leaks.
- Acceptance check: doc names the exact function (`gamescope_watchdog`), spawn site (`spawn_gamescope_watchdog` @ `src-tauri/src/commands/launch.rs:1067`), and states explicitly that no `RequestBackground` call exists anywhere in the repo (verified with grep in this investigation).
- Risk: Low.

### 1.3 Audit Flatpak manifest permissions and prior ADRs

- Depends on `[]`
- Files (read-only):
  - `packaging/flatpak/dev.crosshook.CrossHook.yml`
  - `docs/architecture/adr-0001-platform-host-gateway.md`
  - `docs/internal/host-tool-dashboard.md`
- Output: append to `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md`:
  1. Required `finish-args` additions:
     - `--talk-name=org.freedesktop.portal.Desktop` (xdg-desktop-portal for Background + GameMode subportals). Today the manifest has only `--talk-name=org.freedesktop.Flatpak`.
  2. Whether the GNOME 50 runtime exposes a D-Bus client library usable from Rust without new system deps (prefer a zbus dependency since the crate graph has no D-Bus client today; `zbus = { default-features = false, features = ["tokio"] }` is the smallest viable addition).
  3. ADR-0001 scope: these changes must still flow through `platform.rs` since any fallback path to `gamemoderun` is a denylisted host tool.
- Acceptance check: doc finalizes the permission and dependency list and flags zbus as a new crate subject to review.
- Risk: Low.

---

## Step 2 — Design contracts (Batch B)

### 2.1 Design the GameMode portal contract (pure design doc)

- Depends on `[1.1, 1.3]`
- File (new): `docs/architecture/adr-0002-flatpak-portal-contracts.md`
- Content:
  - One reusable Rust contract in `crosshook-core/src/platform/portals/gamemode.rs` with:
    - `enum GameModeBackend { Portal, HostGamemodeRun, Unavailable }`
    - `fn resolve_backend(is_flatpak: bool, portal_available: bool, host_gamemoderun_available: bool) -> GameModeBackend`
    - `async fn register_self_pid_with_portal() -> Result<GameModeRegistration>` (RAII drop-unregisters)
    - `async fn portal_available() -> bool` (D-Bus `Ping` on `org.freedesktop.portal.Desktop`, object path `/org/freedesktop/portal/desktop`, interface `org.freedesktop.portal.GameMode`)
  - Decision matrix documented in the ADR: Flatpak + portal reachable → Portal for CrossHook's own PID; **games** continue to use `gamemoderun` via `host_command_with_env()`; native → `gamemoderun` for everything (no portal).
  - Document the known PID-namespace caveat: the portal already handles sandbox→host PID translation for CrossHook, but does **not** help register host game PIDs (which is correct — they are already host PIDs and use `gamemoderun`).
- Acceptance check: ADR lists one reusable contract owner, clearly distinguishes "self-register" from "wrap host games", names the denylist implications, and records the `RequestBackground` link so the two portals are treated uniformly.
- Risk: Low. Design only.

### 2.2 Design the Background portal contract (pure design doc)

- Depends on `[1.2, 1.3]`
- File: extend the same ADR `docs/architecture/adr-0002-flatpak-portal-contracts.md`
- Content:
  - Contract in `crosshook-core/src/platform/portals/background.rs`:
    - `async fn request_background(reason: &str, autostart: bool) -> Result<BackgroundGrant>`
    - Grant holds the D-Bus request handle and times out gracefully (debounced single in-flight request).
    - No-op on native (non-Flatpak) builds — `is_flatpak()` gate.
  - Lifecycle: CrossHook requests background **once** at app start (Tauri `setup` closure) and re-requests if revoked. The watchdog spawn sites (`spawn_gamescope_watchdog` in `src-tauri/src/commands/launch.rs`) observe the grant but never block launches on it.
  - Failure mode: if the portal returns "denied", the watchdog still runs but we log a single `tracing::warn!` and surface the capability as degraded in the host tool dashboard (reusing `CapabilityState::Degraded` from `crosshook-core/src/onboarding/capability.rs`).
  - Explicit non-goal: do **not** try to track host game PIDs with this portal.
- Acceptance check: ADR ends with a single table mapping each call site (app setup, watchdog spawn, portal revoke handler) to the corresponding function in `platform/portals/background.rs`.
- Risk: Low. Design only.

---

## Step 3 — Platform module scaffolding (Batch C)

### 3.1 Introduce `crosshook-core/src/platform/` module layout with GameMode portal

- Depends on `[2.1]`
- Files (new + refactor):
  - Convert `src/crosshook-native/crates/crosshook-core/src/platform.rs` into a directory module `crosshook-core/src/platform/mod.rs` (split boundary — the file is already ~1,434 lines; issue's "maintainability constraints" explicitly asks for split modules).
    - `platform/mod.rs` keeps the current public gateway API (`is_flatpak`, `host_command*`, `host_std_command*`, `normalize_flatpak_host_path`, `host_command_exists`, `host_path_is_*`, `override_xdg_for_flatpak_host_access`) as `pub use` re-exports so existing imports (`crate::platform::host_command`, `crate::platform::is_flatpak`, etc.) stay stable.
    - Move the existing `host_command*`/`host_std_command*` implementation bodies into `platform/host_gateway.rs` (verbatim; behaviour must not change).
    - Move XDG override into `platform/xdg.rs`.
    - Move host-path probes (`normalize_flatpak_host_path`, `is_allowed_host_system_compat_listing_path`, `host_path_is_*`, `host_read_*`, `normalized_path_*`) into `platform/host_fs.rs`.
  - Add `platform/portals/mod.rs`, `platform/portals/gamemode.rs` implementing the contract from 2.1.
  - Add `zbus = "5"` (with tokio feature, default-features = false) to `src/crosshook-native/crates/crosshook-core/Cargo.toml`.
- Interfaces/functions exposed:
  - `platform::portals::gamemode::resolve_backend`
  - `platform::portals::gamemode::portal_available`
  - `platform::portals::gamemode::register_self_pid_with_portal` (returns `GameModeRegistration` guard)
- Acceptance check:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` still passes; no existing import in the workspace needs to change.
  - `rg "use crate::platform::"` / `rg "crosshook_core::platform::"` all still resolve (re-exports preserved).
  - Unit tests in `platform/portals/gamemode.rs` cover the three `GameModeBackend` resolution branches with fake inputs (no live D-Bus in tests).
- Risk: Medium. Refactor risk is in the file split; the implementation bodies must be moved verbatim. Mitigation: split is literal `git mv` + module declarations; no logic edits in this step.

### 3.2 Add the Background portal implementation

- Depends on `[2.2]`
- Files (new):
  - `crosshook-core/src/platform/portals/background.rs`
- Interfaces/functions exposed:
  - `pub async fn request_background(reason: &str, autostart: bool) -> Result<BackgroundGrant, BackgroundError>`
  - `pub struct BackgroundGrant { handle: zbus::Connection, request_path: zvariant::OwnedObjectPath }`
  - `pub fn background_supported() -> bool` (returns `is_flatpak()`; native builds return `false`).
- Acceptance check:
  - Unit tests assert the no-op path on native builds and cover the serde/JSON of the Background request response parser with recorded fixtures.
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- platform::portals::background` passes.
- Risk: Medium. D-Bus type handling via `zbus::Proxy` needs careful `Variant` parsing; mitigation: tests parse recorded response fixtures rather than hitting D-Bus.

---

## Step 4 — Integration + docs (Batch D)

### 4.1 Wire GameMode portal into launch flow

- Depends on `[3.1]`
- Files to edit:
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` — when `use_gamemode` is active **and** `is_flatpak()` returns true **and** `portals::gamemode::portal_available().await` returns true, register CrossHook's self PID with the portal before appending `gamemoderun` to `wrappers`. Keep the wrapper (games are host processes; the wrapper stays the game-facing path).
  - `src/crosshook-native/src-tauri/src/commands/launch.rs` — hold the returned `GameModeRegistration` guard for the duration of the launch operation state; drop on launch exit.
  - `packaging/flatpak/dev.crosshook.CrossHook.yml` — add `--talk-name=org.freedesktop.portal.Desktop` under `finish-args`.
  - `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` — the existing `gamemode` capability gains a new derived state "portal available but host binary missing → Degraded" (keyed off `portals::gamemode::portal_available` at probe time) and persists through `check_generalized_readiness`.
- Acceptance check:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes.
  - A new test `use_gamemode_registers_portal_when_flatpak_and_portal_available` asserts the wrapper chain still contains `gamemoderun` and the registration guard is returned.
  - Manual verification stub: capture a `tracing` log line `"gamemode portal registration: backend=Portal"` emitted exactly once per launch.
- Risk: Medium. Scope trap: do **not** touch the `gamemoderun` wrapper for host games — the portal is for CrossHook's own PID only. Reviewer must confirm both paths execute.

### 4.2 Wire `RequestBackground` into app setup and watchdog lifetime

- Depends on `[3.2]`
- Files to edit:
  - `src/crosshook-native/src-tauri/src/lib.rs` — inside the `.setup(|app| { ... })` closure (around line 147), spawn one task that calls `platform::portals::background::request_background("keep CrossHook running during launches", /*autostart=*/ false).await`. Hold the grant in a Tauri managed state (`.manage(BackgroundGrantHolder::new(grant))`) so drop equals revoke at app shutdown.
  - `src/crosshook-native/src-tauri/src/commands/launch.rs` — in `spawn_gamescope_watchdog` (line 1067), log `background_supported()` / grant status at watchdog start so we can correlate reports where the watchdog died under minimize; if supported but grant is `None`, re-request once before spawning.
  - `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` — new synthetic capability `background_protection` with `CapabilityState::Available | Degraded | Unavailable`, reported by the host tool dashboard.
- Acceptance check:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes.
  - New test `background_capability_is_available_only_on_flatpak_with_grant` covers the three state transitions using the fake portal injection point.
  - Manual sanity: start CrossHook as AppImage → `background_supported()` returns false and zero D-Bus calls happen (verified by not depending on `--talk-name=org.freedesktop.portal.Desktop` at runtime on native).
- Risk: Medium. Must debounce repeated requests; ensure one grant per app lifetime and that test doubles don't leak tokio tasks.

### 4.3 Documentation + internal notes

- Depends on `[3.1, 3.2]`
- Files to edit:
  - `docs/architecture/adr-0001-platform-host-gateway.md` — append a cross-reference "Portal contracts (ADR-0002)" subsection explaining the relationship (portals for self-PID registration; host tools for game-facing wrappers).
  - `docs/architecture/adr-0002-flatpak-portal-contracts.md` — mark "Accepted".
  - `docs/internal/host-tool-dashboard.md` — add a short subsection "Background protection & GameMode portal" linking to the ADR.
  - `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md` — append an "Implementation" subsection that points at the ADR and Phase 1.4 / 2.4 tasks from `docs/research/flatpak-bundling/14-recommendations.md` so the research tracker #276 has a closure path.
- Acceptance check: `rg -n "adr-0002" docs/` shows at least the ADR itself plus the ADR-0001 cross-reference and the host-tool-dashboard link.
- Risk: Low.

---

## Step 5 — Tests + compliance (Batch E)

### 5.1 Rust unit and integration tests

- Depends on `[4.1, 4.2]`
- Files (new or extended):
  - `crosshook-core/src/platform/portals/gamemode.rs` — tests for `resolve_backend` truth table (Flatpak+portal; Flatpak+no-portal; native).
  - `crosshook-core/src/platform/portals/background.rs` — tests for parser + no-op-on-native; record-and-replay fixtures under `crosshook-core/tests/fixtures/portals/`.
  - `crosshook-core/src/launch/optimizations.rs` — extend `gamemode_wrapper_chain_includes_gamemoderun` to also assert the portal registration path is taken (via injected fake).
  - `crosshook-core/src/onboarding/capability.rs` — test that `background_protection` is omitted on native, reported `Available` on Flatpak with a grant, `Degraded` on Flatpak with denial.
- Acceptance check: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes with zero new warnings (workspace lints are `-D warnings`).
- Risk: Low-Medium. Injecting a fake portal requires a small trait seam in `portals/mod.rs`; keep it test-only (`#[cfg(test)]` dyn dispatch).

### 5.2 Host-gateway compliance and lint checks

- Depends on `[4.1]`
- Commands to verify:
  - `bash scripts/check-host-gateway.sh` — `gamemoderun` is in the denylist; the new code must still call `gamemoderun` only through the existing `platform::host_command*` APIs (no direct `Command::new("gamemoderun")`).
  - `bash scripts/lint.sh` — rust + ts + shellcheck.
- Acceptance check: both scripts exit 0.
- Risk: Low. The only change to `gamemoderun` usage is adding portal registration **around** the existing wrapper, not replacing it.

### 5.3 Metadata/persistence audit

- Depends on `[4.2]`
- Files (review only):
  - `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- Verify:
  - **No schema bump is needed.** Per the issue's "Storage strategy": portal/background grant state is **runtime-only**; any derived capability (e.g. `background_protection`) is re-derived from `is_flatpak()` + a live probe each time, so it reuses the existing `host_readiness_snapshots` row (schema v21) with no new column. If a cached hint is ever persisted, it must reuse `host_readiness_snapshots`/`readiness_nag_dismissals` — not a new table.
  - No new TOML setting is introduced (issue explicitly scopes user-facing background preferences out).
- Acceptance check: `grep -n "schema_version" src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` still shows 21 (unchanged). Document this explicitly in the ADR.
- Risk: Low.

---

## Testing Strategy

- **Unit tests (Rust)**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` covers:
  - `platform::portals::gamemode::resolve_backend` truth table
  - `platform::portals::background::request_background` parsing + native no-op
  - `launch::optimizations` gamemode wrapper + portal registration interaction
  - `onboarding::capability` new `background_protection` capability
- **Integration (host-gateway contract)**: `scripts/check-host-gateway.sh` + `scripts/lint.sh`.
- **Manual Flatpak smoke (documented but not blocking CI)**: build with `./scripts/build-native-container.sh`, install locally, minimize window during launch, confirm watchdog still fires SIGTERM on game exit. Documented in `docs/internal/host-tool-dashboard.md`.
- **Frontend**: no test framework exists in-repo. Host tool dashboard UI is not changed in this issue beyond consuming the existing `HostToolCheckResult` — no UI work required.

## Risks & Mitigations

- **Risk**: GNOME 50 runtime may not expose the GameMode portal D-Bus interface in the sandbox.
  - Mitigation: `portal_available()` probe keeps the contract graceful; worst case the backend is `HostGamemodeRun` (current behaviour). Documented in 2.1.
- **Risk**: `platform.rs` split (step 3.1) changes a 1,434-line file with active test coverage.
  - Mitigation: literal file-move refactor only; existing tests must stay put behind `pub use`. CI `cargo test` is the gate.
- **Risk**: `RequestBackground` denied by the portal backend (e.g. KDE/Plasma can prompt the user).
  - Mitigation: degrade to `CapabilityState::Degraded` and keep the watchdog running; log once.
- **Risk**: Adding `zbus` increases build time for `crosshook-core`.
  - Mitigation: restrict features to `tokio`; disable default features; document the crate graph delta in the ADR.
- **Risk**: Scope creep into host-game "background protection" (explicitly a non-goal).
  - Mitigation: the ADR (2.2) and the ground-truth doc (1.2) state explicitly that host game PIDs are never passed to `RequestBackground`; code review gate on 4.2 checks for this.

## Host-gateway compliance

No new direct `Command::new("<denylisted>")` calls are introduced. The only denylisted tool involved (`gamemoderun`) continues to flow through `platform::host_command*` via the existing `launch/optimizations.rs` wrapper path. ADR-0001 scope is unchanged; ADR-0002 layers on top.

## Schema / metadata DB

No schema bump. Current schema version remains 21. No new tables. Runtime-only state reuses existing `host_readiness_snapshots`. No new TOML settings.

## Success Criteria

- [ ] `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md` lands and tracks back to issue #276.
- [ ] `docs/architecture/adr-0002-flatpak-portal-contracts.md` is Accepted and cross-referenced from ADR-0001.
- [ ] `crosshook-core/src/platform/portals/gamemode.rs` and `background.rs` exist with unit-test coverage.
- [ ] GameMode continues to reach host games through `gamemoderun`; CrossHook's own PID is registered via `org.freedesktop.portal.GameMode` under Flatpak when the portal is reachable.
- [ ] CrossHook's watchdog is protected by `RequestBackground` under Flatpak; native builds make zero portal calls.
- [ ] `scripts/check-host-gateway.sh` passes (no new denylist bypasses).
- [ ] `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes with `-D warnings`.
- [ ] `packaging/flatpak/dev.crosshook.CrossHook.yml` declares `--talk-name=org.freedesktop.portal.Desktop`.
- [ ] Schema version unchanged (21).
- [ ] Host tool dashboard exposes a `background_protection` capability row with `Available` / `Degraded` / `Unavailable` and clear copy that it applies to CrossHook itself, not to host-launched games.
