# M1-05 / YAN-798 — Security review: moving the launch pipeline into `crosshook-app`

Target file: `docs/plans/launch-service/research-security.md`. Issue: [issue.md](issue.md).
Paths are relative to `src/crosshook-native/` unless rooted at the repo.

**Method**: read-only audit of `src-tauri/src/commands/launch/{execution,streaming,shared,diagnostics,portal,queries,warnings}.rs`, `crates/crosshook-cli/src/launch.rs`, `crates/crosshook-core/src/launch/{watchdog/*,session/*,hooks.rs,request/{validation,error_text,models}.rs,script_runner/*,runtime_helpers/*}`, `crates/crosshook-core/src/platform/gateway.rs`, `crates/crosshook-core/src/runtime_helpers/mod.rs`, `crates/crosshook-app/src/{log_tail.rs,jobs/*,error.rs,presenter/mod.rs,services/mod.rs,events.rs}`, `scripts/check-host-gateway.sh`, `docs/architecture/adr-0001-platform-host-gateway.md`, and the React consumers (`src/utils/log.ts`, `src/hooks/useLaunchState.ts`, `src/components/ConsoleView.tsx`, `src/components/layout/ConsoleDrawer.tsx`). Nothing was built, run or tested. `[INFERENCE]` marks claims I did not observe directly.

**Threat model** (same as the profile-writer review): malicious or corrupted profile and log content, and a compromised webview sending arbitrary `LaunchRequest`s. The webview can already launch any `game_path` or hook path as the user, so the goal is to avoid **widening** that capability, not to sandbox it. A same-UID attacker with arbitrary file access is out of scope. A _different_ local user who can write to the shared host `/tmp` is listed only as an advisory.

## Executive summary

No new CRITICAL issue appears if the move is mechanical. Four places in the moved code are where the refactor can break isolation or leak data:

1. **Teardown scoping.** Each session kills only its own gamescope `ShutdownTarget`. The refactor must not "simplify" this into a prefix sweep or process-group kill, because a game and its trainer share one `WINEPREFIX`.
2. **Finalizer guarantees.** Today a panic in the stream task skips `deregister` and `cancel_linked_children`.
3. **Pipe reader.** It stops reading at the first invalid UTF-8 byte.
4. **CLI parity.** The CLI uses a 0700 log directory, a helper ownership check and different output bytes.

Removing the 500 ms whole-file loop takes out one panic path (H-3).

---

## 1. Controls the move must preserve (severity-ranked)

### CRITICAL — hard stops

**C-1. Teardown never crosses session boundaries; no prefix sweep or group kill in the launch path.**

- _Today_: the gamescope watchdog signals only its own `ShutdownTarget { pid, host_namespace }` (`crates/crosshook-core/src/launch/watchdog/mod.rs:24-28`). That target comes from this session's child PID or this session's `<log>.gamescope.pid` capture file (`watchdog/resolver.rs:74-125`). The fallback exe-ancestor search refuses ambiguous matches (`resolve_unique_gamescope_ancestor`, `resolver.rs:127-150`, returns `None` when two gamescope ancestors match). Descendants are taken from that target's subtree only (`tasks.rs:142-174`, `collect_descendant_pids_task` `tasks.rs:215-231`). The registry never reads PIDs (`session/registry.rs:1-6` doc). A cascade sends a `TeardownReason` only to children with `parent == Some(id)` (`registry.rs:200-219`). Reset targets one profile key (`registry.rs:231-236`, `execution.rs:503-509`).
- _Risk_: `crosshook_app::jobs::kill_processes_using_prefix` (`jobs/process.rs:298-362`) is the obvious "tear down Wine" helper. It SIGKILLs **every** process whose environ names the prefix. Trainer and game share the prefix (Flatpak Steam trainers rewrite `runtime.prefix_path = steam.compatdata_path`, `script_runner/proton_trainer.rs:22-38`). Calling it on trainer teardown would kill the game and any sibling session. `supervise_child` (`jobs/process.rs:84`) kills a whole process group. Launch children are not spawned with `process_group(0)`, so it would fail with `InvalidInput`. If someone adds `process_group(0)` to make it work, a cancel would also reach the Flatpak proxy group but **not** the host game, because the launch builders are not `--watch-bus` lifecycle builders (`gateway.rs:52-75` vs `96-123`).
- _Mitigation_: the service's teardown is exactly `gamescope_watchdog` / `drain_cancel_into_outcome` on the session's own `cancel_rx`. Do not add `kill_processes_using_prefix`, `killpg`, or `kill_on_drop`/`supervise_child` to launch children. Acceptance test: "sibling session survives". Register game A, game B (another profile) and trainer T linked to A. Finalize A. Assert that T's receiver got `LinkedSessionExit`, B's receiver got nothing, and B is still in `sessions_for_profile`.

