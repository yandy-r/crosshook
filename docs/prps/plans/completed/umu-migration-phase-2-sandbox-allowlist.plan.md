# Plan: umu-launcher Migration — Phase 2: Sandbox Allowlist Plumbing

## Summary

Plumb pressure-vessel filesystem allowlist env vars (`STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW`) into both Proton builders so trainer paths outside `$HOME` become reachable when pressure-vessel eventually wraps execution in Phase 3. A new pure helper `collect_pressure_vessel_paths(request) -> Vec<String>` in `runtime_helpers.rs` returns a deduplicated list of `{dirname(game_path), dirname(trainer_host_path) when SourceDirectory, working_directory}`; callers colon-join it before `env.insert`. The two env keys join `WINE_ENV_VARS_TO_CLEAR` + the matching shell-helper `unset` block, and surface in the Launch Preview next to `PROTON_VERB`. **Zero observable behavior change under direct Proton today** — pressure-vessel is not in the process graph yet, and direct Proton ignores both vars.

## User Story

As a CrossHook developer preparing the umu-launcher migration, I want pressure-vessel RW paths to be computed and injected into the Proton env map per-builder, so that when Phase 3 activates `umu-run`, trainers stored under `/opt/games/...` or any non-`$HOME` prefix are visible inside the pressure-vessel container — and the allowlist plumbing is already proven and tested before any umu code path goes live.

## Problem → Solution

- **Current state**: Neither `build_proton_game_command`, `build_proton_trainer_command`, nor `build_flatpak_steam_trainer_command` sets `STEAM_COMPAT_LIBRARY_PATHS` or `PRESSURE_VESSEL_FILESYSTEMS_RW`. Repo-wide search confirms greenfield: the keys do not appear in any source file, shell helper, test fixture, or Flatpak manifest. When Phase 3 routes non-Steam trainers through `umu-run` (pressure-vessel wrapped), any trainer host path outside the default pressure-vessel mount set (home, `/tmp`, prefix) becomes invisible inside the sandbox and the trainer exec fails.
- **Desired state**: A pure `collect_pressure_vessel_paths(&LaunchRequest) -> Vec<String>` helper in `runtime_helpers.rs` collects `{dirname(game_path), dirname(trainer_host_path) when SourceDirectory, working_directory}`, skipping empty strings and deduplicating while preserving insertion order. Both Proton builders call `paths.join(":")` and insert `STEAM_COMPAT_LIBRARY_PATHS` + `PRESSURE_VESSEL_FILESYSTEMS_RW` immediately after the Phase 1 `PROTON_VERB` insert. Preview re-derives the same value via `collect_runtime_proton_environment` so users see the allowlist before launch. `WINE_ENV_VARS_TO_CLEAR` (and the parallel shell-helper `unset` block) add the two keys to prevent host leakage. Under direct Proton the vars are inert — the `cargo test -p crosshook-core` suite is green with zero behavior change, and preview grows two `ProtonRuntime`-tagged env entries.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/umu-launcher-migration.prd.md`
- **PRD Phase**: Phase 2 — Sandbox allowlist plumbing
- **Tracking Issue**: [#255](https://github.com/yandy-r/crosshook/issues/255) (phase tracker), [#235](https://github.com/yandy-r/crosshook/issues/235) (implementation)
- **Estimated Files**: 5

### Persistence / Usability

| Datum                                                                                             | Classification                   | Notes                                                                                                                                              |
| ------------------------------------------------------------------------------------------------- | -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW` (values on the spawned process) | **Ephemeral runtime state**      | Derived each launch from `LaunchRequest` (paths, trainer mode, working directory); not written to TOML or SQLite.                                  |
| Output of `collect_pressure_vessel_paths`                                                         | **Ephemeral runtime state**      | Pure function of the same inputs; recomputed per builder/preview call.                                                                             |
| Inclusion of those two key names in `WINE_ENV_VARS_TO_CLEAR`                                      | **Operational/history metadata** | In-repo constant (not SQLite `metadata.db`): names of host env keys cleared before per-launch inject; paired with shell-helper `unset` for parity. |

| Datum                                                                                                            | Classification                  | Notes                                                                                                                                                                                                                                                                                                                      |
| ---------------------------------------------------------------------------------------------------------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW` (values on the spawned process)                | **Ephemeral runtime state**     | Inserted into the command env map at launch from the current `LaunchRequest`; not persisted.                                                                                                                                                                                                                               |
| Output of `collect_pressure_vessel_paths` and the colon-joined allowlist string                                  | **Ephemeral runtime state**     | Computed in memory per builder / preview call; identical inputs yield the same string; no caching layer or DB write.                                                                                                                                                                                                       |
| `WINE_ENV_VARS_TO_CLEAR` (including `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW` key names) | **Source-only (non-persisted)** | Static slice in `env.rs`—**not** a `metadata.db` table or migration. Defines which host env keys to strip before CrossHook injects its own map; **runtime effect** is per-launch host clearing (ephemeral). **Parity:** `steam-host-trainer-runner.sh` `unset` lists the same names so helper and Rust paths stay aligned. |

**Primary classification:** **(B)** — **Ephemeral runtime state** for derived paths and injected env values; **source-only** for the clear-list constant. **Not** “operational/history metadata” in the SQLite sense—no Phase 2 rows or schema changes in `metadata.db`.

**Migration / backward compatibility:** No SQLite or TOML migration; Phase 2 adds source lines only. Inert under direct Proton until Phase 3 consumes these vars.

**Offline:** Path math is local; no network.

**Degraded / fallback:** Empty or absent path components → empty join → pressure-vessel treats as no extra RW mounts (same as “no allowlist”).

**User visibility / editability:** Allowlist **values** visible in Launch Preview only (`ProtonRuntime`, `ProtonRun`). Clear-list membership is **not** user-facing configuration—editability is via code review / `env.rs` + shell-helper sync, not UI or DB.

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order. File ownership is disjoint within each batch — no two tasks in the same batch write to the same file.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | —          | 3              |
| B2    | 2.1, 2.2      | B1         | 2              |
| B3    | 3.1           | B2         | 1              |

- **Total tasks**: 6
- **Total batches**: 3
- **Max parallel width**: 3

---

## UX Design

### Before

```
Launch Preview — game (non-Steam, proton_run)
  [environment] # N vars (Phase 1 complete)
  WINEPREFIX = "/home/user/.prefixes/the-game"
  STEAM_COMPAT_DATA_PATH = "/home/user/.prefixes/the-game"
  STEAM_COMPAT_CLIENT_INSTALL_PATH = "/home/user/.local/share/Steam"
  GAMEID = "12345"
  PROTON_VERB = "waitforexitandrun"
  ...
  (no STEAM_COMPAT_LIBRARY_PATHS)
  (no PRESSURE_VESSEL_FILESYSTEMS_RW)

