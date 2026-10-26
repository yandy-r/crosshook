# Research — Launch business invariants (M1-05 / YAN-798 / #534)

Paths are relative to `src/crosshook-native/`. Line numbers are from the `crosshook-yan-798` worktree at the time of research.
Abbreviations: `exec.rs` = `src-tauri/src/commands/launch/execution.rs`, `stream.rs` = `src-tauri/src/commands/launch/streaming.rs`, `warn.rs` = `src-tauri/src/commands/launch/warnings.rs`, `core/` = `crates/crosshook-core/src/`.
Tags: **[OBS]** means observed in code. **[INFERENCE]** means not verified by running it. **[DIVERGENCE]** means current behaviour looks unintended; the refactor must either preserve it on purpose or fix it as a recorded decision.

---

## 0. Pipeline at a glance (today)

`launch_game` (`exec.rs:62`) and `launch_trainer` (`exec.rs:239`) run the same 13 steps. Steps marked † differ by kind.

| #   | Step                                                                       | Game                                                                 | Trainer                                                                 | Error behaviour                                                                                        |
| --- | -------------------------------------------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| 1   | Force flags                                                                | `launch_game_only=true, launch_trainer_only=false` (`exec.rs:70-71`) | inverse (`exec.rs:247-248`)                                             | —                                                                                                      |
| 2   | `validate(&request)`                                                       | `exec.rs:72`                                                         | `exec.rs:249`                                                           | `Err(error.to_string())`, no side effects                                                              |
| 3   | UMU enrich                                                                 | `enrich_umu_gameid_resolution` `exec.rs:47-59,75`                    | `exec.rs:252`                                                           | never fails. Settings load error → `AppSettingsData::default()`                                        |
| 4   | Warnings: low-disk → offline → trainer-hash → mod-coexistence              | `exec.rs:77-102`                                                     | `exec.rs:254-279`                                                       | never fail (fail-open). **Have DB side effects, see I-22/I-25**                                        |
| 5   | Method resolution †                                                        | `resolved_method()` (`exec.rs:103-107`)                              | plus `resolve_trainer_launch_method` (`exec.rs:285`)                    | —                                                                                                      |
| 6   | `create_log_path(kind, slug)`                                              | `"game"` `exec.rs:108`                                               | `"trainer"` `exec.rs:287`                                               | `Err(io string)`                                                                                       |
| 7   | Gamescope predicate + PID-capture path †                                   | `exec.rs:109-117`                                                    | `exec.rs:293-301`                                                       | —                                                                                                      |
| 8   | Build command †                                                            | `exec.rs:118-135`                                                    | `exec.rs:303-330`                                                       | `Err("failed to build … launch: {e}")`; the log file from step 6 is left on disk                       |
| 9   | GameMode portal register                                                   | `exec.rs:137` (effective=`method`)                                   | `exec.rs:332` (effective=`execution_method`)                            | never fails (returns `None`)                                                                           |
| 10  | Hook context + pre-launch hooks                                            | `exec.rs:139-144`                                                    | `exec.rs:334-339`                                                       | context error → `Err("failed to prepare launch hook context: …")`. Hook failures only produce warnings |
| 11  | Session register †                                                         | `register(Game, key)` `exec.rs:150`                                  | `register_and_link_to_parent_of_kind(Trainer, key, Game)` `exec.rs:347` | —                                                                                                      |
| 12  | `command.spawn()`                                                          | `exec.rs:152-158`                                                    | `exec.rs:378-396`                                                       | `deregister(session_id)` and then `Err(…)`                                                             |
| 13  | `record_launch_start` → stream task → cancel consumer → `Ok(LaunchResult)` | `exec.rs:162,210,219,230`                                            | `exec.rs:421,466,475,486`                                               | a history failure is logged and gives `operation_id=None`                                              |

The finalizer (`stream.rs:232-445` plus `finalize_launch_session` `stream.rs:513-542`) runs once per session, after the child exits.

---

## 1. Numbered invariant checklist

Each item is written so it can become a test assertion. "Service" means the new `crosshook-app/src/services/launch/`.

### A. Request normalization and validation

- **I-1** The game launch sets `launch_game_only=true` and `launch_trainer_only=false` before validation. The trainer launch sets the inverse (`exec.rs:70-71,247-248`). Downstream behaviour depends on these flags:
  - `effective_gamescope_config()`, which returns the trainer config when `launch_trainer_only` (`core/launch/request/models.rs:207-213`);
  - hook directives and working directory (`core/launch/hooks.rs:70-74,92-101`);
  - helper `--game-only`/`--trainer-only` args and unshare gating (`core/launch/script_runner/steam_helpers.rs:108-111`);
  - `force_no_umu_for_launch_request` (`core/launch/script_runner/umu.rs:48-52`).
- **I-2** `validate` runs **before** UMU enrichment in Tauri (`exec.rs:72→75`). The CLI runs it **after** enrichment (`crates/crosshook-cli/src/launch.rs:55-59`). Validation does not read `resolved_umu_game_id`, so the order is unobservable for valid input. Keep the CLI's dry-run output byte-identical.
- **I-3** `validate` (`core/launch/request/validation.rs:264`) rejects a trimmed `method` that is not one of `"" | steam_applaunch | proton_run | native` with `UnsupportedMethod`. It then dispatches on `resolved_method()`. A trainer request with method native fails with `NativeTrainerLaunchUnsupported` (`validation.rs` `validate_native`). The `METHOD_NATIVE => Err("native launch does not support trainer launch.")` arm at `exec.rs:328` cannot be reached, but keep it as a defensive arm.
- **I-4** Method resolution is `LaunchRequest::resolved_method` (`core/launch/request/models.rs:215-224`):
  1. an explicit trimmed method wins;
  2. otherwise a non-empty `steam.app_id` → `steam_applaunch`;
  3. otherwise `looks_like_windows_executable(game_path)` → `proton_run`;
  4. otherwise `native`.

  The CLI builds requests with the profile-side `resolve_launch_method` (`core/profile/models/resolve.rs:6-25`), which uses `profile.steam.enabled` instead of `app_id`. The service must keep that split: profile → request uses the profile resolver, request → runtime uses `resolved_method`.