**C-2. Every host tool goes through `crosshook-core::platform`; `ProcessSpawner` must not become a bypass.**

- _Today_: every launch `Command` comes from core builders that use the gateway (`runtime_helpers/proton_command.rs:100,112,277,306`; `script_runner/steam_helpers.rs:55`; hooks `hooks.rs` → `host_std_command_with_env_and_directory`; watchdog `kill`/`ps`/`cat`/`test` via `host_std_command` in `watchdog/tasks.rs:263-301` and `watchdog/host.rs:32-47,70-94`). Tauri only calls `.stdout/.stderr/.spawn()` on them (`execution.rs:127-131,152,316-323,378`). `scripts/check-host-gateway.sh:11-18` already scans `crates/crosshook-app/src`, so the new module is covered.
- _Risk_: a `ProcessSpawner` that takes `program: &str, args` invites `Command::new("gamescope"|"proton"|"umu-run")` in app code. The lexical guard catches literals, but not variables or constants (`check-host-gateway.sh` "ponytail" comment; ADR-0001 § Limitations, line 140). A fake spawner in an inline `#[cfg(test)] mod` under `src/` is scanned too (ADR-0001:108).
- _Mitigation_: define the seam as `fn spawn(&self, cmd: tokio::process::Command) -> io::Result<SpawnedChild>`, taking the already-built core `Command`. The real implementation is `cmd.spawn()`. Fakes go under `src/services/launch/tests/` (path contains `/tests/`, which the scanner exempts). Add `src/crosshook-native/crates/crosshook-cli/src` to `SCAN_DIRS` (cheap; the CLI has no literal `Command::new` today; grep found none) so the CLI side of the reuse is guarded too.

### HIGH

**H-1. The finalizer runs exactly once, even on panic or task abort.**

- _Today_: `spawn_log_stream` runs `stream_log_lines` → `finalize_launch_stream` → `finalize_launch_session` in one task (`streaming.rs:26-59,513-542`). On panic the outer task only logs the `JoinError` (`streaming.rs:54-58`). Nothing then runs `cancel_linked_children`, `run_post_exit_hooks`, `registry.deregister`, `launch-complete` or the GameMode `unregister`. Effects: the trainer is never cascaded (its gamescope tree lives on), the stale game key stays in `list_running_profiles` (`queries.rs:113-118`), the UI never leaves the running phase, and the launch-history row stays "started".
- _Mitigation_: borrow the `FinishGuard` pattern from `jobs/registry.rs:321-380`. A `SessionGuard` owned by the session task performs cascade → post-exit hooks → deregister → `Exited` in `Drop` if the normal path did not. Keep today's order: cascade **before** post-exit hooks **before** deregister (`streaming.rs:517-541`), and diagnostic → complete → injection finalization before that (`streaming.rs:427-444`). Test: a fake spawner whose stream panics must still yield `Exited`, and its linked child must still be cancelled.

**H-2. The pipe reader must not stop on invalid UTF-8 or unbounded lines.** This bug lives in the moved code; fix it as part of the move.

