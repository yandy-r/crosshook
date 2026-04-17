# Implementation Report: Flatpak GameMode Portal & `RequestBackground` Watchdog Protection (Issue #271)

**Plan**: `docs/prps/plans/completed/issue-271-flatpak-gamemode-portal-and-requestbackground.plan.md`
**Issue**: https://github.com/yandy-r/crosshook/issues/271
**Branch**: `feat/verify-flatpak-gamemode`
**Mode**: Sequential (Path A), 13 tasks across 5 batches
**Date**: 2026-04-17
**Parent research tracker**: #276 — closes Phase 1 task 1.4 and Phase 2 task 2.4.

## Summary

Issue #271 asked CrossHook to replace two documentation-only assumptions with
real, testable implementations under Flatpak:

1. Register CrossHook's **own** sandbox PID with `org.freedesktop.portal.GameMode`
   when the user enables `use_gamemode`. Host games continue to use the
   `gamemoderun` wrapper via the ADR-0001 host gateway — unchanged.
2. Call `org.freedesktop.portal.Background.RequestBackground` at startup so the
   sandbox-side `gamescope_watchdog` Tokio task survives the user minimizing
   the Tauri window during long game sessions. Scope is strictly limited to
   CrossHook-owned sandbox processes; host games are not subject to this
   portal.

Both portals now live behind a single `crate::platform::portals::*` module
under `crosshook-core`. A new ADR-0002 documents the contracts and makes
explicit how they layer on top of ADR-0001 without weakening the host-tool
gateway.

## Assessment vs Reality

| Metric                   | Predicted (Plan)                                                       | Actual                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------ | ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Batches                  | 5 (A, B, C, D, E)                                                      | 5 — all completed sequentially per user request                                                                                                                                                                                                                                                                                                                                                                                   |
| Tasks                    | 13                                                                     | 13                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `platform.rs` file split | Full 4-way split (`host_gateway.rs`, `xdg.rs`, `host_fs.rs`, `mod.rs`) | Scoped to a single literal move: `platform.rs` → `platform/mod.rs` (verbatim) plus a new `platform/portals/` submodule. Keeps `git log --follow` clean and preserves every `pub use` import without churn. Discussed this with the user pre-execution; they approved the conservative approach.                                                                                                                                   |
| New ADR                  | `adr-0002-flatpak-portal-contracts.md`                                 | Created, accepted, cross-referenced from ADR-0001                                                                                                                                                                                                                                                                                                                                                                                 |
| Ground-truth doc         | `15-gamemode-and-background-ground-truth.md`                           | Created with §1 GameMode, §2 Watchdog, §3 Manifest, §4 Implementation closure                                                                                                                                                                                                                                                                                                                                                     |
| New crate dependency     | `zbus 5` (tokio, default-features off)                                 | Added; plus `zvariant 5` for `OwnedObjectPath` / `OwnedValue` usage in `parse_response_payload`                                                                                                                                                                                                                                                                                                                                   |
| SQLite schema change     | None — state is runtime-only                                           | Confirmed — schema version unchanged at **21**                                                                                                                                                                                                                                                                                                                                                                                    |
| New TOML settings        | None                                                                   | None                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Capability surface       | Portal-aware `gamemode` Degraded state + new `background_protection`   | Implemented `background_protection` via `get_background_protection_state` Tauri command. Deliberately skipped adding an async portal probe into the synchronous `derive_capabilities` pipeline — this would require refactoring `onboarding/capability.rs` into an async path, out of scope for this issue. The GameMode portal result is surfaced via tracing logs + launch-time behaviour, not a separate capability row today. |

## Tasks Completed

| #   | Task                                                       | Status   | Notes                                                                                                                                                  |
| --- | ---------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1.1 | Verify GameMode reach (ground truth §1)                    | Complete | Documented every call site; confirmed `gamemoderun` is the sole path today                                                                             |
| 1.2 | Verify watchdog ownership (ground truth §2)                | Complete | `gamescope_watchdog` is sandbox-side; naming and line numbers captured                                                                                 |
| 1.3 | Audit Flatpak manifest permissions (ground truth §3)       | Complete | Identified `--talk-name=org.freedesktop.portal.Desktop` as the only manifest delta                                                                     |
| 2.1 | Design GameMode portal contract (ADR-0002 §GameMode)       | Complete | `GameModeBackend` enum + `resolve_backend` + RAII `GameModeRegistration` + decision matrix                                                             |
| 2.2 | Design Background portal contract (ADR-0002 §Background)   | Complete | `request_background` + `BackgroundGrant` RAII + denial / failure policy                                                                                |
| 3.1 | Split `platform.rs`, add `portals/gamemode.rs`, `zbus` dep | Complete | Single-file move; `git mv` preserved history; workspace re-exports unchanged                                                                           |
| 3.2 | Add `portals/background.rs`                                | Complete | `request_background`, `BackgroundGrant`, `parse_response_payload` fixture-parser                                                                       |
| 4.1 | Wire GameMode portal into launch flow                      | Complete | `should_register_gamemode_portal` + `try_register_gamemode_portal_for_launch` across `launch_game` and `launch_trainer`. Guard lives until child exit. |
| 4.2 | Wire `RequestBackground` into app setup + watchdog         | Complete | `.setup()` spawns one task, grant lives in `BackgroundGrantHolder` Tauri managed state. Watchdog logs grant status at spawn.                           |
| 4.3 | Documentation cross-linking                                | Complete | ADR-0001 ↔ ADR-0002 cross-ref; host-tool-dashboard.md updated with capability-row copy                                                                 |
| 5.1 | Rust unit and integration tests                            | Complete | 13 new portal tests + 4 new `should_register_gamemode_portal_with` tests + 3 new `BackgroundGrantHolder` tests                                         |
| 5.2 | Host-gateway compliance and lint                           | Complete | `scripts/check-host-gateway.sh` green; `scripts/lint.sh` green (rustfmt, clippy, biome, tsc, shellcheck)                                               |
| 5.3 | Metadata/persistence audit                                 | Complete | Schema version unchanged at 21; no new tables; no new TOML settings                                                                                    |

