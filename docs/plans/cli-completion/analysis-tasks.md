# CLI Completion: Task Structure Analysis

## Executive Summary

The feature-spec's 5-phase breakdown is well-structured and sound. This analysis refines task granularity, identifies true parallelization boundaries within `main.rs`, flags gotchas that affect task sequencing, and ensures every security mitigation and polish item is atomically assigned. All work lands in two files (`main.rs` + `args.rs`); parallelism is bounded by logical independence of the functions being added, not by file ownership.

---

## Recommended Phase Structure

### Phase 1 — Foundation and Simple Reads (2–3 tasks, fully parallelizable)

**Rationale**: Establishes the output pattern all later tasks follow. `profile list` is the canonical reference for all read commands. `status` is slightly more complex (aggregates multiple stores) but is read-only and shares the same output helpers.

| Task | Scope                                                                                                                                                                                                 | Files     | Parallelizable                                 |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | ---------------------------------------------- |
| 1-A  | Add `/// doc` comments to all command variants in `args.rs` for `--help` output                                                                                                                       | `args.rs` | Yes — purely additive, no logic                |
| 1-B  | Wire `profile list` handler (adds `handle_profile_list()` in `main.rs`)                                                                                                                               | `main.rs` | Yes — independent of 1-A and 1-C               |
| 1-C  | Wire `status` handler (adds `handle_status()` in `main.rs`); deduplicate `resolve_steam_client_install_path()` to call `crosshook_core::launch::runtime_helpers::resolve_steam_client_install_path()` | `main.rs` | Yes — independent function; dedup is same task |

**Dependency note**: 1-A does not block 1-B or 1-C. Tasks 1-B and 1-C can land in any order or as a single PR.

**Phase 1 acceptance criteria**:

- `crosshook profile list` prints profile names one per line; `--json` produces `{"profiles":[...],"count":N,"profiles_dir":"..."}`
- `crosshook status` prints version, profile count, Steam roots, Proton installs; `--json` produces the full schema
- `crosshook --help` shows accurate descriptions for all subcommands

---

### Phase 2 — Import/Export with Security Mitigations (2 tasks, parallelizable)

**Rationale**: Both operations touch `ProfileStore` but call different functions. Security mitigations (C-2, W-4) are bundled into the wiring task — they cannot be deferred.

| Task | Scope                                                                                                                                                                                                                                                                        | Files     | Parallelizable             |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | -------------------------- |
| 2-A  | Wire `profile import` with C-2 mitigation: validate `legacy_path` is a regular file (not symlink/device) via `symlink_metadata()` before calling `store.import_legacy()`; derive profile name from file stem; validate stem is valid profile name before saving              | `main.rs` | Yes — independent function |
| 2-B  | Wire `profile export` with W-4 mitigation: validate output parent dir is writable, output path is not a symlink; default output to `<cwd>/<name>.crosshook.json` when `--output` is omitted; pass `store.base_path.as_path()` (not `&store`) to `export_community_profile()` | `main.rs` | Yes — independent function |

**Gotcha — export_community_profile signature**: Takes `profiles_dir: &Path`, not `&ProfileStore`. Must call `export_community_profile(store.base_path.as_path(), &profile_name, &output_path)`.

**Phase 2 acceptance criteria**:

- `crosshook profile import --legacy-path /path/to/file.profile` converts and saves as TOML
- Import rejects symlinks and devices; emits `error:` + `hint:` to stderr
- `crosshook profile export --profile <name>` writes community JSON to `<cwd>/<name>.crosshook.json` by default
- Export validates output path before write (not a symlink, parent writable)

---

### Phase 3 — Steam Discovery (2 tasks, parallelizable; 1 sequenced cleanup)

**Rationale**: Both discovery commands call different Steam functions. The design decision on human-readable output formatting can be done within each task.