- _Today_: `pipe_reader_task` uses `BufReader::lines()` (`streaming.rs:211-230`). `next_line` returns `InvalidData` on non-UTF-8 output, which Wine/Proton can emit. The task then sends `"crosshook log capture error: …"` and `break`s, dropping the read end. The child's next write to that pipe gets `EPIPE`/`SIGPIPE` `[INFERENCE: depends on child's SIGPIPE handling]`. That can kill or stall the `proton_run` launch and blinds all later logging. `lines()` also buffers a newline-free line without limit, and the channel is `unbounded_channel` (`streaming.rs:151`).
- _Mitigation_: read bytes (`read_until(b'\n')` or reuse `log_tail`'s decoder). Decode lossily, strip `\r`, cap partial lines at `log_tail::PARTIAL_CAP` (64 KiB, `log_tail.rs:42`), and keep draining until EOF whatever the decode result. Keep appending raw lines to the log file (`streaming.rs:159-178`) so the on-disk log and the M1-04 tail stay the source of truth. Test: a child that writes `b"\xff\xfe\n"` then 1 MiB without a newline, then exits 0 → `Exited { code: Some(0) }` and no early stop.

**H-3. Log reads are bounded, lossy and panic-free. What `safe_read_tail` guards today:**

- `safe_read_tail` (`src-tauri/.../diagnostics.rs:8-41`; CLI copy `crosshook-cli/src/launch.rs:321-344`) does six things. It opens the path, which it generates itself and never receives from IPC, so it applies no confinement or symlink check. It reads `metadata()` from the **opened handle**, so there is no TOCTOU on size. It seeks to `End(-MAX_LOG_TAIL_BYTES)` (2 MiB, `core/launch/diagnostics/models.rs:6`). It reads to EOF, so the file can grow after the seek and the read is not strictly capped. It decodes with `from_utf8_lossy`, so a seek that splits a character is harmless. Every error becomes `String::new()`, so no I/O error or path reaches IPC.
- Downstream `analyze` caps each match at `MAX_LINE_DISPLAY_CHARS` = 500 and the entry list at `MAX_DIAGNOSTIC_ENTRIES` = 50 (`diagnostics/patterns.rs:138,152,182-184`).
- The 500 ms loop being deleted (`streaming.rs:80-120`) has two defects. `read_to_string` fails on invalid UTF-8, which silently stops the live log. `&content[last_len..]` panics when a truncate-and-rewrite leaves `last_len` mid-character, and that panic hits H-1. M1-04 `LogTail` (`log_tail.rs:118-176`) fixes both: 256 KiB per poll (`READ_CHUNK`), a 64 KiB partial cap, lossy decode, inode/dev rotation detection, truncation reset, and a 4 MiB final drain on cancel (`FINAL_DRAIN_CHUNKS`).
- _Mitigation_: the finalizer still needs a **bounded** tail of at most 2 MiB. Use one shared `read_tail(path, MAX_LOG_TAIL_BYTES)` in `crosshook-app`: same semantics, plus `take(max_bytes)` on the read so it is truly capped. Or keep a byte-bounded ring buffer of streamed lines. Never accumulate the whole stream. Errors stay empty or log-only. Delete both copies of `safe_read_tail`, as the issue requires.

**H-4. Env injection controls stay on the only path into launched processes.**

- _Today_:
  - `validate(&request)` runs **before** any builder (`execution.rs:72,249`; CLI `launch.rs:59`). It rejects custom-env keys that are empty, contain `=` or NUL, have a NUL value, or are reserved (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`) (`request/validation.rs:20-57,270`).
  - Under Flatpak, `host_command_with_env_*` uses `--clear-env` plus an explicit allowlist (`host_environment_map`, `runtime_helpers/environment.rs:98-120`). Custom vars travel in a `0600` file inside a `0700` directory (`gateway.rs:276-319`). Only `[A-Za-z0-9_]` keys are written there (`is_valid_shell_env_key`, `gateway.rs:256-258,310-316`). Values are single-quote escaped and the file is `rm -f`'d by the shell (`gateway.rs:233-236`). Values never appear on `flatpak-spawn` argv.
  - Natively, `Command::new(program).envs(combined)` inherits the parent environment (`gateway.rs:241-251`); this is current behaviour.
  - Hooks get the same env and strip custom keys from the base map (`hooks.rs:71-73`).