Launch Preview — trainer (SourceDirectory)
  [environment] # M vars
  WINEPREFIX = "/home/user/.prefixes/the-game"
  GAMEID = "12345"
  PROTON_VERB = "runinprefix"
  ...
  (no STEAM_COMPAT_LIBRARY_PATHS)
  (no PRESSURE_VESSEL_FILESYSTEMS_RW)
```

### After

```
Launch Preview — game (non-Steam, proton_run)
  [environment] # N+2 vars
  WINEPREFIX = "/home/user/.prefixes/the-game"
  STEAM_COMPAT_DATA_PATH = "/home/user/.prefixes/the-game"
  STEAM_COMPAT_CLIENT_INSTALL_PATH = "/home/user/.local/share/Steam"
  GAMEID = "12345"
  PROTON_VERB = "waitforexitandrun"
  STEAM_COMPAT_LIBRARY_PATHS = "/opt/games/TheGame:/opt/trainers"
  PRESSURE_VESSEL_FILESYSTEMS_RW = "/opt/games/TheGame:/opt/trainers"
  ...

Launch Preview — trainer (SourceDirectory)
  [environment] # M+2 vars
  WINEPREFIX = "/home/user/.prefixes/the-game"
  GAMEID = "12345"
  PROTON_VERB = "runinprefix"
  STEAM_COMPAT_LIBRARY_PATHS = "/opt/games/TheGame:/opt/trainers"
  PRESSURE_VESSEL_FILESYSTEMS_RW = "/opt/games/TheGame:/opt/trainers"
  ...

[cleared_variables]
  ... STEAM_COMPAT_LIBRARY_PATHS ... PRESSURE_VESSEL_FILESYSTEMS_RW ...
```

### Interaction Changes

| Touchpoint                  | Before                              | After                                                                                      | Notes                                                     |
| --------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------ | --------------------------------------------------------- |
| Launch Preview (game)       | no `STEAM_COMPAT_LIBRARY_PATHS`     | `STEAM_COMPAT_LIBRARY_PATHS = "<game_dir>:<working_dir>"` (dedup)                          | `ProtonRuntime` source tag; mirrors Phase 1 `PROTON_VERB` |
| Launch Preview (game)       | no `PRESSURE_VESSEL_FILESYSTEMS_RW` | identical value to `STEAM_COMPAT_LIBRARY_PATHS`                                            | Same `collect_pressure_vessel_paths(request).join(":")`   |
| Launch Preview (trainer)    | no `STEAM_COMPAT_LIBRARY_PATHS`     | `SourceDirectory` includes trainer_dir; `CopyToPrefix` omits it (inside prefix)            | `trainer_loading_mode` gates trainer_dir inclusion        |
| Launch Preview (trainer)    | no `PRESSURE_VESSEL_FILESYSTEMS_RW` | identical value to `STEAM_COMPAT_LIBRARY_PATHS`                                            | Paired keys always hold the same value                    |
| Executed command env (both) | unset                               | colon-joined deduped path list                                                             | Inert under direct Proton; activates under umu in Phase 3 |
| `cleared_variables`         | 32 entries                          | 34 entries (adds `STEAM_COMPAT_LIBRARY_PATHS` + `PRESSURE_VESSEL_FILESYSTEMS_RW`)          | `env.rs` + shell-helper parity mirrors Phase 1 pattern    |
| Steam applaunch path        | untouched                           | untouched (preview dispatch gates Proton-runtime env to `ResolvedLaunchMethod::ProtonRun`) | No Steam-profile behavior change                          |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                          | Lines                                    | Why                                                                                                                                                                    |
| -------------- | ----------------------------------------------------------------------------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `docs/prps/plans/completed/umu-migration-phase-1-proton-verb-hygiene.plan.md` | all                                      | Phase 1 is the exact mirror template — pattern for env insert, preview push, env.rs hygiene, shell parity, and test shape                                              |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 402-486                                  | `build_proton_game_command` — env assembly site; insert pressure-vessel vars immediately after `PROTON_VERB` (line 418)                                                |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 488-569                                  | `build_proton_trainer_command` — env assembly site at line 525; `TrainerLoadingMode::SourceDirectory` vs `CopyToPrefix` match at 500-506                               |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 388-400                                  | `build_flatpak_steam_trainer_command` — delegates to `build_proton_trainer_command`; pressure-vessel inserts flow through automatically, one inheritance test required |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`    | 1-85, 338-464                            | Target module for the new helper; `merge_runtime_proton_into_map` + `resolve_effective_working_directory` are sibling precedents                                       |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`            | 435-480                                  | `collect_runtime_proton_environment` — Phase 1 added `PROTON_VERB` at 470-479; Phase 2 pushes both new keys here                                                       |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`            | 294-347                                  | `ResolvedLaunchMethod` dispatch — confirms new env only goes in the `ProtonRun` branch, not `SteamApplaunch` / `Native`                                                |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`                | all                                      | `WINE_ENV_VARS_TO_CLEAR` constant + length-pin test; add both new keys here (34 entries)                                                                               |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`            | 32-100                                   | `LaunchRequest.game_path`, `trainer_host_path`, `trainer_loading_mode`, `runtime.working_directory` — helper input fields                                              |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`            | 51-78                                    | `TrainerLoadingMode::SourceDirectory` / `CopyToPrefix` enum — correct enum name (NOT `TrainerSourceMode`)                                                              |
| P1 (important) | `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`           | 444-470                                  | Parallel `unset` block; Phase 1 added `PROTON_VERB` here; Phase 2 adds the two pressure-vessel keys                                                                    |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 199-227                                  | `insert_sorted_env_key_list` + `collect_trainer_builtin_env_keys` — prior art for dedup + `Vec<String>` helper shape                                                   |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 947-956, 1105-1151, 1286-1331, 1333-1379 | `command_env_value` helper + Phase 1 builder env tests — test-naming and assertion patterns to mirror                                                                  |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/platform.rs`                  | 42-61                                    | `normalize_flatpak_host_path` is infallible, returns `""` on empty input — helper must skip empty-derived paths                                                        |
| P2 (reference) | `docs/prps/prds/umu-launcher-migration.prd.md`                                | 142-175                                  | Phase 2 scope, success signals, and risk notes from the PRD                                                                                                            |

