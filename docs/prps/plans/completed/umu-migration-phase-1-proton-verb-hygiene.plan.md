# Plan: umu-launcher Migration — Phase 1: PROTON_VERB Hygiene

## Summary

Establish correct `PROTON_VERB` semantics **before** any umu code path activates, so the PR #148 trainer-hang regression is architecturally impossible to recreate. The game builder emits `PROTON_VERB=waitforexitandrun`; trainer builders emit `PROTON_VERB=runinprefix`. `PROTON_VERB` joins `WINE_ENV_VARS_TO_CLEAR`, the matching shell-helper `unset` block, and the launch preview — zero observable behavior change under direct Proton today (Proton's default verb is `waitforexitandrun`, and `runinprefix` in secondary invocations is still well-formed direct-Proton).

## User Story

As a CrossHook developer preparing the umu-launcher migration, I want `PROTON_VERB` to be set per-builder with correct game-vs-trainer semantics, so that when Phase 3 activates the umu code path, trainers inject with `runinprefix` (avoiding pressure-vessel process-tree blocking) and the PR #148 regression cannot recur.

## Problem → Solution

- **Current state**: Neither `build_proton_game_command`, `build_proton_trainer_command`, nor `build_flatpak_steam_trainer_command` sets `PROTON_VERB`. Under direct Proton this works (Proton defaults to `waitforexitandrun`); under umu (PR #148) it caused trainer subprocesses to block until the game exited. No `PROTON_VERB` reference exists anywhere in `src/crosshook-native/` — greenfield insertion.
- **Desired state**: Game builder explicitly sets `PROTON_VERB=waitforexitandrun`; both trainer builders explicitly set `PROTON_VERB=runinprefix`. `PROTON_VERB` is listed in `WINE_ENV_VARS_TO_CLEAR` and unset in the sibling shell runner. Preview surfaces the verb for both game and trainer. Tests assert the value per builder. When Phase 3 later branches on `use_umu`, the verb is already correct and the regression mode is eliminated by construction.

## Metadata

- **Complexity**: Small
- **Source PRD**: `docs/prps/prds/umu-launcher-migration.prd.md`
- **PRD Phase**: Phase 1 — PROTON_VERB hygiene
- **Tracking Issue**: [#254](https://github.com/yandy-r/crosshook/issues/254) (phase tracker), [#234](https://github.com/yandy-r/crosshook/issues/234) (implementation)
- **Estimated Files**: 4

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order. File ownership is disjoint across Batch 1 so no two tasks in the same batch write to the same file.

| Batch | Tasks              | Depends On | Parallel Width |
| ----- | ------------------ | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3, 1.4 | —          | 4              |
| B2    | 2.1                | B1         | 1              |

- **Total tasks**: 5
- **Total batches**: 2
- **Max parallel width**: 4

---

## UX Design

### Before

```
Launch Preview — game
  [environment] # N vars
  WINEPREFIX = "/path/to/prefix"
  STEAM_COMPAT_DATA_PATH = "/path/to/prefix"
  GAMEID = "12345"
  ...
  (no PROTON_VERB entry)

Launch Preview — trainer
  [environment] # M vars
  WINEPREFIX = "/path/to/prefix"
  GAMEID = "12345"
  ...
  (no PROTON_VERB entry)
```

### After

```
Launch Preview — game
  [environment] # N+1 vars
  WINEPREFIX = "/path/to/prefix"
  STEAM_COMPAT_DATA_PATH = "/path/to/prefix"
  GAMEID = "12345"
  PROTON_VERB = "waitforexitandrun"
  ...

Launch Preview — trainer
  [environment] # M+1 vars
  WINEPREFIX = "/path/to/prefix"
  GAMEID = "12345"
  PROTON_VERB = "runinprefix"
  ...

[cleared_variables]
  ... PROTON_VERB ...
```

### Interaction Changes

| Touchpoint                  | Before                  | After                               | Notes                                 |
| --------------------------- | ----------------------- | ----------------------------------- | ------------------------------------- |
| Launch Preview (game)       | no `PROTON_VERB`        | `PROTON_VERB = "waitforexitandrun"` | Preview mirrors executed command env  |
| Launch Preview (trainer)    | no `PROTON_VERB`        | `PROTON_VERB = "runinprefix"`       | Distinct verb per builder is visible  |
| Executed command env (game) | inherits Proton default | explicit `waitforexitandrun`        | Idempotent under direct Proton        |
| Executed command (trainer)  | inherits Proton default | explicit `runinprefix`              | Still well-formed under direct Proton |
| `cleared_variables`         | 31 entries              | 32 entries (adds `PROTON_VERB`)     | Preview + shell-helper parity         |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                       | Lines    | Why                                                                                                     |
| -------------- | -------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | 388-566  | All three builders to modify (flatpak-trainer, game, trainer)                                           |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`             | 1-136    | `WINE_ENV_VARS_TO_CLEAR` constant + length test + sync comment                                          |
| P0 (critical)  | `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`        | 444-465  | Existing `unset …` block; "Keep in sync" comment at :446                                                |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | 267-497  | `collect_runtime_proton_environment`, `collect_steam_proton_environment`, `cleared_variables` rendering |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | 898-1235 | Test module, `command_env_value` helper (:944-953), :1048 and :1132 sibling tests                       |
| P2 (reference) | `docs/prps/prds/umu-launcher-migration.prd.md`                             | 162-168  | Phase 1 goal / scope / success signal in PRD                                                            |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | 48-90    | `host_environment_map`, `merge_runtime_proton_into_map` helpers                                         |

## External Documentation

| Topic                                | Source                                                                 | Key Takeaway                                                                                                                                                                                                         |
| ------------------------------------ | ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Proton verbs (`PROTON_VERB`)         | Proton `proton` script docs / GloriousEggroll GE-Proton notes          | `waitforexitandrun` = block until `exe` and its Wine session exit; `runinprefix` = run in the **already-initialized** prefix without spawning a new pressure-vessel; ideal for trainers attaching to a running game. |
| umu-launcher `PROTON_VERB` semantics | Open-Wine-Components/umu-launcher README + Lutris integration patterns | umu inherits `PROTON_VERB`; trainers under umu MUST use `runinprefix` or they hang until the game exits (the PR #148 failure).                                                                                       |
| Phase 1 non-regression anchor        | PRD `docs/prps/prds/umu-launcher-migration.prd.md` §Technical Risks    | "PR #148 regression recurs" → mitigation is "Phase 1 lands `PROTON_VERB=runinprefix` for trainers before any umu code path goes live."                                                                               |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### CONST_ENV_VAR_LIST (append an entry, preserve doc-comment style)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/env.rs:8-40
pub const WINE_ENV_VARS_TO_CLEAR: &[&str] = &[
    "WINESERVER",
    "WINELOADER",
    // ... 29 more entries ...
    "GAMEID", // Cleared for direct Proton; set per-command when umu-run is active
];
```

### BUILDER_ENV_INSERT — game builder (mirror `GAMEID` insert site)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:409-420
let mut env = host_environment_map();
merge_runtime_proton_into_map(&mut env, request.runtime.prefix_path.trim(),
    request.steam.steam_client_install_path.trim());
merge_optimization_and_custom_into_map(&mut env, &directives.env, &BTreeMap::new());
env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
// → PROTON_VERB insert belongs here (game builder sets "waitforexitandrun")
```

### TRAINER_BUILDER_ENV_INSERT — mirror the `GAMEID` insert in trainer builder

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:516-522
let mut env = host_environment_map();
merge_runtime_proton_into_map(&mut env, request.runtime.prefix_path.trim(),
    request.steam.steam_client_install_path.trim());
env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
// → PROTON_VERB insert belongs here (trainer builder sets "runinprefix")
```

### FLATPAK_STEAM_TRAINER_DELEGATION — inheritance via delegated builder

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:388-399
let mut direct_request = request.clone();
direct_request.method = METHOD_PROTON_RUN.to_string();
direct_request.runtime.prefix_path = normalize_flatpak_host_path(&request.steam.compatdata_path);
direct_request.runtime.proton_path = request.steam.proton_path.clone();
build_proton_trainer_command(&direct_request, log_path) // inherits PROTON_VERB=runinprefix
```

### PREVIEW_ENV_PUSH — mirror the existing `WINEPREFIX` push site

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/preview.rs:439-468
env.push(PreviewEnvVar {
    key: "WINEPREFIX".to_string(),
    value: resolved_paths.wine_prefix_path.to_string_lossy().into_owned(),
    source: EnvVarSource::ProtonRuntime,
});
// → push PROTON_VERB alongside with EnvVarSource::ProtonRuntime
```

### TEST_STRUCTURE — existing env-assertion pattern

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:1033-1045
assert_eq!(command_env_value(&command, "PROTON_NO_STEAMINPUT"), Some("1".to_string()));
assert_eq!(command_env_value(&command, "STEAM_COMPAT_DATA_PATH"),
    Some(prefix_path.to_string_lossy().into_owned()));
assert_eq!(command_env_value(&command, "GAMEID"), Some("0".to_string()));
// → add: assert_eq!(command_env_value(&command, "PROTON_VERB"), Some("waitforexitandrun".to_string()));
```

### CONST_LENGTH_TEST — bump `len()` + add `.contains(...)` assertion

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/env.rs:91-98
assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 31);  // → bump to 32
assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"WINESERVER"));
assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"VKD3D_DEBUG"));
// → add: assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_VERB"));
```

### SHELL*UNSET_BLOCK — append to the `PROTON*\*` group

```bash
# SOURCE: src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh:444-460
# Keep in sync with WINE_ENV_VARS_TO_CLEAR in crosshook-core/src/launch/env.rs.
unset PROTON_LOG PROTON_DUMP_DEBUG_COMMANDS PROTON_USE_WINED3D
unset PROTON_NO_ESYNC PROTON_NO_FSYNC PROTON_ENABLE_NVAPI
# → add PROTON_VERB to the PROTON_* group (append to one of these lines)
```

---

## Files to Change

| File                                                                     | Action | Justification                                                                                                                                                                                               |
| ------------------------------------------------------------------------ | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`           | UPDATE | Add `"PROTON_VERB"` to `WINE_ENV_VARS_TO_CLEAR`; bump length assertion 31→32 and add `.contains("PROTON_VERB")` check                                                                                       |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` | UPDATE | Insert `PROTON_VERB` in `build_proton_game_command` (→ `waitforexitandrun`) and `build_proton_trainer_command` (→ `runinprefix`); add sibling tests at :1048 / :1132 and a flatpak-trainer inheritance test |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`       | UPDATE | Push `PROTON_VERB` `PreviewEnvVar` in `collect_runtime_proton_environment` (game) and the trainer preview path (`runinprefix`); add preview test                                                            |
| `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`      | UPDATE | Add `PROTON_VERB` to the `unset` block (lines 444-460) to match `WINE_ENV_VARS_TO_CLEAR` per the "Keep in sync" comment at :446                                                                             |

## NOT Building

- **Any umu-run code path** — Phase 1 is hygiene only. `umu-run` invocation, `UmuPreference` setting, `PROTONPATH` derivation, Flatpak filesystem allowlist, pressure-vessel env plumbing: deferred to Phases 2–5.
- **Changes to `build_helper_command`** — Steam-applaunch Proton invocation (the `steam-launch-helper.sh` route) stays untouched; verb handling is internal to Steam's own runtime.
- **Changes to `steam-launch-trainer.sh` / `steam-launch-helper.sh`** — these scripts have no `unset` block today; introducing one is out of scope unless a concrete runtime leakage is demonstrated.
- **Rust `.env_remove()` infrastructure** — `WINE_ENV_VARS_TO_CLEAR` is documentation + preview + shell-parity, not an active Rust-side `env_remove` driver. No new helper to iterate the constant and call `.env_remove()` on `Command` is being introduced.
- **Modifying `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS`** — `PROTON_VERB` is NOT a user-tunable optimization; it belongs to builder-level semantics (PRD Decisions Log: "Verb semantics are inherent to game-vs-trainer distinction, not user-tunable").
- **Any frontend/UI TypeScript changes** — the preview backend already serializes `PreviewEnvVar { key, value, source }`; new entries surface automatically in existing UI.
- **Telemetry, release messaging, announcement copy** — Phase 1 is intentionally invisible to end users under direct Proton.

---

## Step-by-Step Tasks

> **File-ownership invariant**: Within any batch, no two tasks touch the same file. Batch 1 covers four disjoint files; Batch 2 is verification only.

### Task 1.1: Add `PROTON_VERB` to `WINE_ENV_VARS_TO_CLEAR` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend the constant in `env.rs` and update the length-pinning unit test.
- **IMPLEMENT**:
  - In `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`, append a new entry `"PROTON_VERB"` to `WINE_ENV_VARS_TO_CLEAR` (position: alphabetically inside the `PROTON_*` cluster, or immediately after `PROTON_ENABLE_NVAPI`, matching the existing grouping convention). Include a terse trailing comment mirroring the `GAMEID` comment style: `// Cleared for direct Proton; set per-command by builders (runinprefix for trainers, waitforexitandrun for games).`
  - In the same file's unit-test block (`wine_env_vars_match_expected_list` at :91-98), bump `assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 31);` to `32` and add `assert!(WINE_ENV_VARS_TO_CLEAR.contains(&"PROTON_VERB"));` alongside the existing `WINESERVER` / `VKD3D_DEBUG` assertions.
- **MIRROR**: `CONST_ENV_VAR_LIST`, `CONST_LENGTH_TEST` (above).
- **IMPORTS**: None new.
- **GOTCHA**: `WINE_ENV_VARS_TO_CLEAR` has no Rust-side `env_remove` consumer — adding `PROTON_VERB` here does **not** cause it to be removed from any `Command`. The downstream effects are (a) preview `cleared_variables` now lists `PROTON_VERB`, and (b) the shell helper parity comment (task 1.4) stays truthful. Do NOT add a new Rust `.env_remove()` helper — it is explicitly out of scope.
- **VALIDATE**:

  ```bash
  cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::env::tests
  ```

  EXPECT: `wine_env_vars_match_expected_list` passes with the new length. `./scripts/lint.sh` passes.

### Task 1.2: Add `PROTON_VERB` to `steam-host-trainer-runner.sh` `unset` block — Depends on [none]

- **BATCH**: B1
- **ACTION**: Mirror the `env.rs` change in the sibling shell runner so the `steam_applaunch` trainer path stays in sync.
- **IMPLEMENT**:
  - In `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`, within the `unset` block at lines 444-460, append `PROTON_VERB` to one of the existing `PROTON_*` `unset` lines (preferred insertion: end of `unset PROTON_NO_ESYNC PROTON_NO_FSYNC PROTON_ENABLE_NVAPI` → `unset PROTON_NO_ESYNC PROTON_NO_FSYNC PROTON_ENABLE_NVAPI PROTON_VERB`).
  - Do NOT introduce a new `unset` line if the existing one fits — match the grouping the file already uses.
- **MIRROR**: `SHELL_UNSET_BLOCK` (above). The "Keep in sync with `WINE_ENV_VARS_TO_CLEAR`" comment at `:446` is the contract this task satisfies.
- **IMPORTS**: N/A (shell).
- **GOTCHA**: Do NOT touch `steam-launch-trainer.sh` or `steam-launch-helper.sh` — they have no `unset` block today and introducing one is out of scope (see NOT Building). Preserve `shellcheck` cleanliness: keep one `unset` per line segment; do not mix quoting; do not add blank lines inside the block.
- **VALIDATE**:

  ```bash
  grep -n 'PROTON_VERB' src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh
  shellcheck src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh
  ./scripts/lint.sh
  ```

  EXPECT: `grep` finds exactly one match inside the `unset` block. `shellcheck` clean. `./scripts/lint.sh` passes (lint workflow runs `shellcheck` on this file).

### Task 1.3: Emit `PROTON_VERB` in launch preview env for game and trainer — Depends on [none]

- **ACTION**: Surface `PROTON_VERB` in the Launch Preview so users see which verb will be applied before they click Launch.
- **BATCH**: B1
- **IMPLEMENT**:
  - In `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`, inside `collect_runtime_proton_environment` (≈:439-468, the game-path collector) and inside the trainer-preview env assembly path, push a new `PreviewEnvVar` for `PROTON_VERB`:
    - Game preview path: `key="PROTON_VERB"`, `value="waitforexitandrun".to_string()`, `source: EnvVarSource::ProtonRuntime`.
    - Trainer preview path: `key="PROTON_VERB"`, `value="runinprefix".to_string()`, `source: EnvVarSource::ProtonRuntime`.
  - If the trainer preview path re-uses the same collector as the game path, introduce a small `verb: &'static str` parameter (or gate by `trainer: bool`) so the two sites emit distinct values. Prefer the smallest local change that preserves existing call sites.
  - Add **one** unit test in `preview.rs` (in its existing `#[cfg(test)] mod tests` block) asserting that the game-preview env list contains `PROTON_VERB = "waitforexitandrun"` and the trainer-preview env list contains `PROTON_VERB = "runinprefix"`. Follow the existing preview-test pattern (locate with `rg -n 'fn .*preview.*env' src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`).
- **MIRROR**: `PREVIEW_ENV_PUSH` (above). Exact `WINEPREFIX` / `STEAM_COMPAT_DATA_PATH` push site at `preview.rs:439-468`.
- **IMPORTS**: Likely none new — `PreviewEnvVar`, `EnvVarSource` already in-scope in `preview.rs`.
- **GOTCHA**: `EnvVarSource` has no `ProtonVerb` variant; use `ProtonRuntime` (matches the `WINEPREFIX` peer). `preview.rs` already renders `cleared_variables` from `WINE_ENV_VARS_TO_CLEAR` via task 1.1 — `PROTON_VERB` will appear there automatically; do **not** also push it into `cleared_variables` manually. `STEAM_COMPAT_APP_ID` / `GAMEID` are **not** currently rendered in preview (researcher confirmed) — do not use them as a mirror reference.
- **VALIDATE**:

  ```bash
  cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview
  ```

  EXPECT: New preview test passes; existing preview tests remain green.

### Task 1.4: Insert `PROTON_VERB` in Proton builders + add sibling tests — Depends on [none]

- **ACTION**: The core Phase 1 change — make the verb explicit at builder level for both game and trainer paths, with distinct values; add tests asserting each verb per builder.
- **BATCH**: B1
- **IMPLEMENT**:
  - In `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`:
    - Inside `build_proton_game_command` (≈:401-484), alongside `env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));` at :416, add `env.insert("PROTON_VERB".to_string(), "waitforexitandrun".to_string());`.
    - Inside `build_proton_trainer_command` (≈:486-566), alongside the `GAMEID` insert at :520 (±2), add `env.insert("PROTON_VERB".to_string(), "runinprefix".to_string());`.
    - `build_flatpak_steam_trainer_command` (:388-399) already delegates to `build_proton_trainer_command` — no direct insert is required there; inheritance suffices. Add an inline one-line comment just above the `build_proton_trainer_command(&direct_request, log_path)` call documenting that `PROTON_VERB=runinprefix` is inherited via delegation.
  - In the `#[cfg(test)] mod tests` block of `script_runner.rs`, add sibling tests:
    - Near `:1048` (inside/next to `proton_game_command_applies_optimization_wrappers_and_env` at :955 and `proton_game_custom_env_overrides_duplicate_optimization_key` at :1048): add `#[test] fn proton_game_command_sets_proton_verb_to_waitforexitandrun()` that builds via `build_proton_game_command` and asserts `command_env_value(&command, "PROTON_VERB") == Some("waitforexitandrun".to_string())`.
    - Near `:1132` (inside/next to `proton_trainer_command_ignores_game_optimization_wrappers_and_env`): add `#[test] fn proton_trainer_command_sets_proton_verb_to_runinprefix()` asserting `command_env_value(&command, "PROTON_VERB") == Some("runinprefix".to_string())`.
    - Add `#[test] fn flatpak_steam_trainer_command_inherits_proton_verb_runinprefix()` asserting the same for the flatpak delegation path. Reuse the existing `steam_request()` fixture at :916-937 for the flatpak variant if applicable, otherwise inline-construct a `LaunchRequest { ... ..Default::default() }` consistent with the existing flatpak-trainer tests in the same module.
  - Also update existing nearby assertion blocks that enumerate env values exhaustively (if any test does `None` assertions on an allow-list), so they don't accidentally assert `PROTON_VERB == None`. The researcher confirmed test assertions are **positive** (`Some(value)`) or targeted `None` (per-key) — not exhaustive — but double-check `proton_trainer_command_ignores_game_optimization_wrappers_and_env` at :1132-1233 to ensure no `assert_eq!(command_env_value(&command, "PROTON_VERB"), None)` sneaks in; that assertion would now fail.
- **MIRROR**: `BUILDER_ENV_INSERT`, `TRAINER_BUILDER_ENV_INSERT`, `FLATPAK_STEAM_TRAINER_DELEGATION`, `TEST_STRUCTURE` (above).
- **IMPORTS**: None new.
- **GOTCHA**:
  - `build_flatpak_steam_trainer_command` does NOT set env itself — it mutates a cloned `LaunchRequest` then delegates to `build_proton_trainer_command`. Do NOT duplicate the insert there; rely on delegation. The new test validates the inheritance.
  - The `log_path` parameter in `build_proton_trainer_command` is prefixed `_log_path` (unused) — do not rename; this is intentional.
  - Do NOT set `PROTON_VERB` via the optimization directive pipeline (`merge_optimization_and_custom_into_map`) or `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS`. Verb is builder-level semantics (PRD Decisions Log).
  - The test-builder `steam_request()` helper at :916-937 is NOT used by the Proton builder tests — those tests inline-construct `LaunchRequest { ... ..Default::default() }`. Follow the inline-construction pattern already established at :955 and :1132.
  - `Command` is `tokio::process::Command` — use `command.as_std().get_envs()` (via `command_env_value`) when asserting.
- **VALIDATE**:

  ```bash
  cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner
  cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
  ```

  EXPECT: All three new tests pass. All ~20 pre-existing assertions in `script_runner.rs` remain green. `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings` emits zero new warnings.

### Task 2.1: Full-suite verification + lint + format — Depends on [1.1, 1.2, 1.3, 1.4]

- **ACTION**: After all four B1 tasks land, run the canonical verification set to prove Phase 1 introduced zero regressions under direct Proton.
- **BATCH**: B2
- **IMPLEMENT**: No code change. Execute the commands in the Validation Commands section below in order. For any failure, fix in the owning task's file (not here) and re-run.
- **MIRROR**: N/A — validation only.
- **IMPORTS**: N/A.
- **GOTCHA**: If `cargo test -p crosshook-core` reveals a previously-passing test now asserts `PROTON_VERB == None` (unlikely per research but possible in tests not surfaced in the :1048 / :1132 sibling scan), return to Task 1.4's owning file (`script_runner.rs`) and update the stale `None` assertion. This must NOT be fixed in Task 2.1.
- **VALIDATE**:

  ```bash
  cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
  cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
  cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
  ./scripts/lint.sh
  ```

  EXPECT: All four commands exit 0.

---

## Testing Strategy

### Unit Tests

| Test                                                                   | Input                                         | Expected Output                                                       | Edge Case?                    |
| ---------------------------------------------------------------------- | --------------------------------------------- | --------------------------------------------------------------------- | ----------------------------- |
| `wine_env_vars_match_expected_list` (updated)                          | `WINE_ENV_VARS_TO_CLEAR` constant             | `len() == 32`; `.contains("PROTON_VERB")`                             | Length pin                    |
| `proton_game_command_sets_proton_verb_to_waitforexitandrun` (new)      | Default `LaunchRequest` + `METHOD_PROTON_RUN` | `command_env_value(&cmd, "PROTON_VERB") == Some("waitforexitandrun")` | Core game path                |
| `proton_trainer_command_sets_proton_verb_to_runinprefix` (new)         | Default `LaunchRequest` + trainer             | `command_env_value(&cmd, "PROTON_VERB") == Some("runinprefix")`       | Core trainer path             |
| `flatpak_steam_trainer_command_inherits_proton_verb_runinprefix` (new) | Flatpak Steam trainer `LaunchRequest`         | Delegated command env contains `PROTON_VERB=runinprefix`              | Delegation inheritance        |
| Preview env test (new)                                                 | Game + trainer preview requests               | Preview env list contains `PROTON_VERB` with correct verb per path    | Preview parity with execution |
| Pre-existing :955 / :1048 / :1132-:1233 tests                          | Unchanged                                     | Still green; none assert `PROTON_VERB == None`                        | Regression check              |

### Edge Cases Checklist

- [x] Empty `LaunchRequest` via `Default::default()` — both builders still set the verb.
- [x] Flatpak Steam trainer path — delegation inherits `runinprefix`, not the game's `waitforexitandrun`.
- [ ] Concurrent access — N/A (builders are pure `Command` constructors, no shared state).
- [ ] Network failure — N/A (Phase 1 is local command construction).
- [ ] Permission denied — N/A.
- [x] `PROTON_VERB` pre-set in host env — builder-level `env.insert` overrides via `BTreeMap` semantics. Host leakage prevented by `WINE_ENV_VARS_TO_CLEAR` membership (preview + shell parity).

---

## Validation Commands

### Static Analysis

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
```

EXPECT: Zero format drift, zero clippy warnings.

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::env
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner
```

EXPECT: All tests pass; new tests surface `PROTON_VERB` assertions.

### Full Test Suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: No regressions across the crate.

### Lint

```bash
./scripts/lint.sh
```

EXPECT: Rust + TS + Shell linters all green (workflow mirror of `.github/workflows/lint.yml`).

### Database Validation

N/A — Phase 1 introduces no schema or TOML-serde changes.

### Browser Validation

Optional sanity check on the Launch Preview UI:

```bash
./scripts/dev-native.sh --browser   # loopback-only browser dev mode
```

Then open the app, pick a non-Steam Proton-run profile, and confirm the Launch Preview `[environment]` section shows `PROTON_VERB = "waitforexitandrun"` for game and `PROTON_VERB = "runinprefix"` for trainer. No code change should be needed — preview inherits from builder env + explicit push in Task 1.3.

### Manual Validation

- [ ] Launch a non-Steam Proton-run game under direct Proton (no umu installed) → game starts normally, no observable change.
- [ ] Launch a trainer against a running game under direct Proton → trainer and game processes both alive simultaneously in `ps` (confirms `runinprefix` is at least as permissive as Proton's default).
- [ ] Open Launch Preview for the same profile → `[environment]` section contains `PROTON_VERB` with the correct verb per path.
- [ ] Open Launch Preview for a Steam-applaunch flatpak trainer profile → trainer preview shows `PROTON_VERB = runinprefix` via delegation.
- [ ] `grep -R PROTON_VERB src/crosshook-native/` shows matches only in: `env.rs`, `script_runner.rs`, `preview.rs`, `steam-host-trainer-runner.sh` (and their tests).

---

## Acceptance Criteria

- [ ] All 5 tasks completed (1.1, 1.2, 1.3, 1.4, 2.1).
- [ ] `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` fully green.
- [ ] `./scripts/lint.sh` green.
- [ ] `cargo clippy ... -D warnings` green.
- [ ] Preview renders `PROTON_VERB` with correct verb for both game and trainer paths (manual check or preview test).
- [ ] `WINE_ENV_VARS_TO_CLEAR` contains `"PROTON_VERB"`; `len() == 32`.
- [ ] `steam-host-trainer-runner.sh` unsets `PROTON_VERB` in the sync block (`:444-460`).
- [ ] Zero observable behavior change launching non-Steam games / trainers under direct Proton (no umu installed).
- [ ] PRD tracking issue [#234](https://github.com/yandy-r/crosshook/issues/234) updated in commit trailer (`Closes #234`) and phase tracker [#254](https://github.com/yandy-r/crosshook/issues/254) receives a linking comment.
- [ ] Commits use Conventional Commits: `feat(launch): set PROTON_VERB per builder for Proton direct path`. Per CLAUDE.md §"Internal docs commits", any changes to `docs/prps/` in the same series use `docs(internal):`.

## Completion Checklist

- [ ] Code follows discovered patterns (Patterns to Mirror).
- [ ] Error handling matches codebase style (`std::io::Result<Command>`; no new error types).
- [ ] Logging follows codebase conventions (`tracing::*`; no new log lines needed — builders already log once).
- [ ] Tests follow test patterns (`command_env_value` helper, `Some("...".to_string())` / `None` style).
- [ ] No hardcoded values (verbs are fixed-string literals; correct per PRD).
- [ ] Documentation updated — `env.rs` comment near the new constant entry; no user-facing doc change (invisible under direct Proton).
- [ ] No unnecessary scope additions (confirmed against NOT Building list).
- [ ] Self-contained — no questions needed during implementation.

## Risks

| Risk                                                                                                        | Likelihood | Impact | Mitigation                                                                                                                                                                                  |
| ----------------------------------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Existing tests assert `PROTON_VERB == None` (exhaustive absence checks)                                     | Low        | Low    | Researcher scan of :1132-1233 found only targeted per-key `None` on optimization vars. Task 2.1 catches anything missed; fix in owning file.                                                |
| Test-count pin (`len() == 31`) in `env.rs` missed                                                           | Low        | Low    | Task 1.1 explicitly bumps to 32 and adds `.contains(...)`. Validator surfaces any drift.                                                                                                    |
| Preview trainer-path uses a different collector than expected; `PROTON_VERB` push lands in the wrong branch | Medium     | Low    | Task 1.3 spec requires the implementor to grep `preview.rs` for the trainer-preview env assembly before editing; test covers both game and trainer paths.                                   |
| `build_flatpak_steam_trainer_command` delegation subtlety — someone adds a direct insert there              | Low        | Medium | Task 1.4 explicitly forbids it + adds an inheritance test that would fail if both paths set verbs redundantly in mismatched places.                                                         |
| Shell `unset PROTON_VERB` missed on a sibling runner script                                                 | Low        | Low    | NOT Building section explicitly scopes to `steam-host-trainer-runner.sh`. Researcher confirmed `steam-launch-trainer.sh` / `steam-launch-helper.sh` have no `unset` block; parity unneeded. |
| `EnvVarSource` lacks an ideal variant for verb                                                              | Low        | Low    | Use `ProtonRuntime` (matches `WINEPREFIX`/`STEAM_COMPAT_DATA_PATH` peers). Adding a new variant is out of scope.                                                                            |

## Notes

- **Architectural guarantee**: Landing `PROTON_VERB` hygiene BEFORE any umu code path activates (Phase 3 onwards) makes the PR #148 regression architecturally impossible: when Phase 3 flips the builder to emit `umu-run`, the env map already carries the correct verb per builder. No additional Phase 3 work on verbs is needed — this is the entire point of Phase 1 being the first phase of the migration.
- **Idempotent under Proton**: Proton's default `PROTON_VERB` is `waitforexitandrun`; setting it explicitly in the game builder is a no-op under direct Proton. `runinprefix` in secondary (trainer) invocations is a documented Proton verb and produces a well-formed direct-Proton command — no behavior change for users who never installed umu.
- **Commit grouping suggestion**: single commit `feat(launch): set PROTON_VERB per builder for Proton direct path` with a trailer `Closes #234`. If the implementor prefers per-file commits, they must still all land before Task 2.1 runs.
- **Parallel execution contract**: `/ycc:prp-implement --parallel` can dispatch B1's four tasks concurrently because each task owns exactly one file. B2 is strictly serial (verification against the merged state).
- **Research artifact preservation**: Three researcher discovery tables informed this plan (patterns, quality, infra). Their findings are inlined into the Patterns to Mirror, Mandatory Reading, and Gotcha sections — no follow-up research trip is required during implementation.