| Task | Scope                                                                                                                                                                                                                                                                                                                   | Files     | Parallelizable |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | -------------- |
| 3-A  | Wire `steam discover` — calls `discover_steam_root_candidates("", &mut diags)`, `crosshook_core::steam::libraries::discover_steam_libraries(&roots, &mut diags)` (not re-exported from `steam/mod.rs`, must import directly), `discover_compat_tools(&roots, &mut diags)`; emit diagnostics to stderr under `--verbose` | `main.rs` | Yes            |
| 3-B  | Wire `steam auto-populate --game-path <PATH>` — construct `SteamAutoPopulateRequest { game_path, steam_client_install_path: PathBuf::new() }`, call `attempt_auto_populate(&request)`, format per-field state output                                                                                                    | `main.rs` | Yes            |

**Gotcha — discover_steam_libraries import**: `crosshook_core::steam::libraries::discover_steam_libraries` is NOT re-exported from `steam/mod.rs`. Must be imported directly from the sub-module. `discover_compat_tools` IS re-exported.

**Phase 3 acceptance criteria**:

- `crosshook steam discover` shows roots, libraries, and Proton installs; `--verbose` shows diagnostic strings
- `crosshook steam auto-populate --game-path <path>` shows per-field state (Found/NotFound/Ambiguous)
- Both commands exit 0 even when Steam is not installed (empty result is informational)

---

### Phase 4 — Launch Completion (3 tasks, 1 sequenced + 2 parallelizable)

**Rationale**: The request-builder refactor must complete first — it is the foundation for `proton_run` and `native` dispatch. Once the builder exists, `proton_run` and `native` dispatch can be added in parallel. Security mitigations (C-1, W-2) are bundled into the specific tasks they protect.

| Task | Scope                                                                                                                                                                                                                                                                                                                                    | Files     | Parallelizable                        |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | ------------------------------------- |
| 4-A  | Refactor `steam_launch_request_from_profile()` into `launch_request_from_profile()` using a unified match on `resolve_launch_method(&profile)`; implement the three `LaunchRequest` shapes per the code template in feature-spec §API Design; delete the old function                                                                    | `main.rs` | No — must complete before 4-B and 4-C |
| 4-B  | Wire `proton_run` dispatch: add `METHOD_PROTON_RUN` arm to `launch_profile()` calling `build_proton_game_command(&request, &log_path)?`; ensure log directory is created with mode `0700` before spawning (W-2 mitigation: use `XDG_RUNTIME_DIR` for log path when available); call `stream_helper_log` identically to `steam_applaunch` | `main.rs` | After 4-A                             |
| 4-C  | Wire `native` dispatch: add `METHOD_NATIVE` arm calling `build_native_game_command(&request, &log_path)?`; add C-1 mitigation for helper script: at runtime verify `helper_script.is_file()` and `helper_script.metadata().uid() == nix::unistd::getuid()` before spawning `steam_applaunch`                                             | `main.rs` | After 4-A                             |

**Gotcha — log directory creation**: `build_proton_game_command` and `build_native_game_command` call `attach_log_stdio()` internally which opens the log file but does NOT create parent directories. The CLI's `launch_log_path()` does not create the directory either. Task 4-B must add `fs::create_dir_all(&log_dir)` (with `DirBuilder::mode(0o700)`) before calling these builders.

**Gotcha — proton_run for profiles without compatdata_path**: `resolve_steam_client_install_path` walks ancestors of `compatdata_path` looking for `steam.sh`. For `proton_run` profiles, `compatdata_path` may be empty. The builder for `proton_run` should fall back to `discover_steam_root_candidates("")` when `compatdata_path` is empty rather than relying on the ancestor walk.

**Gotcha — stream_helper_log reuse**: For `proton_run` and `native`, `attach_log_stdio()` inside the builder already handles log I/O redirection. The existing `stream_helper_log` / `drain_log` polling loop works for all three methods since all write to the same log file pattern.

**Phase 4 acceptance criteria**:

- `crosshook launch --profile <name>` works for profiles with `method = "proton_run"` and `method = "native"`
- Helper script existence and UID ownership are verified before `steam_applaunch` spawn (C-1)
- Log directory created with mode `0700` using `XDG_RUNTIME_DIR` when available (W-2)
- `--dry-run` flag exposes `build_launch_preview()` output (can be bundled here or in Phase 5)
- No regressions: `steam_applaunch` continue to work