- _Risk_: the service builds requests through M1-01. If any path skips `validate` (for example the CLI dry-run, or `start_trainer` on a request built from a profile), reserved-key overrides reach Proton. Adding env after construction with `.env()` on a Flatpak `Command` is **silently dropped** (`gateway.rs:13-22`). The issue's "parity test: env equal" would then pass natively and fail in Flatpak.
- _Mitigation_: run `validate` inside `start_game`/`start_trainer` as the first step, before warnings, portal registration, hooks or spawn, whatever the request source. Do not inject new env vars (for example a session id) into children. If one is ever needed, add it to the builder's `envs` map. The parity test should compare the `Command`'s argv/env from the **builder** (as `script_runner/tests/*` do with `command_env_value`), and include `--env=` argv under `flatpak=true` through the `_inner` builders.

**H-5. Error strings crossing IPC leak no more than today.**

- _Today_ the Tauri commands return raw `String`s:
  - validation `to_string()`: fixed text plus profile option ids (`request/error_text.rs`);
  - `runtime_helpers::resolve(...)` → `RuntimeHelperError` Display, which lists **every candidate absolute path and the rejected untrusted paths** (`crosshook-core/src/runtime_helpers/mod.rs:150-176`), returned verbatim at `execution.rs:120-121,309-310`;
  - `format!("failed to build …: {error}")`, where the inner `io::Error` wraps validation text, or `format!("{error:?}")` Debug of `CommandArgumentResolveError` (`script_runner/proton_game.rs:28-32`), which echoes user tokens;
  - spawn errors (`io::Error` Display, no path);
  - `create_log_path` I/O text (`jobs/process.rs:228-241`).

  Diagnostics are sanitized with `display_path_tilde` (`~/…`, `presenter/mod.rs:8-24`) in `sanitize_diagnostic_report` (`diagnostics.rs:43-64`). `record_launch_start` persists a sanitized path (`execution.rs:161,420`). `LaunchResult.helper_log_path` is the raw `/tmp/crosshook-logs/…` path (`execution.rs:233,489`). React shows it (`HeroLaunchGate.tsx:222`) and keys state on it (`useLaunchState.ts:214,334,433`).

- _Risk_: a new service error type that wraps `io::Error` with paths or `{:?}` Debug output adds leaks. Switching from `display_path_tilde` (`~/`) to `AppError`'s `sanitize_message` (`$HOME`, which also scrubs other users' homes, `presenter/mod.rs:27-40`) changes **payload text** in `launch-diagnostic`, which conflicts with "payloads unchanged".
- _Mitigation_: keep two channels apart. (a) **Command errors**: map to `AppError` (D11 convention from profile-writer). `AppError::new` already applies `sanitize_message` (`error.rs:37-42`), so helper candidate paths become `$HOME/...`. That change only removes information; no React code parses these strings (grep for "failed to launch", "unable to resolve runtime helper", "unsupported launch method" found no UI matches). Record it in decisions. Never format with `{:?}`. (b) **Event payloads**: keep `sanitize_diagnostic_report` byte-for-byte (`~/`). Keep `helper_log_path` raw; it is not under home.

**H-6. Panics never cross IPC and never carry payloads.**

- _Today_: registry methods `.expect("launch session registry poisoned")` (`registry.rs:20-26`). This is a deliberate policy; after a poison every later launch panics. `spawn_blocking` join errors are logged with `%e` (`streaming.rs:281-283,423-425`); that goes to logs only. `AppError::from(JoinError)` uses a fixed `"background task failed"` (`error.rs:68-77`).
- _Mitigation_: public service fns return `Result<_, AppError>`. Run blocking work (hooks, which block up to `DEFAULT_HOOK_TIMEOUT` = 10 s **each** inside the async command, `hooks.rs:12,209-226`; metadata; manifest parsing) through `services::run_blocking` (`services/mod.rs:31-39`), so a panic becomes a sanitized `Internal` and the executor never stalls. Event payloads never contain panic text. Keep the poison policy unless decided otherwise, but have the `SessionGuard` (H-1) avoid touching a poisoned registry in `Drop` (`lock()` → `if let Ok`) so an unwind does not double-panic and abort.

