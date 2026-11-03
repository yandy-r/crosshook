# Plan — session registry events (YAN-824 / #551)

Spec: Linear YAN-824 acceptance criteria. Binding design: `decisions.md` (D1–D7). Paths relative to `src/crosshook-native/`.

## Batch 1 (parallel, disjoint files)

### B1-backend — Rust (core, app, src-tauri)

1. `crates/crosshook-core/src/launch/session/`: `State { sessions, seq }`, `SessionChange` / `SessionInfo` types, `with_observer`, `deregister_with_exit -> bool`, `snapshot()`; registry tests (D1, D2).
2. `crates/crosshook-app/src/events.rs`: `SessionStarted` / `SessionExited` (D4).
3. `crates/crosshook-app/src/services/launch/`: `LaunchService::new` takes an `EventBus` and builds the registry `with_observer`; finalizer passes code/signal; spawn failure path; `running_sessions_snapshot()`; Steam game watcher with injected probe and handoff (D3). Wire `bootstrap/stores.rs` and `tests/helpers.rs`. New `tests/sessions.rs` + watcher tests (D7).
4. `src-tauri`: new `session_bridge.rs` (+ tests) emitting `launch-session-changed`; `list_running_sessions` command replaces `list_running_profiles`; `LaunchResult.session_id`; contract test; `lib.rs` registration.
5. `crates/crosshook-app/README.md` LaunchService row.

### B1-frontend — React / mocks

1. `src/types/launch.ts`: wire types from D4; `LaunchResult.session_id`.
2. `src/hooks/runningSessionsStore.ts` + `useRunningProfiles.ts` rewrite (D5).
3. `src/hooks/useLaunchState.ts`: drop interval, correlate by `session_id`.
4. Mocks: new sibling handler for sessions; `list_running_sessions`; emitted events from `_mock_set_profile_running`, mock launch and completion; remove `list_running_profiles` mock and `wrapHandler` entry (D6).
5. Vitest updates + new store tests; Playwright smoke unchanged and green.

## Gate (orchestrator)

`cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` (core/app/native, ×2), `npm run typecheck`, `npm test`, `npm run test:smoke`, `./scripts/lint.sh`, `./scripts/check-mock-coverage.sh`, `./scripts/check-app-deps.sh`, timer grep from `research-risks.md` §4.6.