---

### Phase 5 — Polish and Production Readiness (4 tasks, mostly parallelizable)

**Rationale**: Exit codes, quickstart docs, shell completions, and dead code cleanup are independent. The `emit_placeholder()` deletion is gated on all Phase 1-4 tasks completing, so it lands last.

| Task | Scope                                                                                                                                                                                                                                                                         | Files                              | Parallelizable                                                  |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | --------------------------------------------------------------- |
| 5-A  | Standardize exit codes: replace generic `process::exit(1)` paths with code 3 for `ProfileStoreError::NotFound`, code 4 for launch failure, code 5 for Steam-not-found; add structured exit code handling in `run()` instead of relying on `eprintln!` + exit(1) from `main()` | `main.rs`                          | Yes                                                             |
| 5-B  | Add `--dry-run` flag to `LaunchCommand` args; wire to `build_launch_preview()` in `launch_profile()`; format preview output (human + JSON)                                                                                                                                    | `args.rs` + `main.rs`              | Yes                                                             |
| 5-C  | Add shell completion generation via `clap_complete` to `Cargo.toml` + generate completions command or build-time generation; update `crosshook-cli/Cargo.toml` to add `clap_complete` dependency                                                                              | `Cargo.toml`, `args.rs`, `main.rs` | Yes                                                             |
| 5-D  | Update `docs/getting-started/quickstart.md` with CLI section covering all 7 commands with examples; delete `emit_placeholder()` function (dead code after Phase 1-4); run `cargo test -p crosshook-cli` and `cargo test -p crosshook-core` to confirm no regressions          | `main.rs`, `quickstart.md`         | Dead code deletion depends on Phase 1-4; docs can start earlier |

**Phase 5 acceptance criteria**:

- All commands exit with correct numeric codes on success and the documented failure categories
- `crosshook completions bash` (or equivalent) generates shell completions
- `docs/getting-started/quickstart.md` has a CLI section
- `emit_placeholder()` is deleted with no remaining call sites
- `cargo test -p crosshook-core && cargo test -p crosshook-cli` pass

---

## Task Granularity Recommendations

### Recommended boundaries (1-3 functions per task)

Each task above adds or modifies 1-3 independent handler functions. This is the right granularity because:

1. `main.rs` is a single file — tasks parallelized by function independence, not by file split
2. `args.rs` changes (task 1-A, 5-B, 5-C) are purely additive and do not conflict with `main.rs` tasks
3. Security mitigations (C-1, C-2, W-2, W-4) are inlined into the wiring task they protect — never standalone tasks — so they cannot be accidentally skipped

### Tasks that should NOT be split further

- **1-C (status + dedup)**: `resolve_steam_client_install_path` deduplication is 3 lines in `status` wiring; separating it adds overhead with no benefit
- **4-A (builder refactor)**: The `steam_launch_request_from_profile` → `launch_request_from_profile` refactor is atomic; splitting it leaves the codebase in a broken intermediate state
- **Security mitigations (C-1, C-2, W-2, W-4)**: These must ship with the wiring they protect, not as separate tasks

### Tasks that could be split if two agents work the same file simultaneously

- **Phase 1**: 1-B and 1-C can be assigned to separate agents (different functions in `main.rs`)
- **Phase 3**: 3-A and 3-B can be assigned to separate agents
- **Phase 4**: 4-B and 4-C can run in parallel after 4-A merges

---

## Dependency Analysis

```
Phase 1 (1-A, 1-B, 1-C)         — no dependencies; all parallel
    |
Phase 2 (2-A, 2-B)              — no hard dependency on Phase 1; can overlap
    |                              (Phase 1 establishes pattern, Phase 2 follows it)
Phase 3 (3-A, 3-B)              — no hard dependency on Phase 1 or 2; can overlap
    |
Phase 4-A (builder refactor)    — depends on Phase 1 pattern being established
    |
Phase 4-B, 4-C (parallel)       — depends on 4-A completing
    |
Phase 5-A, 5-B, 5-C (parallel)  — depends on Phase 4 completing
    |
Phase 5-D (dead code deletion)  — depends on ALL Phase 1-4 tasks completing
```

