# Research: `crosshook-cli launch` vs the launch service (YAN-798 / M1-05)

Read-only investigation. Paths are relative to `src/crosshook-native/`. Line numbers are from the `crosshook-yan-798` worktree.

## Exploration: CLI launch path

### Entry Points

- `crates/crosshook-cli/src/main.rs:14-20`: `#[tokio::main] async fn main()` calls `run()`. On `Err(CliError)` it runs `eprintln!("{error}")` and then `std::process::exit(error.exit_code())`.
- `main.rs:22-38`, `run()`: `Command::Launch(command) => launch::launch_profile(command, &cli.global)`.
- `crates/crosshook-cli/src/args.rs:63-79`: `LaunchCommand { profile, profile_dir (hidden), scripts_dir, dry_run }`.
- Global options are at `args.rs:22-39`: `--profile/-p`, `--verbose` (`launch` never reads it), `--json`, `--config`.

### Execution Flow (`crates/crosshook-cli/src/launch.rs:21-177`, `launch_profile`)

1. **Profile name** (`:25-28`): `command.profile` or `global.profile`. If both are missing, the error is `"a profile name is required via --profile"` (`General`, exit 1).
2. **Profile store** (`:29-34`): `store::profile_store(profile_dir or global.config)`, defined at `store.rs:8-21`.
   - `Some(path)` → `ProfileStore::with_base_path`.
   - Otherwise → `SettingsStore::try_new` + `load` + `try_new_with_settings_data`.
   - Errors are `settings store: …`, `settings load: …` or `failed to initialize profile store: …`, all exit 1.
   - `--config` is passed in as the _profiles dir_, even though its help text calls it a settings file. This quirk already exists today.
