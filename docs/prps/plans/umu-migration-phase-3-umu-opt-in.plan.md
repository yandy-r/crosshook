# Plan: umu Migration Phase 3 — umu Opt-In (Non-Steam Only)

> **Status (2026-04-14)**: Tasks 1.1 – 8.1 **complete**, shipped in commit `ae18b92` ("feat(launch): add umu opt-in for non-Steam launches (Phase 3)"). Tracker [#256](https://github.com/yandy-r/crosshook/issues/256) was inadvertently closed by that commit and has been re-opened.
>
> **Resume work from the Phase 3b continuation plan**: [`umu-migration-phase-3b-umu-opt-in.plan.md`](./umu-migration-phase-3b-umu-opt-in.plan.md) — covers the three newly-promoted `phase:3` issues [#247](https://github.com/yandy-r/crosshook/issues/247), [#251](https://github.com/yandy-r/crosshook/issues/251) (duplicate of #247), and [#263](https://github.com/yandy-r/crosshook/issues/263): umu-database CSV coverage warning + HTTP TTL cache.

## Summary

Introduce the first real `umu-run` code path into CrossHook's non-Steam launch runtime, behind an explicit `UmuPreference::Umu` setting. `Auto` (the default) still resolves to Proton in Phase 3 — only `UmuPreference::Umu` flips `build_proton_game_command` and `build_proton_trainer_command` to emit `umu-run <target>` with `PROTONPATH = dirname(proton_path)` instead of `"$PROTON" run <target>`. Steam contexts explicitly opt out via a private trainer-builder variant so `build_flatpak_steam_trainer_command` never crosses into the umu branch. Phase 1's `PROTON_VERB` hygiene and Phase 2's pressure-vessel allowlist (both already merged) activate exactly here.

## User Story

As an adventurous hybrid-launcher user on Arch/Fedora/CachyOS who has `umu-run` installed and wants to road-test umu-launched non-Steam games + trainers, I want to flip a single settings dropdown to `Umu` and have CrossHook emit `umu-run <exe>` (with `PROTONPATH`, `GAMEID`, `PROTON_VERB`, and the pressure-vessel allowlist correctly wired), so that I can verify the PR #148 non-regression (concurrent trainer + game PIDs) and provide signal for Phase 4's default-on flip.

## Problem → Solution

- **Current**: Every non-Steam launch runs `"$PROTON" run <exe>` with `PROTON_VERB`, `GAMEID`, and pressure-vessel allowlist already set (inert). There is no way to swap in `umu-run`. Users pay the full Proton-version management cost.
- **Desired**: A single setting (`UmuPreference { Auto, Umu, Proton }`, default `Auto`) allows users to opt into `umu-run`. When `Umu` is selected AND `umu-run` is on `PATH`, both game and non-Steam trainer builders swap program to `umu-run` and add `PROTONPATH = dirname(runtime.proton_path)`. The `"run"` argv is dropped (umu-run does not take a `run` subcommand). All other env stays identical — Phase 1 and Phase 2 plumbing activates. Steam-context trainers are routed through a private `build_proton_trainer_command` variant with `force_no_umu=true` so the cloned-request delegation from `build_flatpak_steam_trainer_command` cannot accidentally take the umu branch. Preview mirrors builder output exactly. Per-profile `runtime.umu_game_id` becomes a user-editable override for protonfix mapping, with fallback `"umu-0"` replacing today's `"0"` to match umu's canonical default.

## Metadata

- **Complexity**: Large (~25 files touched across Rust core, Rust IPC/CLI, TypeScript, and shell parity)
- **Source PRD**: `docs/prps/prds/umu-launcher-migration.prd.md`
- **PRD Phase**: Phase 3 — umu opt-in (non-Steam only)
- **Tracking issue**: [#256](https://github.com/yandy-r/crosshook/issues/256) — child issues [#236](https://github.com/yandy-r/crosshook/issues/236) (settings + per-profile umu_game_id), [#237](https://github.com/yandy-r/crosshook/issues/237) (builder branch + PROTONPATH + Steam opt-out), [#238](https://github.com/yandy-r/crosshook/issues/238) (tests / matrix / E2E concurrency), [#243](https://github.com/yandy-r/crosshook/issues/243) (PROTONPATH decision — resolved as `dirname(proton_path)`, not tag name)
- **Estimated Files**: ~25 (Rust: 8 core modules, 2 src-tauri/cli, 1 shell; TypeScript: 6; tests: 2 new + edits to existing; docs: 1 PRD footnote)

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order. Tasks touching the same file are forced into different batches.

| Batch | Tasks                        | Depends On | Parallel Width | File-ownership summary                                                                                      |
| ----- | ---------------------------- | ---------- | -------------- | ----------------------------------------------------------------------------------------------------------- |
| B1    | 1.1, 1.2, 1.3, 1.4           | —          | 4              | Foundation Rust types: `settings/mod.rs`, `profile/models.rs`, `launch/request.rs`, `launch/env.rs` + shell |
| B2    | 2.1, 2.2, 2.3, 2.4, 2.5, 2.6 | B1         | 6              | IPC + frontend wiring: `src-tauri/…/settings.rs`, TS types, `utils/launch.ts`, CLI, `SettingsPanel.tsx`     |
| B3    | 3.1                          | B1         | 1              | `script_runner.rs` slot 1 — `resolved_umu_game_id_for_env` precedence + `"umu-0"` fallback                  |
| B4    | 4.1                          | B3, B2     | 1              | `script_runner.rs` slot 2 — game builder umu branch + sibling tests                                         |
| B5    | 5.1                          | B4         | 1              | `script_runner.rs` slot 3 — trainer builder umu branch + private `force_no_umu` variant + flatpak-Steam     |
| B6    | 6.1                          | B5         | 1              | `preview.rs` — preview parity + `ProtonSetup.umu_run_path` tightening                                       |
| B7    | 7.1, 7.2                     | B5, B6     | 2              | `runtime_helpers.rs` tests, new `tests/umu_concurrent_pids.rs` integration test                             |
| B8    | 8.1                          | B7         | 1              | Validation gate + PRD status update + plan closeout                                                         |

- **Total tasks**: 16
- **Total batches**: 8
- **Max parallel width**: 6 (B2)

---

## UX Design

### Before

```
Settings → [no umu preference exists]
Launch (non-Steam game) → exec: "$PROTON" run <game.exe>
Launch (non-Steam trainer) → exec: "$PROTON" run <trainer.exe>  (PROTON_VERB=runinprefix — Phase 1)
Preview → "proton run <game.exe>"
```

### After

```
Settings → [Umu Preference: Auto | Umu | Proton]  (default Auto; Phase 3 Auto still → Proton)
Profile → Runtime → [umu_game_id: <optional>]  (protonfix override)

Launch (non-Steam game, Umu + umu-run on PATH):
  env: PROTONPATH=<dirname(proton_path)>, GAMEID=<steam_app_id|runtime.umu_game_id|"umu-0">, PROTON_VERB=waitforexitandrun
  exec: umu-run <game.exe>

Launch (non-Steam trainer, Umu + umu-run on PATH):
  env: PROTONPATH=…, GAMEID=…, PROTON_VERB=runinprefix
  exec: umu-run <trainer.exe>

Launch (Steam trainer via Flatpak → delegation): ALWAYS "$PROTON" run (Steam opt-out)
Launch (Umu selected, umu-run absent): fall back to "$PROTON" run + tracing::warn! diagnostic
Preview → "umu-run <game.exe>" or "proton run <game.exe>" — mirrors builder exactly
```

### Interaction Changes

| Touchpoint                                | Before                                             | After                                                                                                                    | Notes                                               |
| ----------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------- |
| Settings panel                            | No umu control                                     | `<select>` dropdown (Auto / Umu / Proton); persists to TOML                                                              | Mirrors `default_launch_method` dropdown style      |
| Profile Runtime section                   | `steam_app_id` field (existing)                    | `steam_app_id` + new `umu_game_id` field (optional)                                                                      | Sibling field; both `Option<String>`-shaped in TOML |
| Launch preview (non-Steam, `Umu`)         | Command: `<proton> run <exe>`                      | Command: `umu-run <exe>`; env shows new `PROTONPATH` key                                                                 | Exact parity with builder                           |
| Launch preview (Steam-via-flatpak)        | `<proton> run <exe>`                               | Unchanged (`<proton> run <exe>`) even if `Umu` is set                                                                    | Steam opt-out visible in preview                    |
| Launch preview (`Auto` or `Umu` + no umu) | `<proton> run <exe>`                               | Unchanged; `ProtonSetup.umu_run_path` is `None` even when `resolve_umu_run_path().is_some()` but preference gates it out | Preview surface tells user whether umu will be used |
| Onboarding readiness (umu-run Info entry) | "CrossHook will use it as the preferred launcher." | **Unchanged** — Phase 5 owns this copy upgrade                                                                           | Explicitly out of Phase 3 scope                     |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                          | Lines                    | Why                                                                                                                                     |
| -------------- | ----------------------------------------------------------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 389-608                  | Both builders (`build_proton_game_command`, `build_proton_trainer_command`, `build_flatpak_steam_trainer_command`) + env assembly order |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 875-918                  | `resolved_umu_game_id_for_env` + `resolve_steam_app_id_for_umu` — fallback and precedence logic                                         |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`            | 267-492                  | Preview env assembly + `build_effective_command_string` (644-689) + `build_proton_setup` (731-775)                                      |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`    | 547-608                  | `resolve_umu_run_path()` contract — how umu detection works today                                                                       |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`              | 139-390                  | `AppSettingsData`, `Default`, `Debug`, TOML load/save, existing backward-compat test pattern at 553-569                                 |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`            | 51-78, 296-321, 666-671  | `TrainerLoadingMode` enum pattern (mirror for `UmuPreference`), `RuntimeSection` + `is_empty`, `effective_profile_with` merge           |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`                | 8-104                    | `WINE_ENV_VARS_TO_CLEAR` (add `PROTONPATH`), `wine_env_vars_match_expected_list` length assertion                                       |
| P1 (important) | `docs/prps/plans/completed/umu-migration-phase-1-proton-verb-hygiene.plan.md` | all                      | Phase 1 structural template — how the per-builder env insert was landed; mirror validation commands                                     |
| P1 (important) | `docs/prps/plans/completed/umu-migration-phase-2-sandbox-allowlist.plan.md`   | all                      | Phase 2 structural template — `collect_pressure_vessel_paths`, test footprint, Batches section shape                                    |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`            | 32-100, 331-394, 927-949 | `LaunchRequest`, `RuntimeLaunchConfig`, `ValidationError::code()` (frontend-coupled), `validate_proton_run`                             |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/settings.rs`                     | 102-167                  | `SettingsSaveRequest` Rust side + `merge_settings_from_request` precedent for new optional fields                                       |
| P1 (important) | `src/crosshook-native/src/types/settings.ts`                                  | 12-93                    | `SettingsSaveRequest`, `AppSettingsData`, `DEFAULT_APP_SETTINGS`, `toSettingsSaveRequest` — four touchpoints                            |
| P1 (important) | `src/crosshook-native/src/types/launch.ts`                                    | 36-124                   | `LaunchRequest.runtime` DTO + `ProtonSetup.umu_run_path` TS shape                                                                       |
| P1 (important) | `src/crosshook-native/src/utils/launch.ts`                                    | 24-52                    | `buildProfileLaunchRequest` — currently omits `steam_app_id`; Phase 3 must add `steam_app_id` AND `umu_game_id`                         |
| P1 (important) | `src/crosshook-native/src/components/SettingsPanel.tsx`                       | 1006-1030                | Existing `<select>` dropdown pattern for `default_launch_method` — mirror for `umu_preference`                                          |
| P1 (important) | `src/crosshook-native/crates/crosshook-cli/src/main.rs`                       | 712-767                  | `launch_request_from_profile` — CLI's profile→LaunchRequest converter; Phase 3 threads `umu_game_id` here                               |
| P2 (reference) | `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`           | 450-460                  | Shell-parity `unset PROTON_VERB` block — add `unset PROTONPATH` for symmetry with `WINE_ENV_VARS_TO_CLEAR`                              |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`      | 125-151                  | Existing umu readiness Info entry — **do not touch** (Phase 5 owns)                                                                     |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/platform.rs`                  | 218-220, 774-809         | `is_flatpak_with(env_key, info_path)` and `ScopedEnv` test helper — precedent if Phase 3 tests need flatpak-on/off faking               |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/launch/test_support.rs`       | 1-34                     | `ScopedCommandSearchPath` — precedent for stubbing `umu-run` on test PATH                                                               |

## External Documentation

| Topic                             | Source                                                                          | Key Takeaway                                                                                                                    |
| --------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| umu-run PROTONPATH semantics      | `https://github.com/Open-Wine-Components/umu-launcher/blob/main/docs/umu.1.scd` | Accepts either directory path or tag name; directory path takes precedence and bypasses tag-download. PRD #243 decides dirname. |
| umu-run GAMEID fallback           | Upstream README — `umu-0` is the documented no-protonfix-match sentinel         | Align our fallback: change `"0"` → `"umu-0"` in `resolved_umu_game_id_for_env`.                                                 |
| Lutris shared-umu runtime pattern | `net.lutris.Lutris` Flathub manifest                                            | `--filesystem=xdg-data/umu:create` — **referenced for Phase 5 only**; Phase 3 does not touch Flatpak manifest.                  |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### PREFERENCE_ENUM_PATTERN (mirror for `UmuPreference`)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:51-78
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerLoadingMode { #[default] SourceDirectory, CopyToPrefix }
impl TrainerLoadingMode { pub fn as_str(&self) -> &'static str { /* … */ } }
impl FromStr for TrainerLoadingMode { /* _ => Err(...) */ }
```

### SETTINGS_FIELD_PATTERN (mirror for `AppSettingsData.umu_preference`)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:139-186
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettingsData { /* field-level #[serde(default)] or default_fn */ }
impl Default for AppSettingsData { fn default() -> Self { Self { /* per-field */ } } }
// + manual Debug impl mirrors every field (redacting secrets)
```

### RUNTIME_OPTIONAL_STRING_PATTERN (mirror for `RuntimeSection.umu_game_id`)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:296-312
#[serde(rename = "steam_app_id", default, skip_serializing_if = "String::is_empty")]
pub steam_app_id: String,
// is_empty() aggregator at models.rs:314-320 must include the new field
```

### BUILDER_ENV_INSERT_PATTERN (Phase 1/2 shape; Phase 3 extends)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:418-428
env.insert("GAMEID".to_string(), resolved_umu_game_id_for_env(request));
env.insert("PROTON_VERB".to_string(), "waitforexitandrun".to_string());  // Phase 1
let pressure_vessel_paths = collect_pressure_vessel_paths(request).join(":");
env.insert("STEAM_COMPAT_LIBRARY_PATHS".to_string(), pressure_vessel_paths.clone());  // Phase 2
env.insert("PRESSURE_VESSEL_FILESYSTEMS_RW".to_string(), pressure_vessel_paths);  // Phase 2
```

### ENV_CLEAR_LIST_PATTERN (add `PROTONPATH`)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/env.rs:8-43
"GAMEID",           // Cleared for direct Proton; set per-command when umu-run is active
"PROTON_VERB",      // Cleared; set per-command by builders
"STEAM_COMPAT_LIBRARY_PATHS",     // Phase 2
"PRESSURE_VESSEL_FILESYSTEMS_RW", // Phase 2
// Phase 3 adds: "PROTONPATH",  // Cleared; set per-command by builders when use_umu
```

### ENV_ASSERTION_TEST_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:966-975
fn command_env_value(command: &Command, key: &str) -> Option<String> {
    command.as_std().get_envs().find_map(|(env_key, env_value)|
        (env_key == std::ffi::OsStr::new(key)).then(|| env_value.map(|v| v.to_string_lossy().into_owned()))
    ).flatten()
}
// Usage: assert_eq!(command_env_value(&command, "PROTONPATH"), Some("/opt/proton/GE-Proton9-20".to_string()));
```

### BUILDER_DEBUG_LOG_PATTERN (extend, do not add new sites)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:450-459
tracing::debug!(
    configured_proton_path = request.runtime.proton_path.trim(),
    resolved_proton_path = resolved_proton_path.trim(),
    // Phase 3 adds: use_umu, umu_run_path, protonpath
    "building proton game launch",
);
```

### DEGRADED_FALLBACK_WARN_PATTERN (when `Umu` preferred but `umu-run` absent)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:155-158
tracing::warn!(
    "umu preference requested but umu-run is not on PATH; falling back to direct Proton for this launch",
);
// Then proceed with the Proton branch — matches mangohud-missing precedent (warn + continue).
```

### STEAM_OPT_OUT_DELEGATION_PATTERN (mirror-and-extend)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:389-401
let mut direct_request = request.clone();
direct_request.method = METHOD_PROTON_RUN.to_string();
direct_request.runtime.proton_path = request.steam.proton_path.clone();
// Phase 3: call build_proton_trainer_command_with_umu_override(&direct_request, log_path, /*force_no_umu=*/ true)
```

### BUILDER_SIGNATURE_PATTERN (thread `use_umu` via request, not via new public signature)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:639-668
fn resolve_launch_proton_path_with_mode(proton_path: &str, steam_client: &str, flatpak: bool) -> String { /* … */ }
fn resolve_launch_proton_path(p: &str, s: &str) -> String { resolve_launch_proton_path_with_mode(p, s, platform::is_flatpak()) }
// Phase 3 mirrors: public build_proton_trainer_command(req, log) calls private
//   build_proton_trainer_command_with_umu_override(req, log, /*force_no_umu=*/ false)
```

### TEST_NAMING_PATTERN (Phase 3 additions)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:1124-1170
#[test] fn proton_game_command_sets_proton_verb_to_waitforexitandrun() { /* … */ }
// Phase 3 will add sibling tests named:
//   proton_game_command_swaps_to_umu_run_when_umu_preferred
//   proton_trainer_command_sets_protonpath_to_proton_dirname_when_use_umu
//   proton_game_command_uses_umu_0_game_id_fallback_when_no_app_id
//   flatpak_steam_trainer_command_never_uses_umu_even_when_preferred
//   proton_game_command_falls_back_to_proton_when_umu_preferred_but_missing
//   auto_preference_resolves_to_proton_in_phase_3
```

### BACKWARD_COMPAT_SETTINGS_TEST_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:553-569
let old_toml = "auto_load_last_profile = false\nlast_used_profile = \"\"\n";
fs::write(&path, old_toml).unwrap();
let loaded = store.load().unwrap();
assert_eq!(loaded.umu_preference, UmuPreference::Auto); // Phase 3 addition
```

### FRONTEND_SETTINGS_DROPDOWN_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/SettingsPanel.tsx:1006-1030
<select
  id="default-launch-method"
  className="crosshook-input"
  value={settings.default_launch_method}
  onChange={(event) => void onPersistSettings({ default_launch_method: event.target.value })}
>
  <option value="proton_run">proton_run</option>
  {/* … */}
</select>
// Mirror for: umu_preference with three options 'auto' | 'umu' | 'proton'
```

### CONCURRENT_PID_INTEGRATION_TEST_PATTERN (greenfield — no prior art)

```rust
// Target: src/crosshook-native/crates/crosshook-core/tests/umu_concurrent_pids.rs (new file)
// Structure mirrors existing tests/config_history_integration.rs (Rust integration test convention).
// Uses tokio::process::Command + a stub `umu-run` shell script on scoped PATH.
// Asserts both game-PID and trainer-PID are alive at t+500ms under UmuPreference::Umu.
```

---

## Files to Change

| File                                                                       | Action | Justification                                                                                                                                                                                                                                                                        |
| -------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | UPDATE | Add `UmuPreference` enum + `umu_preference` field to `AppSettingsData` (struct + `Default` + `Debug`) + backward-compat test                                                                                                                                                         |
| `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`         | UPDATE | Add `umu_game_id: String` to `RuntimeSection` + update `is_empty()` + roundtrip test                                                                                                                                                                                                 |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`         | UPDATE | Add `umu_game_id: String` to `RuntimeLaunchConfig` + add `umu_preference: UmuPreference` to `LaunchRequest` (both with `#[serde(default)]`)                                                                                                                                          |
| `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`             | UPDATE | Add `"PROTONPATH"` to `WINE_ENV_VARS_TO_CLEAR`; bump length assertion 34 → 35                                                                                                                                                                                                        |
| `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`        | UPDATE | Add `unset PROTONPATH` to the parity unset block for shell-runner hygiene                                                                                                                                                                                                            |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | UPDATE | `resolved_umu_game_id_for_env` precedence + `"umu-0"` fallback; `should_use_umu` helper; game + trainer umu branches; private `_with_umu_override` trainer variant; flatpak-Steam caller flips; sibling tests; split ~20 existing `"run"` assertions into Proton-branch + umu-branch |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | UPDATE | `build_effective_command_string` branches on `use_umu`; `collect_runtime_proton_environment` pushes `PROTONPATH`; `ProtonSetup.umu_run_path` tightened to reflect actual decision; preview tests                                                                                     |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATE | Add `resolve_umu_run_path` unit tests (tempdir PATH; present/absent)                                                                                                                                                                                                                 |
| `src/crosshook-native/crates/crosshook-core/tests/umu_concurrent_pids.rs`  | CREATE | New integration test — PR #148 non-regression smoke (stub game + stub trainer concurrent PIDs under umu branch)                                                                                                                                                                      |
| `src/crosshook-native/crates/crosshook-cli/src/main.rs`                    | UPDATE | `launch_request_from_profile`: thread `umu_game_id` into `RuntimeLaunchConfig`; thread `umu_preference` from loaded settings into `LaunchRequest`                                                                                                                                    |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                  | UPDATE | Add `umu_preference: Option<UmuPreference>` to Rust `SettingsSaveRequest`; extend `merge_settings_from_request`                                                                                                                                                                      |
| `src/crosshook-native/src/types/settings.ts`                               | UPDATE | Add `umu_preference: 'auto' \| 'umu' \| 'proton'` to `SettingsSaveRequest` + `AppSettingsData`; extend `DEFAULT_APP_SETTINGS` + `toSettingsSaveRequest`                                                                                                                              |
| `src/crosshook-native/src/types/launch.ts`                                 | UPDATE | Add `umu_preference?: UmuPreference`, `umu_game_id?: string` + already-missing `steam_app_id?: string` to `LaunchRequest.runtime`                                                                                                                                                    |
| `src/crosshook-native/src/types/profile.ts`                                | UPDATE | Add `umu_game_id?: string` to profile `RuntimeSection` type                                                                                                                                                                                                                          |
| `src/crosshook-native/src/utils/launch.ts`                                 | UPDATE | `buildProfileLaunchRequest` threads `steam_app_id`, `umu_game_id`, `umu_preference` from profile + settings                                                                                                                                                                          |
| `src/crosshook-native/src/components/SettingsPanel.tsx`                    | UPDATE | Add `<select>` dropdown for `umu_preference` (Auto / Umu / Proton)                                                                                                                                                                                                                   |
| `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                    | UPDATE | Mock preview handler — echo new preview surface so browser dev mode sees umu branch                                                                                                                                                                                                  |
| `docs/prps/prds/umu-launcher-migration.prd.md`                             | UPDATE | Phase 3 row: status `pending` → `in-progress`, add PRP plan link. Add footnote under Open Questions §PROTONPATH recording the #243 decision: "dirname(proton_path); tag-name rejected — see plan"                                                                                    |

## NOT Building

- **No `UmuPreference::Auto` default-on flip** — Phase 4 owns this. In Phase 3, `Auto` resolves to Proton. Only explicit `Umu` opts in.
- **No `org.openwinecomponents.umu.umu-launcher` Flathub manifest change** — Phase 5.
- **No `packaging/flatpak/dev.crosshook.CrossHook.yml` `--filesystem=xdg-data/umu:create` addition** — Phase 5.
- **No onboarding readiness upgrade** (`onboarding/readiness.rs:125-151` Info message stays) — Phase 5.
- **No `install_nag_dismissed_at: Option<DateTime>` settings field** — Phase 5 scope per PRD line 241.
- **No exported-launcher-script `command -v umu-run` probe** (`export/launcher.rs:521-551`) — Phase 4.
- **No `LocalOverride.runtime.umu_game_id`** — PRD Storage Boundary classifies this as "future" (line 243). Keep `umu_game_id` in base `RuntimeSection` only.
- **No removal of `"$PROTON" run` fallback path** — Phase 6 (time-gated end state).
- **No HTTP umu-database `GAMEID` resolver or SQLite cache** — tracked as future issue [#247](https://github.com/yandy-r/crosshook/issues/247).
- **No Steam-profile umu migration** — explicitly out-of-scope per PRD "What We're NOT Building" and tracked in [#248](https://github.com/yandy-r/crosshook/issues/248).
- **No new `ValidationError` variant for `UmuRunUnavailable`** — PRD §Degraded line 260-263 says `Auto` silently falls back to Proton and `Umu`+missing logs a warn. No blocking validation. (Re-evaluate only if Phase 4 matrix tests surface a user-visibility gap.)
- **No change to `resolve_umu_run_path()` behavior** — returns `Option<String>` as-is; Phase 3 only adds a gate downstream + adds unit tests.
- **No watchdog / pressure-vessel teardown changes** — if gamescope SIGTERM propagation turns out broken, tracked as issue [#244](https://github.com/yandy-r/crosshook/issues/244), not addressed here.
- **No CLAUDE.md or AGENTS.md changes** — no new conventions introduced beyond what PRD already documents.
- **No `chrono::DateTime` usage** — Phase 3 fields (`UmuPreference` enum + `umu_game_id: String`) are pure primitives. `chrono` already present but unused here.

---

## Step-by-Step Tasks

### Task 1.1: `UmuPreference` enum + `umu_preference` field in `AppSettingsData` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Define `UmuPreference { Auto, Umu, Proton }` enum in `settings/mod.rs` (mirror `TrainerLoadingMode`). Add field `pub umu_preference: UmuPreference` to `AppSettingsData`. Update `Default::default()`, the manual `Debug` impl's struct-field list, and add a backward-compat test.
- **IMPLEMENT**: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)] #[serde(rename_all = "snake_case")] pub enum UmuPreference { #[default] Auto, Umu, Proton }`. Impl `as_str()` returning `"auto"|"umu"|"proton"` and `FromStr` with `_ => Err(format!("unsupported umu preference: {s}"))`. Add field with implicit `#[serde(default)]` (inherited from struct-level); set `Default::default().umu_preference = UmuPreference::Auto`.
- **MIRROR**: `PREFERENCE_ENUM_PATTERN`, `SETTINGS_FIELD_PATTERN`, `BACKWARD_COMPAT_SETTINGS_TEST_PATTERN`.
- **IMPORTS**: `use std::str::FromStr;` — everything else already imported in `settings/mod.rs`.
- **GOTCHA**: `#[serde(default)]` on the top-level struct means each field inherits default behavior — no per-field `default = "…"` needed for enum fields whose type `impl Default`. But the `Debug` impl at `settings/mod.rs:223-230` is **manual** (redacts `steamgriddb_api_key`) — adding the field to `Default` without also adding it to `Debug` will compile-warn because of `non_exhaustive` match behaviour, and will misrepresent settings in logs. Update both.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core settings::`. New backward-compat test must pass: writing TOML without `umu_preference` and asserting `loaded.umu_preference == UmuPreference::Auto`. Also test `toml::from_str::<AppSettingsData>("umu_preference = \"umu\"\n")` round-trip.

### Task 1.2: Add `umu_game_id: String` to `RuntimeSection` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Add `pub umu_game_id: String` to `RuntimeSection` in `profile/models.rs` with matching Serde attrs. Update `RuntimeSection::is_empty()` to include the new field. Add a roundtrip test.
- **IMPLEMENT**: `#[serde(rename = "umu_game_id", default, skip_serializing_if = "String::is_empty")] pub umu_game_id: String,`. Extend `is_empty()` with `&& self.umu_game_id.trim().is_empty()`.
- **MIRROR**: `RUNTIME_OPTIONAL_STRING_PATTERN`.
- **IMPORTS**: (none new).
- **GOTCHA**: Do **not** add `umu_game_id` to `LocalOverrideRuntimeSection` or to `storage_profile()` split. PRD §Storage Boundary line 243 marks that as future. The field stays in base `RuntimeSection` so it's portable across machines (unlike `prefix_path`/`proton_path`).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::models`. Roundtrip test: build a `GameProfile` with `runtime.umu_game_id = "custom-42"`, serialize to TOML, parse back, assert equal. Also assert `RuntimeSection { umu_game_id: String::new(), ..default() }.is_empty() == true` and with `"foo"` → `false`.

### Task 1.3: Add `umu_game_id` to `RuntimeLaunchConfig` + `umu_preference` to `LaunchRequest` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend the launch-IPC DTOs in `launch/request.rs`. `RuntimeLaunchConfig` gains `umu_game_id`; `LaunchRequest` gains top-level `umu_preference` so builders can read user intent without a settings-handle.
- **IMPLEMENT**: In `RuntimeLaunchConfig`: `#[serde(default, skip_serializing_if = "String::is_empty")] pub umu_game_id: String,` next to `steam_app_id` (line 99). In `LaunchRequest`: `#[serde(default)] pub umu_preference: UmuPreference,` — place after `custom_env_vars` (line ~67); add `use crate::settings::UmuPreference;` at top of file.
- **MIRROR**: `SETTINGS_FIELD_PATTERN`, `RUNTIME_OPTIONAL_STRING_PATTERN`.
- **IMPORTS**: `use crate::settings::UmuPreference;` in `launch/request.rs`.
- **GOTCHA**: `LaunchRequest` is `pub` and Serde-derived — adding a field with `#[serde(default)]` is backwards-compatible for any caller constructing via `..Default::default()`, which is the in-tree test convention (see `script_runner.rs:957`). Also confirm `crosshook-cli` and `src-tauri` (which are workspace crates) compile — clippy `--workspace --all-targets` catches any missed `..Default::default()` site.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::request`. Add a test: `toml::from_str::<LaunchRequest>("method = \"proton_run\"")` succeeds and returns `umu_preference == UmuPreference::Auto`. Another: `LaunchRequest { umu_preference: UmuPreference::Umu, ..Default::default() }` serializes round-trip.

### Task 1.4: Add `PROTONPATH` to `WINE_ENV_VARS_TO_CLEAR` + shell parity — Depends on [none]

- **BATCH**: B1
- **ACTION**: Add `"PROTONPATH"` entry to `WINE_ENV_VARS_TO_CLEAR` in `launch/env.rs`; bump the expected-count assertion from 34 → 35 at `env.rs:96` (or wherever `wine_env_vars_match_expected_list` lives). Add `unset PROTONPATH` to `runtime-helpers/steam-host-trainer-runner.sh` parity unset block.
- **IMPLEMENT**: Insert `"PROTONPATH", // Cleared for direct Proton; set per-command by builders when use_umu.` Keep the block alphabetized within its section (after `"PROTON_VERB"`, before pressure-vessel). Shell: mirror the existing `unset PROTON_VERB` line. Update `assert_eq!(WINE_ENV_VARS_TO_CLEAR.len(), 34);` → `35`.
- **MIRROR**: `ENV_CLEAR_LIST_PATTERN`.
- **IMPORTS**: (none).
- **GOTCHA**: The list ordering is not alphabetical throughout — inspect the existing grouping comments before placing the new entry. Also: the shell script uses `unset` only, with no analogue of Serde defaults; keep the list exactly mirrored to `WINE_ENV_VARS_TO_CLEAR` or Phase 4's exported-script parity work will inherit drift.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::env`. `shellcheck src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`. `./scripts/lint.sh` (shellcheck stage).

### Task 2.1: Tauri `SettingsSaveRequest` + merge for `umu_preference` — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Extend Rust `SettingsSaveRequest` in `src-tauri/src/commands/settings.rs` with optional `umu_preference`. Extend `merge_settings_from_request` to copy the new field using the `unwrap_or(current.…)` precedent.
- **IMPLEMENT**: `#[serde(default)] pub umu_preference: Option<UmuPreference>,`. Merge line: `umu_preference: data.umu_preference.unwrap_or(current.umu_preference),`.
- **MIRROR**: `src-tauri/src/commands/settings.rs:102-167` — `protonup_auto_suggest` is the closest precedent (Option-wrapped optional field with `unwrap_or`).
- **IMPORTS**: `use crosshook_core::settings::UmuPreference;` if not already imported in that module.
- **GOTCHA**: `SettingsSaveRequest` on the Rust side differs from `AppSettingsData` — it's the frontend-bound DTO that strips read-only fields. Keep symmetry with the TS counterpart in Task 2.2.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace`. Manual smoke: `cargo check -p crosshook` (src-tauri crate) compiles without unused-import warnings.

### Task 2.2: TypeScript settings types + defaults — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Update `src/types/settings.ts`: add `umu_preference` to both `SettingsSaveRequest` and `AppSettingsData`; extend `DEFAULT_APP_SETTINGS` and `toSettingsSaveRequest`. Export a `UmuPreference` type alias for reuse.
- **IMPLEMENT**: `export type UmuPreference = 'auto' | 'umu' | 'proton';`. Add field `umu_preference: UmuPreference;` to both interfaces. `DEFAULT_APP_SETTINGS.umu_preference = 'auto'`. `toSettingsSaveRequest` copies `umu_preference: s.umu_preference`.
- **MIRROR**: `types/settings.ts:12-93`.
- **IMPORTS**: (none — this file defines the types).
- **GOTCHA**: Four touchpoints: (1) `SettingsSaveRequest` interface, (2) `AppSettingsData` interface (extends `SettingsSaveRequest`), (3) `DEFAULT_APP_SETTINGS` constant, (4) `toSettingsSaveRequest` function. Missing any one will compile but fail IPC round-trip.
- **VALIDATE**: `npx --prefix src/crosshook-native tsc --noEmit -p src/crosshook-native`. `npx --prefix src/crosshook-native @biomejs/biome check src/crosshook-native/src/types/settings.ts`.

### Task 2.3: TypeScript `LaunchRequest.runtime` + profile runtime type — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Update `src/types/launch.ts` `LaunchRequest.runtime` to include `steam_app_id?: string` (currently **missing** — unrelated bug fix unblocked here), `umu_game_id?: string`. Add top-level `umu_preference?: UmuPreference`. Update `src/types/profile.ts` runtime section type with `umu_game_id?: string`.
- **IMPLEMENT**: `runtime: { prefix_path: string; proton_path: string; working_directory: string; steam_app_id?: string; umu_game_id?: string; };`. Import `UmuPreference` from `./settings` for the top-level preference.
- **MIRROR**: `types/launch.ts:36-124`.
- **IMPORTS**: `import type { UmuPreference } from './settings';`.
- **GOTCHA**: Adding `steam_app_id?` is a latent bug-fix the infra research flagged — `RuntimeLaunchConfig` has it in Rust but TS omits it. Surface in the plan's Notes section. Do not drop it from this change.
- **VALIDATE**: `npx --prefix src/crosshook-native tsc --noEmit -p src/crosshook-native`.

### Task 2.4: TypeScript `buildProfileLaunchRequest` wiring — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Update `src/utils/launch.ts` `buildProfileLaunchRequest` to thread `steam_app_id`, `umu_game_id` from profile runtime and `umu_preference` from settings into the assembled `LaunchRequest`.
- **IMPLEMENT**: Inside the `runtime:` block, add `steam_app_id: profile.runtime.steam_app_id ?? ''`, `umu_game_id: profile.runtime.umu_game_id ?? ''`. At the top level, add `umu_preference: settings.umu_preference`.
- **MIRROR**: `utils/launch.ts:24-52`.
- **IMPORTS**: Function may already receive `settings: AppSettingsData`. If not, signature must extend to accept it (check callers — `useLaunch*` hooks).
- **GOTCHA**: `buildProfileLaunchRequest` is called from multiple hooks; any signature change ripples. If adding a new settings argument is too invasive, read settings via the existing `useAppSettings()` hook inside the helper's caller. Prefer not to bloat the helper.
- **VALIDATE**: `npx --prefix src/crosshook-native tsc --noEmit`. Manual: open `SettingsPanel.tsx` → toggle `Umu` → navigate to a non-Steam profile → Launch Preview shows `umu-run`.

### Task 2.5: CLI `launch_request_from_profile` wiring — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Update `crosshook-cli/src/main.rs` `launch_request_from_profile` to thread `umu_game_id` into `RuntimeLaunchConfig` and load `umu_preference` from `AppSettingsData` into the top-level `LaunchRequest`.
- **IMPLEMENT**: In the `METHOD_PROTON_RUN` arm (line ~750), add `umu_game_id: profile.runtime.umu_game_id.clone()`. At request construction, set `umu_preference: settings.umu_preference` where `settings` is loaded via `SettingsStore::load_unlocked()`.
- **MIRROR**: `crosshook-cli/src/main.rs:712-767` — existing `steam_app_id` wiring is the template.
- **IMPORTS**: `use crosshook_core::settings::{AppSettingsData, SettingsStore};` (already imported in this file).
- **GOTCHA**: The CLI already loads settings elsewhere — reuse the existing load point; do not duplicate TOML reads.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace`. `cargo run -p crosshook-cli -- --help` still works.

### Task 2.6: `SettingsPanel.tsx` umu-preference dropdown — Depends on [B1]

- **BATCH**: B2
- **ACTION**: Add a `<select>` control for `umu_preference` to `SettingsPanel.tsx`, placed adjacent to the existing `default_launch_method` control. Copy: label "umu preference" with tooltip "Auto → Proton during rollout; set Umu to opt into umu-run when available; set Proton to keep direct Proton always."
- **IMPLEMENT**: Mirror lines 1006-1030. Three options: `<option value="auto">Auto</option>`, `<option value="umu">Umu</option>`, `<option value="proton">Proton</option>`. Persist via `onPersistSettings({ umu_preference: event.target.value as UmuPreference })`.
- **MIRROR**: `FRONTEND_SETTINGS_DROPDOWN_PATTERN`.
- **IMPORTS**: `import type { UmuPreference } from '@/types/settings';` (or the project's alias).
- **GOTCHA**: The cast `as UmuPreference` suppresses TS union widening from `<select>`'s `string` type — this is the pattern used for `default_launch_method` already.
- **VALIDATE**: `./scripts/dev-native.sh --browser` (browser dev mode, mocks active). Navigate to Settings, toggle preference to `Umu`, verify mock handler echoes it back (see Task 2.2 for mock update). `npx @biomejs/biome check src/crosshook-native/src/components/SettingsPanel.tsx`.

### Task 3.1: `resolved_umu_game_id_for_env` precedence + `"umu-0"` fallback — Depends on [B1]

- **BATCH**: B3
- **ACTION**: In `script_runner.rs`, update `resolve_steam_app_id_for_umu` to prefer `runtime.umu_game_id` before Steam IDs, and change `resolved_umu_game_id_for_env`'s fallback from `"0"` to `"umu-0"`. Adjust the three existing test assertions that expect `Some("0".to_string())` at lines ~1067, ~1121, ~1340.
- **IMPLEMENT**:

  ```rust
  fn resolve_steam_app_id_for_umu(request: &LaunchRequest) -> &str {
      let umu_override = request.runtime.umu_game_id.trim();
      if !umu_override.is_empty() { return umu_override; }
      let steam_id = request.steam.app_id.trim();
      if !steam_id.is_empty() { return steam_id; }
      request.runtime.steam_app_id.trim()
  }
  fn resolved_umu_game_id_for_env(request: &LaunchRequest) -> String {
      let trimmed = resolve_steam_app_id_for_umu(request).trim();
      if trimmed.is_empty() { "umu-0".to_string() } else { trimmed.to_string() }
  }
  ```

- **MIRROR**: `script_runner.rs:897-918` (existing fn shapes).
- **IMPORTS**: (none).
- **GOTCHA**: Three existing tests must flip `Some("0".to_string())` → `Some("umu-0".to_string())`. Use `rg -n 'Some\("0".to_string\(\)\)' src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` to enumerate before editing. The precedence change also needs a new test verifying `runtime.umu_game_id` beats `steam.app_id`.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner`. All existing GAMEID tests pass with updated assertions; new precedence test passes.

### Task 4.1: Game builder umu branch — Depends on [3.1, 2.3, 2.5]

- **BATCH**: B4
- **ACTION**: Add a private helper `fn should_use_umu(request: &LaunchRequest, force_no_umu: bool) -> (bool, Option<String>)` that returns `(false, None)` when `force_no_umu`, when preference is `Proton` or `Auto`, or when `resolve_umu_run_path()` returns `None`; otherwise `(true, Some(umu_run_path))`. Update `build_proton_game_command` to call it with `force_no_umu=false` and branch: when `use_umu`, replace `resolved_proton_path` with `umu_run_path`, insert `PROTONPATH = dirname(request.runtime.proton_path)` into env, and **do not** append the literal `"run"` argument to the command (umu-run consumes the exe directly). Add sibling tests.
- **IMPLEMENT**:

  ```rust
  let (use_umu, umu_run_path) = should_use_umu(request, false);
  if use_umu { env.insert("PROTONPATH".to_string(), dirname_string(&request.runtime.proton_path)); }
  let program_path = umu_run_path.as_deref().unwrap_or(resolved_proton_path.as_str());
  // Pass use_umu into build_direct_proton_command_with_wrappers_in_directory so it can skip the .arg("run") call.
  ```

  Add `fn dirname_string(p: &str) -> String { Path::new(p.trim()).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default() }`. Extend the existing `tracing::debug!` site with `use_umu` and `umu_run_path` kv fields. When `Umu` preferred but `resolve_umu_run_path()` returns `None`, emit the `DEGRADED_FALLBACK_WARN_PATTERN` warn! and proceed with direct Proton.

- **MIRROR**: `BUILDER_ENV_INSERT_PATTERN`, `BUILDER_DEBUG_LOG_PATTERN`, `DEGRADED_FALLBACK_WARN_PATTERN`, `TEST_NAMING_PATTERN`.
- **IMPORTS**: `use std::path::Path;` (already in module).
- **GOTCHA**: The existing helper `build_direct_proton_command_with_wrappers_in_directory` (around line 118) hard-codes the `"run"` argv. Either (a) add a `use_umu: bool` parameter to it and skip the `.arg("run")` call, or (b) construct the Command inline in the builder when `use_umu`. Option (a) keeps the gamescope-wrapped variant symmetric — prefer. Gamescope-wrapped builder (`build_proton_command_with_gamescope_pid_capture_in_directory`) also takes a program path — thread the same flag.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner`. New tests: `proton_game_command_swaps_to_umu_run_when_umu_preferred` (asserts program ends with `umu-run` and no `"run"` arg), `proton_game_command_sets_protonpath_to_dirname_when_use_umu`, `proton_game_command_falls_back_to_proton_when_umu_preferred_but_missing_on_path` (use `ScopedCommandSearchPath` with empty dir), `auto_preference_resolves_to_proton_in_phase_3`. Split 4-5 existing game-builder "run" assertion tests into Proton-branch asserters and add sibling umu-branch asserters.

### Task 5.1: Trainer builder umu branch + Steam opt-out via `force_no_umu` — Depends on [4.1]

- **BATCH**: B5
- **ACTION**: Introduce a private variant `fn build_proton_trainer_command_with_umu_override(request: &LaunchRequest, log_path: &Path, force_no_umu: bool) -> std::io::Result<Command>` containing the full body of today's `build_proton_trainer_command`. Keep public `build_proton_trainer_command` as a thin wrapper calling with `force_no_umu=false`. Update `build_flatpak_steam_trainer_command` to call the private variant with `force_no_umu=true`. Branch on `use_umu` identically to the game builder; set `PROTONPATH = dirname(proton_path)`. Add sibling tests including a Steam opt-out assertion.
- **IMPLEMENT**: Same `should_use_umu(request, force_no_umu)` call. Same `PROTONPATH` insert. Same warn! on degraded fallback. In `build_flatpak_steam_trainer_command` (lines 389-401), replace the final `build_proton_trainer_command(&direct_request, log_path)` with `build_proton_trainer_command_with_umu_override(&direct_request, log_path, /*force_no_umu=*/ true)`. Extend the inline comment to document the opt-out invariant.
- **MIRROR**: `BUILDER_ENV_INSERT_PATTERN`, `BUILDER_SIGNATURE_PATTERN`, `STEAM_OPT_OUT_DELEGATION_PATTERN`, `TEST_NAMING_PATTERN`.
- **IMPORTS**: (none).
- **GOTCHA**: Trainer builder passes an empty `BTreeMap` as `custom_env_vars` to the inner helper — Phase 3 PROTONPATH must come from the builder-owned `env` map, not the custom-env override path. Also: `effective_working_directory` is keyed on trainer_host_path, not game_path — both builders derive their own; no cross-contamination.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner`. New tests: `proton_trainer_command_swaps_to_umu_run_when_umu_preferred`, `proton_trainer_command_sets_protonpath_to_dirname_when_use_umu`, `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred` (set `UmuPreference::Umu` and put stub `umu-run` on PATH — assert delegation produces Proton command). Split 4-5 trainer "run" assertion tests.

### Task 6.1: Preview parity — Depends on [5.1]

- **BATCH**: B6
- **ACTION**: Update `preview.rs` so the preview mirrors the builder's umu branch exactly. Change `build_effective_command_string` to drop `["proton_path", "run"]` and push `umu-run` when `use_umu`. Extend `collect_runtime_proton_environment` to push `PROTONPATH` (source `EnvVarSource::ProtonRuntime`) when `use_umu`. Tighten `ProtonSetup.umu_run_path` so it is `Some(path)` only when the resolved preference would actually use umu (matches the builder's `should_use_umu` logic). Keep `collect_steam_proton_environment` (Steam branch) unchanged — no PROTONPATH push there. Add preview tests.
- **IMPLEMENT**:

  ```rust
  let (use_umu, umu_run_path) = should_use_umu_preview(request, is_steam_context);
  // build_effective_command_string:
  if use_umu { parts.push(umu_run_path.clone().unwrap()); }
  else { parts.push(request.runtime.proton_path.trim().to_string()); parts.push("run".to_string()); }
  ```

  Expose `should_use_umu` (or a preview-specific wrapper) as `pub(crate)` from `script_runner` so preview can call it.

- **MIRROR**: `BUILDER_ENV_INSERT_PATTERN` (env push parallel), `preview.rs:267-492`.
- **IMPORTS**: `use crate::launch::script_runner::should_use_umu;` (or re-export via `launch/mod.rs`).
- **GOTCHA**: Preview's `ProtonSetup.umu_run_path` is consumed by the TS frontend (`types/launch.ts:118-124`); changing it from "unconditional `resolve_umu_run_path()`" to "only when `use_umu==true`" is a semantic shift. Verify no UI consumer relies on the old "is umu available on host?" meaning — grep `umu_run_path` in `src/` to confirm. If any does, add a separate `ProtonSetup.umu_run_available: Option<String>` alongside.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview`. New tests: `preview_command_string_uses_umu_run_when_use_umu`, `preview_pushes_protonpath_env_when_use_umu`, `preview_steam_branch_does_not_push_protonpath`. Manual: `./scripts/dev-native.sh --browser`, flip preference to `Umu`, verify preview renders `umu-run <exe>` for non-Steam profile.

### Task 7.1: `resolve_umu_run_path` unit tests — Depends on [5.1]

- **BATCH**: B7
- **ACTION**: Add unit tests in `runtime_helpers.rs` for `resolve_umu_run_path()` using `ScopedCommandSearchPath` (or an equivalent tempdir PATH helper).
- **IMPLEMENT**: Three tests: (1) empty PATH → `None`, (2) PATH with directory containing executable `umu-run` → `Some(path)`, (3) PATH with `umu-run` that is not executable → `None`. Use `std::fs::set_permissions` + `PermissionsExt` to toggle the executable bit.
- **MIRROR**: `src/crosshook-native/crates/crosshook-core/src/launch/test_support.rs:1-34` (ScopedCommandSearchPath), and `runtime_helpers.rs` existing `collect_pressure_vessel_paths_*` test style.
- **IMPORTS**: `use std::os::unix::fs::PermissionsExt;`.
- **GOTCHA**: The scoped PATH helper holds a mutex — tests run serially within that guard. Don't `#[tokio::test]` here (unnecessary).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::runtime_helpers::tests::resolve_umu_run_path`. All three tests pass.

### Task 7.2: Concurrent-PID integration test — Depends on [5.1]

- **BATCH**: B7
- **ACTION**: Create `src/crosshook-native/crates/crosshook-core/tests/umu_concurrent_pids.rs` — PR #148 non-regression smoke. Construct two `LaunchRequest`s (game + trainer) under `UmuPreference::Umu`, build commands with `build_proton_game_command` and `build_proton_trainer_command`, swap in a stub `umu-run` shell script (via `ScopedCommandSearchPath` or a tempdir exported to `PATH`), spawn both via `tokio::process::Command`, assert both child PIDs are alive at t+500ms, then gracefully terminate.
- **IMPLEMENT**:

  ```rust
  // The stub umu-run script (created at test time):
  //   #!/bin/sh
  //   sleep 5 & echo $! ; wait
  // Spawn game + trainer concurrently; sleep 500ms; try_wait() on each returns None (still running);
  // send SIGTERM; wait for exit. Cleanup always runs.
  ```

  Use `#[tokio::test(flavor = "multi_thread")]`. Set test-only env `UMU_SKIP_RUNTIME=1` via the stub (documentary; stub doesn't check it). Document in the test's header comment how to run locally: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test umu_concurrent_pids`.

- **MIRROR**: `CONCURRENT_PID_INTEGRATION_TEST_PATTERN`, `src/crosshook-native/crates/crosshook-core/tests/config_history_integration.rs` (integration test layout).
- **IMPORTS**: `tokio::process::Command`, `std::time::Duration`, `tempfile::TempDir`.
- **GOTCHA**: CI must not require a real umu install. The stub satisfies the test; CI passes without umu on PATH. If gamescope or pressure-vessel dependencies turn out to be required (they should not be — PROTON_VERB/PROTONPATH/GAMEID are env-only; only program substitution changes), document and skip the test behind `#[cfg(target_os = "linux")]` + `#[ignore]` escape hatch. Prefer no `#[ignore]` — the stub pattern is self-contained.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test umu_concurrent_pids`. Both PIDs alive at t+500ms; cleanup terminates both; test completes in <3s.

### Task 8.1: Validation gate + PRD phase status + #243 footnote — Depends on [B7]

- **BATCH**: B8
- **ACTION**: Run full validation suite. Update PRD Phase 3 row: status `pending` → `in-progress`, add plan link. Add a PRD footnote under Open Questions recording decision #243 (`PROTONPATH = dirname(proton_path)`; tag-name rejected). Post a comment on GitHub issues #256 and #243 linking to this plan.
- **IMPLEMENT**: Manual validation-command run of `./scripts/lint.sh` and full `cargo test -p crosshook-core` + workspace clippy. Edit PRD row 155 in the Implementation Phases table. Add footnote sentence under Open Questions §PROTONPATH — quote the reasoning (user-explicit choice preserved, no network, custom fork support, existing Flatpak path resolution). Close #243 via PR body `Resolves #243`.
- **MIRROR**: Phase 2 plan's closeout pattern — `docs/prps/plans/completed/umu-migration-phase-2-sandbox-allowlist.plan.md` end-of-plan "move to `completed/` and write report" step.
- **IMPORTS**: (none).
- **GOTCHA**: Do **not** move this plan file to `completed/` in this task — that's the implementer's final step after merge (per Phase 2 precedent). Also do not touch `README.md` or `CHANGELOG.md` — `git-cliff` + release-notes manage those.
- **VALIDATE**: `./scripts/lint.sh` exits 0. `cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace` exits 0. PRD table shows Phase 3 as `in-progress` with plan link. GitHub: comment posted on #256 and #243.

---

## Testing Strategy

### Unit Tests (Rust)

| Test                                                                              | Input                                                                                    | Expected Output                                                                                 | Edge Case?                         |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------- |
| `settings_backward_compat_without_umu_preference`                                 | Legacy TOML without `umu_preference = …`                                                 | `loaded.umu_preference == UmuPreference::Auto`                                                  | Backward compat                    |
| `settings_roundtrip_umu_preference_umu`                                           | TOML `umu_preference = "umu"`                                                            | `UmuPreference::Umu`; re-serialization preserves                                                | Serde variant                      |
| `umu_preference_from_str_rejects_unknown`                                         | `"ghoti".parse::<UmuPreference>()`                                                       | `Err` with message                                                                              | Unknown variant                    |
| `runtime_section_umu_game_id_roundtrip`                                           | `GameProfile { runtime.umu_game_id = "custom-42" }`                                      | TOML round-trip equal                                                                           | —                                  |
| `runtime_section_is_empty_considers_umu_game_id`                                  | `RuntimeSection { umu_game_id: "x", ..default }`                                         | `is_empty() == false`; empty → `true`                                                           | —                                  |
| `launch_request_umu_preference_serde_default`                                     | JSON / TOML without `umu_preference`                                                     | `umu_preference == UmuPreference::Auto`                                                         | Backward compat                    |
| `wine_env_vars_match_expected_list` (updated)                                     | `WINE_ENV_VARS_TO_CLEAR.len()`                                                           | `35`                                                                                            | Constant bump                      |
| `proton_game_command_uses_umu_0_game_id_fallback`                                 | Empty `steam.app_id`, empty `runtime.steam_app_id`, empty `runtime.umu_game_id`          | `GAMEID = "umu-0"`                                                                              | Fallback change                    |
| `proton_game_command_prefers_runtime_umu_game_id_over_steam_app_id`               | `steam.app_id = "70"`, `runtime.umu_game_id = "custom-7"`                                | `GAMEID = "custom-7"`                                                                           | Precedence                         |
| `proton_game_command_swaps_to_umu_run_when_umu_preferred`                         | `UmuPreference::Umu`, stub `umu-run` on PATH                                             | `command.get_program()` ends with `/umu-run`; no `"run"` arg                                    | Primary umu swap                   |
| `proton_game_command_sets_protonpath_to_dirname_when_use_umu`                     | `UmuPreference::Umu`, `runtime.proton_path = "/opt/proton/GE-Proton9-20/proton"`         | `PROTONPATH = "/opt/proton/GE-Proton9-20"`                                                      | PROTONPATH correctness (#243)      |
| `proton_game_command_falls_back_to_proton_when_umu_preferred_but_missing_on_path` | `UmuPreference::Umu`, `umu-run` NOT on PATH                                              | `command.get_program() == resolved_proton_path`; `PROTONPATH` **not** set                       | Degraded fallback (warn!+continue) |
| `auto_preference_resolves_to_proton_in_phase_3`                                   | `UmuPreference::Auto`, stub `umu-run` on PATH                                            | Proton branch taken (Phase 3 `Auto → Proton`)                                                   | Phase 4 gate                       |
| `proton_trainer_command_swaps_to_umu_run_when_umu_preferred`                      | Same as game but trainer builder                                                         | Program ends with `/umu-run`; `PROTON_VERB=runinprefix`                                         | Trainer path                       |
| `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred`                | `method = METHOD_STEAM_APPLAUNCH`, Flatpak, `UmuPreference::Umu`, stub `umu-run` on PATH | Delegation produces direct-Proton command (`force_no_umu=true`)                                 | Steam opt-out invariant (#237)     |
| `preview_command_string_uses_umu_run_when_use_umu`                                | Same as game swap test, but via `build_launch_preview`                                   | Preview command string starts with `<umu-run>` (not `<proton_path> run`)                        | Preview parity                     |
| `preview_pushes_protonpath_env_when_use_umu`                                      | `UmuPreference::Umu`, non-Steam                                                          | `env` contains `PreviewEnvVar { key: "PROTONPATH", value: "<dirname>", source: ProtonRuntime }` | —                                  |
| `preview_steam_branch_does_not_push_protonpath`                                   | `method = METHOD_STEAM_APPLAUNCH`                                                        | No `PROTONPATH` entry                                                                           | Steam opt-out preview              |
| `proton_setup_umu_run_path_none_when_preference_is_proton`                        | `UmuPreference::Proton`, stub `umu-run` on PATH                                          | `ProtonSetup.umu_run_path == None`                                                              | Tightened semantic (#237)          |
| `resolve_umu_run_path_returns_none_on_empty_path`                                 | `PATH=""` scoped                                                                         | `None`                                                                                          | —                                  |
| `resolve_umu_run_path_returns_path_when_executable_present`                       | Tempdir with executable stub `umu-run` on scoped PATH                                    | `Some(<path>)`                                                                                  | —                                  |
| `resolve_umu_run_path_returns_none_when_not_executable`                           | Tempdir with non-exec `umu-run` on scoped PATH                                           | `None`                                                                                          | Permission bit                     |
| `umu_concurrent_pids` (integration)                                               | Stub `umu-run`; game + trainer spawned concurrently under `UmuPreference::Umu`           | Both PIDs alive at t+500ms; both exit cleanly on SIGTERM                                        | PR #148 non-regression (#238)      |

### Existing assertion splits (~15-20 tests)

| Area                                             | Before                                               | After (additive split)                                                                                              |
| ------------------------------------------------ | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `script_runner.rs` `proton_game_*` tests         | Assert program == resolved Proton path + `"run"` arg | Keep under `UmuPreference::Proton` (or default); add sibling under `UmuPreference::Umu` asserting `umu-run` program |
| `script_runner.rs` `proton_trainer_*` tests      | Same                                                 | Same sibling split                                                                                                  |
| `script_runner.rs` GAMEID `Some("0")` tests (×3) | `Some("0".to_string())`                              | `Some("umu-0".to_string())` — fallback value change                                                                 |

### Edge Cases Checklist

- [x] Empty input — covered by `runtime_section_is_empty_considers_umu_game_id`
- [x] Maximum size input — N/A (bounded string fields)
- [x] Invalid types — covered by `umu_preference_from_str_rejects_unknown`
- [x] Concurrent access — covered by `umu_concurrent_pids` integration test
- [x] Network failure (if applicable) — N/A (Phase 3 is offline; umu's SLR bootstrap is not touched)
- [x] Permission denied — covered by `resolve_umu_run_path_returns_none_when_not_executable`
- [x] Missing umu-run binary — covered by `proton_game_command_falls_back_to_proton_when_umu_preferred_but_missing_on_path`
- [x] Steam context opt-out — covered by `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred`

### TypeScript / Frontend

- No TS test framework in-tree (see `AGENTS.md` § "no configured frontend test framework"). Manual validation via `./scripts/dev-native.sh --browser`:
  1. Settings → toggle `Umu`.
  2. Open a non-Steam game profile → Launch Preview shows `umu-run <exe>` with `PROTONPATH` env entry.
  3. Toggle `Proton` → preview reverts to `<proton> run <exe>`; `PROTONPATH` disappears.
  4. Toggle `Auto` → preview still shows `<proton> run <exe>` (Phase 3 `Auto == Proton`).
  5. Open a Steam-applaunch profile under Flatpak → preview shows `<proton> run <exe>` regardless of preference (Steam opt-out).

---

## Validation Commands

### Static Analysis

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
```

EXPECT: Zero format diffs; zero clippy warnings.

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::env
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::script_runner
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::runtime_helpers
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::request
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core settings
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::models
```

EXPECT: All targeted tests pass.

### Integration Test

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test umu_concurrent_pids
```

EXPECT: Both stub PIDs alive at t+500ms; test completes in <3s; zero leaked processes.

### Full Workspace

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
./scripts/lint.sh
```

EXPECT: All tests pass across `crosshook-core`, `crosshook-cli`, `src-tauri`; lint script exits 0 (rustfmt + clippy + biome + tsc + shellcheck).

### Browser Dev Mode Validation

```bash
./scripts/dev-native.sh --browser
```

EXPECT: Settings page renders the new `umu_preference` dropdown; toggling preference updates preview for a non-Steam profile.

### Manual Validation

- [ ] Settings → select `Umu`. Preview for a non-Steam profile shows `umu-run <exe>`.
- [ ] Settings → select `Proton`. Preview shows `<proton> run <exe>`.
- [ ] Settings → select `Auto` (default). Preview shows `<proton> run <exe>` (Phase 3: Auto → Proton).
- [ ] Steam-applaunch profile under Flatpak. Preference = `Umu`. Preview still shows direct Proton (Steam opt-out).
- [ ] Profile Runtime → set `umu_game_id = "my-fix-123"`. Preview `GAMEID` row shows `my-fix-123`.
- [ ] Uninstall `umu-run` (or rename `/usr/bin/umu-run`). Preference = `Umu`. Preview/launch falls back to direct Proton; log contains the `warn!` entry `"umu preference requested but umu-run is not on PATH"`.
- [ ] CLI: `crosshook-cli launch …` for a non-Steam profile with `UmuPreference::Umu` loaded from settings → emits `umu-run` in its command preview.
- [ ] Existing non-Steam profiles load with `umu_preference` missing → default `Auto` applied; no user action required.

---

## Acceptance Criteria

- [ ] All 16 tasks completed
- [ ] All validation commands pass
- [ ] `umu_preference` field exists in `AppSettingsData`, defaults to `UmuPreference::Auto`, round-trips TOML
- [ ] `runtime.umu_game_id` field exists in `RuntimeSection` and `RuntimeLaunchConfig`, threads from profile → LaunchRequest via both TS frontend and Rust CLI
- [ ] `UmuPreference::Umu` + `umu-run` on PATH + non-Steam profile → builder emits `umu-run <target>` with `PROTONPATH = dirname(proton_path)`, `GAMEID`, `PROTON_VERB`, pressure-vessel allowlist; no `"run"` argv
- [ ] `UmuPreference::Umu` + `umu-run` absent → builder emits direct Proton + `tracing::warn!`; no `PROTONPATH` env
- [ ] `UmuPreference::Auto` in Phase 3 → always resolves to direct Proton
- [ ] `UmuPreference::Proton` → always direct Proton regardless of `umu-run` presence
- [ ] `build_flatpak_steam_trainer_command` delegation never takes umu branch (Steam opt-out) — verified by `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred`
- [ ] Preview mirrors builder output exactly (command string + env + `ProtonSetup.umu_run_path`)
- [ ] `"PROTONPATH"` added to `WINE_ENV_VARS_TO_CLEAR` (count 34 → 35) and `steam-host-trainer-runner.sh` `unset` block
- [ ] `resolved_umu_game_id_for_env` fallback is `"umu-0"` (not `"0"`); precedence: `runtime.umu_game_id → steam.app_id → runtime.steam_app_id → "umu-0"`
- [ ] `tests/umu_concurrent_pids.rs` passes in CI without requiring a real `umu-run` binary (uses stub script)
- [ ] ~15-20 existing `"$PROTON" run` / `.arg("run")` test assertions split into Proton-branch + umu-branch sibling tests
- [ ] `UmuPreference` dropdown visible in Settings panel; persists to TOML
- [ ] PRD Phase 3 row marked `in-progress` with plan link; issue #243 footnote added under Open Questions (`dirname(proton_path)`; tag-name rejected)
- [ ] No changes to: `packaging/flatpak/*.yml`, `onboarding/readiness.rs`, `export/launcher.rs`, `install_nag_dismissed_at` field, Steam-profile code paths

## Completion Checklist

- [ ] Code follows discovered patterns (all 11 `MIRROR` references above)
- [ ] Error handling: builders return `std::io::Result<Command>`; degraded-fallback uses `tracing::warn!` + continue (matches mangohud precedent, not `anyhow` / not `ValidationError`)
- [ ] Logging: single `tracing::debug!` per builder extended with `use_umu`, `umu_run_path`, `protonpath` kv fields — **no new log sites** added
- [ ] Tests follow existing naming (`<builder>_command_<verb>_<qualifier>`) and use `command_env_value` for env assertions
- [ ] No hardcoded values beyond `"umu-run"`, `"PROTONPATH"`, `"umu-0"` (documented constants)
- [ ] Documentation updated: PRD Phase 3 row + #243 footnote only — no CLAUDE.md / AGENTS.md churn
- [ ] No unnecessary scope additions (all Phase 4/5/6 items explicitly deferred in `NOT Building`)
- [ ] Self-contained — every open question from PRD Phase 3 has a decision in this plan

---

## Risks

| Risk                                                                                                               | Likelihood | Impact                                                                | Mitigation                                                                                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------------------ | ---------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| PR #148 regression recurs (trainers hang until game exit)                                                          | L          | High — blocks Phase 3 ship                                            | Phase 1's `PROTON_VERB=runinprefix` for trainers already merged; Task 7.2 `umu_concurrent_pids` is the automated non-regression smoke                                                                                    |
| `PROTONPATH = dirname(proton_path)` breaks for hand-placed Proton builds in unusual directory layouts              | L          | Medium — user opts back to `Proton`                                   | Decision #243 documents why dirname beats tag name; preview surfaces the computed value so users can verify before launch                                                                                                |
| Steam opt-out escape: `build_flatpak_steam_trainer_command` clone drops the Steam context signal                   | M          | High — Steam profiles take umu branch, two pressure-vessel containers | Private `build_proton_trainer_command_with_umu_override(..., force_no_umu=true)` variant enforces invariant by signature; test `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred` regresses on violation |
| `"0"` → `"umu-0"` GAMEID fallback change surfaces in 3 existing tests and in user-visible env                      | L          | Low — 3 test edits + release-note bullet                              | Task 3.1 enumerates the 3 assertion sites; PRD already documented `"umu-0"` as the canonical default                                                                                                                     |
| Preview `ProtonSetup.umu_run_path` semantic change ("is available" → "will be used") breaks TS UI                  | L          | Low — preview surface only                                            | Task 6.1 requires a grep of TS consumers before change; fallback is to add `umu_run_available: Option<String>` alongside                                                                                                 |
| Adding `umu_preference: UmuPreference` to `LaunchRequest` breaks a caller that does not use `..Default::default()` | L          | Medium — compile error                                                | `#[serde(default)]` + `UmuPreference::default() == Auto` covers Serde; `--workspace --all-targets` clippy catches direct struct literals                                                                                 |
| Concurrent-PID test flaky under CI load (500ms threshold too tight)                                                | L          | Low — test flake                                                      | Start with 500ms; if flaky, bump to 1000ms and document. Stub script sleeps 5s so there's headroom.                                                                                                                      |
| TS `buildProfileLaunchRequest` caller churn (signature extended with `settings`)                                   | M          | Low — 2-3 call sites                                                  | Either extend signature (preferred — explicit) or read settings via existing hook in the caller. Biome CI catches any missed call.                                                                                       |
| Phase 1 plan said preview `umu_run_path` "stays as-is until Phase 5" — we tighten it in Phase 3                    | L          | Low — documentation drift                                             | Rationale: Phase 3 introduces the preference that gives `umu_run_path` real meaning. Note in PR description.                                                                                                             |

## Notes

- **Decision #243 recorded**: `PROTONPATH = dirname(request.runtime.proton_path)`. Tag-name form rejected because (a) it forces a tag lookup → potential network fetch, (b) breaks custom Proton forks / hand-placed builds, (c) duplicates Proton storage when umu's tag resolver downloads to its own cache, (d) parsing directory-name → tag is brittle across `GE-Proton9-20`, `Proton-9.0-4`, Valve Proton, Proton-GE-Custom variants, (e) Phase 2 already wired sandbox access to the user's Proton dir — tag mode throws that away. `resolve_launch_proton_path_with_mode` already normalizes for Flatpak host access (see `script_runner.rs:639-668`); `dirname()` of that resolved path is the minimal-change approach.
- **Latent TS bug unblocked**: `buildProfileLaunchRequest` at `src/utils/launch.ts:24-52` does not thread `steam_app_id` from profile → request, despite the Rust DTO carrying it. Task 2.3/2.4 fixes this incidentally. If this causes any preview regressions for existing Steam-app-id-aware profiles, that is a **pre-existing bug being fixed**, not introduced.
- **Research gaps noted**:
  - No existing `resolve_umu_run_path` unit tests — Task 7.1 is greenfield but low-risk.
  - No existing concurrent-PID test infrastructure — Task 7.2 establishes the pattern.
  - No existing mechanism to fake `is_flatpak` in builder tests. Phase 3 does **not** need this because the umu branch is flatpak-agnostic at the builder level (pressure-vessel env is inert under direct Proton; active under umu regardless of flatpak). If a Phase 5 test requires it, add a `resolve_launch_proton_path_with_mode`-style inner fn.
- **Batch-ordering rationale**: B1 is maximally parallel (4 distinct files, no cross-deps). B2 is the wiring layer — 6 distinct files, all parallel. B3→B4→B5 serialize on `script_runner.rs` (same-file invariant). B6 (preview) depends on B5 because preview's `should_use_umu` helper is imported from the script_runner module. B7 (integration + helper tests) depends on B5+B6 because the umu_concurrent_pids test spawns commands built by the Phase 3 builders.
- **Rollback plan**: If Phase 3 ships and an umu-branch regression surfaces, users set `UmuPreference::Proton` in settings. `Auto` still resolves to Proton (Phase 4 hasn't flipped yet), so default behavior is unchanged for any user who did not opt in. No emergency revert needed — the escape hatch is a settings flip.
- **Follow-ups after merge**:
  - Monitor `area:launch` + `feat:umu-launcher` + `type:bug` labels for 2 weeks; gate Phase 4 default-on on zero new issues.
  - Close issue #243 in the PR body.
  - When this plan is fully implemented, move the file to `docs/prps/plans/completed/umu-migration-phase-3-umu-opt-in.plan.md` and write the corresponding report at `docs/prps/reports/umu-migration-phase-3-umu-opt-in-report.md` (mirroring Phase 2's closeout pattern).