### MEDIUM

**M-1. One gamescope predicate, which must also decide PID capture.**

- _Today_: the game path computes the predicate twice (`execution.rs:109-116` capture, `182-184` watchdog). The trainer path computes it once (`execution.rs:293-302`) and **creates** a capture path, but the trainer builder never writes one: `build_proton_command_with_gamescope_in_directory` passes `pid_capture_path: None` (`script_runner/proton_trainer.rs:139-149`, `runtime_helpers/proton_command.rs:214-232`). So a Flatpak trainer watchdog waits 20 × 200 ms for a file that never appears (`resolver.rs:80-102`). It then falls back to the host exe-name ancestor heuristic, which picks its kill target by process name. The heuristic is unambiguity-gated, but it is weaker than a per-session capture.
- _Mitigation_: `gamescope_required(req, settings)` returns the effective config and feeds **both** the builder's capture decision and the watchdog spawn. The trainer then gets a session-owned capture file. Table tests should assert the capture path is `Some` exactly when the Flatpak + `proton_run` + gamescope predicate holds, for both kinds. If the core builder changes, call it out as a (security-positive) behaviour change in the parity notes.

**M-2. PID reuse windows stay as narrow as today.**

- _Today_: the local target is CrossHook's own unreaped child, so its PID cannot be reused until the stream task's `wait()` reaps it. After reaping, `is_target_pid_alive` (a `/proc/<pid>` check) fails within one `GAMESCOPE_POLL_INTERVAL` = 2 s and the watchdog stands down (`tasks.rs:88-93`). The descendant SIGKILL TOCTOU is documented and accepted (`tasks.rs:283-290`). Host targets are signalled via host `kill <pid>` (`tasks.rs:262-268`).
- _Risk_: if the service splits ownership (for example the spawner returns a PID and a separate task reaps), or delays the watchdog, the window between reap and signal grows.
- _Mitigation_: one task owns `Child` and reaps it. On `Exited`, cancel the watchdog through a `CancellationToken`/`select!` before the session is deregistered, so no signal is sent after reap. Never persist or emit PIDs.

**M-3. Event payloads stay minimal; `session_id` is the only addition.**

- _Today_:
  - `launch-log` is a plain string (`streaming.rs:93,126,134,183,204`; `bench.rs:292`). Raw helper output is **not** sanitized; that is current behaviour and comes only from the user's own processes. The gamescope `xdg_backend` suppression is kept (`shared.rs:29-64`).
  - `launch-diagnostic` is a sanitized `DiagnosticReport`.
  - `launch-complete` is `{code, signal}` (`streaming.rs:433-441`).
  - `injection-log` uses only fixed messages, plus a trimmed `profile_name` and a UUID `session_id` (`shared.rs:105-185`; the struct doc forbids raw helper output, env dumps or paths).
- _Mitigation_:
  - Send `launch-log` as `{ line, session_id }`. `normalizeLogMessage` already accepts `{line}` (`src/utils/log.ts:6,24-26`), so React works without changes. Do not add `log_path`, PIDs, env or argv to any payload.
  - With batching, `line` is up to `READ_CHUNK` (256 KiB) joined by `\n`. That is bounded, but document it.
  - `Exited { code, signal }` maps to the existing JSON. `session_id` is UUID v4 (`session/types.rs`) and unguessable, the same property as `JobId`.
  - Do not reuse `EventBus` (lossy, `Resync`) for `Exited`. Like `JobEvents`, the terminal event must never be lost to lag (`jobs/registry.rs:504-520`).