- **I-5** The `match … _ => METHOD_NATIVE` fold to `&'static str` (`exec.rs:103-107,279-284`) is identity for every valid input, because `resolved_method` only returns the three constants.
- **I-6** Trainer execution method is `resolve_trainer_launch_method(resolved, is_flatpak)` (`core/profile/models/resolve.rs:32-39`):
  - `steam_applaunch` with Flatpak → `proton_run`;
  - `steam_applaunch` without Flatpak → `steam_applaunch`;
  - `proton_run` → `proton_run`;
  - anything else → `native`.

  This value (not the request method) is used for the gamescope predicate (`exec.rs:294`), portal registration (`exec.rs:332`), the history `launch_method` (`exec.rs:424`), the stream/diagnostic `method` (`exec.rs:470`) and the spawn error text (`exec.rs:393`).

- **I-7** The session profile key is `session_profile_key_for_name(profile_name)`: the trimmed name, or `ANONYMOUS_PROFILE_KEY` (`"__crosshook_anonymous_profile__"`) when it is `None` or blank (`core/launch/session/keys.rs:14-25`). Registration, `launch_reset_sessions` (`exec.rs:~508-514`) and `list_running_profiles` must all derive keys through this one helper.

### B. UMU GAMEID enrichment

`core/umu_database/resolver.rs:265-325`. The result is always stored as `request.resolved_umu_game_id = Some(..)`, both in Tauri (`exec.rs:54-58`) and in the CLI (`cli/launch.rs:55-58`).

- **I-8** Precedence is the first match below:
  1. A non-empty trimmed `runtime.umu_game_id` that passes `validate_umu_id` gives `ExplicitOverride` with `store=None`. If it fails validation, the result is `Fallback` with `game_id="umu-0"` and `error_category="invalid_explicit_umu_game_id"`. Either way this step ends resolution (`resolver.rs:55-75`).
  2. A non-empty `steam.app_id` (or `runtime.steam_app_id` when `steam.app_id` is empty) gives `SteamAppId` with `store="steam"` (`resolver.rs:77-93`).
  3. `resolved_method() != proton_run` gives `Fallback umu-0` with no key.
  4. `should_use_umu(request, force_no_umu_for_launch_request(request))` false gives `Fallback`. `force_no_umu_for_launch_request` = `launch_trainer_only && is_flatpak && method==steam_applaunch`; `should_use_umu` is false for `UmuPreference::Proton` or when `umu-run` is not resolvable (`umu.rs:48-95`).
  5. The key comes from the normalized `runtime.umu_store` and `runtime.umu_codename`. If the settings lookup is disabled the result is `LookupDisabled`, which carries the key if one parsed.
  6. No valid key gives `MissingHints`.
  7. A fresh cache hit is returned as-is. A cache read error gives a stale-cache row (`StaleCache`) or `ApiUnavailable umu-0` with `error_category="metadata_unavailable"`.
  8. Otherwise take the per-`store:codename` async lock (`IN_FLIGHT_LOOKUPS`), re-check the cache, then do a live lookup:
     - Found → cache row with a 7-day TTL (`CACHE_TTL_DAYS`), payload ≤256 KiB, result `FreshLookup`;
     - NotFound → cached "missing", result `CachedNotFound umu-0`;
     - HTTP error → stale cache or `ApiUnavailable "api_unavailable"`.
- **I-9** Env consumption, only when `use_umu`:
  - `GAMEID = resolved.game_id` if it is non-empty, otherwise `runtime.umu_game_id` → `steam.app_id` → `runtime.steam_app_id` → `"umu-0"` (`umu.rs:23-36`);
  - `STORE` is set only if the resolution has a non-blank store (`umu.rs:38-46`);
  - `PROTON_VERB` is `waitforexitandrun` for games and `runinprefix` for trainers (`proton_trainer.rs:93`).
- **I-10** Enrichment is not fatal. If loading settings fails, Tauri uses defaults (`exec.rs:52-54`) and the CLI fails with `CliError::General("settings load: …")` (`cli/launch.rs:40-42`). Live lookups write `umu_gameid_cache` rows even when the launch later fails.

### C. Warnings: all non-blocking, collected for both game and trainer launches

Order in `LaunchResult.warnings`:

1. low-disk
2. offline
3. trainer-hash (mismatch, then community)
4. mod-coexistence
5. pre-launch hook warnings (appended at step 10)

The collectors run identically for game and trainer launches. In particular, trainer-hash and offline warnings are also collected for **game** launches.

- **I-11 Low disk** (`warn.rs:145-185`, `core/storage/prefix_health/disk.rs:10-48`):
  - The prefix is `steam.compatdata_path` when `resolved_method()==steam_applaunch`, otherwise `runtime.prefix_path`. A blank prefix produces no warning.
  - The check resolves the wine prefix and walks ancestors to the first path that exists, then runs `statvfs`. It warns when `available_bytes < 2048 MiB` (`DEFAULT_LOW_DISK_WARNING_MB`, `core/storage/prefix_health/types.rs:4`).
  - Emits `ValidationError::LowDiskSpaceAdvisory{available_mb, threshold_mb, mount_path}`: code `low_disk_space_advisory`, severity Warning. MB values use integer division by 1 MiB.
  - Runs even with no profile name and with the metadata store disabled. Errors are traced and produce no warning.
- **I-12 Offline readiness** (`warn.rs:14-58`):
  - Every guard below is required, and any failure produces no warning: a non-blank profile name, `metadata_store.is_available()`, the profile loads, `effective_profile().trainer.path` is non-empty, and the `profile_id` exists.
  - Fires when `check_offline_readiness_for_profile(..).score < MIN_OFFLINE_READINESS_SCORE (60)` (`core/offline/readiness.rs:20`).
  - Emits `OfflineReadinessInsufficient{score, reasons=blocking_reasons}`, code `offline_readiness_insufficient`, Warning.
  - **Side effect:** `check_offline_preflight` upserts an `offline_readiness_snapshot` row (`readiness.rs:348`).