**Key insight**: Phases 1, 2, and 3 can be worked in parallel by separate agents since they add independent handler functions. Phase 4-A is the single blocking sequential task in the entire plan.

---

## File-to-Task Mapping

### `crates/crosshook-cli/src/args.rs`

| Lines / Struct            | Task | Change                               |
| ------------------------- | ---- | ------------------------------------ |
| All command variant enums | 1-A  | Add `///` doc comments               |
| `LaunchCommand` struct    | 5-B  | Add `--dry-run: bool` field          |
| N/A (new)                 | 5-C  | Add `GenerateCompletions` subcommand |

### `crates/crosshook-cli/src/main.rs`

| Location                                              | Task          | Change                                                     |
| ----------------------------------------------------- | ------------- | ---------------------------------------------------------- |
| `handle_profile_command` → `ProfileCommand::List`     | 1-B           | Replace `emit_placeholder` with `handle_profile_list()`    |
| `run()` → `Command::Status`                           | 1-C           | Replace `emit_placeholder` with `handle_status()`          |
| `resolve_steam_client_install_path()`                 | 1-C           | Delete local fn; use core version                          |
| `handle_profile_command` → `ProfileCommand::Import`   | 2-A           | Replace `emit_placeholder` with real impl + C-2 mitigation |
| `handle_profile_command` → `ProfileCommand::Export`   | 2-B           | Replace `emit_placeholder` with real impl + W-4 mitigation |
| `handle_steam_command` → `SteamCommand::Discover`     | 3-A           | Replace `emit_placeholder` with real impl                  |
| `handle_steam_command` → `SteamCommand::AutoPopulate` | 3-B           | Replace `emit_placeholder` with real impl                  |
| `steam_launch_request_from_profile()`                 | 4-A           | Refactor to `launch_request_from_profile()`                |
| `launch_profile()`                                    | 4-A, 4-B, 4-C | Add multi-method dispatch                                  |
| `launch_log_path()`                                   | 4-B           | Change to use `XDG_RUNTIME_DIR`; add dir creation          |
| `spawn_helper()`                                      | 4-C           | Add C-1 file + uid validation before invoke                |
| `run()` and `main()`                                  | 5-A           | Structured exit codes                                      |
| `launch_profile()`                                    | 5-B           | Add `--dry-run` path                                       |
| `emit_placeholder()`                                  | 5-D           | Delete entire function                                     |
| `default_steam_roots()`                               | 5-D           | Delete (merged into core call)                             |

### `crates/crosshook-cli/Cargo.toml`

| Change                                   | Task |
| ---------------------------------------- | ---- |
| Add `clap_complete` dev/build dependency | 5-C  |

### `docs/getting-started/quickstart.md`

| Change                              | Task |
| ----------------------------------- | ---- |
| Add CLI section with all 7 commands | 5-D  |

---

## Optimization Opportunities

### 1. Phases 1–3 can run in parallel

Since each phase adds independent async handler functions to `main.rs` with no shared state, separate agents can work phases 1, 2, and 3 simultaneously. The only merge coordination needed is ensuring functions are added in non-conflicting line regions (or rebasing trivially).

### 2. `--dry-run` can bundle with Phase 4

`build_launch_preview()` already exists in `crosshook_core::launch`. Adding `--dry-run` to `LaunchCommand` and wiring the preview path takes ~15 lines. It is a natural addition to the Phase 4 launch work (task 4-B or 4-C) rather than a separate Phase 5 task. Bundling it saves a context switch.

### 3. `resolve_steam_client_install_path` dedup is Phase 1, not Phase 4

The local `resolve_steam_client_install_path()` in `main.rs` is only called by `steam_launch_request_from_profile()`. Deduplicating it in Phase 1 (task 1-C) is correct because the `status` command also needs Steam root resolution and should use the canonical core function. Doing it in Phase 4 would mean `status` uses the local version first, creating a temporary inconsistency.

### 4. Security mitigations inline into their parent task