**M-4. CLI parity carries security-relevant differences.**

- _Today_:
  - The CLI writes logs to `$XDG_RUNTIME_DIR/crosshook-logs/<profile>.log` (`crosshook-cli/src/launch.rs:244-263`). That directory is per-user 0700 `[INFERENCE: XDG spec]`, safer than the shared `/tmp/crosshook-logs` used by `create_log_path`.
  - `--scripts-dir` helpers must be regular files owned by the caller (`validate_explicit_helper`, `launch.rs:213-242`, the C-1 mitigation).
  - The CLI runs no pre/post hooks, records no metadata and copies raw log bytes to stdout (`drain_log`, `launch.rs:300-319`).
- _Risk_: reusing the service moves CLI logs to `/tmp`, drops the helper ownership check if M1-29 resolution replaces it, starts executing profile hooks from the CLI, and changes stdout bytes (`LogTail` makes invalid UTF-8 U+FFFD and strips `\r`). The acceptance item "CLI output unchanged" catches the last one.
- _Mitigation_: the service takes a log-directory parameter, so the CLI keeps `XDG_RUNTIME_DIR`. Keep `validate_explicit_helper` (or the core `trusted_real_path` trust check, `runtime_helpers/mod.rs:186-197`) for explicit script directories. Make hooks and metadata an explicit request or option decision and record it in decisions. Pin the stdout bytes in fixtures (ASCII fixtures are unaffected).

### LOW / ADVISORY (pre-existing; do not widen; follow-ups)

- **A-1. Shared host `/tmp` log/PID files.**
  - What happens: `create_log_path` uses `create_dir_all("/tmp/crosshook-logs")` and then `File::create`, which follows symlinks and truncates (`jobs/process.rs:228-241`). `attach_log_stdio` opens in append mode and also follows symlinks (`runtime_helpers/path_resolution.rs:84-100`). The Flatpak capture script runs `mkdir -p` and `printf > "$1"` on the **host** `/tmp` (`runtime_helpers/mod.rs:12-13`), and the file is read back through host `cat` (`watchdog/host.rs:32-38`).
  - Exposure: another local user who pre-creates the directory could plant a symlink to clobber a file, or plant a PID file so the watchdog signals one of the victim's own processes. File names are guessable (`<slug>-<millis>`).
  - Fix: follow-up for a per-user 0700 directory and `create_new(true)` plus `O_NOFOLLOW`. Keep the location unchanged in this PR (storage boundary).
  - In this PR: switch to `create_new` and retry with a suffix on `AlreadyExists`. This closes the same-millisecond collision where two launches with the same slug share one log and one capture file, which could make one session's watchdog act on another session's PID.
- **A-2. `mangohud_conf_path(base, profile_name)` uses the profile name unvalidated** (`profile/mangohud.rs:72-74`, `script_runner/common.rs:98-105`). A `..` name from IPC can point `MANGOHUD_CONFIGFILE` at any existing file. It is read-only and the user's own; follow-up: `validate_name`.
- **A-3. `LaunchRequest.pre_launch_hooks`/`post_exit_hooks` arrive from IPC** (`request/models.rs:74-77`). They are absolute, executable, NUL-free, have a 10 s timeout and use null stdio (`hooks.rs:161-226`). They are no more powerful than `game_path`. Do not add a hook source that bypasses `run_single_hook`'s checks.
- **A-4. Steam launch-options builder** escapes values (`optimizations/steam_options.rs:17-35`) but emits custom-env **keys** unescaped after `trim` (`steam_options.rs:142-151`). The output is a string the user pastes into Steam, so this is low. Moving `build_steam_launch_options_command` should run `validate_custom_env_entry`-equivalent key checks, or leave the behaviour as is. It must not start passing this string to a shell.
- **A-5. `LowDiskSpaceAdvisory` help text includes the raw mount path** (`request/error_text.rs:352-355`, built in `warnings.rs:174-178`). That is current behaviour in `LaunchResult.warnings`; leave it.

---