3. **Load** (`:35-37`): failure returns `ProfileNotFound(format!("profile \"{profile_name}\" not found: {e}"))`, exit 3. `e` is `ProfileStoreError::NotFound(path)`, which displays as `profile file not found: <absolute path>` (`profile/toml_store/error.rs:25`). The path is not sanitized.
4. **Settings** (`:38-42`): `SettingsStore::try_new()` + `load()`, exit 1 on error. A missing file falls back to defaults (`settings/store.rs:82-83`).
5. **Steam client path** (`:43-44`, `:179-212`): a CLI-local `resolve_steam_client_install_path(compatdata)`. It tries, in order:
   - the `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var;
   - the first compatdata ancestor that contains `steam.sh`;
   - `$HOME/{.steam/root,.local/share/Steam,.var/app/com.valvesoftware.Steam/data/Steam}`.

   This is a different algorithm from core `runtime_helpers::resolve_steam_client_install_path` (`launch/runtime_helpers/path_resolution.rs:102`).

6. **Request** (`:45-52`): `LaunchRequest::from_profile(&profile, resolve_launch_method(&profile), steam_path, &profile_name, settings.umu_preference)`. This is the M1-01 builder (`launch/request/from_profile.rs:41`).
   - `LaunchRequestError` maps to `LaunchFailure`, exit 4.
   - Error texts: `profile has no game executable path`, `unsupported launch method "x"`.
   - **There is no `launch_request_from_profile` in the CLI any more.** The old builder (`main.rs:712-767`, cited in `docs/prps/plans/umu-migration-phase-3…`) was removed by M1-01 (`docs/research/native-ui-migration/README.md:163`).
7. **`request.launch_game_only = true`** (`:53`). The CLI never launches a trainer separately. For `steam_applaunch`, the helper gets `--game-only` (`launch/script_runner/steam_helpers.rs:300-302`) and exits before starting the trainer (`runtime-helpers/steam-launch-helper.sh:355-358`).
8. **UMU GAMEID** (`:54-58`): `MetadataStore::try_new().unwrap_or_else(disabled)`, then `umu_database::resolve_umu_game_id(&request, settings.umu_database_lookup, &store)`. This is a third copy of the Tauri enrichment, alongside `src-tauri/src/commands/launch/execution.rs:47-58` and `queries.rs:35-41`.
9. **Validate** (`:59`): `launch::validate(&request)`. Errors become `LaunchFailure(ValidationError::message())`, exit 4 (`launch/request/validation.rs:264`).
10. **Dry-run** (`:61-98`): see the output contract below. Returns `Ok(())`, exit 0.
11. **Log path** (`:100`, `:244-264`): `$XDG_RUNTIME_DIR` (or `/tmp`) + `/crosshook-logs/<name>.log`, with every non-ASCII-alphanumeric character replaced by `-`.
    - The path is **stable per profile and never truncated**.
    - Tauri instead uses a fresh file per launch: `crosshook_app::jobs::create_log_path` (`crates/crosshook-app/src/jobs/process.rs:228`) writes `/tmp/crosshook-logs/<prefix>-<slug>-<millis>.log`.
    - The diagnostic bundle only collects `/tmp/crosshook-logs` (`crosshook-core/src/export/diagnostics/logs.rs:14`), so CLI logs never reach bundles.
12. **Log dir** (`:103-107`): `create_dir_all`. On failure: `failed to create log directory: {e}`, exit 4.
13. **Spawn** (`:109-148`), `match request.method`:
    - **`steam_applaunch`** (`:110-128`):
      - With `--scripts-dir`, the helper is `dir.join(steam-launch-helper.sh)` and must pass `validate_explicit_helper` (`:216-242`, the C-1 check: regular file owned by the current uid).
      - Without it, the helper comes from `runtime_helpers::resolve(SteamLaunchHelper)`. Failure gives `"{e}; pass --scripts-dir <dir containing steam-launch-helper.sh>"`.
      - `spawn_helper` (`:266-276`) calls `script_runner::build_helper_command` and sets stdout and stderr to **`Stdio::null()`**.
      - The helper writes its own log via `exec >>"$log_file" 2>&1` (`steam-launch-helper.sh:217-218`).
    - **`proton_run`** (`:129-135`): `build_proton_game_command(&request, &log_path)` with stdout and stderr set to `Stdio::null()`.
      - **`build_proton_game_command` does not attach the log file** (`script_runner/proton_game.rs:34-161`). It only uses `log_path` to build the gamescope PID capture path.
      - So **CLI `proton_run` writes nothing to the log**: stdout stays empty, and the diagnostics tail is empty or stale.
      - Tauri pipes stdout/stderr and copies them into the log itself (`src-tauri/.../streaming.rs:146-229`).
    - **`native`** (`:136-142`): `build_native_game_command` calls `attach_log_stdio` (append mode, `runtime_helpers/path_resolution.rs:84-100`), and the CLI then sets `Stdio::null()`.
      - [INFERENCE] The later `.stdout(Stdio::null())` replaces the file stdio, so native output would not reach the log either, and native launch stdout is empty.
      - Confirm this when capturing the baseline (golden cases N1/N2).
    - **`other`** → `unsupported launch method: {other}`. Unreachable after `validate`.
    - The child is spawned directly. There is no process group, session registry, watchdog, GameMode portal, pre/post hooks, warnings, or `record_launch_started`.
14. **Stream** (`:149-151`; `stream_helper_log` at `:278-296` + `drain_log` at `:298-319`): a `child.try_wait()` loop.
    - While the child runs: `drain_log` opens the file, seeks to `offset`, `read_to_end`s, writes the **raw bytes** to `tokio::io::stdout()`, and advances `offset`. Then flush and sleep 500 ms.
    - On exit: one final drain, then return the `ExitStatus`.
    - A missing file is silently skipped.
    - `offset` starts at **0**, so the whole existing file is replayed (bug B1).
15. **Tail** (`:153-157`; `safe_read_tail` at `:321-346`):
    - Open the file. If it is longer than `MAX_LOG_TAIL_BYTES` (2 MiB, `launch/diagnostics/models.rs:6`), seek to `End(-max)`.
    - Then `read_to_end` + `from_utf8_lossy`. Any error → `""`.
    - This is a byte-identical twin of `src-tauri/src/commands/launch/diagnostics.rs:8-41`, minus `tracing::warn!`.
16. **Diagnose** (`:158-169`): `launch::analyze(Some(status), &tail, &request.method)`.
    - The CLI passes the raw method. Tauri uses `diagnostic_method_for_log` (`src-tauri/.../diagnostics.rs:66-75`), then sanitizes, applies watchdog overrides and records history.
    - If `should_surface_report(&report)` (`launch/diagnostics/mod.rs:39-48`), the CLI prints the summary and one line per pattern to stderr.
17. **Exit** (`:170-176`): if `!status.success()` → `LaunchFailure(format!("launch process exited with status {status}"))`, exit 4. Otherwise `Ok(())`.

### Exit codes (`crates/crosshook-cli/src/cli_error.rs`)

| Code | Source                                                           | Launch triggers                                                                                                      |
| ---- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| 0    | `Ok(())`                                                         | Dry-run; launch with `ExitStatus::success()`                                                                         |
| 1    | `General` (`From<&str/String/Box<dyn Error>/serde_json::Error>`) | Missing `--profile`; settings or profile-store init; JSON serialization                                              |
| 2    | clap usage error (`EXIT_USAGE_ERROR` itself is unused)           | Bad flags                                                                                                            |
| 3    | `ProfileNotFound`                                                | Any `store.load` error, not only "not found"                                                                         |
| 4    | `LaunchFailure`                                                  | `from_profile`, `validate`, preview build, log dir, helper resolve/C-1, build/spawn, stream I/O, non-zero child exit |
| 5    | `EXIT_STEAM_NOT_FOUND`                                           | Reserved, unused                                                                                                     |

`Display` prints only the message (`cli_error.rs:21-29`). `main` writes it to stderr plus `\n`.

## Output contract (what must stay byte-identical)

### Dry-run, text mode (`launch.rs:66-95`)

stdout, in this order. Optional lines are omitted, never printed blank:

```
Method:     {request.method}
Game:       {request.game_path}                  # raw, untrimmed
Trainer:    {request.trainer_path}               # only if !trainer_path.is_empty()
Command:    {preview.effective_command}          # only if Some
Environment:                                     # only if Some(env) && !env.is_empty()
  {key}={value}                                  # per PreviewEnvVar, preview order