- **I-13 Trainer hash** (`warn.rs:61-87`, `core/launch/trainer_hash.rs:37-66`, `core/offline/hash.rs:134-…`):
  - Guards are the same as I-12 (name, available, load, profile_id), plus a non-empty effective trainer path that is a regular file.
  - The community digest is compared first. A mismatch on a non-blank, normalizable digest emits `trainer_hash_community_mismatch`. A `trainer_hash_cache` baseline mismatch (when stats match) emits `trainer_hash_mismatch`. Both are Warning, and the baseline mismatch is pushed first (`trainer_hash.rs:16-31`).
  - **Side effect:** on the first sighting of a `(profile_id, path)` pair, the baseline is **written** (`FirstBaselineRecorded`). A mismatch does not overwrite the stored baseline.
- **I-14 Mod coexistence** (`warn.rs:90-122`, `core/launch/mod_coexistence.rs:18-36`):
  - Guards run cheapest first: name, available, `lookup_profile_id`, then `list_enabled_profile_mods` non-empty, then the profile loads.
  - Uses the pure rules only; there is no filesystem stat (this differs from the panel's `analyze_profile_mod_coexistence`).
  - The context comes from the request: `trainer_configured` = `request.trainer_path` OR the effective trainer path; loading mode, `mangohud.enabled` and `gamescope.enabled` come from the **game** gamescope config even for trainer launches; the injection fields come from the profile.
  - Codes are prefixed `mod_coexistence_`.
- **I-15** `coexistence_records_from_warnings` (`warn.rs:126-143`) keeps only `mod_coexistence_*` codes, capped at `MAX_COEXISTENCE_ADVISORY_RECORDS=16` (`mod_coexistence.rs:11`). The records go into `DiagnosticReport.coexistence_advisories` at finalize (`stream.rs:267`).
- **I-16** `preview_launch` runs UMU enrichment, then `build_launch_preview`, then appends **only** the mod-coexistence warnings to `preview.validation.issues` (`queries.rs:16-40`). It does not add low-disk, offline or trainer-hash warnings.

### D. Command build per kind × method × Flatpak

- **I-17** Game:
  - `steam_applaunch` → `runtime_helpers::resolve(SteamLaunchHelper)` + `build_helper_command` (`steam_helpers.rs:61-148`). Not piped, so it uses **file-poll** streaming. Optimizations are resolved as the `proton_run` directive set. `unshare --net` applies only when `network_isolation && !launch_game_only`, so never for a game launch. The helper is passed the **trainer** gamescope args.
  - `proton_run` → `build_proton_game_command`, stdout/stderr **piped** (`exec.rs:125-131`).
  - `native` → `build_native_game_command`, which uses `attach_log_stdio` to the log file and is not piped, so file-poll (`script_runner/native.rs:8-30`).
- **I-18** Trainer:
  - `steam_applaunch` + Flatpak → `build_flatpak_steam_trainer_command`, piped (`proton_trainer.rs:22-37`). It clones the request with `method=proton_run`, `runtime.prefix_path=normalize(steam.compatdata_path)` and `runtime.proton_path=steam.proton_path`, and passes `force_no_umu=true`.
  - `steam_applaunch` without Flatpak → `SteamLaunchTrainer` script + `build_trainer_command`, file-poll.
  - `proton_run` → `build_proton_trainer_command` (UMU allowed), piped.
- **I-19 Parity (acceptance test).** The Flatpak Steam trainer and the `proton_run` trainer share one builder, `build_proton_trainer_command_with_umu_override`. So these match when the inputs map identically (prefix = compatdata, proton = steam.proton): env, `working_directory = resolve_effective_working_directory(runtime.working_directory, trainer_host_path)`, `resolved_trainer_gamescope()`, wrappers and loading-mode staging.
  - The only built-in difference is UMU, which is forced off for the Steam variant.
  - **The parity fixture must therefore use `UmuPreference::Proton`, or assert with `use_umu=false`.**
- **I-20 [DIVERGENCE]** The hook execution context for a Flatpak Steam trainer is built from `request.resolved_method()` (steam), not `proton_run`. That means Steam compat env plus _empty_ directives (`hooks.rs:41-46,92-101`). This differs from the trainer's actual runtime path. Preserve it unless it is fixed as a deliberate decision.

### E. GameMode portal

- **I-21** `should_register_gamemode_portal(request, effective_method)` is true only when all three hold (`core/launch/optimizations/gamemode.rs:34-56`):
  - `is_flatpak()`;
  - `effective_method == proton_run`;
  - `optimizations.enabled_option_ids` contains `"use_gamemode"`.

  For a game the effective method is `method`, so a Steam game is never registered. For a trainer it is `execution_method`, so a Flatpak Steam trainer **is** registered.
  - Registration happens **after the command build and before pre-launch hooks and spawn** (`exec.rs:137,332`).
  - `Ok(None)` (portal unreachable) and `Err` both give `None` plus a log line; neither is fatal (`portal.rs:12-37`).

- **I-22** The guard moves into the stream task. It is unregistered explicitly with `guard.unregister().await` **after** the whole finalizer (`stream.rs:46-51`), i.e. after the cascade, post-exit hooks and deregister.
  - On an early `Err` after registration (hook-context failure, spawn failure) the guard is just dropped. `Drop` does **not** call `UnregisterGame`; it logs at debug and relies on bus teardown (`core/platform/portals/gamemode.rs:273-287`).
  - **[INFERENCE]** Game and trainer sessions each register the same self PID. Whether the first `UnregisterGame` releases both depends on the portal's ref-counting.

### F. Pre/post hooks

- **I-23** `build_launch_hook_execution_context` (`hooks.rs:34-89`) uses method-specific env with custom env keys removed. The working directory comes from the trainer path for a trainer-only launch and the game path otherwise. A context error is **fatal** to the launch.
- **I-24** `run_pre_launch_hooks` (`hooks.rs:135-143`) runs only enabled hooks with stage `PreLaunch`, sequentially and **synchronously on the command task**. Each hook has `DEFAULT_HOOK_TIMEOUT=10s`, after which it is killed (`hooks.rs:23,~285-300`). stdio is null.
  - Each failure becomes one warning, and the launch continues. The failure codes are: empty path → `launch_hook_missing`; NUL or non-absolute → `launch_hook_invalid_path`; not executable on host → `launch_hook_not_executable`; spawn failure → `launch_hook_spawn_failed`; timeout; non-zero exit; wait failure → `launch_hook_failed`.
  - Pre-launch warnings are appended to `LaunchResult.warnings`.
- **I-25** `run_post_exit_hooks` runs in `finalize_launch_session` **after** the cascade and **before** `deregister` (`stream.rs:529-541`). Its warnings are only traced (`tracing::warn`) and never surfaced to the UI.
  - It runs on every child exit: success, failure, signal or watchdog kill.
  - It does **not** run when spawn failed.
  - It uses the hook context captured at launch time.

### G. Ordering and session registration

- **I-26** Register **before** spawn (`exec.rs:146-150,340-352`) so a concurrent `launch_reset_sessions` cannot miss a live process. On spawn failure, deregister before returning `Err`.
- **I-27** A trainer registers through `register_and_link_to_parent_of_kind(Trainer, key, Game)` (`core/launch/session/registry.rs:142-181`).
  - A **single lock acquisition** inserts the trainer and links it to the **most recently registered** `Game` with the same key.
  - If there is no such game, the trainer is registered unlinked (`None`) and only a debug trace is written.
  - Only Trainer→Game links are legal.
- **I-28** `record_launch_start` runs **after** a successful spawn (`exec.rs:162,421`). A spawn failure therefore writes no history row.
- **I-29** The stream task is spawned before the cancel consumer, and both before `Ok(LaunchResult)`. The returned `LaunchResult` is:
  - `succeeded: true`;
  - message `"Game launch started."` or `"Trainer launch started."`;
  - `helper_log_path`: the **raw** (unsanitized) log path;
  - `warnings`, omitted from JSON when empty.
- **I-30** Log path is `/tmp/crosshook-logs/<prefix>-<slug>-<epoch_ms>.log`. The file is created eagerly (`crates/crosshook-app/src/jobs/process.rs:228-243`), the prefix is `game` or `trainer`, and the slug is `request.log_target_slug()` (`models.rs:245-283`).
  - The CLI uses a **different** path, `$XDG_RUNTIME_DIR|/tmp` + `/crosshook-logs/<profile name with non-alnum→'-'>.log` (`cli/launch.rs:~255-275`). It keeps case and has no timestamp.
  - The gamescope PID capture file is `<log_path>.gamescope.pid` (`core/launch/script_runner/common.rs:48-50`). It is never deleted.

### H. Cancel channel, watchdog and background grant

- **I-31** Every session gets exactly one cancel consumer (`consume_cancel_channel`, `exec.rs:519-556`).
  - If gamescope is active and `child.id()` is `Some`, the consumer is `spawn_gamescope_watchdog`.
  - Otherwise it is `drain_cancel_into_outcome` (`core/launch/session/drain.rs:31-62`). The drain records the reason without `killed`; on `Closed` it records nothing; on `Lagged` it calls `cancel_reason_after_lag`.
- **I-32 [DIVERGENCE]** If gamescope is active but the exe basename is empty, `spawn_gamescope_watchdog` returns early and **drops `cancel_rx`** without draining (`exec.rs:600-606`). Cascades into that session are then lost, and `teardown_reason` falls back to `NaturalExit`.
- **I-33** The watchdog exe name is `Path::file_name` of the raw `game_path` for a game (`exec.rs:185-189`) or of `trainer_host_path` for a trainer.
- **I-34** On Flatpak (`background_supported()==is_flatpak()`, `core/platform/portals/background.rs:64`) the watchdog start is delayed by up to 500 ms. During that time it awaits `BackgroundGrantHolder::wait_for_initialization` and logs the grant state (`exec.rs:612-631`). This is diagnostic only and never blocks the launch result. In the service this needs a `BackgroundGrantProbe`-style seam, because `BackgroundGrantHolder` lives in `src-tauri` (`crate::BackgroundGrantHolder`).
- **I-35** Watchdog behaviour (`core/launch/watchdog/tasks.rs:31-136`, constants in `watchdog/mod.rs:15-22`):
  1. **Resolve the target.** Without a capture path the target is the direct child PID in the sandbox namespace. With one, the watchdog polls the capture file 20×200 ms, then falls back to a host ancestor search by exe name. An ambiguous result (more than one gamescope ancestor) means it stands down (`resolver.rs:74-150`).
  2. **Startup.** It polls up to 60×2 s for the exe to appear. If the exe never appears it stands down **without** marking the outcome.
  3. **Main loop.** When the exe disappears, it waits 5 s of grace and then calls `shutdown_gamescope_tree(WatchdogNaturalExit)`. Shutdown is SIGTERM, 3 s, SIGKILL, 500 ms, then kill any remaining descendants. All signals go through `host_std_command("kill")`.
  4. **Cancel.** It honours cancel in both loops. `Ok(r)` uses `r`; `Lagged` uses `try_recv` (falling back to `LinkedSessionExit`); `Closed` gives `ReceiverClosed`. Every cancel leads to an immediate tree shutdown that marks the outcome.
- **I-36** `WatchdogOutcome`: the first recorded reason wins, `mark` sets `killed=true`, and `record_reason` does not (`core/launch/session/types.rs:~165-205`).

### I. Streaming (`launch-log`)

- **I-37** The mode is chosen by `child.stdout.is_some() || child.stderr.is_some()` (`stream.rs:35`).
  - **Pipe mode** (`stream.rs:142-208`): stdout and stderr are read line by line on separate tasks into an mpsc channel. Each line is appended (with `\n`) to the log file, and emitted. If a log write fails, file writes stop but emitting continues.
  - **File-poll mode** (`stream.rs:61-139`): `read_to_string(log)` every **500 ms**, with a reset to 0 when the file shrinks. After exit there is one final read and emit. _This is the re-read that M1-04 tailing replaces._
- **I-38** The UI relay (`shared.rs:31-61`) handles gamescope `xdg_backend:` + "Compositor released us but we were not acquired" lines: the first is emitted verbatim, the second is replaced by `GAMESCOPE_XDG_BACKEND_SUPPRESSION_NOTICE`, and the rest are dropped. At stream end, if the suppressed count is > 0, one summary line is emitted (`stream.rs:131-135,204-207`). Empty lines are never emitted. The relay state is **per session**. The raw log file keeps every line.
- **I-39** The `launch-log` payload is a **plain JSON string**. React's `normalizeLogMessage` (`src/utils/log.ts:13-37`) also accepts `{line}`, `{message}` and `{text}`. So adding `session_id` means switching to `{ line, session_id }`; the existing consumers (`ConsoleView.tsx:65`, `ConsoleDrawer.tsx:148`) keep working. Tests at `layout/__tests__/ConsoleDrawer.test.tsx:55` and `AppShell.test.tsx:256` emit plain strings and need both shapes to be accepted.

### J. Finalizer: strict order (`stream.rs:232-445,513-542`)

- **I-40** Exit info: `code = ExitStatus::code`, `signal = ExitStatusExt::signal`. If the wait failed (pipe-mode join error or `try_wait` error), `exit_status=None` and the failure mode is `Unknown` (`core/launch/diagnostics/exit_codes.rs:7-20`).
- **I-41** The log tail is `safe_read_tail(log, MAX_LOG_TAIL_BYTES = 2 MiB)` (`diagnostics.rs:8-39`, `core/launch/diagnostics/models.rs:6`).
  - `diagnostic_method_for_log` rewrites `steam_applaunch` to `proton_run` when the tail contains both `[steam-trainer-runner]` and `trainer_launch_mode=` (`diagnostics.rs:~64-73`).
  - Then `analyze(exit_status, tail, diag_method)`, set `log_tail_path`, and `sanitize_diagnostic_report`, which applies `display_path_tilde` to summary, description, method, path, patterns and suggestions.
- **I-42** Exit classification (`exit_codes.rs:22-121`):
  - code 0 with `steam_applaunch` → **Indeterminate** (Info);
  - code 0 otherwise → CleanExit;
  - 127 → CommandNotFound; 126 → PermissionDenied;
  - any other code → NonZeroExit;
  - signals: 11 Segfault, 6 Abort, 9 Kill, 15 Terminated, 7 BusError, 4 IllegalInstruction, 8 FPE, 13 BrokenPipe, others UnknownSignal.
- **I-43** If `watchdog_outcome.was_killed()`, the report is rewritten before persisting (`stream.rs:248-259`):
  - `failure_mode=CleanExit`, severity Info;
  - summary and description set to `"Game exited; gamescope compositor cleaned up."` or `"Trainer exited; …"`;
  - suggestions cleared.
- **I-44** `teardown_reason = outcome.reason().or(Some(NaturalExit))`. Then `coexistence_advisories` is attached (`stream.rs:263-267`).
- **I-45 History row.** When `operation_id` is `Some`, `record_launch_finished(op, code, signal, &report)` runs on `spawn_blocking` (`stream.rs:269-284`, `core/metadata/launch_history.rs:56-121`). It writes:
  - `status = succeeded` if the failure mode is `CleanExit|Indeterminate`, otherwise `failed`;
  - `exit_code` and `signal` from the raw exit status (not rewritten by the watchdog override);
  - `diagnostic_json`, or NULL if it exceeds `MAX_DIAGNOSTIC_JSON_BYTES=4096` (`metadata/models.rs:227`);
  - `severity` and `failure_mode` as promoted columns;
  - `finished_at`.

  Zero rows affected is only a warning. Errors are logged and swallowed.

- **I-46 History start row** (`launch_history.rs:8-54`) is an INSERT with:
  - `status='started'`;
  - `profile_id` looked up by name, NULL if not found;
  - `profile_name` raw from the request;
  - `launch_method` = game method or trainer `execution_method`;
  - `log_path` = `display_path_tilde(log)`.

  A disabled store returns `Ok("")`, which gives `operation_id=None` (`core/metadata/store.rs:172-186`, `exec.rs:584-588`). Rows left in `started` are marked `abandoned` at the next startup (`crates/crosshook-app/src/bootstrap/reconcile.rs:41`).

- **I-47 Success gate.** Version snapshot and known-good tagging run only when the failure mode is `CleanExit|Indeterminate` (after the I-43 override) **and** `profile_name` is `Some`. This applies to **both game and trainer sessions** (`stream.rs:286-425`).
  - Failure or signal (not overridden): neither runs, and the history row is `failed`.
  - Watchdog kill: both run, and the history row is `succeeded`.
- **I-48 Version snapshot** (`stream.rs:290-385`):
  1. `lookup_profile_id`. `None` or an error skips the snapshot.
  2. If `steam.app_id.trim()` is non-empty, discover Steam roots and libraries from `steam_client_install_path` and parse the first `appmanifest_<id>.acf` that exists, giving `build_id` and `state_flags`.
  3. `trainer_file_hash = hash_trainer_file(trainer_host_path.trim())`, where an empty path gives `None`. The hash is computed for game launches too.
  4. Read the prior snapshot; an error there is treated as `None`.
  5. `compute_correlation_status(current_build or "", prior.build, hash, prior.hash, state_flags)` (`core/metadata/version_store.rs:182-207`):
     - `state_flags` is `Some(≠4)` → **UpdateInProgress**, and the snapshot is skipped;
     - no prior snapshot → Untracked;
     - otherwise BothChanged, GameUpdated, TrainerChanged or Matched.
  6. Otherwise `upsert_version_snapshot(profile_id, app_id.trim(), build or NULL, None, hash, None, status.as_str())`. This inserts a row and prunes to `MAX_VERSION_SNAPSHOTS_PER_PROFILE=20` in one savepoint (`version_store.rs:10-69`).

  The snapshot is written even for `proton_run` or `native` launches with an empty app id.

- **I-49 Known-good tagging** (`stream.rs:387-424`) is an independent block. It still runs when the snapshot was skipped (UpdateInProgress).
  1. `lookup_profile_id`. `None` or an error skips tagging.
  2. `list_config_revisions(profile_id, Some(1))`. An empty list skips tagging.
  3. `set_known_good_revision(profile_id, latest.id)`. In one savepoint this clears `is_last_known_working` for every revision of the profile and sets it on the latest one (`core/metadata/config_history_store/mod.rs:~253-295`).
- **I-50** `launch-diagnostic` is emitted with the sanitized `DiagnosticReport` iff `should_surface_report` (`core/launch/diagnostics/mod.rs:39-48`): a steam method with pattern matches, or a failure mode that is not CleanExit or Indeterminate.
- **I-51** `launch-complete` with payload `{"code": i32|null, "signal": i32|null}` is emitted **for every session, game and trainer** (`stream.rs:433-443`). React consumers: `useLaunchState.ts:282`, `useProfileHealth.ts:292`, `useRunningProfiles.ts:47`.
- **I-52** Injection finalization runs for trainers only (`stream.rs:447-506`):
  - First, if `teardown_reason != NaturalExit`: Warning/Runtime `"Trainer session teardown was recorded."`.
  - Then exactly one of:
    - natural exit and CleanExit → Info/Trainer `"Trainer launch completed."`;
    - Indeterminate → Warning/Runtime `"Trainer launch outcome is indeterminate."`;
    - anything else → Error/Trainer `"Trainer launch ended with diagnostics."`.
  - **Consequence:** a trainer that the watchdog cleaned up (CleanExit but `WatchdogNaturalExit`) emits the teardown warning **plus the Error message**. A cascaded trainer likewise emits warning plus Error/Indeterminate.
- **I-53** `finalize_launch_session` order:
  1. game only: `cancel_linked_children(id, LinkedSessionExit)`;
  2. post-exit hooks;
  3. `deregister(id)`;
  4. then (in `spawn_log_stream`) the portal unregister.

  The complete event order per session is: last `launch-log` lines and summary → history update → snapshot → known-good → `launch-diagnostic`? → `launch-complete` → trainer `injection-log` finalization → cascade → post-exit hooks → deregister → portal unregister.

### K. Injection-log lifecycle (trainer sessions only)

- **I-54** The payload is `InjectionLogEvent` (`shared.rs:114-161`):
  - `timestamp` (UTC RFC3339);
  - `profile_name`: trimmed, or `"Unknown profile"`;
  - `session_id` (UUID string);
  - `session_kind` (`game`/`trainer`);
  - `level` (`info`/`warning`/`error`);
  - `source` (`trainer`/`injection`/`runtime`);
  - `message`;
  - `hook_id` and `hook_name` are omitted when `None` (always, today);
  - `unsupported_runtime` is omitted when false.

  React filters on `profile_name`, `session_id` and `session_kind` (`components/library/trainer/InjectionLogTail.tsx:36-41`).

- **I-55** Launch-time sequence (`exec.rs:367-418`):
  1. after register, before spawn: Info/Trainer `"Trainer launch requested."`;
  2. on spawn failure: Error/Trainer `"Trainer launch failed before the trainer process started."`, then deregister, then `Err("failed to launch trainer (method={execution_method}): {e}")`;
  3. on success: Info/Trainer `"Trainer process started."`, then Warning/Injection `"DLL injection engine is not available; stored hook configuration was not applied."` with `unsupported_runtime=true`;
  4. at exit: I-52.

  Game sessions emit **no** `injection-log`.

### L. Cascade and isolation

- **I-56** Only a **Game** session's finalizer cascades. It sends `LinkedSessionExit` to every entry whose `parent == game_id`. The lock is released before the sends, and every send counts as delivered (`registry.rs:188-207`). A trainer's exit never signals its parent or siblings.
- **I-57** What the cascade does depends on the trainer's consumer:
  - With an active gamescope watchdog, the trainer's own tree is shut down (killed, reason `LinkedSessionExit`).
  - With only the drain, the trainer process **keeps running**. Only `LinkedSessionExit` is recorded, to be stamped into its diagnostic report when it exits naturally.
- **I-58 Isolation.** These sessions are unaffected by a game's exit:
  - sessions of other profiles;
  - trainers linked to a _different_ game of the same profile, because the link targets the newest game at trainer-registration time;
  - unlinked trainers;
  - other games.

  Each watchdog targets only its own `ShutdownTarget`: its own child PID, its own capture file, or a _unique_ gamescope ancestor of its own exe. The registry never touches PIDs (`registry.rs:1-6`).

- **I-59** A Steam game's finalizer runs when the **helper script exits**. That can be before the game exits; the exit code 0 Indeterminate case is the signal for this. A linked trainer therefore gets `LinkedSessionExit` at helper exit **[INFERENCE from I-42/I-56; depends on helper script lifetime]**.
- **I-60** `launch_reset_sessions(profile)` cancels every session with the key, of either kind, **once each** with `UserRequest`. It does not cascade (`registry.rs:222-238`) and returns the count. A game session's own cancel receiver only receives `UserRequest`.

### M. Read-only commands moving into the service

- **I-61** `list_launch_history_for_profile`:
  - a trimmed empty name gives `Err("profile_name is required")`;
  - an unavailable store gives `Ok([])`;
  - `limit` defaults to 20 and is capped at `MAX_HISTORY_LIST_LIMIT=50` (`metadata/models.rs:421`);
  - calls `query_launch_history_for_profile` (`queries.rs:~127-140`).
- **I-62** `validate_launch` is `validate(&request).map_err(issue)`. `build_steam_launch_options_command` is the core builder plus `escape_steam_token`-joined tokens from `resolve_command_arguments_for_method(…, steam_applaunch)`. Its error is formatted `{:?}` (`queries.rs:~44-75`).

### N. CLI `launch` (observable output to keep unchanged)

- **I-63** `cli/launch.rs:21-176`:
  - Game only. Uses the profile store from `--profile-dir`/`--config` and `resolve_steam_client_install_path`: the env var, then a `steam.sh` ancestor of compatdata, then the default roots.
  - UMU resolves with `MetadataStore::try_new()` or `disabled()`.
  - **Dry run**:
    - with `--json`, prints `build_launch_preview` as pretty JSON;
    - otherwise prints the `Method:`/`Game:`/`Trainer:`/`Command:`/`Environment:`/`Wrappers:`/`Steam opts:` lines, and `warning: {directives_error}` on stderr.
  - **Launch**:
    - **No** warnings, hooks, registry, portal, history, watchdog or finalizer side effects.
    - `proton_run` and native have stdout/stderr set to null. The Steam helper is spawned with null stdio, and `--scripts-dir` passes `validate_explicit_helper` (regular file, same UID).
    - `stream_helper_log` copies the log to stdout by byte offset every 500 ms, then does a final drain.
    - It then runs `safe_read_tail`, then `analyze(status, tail, request.method)` (no `diagnostic_method_for_log` rewrite).
    - If the report should surface, it prints the summary and `[severity] summary: suggestion` lines on stderr.
    - A non-success status gives `LaunchFailure("launch process exited with status {status}")`.
  - **[OBS]** With proton_run in the CLI, stdout and stderr are null and the builder does not attach a log, so the CLI log file stays empty.

---

## 2. Gamescope gating truth table (derived from current code)

**Inputs**

- **Method**: `resolved_method()`.
- **FP**: `platform::is_flatpak()`.
- **Config**: for a game, `request.gamescope`; for a trainer, `resolved_trainer_gamescope()` (`core/launch/mod.rs:78-95`). The trainer config is resolved as:
  - an enabled `trainer_gamescope` override is used as-is;
  - otherwise, if the game config is enabled, a clone of it with `fullscreen=false` and `borderless=false`;
  - otherwise the disabled default.
- **N**: `allow_nested || !is_inside_gamescope_session()`, where inside means `GAMESCOPE_WAYLAND_DISPLAY` is set (`models.rs:13-15`). `should_skip_gamescope` (`common.rs:52-54`) is exactly `!N`.

**Outputs**

- **Wrap**: CrossHook's builder wraps the child in gamescope.
- **WD**: the watchdog is spawned. This also requires `child.id().is_some()` and a non-empty exe basename (I-31/I-32).
- **Cap→WD**: a PID-capture path is handed to the watchdog.
- **Cap written**: the builder's command writes that file.

**Current predicate at both call sites:** `exec_method == proton_run && cfg.enabled && N`. The game version is at `exec.rs:109-112,182-184` and the trainer version at `exec.rs:294-296`. Capture = predicate && FP.

### Game

| Method          | FP  | cfg.enabled | N   | Wrap                                                                                                                                                                                                                    | WD       | Cap→WD | Cap written |
| --------------- | --- | ----------- | --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------ | ----------- |
| proton_run      | no  | yes         | yes | ✅ `proton_game.rs:39,131-140`                                                                                                                                                                                          | ✅       | —      | —           |
| proton_run      | no  | yes         | no  | ❌ (skip)                                                                                                                                                                                                               | ❌ drain | —      | —           |
| proton_run      | no  | no          | *   | ❌                                                                                                                                                                                                                      | ❌ drain | —      | —           |
| proton_run      | yes | yes         | yes | ✅ via pid-capture builder `proton_game.rs:121-130`                                                                                                                                                                     | ✅       | ✅     | ✅          |
| proton_run      | yes | yes         | no  | ❌                                                                                                                                                                                                                      | ❌ drain | ❌     | ❌          |
| proton_run      | yes | no          | *   | ❌                                                                                                                                                                                                                      | ❌ drain | ❌     | ❌          |
| steam_applaunch | *   | *           | *   | ❌ CrossHook does not wrap the Steam game (users put gamescope in Steam launch options). The helper receives the _trainer_ cfg as `--gamescope-*` when trainer cfg.enabled && N (`steam_helpers.rs` `helper_arguments`) | ❌ drain | ❌     | ❌          |
| native          | *   | *           | *   | ❌ (the native builder ignores gamescope)                                                                                                                                                                               | ❌ drain | ❌     | ❌          |

### Trainer (cfg = resolved trainer cfg: enabled ⇔ override.enabled ∨ game.enabled)

| Method          | FP  | exec_method     | cfg.enabled | N   | Wrap                                                                               | WD       | Cap→WD | Cap written         |
| --------------- | --- | --------------- | ----------- | --- | ---------------------------------------------------------------------------------- | -------- | ------ | ------------------- |
| proton_run      | no  | proton_run      | yes         | yes | ✅ `proton_trainer.rs:73-74,139-148`                                               | ✅       | —      | —                   |
| proton_run      | no  | proton_run      | yes         | no  | ❌                                                                                 | ❌ drain | —      | —                   |
| proton_run      | no  | proton_run      | no          | *   | ❌                                                                                 | ❌ drain | —      | —                   |
| proton_run      | yes | proton_run      | yes         | yes | ✅                                                                                 | ✅       | ✅     | **❌ [DIVERGENCE]** |
| proton_run      | yes | proton_run      | yes         | no  | ❌                                                                                 | ❌ drain | ❌     | ❌                  |
| proton_run      | yes | proton_run      | no          | *   | ❌                                                                                 | ❌ drain | ❌     | ❌                  |
| steam_applaunch | no  | steam_applaunch | yes         | yes | script-side (`--gamescope-enabled`/`--gamescope-arg` to `steam-launch-trainer.sh`) | ❌ drain | ❌     | ❌                  |
| steam_applaunch | no  | steam_applaunch | yes         | no  | ❌ (args omitted)                                                                  | ❌ drain | ❌     | ❌                  |
| steam_applaunch | no  | steam_applaunch | no          | *   | ❌                                                                                 | ❌ drain | ❌     | ❌                  |
| steam_applaunch | yes | **proton_run**  | yes         | yes | ✅ (direct Proton builder)                                                         | ✅       | ✅     | **❌ [DIVERGENCE]** |
| steam_applaunch | yes | proton_run      | yes         | no  | ❌                                                                                 | ❌ drain | ❌     | ❌                  |
| steam_applaunch | yes | proton_run      | no          | *   | ❌                                                                                 | ❌ drain | ❌     | ❌                  |
| native          | *   | native          | *           | *   | rejected by `validate` (`NativeTrainerLaunchUnsupported`)                          | —        | —      | —                   |

**[DIVERGENCE] on the trainer capture file.** Despite the comment at `exec.rs:289-292` ("the pre-spawn PID capture file must exist"), the trainer builder calls `build_proton_command_with_gamescope_in_directory`, which passes `pid_capture_path=None` (`proton_trainer.rs:139-148`, `runtime_helpers/proton_command.rs:213-232`). It never writes `<log>.gamescope.pid`. The Flatpak trainer watchdog therefore spends 4 s polling (`GAMESCOPE_HOST_PID_CAPTURE_WAIT_*`) and then resolves through the host ancestor search, or stands down if the result is ambiguous. If the single `gamescope_required` predicate also drives builder capture, this changes behaviour. Record that as an explicit decision.

**A single predicate that reproduces every row above:**

```text
exec_method(kind, method, fp) = match kind { Game => method, Trainer => resolve_trainer_launch_method(method, fp) }
cfg(kind, req)               = match kind { Game => req.gamescope, Trainer => req.resolved_trainer_gamescope() }
gamescope_required           = exec_method == proton_run && cfg.enabled && (cfg.allow_nested || !inside_gamescope)
pid_capture_for_watchdog     = gamescope_required && fp
spawn_watchdog               = gamescope_required && child_pid.is_some() && !exe_name.is_empty()
```

For table tests, inject `fp` and `inside_gamescope` as parameters. Today both are read from the process env (`platform::is_flatpak`, `GAMESCOPE_WAYLAND_DISPLAY`), so tests would otherwise need env guards.

---

## 3. Error matrix (what is left behind at each failure point)

| Failure at                    | Return                                                         | Session                   | History                              | Log file            | Portal                         | Events                             |
| ----------------------------- | -------------------------------------------------------------- | ------------------------- | ------------------------------------ | ------------------- | ------------------------------ | ---------------------------------- |
| validate                      | `Err(msg)`                                                     | none                      | none                                 | none                | none                           | none                               |
| create_log_path               | `Err(io)`                                                      | none                      | none                                 | maybe dir           | none                           | none                               |
| helper resolve / build        | `Err("failed to build …")` or a `runtime_helpers` error string | none                      | none                                 | **empty file left** | none                           | none                               |
| hook context                  | `Err("failed to prepare launch hook context: …")`              | none                      | none                                 | left                | **dropped without unregister** | none                               |
| spawn (game)                  | `Err("failed to launch helper: {e}")`                          | registered → deregistered | none                                 | left                | dropped                        | none                               |
| spawn (trainer)               | `Err("failed to launch trainer (method=…): {e}")`              | registered → deregistered | none                                 | left                | dropped                        | injection: requested + failed      |
| record_launch_started         | launch proceeds                                                | live                      | none (`op=None`)                     | written             | live                           | normal                             |
| child exits non-zero / signal | —                                                              | finalized                 | `failed`                             | full                | unregistered                   | diag? + complete (+ trainer Error) |
| child exits 0 / watchdog kill | —                                                              | finalized                 | `succeeded`                          | full                | unregistered                   | complete; snapshot + known-good    |
| app crash mid-session         | —                                                              | lost (runtime-only)       | `started` → `abandoned` at next boot | full                | bus teardown                   | —                                  |

Even an early failure keeps the side effects from steps 3–4: UMU cache rows, the trainer-hash first baseline, and the offline readiness snapshot.

---

## 4. Test-seam notes for the service

- **`ProcessSpawner`** must cover `command.spawn()` and expose:
  - `id()`;
  - whether stdout and stderr are piped;
  - `wait`/`try_wait`.

  The finalizer needs only `Option<ExitStatus>`, and `ExitStatus::from_raw` gives you code and signal fixtures.

- **Finalizer tests:** use `MetadataStore::open_in_memory`-style fixtures and assert I-45..I-49:
  - success writes the row as `succeeded` and writes the snapshot (status `untracked` on the first run) and the known-good marker;
  - failure (code 1) writes `failed` with no snapshot or tag;
  - signal 9 writes `failed` with `signal=9`;
  - watchdog-killed writes `succeeded` and the snapshot and tag, with the summary rewritten;
  - `UpdateInProgress` skips the snapshot but still tags.
- **Cascade tests:** register G1 (profile A), T1 linked to G1, G2 (profile B), T2 linked to G2 and T0 (profile A, unlinked before G1). Finalize G1, then:
  - T1's outcome is `LinkedSessionExit`;
  - T2 and T0 have no reason recorded;
  - G2 is still registered.
- **Existing tests to preserve or move:**
  - `src-tauri/src/commands/launch/tests/{diagnostics,log_relay,coexistence}.rs`;
  - `core/launch/session/{registry_tests.rs,drain.rs}`;
  - `core/launch/optimizations/gamemode.rs` tests.