## 2. Risks introduced by the refactor → mitigation map

| #   | Refactor step           | Risk                                                               | Control                     | Test                                                           |
| --- | ----------------------- | ------------------------------------------------------------------ | --------------------------- | -------------------------------------------------------------- |
| R1  | Generic teardown helper | Prefix sweep or `killpg` kills a sibling or the shared-prefix game | C-1                         | sibling-survives cascade `#[tokio::test]`                      |
| R2  | `ProcessSpawner` trait  | Gateway bypass via literals or vars                                | C-2, add CLI to `SCAN_DIRS` | `./scripts/check-host-gateway.sh` + `--self-test`              |
| R3  | Task restructuring      | Finalizer skipped on panic → leaked session, no cascade            | H-1 `SessionGuard`          | panicking fake stream → `Exited` + child cancelled             |
| R4  | Stream via M1-04        | Pipe stalls on non-UTF-8; unbounded line                           | H-2                         | `\xff` + 1 MiB line fixture                                    |
| R5  | Delete `safe_read_tail` | Finalizer reads the whole log / errors reach IPC                   | H-3 bounded `read_tail`     | > 2 MiB log → tail ≤ 2 MiB; missing file → `""`                |
| R6  | M1-01 request building  | `validate` skipped; `.env()` dropped under Flatpak                 | H-4                         | parity test on builder env/argv, flatpak `_inner` variant      |
| R7  | `AppError` adoption     | Path/Debug leaks, or `~/` → `$HOME` drift in payloads              | H-5 split                   | golden `launch-diagnostic` JSON unchanged                      |
| R8  | Async service fns       | Blocking hooks stall the executor; panics escape                   | H-6 `run_blocking`          | hook-timeout test off-executor                                 |
| R9  | Predicate unification   | Trainer capture path still unwritten → heuristic target            | M-1                         | predicate × capture table                                      |
| R10 | Child ownership split   | Larger PID-reuse window                                            | M-2                         | watchdog cancelled on `Exited`                                 |
| R11 | Event model             | Extra payload fields; terminal event lost to lag                   | M-3                         | payload snapshot; lagged receiver still gets `Exited`          |
| R12 | CLI reuse               | `/tmp` logs, helper trust lost, hooks run, stdout bytes change     | M-4                         | CLI fixture output + `--scripts-dir` non-owned helper rejected |

## 3. Invariants checklist for the plan

1. `validate(&request)` is the first statement of `start_game`/`start_trainer`, for every caller.
2. Only core builders construct launch `Command`s; the spawner receives built `Command`s.
3. A session signals only its own `ShutdownTarget`. Cascade only parent → linked trainers. Reset only by profile key. No `kill_processes_using_prefix`, `killpg` or `kill_on_drop` on launch children.
4. Exactly one finalize per session (guard): diagnostic → complete → injection finalization → cascade → post-exit hooks → deregister.
5. Log reads use lossy UTF-8, have per-read and partial-line caps, take ≤ 2 MiB for diagnostics, and keep I/O errors out of events.
6. Payloads: existing four event names and shapes, plus optional `session_id` only; `launch-log` is `{line, session_id}`; diagnostics stay sanitized with `~/`.
7. Command errors are `AppError` (sanitized), never `{:?}` Debug output; panics map to fixed `Internal`.
8. The gateway check covers `crosshook-app/src` (it already does) and `crosshook-cli/src` (add it).

## 4. Coverage

Reviewed: every file listed under Method. Not audited: `launch/request/from_profile.rs` (M1-01 request building), `umu_database` resolution, `platform/portals/{gamemode,background}.rs` beyond guard lifetime (`gamemode.rs:250-280`), and the Tauri 2 behaviour when an async command panics (`[INFERENCE]`: the invoke promise rejects or never settles; either way H-6 keeps panics off the IPC path). Checks the main agent should run after implementation: workspace `cargo test`, `./scripts/check-host-gateway.sh` and `--self-test`, and the frontend smoke for the `{line, session_id}` payload.