## External Documentation

| Topic                                           | Source                                                            | Key Takeaway                                                                                                                                                                                                                                                               |
| ----------------------------------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pressure-vessel filesystem bindings (reference) | `docs/prps/prds/umu-launcher-migration.prd.md` Technical Approach | `STEAM_COMPAT_LIBRARY_PATHS` + `PRESSURE_VESSEL_FILESYSTEMS_RW` are consumed by `steam-runtime-tools`' pressure-vessel launcher. Direct Proton does not consume them (inert). umu wraps Proton with pressure-vessel and reads both lists to build the container mount map. |

(No external library lookups required for Phase 2 — all knowledge is captured from Phase 1 precedent and in-tree PRD.)

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING*CONVENTION — `collect*\*`returns`Vec<T>` (pure)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:217-227
fn collect_trainer_builtin_env_keys(
    env: &BTreeMap<String, String>,
    custom_env_vars: &BTreeMap<String, String>,
) -> Vec<String> {
    // ... pure helper: iterates, filters, collects, returns Vec<String>
}
```

`collect_pressure_vessel_paths(&LaunchRequest) -> Vec<String>` follows this shape exactly: pure fn, no mutation, caller owns env insertion.

### ERROR_HANDLING — infallible `&str` normalization + trim-then-check

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/platform.rs:42-61
pub fn normalize_flatpak_host_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() { return String::new(); }
    // ... infallible passthrough; empty in → empty out
}
```

Helper must skip empty-derived paths (e.g., `Path::new("").parent()` returns `None`). Do NOT fail — propagate "skip this entry" semantics consistent with `resolve_effective_working_directory`.

### LOGGING_PATTERN — `tracing::debug!` per builder, no env dumps

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:440-449
tracing::debug!(
    configured_proton_path = request.runtime.proton_path.trim(),
    // ... kv fields
    "building proton game launch"
);
```

Do NOT add a new `tracing` call for this plumbing — mirror Phase 1 silence. (Values surface via preview; allowlist content is reproducible from request.)

### REPOSITORY_PATTERN — env BTreeMap assembly order

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:410-422 (game)
let mut env = host_environment_map();
merge_runtime_proton_into_map(&mut env, ...);
merge_optimization_and_custom_into_map(&mut env, &directives.env, &BTreeMap::new());
env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
env.insert("PROTON_VERB".to_string(), "waitforexitandrun".to_string());
// ← pressure-vessel inserts belong here (both builders, after PROTON_VERB)
```

Trainer builder (script_runner.rs:518-525) mirrors this, minus the optimization merge. Both builders get the two inserts at identical relative position.

### SERVICE_PATTERN — dedup preserving first occurrence

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:205-213 (insert_sorted_env_key_list)
let mut keys = ...;
keys.sort_unstable();
keys.dedup();
// joined with "," for env value
```

For `collect_pressure_vessel_paths` the PRD explicitly requires **insertion-order dedup** (not sorted) — the game dir / trainer dir / working dir order is meaningful to the user. Use a manual seen-set loop rather than `sort + dedup`.

### TEST_STRUCTURE — builder env assertion via `command_env_value`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:947-956
fn command_env_value(command: &Command, key: &str) -> Option<String> {
    command.as_std().get_envs().find_map(|(k, v)|
        (k == OsStr::new(key)).then(|| v.map(...))
    ).flatten()
}
```