Do not create standalone security tasks. Reviewers need to see the mitigation alongside the vulnerable code path in the same diff. Inline each mitigation:

- C-1 (helper script validation) → Task 4-C
- C-2 (import path containment) → Task 2-A
- W-2 (log path) → Task 4-B
- W-4 (export path validation) → Task 2-B

---

## Implementation Strategy Recommendations

### 1. Use `handle_diagnostics_command` as the exact template

Every new handler should match this structure: initialize store, call core function, branch on `global.json`. Do not introduce a shared `output()` helper — the diagnostics example shows the inline `if global.json` branch is already concise enough and avoids an unnecessary abstraction.

### 2. All handler functions must be `async fn`

The existing `handle_diagnostics_command` is `fn` (not `async`) — this is explicitly documented as an anomaly to avoid repeating. All new handlers must be `async fn` for consistency with `handle_profile_command` and `handle_steam_command`.

### 3. `ProfileCommand::List` must pass `profile_dir` arg

The `ProfileCommand::List` variant currently has no args struct — it is just a unit variant. To support `--config` override for the profile directory, the handler must use `global.config.clone()` (not a per-command `profile_dir`) since `List` has no `profile_dir` field. This matches how `handle_diagnostics_command` uses `global.config`.

### 4. For `status`, partial failure must not abort

The `status` command aggregates ProfileStore, SettingsStore, and Steam discovery. Each of these can fail independently. The handler should collect errors into a `Vec<String>` and always emit partial output rather than failing entirely. This matches the UX spec's requirement: "Individual section failures should not abort the command."

### 5. `LaunchRequest` default fields

The `LaunchRequest` struct does not implement `Default`. Use explicit `RuntimeLaunchConfig::default()` and `SteamLaunchConfig { ..Default::default() }` for methods that don't use those sections. Confirm `Default` is derived on `SteamLaunchConfig` and `RuntimeLaunchConfig` before using `..Default::default()` spread syntax.

### 6. Async convention for `handle_diagnostics_command`

`handle_diagnostics_command` is currently a synchronous `fn` called with `?` from the async `run()`. All new Phase 1-3 handlers are `async fn`. Task 5-A (exit codes) should also convert `handle_diagnostics_command` to `async fn` for consistency — or leave it as a known exception with a code comment. Either way, do not silently break the pattern.

### 7. Test additions

Each new command wired in Phase 1-3 should add at minimum one parse test in `args.rs` following the existing `parses_profile_import_command` / `parses_steam_auto_populate_command` pattern. Phase 4 does not need new parse tests (args don't change). Phase 5-B (`--dry-run`) needs one new parse test.

---

## Gotchas Summary (from research)

| #   | Gotcha                                                                                                                                       | Affects Tasks       |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- |
| G-1 | `discover_steam_libraries` is NOT re-exported from `steam/mod.rs` — import as `crosshook_core::steam::libraries::discover_steam_libraries`   | 3-A, 1-C            |
| G-2 | `export_community_profile` takes `&Path` (not `&ProfileStore`) as first arg                                                                  | 2-B                 |
| G-3 | Log parent directory must be created before `build_proton_game_command` / `build_native_game_command`                                        | 4-B, 4-C            |
| G-4 | `handle_diagnostics_command` is `fn` not `async fn` — anomaly, do not copy                                                                   | All Phase 1-3 tasks |
| G-5 | `ProfileStore::load()` returns effective profile (local overrides merged) — no need to call `effective_profile()` again                      | 4-A                 |
| G-6 | `LaunchRequest.method` may be empty string (deferred to `resolved_method()`) — `validate()` handles this correctly                           | 4-A                 |
| G-7 | `proton_run` profiles may have empty `compatdata_path` — steam client path resolution must fall back to `discover_steam_root_candidates("")` | 4-A                 |
| G-8 | `build_proton_game_command` and `build_native_game_command` return `std::io::Result<Command>` — must propagate with `?`                      | 4-B, 4-C            |
| G-9 | `import_legacy` derives profile name from file stem — validate stem against `validate_name()` before saving                                  | 2-A                 |