Wrappers:   {wrappers.join(" → ")}               # U+2192 with spaces; only if Some && non-empty
Steam opts: {preview.steam_launch_options}       # only if Some
```

stderr: `warning: {preview.directives_error}` when it is `Some`. The value may contain embedded `\n` (`append_preview_error`, `launch/preview/builder.rs:255-266`). Exit code is 0.

### Dry-run, `--json` (`launch.rs:62-65`)

Output is `serde_json::to_string_pretty(&LaunchPreview)` + `\n`.

These fields depend on things other than the profile:

- `generated_at`, also embedded in `display_text` (`preview/builder.rs:182`, `preview/display.rs:19`).
- Host env vars: `HOME USER LOGNAME SHELL PATH DISPLAY WAYLAND_DISPLAY XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS` (`preview/environment.rs:16-38`).
- `GAMESCOPE_WAYLAND_DISPLAY` (`request/models.rs:13`).
- Whether `umu-run` is on `PATH`.
- Whether a umu CSV exists under `/usr/share/umu-protonfixes`, for `umu_decision.csv_coverage` (`umu_database/coverage.rs:51`, `paths.rs:9-22`).
- `unshare --net` availability, and `FLATPAK_ID` / `/.flatpak-info`.

The CLI dry-run does **not** append mod-coexistence issues to `validation.issues`. Tauri `preview_launch` does (`src-tauri/.../queries.rs:42-50`). Routing the CLI dry-run through a service `preview_launch` that adds them would change the JSON for profiles that have mods.

### Launch (`launch.rs:149-174`)

**stdout**: the exact bytes appended to the log file, starting at offset 0, written verbatim.

- Not split into lines and not decoded: CR and invalid UTF-8 are kept.
- A final partial line is printed without a newline.
- No prefix is added.

**stderr**, only when `should_surface_report` is true:

```
{report.summary}
[{fatal|warning|info}] {pattern.summary}: {pattern.suggestion}    # per pattern_match, in order
```

- `summary` is `exit_info.description`. It is static text from `exit_codes.rs`, for example `Process exited with code N.`, `Steam launch helper exited cleanly; Steam may still be running the game.`, `Process terminated by SIGX.`
- It may be followed by ` Matched 1 known issue: {s}.` or ` Matched {n} known issues; the highest priority match is {s}.` (`diagnostics/mod.rs:152-169`).
- Pattern texts are static (`diagnostics/patterns.rs`). These lines contain no paths, so `sanitize_diagnostic_report` would not change them.

**On failure**, stderr also gets: `launch process exited with status {std::process::ExitStatus}`, then exit 4. Std's unix `Display` renders this as `exit status: N`, `signal: N (SIGNAME)` or `signal: N (SIGNAME) (core dumped)`.

### Byte-identity hazards when moving to `crosshook_app::log_tail`

`LogTail::poll` and `follow` (`crates/crosshook-app/src/log_tail.rs:118`, `:332`) deliver `LogBatch::Lines(Vec<String>)`. If the CLI prints `line + "\n"`, the output is identical **only for valid UTF-8, LF-terminated output with lines ≤ 64 KiB**.

| Old CLI (`drain_log`)                       | `LogTail` + `println!`                                                                     |
| ------------------------------------------- | ------------------------------------------------------------------------------------------ |
| CRLF kept                                   | Trailing `\r` stripped (`decode_line`)                                                     |
| Invalid UTF-8 bytes kept                    | Replaced with U+FFFD                                                                       |
| Unterminated last line printed without `\n` | `flush_partial` on drain, then `\n` added                                                  |
| Any line length                             | Lines longer than `PARTIAL_CAP` (64 KiB) are split (`buffer_partial`)                      |
| After truncation: bytes from the old offset | After truncation: restart at 0, partial line dropped                                       |
| Never lossy                                 | A lagging broadcast subscriber gets `LogBatch::Dropped { n }` (`jobs/registry.rs:504-517`) |

**Service design requirement:** `LaunchEvent::Log { session_id, batch }` must carry the **raw** lines, including empty ones. These UI-only transforms must stay in the Tauri adapter:

- the `!line.is_empty()` filter (`streaming.rs:89`, `:128`);
- `transform_launch_log_line_for_ui` (gamescope xdg_backend suppression);
- `suppression_summary_line` (`src-tauri/.../shared.rs:23-63`).

If they move into the service, CLI output loses blank lines and gains `[crosshook] Suppressed …` lines.

**Exit status:** `LaunchEvent::Exited { code, signal }` drops the core-dump bit. To keep the `launch process exited with status …` line identical, the CLI needs either the `ExitStatus` itself (it is `Copy + Send`) or a `core_dumped` field in the event. Rebuilding it with `ExitStatusExt::from_raw` only works if the raw wait status is kept.

### Existing bugs (do not freeze them into goldens)

- **B1. Replay and stale diagnostics.** The log path is stable, and the helper and `attach_log_stdio` both append. Each launch therefore re-prints every earlier run, and `safe_read_tail` analyses up to 2 MiB of old output. For `steam_applaunch`, an old `Steam is not running` line keeps surfacing, because any pattern match triggers surfacing (`diagnostics/mod.rs:40-42`).
- **B2. `proton_run` is blind.** There is no log capture (step 13). stdout is empty, and diagnostics see only the exit code.
- **B3. Gamescope hang.** `proton_run` with gamescope makes gamescope the child. Gamescope does not exit when the game does (`execution.rs` `spawn_gamescope_watchdog` docs), and the CLI has no watchdog, so `try_wait` never returns.
- **B4. `--config` semantics.** The help text says settings file; the code uses it as the profiles dir (`args.rs:36-38`, `launch.rs:29-34`).

These belong to YAN-837 or need an explicitly approved delta. Do not silently fix them under the "unchanged" criterion.

## Existing CLI tests and fixtures

- The only tests are clap parser tests in `crates/crosshook-cli/src/args.rs:162-319`, e.g. `parses_launch_dry_run_flag` at `:247`.
- There are **no** launch or dry-run output tests, no `tests/` directory, no fixtures, and no dev-dependencies (`crates/crosshook-cli/Cargo.toml`).
- **CI never runs CLI tests.** `.github/workflows/lint.yml:52` runs `cargo test -p crosshook-app -p crosshook-native`, and `release.yml:108` runs `-p crosshook-core`. Clippy `--all-targets` (`lint.yml:49`) does cover the CLI.
- A new CLI golden must also add `-p crosshook-cli` to `lint.yml:52`, or it never gates merges.

Prior art to reuse:

- `crates/crosshook-core/tests/fixtures/launch_request/*.json`: 17 profile→request goldens, driven by `crosshook-core/tests/launch_request_golden.rs`.
- `crates/crosshook-app/tests/profile_writer_golden.rs`: the frozen-golden convention ("a mismatch is a regression, never a reason to regenerate").

## Golden test design for "CLI dry-run and launch output unchanged for fixture profiles"

**Shape.** `crates/crosshook-cli/tests/launch_golden.rs`, a normal libtest integration test. It drives the real binary through `env!("CARGO_BIN_EXE_crosshook")` (Cargo provides this to integration tests of a bin crate) using `std::process::Command`.

- The test process never mutates its own environment. Each case runs the child with `env_clear()`, so unlike `profile_writer_golden` there is no need for `harness = false`.
- Only new dev-dependency: `tempfile = "3"`, which is already in `Cargo.lock`.

**Order.** Land this test **first**, against the unmodified CLI. Capture the expected files from that binary and record the base commit SHA in the test's doc comment. Then refactor. The files are frozen from then on, as in `profile_writer_golden`.

**Isolation per case.** Each case gets a tempdir, referred to as `@ROOT@`. Child environment:

- `HOME=@ROOT@/home`, `XDG_CONFIG_HOME=@ROOT@/home/.config`, `XDG_DATA_HOME=@ROOT@/home/.local/share`, `XDG_RUNTIME_DIR=@ROOT@/run`
- `PATH=@ROOT@/bin:/usr/bin:/bin`, `USER=LOGNAME=tester`, `SHELL=/bin/bash`
- `DISPLAY=:99`, `WAYLAND_DISPLAY=wayland-test`, `DBUS_SESSION_BUS_ADDRESS=unix:path=@ROOT@/bus`
- No `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `GAMESCOPE_WAYLAND_DISPLAY` or `FLATPAK_ID`.

Further setup:

- Skip the whole suite if `/.flatpak-info` exists.
- Pass `--profile-dir @ROOT@/profiles`.
- Settings resolve to the empty XDG config, so defaults apply: `umu_database_lookup = Disabled` (`settings/types.rs:356`).
- `MetadataStore::try_new` creates a fresh database under `XDG_DATA_HOME`.

**Fixtures.** Each case has:

- `tests/fixtures/launch/<case>.toml`: the profile, with `@ROOT@` placeholders;
- `<case>.stdout`, `<case>.stderr` and `<case>.code`, compared after substituting `@ROOT@`.

The harness materializes the executables the profiles point at:

- a stub proton (`#!/bin/sh`);
- a native `game.sh`;
- `steam/steam.sh` and `steam/steamapps/compatdata/<appid>/pfx`;
- a stub `steam-launch-helper.sh` that parses `--log-file` and appends to it.

Pin `runtime.umu_preference = "proton"` in `proton_run` fixtures, so the `Command:` line does not depend on a host `umu-run`. Keep gamescope, mangohud, hooks and mods off.

| Case | Mode                                                                                             | Asserts                                                                                                                                                                                                                                                                                                       |
| ---- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1   | native dry-run, text                                                                             | Method/Game/Command/Environment lines present; no Wrappers or Steam opts lines; exit 0                                                                                                                                                                                                                        |
| D2   | `proton_run` dry-run, text, with custom env and one dependency-free optimization                 | Environment order; Command line                                                                                                                                                                                                                                                                               |
| D3   | `steam_applaunch` dry-run, text                                                                  | `Trainer:` and `Steam opts:` lines; steam client path resolved via `steam.sh` in a compatdata ancestor                                                                                                                                                                                                        |
| D4   | `steam_applaunch` dry-run, `--json`                                                              | Compare as `serde_json::Value` after masking `generated_at` and the `generated_at = "…"` line in `display_text`; `umu_decision` is `None` for steam                                                                                                                                                           |
| E1   | missing profile                                                                                  | stderr `profile "missing" not found: profile file not found: @ROOT@/profiles/missing.toml`; exit 3                                                                                                                                                                                                            |
| E2   | `proton_run`, prefix missing                                                                     | stderr `The runtime prefix path does not exist.`; exit 4                                                                                                                                                                                                                                                      |
| E3   | no `--profile`                                                                                   | stderr `a profile name is required via --profile`; exit 1                                                                                                                                                                                                                                                     |
| S1   | `steam_applaunch` launch; stub helper writes `[steam-helper] Steam is not running\n` and exits 0 | stdout is exactly that line. stderr is `Steam launch helper exited cleanly; Steam may still be running the game. Matched 1 known issue: Steam was not available for the launch helper.` then `[fatal] Steam was not available for the launch helper.: Start Steam and sign in before launching again.` Exit 0 |
| S2   | `steam_applaunch`; stub helper exits 3                                                           | stderr ends with `launch process exited with status exit status: 3`; exit 4                                                                                                                                                                                                                                   |
| S3   | `--scripts-dir` whose helper is not a regular file                                               | C-1 message; exit 4                                                                                                                                                                                                                                                                                           |
| N1   | native launch; `game.sh` exits 0                                                                 | Baseline stdout (expected empty per step 13 [INFERENCE]); stderr empty; exit 0                                                                                                                                                                                                                                |
| N2   | native launch; prints `Permission denied\n`, exits 3                                             | Freeze whatever the baseline shows for stdout and tail. stderr is the summary, plus `[fatal] The executable could not be started…` if the tail sees the line. Exit 4                                                                                                                                          |
| P1   | `proton_run` launch; stub proton exits 0                                                         | stdout empty (B2); exit 0                                                                                                                                                                                                                                                                                     |

**Scope guard (cheap, recommended).** After every launch case, open `@ROOT@/home/.local/share/crosshook/metadata.db` and assert that `launch_operations` has 0 rows. This proves YAN-798 did not silently take on YAN-837's history recording.

**Not covered by goldens:**

- non-UTF-8 output, CRLF, an unterminated last line, and lines over 64 KiB (see the hazards table);
- replay on a second run (B1).

Each is either a deliberate delta, which needs approval and a record in `decisions.md`, or gets its own test once fixed.

## Overlap with YAN-837 (M1-09: CLI reuses services via `App::bootstrap`, records history)

The Linear text for YAN-837 was not readable from this agent: the MCP scope guard blocked `linear get_issue`. The scope used here is the summary given in the assignment.

What full `start_game(req)` would add to the CLI beyond streaming:

- **Warnings**: offline, trainer-hash, low-disk and mod (`src-tauri/.../warnings.rs:14`, `:61`, `:90`, `:145`). Printing them is new output; ignoring them is dead work.
- **Pre-launch and post-exit hooks** (`execution.rs:141`, `streaming.rs:529`). User scripts would start running from the CLI.
- **GameMode portal** D-Bus registration (`execution.rs:137`).
- **Session registry + watchdog**, which fixes B3.
- **History**: `record_launch_started`/`record_launch_finished`, version snapshot, known-good tag (`execution.rs:162`, `streaming.rs:276`, `:362`, `:416`). This is **YAN-837's "records history"**.
- **Per-launch `create_log_path`**, which fixes B1 but changes where logs are written.
- **Piped proton capture**, which fixes B2 but **changes `proton_run` stdout**.

`App::bootstrap` also requires `pre_runtime()` **before** any Tokio runtime exists (CLAUDE.md startup contract; `crosshook-app/src/bootstrap/pre_runtime.rs:106`). That conflicts with `#[tokio::main]` (`main.rs:14`), so `main` would need restructuring. That is YAN-837 work.

### Recommendation: split; minimal CLI change in YAN-798

1. **Dependency.** Add `crosshook-app = { path = "../crosshook-app" }` to `crates/crosshook-cli/Cargo.toml`, plus `tokio-util` if the call site needs a `CancellationToken` for `follow`. `check-app-deps.sh` only constrains crosshook-app's own tree, so it is unaffected.
2. **Request prep.** Call the service's UMU enrichment, which replaces `execution.rs:47` and `queries.rs:35`, instead of `launch.rs:54-58`. Keep `from_profile` + `validate`; they are already shared since M1-01.
3. **Streaming.** Replace `stream_helper_log` + `drain_log` with the service's file-follow primitive, built on `log_tail::follow` with `TailConfig::default()`.
   - It must be the same function `start_game` uses internally.
   - It is cancelled when `child.wait()` resolves. It then drains (at most `FINAL_DRAIN_CHUNKS`) and flushes the partial line.
   - The CLI sink `println!`s each raw line.
   - The sink must be lossless: a direct `FnMut` callback or an mpsc the CLI drains, not a lag-prone broadcast.
   - If a broadcast is used anyway, print `LogBatch::Dropped` to stderr as `warning: {n} log batches dropped`. That only happens on lag, so the goldens are unaffected.
4. **Tail.** Move the Tauri `safe_read_tail` (`src-tauri/.../diagnostics.rs:8-41`) into crosshook-app, e.g. `log_tail::read_tail(path, max_bytes) -> String` running on the blocking pool, keeping the `tracing::warn!`s. Delete both copies: CLI `launch.rs:321-346` and the src-tauri one.
5. **Diagnose.** Call the finalizer's _pure_ report step: `analyze` + `diagnostic_method_for_log` + sanitize, with the watchdog override optional (`Option<WatchdogOutcome>`, so the CLI passes `None`).
   - The CLI keeps printing `summary` and the pattern lines itself, since that is presentation.
   - The `diagnostic_method_for_log` remap only differs from today when a replayed stale log contains trainer-runner markers (B1), which the goldens do not cover.
6. **Leave unchanged in YAN-798:**
   - the log-path policy (`launch.rs:244-264`);
   - `Stdio::null()` for each method;
   - `validate_explicit_helper`, which concerns the CLI's `--scripts-dir` flag and stays CLI-side;
   - the CLI `resolve_steam_client_install_path`;
   - the dry-run code, which keeps calling `build_launch_preview`, so the JSON contract is unchanged;
   - no hooks, warnings, portal, registry, history or `App::bootstrap`.
7. **Delete:**
   - `stream_helper_log`, `drain_log` and `safe_read_tail`;
   - the now-unused imports: `tokio::fs::{self, OpenOptions}`, `tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt}`, `tokio::time::{sleep, Duration}`;
   - the CLI `Cargo.toml` tokio features `fs`, `io-util` and `time`, if nothing else needs them.

   Also keep `ExitStatus` (or `core_dumped`) available; do not reduce exit information to `Exited { code, signal }` only (see the hazards section).

**API this requires.** `services::launch::streaming::follow_until_exit` and `finalizer::{read_tail, diagnose}` as `pub` building blocks, which `start_game` composes. That gives one implementation with two callers, not a CLI shim. YAN-837 later replaces steps 3–6 with `App::bootstrap` + `start_game`, fixing B1–B3 along the way. Its own goldens then update the frozen files deliberately.

**Why not do them together.** YAN-798's "CLI output unchanged" criterion is easiest to prove when the CLI keeps its own spawn and stdio policy. `start_game` would at least:

- change `proton_run` stdout (the B2 fix);
- start running user hooks;
- write SQLite rows.

None of that is needed to delete the two helpers, and folding it in would silently pull YAN-837 into an 8-pt issue. Doing both together would only be forced if the service exposed nothing but `start_game`, which the design above avoids.

## Host-gateway coverage (`scripts/check-host-gateway.sh`)

- `SCAN_DIRS` (`check-host-gateway.sh:11-18`) covers crosshook-core, src-tauri, crosshook-app, crosshook-ui, crosshook-widgets and crosshook-view-model. **`crates/crosshook-cli/src` is not scanned.** The self-test `ROOTS` (`scripts/tests/check-host-gateway/run.sh:9-16`) and the ADR-0001 enforcement list (`docs/architecture/adr-0001-platform-host-gateway.md`, § Enforcement) match that list.
- Exposure today is nil. The CLI has no `Command::new` or `open::` calls at all and only uses core builders. After YAN-798, all CLI-triggered spawns still go through core builders, or through crosshook-app once a `ProcessSpawner` lands there. Both of those are scanned.
- Closing the gap is three lines: add the CLI root to `SCAN_DIRS`, add it to the self-test `ROOTS`, and add one bullet to ADR-0001. The self-test's per-root loop (`run.sh:54-58`) then covers the new root automatically.
- This is **not** in the YAN-798 scope list. Ship it only with approval, or under the M1 "gateway-check coverage" item.
- `scripts/check-startup-path.sh:105-112` does include `crosshook-cli/src`, but only for the `BaseDirs::new` rule. Its sleep and exit rules only cover `crosshook-app/src/bootstrap/*` and specific src-tauri files. The CLI's `sleep(…500ms)` and `process::exit` are therefore legal today, and step 3 removes the CLI `sleep`.

## Key Files

| File                                                                       | Role                                                                   | Importance |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ---------- |
| `crates/crosshook-cli/src/launch.rs:21-354`                                | The whole CLI launch path; helpers to delete at `:278-346`             | High       |
| `crates/crosshook-cli/src/cli_error.rs`                                    | Exit-code mapping                                                      | High       |
| `crates/crosshook-cli/src/main.rs:14-38`                                   | Error print + exit; `#[tokio::main]` blocks `App::bootstrap`           | Medium     |
| `crates/crosshook-app/src/log_tail.rs:23,62,118,178,266,332`               | `LogBatch`, `LogTail`, `poll`, `flush_partial`, `TailConfig`, `follow` | High       |
| `src-tauri/src/commands/launch/diagnostics.rs:8-75`                        | Tail twin, sanitize, diagnostic-method remap                           | High       |
| `src-tauri/src/commands/launch/streaming.rs:60-229,232-445`                | 500 ms re-read, pipe capture, finalizer                                | High       |
| `src-tauri/src/commands/launch/shared.rs:23-63`                            | UI-only log transforms; must stay in the adapter                       | High       |
| `crosshook-core/src/launch/diagnostics/{mod.rs,exit_codes.rs,patterns.rs}` | `analyze`, surface predicate, static texts                             | Medium     |
| `crosshook-core/src/launch/preview/{builder.rs,environment.rs}`            | Dry-run inputs and non-determinism                                     | Medium     |
| `scripts/check-host-gateway.sh:11-18`                                      | Scan roots (CLI missing)                                               | Low        |
| `.github/workflows/lint.yml:52`                                            | Test step lacks `-p crosshook-cli`                                     | Medium     |

## Recommendations for New Development

- **Follow** the `profile_writer_golden.rs` frozen-golden convention. Capture from pre-refactor `main` and commit the golden test in the first task.
- **Reuse** `log_tail::follow` and the new shared `read_tail` / `diagnose` / UMU-enrichment functions from both Tauri and the CLI. Keep CLI presentation local to the CLI: raw-line `println!`, the `severity_label` stderr lines, and the error strings.
- **Avoid:**
  - putting `!line.is_empty()` or gamescope suppression into service `Log` events;
  - a lag-prone broadcast as the CLI's only log source;
  - dropping `core_dumped` from `Exited`;
  - running hooks or writing launch history from the CLI in YAN-798;
  - "fixing" B1–B4 under the "unchanged" criterion without a recorded, approved delta.
- **Add** `-p crosshook-cli` to the `lint.yml` test step together with the golden test.