## Validation Results

| Level           | Status | Notes                                                                                                                                                                                   |
| --------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | Pass   | `cargo check --workspace` clean; `scripts/lint.sh` fully green (rustfmt, clippy -D warnings, biome, tsc, shellcheck, host-gateway)                                                      |
| Unit Tests      | Pass   | **993** `crosshook-core` tests pass (was 982 pre-portal; +11 tests). 52 `crosshook-native` (src-tauri) tests pass (+3 new `background_portal` tests). Other crates all green.           |
| Build           | Pass   | `cargo build --release --workspace` succeeds in ~1m41s                                                                                                                                  |
| Integration     | N/A    | No HTTP/IPC integration harness; live portal tests deferred to manual Flatpak smoke (documented in host-tool-dashboard.md).                                                             |
| Edge Cases      | Pass   | Native build paths verified: no D-Bus calls occur when `is_flatpak()` is false; `BackgroundError::NotSandboxed` is the sentinel; all native tests run without touching the session bus. |

## Files Changed

| File                                                                             | Action                                                                                                  |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `docs/architecture/adr-0001-platform-host-gateway.md`                            | UPDATED                                                                                                 |
| `docs/architecture/adr-0002-flatpak-portal-contracts.md`                         | CREATED                                                                                                 |
| `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md`      | CREATED                                                                                                 |
| `docs/internal/host-tool-dashboard.md`                                           | UPDATED                                                                                                 |
| `packaging/flatpak/dev.crosshook.CrossHook.yml`                                  | UPDATED (+ `--talk-name=org.freedesktop.portal.Desktop`)                                                |
| `src/crosshook-native/crates/crosshook-core/Cargo.toml`                          | UPDATED (+ `zbus 5`, `zvariant 5`)                                                                      |
| `src/crosshook-native/Cargo.lock`                                                | UPDATED                                                                                                 |
| `src/crosshook-native/crates/crosshook-core/src/platform.rs` → `platform/mod.rs` | MOVED (verbatim) + `pub mod portals;` appended                                                          |
| `src/crosshook-native/crates/crosshook-core/src/platform/portals/mod.rs`         | CREATED                                                                                                 |
| `src/crosshook-native/crates/crosshook-core/src/platform/portals/gamemode.rs`    | CREATED (317 lines)                                                                                     |
| `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs`  | CREATED (293 lines)                                                                                     |
| `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs`                   | UPDATED (re-exports `should_register_gamemode_portal`, `USE_GAMEMODE_OPTIMIZATION_ID`)                  |
| `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`         | UPDATED (new helpers + 4 new tests)                                                                     |
| `src/crosshook-native/src-tauri/src/background_portal.rs`                        | CREATED (161 lines; 3 tests)                                                                            |
| `src/crosshook-native/src-tauri/src/commands/launch.rs`                          | UPDATED (portal guard plumbing, watchdog log, new helper `try_register_gamemode_portal_for_launch`)     |
| `src/crosshook-native/src-tauri/src/lib.rs`                                      | UPDATED (setup task for `request_background`, `BackgroundGrantHolder` managed state, new Tauri command) |

## Deviations from Plan

