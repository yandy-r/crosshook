# YAN-798 / GitHub #534 — M1-05 launch service

Target release: crosshook v0.7.0-alpha.1 (base `main`).

_Paths are relative to_ `src/crosshook-native/`_._

## Context

About 1,700 lines in `src-tauri/src/commands/launch/` hold launch orchestration. The files are `execution.rs`, `streaming.rs`, `warnings.rs`, `shared.rs`, `portal.rs` and `diagnostics.rs`. They contain:

- UMU GAMEID enrichment;
- offline, trainer-hash, low-disk and mod-coexistence warnings;
- method resolution;
- the gamescope gating predicate, duplicated between the game and trainer paths;
- GameMode portal registration;
- pre- and post-launch hooks;
- session registration, with atomic register-and-link for trainers;
- watchdog selection and the background-grant wait;
- log streaming;
- the finalizer: diagnostics, `record_launch_finished`, version snapshot, known-good tagging, injection-log lifecycle and the `cancel_linked_children` cascade.

The `launch-log` payload has no session id, so game and trainer lines can't be told apart. `crosshook-cli` duplicates streaming and tail reading (`stream_helper_log`, `safe_read_tail`).

## Scope

- Create `crates/crosshook-app/src/services/launch/` with `game.rs`, `trainer.rs`, `gamescope.rs`, `finalizer.rs`, `warnings.rs` and `streaming.rs`.
- Expose `start_game(req)` and `start_trainer(req)`, each returning `LaunchSession { id, kind, warnings, events }`.
  - `LaunchEvent` is `Log { session_id, batch }`, `Injection`, `Diagnostic` or `Exited { code, signal }`.
- Build requests with M1-01, resolve helpers with M1-29, and stream and tail through M1-04. The 500 ms whole-file re-read goes away.
- Use one `gamescope_required(req, settings)` predicate for both the game and trainer paths, and spawn the watchdog only when it is true.
- Preserve trainer parity:
  - Steam trainers run with `proton_run` semantics.
  - Trainers register with `LaunchSessionRegistry` and link to their parent atomically.
  - Parent teardown triggers `cancel_linked_children`.
  - Each session owns its own `ShutdownTarget`; a session never kills another session's process tree.
- Inject a `ProcessSpawner` trait so the finalizer and cascade can be tested without real Proton.
- Move `list_launch_history_for_profile`, `validate_launch`, `preview_launch` and `build_steam_launch_options_command` into the same service.
- The Tauri adapter maps events to the existing `launch-log`, `launch-diagnostic`, `launch-complete` and `injection-log` names and payloads. React is unchanged except for an optional `session_id` field.
- `crosshook-cli launch` uses this service and its tail, and `stream_helper_log` and `safe_read_tail` are deleted.

## Storage boundary

No format changes. The SQLite launch history, version snapshots and known-good tags are written exactly as today, and sessions stay runtime-only.

## Acceptance criteria

- [ ] Predicate table tests cover method × Flatpak × gamescope setting × game/trainer.
- [ ] A parity test shows that a Steam-profile trainer request equals a `proton_run` trainer request in env, `working_directory` and gamescope config.
- [ ] `#[tokio::test]`s cover the finalizer side effects (history row, version snapshot, known-good tag) and the linked-child cascade, including that a sibling session survives.
- [ ] Tauri `launch_game` and `launch_trainer` delegate to the service with behaviour unchanged, and React smoke passes.
- [ ] The CLI dry-run and launch output is unchanged for fixture profiles.
- [ ] Every event carries `session_id`.

## Test plan

- Workspace `cargo test`
- Manual launches:
  - a `proton_run` game plus its trainer, verifying the cascade on game exit;
  - a `steam_applaunch` game plus its trainer inside Flatpak.

---

**Plan key:** `M1-05` · **Release:** v0.7.0-alpha.1 · **Estimate:** 8 pts · **GitHub labels:** `type:refactor`, `area:launch`, `priority:high` · **Blocked by:** M1-01, M1-04, M1-29