Phase 1 builder tests use this exact helper + naming pattern `proton_<builder>_command_sets_<key>_to_<value>`. Mirror for Phase 2: `proton_game_command_sets_pressure_vessel_paths_from_request`, `proton_trainer_command_omits_trainer_dir_under_copy_to_prefix`, `flatpak_steam_trainer_command_inherits_pressure_vessel_allowlist`.

### PREVIEW_PATTERN — `EnvVarSource::ProtonRuntime` push in `collect_runtime_proton_environment`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/preview.rs:475-479
env.push(PreviewEnvVar {
    key: "PROTON_VERB".to_string(),
    value: proton_verb.to_string(),
    source: EnvVarSource::ProtonRuntime,
});
```

Two new `env.push(PreviewEnvVar { ... })` calls land immediately after the `PROTON_VERB` push. Only the `ResolvedLaunchMethod::ProtonRun` dispatch branch (preview.rs:312-324) routes here — `SteamApplaunch` and `Native` remain untouched.

---

## Files to Change

| File                                                                       | Action | Justification                                                                                                                                                         |
| -------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATE | Add pure helper `collect_pressure_vessel_paths(&LaunchRequest) -> Vec<String>` + unit tests                                                                           |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | UPDATE | Insert `STEAM_COMPAT_LIBRARY_PATHS` + `PRESSURE_VESSEL_FILESYSTEMS_RW` into both builders' env maps + sibling tests (game, trainer, flatpak-steam-trainer delegation) |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | UPDATE | Push both keys from `collect_runtime_proton_environment` + mirror test on `launch_trainer_only` toggle + `CopyToPrefix` branch                                        |
| `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`             | UPDATE | Append `STEAM_COMPAT_LIBRARY_PATHS` + `PRESSURE_VESSEL_FILESYSTEMS_RW` to `WINE_ENV_VARS_TO_CLEAR`; bump length assertion 32 → 34                                     |
| `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`        | UPDATE | Add matching `unset STEAM_COMPAT_LIBRARY_PATHS PRESSURE_VESSEL_FILESYSTEMS_RW` lines in the shell parity block                                                        |

## NOT Building

- **No umu branching** — Phase 2 is inert-under-Proton plumbing only. `resolve_umu_run_path()` must not be called from either builder (Phase 3's job). No `UmuPreference` reads.
- **No Flatpak manifest changes** — `packaging/flatpak/dev.crosshook.CrossHook.yml` is unchanged. `--filesystem=xdg-data/umu:create` is Phase 5.
- **No onboarding / readiness changes** — the stale umu readiness check stays as-is until Phase 5.
- **No frontend / Tauri IPC changes** — the two new preview env entries flow through the existing `Vec<PreviewEnvVar>` IPC surface with no schema change (same `EnvVarSource::ProtonRuntime` enum variant).
- **No CLI changes** — `crosshook-cli` already hands `&request` to `build_proton_*_command`; no new flags.
- **No Steam-applaunch path mutation** — preview dispatch (preview.rs:312-324) confines the new env to `ResolvedLaunchMethod::ProtonRun`; Steam runtime is untouched.
- **No `collect_pressure_vessel_paths` caching / memoization** — it is pure and cheap; re-derive per call from the request.
- **No handling of non-x86_64 architectures** — out of scope per PRD.
- **No change to `stage_trainer_into_prefix`** — `CopyToPrefix` already lands the trainer inside `$WINEPREFIX/drive_c/CrossHook/StagedTrainers/`, which is already under a mounted path; the helper correctly omits the trainer dir in that mode.

---

## Step-by-Step Tasks

### Task 1.1: Add `collect_pressure_vessel_paths` helper + unit tests — Depends on [none]

- **BATCH**: B1
- **ACTION**: Implement the pure helper in `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` and add its unit tests at the bottom of the same file's `#[cfg(test)] mod tests` block.
- **IMPLEMENT**: Add `pub fn collect_pressure_vessel_paths(request: &LaunchRequest) -> Vec<String>` that computes, in order: `dirname(normalize_flatpak_host_path(&request.game_path))`; when `request.trainer_loading_mode == TrainerLoadingMode::SourceDirectory` and `trainer_host_path` is non-empty, `dirname(normalize_flatpak_host_path(&request.trainer_host_path))`; when `request.runtime.working_directory` is non-empty, `normalize_flatpak_host_path(&request.runtime.working_directory).trim().to_string()`. Skip empty strings; use a seen `HashSet<String>` to dedup while preserving first-occurrence order. Return `Vec<String>`. Add `use crate::launch::request::LaunchRequest; use crate::profile::TrainerLoadingMode;` to the top of the file if not present (check existing imports first).
- **MIRROR**: `SERVICE_PATTERN` (insertion-order dedup) + `NAMING_CONVENTION` (`collect_*` returns `Vec<T>`). Signature matches `collect_trainer_builtin_env_keys` shape; body uses `normalize_flatpak_host_path` + `Path::new(...).parent()` idiom.
- **IMPORTS**: `std::collections::HashSet`, `std::path::Path`, `crate::platform::normalize_flatpak_host_path`, `crate::launch::request::LaunchRequest`, `crate::profile::TrainerLoadingMode`.
- **GOTCHA**: `Path::new("").parent()` returns `None` — skip silently. `Path::new("/game.exe").parent()` returns `Some("/")` — treat `"/"` as a valid (if unusual) entry and let dedup handle repeats. Do NOT call `resolve_effective_working_directory` here; that function falls back to `primary_path.parent()` which would double-count the game dir. Use the raw `runtime.working_directory` only if explicitly set.
- **VALIDATE**: Six unit tests mirroring `runtime_helpers.rs` test-module style: (a) empty `LaunchRequest` returns empty Vec; (b) game + trainer SourceDirectory + working_dir returns deduped 3-entry list; (c) game == working_dir collapses to 1 entry (dedup); (d) `CopyToPrefix` omits trainer dir; (e) empty `trainer_host_path` with `SourceDirectory` omits trainer entry (no crash, no empty push); (f) Flatpak-prefixed paths (`/run/host/opt/games/...`) normalize to host paths. Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::runtime_helpers::tests::collect_pressure_vessel_paths` — expect 6/6 green.

### Task 1.2: Add pressure-vessel keys to `WINE_ENV_VARS_TO_CLEAR` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend the constant in `src/crosshook-native/crates/crosshook-core/src/launch/env.rs` and update the length-pin test.
- **IMPLEMENT**: Append two entries to `WINE_ENV_VARS_TO_CLEAR` after the existing `"VKD3D_DEBUG"` line: `"STEAM_COMPAT_LIBRARY_PATHS", // Cleared for direct Proton; set per-command by builders (pressure-vessel RW allowlist).` and `"PRESSURE_VESSEL_FILESYSTEMS_RW", // Cleared for direct Proton; set per-command by builders (pressure-vessel RW allowlist, paired with STEAM_COMPAT_LIBRARY_PATHS).`. Update the `wine_env_vars_match_expected_list` test in the same file: bump `assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 32)` to `34`, and add two `assert!(WINE_ENV_VARS_TO_CLEAR.contains(&...))` for the new keys.
- **MIRROR**: Phase 1's `PROTON_VERB` entry at env.rs:35 — identical trailing-comment format explaining the cleared-for-direct-Proton / set-per-command split.
- **IMPORTS**: None new.
- **GOTCHA**: The shell helper `steam-host-trainer-runner.sh` maintains a parallel unset list (Task 1.3). `env.rs` has a comment about "Keep in sync" — both tasks must ship together or the CI lint job enforcing parity will fail.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::env::tests` expects all four tests green, with `wine_env_vars_match_expected_list` passing the new length of 34.

### Task 1.3: Shell-helper parity — add `unset` entries — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend the `unset` block in `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh` to include the two new keys.
- **IMPLEMENT**: Locate the `unset` block (around lines 444-465; Phase 1 added `PROTON_VERB` there). Append `unset STEAM_COMPAT_LIBRARY_PATHS` and `unset PRESSURE_VESSEL_FILESYSTEMS_RW` adjacent to the `PROTON_VERB` unset line. Preserve existing ordering/grouping. Match the existing indent and trailing comment style (if any). Do NOT `export` — this block clears host-inherited values before the Rust path sets its own.
- **MIRROR**: The Phase 1 change to this same file added the `unset PROTON_VERB` line in the same location; follow that diff shape.
- **IMPORTS**: Shell script — no imports.
- **GOTCHA**: The shell-helper's purpose is to clear host bleed on the `steam_applaunch` path; direct Proton non-Steam launches do NOT run this script. Still required for parity per the "Keep in sync" comment in `env.rs`. Run `shellcheck` locally: `shellcheck src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh` expects no new warnings.
- **VALIDATE**: `./scripts/lint.sh` passes the shellcheck stage. Manually grep confirms: `grep -n "STEAM_COMPAT_LIBRARY_PATHS\|PRESSURE_VESSEL_FILESYSTEMS_RW" src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh` returns 2 lines, both `unset`.

### Task 2.1: Wire env inserts into both Proton builders + builder tests — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: In `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`, call `collect_pressure_vessel_paths` from both `build_proton_game_command` and `build_proton_trainer_command`, insert the resulting colon-joined value into the env map under both keys, and add three sibling tests mirroring Phase 1.
- **IMPLEMENT**: In `build_proton_game_command` (line 418, immediately after the `env.insert("PROTON_VERB"...)` line): `let pressure_vessel_paths = collect_pressure_vessel_paths(request).join(":"); env.insert("STEAM_COMPAT_LIBRARY_PATHS".to_string(), pressure_vessel_paths.clone()); env.insert("PRESSURE_VESSEL_FILESYSTEMS_RW".to_string(), pressure_vessel_paths);`. In `build_proton_trainer_command` (line 525, same relative position): identical three-line block. `build_flatpak_steam_trainer_command` (line 388-400) requires NO change — it delegates to `build_proton_trainer_command` via a cloned `LaunchRequest`, so both inserts inherit automatically. Add three tests in the existing `#[cfg(test)] mod tests` block, mirroring Phase 1 test names: `proton_game_command_sets_pressure_vessel_paths_from_request`, `proton_trainer_command_sets_pressure_vessel_paths_skipping_copy_to_prefix_trainer_dir`, `flatpak_steam_trainer_command_inherits_pressure_vessel_allowlist`.
- **MIRROR**: `REPOSITORY_PATTERN` (env assembly order) + Phase 1 test pattern at script_runner.rs:1105-1151 (game), 1286-1331 (trainer), 1333-1379 (flatpak delegation). Use `command_env_value(&command, "STEAM_COMPAT_LIBRARY_PATHS")` and `command_env_value(&command, "PRESSURE_VESSEL_FILESYSTEMS_RW")` assertions with `assert_eq!` against the exact expected colon-joined value.
- **IMPORTS**: Add `use super::runtime_helpers::collect_pressure_vessel_paths;` near the existing `runtime_helpers` imports at the top of the file (check — script_runner.rs already imports multiple helpers from this module).
- **GOTCHA**: The builder tests must construct `LaunchRequest` fixtures with concrete paths (not `tempfile::tempdir()`) because the helper reads path strings verbatim — a dynamic tempdir path makes assertions non-deterministic. Use fixed strings like `"/opt/games/TheGame/game.exe"` and `"/opt/trainers/trainer.exe"`. The flatpak-steam test must set `request.steam.proton_path` and call `build_flatpak_steam_trainer_command` (not the direct trainer builder) — inheritance proof is the critical assertion. Do NOT add the inserts to `build_flatpak_steam_trainer_command` directly (double-insert bug); inheritance is the contract.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner::tests::proton_game_command_sets_pressure_vessel_paths_from_request`, `...proton_trainer_command_sets_pressure_vessel_paths_skipping_copy_to_prefix_trainer_dir`, `...flatpak_steam_trainer_command_inherits_pressure_vessel_allowlist` — expect 3/3 green. All pre-existing builder tests (Phase 1 `PROTON_VERB` tests and earlier) remain green — zero assertion churn.

### Task 2.2: Preview parity — push both keys from `collect_runtime_proton_environment` + tests — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: In `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`, add two `env.push(PreviewEnvVar { ... })` calls inside `collect_runtime_proton_environment` immediately after the Phase 1 `PROTON_VERB` push, and add a preview test that exercises both the `launch_trainer_only` toggle and the `CopyToPrefix` gate.
- **IMPLEMENT**: After preview.rs:479 (the closing brace of the `PROTON_VERB` push): `let pressure_vessel_paths = collect_pressure_vessel_paths(request).join(":"); env.push(PreviewEnvVar { key: "STEAM_COMPAT_LIBRARY_PATHS".to_string(), value: pressure_vessel_paths.clone(), source: EnvVarSource::ProtonRuntime }); env.push(PreviewEnvVar { key: "PRESSURE_VESSEL_FILESYSTEMS_RW".to_string(), value: pressure_vessel_paths, source: EnvVarSource::ProtonRuntime });`. Add `use super::runtime_helpers::collect_pressure_vessel_paths;` at the top of the file. In the `#[cfg(test)] mod tests` block, add one test `preview_runtime_proton_env_includes_pressure_vessel_paths` asserting both keys surface for a `ProtonRun` request (game + SourceDirectory trainer), and one test `preview_runtime_proton_env_pressure_vessel_omits_trainer_under_copy_to_prefix` asserting the `CopyToPrefix` variant omits the trainer dir from the colon-joined value.
- **MIRROR**: `PREVIEW_PATTERN` — Phase 1 `PROTON_VERB` push at preview.rs:475-479 and the Phase 1 preview test at preview.rs:1461-1483 show the proof-test contract (two renders, one with `launch_trainer_only=false`, one with `=true`).
- **IMPORTS**: `use super::runtime_helpers::collect_pressure_vessel_paths;` in `preview.rs` top-of-file imports.
- **GOTCHA**: Preview dispatch at preview.rs:312-324 routes `ProtonRun` to this function; `SteamApplaunch` and `Native` do NOT go through here — the preview tests must use `ResolvedLaunchMethod::ProtonRun` request fixtures (method: `METHOD_PROTON_RUN`). Confirm no `STEAM_COMPAT_LIBRARY_PATHS` entry leaks into the `SteamApplaunch` preview branch — a negative-assertion test on `collect_steam_proton_environment` output is optional but a good hedge.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview::tests::preview_runtime_proton_env_includes_pressure_vessel_paths` and `...preview_runtime_proton_env_pressure_vessel_omits_trainer_under_copy_to_prefix` — expect 2/2 green. Existing Phase 1 preview tests remain green.

### Task 3.1: Full validation gate — Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Run the full test suite, formatter, and lint tooling. Confirm zero behavior change under direct Proton by scanning for any regressions in pre-existing tests.
- **IMPLEMENT**: Execute `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` (full crate), `cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check`, `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings`, and `./scripts/lint.sh`. Manually diff preview snapshots for a representative non-Steam profile (via `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview`) and confirm exactly +2 env entries (`STEAM_COMPAT_LIBRARY_PATHS`, `PRESSURE_VESSEL_FILESYSTEMS_RW`) appear under `ProtonRun` dispatch, zero entries change under `SteamApplaunch` / `Native`.
- **MIRROR**: Phase 1 validation gate (same commands, same expectation: no regression in direct-Proton behavior).
- **IMPORTS**: None.
- **GOTCHA**: If `cargo clippy` flags any `.clone()` on a small `String` as unnecessary, replace with `.as_str()` only if semantically safe — the pressure-vessel value must be inserted under both keys, so one `clone()` and one move is idiomatic. Do not attempt to refactor the Phase 1 `PROTON_VERB` logic or the builder signatures.
- **VALIDATE**: All 4 commands exit 0. No new clippy warnings. `./scripts/lint.sh` green (rustfmt + biome + shellcheck). `git diff --stat` shows exactly 5 files touched (see **Files to Change**).

---

## Testing Strategy

### Unit Tests

| Test                                                                                    | Input                                                                                                                                       | Expected Output                                                                                | Edge Case? |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ---------- |
| `collect_pressure_vessel_paths_empty_request_returns_empty`                             | `LaunchRequest::default()` (all strings empty)                                                                                              | `Vec::<String>::new()`                                                                         | Yes        |
| `collect_pressure_vessel_paths_game_trainer_working_dir_deduped`                        | `game_path=/opt/games/TheGame/game.exe`, `trainer_host_path=/opt/trainers/t.exe`, `SourceDirectory`, `working_directory=/opt/games/TheGame` | `["/opt/games/TheGame", "/opt/trainers"]` — working_dir collapsed into game_dir via dedup      | No         |
| `collect_pressure_vessel_paths_copy_to_prefix_omits_trainer_dir`                        | same, but `trainer_loading_mode=CopyToPrefix`                                                                                               | `["/opt/games/TheGame"]`                                                                       | No         |
| `collect_pressure_vessel_paths_empty_trainer_host_path_source_directory_omits_entry`    | `SourceDirectory` + `trainer_host_path=""`                                                                                                  | only `[dirname(game_path), working_dir]` — no empty push                                       | Yes        |
| `collect_pressure_vessel_paths_flatpak_host_prefix_normalized`                          | `game_path=/run/host/opt/games/TheGame/game.exe`                                                                                            | `["/opt/games/TheGame"]` — `normalize_flatpak_host_path` strips `/run/host` prefix             | Yes        |
| `collect_pressure_vessel_paths_root_directory_preserved`                                | `game_path=/game.exe`                                                                                                                       | `["/"]` — valid but unusual; dedup still works                                                 | Yes        |
| `wine_env_vars_match_expected_list` (updated)                                           | constant `WINE_ENV_VARS_TO_CLEAR`                                                                                                           | `.len() == 34`; contains both new keys                                                         | No         |
| `proton_game_command_sets_pressure_vessel_paths_from_request`                           | game+trainer(SourceDirectory)+working_dir request                                                                                           | both `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW` set; colon-joined dedup | No         |
| `proton_trainer_command_sets_pressure_vessel_paths_skipping_copy_to_prefix_trainer_dir` | same request with `CopyToPrefix`                                                                                                            | trainer_dir omitted; game_dir + working_dir only                                               | No         |
| `flatpak_steam_trainer_command_inherits_pressure_vessel_allowlist`                      | Steam-context trainer request with populated `steam.proton_path`                                                                            | builder delegates to trainer builder; both keys present via inheritance                        | Yes        |
| `preview_runtime_proton_env_includes_pressure_vessel_paths`                             | `METHOD_PROTON_RUN` request, `launch_trainer_only=false`                                                                                    | preview env contains both keys tagged `EnvVarSource::ProtonRuntime`                            | No         |
| `preview_runtime_proton_env_pressure_vessel_omits_trainer_under_copy_to_prefix`         | same, but `trainer_loading_mode=CopyToPrefix`                                                                                               | colon-joined value omits trainer_dir                                                           | No         |

### Edge Cases Checklist

- [x] Empty `LaunchRequest` (all path strings empty) — helper returns empty Vec; builders insert empty-string value; preview renders `""` entry (acceptable — pressure-vessel treats empty list as "no extra mounts")
- [x] Game path only, no trainer, no working_dir — 1-entry list
- [x] All three paths identical (dedup test)
- [x] `CopyToPrefix` vs `SourceDirectory` branch (enum exhaustiveness)
- [x] Flatpak host-prefixed paths (`/run/host/...`) normalize correctly
- [x] Root `/` as a valid dirname (unusual but valid)
- [x] Path containing spaces (`/games/My Game/game.exe`) — no splitting occurs; colon-joined output preserves spaces as-is
- [x] Steam applaunch dispatch does NOT emit these keys (negative assertion via preview dispatch)
- [x] `build_flatpak_steam_trainer_command` inherits via delegation (no double insert)

---

## Validation Commands

### Static Analysis

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings
```

