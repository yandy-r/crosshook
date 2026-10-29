# Security research: install / update / run-executable / prefix-deps → `crosshook-app` services

Scope: YAN-815 (M1-31) + YAN-1538, read-only review of current flows. Paths are relative to
`src/crosshook-native/` unless they start with `scripts/`/`docs/`. Unobserved claims: `[INFERENCE]`.
`./scripts/check-host-gateway.sh` exits 0 on this worktree today (run 2026-10-29).

## 1. Findings (prioritized)

### HIGH

**H1. Under Flatpak, cancel/stop sends host `kill` to a sandbox-namespace PID.**

- `src-tauri/src/commands/update.rs:50-52,76-79` and `run_executable.rs:130-135,186,214-217` store
  `child.id()` and later run `host_std_command("kill") [-KILL] <pid>`.
- Under Flatpak the child is the `flatpak-spawn` proxy (`platform/gateway.rs:125-134`), so the PID
  is in the sandbox namespace, while `host_std_command` (`gateway.rs:333-346`) runs `kill` on the
  **host**, where the number names an unrelated process or nothing (see
  `docs/research/flatpak-bundling/01-historical.md:235`, `docs/prps/reviews/pr-268-review.md:19`).
  The real Proton tree is never signalled, and another same-user process may get SIGTERM/SIGKILL.
- **Fix in scope.** Signal only via `jobs::supervise_child` (`crosshook-app/src/jobs/process.rs:84-153`),
  i.e. local `nix::killpg` on the proxy group; host teardown then relies on `--watch-bus` (H2).
  Delete every `host_std_command("kill") <child pid>` in these flows.

**H2. The proxy for Proton-based flows has no `--watch-bus`, so cancel/stop/app exit can leave host processes running.**

- install, update and run-exe build via `build_direct_proton_command_with_wrappers_in_directory`
  (`launch/runtime_helpers/proton_command.rs:85-126`), which calls the **non-lifecycle**
  `host_command_with_env_and_directory` (`:100,112`). Callers: `install/service.rs:110-129`,
  `update/service.rs:29-50`, `run_executable/service/command_builder.rs:31-50`. `supervise_child`
  requires a lifecycle builder (`jobs/process.rs:67-76`; `crosshook-app/README.md:394-400`);
  without it, SIGKILL on the proxy group leaves the host proton/Wine tree running.
- **Fix in scope.** Add a lifecycle variant of the direct-Proton builder over
  `host_command_with_env_and_directory_lifecycle` (`gateway.rs:110-123`, `--watch-bus` right after
  `--host` at `:208-212`). Spawn with `.process_group(0)` + `.kill_on_drop(true)` like
  `services/community/runner.rs:66-78`. Add argv tests like `platform/tests/gateway.rs:95-139`.
- **Decision for Main:** with `--watch-bus`, quitting CrossHook kills a running installer, updater
  or ad-hoc executable on the host, which today survives app exit. This matches registry
  semantics (cancel on shutdown); the PR should state it.

**H3. Concurrent ad-hoc runs of executables with the same stem share one throwaway prefix. The first run to exit deletes it and SIGKILLs the other run.**

- The ad-hoc prefix is `_run-adhoc/<slugify(stem)>` (`run_executable/service/adhoc_prefix.rs:101-112,131-149`).
  There is no running-instance check; the single slot is overwritten (`run_executable.rs:130-135`).