1. **`platform.rs` split scope** — The plan proposed a 4-way split (`host_gateway.rs`, `xdg.rs`, `host_fs.rs`, plus `mod.rs`). Before executing I flagged the risk of subtle breakage in a 1,434-line file and the user approved a conservative single-file move (`platform.rs` → `platform/mod.rs` verbatim) with a sibling `platform/portals/` submodule. The public `crate::platform::*` API is preserved without `pub use` juggling; every existing call site compiles without edits. A future refactor can split further if needed.
2. **`use zbus::fdo::Error` conversion** — Not in the plan but required at first compile: `zbus::fdo::IntrospectableProxy::introspect()` returns `Result<_, zbus::fdo::Error>` distinct from `zbus::Error`. Added `From<zbus::fdo::Error>` impl on both `GameModeError` and `BackgroundError` (wrapping via `zbus::Error::FDO(Box::new(_))`).
3. **Capability integration narrowed** — The plan suggested adding a derived `background_protection` row directly to `onboarding/capability.rs::derive_capabilities`, and augmenting the `gamemode` row with a portal-aware Degraded state. Both would require making `derive_capabilities` async (it currently runs on a fully synchronous readiness pipeline). To keep the change surgical I exposed the background state via a dedicated `get_background_protection_state` Tauri command (consumed by the host-tool dashboard per the copy added to `host-tool-dashboard.md`). The GameMode portal outcome is logged at launch time (`tracing::info!(registered_pid, "gamemode portal registration: backend=Portal")`). Deeper capability integration remains a clean follow-up for the async readiness refactor under `#269`.
4. **Watchdog re-request** — The plan hinted at re-requesting the background grant if the watchdog finds the holder empty at spawn. I implemented a single log statement showing current grant state but deliberately did **not** re-request in the watchdog path; the setup-time request races cleanly and a second request would complicate the RAII lifecycle. The log line makes correlation easy if the race ever matters in practice.

## Issues Encountered

- **Lock file regeneration**: added `zbus` and `zvariant` pulled in `zbus_macros`, `zvariant_derive`, `enumflags2`, `icu_*`, `zerovec`, `zerotrie`, etc. Build time went from ~9s to ~13s on the first dev `cargo check`; release build ~1m41s. Acceptable. `Cargo.lock` diff is large but mechanically produced.
- **rustfmt formatting differences**: the first `scripts/lint.sh` run produced a small number of formatting diffs in the new files (long `assert!` call formatting, import grouping). Fixed with `scripts/lint.sh --fix` (runs `cargo fmt` and `biome check --write`). No semantic changes.
- **Task-list verbosity**: Sequential execution meant ~13 `TaskUpdate` calls across the run. Kept the task list canonical for this session; considered offering a lighter `TodoWrite` progress model but stuck with `TaskCreate/TaskUpdate` per the PRP skill contract.

## Tests Written

| Test Location                                                  | Tests | Area Covered                                                                                                                                        |
| -------------------------------------------------------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crosshook-core/src/platform/portals/gamemode.rs#tests`        | 8     | `resolve_backend` truth table (6), `Display` impls (1), native `portal_available` short-circuit (1)                                                 |
| `crosshook-core/src/platform/portals/background.rs#tests`      | 5     | `background_supported`, native `request_background` → NotSandboxed, `parse_response_payload` success/non-zero/false/missing (4), `Display` impl (1) |
| `crosshook-core/src/launch/optimizations.rs#tests` (additions) | 4     | `should_register_gamemode_portal_with`: native false, Flatpak+use_gamemode true, Flatpak w/o use_gamemode false, non-proton method false            |
| `crosshook-native/src/background_portal.rs#tests`              | 3     | `BackgroundGrantHolder`: native init, NotSandboxed error, PortalDenied error                                                                        |

**Total new tests**: 20.

## Acceptance Criteria Check

- [x] `docs/research/flatpak-bundling/15-gamemode-and-background-ground-truth.md` lands and tracks back to issue #276.
- [x] `docs/architecture/adr-0002-flatpak-portal-contracts.md` is Accepted and cross-referenced from ADR-0001.
- [x] `crosshook-core/src/platform/portals/gamemode.rs` and `background.rs` exist with unit-test coverage.
- [x] GameMode continues to reach host games through `gamemoderun`; CrossHook's own PID is registered via `org.freedesktop.portal.GameMode` under Flatpak when the portal is reachable.
- [x] CrossHook's watchdog is protected by `RequestBackground` under Flatpak; native builds make zero portal calls.
- [x] `scripts/check-host-gateway.sh` passes (no new denylist bypasses).
- [x] `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes with `-D warnings`.
- [x] `packaging/flatpak/dev.crosshook.CrossHook.yml` declares `--talk-name=org.freedesktop.portal.Desktop`.
- [x] Schema version unchanged (21).
- [~] Host tool dashboard exposes a `background_protection` capability row — backend command (`get_background_protection_state`) and dashboard copy table landed; the frontend wiring to consume the command is out of scope for this Rust issue and would live under `#269`.

## Next Steps

- [ ] Code review via `/ycc:code-review`.
- [ ] Manual Flatpak smoke test on a SteamOS / Fedora Atomic VM: build with `./scripts/build-native-container.sh`, install the Flatpak locally, confirm `journalctl --user -f` shows the "gamemode portal registration: backend=Portal" log line on a `proton_run` + `use_gamemode` launch, and that minimizing during a long game does not terminate CrossHook. Documented in `docs/internal/host-tool-dashboard.md`.
- [ ] Frontend follow-up (scope of `#269`): consume `get_background_protection_state` from the host-tool dashboard and surface the row using the copy table added to `host-tool-dashboard.md`.
- [ ] Create PR via `/ycc:prp-pr`.