EXPECT: Zero fmt diff, zero clippy warnings.

### Unit Tests (affected area)

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::runtime_helpers
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::env
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview
```

EXPECT: All tests pass; helper tests (6), env test (updated length), builder tests (3 new + Phase 1 unchanged), preview tests (2 new + Phase 1 unchanged).

### Full Test Suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: No regressions. All pre-Phase-2 tests remain green.

### Lint Gate

```bash
./scripts/lint.sh
```

EXPECT: rustfmt + clippy + biome + shellcheck all green; shell-helper change passes shellcheck.

### Manual Validation

- [ ] Open a non-Steam profile in the CrossHook UI, trigger Launch Preview — confirm `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW` appear under `ProtonRun` branch with `ProtonRuntime` source tag.
- [ ] Open a Steam profile, trigger Launch Preview — confirm both keys are ABSENT (preview dispatch routes to `SteamApplaunch`).
- [ ] Toggle between `SourceDirectory` and `CopyToPrefix` trainer_loading_mode in a profile — preview value updates: trainer dir appears/disappears from the colon-joined list.
- [ ] Run `./scripts/dev-native.sh`, launch a non-Steam game + trainer on direct Proton — confirm game and trainer still boot identically to pre-Phase-2 behavior (zero observable change under direct Proton).

---

## Acceptance Criteria

- [ ] `collect_pressure_vessel_paths` exists in `runtime_helpers.rs` with a `pub` signature matching `fn(&LaunchRequest) -> Vec<String>`
- [ ] `build_proton_game_command` sets both `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW` on its env map
- [ ] `build_proton_trainer_command` sets both `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW` on its env map
- [ ] `build_flatpak_steam_trainer_command` inherits both keys via delegation (no direct insert, proven by test)
- [ ] `collect_runtime_proton_environment` in `preview.rs` pushes both keys with `EnvVarSource::ProtonRuntime`
- [ ] `WINE_ENV_VARS_TO_CLEAR` length is 34 and contains both new keys
- [ ] `steam-host-trainer-runner.sh` has matching `unset` lines for both keys
- [ ] Direct Proton non-Steam launch behavior is bit-identical (smoke-test manually)
- [ ] Steam applaunch preview is UNCHANGED (negative assertion via test or manual inspection)
- [ ] All existing tests remain green; zero assertion churn in Phase 1 tests

## Completion Checklist

- [ ] Helper follows discovered `collect_*` → `Vec<String>` pattern (NAMING_CONVENTION)
- [ ] Empty-path handling matches `normalize_flatpak_host_path` convention (ERROR_HANDLING)
- [ ] No new `tracing` calls added (LOGGING_PATTERN silence)
- [ ] Env-insert position mirrors Phase 1 `PROTON_VERB` placement (REPOSITORY_PATTERN)
- [ ] Dedup preserves insertion order, not sort order (SERVICE_PATTERN)
- [ ] Test names follow Phase 1 convention: `<builder>_command_<verb>_<key>_<qualifier>`
- [ ] Preview push uses `EnvVarSource::ProtonRuntime` like Phase 1
- [ ] No hardcoded dev paths in production code (only in tests)
- [ ] `env.rs` and shell-helper stay in sync (no CI parity lint regression)
- [ ] Self-contained — no questions needed during implementation

## Risks

| Risk                                                                                            | Likelihood | Impact | Mitigation                                                                                                                                                        |
| ----------------------------------------------------------------------------------------------- | ---------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Helper accidentally emits empty string into the colon-joined list (e.g. `"/a::/b"`)             | M          | M      | Explicit non-empty guards on every candidate before push; Test 4 asserts empty `trainer_host_path` + `SourceDirectory` omits entry rather than pushing empty      |
| `CopyToPrefix` branch still emits trainer_dir because logic forgot the `match`                  | M          | M      | Test 3 (`..._copy_to_prefix_omits_trainer_dir`) is the explicit proof; reviewer must see it before approval                                                       |
| `Path::new("").parent()` returns `None` and helper panics on `.unwrap()`                        | L          | H      | Pattern `if let Some(parent) = Path::new(...).parent() { ... }` guards; unit test 5 covers empty trainer_host_path path                                           |
| `build_flatpak_steam_trainer_command` double-inserts via both delegation and a direct insert    | L          | M      | Delegation contract is explicit in Phase 1 plan; Task 2.1 notes "NO change to `build_flatpak_steam_trainer_command`"; inheritance test proves single-source truth |
| `env.rs` length assertion drifts from shell-helper unset list (CI parity regression)            | L          | L      | Tasks 1.2 + 1.3 are in the same batch and MUST ship together; completion checklist calls out sync                                                                 |
| Clippy flags the `.clone()` on the colon-joined String as unnecessary                           | L          | L      | One `clone()` + one move is the idiomatic pattern for inserting the same value under two keys; acceptable per Phase 1 precedent                                   |
| Test fixture path strings (`/opt/games/...`) drift from what future Phase 3 umu tests will need | L          | L      | Phase 3 plan can introduce additional fixtures without rewriting Phase 2 fixtures — `/opt/games/...` is reasonable and avoids `$HOME` coupling in tests           |
| Preview snapshot drift breaks frontend snapshot tests                                           | L          | M      | Frontend has no formal snapshot framework (per CLAUDE.md); preview-env rendering is Rust-tested only                                                              |
| Space-containing paths (`/games/My Game/trainer.exe`) get truncated by colon join               | L          | M      | Colon-join never splits strings; only string concatenation. No fixture collision risk — tested explicitly via Edge Cases checklist                                |

## Notes

- **Why helper returns `Vec<String>` (not a joined `String`)**: Caller owns the `.join(":")` so Task 2.1 (builders) and Task 2.2 (preview) independently consume the same helper without coupling them. Matches `collect_trainer_builtin_env_keys` precedent in the same module.
- **Why builder-level insert (not a shared `merge_pressure_vessel_paths_into_map`)**: Phase 1 set `PROTON_VERB` per-builder rather than via a shared helper because the verb differs per builder (`waitforexitandrun` vs `runinprefix`). The pressure-vessel value does NOT differ per builder (same path set), but the codebase convention already accepts the minor duplication for symmetry with Phase 1 and clarity of intent.
- **Why both keys hold the same value**: Pressure-vessel's documentation treats `STEAM_COMPAT_LIBRARY_PATHS` as the "Steam-era" name and `PRESSURE_VESSEL_FILESYSTEMS_RW` as the modern pressure-vessel-native name; both are consulted. Setting both ensures forward and backward compatibility across pressure-vessel versions shipped with different SLR builds.
- **No CLI or Tauri IPC schema change**: The two new `PreviewEnvVar` entries are indistinguishable from existing `ProtonRuntime`-tagged entries to the frontend — no new enum variants, no new struct fields. TypeScript types do not need regeneration.
- **Relationship to Phase 3**: Phase 3's umu branch will replace `"$PROTON" run <target>` with `umu-run <target>` when `use_umu = true`; the env map built in these builders flows through unchanged, and the pressure-vessel keys become active at that moment. Phase 2 is deliberately a no-op under direct Proton so the activation seam is clean.
- **Relationship to PRD Open Questions**: This phase does not touch `GAMEID` resolution, `PROTONPATH` derivation, gamescope SIGTERM, Steam Deck gaming-mode edge cases, or umu-database HTTP lookups. Those belong to Phase 3+.