- When the first run exits, its callback clears the slot (`:155-157`) and starts cleanup
  (`:159-165`). Cleanup calls `kill_processes_using_prefix` (`:260`), which SIGKILLs every
  same-user process whose env references the prefix (killing run #2), then removes the directory
  (`:255-270`). Run #1 also becomes uncancellable once overwritten.
- **Fix in scope.** Hold an exclusive per-prefix `JobKey` on the **resolved** prefix for the whole
  job; a second run gets `Conflict`. Run cleanup inside the job before `Finished`, so the key is
  held until deletion completes.

**H4. Prefix-deps stdout/stderr readers stop at the first invalid UTF-8. The pipe is then never drained, the install can hang, and the prefix lock is held forever.**

- Both readers are `while let Ok(Some(line)) = lines.next_line().await` (`src-tauri/src/commands/prefix_deps.rs:405-444`).
  `next_line` returns `Err` on invalid UTF-8, which ends the loop and leaves the pipe undrained.
  Wine/winetricks output in a localized locale can contain such bytes. Once the pipe buffer
  (64 KiB) fills, the child blocks on write, `wait_and_restore` never finishes, and the lock guard
  (`prefix_deps.rs:449-451`) is never dropped. The same prefix cannot be repaired or installed
  again until restart.
- `lines()` also buffers without a limit until `\n`. wget-style progress output uses `\r`, so one
  "line" can grow very large.
- **Fix in scope.** Drain pipes as lossy, bounded byte chunks. Reuse the tee/drain pattern in
  `services/launch/streaming.rs:57-95` (bounded by `PARTIAL_CAP`). Also follow the rule for
  descendants that outlive the leader: drains must be bounded and cancel-aware after the leader
  exits (as in the community runner, `services/community/runner.rs:118`).

**H5 (YAN-1538). `check_installed` spawns a host probe without `--watch-bus`, and its timeout kills only the proxy.**

- `runner.rs:504` uses `build_prefix_dep_command`, which maps to `host_command_with_env_and_directory`
  (`runner.rs:28-37`). On timeout (`runner.rs:523-534`), the `Child` is dropped with
  `kill_on_drop` (`runner.rs:514`). Under Flatpak only the proxy dies; the host `winetricks/protontricks list-installed`
  keeps running against the prefix.
- **Fix in scope.** Use `host_command_with_env_and_directory_lifecycle` for the probe as well:
  either fold `build_prefix_dep_command` into `build_prefix_dep_mutation_command`
  (`runner.rs:41-50`) or rename it, so that no non-lifecycle builder is left in `prefix_deps`. Add an
  argv test for the Flatpak form asserting `flatpak-spawn --host --watch-bus --clear-env …`.
  Natively, also spawn with `process_group(0)` and supervise it, so that Wine children of
  winetricks die on timeout. `kill_on_drop` only kills the direct child.

### MEDIUM

**M1. Cancel/stop has no ownership check.**

- `cancel_run_executable` / `stop_run_executable` / `cancel_update` take no id. They act on
  whatever the global slot holds (`run_executable.rs:180-225`, `update.rs:72-87`). Today the only
  window is `main` (`src-tauri/capabilities/default.json:5`), so the exposure is latent.
- **Fix in scope.** Return a `JobId` handle and cancel by id **plus** an owner derived from the
  injected `tauri::Webview` label, never from a JS argument. Mirror ProtonUp:
  `src-tauri/src/commands/protonup.rs:21-34,118-126` and
  `crates/crosshook-app/src/services/protonup/cancel.rs:18-22`. Unknown or foreign ids map to
  `NotFound` / `false`. JobIds are unguessable (`crates/crosshook-app/src/jobs.rs:5-6`), but log
  and progress events are broadcast to every webview, so ownership is the real gate.

**M2. The prefix-wide SIGKILL sweep is too broad natively and does the wrong thing under Flatpak.**

- `kill_processes_using_prefix` (`jobs/process.rs:335-399`) SIGKILLs every readable `/proc/*`
  process whose environment mentions the prefix at a path boundary.
- `stop_run_executable` runs it for **user-chosen** prefixes too (`run_executable.rs:219`). That
  can kill an unrelated game or a user shell that uses the same prefix (for example a Steam
  compatdata shared with a running game).
- Under Flatpak, `/proc` shows only sandbox PIDs, yet the kill goes through host `kill`
  (`process.rs:388`). This is the same cross-namespace PID problem as H1. In practice the sweep
  probably matches nothing, because the proxy receives env through `--env=` argv rather than its
  environment [INFERENCE].
- **Fix in scope:**
  1. Run the sweep only for throwaway prefixes (`is_throwaway_prefix_path`), or only after the
     job's own group is gone.
  2. Signal sandbox-visible PIDs with local `nix::sys::signal::kill`, never host `kill`.
  3. Skip the sweep under Flatpak and rely on `--watch-bus` plus group kill.
- **Follow-up:** a host-side sweep under Flatpak, for example `wineserver -k` through the gateway
  with `WINEPREFIX` set.

**M3. Log file is reopened by path; `/tmp/crosshook-logs` is a shared, world-writable location.**

- `create_log_path_in` creates the directory with `create_dir_all` (`process.rs:249`) and then the
  file with `create_new` (`process.rs:264-267`). That correctly refuses a pre-planted final symlink.
  However, it **drops** the handle (`process.rs:269`).
- `attach_log_stdio` then reopens the path twice with `create(true).append(true)`
  (`launch/runtime_helpers/path_resolution.rs:84-100`), which follows symlinks. If another local
  account created `/tmp/crosshook-logs` first, it owns the directory and can swap the file for a
  symlink between creation and reopen, redirecting child output into any file the user can write.
  Files get the default umask (typically `0644`), so other users can read paths and env-derived
  output.
- The 500 ms tailer also reopens by path (`src-tauri/src/commands/log_stream.rs:57,129`).
- Whether Flatpak gives the app a private `/tmp` [INFERENCE: yes] does not matter for the native
  build.
- **Fix in scope (cheap):** keep the `File` returned by `create_new`, open it with mode `0600`, and
  hand `try_clone()` handles to the child's stdout/stderr instead of reopening the path. This
  preserves the "logs stay in /tmp/crosshook-logs" contract.
- **Follow-up (shared with `LaunchService`):** create the directory as `0700`, verify with `lstat`
  that it is a real directory owned by the uid and not a symlink, and open with `O_NOFOLLOW`
  (precedent: `metadata/config_history_hmac.rs:392`).

**M4. A replayed journal row runs `binary_path` without re-validating it.**

- `restore_plan_from_row` re-validates `tool_type`, `restore_verb` and the Steam App ID
  (`src-tauri/src/commands/prefix_deps.rs:98-125`; verb allow-list `runner.rs:209,342`). It copies
  `binary_path` unchanged (`prefix_deps.rs:119`), and that value is executed during repair
  (`runner.rs:346-359`). The table is in the same-user metadata DB
  (`metadata/prefix_version_restore_store.rs:11-50`), so impact is limited to local tampering.
- **Fix in scope:** inside the new `PrefixVersionJournal` decode, require either a bare
  `winetricks` or `protontricks` name or an absolute path that passes
  `normalized_path_is_executable_file_on_host`. Startup recovery runs **without** user action, so
  every row must be validated before it executes. A row that fails validation is marked failed and
  not run.

**M5. Prefix-deps uses one global lock, which covers neither repair/startup recovery fully nor the other prefix mutators.**

- `PrefixDepsInstallLock` holds a single `Option<String>` (`prefix_deps/lock.rs:5-47`), so any two
  installs conflict regardless of prefix. Install and repair share it
  (`prefix_deps.rs:336-340,527-531`).
- `check_prefix_dependencies` takes no lock (`prefix_deps.rs:222-257`).
- install-game, update and run-executable can all target the same prefix while a winetricks
  install is running.
- **Fix in scope:** add one shared `JobKey` namespace on the **canonical** prefix, for example
  `prefix:<canonical>`. `normalize_prefix_path` already canonicalizes (`runner.rs:68-89`); keep
  using it for the key. Prefix-deps install, repair and startup recovery must take it.
  run-executable must take it on the resolved prefix (H3). Use it for install-game and update too
  if Main agrees.
- **Decision:** whether `check` should take the key (and get `Conflict`) or read cached state.

### LOW

- **L1. Paths beginning with `-` can be parsed as options.** `flatpak-spawn` parses options before
  the program, which comes from user settings (`gateway.rs:220,238`; `detection.rs:34-54`). Native
  validators accept relative paths (`run_executable/service/validation.rs:19-54`,
  `update/service.rs:88-123`); Flatpak host `test` already requires absolute
  (`platform/host_fs.rs:215-218`). **In scope:** require absolute executable/installer/updater/
  Proton/override paths; PATH-found `winetricks`/`protontricks` stay bare names.
- **L2. The native build inherits CrossHook's full environment.** It only calls `.envs()`, never
  `env_clear()` (`gateway.rs:241-252`), so e.g. `LD_PRELOAD` reaches Proton/winetricks. Flatpak
  uses `--clear-env` + allow-list (`gateway.rs:212-218`). **Follow-up** (affects all launch paths).
- **L3. Stale deletion doc.** It still describes a pinned `/usr/bin/rm -rf` fallback
  (`run_executable.rs:239-241`), but the code only retries `remove_dir_all` (`:267-280`). Fix the
  doc; do not reintroduce a shell `rm`.
- **L4. Run-exe log lines are emitted raw** (prefix-deps sanitizes, `runner.rs:453-493`). The UI
  must render them as text; there is no `dangerouslySetInnerHTML` in `src/` today — keep it so.
- **L5. PID reuse.** Cancel can signal a stored PID after the child is reaped, before the 500 ms
  poll clears the slot (`log_stream.rs:104-126` vs `update.rs:72-79`). `supervise_child` signals
  only while the child is unreaped (`process.rs:96-104`). Never signal a stored numeric PID.
- **L6. Lexical limits of the gateway lint.** It flags only literal denylisted
  `Command::new("…")` and skips `*/tests/*` (`scripts/check-host-gateway.sh:7-10,81-87`), so
  `Command::new(binary_path)` passes. New services must use `crosshook_core::platform::*lifecycle`
  builders; reviewers should grep `Command::new(` in the new service dirs.

## 2. Throwaway-prefix deletion safety (current)

- **What gets deleted:** only a prefix that was **defaulted** (empty `request.prefix_path`,
  `run_executable.rs:120,141-145`) and that also passes `is_throwaway_prefix_path`
  (`adhoc_prefix.rs:25-67`). That check requires the parent to equal
  `<data_local>/crosshook/prefixes/_run-adhoc` (or the Flatpak `host_prefix_root()/_run-adhoc`),
  a non-empty file name, and no `..` component. An explicit user prefix is never deleted, even if
  it points into `_run-adhoc`.
- **Symlinks:** `std::fs::remove_dir_all` removes a top-level symlink without following it, and
  does not follow symlinks inside the tree (Rust ≥ 1.58.1, CVE-2022-21658). The check is lexical,
  so a symlinked ancestor such as `prefixes/` is followed, but the only thing deleted is the
  `<slug>` entry CrossHook created.
- **Must keep:** the guard, the "only when defaulted" rule, `remove_dir_all` with no shell
  fallback, and the bounded retries (`run_executable.rs:255-280`). Retries need the job's processes
  gone, so cleanup runs after `supervise_child` returns.
- **Under Flatpak**, wineserver can linger on the host past the ~500 ms retry budget
  [INFERENCE]. The cleanup should report failure (the warning today at `run_executable.rs:276-279`)
  instead of looping. Follow-up: a host-side `wineserver -k` through the gateway.

## 3. argv / injection review (current)

| Flow                             | argv                                                                                                                                     | Guard (keep)                                                                                                                                                                                                                                                                       |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| prefix-deps install              | `<bin> [appid] -q <verb>...` (`runner.rs:577-597`)                                                                                       | `validate_protontricks_verbs` runs first (`runner.rs:575`): non-empty, at most 50 verbs, no leading `-`, `^[a-z0-9][a-z0-9_-]{0,63}$` (`validation.rs:9-22,57-82`). Each verb is a separate `.arg()`. winetricks has no `--`, so validation is the boundary (`runner.rs:592-594`). |
| Steam App ID                     | positional before `-q`                                                                                                                   | `parse_nonzero_steam_app_id`: ASCII digits only, nonzero `u32`, re-rendered canonically (`prefix_deps.rs:178-191`). Profile fallback order: `steam.app_id` → `runtime.steam_app_id` (`:193-201`).                                                                                  |
| protontricks→winetricks fallback | swaps binary + tool                                                                                                                      | Only when the tool is protontricks **and** there is no valid App ID (`prefix_deps.rs:237-247,319-329`). Uses `resolve_winetricks_path()` (bare name after a host `command -v`, `detection.rs:10-12,97-99`). The duplicated block must become one service function and be tested.   |
| restore / repair                 | `<bin> [appid] -q <winver>` (`runner.rs:351-359`)                                                                                        | `is_windows_version_verb` allow-list (`runner.rs:209,342`). Journal row re-validated (`prefix_deps.rs:98-125`); see M4 for `binary_path`.                                                                                                                                          |
| list-installed                   | `<bin> [appid] list-installed` (`runner.rs:505-513`)                                                                                     | No user strings except the App ID.                                                                                                                                                                                                                                                 |
| run-exe / install / update       | `proton run <path>` or `proton run msiexec /i <path> /qb` (`command_builder.rs:40-50`; `install/service.rs:129`; `update/service.rs:50`) | Path must be a file with `.exe`/`.msi` extension (`run_executable/service/validation.rs:19-37,73-79`); single `.arg`. See L1 for the absolute-path requirement.                                                                                                                    |
| Flatpak wrapper                  | `flatpak-spawn --host [--watch-bus] --clear-env [--directory=] --env=K=V… <prog>` (`gateway.rs:207-220`)                                 | Env passed as argv (`--env=`). `custom_env_vars` is empty in all four flows, so the bash env-file path (`gateway.rs:221-239`) is never reached. Keep it that way.                                                                                                                  |

SQL: every journal and dep-state query is parameterized (`metadata/prefix_version_restore_store.rs:11-104`).
Env (§4): prefix-deps = `host_environment_map()` (HOME/USER/LOGNAME/SHELL/PATH/DISPLAY/WAYLAND_DISPLAY/
XDG_RUNTIME_DIR/DBUS_SESSION_BUS_ADDRESS) + `WINEPREFIX` (`runner.rs:52-66`); Proton flows add
`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, optional `STEAM_COMPAT_CLIENT_INSTALL_PATH`
(`environment.rs:121-150`). No user env reaches these flows; keep that and `--clear-env`.

## 5. Flatpak lifecycle matrix (every host spawn in scope)

| Spawn                      | File:line today                                   | `--watch-bus` | Local group/supervise                                                                  | Required in service                                     |
| -------------------------- | ------------------------------------------------- | ------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| prefix-deps install        | `runner.rs:577` (mutation builder)                | yes           | `kill_on_drop` only, no `process_group`                                                | lifecycle ✔ + `process_group(0)` + `supervise_child`    |
| prefix-deps restore        | `runner.rs:346-347`                               | yes           | `kill_on_drop`; on timeout `child.kill()` hits the direct child only (`runner.rs:389`) | + `process_group(0)` + supervise with timeout as cancel |
| check_installed (YAN-1538) | `runner.rs:504`                                   | **no**        | `kill_on_drop`                                                                         | lifecycle + group + supervise (H5)                      |
| run-executable             | `command_builder.rs:31` → `proton_command.rs:100` | **no**        | none (PID slot)                                                                        | lifecycle + group + supervise (H1/H2)                   |
| install-game               | `install/service.rs:110-129`                      | **no**        | none; `block_on(child.wait())` is not cancellable (`install/service.rs:61-75`)         | lifecycle + group + supervise                           |
| update-game                | `update/service.rs:29-50`                         | **no**        | none (PID slot)                                                                        | lifecycle + group + supervise                           |
| cancel/stop `kill`         | `update.rs:76`, `run_executable.rs:186,214`       | n/a           | host `kill` of a sandbox PID (H1)                                                      | **delete**                                              |
| prefix sweep `kill`        | `jobs/process.rs:388`                             | n/a           | host `kill` of PIDs read from sandbox `/proc` (M2)                                     | local `nix::kill`; skip under Flatpak                   |
| binary detection           | `detection.rs:10-12` (`host_command_exists`)      | no            | short-lived                                                                            | out of scope (not cancellable)                          |

[INFERENCE] `flatpak-spawn` forwards SIGTERM to the host process, so SIGTERM to the proxy group
gives a polite cancel. SIGKILL cannot be forwarded, so after a SIGKILL only `--watch-bus` cleans up
the host side.

## 6. Must-preserve list

1. Verb validation, the 50-verb cap, one `.arg()` per verb, App-ID parsing, and the
   restore-verb allow-list (`validation.rs`, `prefix_deps.rs:178-191`, `runner.rs:209`).
2. The journal row is written **before** spawn; a spawn failure leaves it `pending`
   (`prefix_deps.rs:367-389`).
3. The prefix lock is held through install, restore, journal persistence and the completion event
   (`prefix_deps.rs:334-340,449-451`). Repair reloads the row after taking the lock
   (`prefix_deps.rs:533-541`).
4. Journal decision table: `Succeeded`→delete; `Failed`→mark failed; `NotRequired`/`Pending`→no
   mutation (`prefix_deps.rs:127-140`). A failed delete after a successful restore becomes `Failed`
   (`prefix_deps.rs:163-176,554-567`). An invalid restore state is marked failed
   (`prefix_deps.rs:582-591`).
5. Prefix canonicalization (`runner.rs:68-82`) is used for the lock key and the journal key, and the
   restore result is verified by re-reading `system.reg` (`runner.rs:410-422`).
6. The throwaway guard `is_throwaway_prefix_path`, deletion only when the prefix was defaulted,
   `remove_dir_all` only (no `rm`), and bounded retries.
7. `create_new` plus the `-N` suffix for log files, the log directory `/tmp/crosshook-logs`, and
   the log-dir seam `create_log_path_in` for tests.
8. Every host process goes through `crosshook_core::platform` builders, with `--clear-env` and an
   allow-listed environment under Flatpak, and no `custom_env_vars`.
9. Prefix-deps UI output is sanitized (`sanitize_output_for_ui`, `runner.rs:491`). The frontend
   renders logs as text only.
10. Structured `RunCommandError` with `field` placement (`run_executable.rs:50-93`). Keep the IPC
    error shape.

## 7. Must-fix in scope vs follow-up

**In scope (YAN-815 / YAN-1538):**

- H1: remove host-`kill`-by-PID; cancel = SIGTERM via `supervise_child`; stop = immediate group
  SIGKILL.
- H2: lifecycle-bound direct-Proton builder, plus `process_group(0)`, `kill_on_drop` and
  supervision for run-exe, install and update.
- H3/M5: per-prefix exclusive `JobKey` on the canonical/resolved prefix for prefix-deps install,
  repair and startup recovery, and for run-executable (throwaway included). Cleanup runs inside the
  job.
- H4: bounded, lossy, cancel-aware pipe drains for prefix-deps.
- H5 (YAN-1538): `check_installed` uses `host_command_with_env_and_directory_lifecycle`, with an
  argv test.
- M1: cancel by `JobId` plus a webview-label owner, mirroring ProtonUp.
- M2: restrict the prefix sweep (throwaway only or after group exit), signal locally, skip under
  Flatpak.
- M3 (cheap part): reuse the `create_new` handle at mode `0600` for the child's stdio; tail the
  same file.
- M4: `PrefixVersionJournal` decode validates `binary_path`. Invalid rows are marked failed, never
  executed, including at startup.
- L1: require absolute paths for user-supplied executables and binaries. L3: fix the stale doc.

**Follow-up (separate issues):**

- M3: `/tmp/crosshook-logs` ownership/`lstat`/`0700`/`O_NOFOLLOW` hardening, shared with
  `LaunchService` and `export/diagnostics/logs.rs:14`.
- M2: a host-side Wine teardown under Flatpak (`wineserver -k` with `WINEPREFIX` through the
  gateway).
- L2: `env_clear()` plus an allow-list for native host commands across all launch paths.
- L6: a Rust-aware gateway check covering `Command::new(<variable>)`.
- Decide whether `check_prefix_dependencies` takes the prefix key.

## 8. Tests the security fixes need

- Flatpak argv of every in-scope builder has `--host --watch-bus` (`*_inner(…, flatpak=true)` seams).
- Cancel/drop of a supervised fake winetricks/Proton kills the group incl. a grandchild (extend `runner.rs:927-974`).
- 2nd job on the same canonical prefix → `Conflict` (incl. two ad-hoc runs with one exe stem; install vs repair).
- Non-UTF-8 / newline-free output flood does not stall the child; the job finishes.
- Foreign-owner or unknown-id cancel → `NotFound`; the job keeps running.
- Journal row with bad `binary_path`, unknown tool, non-allow-listed verb, or App ID `0`/`abc` → marked failed, never spawned (startup + pre-install).
- Break each guard once to confirm its test fails; fakes must not ignore the arguments under test.
